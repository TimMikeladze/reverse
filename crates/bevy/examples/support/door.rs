use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer, Handle};
use bevy_ecs::{component::Component, entity::Entity, message::Message};
use bevy_time::{Fixed, Time, TimePlugin, TimeUpdateStrategy};
use reverse_bevy::{ReverseAppExt, ReversePlugin, VerseAsset, VerseScript};
use std::time::Duration;

#[derive(Component, Default)]
pub struct Door(pub bool);

#[derive(Message)]
pub struct Interact {
    pub door: Entity,
    pub allowed: bool,
}

#[derive(Message)]
pub struct Opened(pub Entity);

pub fn app(asset_root: &str) -> App {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        TimePlugin,
        AssetPlugin {
            file_path: asset_root.into(),
            ..Default::default()
        },
        ReversePlugin::default(),
    ))
    .insert_resource(Time::<Fixed>::from_duration(Duration::from_millis(250)))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
    .register_verse_property(
        "DoorOpen",
        "SetDoorOpen",
        |door: &Door| door.0,
        |door, open| {
            door.0 = open;
        },
    )
    .register_verse_input("OnInteract", |message: &Interact| {
        (message.door, message.allowed)
    })
    .register_verse_output("DoorOpened", |door, ()| Opened(door));
    app
}

pub fn spawn(app: &mut App) -> (Entity, Handle<VerseAsset>) {
    let script = app.world().resource::<AssetServer>().load("door.verse");
    let door = app
        .world_mut()
        .spawn((Door::default(), VerseScript(script.clone())))
        .id();
    (door, script)
}
