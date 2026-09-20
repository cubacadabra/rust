use std::collections::BTreeMap;

use cubacadabra_morphs::MorphParameterValue;
use serde::{Deserialize, Deserializer};

use crate::effects::EffectLibraryDefinition;

mod environment;
#[allow(unused_imports)]
pub(crate) use environment::{
    CameraDefinition, ColorCorrectionDefinition, DaylightDefinition, PresentationBoundsDefinition,
    SunRaysDefinition, VisualSettingsDefinition,
};
#[path = "game_package_definitions.rs"]
mod game_package_definitions;
pub(crate) use game_package_definitions::*;

fn default_start_world() -> String {
    "lobby".to_owned()
}

fn default_lobby_enabled() -> bool {
    true
}

fn default_ground_size() -> f32 {
    120.0
}

fn default_grid_size() -> f32 {
    112.0
}

fn default_grid_divisions() -> usize {
    28
}

fn default_true() -> bool {
    true
}

fn default_radius() -> f32 {
    2.7
}

fn default_countdown() -> f32 {
    8.0
}

fn default_scale() -> f32 {
    1.0
}

fn default_gravity() -> f32 {
    28.0
}

fn default_jump_velocity() -> f32 {
    10.5
}

fn default_ground_collision() -> bool {
    true
}

fn default_void_y() -> f32 {
    -100.0
}

fn default_respawn_delay() -> f32 {
    0.55
}

fn default_climb_speed() -> f32 {
    4.5
}

fn default_max_health() -> f32 {
    100.0
}

fn default_start_health() -> f32 {
    100.0
}

fn default_respawn_mode() -> String {
    "checkpoint".to_owned()
}

pub(crate) const PREVIEW_SDK_VERSION: &str = "0.3.0";
pub(crate) const TERRAIN_SDK_VERSION: &str = "0.4.0";
pub(crate) const LEGACY_CURRENT_SDK_VERSION: &str = "0.5.0";
pub(crate) const CURRENT_SDK_VERSION: &str = "0.6.0";

fn deserialize_sdk_version<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error as _;

    let value = Option::<String>::deserialize(deserializer)?;
    if let Some(version) = value.as_deref()
        && version != PREVIEW_SDK_VERSION
        && version != TERRAIN_SDK_VERSION
        && version != LEGACY_CURRENT_SDK_VERSION
        && version != CURRENT_SDK_VERSION
    {
        return Err(D::Error::custom(format!(
            "unsupported sdkVersion {version:?}; runtime supports {PREVIEW_SDK_VERSION}, {TERRAIN_SDK_VERSION}, {LEGACY_CURRENT_SDK_VERSION}, and {CURRENT_SDK_VERSION}"
        )));
    }
    Ok(value)
}

#[cfg(test)]
#[path = "game_package_tests.rs"]
mod game_package_tests;
