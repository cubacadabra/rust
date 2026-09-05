//! Bounded, additive identity and remote-motion protocol adapters.
//!
//! The host owns persistence, entitlement and transport. The engine only
//! validates a small versioned message, resolves it to bundled assets, and
//! keeps enough identity metadata for presentation continuity.

use crate::game_package::CharacterDefinition;
use serde::Deserialize;

pub(crate) const MAX_APPEARANCE_BYTES: usize = 4 * 1024;
pub(crate) const MAX_REMOTE_UPDATE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_REMOTE_ID_BYTES: usize = 96;
pub(crate) const MAX_WORLD_ID_BYTES: usize = 96;
pub(crate) const REMOTE_PROTOCOL_VERSION: u16 = 1;

/// Status returned by the additive buffer APIs. `Fallback` means the message
/// was accepted but one or more fields resolved to a bundled safe default.
pub(crate) const STATUS_INVALID: u8 = 0;
pub(crate) const STATUS_APPLIED: u8 = 1;
pub(crate) const STATUS_STALE: u8 = 2;
pub(crate) const STATUS_FALLBACK: u8 = 3;
pub(crate) const STATUS_DUPLICATE: u8 = 4;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteStateMessage {
    pub(crate) version: u16,
    pub(crate) sequence: u64,
    #[serde(default)]
    pub(crate) world_id: Option<String>,
    #[serde(default)]
    pub(crate) players: Vec<RemotePlayerMessage>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemotePlayerMessage {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) generation: u32,
    pub(crate) position: [f32; 3],
    pub(crate) yaw: f32,
    #[serde(default)]
    pub(crate) look_yaw: Option<f32>,
    #[serde(default)]
    pub(crate) planar_velocity: Option<[f32; 2]>,
    #[serde(default)]
    pub(crate) vertical_velocity: Option<f32>,
    #[serde(default)]
    pub(crate) grounded: Option<bool>,
    #[serde(default)]
    pub(crate) support_height: Option<f32>,
    #[serde(default)]
    pub(crate) stride_phase: Option<f32>,
    #[serde(default)]
    pub(crate) moving: bool,
    #[serde(default)]
    pub(crate) sprinting: bool,
    #[serde(default)]
    pub(crate) motion_sequence: Option<u64>,
    #[serde(default)]
    pub(crate) emote: Option<String>,
    #[serde(default)]
    pub(crate) emote_sequence: Option<u64>,
    #[serde(default)]
    pub(crate) appearance: Option<CharacterDefinition>,
}

pub(crate) fn bounded_utf8(bytes: &[u8], max: usize) -> Option<&str> {
    (bytes.len() <= max).then(|| std::str::from_utf8(bytes).ok())?
}

pub(crate) fn valid_identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.is_ascii()
        && value.bytes().all(|byte| !byte.is_ascii_control())
}

pub(crate) fn stable_identity(value: &str) -> u64 {
    // FNV-1a is deterministic across native/WASM and cheap enough for a
    // roster update. The original string remains on the remote record so the
    // hash is only a compact presentation key, never an authority boundary.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash.max(1)
}

pub(crate) fn valid_remote_message(message: &RemoteStateMessage) -> bool {
    message.version == REMOTE_PROTOCOL_VERSION
        && message.sequence > 0
        && message.world_id.as_deref().is_none_or(|world| {
            valid_identifier(world, MAX_WORLD_ID_BYTES)
        })
        && message.players.len() <= crate::engine::MAX_AGENTS
        && message.players.iter().all(valid_remote_player)
}

fn valid_remote_player(player: &RemotePlayerMessage) -> bool {
    valid_identifier(&player.id, MAX_REMOTE_ID_BYTES)
        && player.position.iter().all(|value| value.is_finite())
        && player.yaw.is_finite()
        && player.look_yaw.is_none_or(f32::is_finite)
        && player
            .planar_velocity
            .is_none_or(|velocity| velocity.iter().all(|value| value.is_finite()))
        && player.vertical_velocity.is_none_or(f32::is_finite)
        && player.support_height.is_none_or(f32::is_finite)
        && player.stride_phase.is_none_or(f32::is_finite)
        && player
            .emote
            .as_deref()
            .is_none_or(|emote| emote.len() <= 32 && emote.is_ascii())
        && player
            .appearance
            .as_ref()
            .is_none_or(CharacterDefinition::bounded)
}

pub(crate) fn parse_remote_message(source: &str) -> Result<RemoteStateMessage, ()> {
    let message: RemoteStateMessage = serde_json::from_str(source).map_err(|_| ())?;
    valid_remote_message(&message).then_some(message).ok_or(())
}
