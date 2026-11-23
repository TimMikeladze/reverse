use bevy_app::App;
use bevy_ecs::{
    component::{Component, Mutable},
    entity::Entity,
    message::{Message, MessageCursor, Messages},
    world::World,
};
use reverse_check::{
    analyze_with_host,
    types::{Effects, FuncSig, Ty},
    CheckResult,
};
use reverse_syntax::{ast::Module, Diagnostic, Span};
use reverse_vm::{Ctrl, Host, RuntimeError, Value};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::runtime::Runtime;

pub(crate) const START_ENTRY: &str = "OnStart";

mod scalar {
    use super::{Ty, Value};

    pub trait Scalar {
        fn ty() -> Ty;
        fn into_value(self) -> Value;
        fn from_value(value: &Value) -> Option<Self>
        where
            Self: Sized;
    }

    macro_rules! scalar {
        ($rust:ty, $variant:ident) => {
            impl Scalar for $rust {
                fn ty() -> Ty {
                    Ty::$variant
                }

                fn into_value(self) -> Value {
                    Value::$variant(self)
                }

                fn from_value(value: &Value) -> Option<Self> {
                    match value {
                        Value::$variant(value) => Some(*value),
                        _ => None,
                    }
                }
            }
        };
    }

    scalar!(bool, Logic);
    scalar!(i64, Int);
    scalar!(f64, Float);

    impl Scalar for String {
        fn ty() -> Ty {
            Ty::Str
        }

        fn into_value(self) -> Value {
            Value::Str(self.into())
        }

        fn from_value(value: &Value) -> Option<Self> {
            match value {
                Value::Str(value) => Some(value.to_string()),
                Value::Array(chars) => chars
                    .iter()
                    .map(|value| match value {
                        Value::Char(character) => Some(*character),
                        _ => None,
                    })
                    .collect(),
                _ => None,
            }
        }
    }

    impl Scalar for () {
        fn ty() -> Ty {
            Ty::Void
        }

        fn into_value(self) -> Value {
            Value::Void
        }

        fn from_value(value: &Value) -> Option<Self> {
            matches!(value, Value::Void).then_some(())
        }
    }
}

/// A value exchanged through a Verse binding. The supported Rust types are
/// `bool`, `i64`, `f64`, `String`, and `()` for messages with no payload.
pub trait VerseValue: scalar::Scalar + 'static {}

impl VerseValue for bool {}
impl VerseValue for i64 {}
impl VerseValue for f64 {}
impl VerseValue for String {}
impl VerseValue for () {}

/// Register bindings after adding [`crate::ReversePlugin`].
pub trait ReverseAppExt {
    /// Register a getter and setter on the script's entity. Both calls fail
    /// when the entity lacks `C`, so Verse calls them with square brackets.
    /// The setter must change only the registered property so speculative reads
    /// can see the value without applying changes to the component.
    fn register_verse_property<C: Component<Mutability = Mutable>, T: VerseValue>(
        &mut self,
        getter: &str,
        setter: &str,
        read: fn(&C) -> T,
        write: fn(&mut C, T),
    ) -> &mut Self;

    /// Route a Bevy message to a Verse function on the entity returned by
    /// `map`. The function takes one positional payload and returns `void`.
    /// Map to `()` for a function that takes no arguments.
    fn register_verse_input<M: Message, T: VerseValue>(
        &mut self,
        entry: &str,
        map: fn(&M) -> (Entity, T),
    ) -> &mut Self;

    /// Register a Verse function that publishes a Bevy message when its
    /// enclosing failure contexts succeed. `map` receives the script's entity.
    fn register_verse_output<M: Message, T: VerseValue>(
        &mut self,
        function: &str,
        map: fn(Entity, T) -> M,
    ) -> &mut Self;

    /// Check source against the same bindings used by in-game assets.
    fn check_verse(&self, source: &str) -> CheckResult;
}

impl ReverseAppExt for App {
    fn register_verse_property<C: Component<Mutability = Mutable>, T: VerseValue>(
        &mut self,
        getter: &str,
        setter: &str,
        read: fn(&C) -> T,
        write: fn(&mut C, T),
    ) -> &mut Self {
        assert!(T::ty() != Ty::Void, "a Verse property must have a value");
        let registry = &mut self
            .world_mut()
            .non_send_mut::<Runtime>()
            .into_inner()
            .registry;
        let index = registry.properties.len();
        registry.add_call(getter, vec![], T::ty(), Call::Read(index));
        registry.add_call(setter, vec![T::ty()], Ty::Void, Call::Write(index));
        registry.properties.push(Property {
            setter: setter.into(),
            read: Box::new(move |world, entity| {
                world
                    .get::<C>(entity)
                    .map(|component| read(component).into_value())
            }),
            present: |world, entity| world.get::<C>(entity).is_some(),
            write: Box::new(move |world, entity, value| {
                let value = T::from_value(value).expect("staged value was checked");
                write(
                    &mut world.get_mut::<C>(entity).expect("validated component"),
                    value,
                );
            }),
        });
        self
    }

    fn register_verse_input<M: Message, T: VerseValue>(
        &mut self,
        entry: &str,
        map: fn(&M) -> (Entity, T),
    ) -> &mut Self {
        check_binding_name(entry);
        self.add_message::<M>();
        let mut cursor = MessageCursor::default();
        let name = entry.to_owned();
        let registry = &mut self
            .world_mut()
            .non_send_mut::<Runtime>()
            .into_inner()
            .registry;
        assert!(
            !registry.calls.contains_key(entry),
            "Verse input `{entry}` is a registered host function"
        );
        let params = parameters::<T>();
        assert!(
            registry
                .inputs
                .iter()
                .all(|input| input.entry != entry || input.params == params),
            "the Verse input `{entry}` has conflicting payload types"
        );
        registry.inputs.push(Input {
            entry: entry.into(),
            params,
            collect: Box::new(move |world, pending| {
                for message in cursor.read(world.resource::<Messages<M>>()) {
                    let (entity, value) = map(message);
                    let args = if T::ty() == Ty::Void {
                        vec![]
                    } else {
                        vec![value.into_value()]
                    };
                    pending
                        .entry(entity)
                        .or_default()
                        .push((name.clone(), args));
                }
            }),
        });
        registry.revision += 1;
        self
    }

    fn register_verse_output<M: Message, T: VerseValue>(
        &mut self,
        function: &str,
        map: fn(Entity, T) -> M,
    ) -> &mut Self {
        self.add_message::<M>();
        let registry = &mut self
            .world_mut()
            .non_send_mut::<Runtime>()
            .into_inner()
            .registry;
        let index = registry.outputs.len();
        registry.add_call(function, parameters::<T>(), Ty::Void, Call::Emit(index));
        registry.outputs.push(Box::new(move |world, entity, value| {
            let value = T::from_value(value).expect("staged value was checked");
            world.write_message(map(entity, value));
        }));
        self
    }

    fn check_verse(&self, source: &str) -> CheckResult {
        self.world()
            .non_send::<Runtime>()
            .registry
            .analyze(source)
            .1
    }
}

fn parameters<T: VerseValue>() -> Vec<Ty> {
    match T::ty() {
        Ty::Void => vec![],
        ty => vec![ty],
    }
}

fn check_binding_name(name: &str) {
    let mut bytes = name.bytes();
    let valid = bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    assert!(
        valid && !reverse_syntax::is_keyword(name),
        "invalid Verse binding name `{name}`"
    );
    assert!(
        !reverse_check::builtins::is_builtin(name),
        "`{name}` is a Verse built-in"
    );
    assert_ne!(
        name, START_ENTRY,
        "`{START_ENTRY}` is reserved for script startup"
    );
}

pub(crate) type Invocation = (String, Vec<Value>);
pub(crate) type Pending = HashMap<Entity, Vec<Invocation>>;
type Capture = Box<dyn FnMut(&World, &mut Pending)>;
type ReadProperty = Box<dyn Fn(&World, Entity) -> Option<Value>>;
type SendEffect = Box<dyn Fn(&mut World, Entity, &Value)>;

pub(crate) struct Property {
    pub setter: String,
    pub read: ReadProperty,
    pub present: fn(&World, Entity) -> bool,
    pub write: SendEffect,
}

pub(crate) struct Input {
    pub entry: String,
    pub params: Vec<Ty>,
    pub collect: Capture,
}

#[derive(Clone, Copy)]
enum Call {
    Read(usize),
    Write(usize),
    Emit(usize),
}

#[derive(Default)]
pub(crate) struct Registry {
    pub properties: Vec<Property>,
    pub inputs: Vec<Input>,
    pub outputs: Vec<SendEffect>,
    pub revision: usize,
    calls: Rc<HashMap<String, (Call, Vec<Ty>)>>,
    signatures: Vec<FuncSig>,
}

impl Registry {
    fn add_call(&mut self, name: &str, params: Vec<Ty>, ret: Ty, call: Call) {
        check_binding_name(name);
        assert!(
            !self.calls.contains_key(name),
            "duplicate Verse host function `{name}`"
        );
        assert!(
            !self.inputs.iter().any(|input| input.entry == name),
            "`{name}` is a Verse input function"
        );
        let effects = match call {
            Call::Read(_) => Effects {
                reads: true,
                ..Effects::COMPUTES
            }
            .decides(),
            Call::Write(_) => Effects::TRANSACTS.decides(),
            Call::Emit(_) => Effects::TRANSACTS,
        };
        self.signatures
            .push(FuncSig::simple(name, &params, ret, effects));
        Rc::make_mut(&mut self.calls).insert(name.into(), (call, params));
        self.revision += 1;
    }

    pub fn analyze(&self, source: &str) -> (Module, CheckResult) {
        let (module, mut result) = analyze_with_host(source, &self.signatures);
        let mut checked_entries = std::collections::HashSet::new();
        check_entry(&mut result, START_ENTRY, &[]);
        for input in &self.inputs {
            if checked_entries.insert(&input.entry) {
                check_entry(&mut result, &input.entry, &input.params);
            }
        }
        (module, result)
    }

    pub fn names(&self) -> impl Iterator<Item = String> + '_ {
        self.signatures.iter().map(|sig| sig.name.clone())
    }

    pub fn host(&self, state: Rc<RefCell<HostState>>) -> Box<dyn Host> {
        Box::new(BevyHost {
            state,
            calls: self.calls.clone(),
        })
    }
}

fn check_entry(result: &mut CheckResult, entry: &str, params: &[Ty]) {
    let mut functions = result
        .functions
        .iter()
        .filter(|function| function.name == entry);
    let Some(function) = functions.next() else {
        return;
    };
    if function.return_type == Ty::Void
        && (entry != START_ENTRY || !function.decides)
        && function.params.len() == params.len()
        && function.params.iter().zip(params).all(|(p, ty)| {
            !p.named
                && !p.has_default
                && (p.ty == *ty
                    || matches!((&p.ty, ty), (Ty::Array(inner), Ty::Str) if **inner == Ty::Char))
        })
        && functions.next().is_none()
    {
        return;
    }
    let parameter = params.first().map_or("", |ty| match ty {
        Ty::Logic => "Payload : logic",
        Ty::Float => "Payload : float",
        Ty::Int => "Payload : int",
        Ty::Str => "Payload : string",
        _ => unreachable!("only scalar values are registered"),
    });
    result.diagnostics.push(
        Diagnostic::error(
            "V2001",
            function.name_span,
            format!("`{entry}` has the wrong signature for a Bevy callback"),
        )
        .with_help(format!("define one `{entry}({parameter}) : void` handler")),
    );
}

#[derive(Default)]
pub(crate) struct HostState {
    pub snapshot: Vec<Option<Value>>,
    pub effects: Vec<Effect>,
}

pub(crate) enum Effect {
    Write(usize, Value),
    Emit(usize, Value),
}

struct BevyHost {
    state: Rc<RefCell<HostState>>,
    calls: Rc<HashMap<String, (Call, Vec<Ty>)>>,
}

impl Host for BevyHost {
    fn call(
        &mut self,
        name: &str,
        args: &[(Option<String>, Value)],
        span: Span,
    ) -> Result<Value, Ctrl> {
        let error = || {
            Ctrl::Error(Box::new(RuntimeError {
                kind: "bevy_binding",
                message: format!("invalid call to Bevy host function `{name}`"),
                span: Some(span),
            }))
        };
        let (call, params) = self.calls.get(name).ok_or_else(error)?;
        let argument = match (params.as_slice(), args) {
            ([], []) => Value::Void,
            ([Ty::Logic], [(None, v @ Value::Logic(_))])
            | ([Ty::Int], [(None, v @ Value::Int(_))])
            | ([Ty::Float], [(None, v @ Value::Float(_))])
            | ([Ty::Str], [(None, v @ Value::Str(_))]) => v.clone(),
            ([Ty::Str], [(None, v @ Value::Array(_))]) => <String as scalar::Scalar>::from_value(v)
                .map(scalar::Scalar::into_value)
                .ok_or_else(error)?,
            _ => return Err(error()),
        };
        let mut state = self.state.borrow_mut();
        match *call {
            Call::Read(index) => {
                let value = state.effects.iter().rev().find_map(|effect| match effect {
                    Effect::Write(property, value) if *property == index => Some(value),
                    _ => None,
                });
                value
                    .or(state.snapshot[index].as_ref())
                    .cloned()
                    .ok_or(Ctrl::Fail)
            }
            Call::Write(index) => {
                if state.snapshot[index].is_none() {
                    return Err(Ctrl::Fail);
                }
                state.effects.push(Effect::Write(index, argument));
                Ok(Value::Void)
            }
            Call::Emit(index) => {
                state.effects.push(Effect::Emit(index, argument));
                Ok(Value::Void)
            }
        }
    }

    fn checkpoint(&self) -> usize {
        self.state.borrow().effects.len()
    }

    fn rollback(&mut self, checkpoint: usize) {
        self.state.borrow_mut().effects.truncate(checkpoint);
    }
}
