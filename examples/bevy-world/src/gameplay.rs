use bevy::prelude::*;
use reverse_bevy::ReverseAppExt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Station {
    Relay,
    Gate,
    Beacon,
}

impl Station {
    pub(crate) const ALL: [Self; 3] = [Self::Relay, Self::Gate, Self::Beacon];

    pub(crate) fn asset(self) -> &'static str {
        match self {
            Self::Relay => "relay.verse",
            Self::Gate => "gate.verse",
            Self::Beacon => "beacon.verse",
        }
    }

    pub(crate) fn source(self) -> &'static str {
        match self {
            Self::Relay => include_str!("../assets/relay.verse"),
            Self::Gate => include_str!("../assets/gate.verse"),
            Self::Beacon => include_str!("../assets/beacon.verse"),
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Relay => "Relay",
            Self::Gate => "Gate",
            Self::Beacon => "Beacon",
        }
    }

    pub(crate) fn position(self) -> Vec2 {
        match self {
            Self::Relay => Vec2::new(-400.0, 130.0),
            Self::Gate => Vec2::new(-170.0, 0.0),
            Self::Beacon => Vec2::new(68.0, 50.0),
        }
    }
}

#[derive(Component)]
pub(crate) struct Device {
    pub station: Station,
    pub active: bool,
    pub units: i64,
    pub level: f64,
    pub label: String,
}

impl Device {
    pub(crate) fn new(station: Station) -> Self {
        let label = match station {
            Station::Relay => "STANDBY",
            Station::Gate => "LOCKED",
            Station::Beacon => "BOOTING",
        };
        Self {
            station,
            active: false,
            units: 0,
            level: 0.0,
            label: label.into(),
        }
    }
}

#[derive(Message)]
pub(crate) struct Interact {
    pub target: Entity,
    pub allowed: bool,
}

#[derive(Message)]
pub(crate) struct DeviceMessage {
    pub target: Entity,
    pub text: String,
}

pub(crate) fn register(app: &mut App) {
    app.register_verse_property(
        "GetActive",
        "SetActive",
        |device: &Device| device.active,
        |device, value| device.active = value,
    )
    .register_verse_property(
        "GetUnits",
        "SetUnits",
        |device: &Device| device.units,
        |device, value| device.units = value,
    )
    .register_verse_property(
        "GetLevel",
        "SetLevel",
        |device: &Device| device.level,
        |device, value| device.level = value,
    )
    .register_verse_property(
        "GetLabel",
        "SetLabel",
        |device: &Device| device.label.clone(),
        |device, value| device.label = value,
    )
    .register_verse_input("OnInteract", |message: &Interact| {
        (message.target, message.allowed)
    })
    .register_verse_output("Report", |target, text| DeviceMessage { target, text });
}

pub(crate) fn beacon_alternate() -> String {
    Station::Beacon
        .source()
        .replace(
            "BeaconProfile : string = \"ORBIT\"",
            "BeaconProfile : string = \"AURORA\"",
        )
        .replace(
            "PulseSeconds : float = 0.25",
            "PulseSeconds : float = 0.125",
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        app::TaskPoolPlugin,
        asset::AssetPlugin,
        ecs::message::{MessageCursor, Messages},
        time::{Fixed, TimePlugin, TimeUpdateStrategy},
    };
    use reverse_bevy::{
        ReversePlugin, VerseAsset, VerseDiagnostic, VerseLoaded, VerseScript, VerseWrite,
    };
    use std::time::Duration;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            TimePlugin,
            AssetPlugin::default(),
            ReversePlugin::default(),
        ));
        app.insert_resource(Time::<Fixed>::from_duration(Duration::from_millis(125)))
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        register(&mut app);
        app
    }

    fn spawn(app: &mut App, station: Station, source: &str) -> Entity {
        let check = app.check_verse(source);
        assert!(!check.has_errors(), "{:?}", check.diagnostics);
        let asset = app
            .world_mut()
            .resource_mut::<Assets<VerseAsset>>()
            .add(VerseAsset::new(source));
        app.world_mut()
            .spawn((Device::new(station), VerseScript(asset)))
            .id()
    }

    fn load(app: &mut App, expected: usize) {
        let mut loaded = MessageCursor::<VerseLoaded>::default();
        let mut diagnostics = MessageCursor::<VerseDiagnostic>::default();
        let mut count = 0;
        for _ in 0..4 {
            app.update();
            let errors: Vec<_> = diagnostics
                .read(app.world().resource::<Messages<VerseDiagnostic>>())
                .collect();
            assert!(errors.is_empty(), "{errors:?}");
            count += loaded
                .read(app.world().resource::<Messages<VerseLoaded>>())
                .count();
            if count == expected {
                return;
            }
        }
        panic!("{count} scripts loaded, expected {expected}");
    }

    #[test]
    fn gate_rolls_back_an_unauthorized_open_and_times_an_authorized_open() {
        let mut app = app();
        let gate = spawn(&mut app, Station::Gate, Station::Gate.source());
        load(&mut app, 1);
        let mut writes = MessageCursor::<VerseWrite>::default();
        let mut messages = MessageCursor::<DeviceMessage>::default();

        app.world_mut().write_message(Interact {
            target: gate,
            allowed: false,
        });
        app.update();
        assert!(!app.world().get::<Device>(gate).unwrap().active);
        assert_eq!(
            writes
                .read(app.world().resource::<Messages<VerseWrite>>())
                .count(),
            0
        );
        assert_eq!(
            messages
                .read(app.world().resource::<Messages<DeviceMessage>>())
                .count(),
            0
        );

        app.world_mut().write_message(Interact {
            target: gate,
            allowed: true,
        });
        app.update();
        assert!(app.world().get::<Device>(gate).unwrap().active);
        assert!(messages
            .read(app.world().resource::<Messages<DeviceMessage>>())
            .any(|message| message.target == gate));
        for _ in 0..23 {
            app.update();
        }
        assert!(app.world().get::<Device>(gate).unwrap().active);
        app.update();
        assert!(!app.world().get::<Device>(gate).unwrap().active);
        assert!(app
            .world()
            .resource::<Messages<VerseDiagnostic>>()
            .is_empty());
    }

    #[test]
    fn relay_charges_and_beacon_completes_a_scan_in_both_revisions() {
        let mut app = app();
        let relay = spawn(&mut app, Station::Relay, Station::Relay.source());
        let orbit = spawn(&mut app, Station::Beacon, Station::Beacon.source());
        let aurora = spawn(&mut app, Station::Beacon, &beacon_alternate());
        load(&mut app, 3);
        assert_eq!(
            app.world().get::<Device>(orbit).unwrap().label,
            "ORBIT READY"
        );
        assert_eq!(
            app.world().get::<Device>(aurora).unwrap().label,
            "AURORA READY"
        );
        for target in [relay, orbit, aurora] {
            app.world_mut().write_message(Interact {
                target,
                allowed: true,
            });
        }
        app.update();
        assert_eq!(app.world().get::<Device>(relay).unwrap().units, 1);
        assert!(app.world().get::<Device>(orbit).unwrap().active);
        let mut cursor = MessageCursor::<DeviceMessage>::default();
        let mut reports = Vec::new();
        for tick in 1..=12 {
            app.update();
            reports.extend(
                cursor
                    .read(app.world().resource::<Messages<DeviceMessage>>())
                    .map(|message| (tick, message.target, message.text.clone())),
            );
        }
        for (entity, label, archive_tick) in [
            (orbit, "ORBIT SCAN COMPLETE", 12),
            (aurora, "AURORA SCAN COMPLETE", 8),
        ] {
            let beacon = app.world().get::<Device>(entity).unwrap();
            assert_eq!(beacon.label, label);
            assert_eq!(beacon.units, 1);
            assert_eq!(beacon.level, 1.0);
            assert!(!beacon.active);
            assert!(reports.contains(&(3, entity, "Signal acquired by primary receiver".into(),)));
            assert!(reports.contains(&(archive_tick, entity, "Scan 1 archived".into())));
        }
        assert!(app
            .world()
            .resource::<Messages<VerseDiagnostic>>()
            .is_empty());
    }
}
