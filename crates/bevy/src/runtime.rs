use bevy_asset::{AssetId, AssetLoadFailedEvent, AssetServer, Assets, LoadState};
use bevy_ecs::{
    entity::Entity,
    message::{MessageCursor, Messages},
    world::World,
};
use bevy_time::{Fixed, Time};
use reverse_syntax::{Diagnostic, Severity};
use reverse_vm::{Limits, Program, RuntimeError, Session, Value, Vm};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use crate::{
    bindings::{Effect, HostState, Invocation, Pending, Registry, START_ENTRY},
    VerseAsset, VerseDiagnostic, VerseLoaded, VerseOutput, VerseScript, VerseWrite,
};

type ScriptId = AssetId<VerseAsset>;

pub(crate) struct Runtime {
    pub registry: Registry,
    limits: Limits,
    pending: Pending,
    scripts: HashMap<Entity, Script>,
    compiled: HashMap<ScriptId, Rc<Compiled>>,
    failed_loads: HashMap<ScriptId, String>,
    load_reader: MessageCursor<AssetLoadFailedEvent<VerseAsset>>,
}

#[derive(Default)]
struct Script {
    attempted: Option<Rc<Compiled>>,
    reported_load_failure: Option<String>,
    active: Option<Active>,
    queued: Vec<Invocation>,
}

struct Active {
    session: Session,
    host: Rc<RefCell<HostState>>,
    checked: Rc<Checked>,
    path: String,
}

struct Compiled {
    path: String,
    source: Arc<str>,
    revision: usize,
    diagnostics: Vec<Diagnostic>,
    checked: Option<Rc<Checked>>,
}

struct Checked {
    program: Rc<Program>,
    entries: HashSet<String>,
}

impl Runtime {
    pub fn new(limits: Limits) -> Self {
        Self {
            registry: Registry::default(),
            limits,
            pending: HashMap::new(),
            scripts: HashMap::new(),
            compiled: HashMap::new(),
            failed_loads: HashMap::new(),
            load_reader: MessageCursor::default(),
        }
    }

    fn collect(&mut self, world: &World) {
        for input in &mut self.registry.inputs {
            (input.collect)(world, &mut self.pending);
        }
        for failure in self
            .load_reader
            .read(world.resource::<Messages<AssetLoadFailedEvent<VerseAsset>>>())
        {
            self.failed_loads
                .insert(failure.id, failure.error.to_string());
        }
    }

    fn compile(&mut self, id: ScriptId, path: &str, source: Arc<str>) -> Rc<Compiled> {
        if let Some(old) = self.compiled.get(&id) {
            if old.revision == self.registry.revision && Arc::ptr_eq(&old.source, &source) {
                return old.clone();
            }
        }
        self.failed_loads.remove(&id);
        let (module, report) = self.registry.analyze(&source);
        let checked = (!report.has_errors()).then(|| {
            let entries: HashSet<_> = self
                .registry
                .inputs
                .iter()
                .map(|input| input.entry.as_str())
                .chain([START_ENTRY])
                .collect();
            Rc::new(Checked {
                program: Rc::new(Program::load_with_host(&module, self.registry.names())),
                entries: report
                    .functions
                    .into_iter()
                    .map(|f| f.name)
                    .filter(|name| entries.contains(name.as_str()))
                    .collect(),
            })
        });
        let compiled = Rc::new(Compiled {
            path: path.into(),
            source,
            revision: self.registry.revision,
            diagnostics: report.diagnostics,
            checked,
        });
        self.compiled.insert(id, compiled.clone());
        compiled
    }
}

impl Script {
    fn prepare(
        &mut self,
        world: &mut World,
        entity: Entity,
        compiled: Rc<Compiled>,
        registry: &Registry,
        limits: &Limits,
        snapshot: &[Option<Value>],
    ) -> Option<Active> {
        if self
            .attempted
            .as_ref()
            .is_some_and(|old| Rc::ptr_eq(old, &compiled))
        {
            return None;
        }
        let path = &compiled.path;
        self.attempted = Some(compiled.clone());
        for d in &compiled.diagnostics {
            world.write_message(VerseDiagnostic {
                entity,
                path: path.into(),
                code: d.code,
                severity: d.severity,
                message: d.message.clone(),
                span: Some(d.span),
                help: d.help.clone(),
                fix: d.fix.clone(),
            });
        }
        let checked = compiled.checked.clone()?;
        let host = Rc::new(RefCell::new(HostState {
            snapshot: snapshot.to_vec(),
            effects: Vec::new(),
        }));
        match Vm::new_with_host(
            checked.program.clone(),
            limits.clone(),
            registry.host(host.clone()),
        ) {
            Ok(vm) => Some(Active {
                session: Session::new(vm),
                host,
                checked,
                path: path.into(),
            }),
            Err(error) => {
                report_runtime(world, entity, path, error);
                None
            }
        }
    }
}

impl Active {
    fn step(
        &mut self,
        delta: Duration,
        inputs: impl IntoIterator<Item = Invocation>,
    ) -> Result<(), RuntimeError> {
        let messages = inputs
            .into_iter()
            .filter(|(entry, _)| self.checked.entries.contains(entry));
        self.session.tick(delta, messages)?;
        Ok(())
    }
}

pub(crate) fn collect(world: &mut World) {
    let mut runtime = world
        .remove_non_send::<Runtime>()
        .expect("ReversePlugin runtime");
    runtime.collect(world);
    world.insert_non_send(runtime);
}

pub(crate) fn tick(world: &mut World) {
    let mut runtime = world
        .remove_non_send::<Runtime>()
        .expect("ReversePlugin runtime");
    runtime.collect(world);
    let delta = world.resource::<Time<Fixed>>().delta();
    let mut scripts: Vec<_> = world
        .query::<(Entity, &VerseScript)>()
        .iter(world)
        .map(|(entity, script)| (entity, script.0.clone()))
        .collect();
    scripts.sort_unstable_by_key(|(entity, _)| entity.to_bits());
    runtime.scripts.retain(|entity, _| {
        scripts
            .binary_search_by_key(&entity.to_bits(), |(entity, _)| entity.to_bits())
            .is_ok()
    });
    let live_assets: HashSet<_> = scripts.iter().map(|(_, handle)| handle.id()).collect();
    runtime.compiled.retain(|id, _| live_assets.contains(id));
    runtime
        .failed_loads
        .retain(|id, _| live_assets.contains(id));

    let mut batches = Vec::new();
    for (entity, handle) in scripts {
        let id = handle.id();
        let path = handle
            .path()
            .map_or_else(|| format!("{id:?}"), ToString::to_string);
        let source = world
            .resource::<Assets<VerseAsset>>()
            .get(id)
            .map(|asset| asset.0.clone());
        let compiled = source.map(|source| runtime.compile(id, &path, source));
        let script = runtime.scripts.entry(entity).or_default();
        let error = runtime.failed_loads.get(&id).cloned().or_else(|| {
            match world.resource::<AssetServer>().load_state(id) {
                LoadState::Failed(error) => Some(error.to_string()),
                _ => None,
            }
        });
        if let Some(error) = error {
            if script.reported_load_failure.as_ref() != Some(&error) {
                report_runtime(
                    world,
                    entity,
                    &path,
                    RuntimeError {
                        kind: "bevy_asset_load",
                        message: error.clone(),
                        span: None,
                    },
                );
                script.reported_load_failure = Some(error);
            }
        } else {
            script.reported_load_failure = None;
        }

        let snapshot: Vec<_> = runtime
            .registry
            .properties
            .iter()
            .map(|property| (property.read)(world, entity))
            .collect();
        let mut inputs = std::mem::take(&mut script.queued);
        inputs.extend(runtime.pending.remove(&entity).into_iter().flatten());
        let mut ran_startup = false;
        if let Some(mut candidate) = compiled.and_then(|compiled| {
            script.prepare(
                world,
                entity,
                compiled,
                &runtime.registry,
                &runtime.limits,
                &snapshot,
            )
        }) {
            let has_startup = candidate.checked.entries.contains(START_ENTRY);
            let startup = if has_startup {
                candidate.step(delta, [(START_ENTRY.into(), Vec::new())])
            } else {
                Ok(())
            };
            match startup {
                Ok(()) => {
                    script.active = Some(candidate);
                    ran_startup = has_startup;
                    world.write_message(VerseLoaded {
                        entity,
                        path: path.clone(),
                    });
                }
                Err(error) => report_runtime(world, entity, &path, error),
            }
        }

        let Some(active) = &mut script.active else {
            continue;
        };
        if ran_startup {
            // Startup validation and gameplay each use a tick's budget. Deliver
            // queued gameplay to the new instance on its next tick.
            script.queued = inputs;
        } else {
            active.host.borrow_mut().snapshot = snapshot;
            if let Err(error) = active.step(delta, inputs) {
                report_runtime(world, entity, &active.path, error);
                script.active = None;
                continue;
            }
        }
        for line in active.session.take_output() {
            world.write_message(VerseOutput { entity, line });
        }
        let effects = std::mem::take(&mut active.host.borrow_mut().effects);
        if !effects.is_empty() {
            batches.push((entity, active.path.clone(), effects));
        }
    }
    runtime.pending.clear();
    for (entity, path, effects) in batches {
        apply(world, &runtime.registry, entity, &path, effects);
    }
    world.insert_non_send(runtime);
}

fn apply(world: &mut World, registry: &Registry, entity: Entity, path: &str, effects: Vec<Effect>) {
    if !world.entities().contains(entity)
        || effects.iter().any(|effect| match effect {
            Effect::Write(index, _) => !(registry.properties[*index].present)(world, entity),
            Effect::Emit(_, _) => false,
        })
    {
        report_runtime(world, entity, path, RuntimeError {
            kind: "bevy_commit",
            message: "the script's entity or a component disappeared before its effects could be applied".into(),
            span: None,
        });
        return;
    }
    for effect in effects {
        match effect {
            Effect::Write(index, value) => {
                let property = &registry.properties[index];
                (property.write)(world, entity, &value);
                world.write_message(VerseWrite {
                    entity,
                    setter: property.setter.clone(),
                });
            }
            Effect::Emit(index, value) => (registry.outputs[index])(world, entity, &value),
        }
    }
}

fn report_runtime(world: &mut World, entity: Entity, path: &str, error: RuntimeError) {
    world.write_message(VerseDiagnostic {
        entity,
        path: path.into(),
        code: error.kind,
        severity: Severity::Error,
        message: error.message,
        span: error.span,
        help: None,
        fix: None,
    });
}
