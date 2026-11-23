use super::{Console, Device, Edit, Instant, Reload, Station, Tone, VerseAsset, VerseScript};
use bevy::{ecs::system::SystemParam, prelude::*};
use reverse_bevy::{VerseDiagnostic, VerseLoaded};
use std::{
    cell::{Cell, RefCell},
    time::Duration,
};
use wasm_bindgen::prelude::*;

thread_local! {
    static QUEUED: RefCell<Option<Draft>> = const { RefCell::new(None) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
}

struct Draft {
    id: u32,
    station: Station,
    source: String,
}

struct Pending {
    id: u32,
    entity: Entity,
    station: Station,
    bundled: bool,
    started: Instant,
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = observatoryEditorDiagnostic)]
    fn editor_diagnostic(
        id: u32,
        code: &str,
        severity: &str,
        message: &str,
        help: &str,
        start: i32,
        end: i32,
    );

    #[wasm_bindgen(js_name = observatoryEditorFinished)]
    fn editor_finished(id: u32, applied: bool, message: &str);
}

#[wasm_bindgen]
pub fn submit_verse_edit(id: u32, station: &str, source: String) -> bool {
    let station = match station {
        "relay" => Station::Relay,
        "gate" => Station::Gate,
        "beacon" => Station::Beacon,
        _ => return false,
    };
    if BUSY.replace(true) {
        return false;
    }
    QUEUED.replace(Some(Draft {
        id,
        station,
        source,
    }));
    true
}

pub(crate) fn busy() -> bool {
    BUSY.get()
}

fn finish(id: u32, applied: bool, message: &str) {
    BUSY.set(false);
    editor_finished(id, applied, message);
}

pub(crate) fn install(app: &mut App) {
    app.add_systems(
        Update,
        edit_assets
            .after(super::reload)
            .before(super::read_plugin_messages),
    );
}

#[derive(SystemParam)]
struct RevisionEvents<'w, 's> {
    loaded: MessageReader<'w, 's, VerseLoaded>,
    diagnostics: MessageReader<'w, 's, VerseDiagnostic>,
}

fn edit_assets(
    mut pending: Local<Option<Pending>>,
    mut events: RevisionEvents,
    devices: Query<(Entity, &Device, &VerseScript)>,
    mut assets: ResMut<Assets<VerseAsset>>,
    mut reload: ResMut<Reload>,
    mut console: ResMut<Console>,
) {
    let mut activated = false;
    let mut rejected = false;
    for event in events.diagnostics.read() {
        let Some(edit) = pending.as_ref().filter(|edit| edit.entity == event.entity) else {
            continue;
        };
        let severity = event.severity.as_str();
        rejected |= severity == "error";
        let offset = |value| i32::try_from(value).unwrap_or(-1);
        let (start, end) = event
            .span
            .map_or((-1, -1), |span| (offset(span.start), offset(span.end)));
        editor_diagnostic(
            edit.id,
            event.code,
            severity,
            &event.message,
            event.help.as_deref().unwrap_or_default(),
            start,
            end,
        );
    }
    for event in events.loaded.read() {
        activated |= pending
            .as_ref()
            .is_some_and(|edit| edit.entity == event.entity);
    }

    if let Some(edit) = pending.as_ref() {
        if activated {
            if edit.station == Station::Beacon {
                reload.active_label = if edit.bundled {
                    Edit::Orbit.label()
                } else {
                    "CUSTOM"
                };
                console.revision = reload.active_label.into();
            }
            console.log(
                Tone::Good,
                format!("{} editor draft activated", edit.station.title()),
            );
            finish(edit.id, true, "Applied to the running station.");
        } else if rejected {
            finish(
                edit.id,
                false,
                "Draft rejected. The last working script is still running.",
            );
        } else if edit.started.elapsed() >= Duration::from_secs(4) {
            finish(edit.id, false, "The station did not confirm the edit.");
        } else {
            return;
        }
        pending.take();
    }

    let Some(draft) = QUEUED.take() else { return };
    if draft.station == Station::Beacon && reload.waiting_since.is_some() {
        finish(
            draft.id,
            false,
            "Wait for the beacon reload to finish before applying a draft.",
        );
        return;
    }
    let Some((entity, _, script)) = devices
        .iter()
        .find(|(_, device, _)| device.station == draft.station)
    else {
        finish(
            draft.id,
            false,
            "The station is still loading. Try again when it is ready.",
        );
        return;
    };
    if assets.get(script.0.id()).is_none() {
        finish(
            draft.id,
            false,
            "The station script is still loading. Try again when it is ready.",
        );
        return;
    }
    let bundled = draft.source == draft.station.source();
    match assets.insert(script.0.id(), VerseAsset::new(draft.source)) {
        Ok(()) => {
            *pending = Some(Pending {
                id: draft.id,
                entity,
                station: draft.station,
                bundled,
                started: Instant::now(),
            })
        }
        Err(error) => finish(draft.id, false, &error.to_string()),
    }
}
