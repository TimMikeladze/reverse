#[path = "../examples/support/door.rs"]
mod door;

use bevy_app::App;
use bevy_asset::Assets;
use bevy_ecs::{
    component::Component,
    message::{Message, MessageCursor, Messages},
};
use door::{Door, Interact, Opened};
use reverse_bevy::{
    ReverseAppExt, VerseAsset, VerseDiagnostic, VerseLoaded, VerseScript, VerseWrite,
};
use std::{
    thread,
    time::{Duration, Instant},
};

const ASSET_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/assets");

fn load(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut cursor = MessageCursor::<VerseLoaded>::default();
    loop {
        app.update();
        if cursor
            .read(app.world().resource::<Messages<VerseLoaded>>())
            .next()
            .is_some()
        {
            return;
        }
        let diagnostics = app.world().resource::<Messages<VerseDiagnostic>>();
        assert!(
            diagnostics.is_empty(),
            "the asset did not check or initialize: {:?}",
            MessageCursor::default()
                .read(diagnostics)
                .collect::<Vec<_>>()
        );
        assert!(Instant::now() < deadline, "door.verse did not load");
        thread::yield_now();
    }
}

#[test]
fn rejected_interaction_rolls_back_and_allowed_interaction_closes_after_two_ticks() {
    let mut app = door::app(ASSET_ROOT);
    let (door, _) = door::spawn(&mut app);
    load(&mut app);
    let mut writes = MessageCursor::<VerseWrite>::default();
    let mut opened = MessageCursor::<Opened>::default();

    app.world_mut().write_message(Interact {
        door,
        allowed: false,
    });
    app.update();
    assert!(!app.world().get::<Door>(door).unwrap().0);
    assert_eq!(
        writes
            .read(app.world().resource::<Messages<VerseWrite>>())
            .count(),
        0
    );
    assert_eq!(
        opened
            .read(app.world().resource::<Messages<Opened>>())
            .count(),
        0
    );

    app.world_mut().write_message(Interact {
        door,
        allowed: true,
    });
    app.update();
    assert!(app.world().get::<Door>(door).unwrap().0);
    assert_eq!(
        writes
            .read(app.world().resource::<Messages<VerseWrite>>())
            .count(),
        1
    );
    assert_eq!(
        opened
            .read(app.world().resource::<Messages<Opened>>())
            .next()
            .unwrap()
            .0,
        door
    );
    app.update();
    assert!(app.world().get::<Door>(door).unwrap().0);
    assert_eq!(
        writes
            .read(app.world().resource::<Messages<VerseWrite>>())
            .count(),
        0
    );
    app.update();
    assert!(!app.world().get::<Door>(door).unwrap().0);
    assert_eq!(
        writes
            .read(app.world().resource::<Messages<VerseWrite>>())
            .count(),
        1
    );
}

#[test]
fn accepted_reload_restarts_tasks_and_keeps_components() {
    let mut app = door::app(ASSET_ROOT);
    let (door, script) = door::spawn(&mut app);
    load(&mut app);
    app.world_mut().write_message(Interact {
        door,
        allowed: true,
    });
    app.update();
    assert!(app.world().get::<Door>(door).unwrap().0);

    app.world_mut()
        .resource_mut::<Assets<VerseAsset>>()
        .insert(
            script.id(),
            VerseAsset::new(
                "OnInteract(Allowed : logic) : void =\n    if (SetDoorOpen[false]) {}\n",
            ),
        )
        .unwrap();
    app.update();
    app.update();
    assert!(app.world().get::<Door>(door).unwrap().0);
    app.world_mut().write_message(Interact {
        door,
        allowed: true,
    });
    app.update();
    assert!(!app.world().get::<Door>(door).unwrap().0);
}

#[test]
fn a_faulting_message_does_not_replay_on_the_previous_revision() {
    let mut app = door::app(ASSET_ROOT);
    let (door, script) = door::spawn(&mut app);
    load(&mut app);
    let mut activations = MessageCursor::<VerseLoaded>::default();
    activations
        .read(app.world().resource::<Messages<VerseLoaded>>())
        .for_each(drop);

    app.world_mut()
        .resource_mut::<Assets<VerseAsset>>()
        .insert(
            script.id(),
            VerseAsset::new("OnInteract(Allowed : logic) : void =\n    loop:\n        1 + 1\n"),
        )
        .unwrap();
    app.world_mut().write_message(Interact {
        door,
        allowed: true,
    });
    app.update();

    assert!(!app.world().get::<Door>(door).unwrap().0);
    assert_eq!(
        activations
            .read(app.world().resource::<Messages<VerseLoaded>>())
            .count(),
        1
    );
    let diagnostics = app.world().resource::<Messages<VerseDiagnostic>>();
    assert!(MessageCursor::default()
        .read(diagnostics)
        .any(|d| d.code == "step_limit"));
}

#[test]
fn startup_once_without_input() {
    let mut app = door::app(ASSET_ROOT);
    let script = app.world_mut().resource_mut::<Assets<VerseAsset>>().add(VerseAsset::new(
        "OnStart()<suspends> : void =\n    if (SetDoorOpen[true]) {}\n    Sleep(0.5)\n    if (SetDoorOpen[false]) {}\n",
    ));
    let door = app
        .world_mut()
        .spawn((Door::default(), VerseScript(script)))
        .id();
    load(&mut app);
    assert!(app.world().get::<Door>(door).unwrap().0);
    app.update();
    assert!(app.world().get::<Door>(door).unwrap().0);
    app.update();
    assert!(!app.world().get::<Door>(door).unwrap().0);
    app.update();
    assert!(!app.world().get::<Door>(door).unwrap().0);
}

#[test]
fn activation_delivers_queued_input_after_startup() {
    let mut app = door::app(ASSET_ROOT);
    let script = app.world_mut().resource_mut::<Assets<VerseAsset>>().add(VerseAsset::new(
        "OnStart() : void =\n    if (SetDoorOpen[true]) {}\n\nOnInteract(Allowed : logic) : void =\n    if (SetDoorOpen[false]) {}\n",
    ));
    let door = app
        .world_mut()
        .spawn((Door::default(), VerseScript(script)))
        .id();
    app.world_mut().write_message(Interact {
        door,
        allowed: true,
    });
    load(&mut app);
    assert!(app.world().get::<Door>(door).unwrap().0);
    app.update();
    assert!(!app.world().get::<Door>(door).unwrap().0);
}

#[test]
fn char_arrays_cross_string_bindings() {
    #[derive(Component, Default)]
    struct Caption(String);
    #[derive(Message)]
    struct CaptionPublished(String);
    #[derive(Message)]
    struct CaptionChanged(bevy_ecs::entity::Entity, String);

    let mut app = door::app(ASSET_ROOT);
    app.register_verse_property(
        "Caption",
        "SetCaption",
        |caption: &Caption| caption.0.clone(),
        |caption, value| caption.0 = value,
    )
    .register_verse_output("PublishCaption", |_, value| CaptionPublished(value))
    .register_verse_input("OnCaption", |message: &CaptionChanged| {
        (message.0, message.1.clone())
    });
    let script = app.world_mut().resource_mut::<Assets<VerseAsset>>().add(VerseAsset::new(
        "OnStart() : void =\n    if (SetCaption[array{'a'}], Caption[] = \"a\") {}\n    Empty : []char = array{}\n    PublishCaption(Empty)\n\nOnCaption(New : []char) : void =\n    if (SetCaption[New]) {}\n",
    ));
    let entity = app
        .world_mut()
        .spawn((Caption::default(), VerseScript(script)))
        .id();
    load(&mut app);
    assert_eq!(app.world().get::<Caption>(entity).unwrap().0, "a");
    assert_eq!(
        MessageCursor::default()
            .read(app.world().resource::<Messages<CaptionPublished>>())
            .next()
            .unwrap()
            .0,
        ""
    );
    app.world_mut()
        .write_message(CaptionChanged(entity, "new".into()));
    app.update();
    assert_eq!(app.world().get::<Caption>(entity).unwrap().0, "new");
}

#[test]
fn rejected_registration_does_not_route_new_messages_into_the_prior_script() {
    #[derive(Message)]
    struct Extra(bevy_ecs::entity::Entity);

    let mut app = door::app(ASSET_ROOT);
    let mut source = include_str!("../examples/assets/door.verse").to_owned();
    source.push_str("\nOnExtra(Value : int) : void =\n    if (SetDoorOpen[false]) {}\n");
    let script = app
        .world_mut()
        .resource_mut::<Assets<VerseAsset>>()
        .add(VerseAsset::new(source));
    let entity = app
        .world_mut()
        .spawn((Door::default(), VerseScript(script)))
        .id();
    load(&mut app);

    app.register_verse_input("OnExtra", |message: &Extra| (message.0, true));
    app.world_mut().write_message(Extra(entity));
    app.world_mut().write_message(Interact {
        door: entity,
        allowed: true,
    });
    app.update();
    assert!(app.world().get::<Door>(entity).unwrap().0);
    let diags = app.world().resource::<Messages<VerseDiagnostic>>();
    let mut cursor = MessageCursor::default();
    let reported: Vec<_> = cursor.read(diags).collect();
    assert_eq!(reported.len(), 1);
    assert_eq!(reported[0].code, "V2001");
}

#[cfg(feature = "file_watcher")]
#[test]
fn editing_the_file_activates_the_new_script() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let path = root.join("door.verse");
    std::fs::write(&path, include_str!("../examples/assets/door.verse")).unwrap();
    let mut app = door::app(root.to_str().unwrap());
    let (door, _) = door::spawn(&mut app);
    load(&mut app);
    assert!(!app.world().get::<Door>(door).unwrap().0);

    std::fs::write(&path, "OnStart() : void =\n    if (SetDoorOpen[true]) {}\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.world().get::<Door>(door).unwrap().0 {
        app.update();
        assert!(
            Instant::now() < deadline,
            "file watcher did not reload door.verse"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn rejected_reload_reports_bad_types_and_preserves_suspended_handler() {
    let mut app = door::app(ASSET_ROOT);
    let (door, script) = door::spawn(&mut app);
    load(&mut app);
    app.world_mut().write_message(Interact {
        door,
        allowed: true,
    });
    app.update();
    assert!(app.world().get::<Door>(door).unwrap().0);

    app.world_mut()
        .resource_mut::<Assets<VerseAsset>>()
        .insert(
            script.id(),
            VerseAsset::new(
                "OnInteract(Allowed : logic) : void =\n    if (SetDoorOpen[\"open\"]) {}\n",
            ),
        )
        .unwrap();
    app.update();
    assert!(app.world().get::<Door>(door).unwrap().0);
    let mut diagnostics = MessageCursor::<VerseDiagnostic>::default();
    let reported: Vec<_> = diagnostics
        .read(app.world().resource::<Messages<VerseDiagnostic>>())
        .collect();
    assert!(!reported.is_empty());
    assert_eq!(reported[0].path, "door.verse");
    assert!(reported[0].span.is_some());
    app.update();
    assert!(!app.world().get::<Door>(door).unwrap().0);
}
