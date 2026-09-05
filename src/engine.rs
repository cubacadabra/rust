mod camera;
mod initialization;
mod simulation;
mod snapshot;
mod state;
mod worlds;

#[cfg(test)]
mod tests;

use crate::game_package::GamePackageDefinition;
use crate::math::Random;
use crate::scripting::GameScript;
use crate::types::{Agent, BuildBlock, Input, Player, RemotePlayer};
use crate::ui::UiRuntime;
use crate::world::{Aabb, LaunchPad, RuntimeWorld};
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) const TOTAL_PLAYERS: usize = 18;
pub(crate) const MAX_AGENTS: usize = TOTAL_PLAYERS - 1;
pub(crate) const SNAPSHOT_STRIDE: usize = 8;

// Local NPCs are paused until the World Durable Object becomes authoritative.
// Keeping this switch here preserves the existing simulation for that later
// integration without showing divergent agents on different clients.
const ENABLE_LOCAL_NPCS: bool = false;

pub(crate) const BODY_HEIGHT: f32 = 3.15;
pub(crate) const PLAYER_RADIUS: f32 = 0.52;
pub(crate) const WALK_SPEED: f32 = 6.4;
pub(crate) const RUN_SPEED: f32 = 11.5;
pub(crate) const ACCELERATION: f32 = 22.0;
pub(crate) const AIR_ACCELERATION: f32 = 9.0;
pub(crate) const GRAVITY: f32 = 28.0;
pub(crate) const JUMP_VELOCITY: f32 = 10.5;
pub(crate) const WORLD_LIMIT: f32 = 57.5;
pub(crate) const DEFAULT_LAUNCH_COUNTDOWN: f32 = 8.0;

const LOOK_SENSITIVITY: f32 = 0.0062;
// Let the classic third-person camera orbit to an almost overhead view while
// staying just short of the look-at singularity.
const MAX_PITCH: f32 = 1.45;
// Leave enough room for a genuinely high bird's-eye view of the world.
const MAX_CAMERA_DISTANCE: f32 = 48.0;

pub struct Engine {
    pub(crate) player: Player,
    pub(crate) agents: Vec<Agent>,
    pub(crate) remote_players: Vec<RemotePlayer>,
    pub(crate) obstacles: Vec<Aabb>,
    pub(crate) base_obstacles: Vec<Aabb>,
    pub(crate) build_blocks: Vec<BuildBlock>,
    pub(crate) launch_pads: Vec<LaunchPad>,
    pub(crate) input: Input,
    pub(crate) elapsed: f32,
    pub(crate) next_spawn_at: f32,
    pub(crate) view_yaw: f32,
    pub(crate) view_pitch: f32,
    pub(crate) target_yaw: f32,
    pub(crate) target_pitch: f32,
    pub(crate) camera_distance: f32,
    pub(crate) target_camera_distance: f32,
    pub(crate) random: Random,
    pub(crate) snapshot: Vec<f32>,
    pub(crate) motion_sequence: u64,
    pub(crate) launch_event_id: u32,
    pub(crate) last_launch_pad: usize,
    pub(crate) last_launch_occupants: usize,
    pub(crate) worlds: Vec<RuntimeWorld>,
    pub(crate) active_world: usize,
    pub(crate) world_event_id: u32,
    pub(crate) last_world_source_pad: usize,
    pub(crate) last_world_destination: usize,
    pub(crate) script: Option<GameScript>,
    pub(crate) script_buffer: Vec<u8>,
    pub(crate) script_error_buffer: Vec<u8>,
    pub(crate) package: Option<GamePackageDefinition>,
    pub(crate) package_generation: u32,
    pub(crate) package_buffer: Vec<u8>,
    pub(crate) authoritative_launch: bool,
    pub(crate) world_ids: Vec<String>,
    pub(crate) username: String,
    pub(crate) username_buffer: Vec<u8>,
    pub(crate) portal_cooldown_until: f32,
    pub(crate) ui: Rc<RefCell<UiRuntime>>,
    pub(crate) ui_document_buffer: Vec<u8>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
