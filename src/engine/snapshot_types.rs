use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

use crate::data_model::DataModelSnapshot;

/// A portable, versioned freeze-frame of authoritative deterministic engine
/// state. Static package content is identified by `content_fingerprint` and
/// must be loaded before restoring; GPU objects, sockets, caches, UI gesture
/// state, remote presentation state, and Lua VM internals are excluded.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineSnapshot {
    pub format: String,
    pub version: u16,
    pub content_fingerprint: u64,
    pub world_id: String,
    pub simulation_tick: u64,
    pub elapsed: f32,
    pub random_state: u32,
    pub next_spawn_at: f32,
    pub player: PlayerSnapshot,
    pub player_runtime: PlayerRuntimeSnapshot,
    pub input: InputSnapshot,
    pub camera: CameraSnapshot,
    pub build_blocks: Vec<BuildBlockSnapshot>,
    pub launch_pads: Vec<LaunchPadSnapshot>,
    pub agents: Vec<AgentSnapshot>,
    pub interactions: InteractionSnapshot,
    pub pending_player_events: Vec<PlayerEventSnapshot>,
    pub pending_network_messages: Vec<String>,
    pub launch_event_id: u32,
    pub last_launch_pad: u32,
    pub last_launch_occupants: u32,
    pub world_event_id: u32,
    pub last_world_source_pad: u32,
    pub last_world_destination: u32,
    pub portal_cooldown_until: f32,
    pub pending_reconciliation: [f32; 3],
    pub data_model: DataModelSnapshot,
    /// Meaningful game state returned by the script's explicit `on_save`
    /// callback and restored through `on_restore`.
    pub game_state: Value,
    pub checkpoint_id: String,
    pub checkpoint_index: u64,
    pub respawn_position: [f32; 3],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub position: [f32; 3],
    pub facing_yaw: f32,
    pub velocity: [f32; 3],
    pub grounded: bool,
    pub climbing: bool,
    pub moving: bool,
    pub sprinting: bool,
    pub walk_cycle: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerRuntimeSnapshot {
    pub dead: bool,
    pub respawn_at: f32,
    pub respawn_event_id: u32,
    pub deaths: u32,
    pub health: f32,
    pub max_health: f32,
    pub damage_since_event: f32,
    pub heal_since_event: f32,
    pub next_damage_event_at: f32,
    pub next_heal_event_at: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputSnapshot {
    pub forward: f32,
    pub strafe: f32,
    pub sprint: bool,
    pub jump: bool,
    pub climb: bool,
    pub look_x: f32,
    pub look_y: f32,
    pub zoom_delta: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraSnapshot {
    pub view_yaw: f32,
    pub view_pitch: f32,
    pub target_yaw: f32,
    pub target_pitch: f32,
    pub distance: f32,
    pub target_distance: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildBlockSnapshot {
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub color: u32,
    pub rotation: u8,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPadSnapshot {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
    pub countdown: f32,
    pub phase: u8,
    pub launch_at: f32,
    pub occupants: u32,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSnapshot {
    pub position: [f32; 3],
    pub target: [f32; 2],
    pub meeting_target: [f32; 2],
    pub meeting_index: u32,
    pub phase: u8,
    pub spawned_at: f32,
    pub next_decision_at: f32,
    pub gather_at: f32,
    pub next_jump_at: f32,
    pub speed: f32,
    pub walk_cycle: f32,
    pub vertical_velocity: f32,
    pub grounded: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionSnapshot {
    pub event_id: u32,
    pub zones: Vec<InteractionZoneSnapshot>,
    pub pending_events: Vec<InteractionEventSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionZoneSnapshot {
    pub id: String,
    pub inside: bool,
    pub players: u32,
    pub event_id: u32,
    pub was_inside: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionEventSnapshot {
    pub id: String,
    pub phase: String,
    pub players: u32,
    #[serde(default)]
    pub position: [f32; 3],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum PlayerEventSnapshot {
    Spawn {
        health: f32,
        max_health: f32,
        deaths: u32,
    },
    Checkpoint {
        id: String,
        position: [f32; 3],
    },
    Death {
        cause: String,
        checkpoint: String,
        deaths: u32,
        health: f32,
        max_health: f32,
    },
    Respawn {
        checkpoint: String,
        deaths: u32,
        health: f32,
        max_health: f32,
    },
    Damage {
        source: String,
        amount: f32,
        health: f32,
        max_health: f32,
    },
    Heal {
        source: String,
        amount: f32,
        health: f32,
        max_health: f32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotError {
    Invalid(String),
    Serialization(String),
    ContentMismatch { expected: u64, actual: u64 },
    WorldNotLoaded(String),
    DataModel(crate::data_model::DataModelError),
    Script(String),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => write!(formatter, "invalid engine snapshot: {reason}"),
            Self::Serialization(reason) => {
                write!(formatter, "snapshot serialization failed: {reason}")
            }
            Self::ContentMismatch { expected, actual } => write!(
                formatter,
                "snapshot content fingerprint {expected:016x} does not match loaded content {actual:016x}"
            ),
            Self::WorldNotLoaded(world) => {
                write!(formatter, "snapshot world is not loaded: {world}")
            }
            Self::DataModel(error) => error.fmt(formatter),
            Self::Script(error) => write!(formatter, "snapshot script hook failed: {error}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

impl From<crate::data_model::DataModelError> for SnapshotError {
    fn from(error: crate::data_model::DataModelError) -> Self {
        Self::DataModel(error)
    }
}
