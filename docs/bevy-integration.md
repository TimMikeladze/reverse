# Reverse in Bevy

```sh
cargo run --locked --manifest-path examples/bevy-world/Cargo.toml
```

Play the [2D observatory showcase](bevy-world.md) to see rollback, fixed-clock
concurrency, typed ECS bindings, and hot reload in a Bevy window.

```sh
cargo run -p reverse-bevy --example timed_door
cargo test -p reverse-bevy --test door
```

The example loads `door.verse`, sends an interaction to a Bevy entity, and runs the fixed
clock until Verse closes the door. The headless test uses the same plugin and bindings.
It checks that a failed Verse condition leaves both the door and its outgoing messages
unchanged. [Example](../crates/bevy/examples/timed_door.rs) · [Verse script](../crates/bevy/examples/assets/door.verse)

## Install the plugin

Use Bevy 0.20 and add `reverse-bevy` from a checkout. For adjacent project and Reverse
checkouts, use this Cargo dependency.

```toml
[dependencies]
bevy = "0.20.1"
reverse-bevy = { path = "../reverse/crates/bevy" }
```

```rust
use bevy::prelude::*;
use reverse_bevy::{ReverseAppExt, ReversePlugin, VerseScript};

#[derive(Component)]
struct Door(bool);

#[derive(Message)]
struct Interact {
    door: Entity,
    allowed: bool,
}

#[derive(Message)]
struct Opened(Entity);

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ReversePlugin::default()))
        .register_verse_property("DoorOpen", "SetDoorOpen", |door: &Door| door.0,
            |door, open| door.0 = open)
        .register_verse_input("OnInteract", |message: &Interact| (message.door, message.allowed))
        .register_verse_output("DoorOpened", |door, ()| Opened(door))
        .add_systems(Startup, spawn_door)
        .run();
}

fn spawn_door(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((Door(false), VerseScript(assets.load("door.verse"))));
}
```

Place the script in `assets/door.verse`. Send `Interact { door, allowed }` from a Bevy
system to call `OnInteract(Allowed : logic)<suspends> : void` on that entity's script.
Define `OnStart() : void` for work that starts without a message. `OnStart` can use
`<suspends>`. When a revision defines `OnStart`, it gets the activation tick to start.
Messages queued during activation reach that instance on the next fixed tick.
For a headless app, add `TaskPoolPlugin`, `TimePlugin`, and `AssetPlugin` before
`ReversePlugin`. [Bevy plugins](https://bevy.org/learn/quick-start/getting-started/plugins/)

## Register bindings

- `register_verse_property` creates a getter and setter on the script's entity. Both can
  fail when the component is absent. Call them as `DoorOpen[]` and `SetDoorOpen[true]`.
  Register each property independently and have each setter update its corresponding field.
- `register_verse_input` maps a Bevy message to the owning entity and a Verse handler
  argument. Use `()` for a handler with no arguments.
- `register_verse_output` creates a Verse function that publishes a typed Bevy message
  after commit. The Rust mapper receives the script's entity and the payload.
- `app.check_verse(source)` checks source using the same bindings and handler signatures
  as loaded assets. Supported payload values are `bool`, `i64`, `f64`, `String`, and `()`
  for messages without a value.

## How a tick runs

Bevy owns entity lifetimes and the registered component state. Each entity with
`VerseScript` has its own persistent VM and Verse globals. `ReverseSet::Collect` in
`PreUpdate` queues input. `ReverseSet::Tick` in `FixedUpdate` collects newly arrived
messages, steps the VMs using `Time<Fixed>`, and applies committed effects.
`Sleep(0.0)` yields until the next fixed tick. Headless tests can advance one fixed tick
per update with `TimeUpdateStrategy::FixedTimesteps(1)`.
[Bevy fixed clock](https://docs.rs/bevy_time/0.20.0/bevy_time/struct.Fixed.html) · [Test clock](https://docs.rs/bevy_time/0.20.0/bevy_time/enum.TimeUpdateStrategy.html#variant.FixedTimesteps)

A script reads its entity's registered properties from the tick snapshot and sees its
own staged writes. A failed inner condition removes its staged writes and outgoing
messages even when the enclosing function returns normally. The plugin reports committed
property changes as `VerseWrite`, printed lines as `VerseOutput`, and script problems
as `VerseDiagnostic`. A runtime fault discards the script's pending effects for that tick
and stops that instance. Configure per-instance tick limits through `ReversePlugin::limits`.
[Verse failure contexts](https://verselang.github.io/book/08_failure/)

## Reload scripts

Enable the `file_watcher` feature on `reverse-bevy` for desktop development. Bevy reloads
the file and the plugin checks the new revision before switching at a fixed tick boundary.
A check, global initialization, or first `OnStart` turn error leaves the last valid
instance running.
A valid revision starts fresh Verse globals and cancels the previous revision's suspended
tasks while preserving components on the entity. `VerseLoaded` reports activation.
[Bevy asset hot reload](https://docs.rs/bevy_asset/0.20.0/bevy_asset/#hot-reloading-assets)
