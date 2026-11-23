mod gameplay;
mod presentation;
#[cfg(target_arch = "wasm32")]
mod web_editor;

#[cfg(not(target_arch = "wasm32"))]
use bevy::asset::AssetPlugin;
use bevy::{
    ecs::system::SystemParam,
    platform::time::Instant,
    prelude::*,
    window::{PresentMode, WindowResolution},
};
use gameplay::{Device, DeviceMessage, Interact, Station};
use reverse_bevy::{ReversePlugin, VerseDiagnostic, VerseLoaded, VerseOutput, VerseWrite};
#[cfg(target_arch = "wasm32")]
use reverse_bevy::{VerseAsset, VerseScript};
use std::{collections::VecDeque, time::Duration};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs, io,
    path::{Path, PathBuf},
};

const WINDOW_WIDTH: u32 = 1180;
const WINDOW_HEIGHT: u32 = 800;
const INVALID_BEACON: &str = "OnStart() : void =\n    if (SetLevel[\"broken\"]) {}\n";

#[derive(Component)]
pub(crate) struct Player;

#[derive(Clone, Copy)]
pub(crate) enum Tone {
    Info,
    Good,
    Warn,
}

pub(crate) struct LogLine {
    pub text: String,
    pub tone: Tone,
}

#[derive(Resource)]
pub(crate) struct Console {
    pub lines: VecDeque<LogLine>,
    pub hint: String,
    pub revision: String,
    pub writes: u64,
    pub messages: u64,
}

impl Console {
    fn log(&mut self, tone: Tone, text: impl Into<String>) {
        self.lines.push_front(LogLine {
            text: text.into(),
            tone,
        });
        self.lines.truncate(5);
    }
}

#[derive(Clone, Copy)]
enum Edit {
    Orbit,
    Aurora,
    Invalid,
}

impl Edit {
    fn next(self) -> Self {
        match self {
            Self::Orbit => Self::Aurora,
            Self::Aurora => Self::Invalid,
            Self::Invalid => Self::Orbit,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Orbit => "ORBIT",
            Self::Aurora => "AURORA",
            Self::Invalid => "INVALID",
        }
    }

    fn source(self) -> String {
        match self {
            Self::Orbit => Station::Beacon.source().to_owned(),
            Self::Aurora => gameplay::beacon_alternate(),
            Self::Invalid => INVALID_BEACON.to_owned(),
        }
    }
}

#[derive(Resource)]
struct Reload {
    #[cfg(not(target_arch = "wasm32"))]
    file: PathBuf,
    requested: Edit,
    active_label: &'static str,
    waiting_since: Option<Instant>,
}

fn window_plugin() -> WindowPlugin {
    WindowPlugin {
        primary_window: Some(Window {
            title: "Reverse Observatory".into(),
            resolution: WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT),
            present_mode: PresentMode::AutoVsync,
            resizable: false,
            #[cfg(target_arch = "wasm32")]
            canvas: Some("#observatory".into()),
            #[cfg(target_arch = "wasm32")]
            fit_canvas_to_parent: true,
            ..default()
        }),
        ..default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> io::Result<()> {
    let (_assets, assets_path) = demo_assets()?;
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(window_plugin())
            .set(watched_assets(&assets_path)),
    );
    install_showcase(&mut app, &assets_path);
    app.run();
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(window_plugin()));
    install_showcase(&mut app);
    web_editor::install(&mut app);
    app.run();
}

#[cfg(not(target_arch = "wasm32"))]
fn demo_assets() -> io::Result<(tempfile::TempDir, PathBuf)> {
    let assets = tempfile::Builder::new()
        .prefix("reverse-observatory-")
        .tempdir()?;
    let assets_path = assets.path().canonicalize()?;
    for station in Station::ALL {
        fs::write(assets_path.join(station.asset()), station.source())?;
    }
    Ok((assets, assets_path))
}

#[cfg(not(target_arch = "wasm32"))]
fn watched_assets(path: &Path) -> AssetPlugin {
    AssetPlugin {
        file_path: path.to_string_lossy().into_owned(),
        watch_for_changes_override: Some(true),
        ..default()
    }
}

fn install_showcase(app: &mut App, #[cfg(not(target_arch = "wasm32"))] assets_path: &Path) {
    app.add_plugins(ReversePlugin::default())
        .insert_resource(Reload {
            #[cfg(not(target_arch = "wasm32"))]
            file: assets_path.join("beacon.verse"),
            requested: Edit::Orbit,
            active_label: Edit::Orbit.label(),
            waiting_since: None,
        })
        .insert_resource(Console {
            lines: VecDeque::new(),
            hint: String::new(),
            revision: "ORBIT".into(),
            writes: 0,
            messages: 0,
        })
        .add_systems(Startup, presentation::setup)
        .add_systems(
            Update,
            (
                move_player,
                interact,
                reload,
                read_plugin_messages,
                presentation::refresh,
            )
                .chain(),
        );
    gameplay::register(app);
}

fn move_player(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    stations: Query<&Device>,
    mut player: Query<&mut Transform, With<Player>>,
) {
    let Ok(mut player) = player.single_mut() else {
        return;
    };
    let axis = |forward: KeyCode, forward_alt: KeyCode, back: KeyCode, back_alt: KeyCode| {
        f32::from(keys.any_pressed([forward, forward_alt]))
            - f32::from(keys.any_pressed([back, back_alt]))
    };
    let direction = Vec2::new(
        axis(
            KeyCode::KeyD,
            KeyCode::ArrowRight,
            KeyCode::KeyA,
            KeyCode::ArrowLeft,
        ),
        axis(
            KeyCode::KeyW,
            KeyCode::ArrowUp,
            KeyCode::KeyS,
            KeyCode::ArrowDown,
        ),
    )
    .normalize_or_zero();
    if direction == Vec2::ZERO {
        return;
    }

    let before = player.translation.truncate();
    let mut next = (before + direction * (220.0 * time.delta_secs().min(0.05)))
        .clamp(Vec2::new(-550.0, -255.0), Vec2::new(178.0, 248.0));
    let gate = Station::Gate.position().x;
    let wide_x = 22.0;
    let gate_open = stations
        .iter()
        .any(|device| device.station == Station::Gate && device.active);
    if !(gate_open && next.y.abs() < 43.0) && (next.x - gate).abs() < wide_x {
        let side = if before.x <= gate { -1.0 } else { 1.0 };
        next.x = gate + wide_x * side;
    }
    player.translation.x = next.x;
    player.translation.y = next.y;
}

fn interact(
    keys: Res<ButtonInput<KeyCode>>,
    stations: Query<(Entity, &Device)>,
    player: Query<&Transform, With<Player>>,
    mut actions: MessageWriter<Interact>,
    mut console: ResMut<Console>,
) {
    let Ok(player) = player.single() else { return };
    let nearest = stations
        .iter()
        .min_by(|(_, a), (_, b)| {
            let distance = |station: Station| {
                station
                    .position()
                    .distance_squared(player.translation.truncate())
            };
            distance(a.station).total_cmp(&distance(b.station))
        })
        .filter(|(_, device)| {
            device
                .station
                .position()
                .distance(player.translation.truncate())
                <= 92.0
        });
    let hint = nearest.map_or_else(
        || "Move to a station, or press 1, 2, or 3.".to_owned(),
        |(_, device)| format!("E  Interact with {}", device.station.title()),
    );
    if console.hint != hint {
        console.hint = hint;
    }

    let shortcut = [
        (KeyCode::Digit1, Station::Relay),
        (KeyCode::Digit2, Station::Gate),
        (KeyCode::Digit3, Station::Beacon),
    ]
    .into_iter()
    .find_map(|(key, station)| keys.just_pressed(key).then_some(station));
    let choice = if let Some(station) = shortcut {
        stations
            .iter()
            .find(|(_, device)| device.station == station)
    } else if keys.just_pressed(KeyCode::KeyE) {
        nearest
    } else {
        None
    };
    if let Some((target, device)) = choice {
        let relay_ready = stations
            .iter()
            .any(|(_, device)| device.station == Station::Relay && device.units > 0);
        let allowed = device.station != Station::Gate || relay_ready;
        actions.write(Interact { target, allowed });
    }
}

fn reload(
    keys: Res<ButtonInput<KeyCode>>,
    mut reload: ResMut<Reload>,
    mut console: ResMut<Console>,
    #[cfg(target_arch = "wasm32")] devices: Query<(&Device, &VerseScript)>,
    #[cfg(target_arch = "wasm32")] mut scripts: ResMut<Assets<VerseAsset>>,
) {
    #[cfg(target_arch = "wasm32")]
    if web_editor::busy() {
        return;
    }
    if let Some(since) = reload.waiting_since {
        if since.elapsed() < Duration::from_secs(4) {
            return;
        }
        reload.waiting_since = None;
        console.log(Tone::Warn, "Beacon reload did not finish");
        console.revision = reload.active_label.into();
    }
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }

    let next = reload.requested.next();
    #[cfg(not(target_arch = "wasm32"))]
    let replaced = fs::write(&reload.file, next.source()).map_err(|error| error.to_string());
    #[cfg(target_arch = "wasm32")]
    let replaced = devices
        .iter()
        .find(|(device, _)| device.station == Station::Beacon)
        .filter(|(_, script)| scripts.get(script.0.id()).is_some())
        .ok_or_else(|| "Beacon script is still loading".to_owned())
        .and_then(|(_, script)| {
            scripts
                .insert(script.0.id(), VerseAsset::new(next.source()))
                .map_err(|error| error.to_string())
        });
    match replaced {
        Ok(()) => {
            reload.requested = next;
            reload.waiting_since = Some(Instant::now());
            console.revision = format!("{} -> {}", reload.active_label, next.label());
            console.log(
                Tone::Info,
                format!("Editing beacon.verse / {}", next.label()),
            );
        }
        Err(error) => console.log(
            Tone::Warn,
            format!("Could not replace beacon.verse: {error}"),
        ),
    }
}

#[derive(SystemParam)]
struct VerseEvents<'w, 's> {
    loaded: MessageReader<'w, 's, VerseLoaded>,
    diagnostics: MessageReader<'w, 's, VerseDiagnostic>,
    writes: MessageReader<'w, 's, VerseWrite>,
    output: MessageReader<'w, 's, VerseOutput>,
    reports: MessageReader<'w, 's, DeviceMessage>,
}

fn read_plugin_messages(
    mut events: VerseEvents,
    devices: Query<&Device>,
    mut console: ResMut<Console>,
    mut reload: ResMut<Reload>,
) {
    let did_write = events.writes.read().count() as u64;
    if did_write > 0 {
        console.writes += did_write;
    }
    for event in events.loaded.read() {
        let Ok(device) = devices.get(event.entity) else {
            continue;
        };
        if device.station == Station::Beacon && reload.waiting_since.take().is_some() {
            reload.active_label = reload.requested.label();
            console.revision = reload.active_label.into();
            console.log(
                Tone::Good,
                format!("Activated {} / ECS preserved", reload.active_label),
            );
        } else {
            console.log(
                Tone::Info,
                format!("{} script loaded", device.station.title()),
            );
        }
    }
    for event in events.diagnostics.read() {
        if event.path.ends_with("beacon.verse") && reload.waiting_since.take().is_some() {
            console.revision = format!("{} / retained", reload.active_label);
        }
        console.log(Tone::Warn, format!("{}  {}", event.code, event.message));
    }
    for event in events.output.read() {
        let warning = event.line.to_ascii_lowercase().contains("denied");
        console.log(
            if warning { Tone::Warn } else { Tone::Info },
            event.line.as_str(),
        );
    }
    for event in events.reports.read() {
        let Ok(device) = devices.get(event.target) else {
            continue;
        };
        console.messages += 1;
        console.log(
            Tone::Good,
            format!("{} / {}", device.station.title(), event.text),
        );
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset(key);
        app.update();
    }

    fn wait_for(app: &mut App, ready: impl Fn(&Console) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            if ready(app.world().resource::<Console>()) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "showcase action did not complete"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn keyboard_actions_drive_the_world_and_reload_the_watched_script() {
        let (_assets, path) = demo_assets().unwrap();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, watched_assets(&path)))
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        install_showcase(&mut app, &path);
        wait_for(&mut app, |console| {
            console
                .lines
                .iter()
                .filter(|line| line.text.ends_with("script loaded"))
                .count()
                == 3
        });

        let before = app.world().resource::<Console>().writes;
        press(&mut app, KeyCode::Digit2);
        let console = app.world().resource::<Console>();
        assert_eq!(console.writes, before);
        assert_eq!(console.messages, 0);
        assert!(console.lines.front().unwrap().text.contains("rolled back"));

        press(&mut app, KeyCode::Digit1);
        press(&mut app, KeyCode::Digit2);
        assert!(app
            .world()
            .resource::<Console>()
            .lines
            .front()
            .unwrap()
            .text
            .contains("Gate opened"));

        press(&mut app, KeyCode::KeyR);
        wait_for(&mut app, |console| console.revision == "AURORA");
        press(&mut app, KeyCode::KeyR);
        wait_for(&mut app, |console| console.revision == "AURORA / retained");
        assert!(app
            .world()
            .resource::<Console>()
            .lines
            .iter()
            .any(|line| line.text.starts_with("V2001")));

        press(&mut app, KeyCode::Digit3);
        wait_for(&mut app, |console| {
            console
                .lines
                .iter()
                .any(|line| line.text.contains("AURORA scan 1 complete"))
        });
    }
}
