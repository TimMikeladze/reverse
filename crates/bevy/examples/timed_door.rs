#[path = "support/door.rs"]
mod door;

use bevy_ecs::message::{MessageCursor, Messages};
use door::{Door, Interact, Opened};
use reverse_bevy::{VerseDiagnostic, VerseLoaded};
use std::{
    thread,
    time::{Duration, Instant},
};

fn main() {
    let mut app = door::app(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/assets"));
    let (entity, _) = door::spawn(&mut app);
    let mut loaded = MessageCursor::<VerseLoaded>::default();
    let mut diagnostics = MessageCursor::<VerseDiagnostic>::default();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        for diagnostic in diagnostics.read(app.world().resource::<Messages<VerseDiagnostic>>()) {
            eprintln!(
                "{} {}: {}",
                diagnostic.path, diagnostic.code, diagnostic.message
            );
        }
        if loaded
            .read(app.world().resource::<Messages<VerseLoaded>>())
            .any(|loaded| loaded.entity == entity)
        {
            break;
        }
        assert!(Instant::now() < deadline, "door.verse did not load");
        thread::yield_now();
    }

    app.world_mut().write_message(Interact {
        door: entity,
        allowed: false,
    });
    app.update();
    assert!(!app.world().get::<Door>(entity).unwrap().0);
    println!("Rejected interaction leaves the door closed");

    app.world_mut().write_message(Interact {
        door: entity,
        allowed: true,
    });
    app.update();
    let mut opened = MessageCursor::<Opened>::default();
    assert!(opened
        .read(app.world().resource::<Messages<Opened>>())
        .any(|opened| opened.0 == entity));
    println!("Allowed interaction opens the door");

    app.update();
    assert!(app.world().get::<Door>(entity).unwrap().0);
    app.update();
    assert!(!app.world().get::<Door>(entity).unwrap().0);
    println!("Verse closes the door after two fixed ticks");
}
