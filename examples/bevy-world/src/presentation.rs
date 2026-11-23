use bevy::{camera::ScalingMode, prelude::*, sprite::Anchor, text::TextBounds};
use reverse_bevy::VerseScript;
use std::f32::consts::{FRAC_PI_4, TAU};

use crate::{
    gameplay::{Device, Station},
    Console, Player, Tone,
};

const INK: Color = Color::srgb_u8(230, 241, 234);
const MUTED: Color = Color::srgb_u8(159, 181, 188);
const NIGHT: Color = Color::srgb_u8(10, 17, 24);
const PANEL: Color = Color::srgb_u8(19, 31, 40);
const BORDER: Color = Color::srgb_u8(43, 62, 72);
const FLOOR_MARK: Color = Color::srgb_u8(39, 71, 77);
const MINT: Color = Color::srgb_u8(130, 240, 185);
const AMBER: Color = Color::srgb_u8(255, 196, 113);
const ICE: Color = Color::srgb_u8(122, 213, 249);
const VIOLET: Color = Color::srgb_u8(201, 157, 251);

const MAP_MIN: Vec2 = Vec2::new(-566.0, -280.0);
const MAP_MAX: Vec2 = Vec2::new(194.0, 278.0);

#[derive(Component)]
pub(crate) struct MappedLabel(Entity);

#[derive(Component)]
pub(crate) struct DeviceSprite {
    device: Entity,
    part: Part,
}

enum Part {
    RelayCell,
    GateLeaf(f32),
    BeaconLight,
    BeaconNeedle,
    BeaconReadout,
}

#[derive(Component)]
pub(crate) enum ConsoleText {
    Revision,
    Writes,
    Messages,
    Log(usize),
    Hint,
}

pub(crate) fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(ClearColor(NIGHT));
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 1180.0,
                height: 800.0,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));

    draw_map(&mut commands);
    draw_header(&mut commands);
    draw_console(&mut commands);

    for station in Station::ALL {
        let location = station.position();
        let device = commands
            .spawn((
                Device::new(station),
                VerseScript(assets.load(station.asset())),
                Transform::from_translation(location.extend(0.0)),
                Visibility::default(),
            ))
            .id();
        match station {
            Station::Relay => draw_relay(&mut commands, device, station.title()),
            Station::Gate => draw_gate(&mut commands, device, station.title()),
            Station::Beacon => draw_beacon(&mut commands, device, station.title()),
        }
    }

    draw_player(&mut commands);
}

fn draw_header(commands: &mut Commands) {
    commands.spawn(label("Signal Observatory", -566., 351., 42., INK));
    commands.spawn(label(
        "Reverse Bevy   /   Verse gameplay",
        -563.,
        312.,
        16.,
        MUTED,
    ));
    commands.spawn(label("Research deck", 222., 322., 15., ICE));
    commands.spawn((
        ConsoleText::Hint,
        label("", -563., -305., 15., INK),
        TextBounds::new_horizontal(750.),
    ));

    commands.spawn(block((0., -351.), (1132., 50.), 2., PANEL));
    commands.spawn(block((0., -327.), (1132., 2.), 3., BORDER));
    for (x, keys, action) in [
        (-550., "WASD / arrows", "Move"),
        (-349., "E", "Interact"),
        (-201., "1", "Relay"),
        (-75., "2", "Gate"),
        (51., "3", "Beacon"),
        (204., "R", "Reload beacon"),
    ] {
        let width = if keys.len() > 1 { 112. } else { 24. };
        commands.spawn(block((x + width / 2., -351.), (width, 25.), 3., BORDER));
        commands.spawn(label(keys, x + 7., -351., 13., INK));
        commands.spawn(label(action, x + width + 10., -351., 14., MUTED));
    }
}

fn draw_map(commands: &mut Commands) {
    let center = ((MAP_MIN + MAP_MAX) / 2.).into();
    let size = (MAP_MAX - MAP_MIN).into();
    commands.spawn(block(center, size, -20., BORDER));
    commands.spawn(block(
        center,
        (756., 554.),
        -19.,
        Color::srgb_u8(17, 31, 40),
    ));

    for column in 0..22 {
        for row in 0..16 {
            let x = -543. + column as f32 * 34.;
            let y = -258. + row as f32 * 34.;
            let tone = ((column * 19 + row * 17 + column * row * 7) % 5) as u8;
            let color = Color::srgb_u8(20 + tone, 33 + tone, 40 + tone);
            commands.spawn(block((x, y), (33., 33.), -17., color));
        }
    }

    // Recessed raceways painted into the deck.
    let relay = Station::Relay.position();
    let gate = Station::Gate.position();
    let beacon = Station::Beacon.position();
    for (from, to) in [
        (relay, Vec2::new(relay.x, gate.y)),
        (Vec2::new(relay.x, gate.y), gate),
        (gate, Vec2::new(beacon.x, gate.y)),
        (Vec2::new(beacon.x, gate.y), beacon),
    ] {
        commands.spawn(stroke(from, to, 13., -15., PANEL));
        commands.spawn(stroke(from, to, 2., -14., FLOOR_MARK));
    }

    for y in [-167., 166.] {
        commands.spawn(block((gate.x + 7., y - 4.), (35., 232.), -12., NIGHT));
        commands.spawn(block((gate.x, y), (27., 226.), -11., BORDER));
        commands.spawn(block(
            (gate.x - 5., y),
            (10., 226.),
            -10.,
            Color::srgb_u8(58, 79, 86),
        ));
        for offset in -4..5 {
            commands.spawn(block(
                (gate.x - 3., y + offset as f32 * 23.),
                (12., 3.),
                -9.,
                FLOOR_MARK,
            ));
        }
    }

    for y in [-269., 267.] {
        for x in [-550., -234., -105., 178.] {
            commands.spawn(block((x, y), (16., 4.), -10., FLOOR_MARK));
        }
    }

    // Dark glass panels and the hatch markings are floor decals.
    for (x, y, width) in [(-465., 219., 146.), (-340., 219., 78.), (66., 217., 184.)] {
        commands.spawn(block((x, y), (width, 26.), -9., BORDER));
        commands.spawn(block(
            (x, y),
            (width - 6., 20.),
            -8.,
            Color::srgb_u8(14, 26, 36),
        ));
        for star in 0..12 {
            let sx = x - width / 2. + 9. + (star * 67 % (width as i32 - 15)) as f32;
            let sy = y - 7. + (star * 11 % 15) as f32;
            let size = if star % 5 == 0 { 2. } else { 1. };
            commands.spawn(block((sx, sy), (size, size), -7., FLOOR_MARK));
        }
    }

    for (x, y, w) in [(-460., -221., 132.), (69., -211., 170.)] {
        commands.spawn(block((x, y), (w, 22.), -9., PANEL));
        for offset in -4..5 {
            commands.spawn(block((x + offset as f32 * 12., y), (7., 2.), -8., BORDER));
        }
    }
}

fn draw_relay(commands: &mut Commands, device: Entity, title: &str) {
    for (xy, size, z, color) in [
        ((0., -4.), (83., 66.), 2., NIGHT),
        ((0., 0.), (80., 62.), 3., BORDER),
        ((0., 0.), (74., 55.), 4., PANEL),
        ((0., 17.), (64., 10.), 6., FLOOR_MARK),
        ((-30., -18.), (9., 5.), 6., MUTED),
        ((30., -18.), (9., 5.), 6., MUTED),
    ] {
        commands.spawn((ChildOf(device), block(xy, size, z, color)));
    }
    for x in [-17., 0., 17.] {
        commands.spawn((ChildOf(device), block((x, -1.), (13., 15.), 7., BORDER)));
        commands.spawn((
            ChildOf(device),
            DeviceSprite {
                device,
                part: Part::RelayCell,
            },
            block((x, -1.), (9., 11.), 8., FLOOR_MARK),
        ));
    }
    commands.spawn((ChildOf(device), label(title, 55., 11., 17., INK)));
    commands.spawn((
        ChildOf(device),
        MappedLabel(device),
        label("", 55., -13., 13., AMBER),
    ));
}

fn draw_gate(commands: &mut Commands, device: Entity, title: &str) {
    commands.spawn((ChildOf(device), block((0., 0.), (44., 113.), 2., NIGHT)));
    commands.spawn((ChildOf(device), block((0., 0.), (33., 110.), 3., BORDER)));
    commands.spawn((ChildOf(device), block((0., 0.), (21., 109.), 4., PANEL)));
    for sign in [-1., 1.] {
        commands.spawn((
            ChildOf(device),
            DeviceSprite {
                device,
                part: Part::GateLeaf(sign),
            },
            block((0., sign * 28.), (23., 53.), 5., AMBER.with_alpha(0.84)),
        ));
        for x in [-17., 17.] {
            commands.spawn((
                ChildOf(device),
                block((x, sign * 53.), (8., 6.), 10., AMBER),
            ));
        }
    }
    commands.spawn((ChildOf(device), label(title, -124., 43., 17., INK)));
    commands.spawn((
        ChildOf(device),
        MappedLabel(device),
        label("", -123., 21., 13., AMBER),
    ));
}

fn draw_beacon(commands: &mut Commands, device: Entity, title: &str) {
    for (radius, segments, width, tint) in [
        (89., 32, 2., BORDER),
        (73., 32, 2., FLOOR_MARK),
        (47., 24, 3., BORDER),
    ] {
        for index in 0..segments {
            let angle = index as f32 * TAU / segments as f32;
            let pos = Vec2::from_angle(angle) * radius;
            let arc = TAU * radius / segments as f32 - 3.;
            let (sprite, mut transform) = block(pos.into(), (arc, width), 2., tint);
            transform.rotate_z(angle + TAU / 4.);
            commands.spawn((ChildOf(device), sprite, transform));
        }
    }
    let (sprite, mut transform) = block((0., 0.), (62., 62.), 3., BORDER);
    transform.rotate_z(FRAC_PI_4);
    commands.spawn((ChildOf(device), sprite, transform));
    let (sprite, mut transform) = block((0., 0.), (45., 45.), 5., PANEL);
    transform.rotate_z(FRAC_PI_4);
    commands.spawn((ChildOf(device), sprite, transform));
    let (sprite, mut transform) = block((0., 0.), (24., 24.), 7., ICE);
    transform.rotate_z(FRAC_PI_4);
    commands.spawn((
        ChildOf(device),
        DeviceSprite {
            device,
            part: Part::BeaconLight,
        },
        sprite,
        transform,
    ));
    commands.spawn((
        ChildOf(device),
        DeviceSprite {
            device,
            part: Part::BeaconNeedle,
        },
        block((45., 0.), (30., 3.), 6., ICE.with_alpha(0.)),
    ));
    commands.spawn((ChildOf(device), centered(title, 0., -108., 17., INK)));
    commands.spawn((
        ChildOf(device),
        MappedLabel(device),
        centered("", 0., -131., 13., ICE),
    ));
    commands.spawn((ChildOf(device), block((0., -153.), (138., 6.), 4., BORDER)));
    commands.spawn((
        ChildOf(device),
        DeviceSprite {
            device,
            part: Part::BeaconReadout,
        },
        block((-68., -153.), (0., 4.), 5., ICE),
    ));
}

fn draw_player(commands: &mut Commands) {
    let player = commands
        .spawn((
            Player,
            Transform::from_xyz(-260., -85., 30.),
            Visibility::default(),
        ))
        .id();
    for (xy, size, z, color) in [
        ((0., -11.), (30., 9.), 0., NIGHT.with_alpha(0.75)),
        ((0., -4.), (20., 20.), 1., NIGHT),
        ((0., -4.), (15., 17.), 2., Color::srgb_u8(80, 157, 135)),
        ((-8., -4.), (5., 12.), 1., MINT),
        ((8., -4.), (5., 12.), 1., MINT),
        ((0., 10.), (18., 17.), 3., MINT),
        ((0., 10.), (13., 10.), 4., Color::srgb_u8(23, 58, 67)),
        ((4., 12.), (4., 3.), 5., ICE),
        ((-5., -14.), (6., 6.), 3., MUTED),
        ((5., -14.), (6., 6.), 3., MUTED),
    ] {
        commands.spawn((ChildOf(player), block(xy, size, z, color)));
    }
    commands.spawn((ChildOf(player), centered("You", 0., 31., 12., MINT)));
}

fn draw_console(commands: &mut Commands) {
    commands.spawn(block((394., -1.), (344., 558.), 2., BORDER));
    commands.spawn(block((394., -1.), (340., 554.), 3., PANEL));
    commands.spawn(label("Verse console", 242., 244., 23., INK));
    commands.spawn((
        ConsoleText::Revision,
        label("", 242., 213., 13., ICE),
        TextBounds::new_horizontal(304.),
    ));

    for (left, text, metric) in [
        (242., "Committed writes", ConsoleText::Writes),
        (399., "Typed messages", ConsoleText::Messages),
    ] {
        commands.spawn(block((left + 73., 161.), (146., 64.), 4., BORDER));
        commands.spawn(block((left + 73., 161.), (144., 62.), 5., NIGHT));
        commands.spawn((metric, label("", left + 11., 170., 26., INK)));
        commands.spawn(label(text, left + 11., 143., 12., MUTED));
    }

    commands.spawn(label("Recent events", 242., 96., 16., INK));
    for index in 0..5 {
        let y = 69. - index as f32 * 64.;
        commands.spawn(block((394., y), (305., 1.), 5., BORDER));
        commands.spawn((
            ConsoleText::Log(index),
            top_left("", 242., y - 9., 14., MUTED),
            TextBounds::new(300., 50.),
        ));
    }
}

pub(crate) fn refresh(
    devices: Query<&Device>,
    console: Res<Console>,
    time: Res<Time>,
    mut sprites: Query<(&DeviceSprite, &mut Sprite, &mut Transform)>,
    mut labels: Query<(&MappedLabel, &mut Text2d, &mut TextColor), Without<ConsoleText>>,
    mut console_text: Query<(&ConsoleText, &mut Text2d, &mut TextColor)>,
) {
    for (marker, mut sprite, mut transform) in &mut sprites {
        let Ok(device) = devices.get(marker.device) else {
            continue;
        };
        let tint = station_color(device);
        match marker.part {
            Part::RelayCell => sprite.color = if device.active { AMBER } else { FLOOR_MARK },
            Part::GateLeaf(sign) => {
                let y = if device.active { 79. } else { 28. };
                let blend = (time.delta_secs() * 14.).min(1.);
                transform.translation.y += (sign * y - transform.translation.y) * blend;
                sprite.color = tint.with_alpha(0.84);
            }
            Part::BeaconLight => {
                sprite.color = tint.with_alpha(if device.active { 1. } else { 0.69 });
                let scale = if device.active {
                    1. + (time.elapsed_secs() * 7.).sin() * 0.12
                } else {
                    1.
                };
                transform.scale = Vec3::splat(scale);
            }
            Part::BeaconNeedle => {
                let angle = time.elapsed_secs() * 3.;
                transform.translation = (Vec2::from_angle(angle) * 63.).extend(6.);
                transform.rotation = Quat::from_rotation_z(angle);
                sprite.color = tint.with_alpha(if device.active { 0.85 } else { 0. });
            }
            Part::BeaconReadout => {
                let width = device.level.clamp(0., 1.) as f32 * 136.;
                sprite.custom_size = Some(Vec2::new(width, 4.));
                transform.translation.x = -68. + width / 2.;
                sprite.color = tint;
            }
        }
    }
    for (marker, mut text, mut color) in &mut labels {
        let Ok(device) = devices.get(marker.0) else {
            continue;
        };
        let detail = match device.station {
            Station::Relay => format!("{}   /   {} CHARGES", device.label, device.units),
            Station::Gate => device.label.clone(),
            Station::Beacon => format!("{}   /   {} SCANS", device.label, device.units),
        };
        if text.0 != detail {
            text.0 = detail;
        }
        let tint = station_color(device);
        if color.0 != tint {
            color.0 = tint;
        }
    }
    if !console.is_changed() {
        return;
    }
    for (field, mut text, mut color) in &mut console_text {
        let (label, tint) = match field {
            ConsoleText::Revision => (console.revision.clone(), ICE),
            ConsoleText::Writes => (console.writes.to_string(), INK),
            ConsoleText::Messages => (console.messages.to_string(), INK),
            ConsoleText::Hint => (console.hint.clone(), INK),
            ConsoleText::Log(index) => console
                .lines
                .get(*index)
                .map(|line| {
                    let tint = match line.tone {
                        Tone::Info => MUTED,
                        Tone::Good => MINT,
                        Tone::Warn => AMBER,
                    };
                    (line.text.clone(), tint)
                })
                .unwrap_or_else(|| (String::new(), MUTED)),
        };
        if text.0 != label {
            text.0 = label;
        }
        if color.0 != tint {
            color.0 = tint;
        }
    }
}

fn station_color(device: &Device) -> Color {
    match device.station {
        Station::Relay => AMBER,
        Station::Gate if device.active => MINT,
        Station::Gate => AMBER,
        Station::Beacon if device.label.starts_with("AURORA") => VIOLET,
        Station::Beacon => ICE,
    }
}

fn block(position: (f32, f32), size: (f32, f32), z: f32, color: Color) -> (Sprite, Transform) {
    (
        Sprite::from_color(color, Vec2::from(size)),
        Transform::from_xyz(position.0, position.1, z),
    )
}

fn stroke(from: Vec2, to: Vec2, width: f32, z: f32, color: Color) -> (Sprite, Transform) {
    let delta = to - from;
    let (sprite, mut transform) =
        block(((from + to) / 2.).into(), (delta.length(), width), z, color);
    transform.rotate_z(delta.to_angle());
    (sprite, transform)
}

fn label(text: impl Into<String>, x: f32, y: f32, px: f32, color: Color) -> impl Bundle {
    (
        Text2d::new(text),
        TextFont::from_font_size(FontSize::Px(px)),
        TextColor(color),
        Anchor::CENTER_LEFT,
        Transform::from_xyz(x, y, 40.),
    )
}

fn centered(text: impl Into<String>, x: f32, y: f32, px: f32, color: Color) -> impl Bundle {
    (
        Text2d::new(text),
        TextFont::from_font_size(FontSize::Px(px)),
        TextColor(color),
        TextLayout::justify(Justify::Center),
        Transform::from_xyz(x, y, 40.),
    )
}

fn top_left(text: impl Into<String>, x: f32, y: f32, px: f32, color: Color) -> impl Bundle {
    (
        Text2d::new(text),
        TextFont::from_font_size(FontSize::Px(px)),
        TextColor(color),
        Anchor::TOP_LEFT,
        Transform::from_xyz(x, y, 40.),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use reverse_bevy::ReversePlugin;

    #[test]
    fn the_scene_spawns_all_stations_and_the_player() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
                ..default()
            },
            ReversePlugin::default(),
        ));
        crate::gameplay::register(&mut app);
        app.add_systems(Startup, setup);
        app.update();

        let world = app.world_mut();
        assert_eq!(
            world.query::<(&Device, &VerseScript)>().iter(world).count(),
            3
        );
        assert_eq!(world.query::<&Player>().iter(world).count(), 1);
    }
}
