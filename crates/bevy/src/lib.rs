//! Load Verse scripts as Bevy assets and run gameplay on the fixed clock.

mod asset;
mod bindings;
mod runtime;

pub use asset::VerseAsset;
pub use bindings::{ReverseAppExt, VerseValue};

use asset::VerseLoader;
use bevy_app::{App, FixedUpdate, Plugin, PreUpdate};
use bevy_asset::{AssetApp, AssetPlugin, Handle};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    message::Message,
    schedule::{IntoScheduleConfigs, SystemSet},
};
use bevy_time::TimePlugin;
use reverse_syntax::{Fix, Severity, Span};
use reverse_vm::Limits;

/// The Verse asset to run with this entity's registered components.
#[derive(Component, Clone)]
pub struct VerseScript(pub Handle<VerseAsset>);

/// Order Bevy producers before collection and consumers after the Verse tick.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum ReverseSet {
    /// Collect Bevy input messages in `PreUpdate`.
    Collect,
    /// Collect fixed-update inputs, step Verse, and commit effects in `FixedUpdate`.
    Tick,
}

/// A script passed checking and initialization and is ready to receive input.
#[derive(Clone, Debug, Message)]
pub struct VerseLoaded {
    pub entity: Entity,
    pub path: String,
}

/// A committed write to a registered property.
#[derive(Clone, Debug, Message)]
pub struct VerseWrite {
    pub entity: Entity,
    pub setter: String,
}

/// A line emitted by `Print`.
#[derive(Clone, Debug, Message)]
pub struct VerseOutput {
    pub entity: Entity,
    pub line: String,
}

/// A load, checking, or runtime diagnostic for an entity's script.
#[derive(Clone, Debug, Message)]
pub struct VerseDiagnostic {
    pub entity: Entity,
    pub path: String,
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub help: Option<String>,
    pub fix: Option<Fix>,
}

/// Install after Bevy's asset and time plugins.
#[derive(Clone)]
pub struct ReversePlugin {
    /// The step and output limits apply to each script on every fixed tick.
    pub limits: Limits,
}

impl Default for ReversePlugin {
    fn default() -> Self {
        Self {
            limits: Limits {
                max_steps: 50_000,
                max_depth: 100,
                max_output_bytes: 16 * 1024,
                ..Limits::default()
            },
        }
    }
}

impl Plugin for ReversePlugin {
    fn build(&self, app: &mut App) {
        assert!(
            app.is_plugin_added::<AssetPlugin>(),
            "add AssetPlugin before ReversePlugin"
        );
        assert!(
            app.is_plugin_added::<TimePlugin>(),
            "add TimePlugin before ReversePlugin"
        );
        app.init_asset::<VerseAsset>()
            .init_asset_loader::<VerseLoader>()
            .add_message::<VerseLoaded>()
            .add_message::<VerseWrite>()
            .add_message::<VerseOutput>()
            .add_message::<VerseDiagnostic>()
            .insert_non_send(runtime::Runtime::new(self.limits.clone()))
            .add_systems(PreUpdate, runtime::collect.in_set(ReverseSet::Collect))
            .add_systems(FixedUpdate, runtime::tick.in_set(ReverseSet::Tick));
    }
}
