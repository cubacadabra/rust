use crate::data_model::DataModel;
use crate::engine::{Engine, SNAPSHOT_STRIDE};
use crate::math::bool_as_float;
use crate::types::{
    Agent, AgentPhase, BuildBlock, Input, InteractionEvent, LaunchPadPhase, Player,
};
use serde_json::Value;

pub const ENGINE_SNAPSHOT_FORMAT: &str = "cubacadabra.engine.snapshot";
pub const ENGINE_SNAPSHOT_VERSION: u16 = 1;
pub const MAX_ENGINE_SNAPSHOT_BYTES: usize = 4 * 1024 * 1024;
const MAX_GAME_STATE_BYTES: usize = 256 * 1024;
#[path = "snapshot_conversions.rs"]
mod snapshot_conversions;
#[path = "snapshot_legacy.rs"]
mod snapshot_legacy;
#[path = "snapshot_types.rs"]
mod snapshot_types;

pub use snapshot_types::*;

impl EngineSnapshot {
    pub fn to_json(&self) -> Result<String, SnapshotError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| SnapshotError::Serialization(error.to_string()))?;
        if bytes.len() > MAX_ENGINE_SNAPSHOT_BYTES {
            return Err(SnapshotError::Invalid(format!(
                "snapshot exceeds {MAX_ENGINE_SNAPSHOT_BYTES} bytes"
            )));
        }
        String::from_utf8(bytes).map_err(|error| SnapshotError::Serialization(error.to_string()))
    }

    pub fn from_json(source: &str) -> Result<Self, SnapshotError> {
        if source.len() > MAX_ENGINE_SNAPSHOT_BYTES {
            return Err(SnapshotError::Invalid(format!(
                "snapshot exceeds {MAX_ENGINE_SNAPSHOT_BYTES} bytes"
            )));
        }
        let snapshot: Self = serde_json::from_str(source)
            .map_err(|error| SnapshotError::Serialization(error.to_string()))?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    fn validate(&self) -> Result<(), SnapshotError> {
        if self.format != ENGINE_SNAPSHOT_FORMAT {
            return Err(SnapshotError::Invalid(format!(
                "unsupported format {}",
                self.format
            )));
        }
        if self.version != ENGINE_SNAPSHOT_VERSION {
            return Err(SnapshotError::Invalid(format!(
                "unsupported version {}",
                self.version
            )));
        }
        if self.random_state == 0 {
            return Err(SnapshotError::Invalid(
                "random state must be non-zero".to_owned(),
            ));
        }
        if self.all_floats().any(|value| !value.is_finite()) {
            return Err(SnapshotError::Invalid(
                "snapshot contains a non-finite number".to_owned(),
            ));
        }
        if serde_json::to_vec(&self.game_state)
            .map_err(|error| SnapshotError::Serialization(error.to_string()))?
            .len()
            > MAX_GAME_STATE_BYTES
        {
            return Err(SnapshotError::Invalid(format!(
                "game state exceeds {MAX_GAME_STATE_BYTES} bytes"
            )));
        }
        if self.pending_network_messages.len() > 64 {
            return Err(SnapshotError::Invalid(
                "too many pending network messages".to_owned(),
            ));
        }
        let mut pending_network_bytes = 0usize;
        for message in &self.pending_network_messages {
            if message.len() > crate::engine::identity::MAX_NETWORK_MESSAGE_BYTES
                || serde_json::from_str::<Value>(message).is_err()
            {
                return Err(SnapshotError::Invalid(
                    "snapshot contains an invalid network message".to_owned(),
                ));
            }
            pending_network_bytes = pending_network_bytes.saturating_add(message.len());
        }
        if pending_network_bytes > 256 * 1024 {
            return Err(SnapshotError::Invalid(
                "pending network messages exceed the queue byte limit".to_owned(),
            ));
        }
        if self.agents.len() > 128 || self.launch_pads.len() > 64 || self.build_blocks.len() > 256 {
            return Err(SnapshotError::Invalid(
                "snapshot contains too many runtime objects".to_owned(),
            ));
        }
        if self
            .interactions
            .zones
            .iter()
            .any(|zone| zone.id.is_empty())
        {
            return Err(SnapshotError::Invalid(
                "interaction IDs cannot be empty".to_owned(),
            ));
        }
        Ok(())
    }

    fn all_floats(&self) -> impl Iterator<Item = f32> + '_ {
        let values = self
            .player
            .position
            .iter()
            .copied()
            .chain(std::iter::once(self.player.facing_yaw))
            .chain(self.player.velocity.iter().copied())
            .chain([
                self.player.walk_cycle,
                self.player_runtime.respawn_at,
                self.player_runtime.health,
                self.player_runtime.max_health,
                self.player_runtime.damage_since_event,
                self.player_runtime.heal_since_event,
                self.player_runtime.next_damage_event_at,
                self.player_runtime.next_heal_event_at,
                self.elapsed,
                self.next_spawn_at,
                self.portal_cooldown_until,
            ])
            .chain(self.input.floats())
            .chain(self.camera.floats())
            .chain(self.respawn_position.iter().copied())
            .chain(self.pending_reconciliation.iter().copied());
        values
            .chain(
                self.build_blocks
                    .iter()
                    .flat_map(|block| block.position.into_iter().chain(block.size)),
            )
            .chain(
                self.launch_pads
                    .iter()
                    .flat_map(|pad| [pad.x, pad.z, pad.radius, pad.countdown, pad.launch_at]),
            )
            .chain(self.agents.iter().flat_map(|agent| {
                agent
                    .position
                    .into_iter()
                    .chain(agent.target)
                    .chain(agent.meeting_target)
                    .chain([
                        agent.spawned_at,
                        agent.next_decision_at,
                        agent.gather_at,
                        agent.next_jump_at,
                        agent.speed,
                        agent.walk_cycle,
                        agent.vertical_velocity,
                    ])
            }))
            .chain(
                self.pending_player_events
                    .iter()
                    .flat_map(player_event_floats),
            )
    }
}

fn player_event_floats(event: &PlayerEventSnapshot) -> Vec<f32> {
    match event {
        PlayerEventSnapshot::Spawn {
            health, max_health, ..
        }
        | PlayerEventSnapshot::Respawn {
            health, max_health, ..
        } => vec![*health, *max_health],
        PlayerEventSnapshot::Checkpoint { position, .. } => position.to_vec(),
        PlayerEventSnapshot::Death {
            health, max_health, ..
        }
        | PlayerEventSnapshot::Damage {
            health, max_health, ..
        }
        | PlayerEventSnapshot::Heal {
            health, max_health, ..
        } => vec![*health, *max_health],
    }
}

impl InputSnapshot {
    fn floats(&self) -> impl Iterator<Item = f32> + '_ {
        [
            self.forward,
            self.strafe,
            self.look_x,
            self.look_y,
            self.zoom_delta,
        ]
        .into_iter()
    }
}

impl CameraSnapshot {
    fn floats(&self) -> impl Iterator<Item = f32> + '_ {
        [
            self.view_yaw,
            self.view_pitch,
            self.target_yaw,
            self.target_pitch,
            self.distance,
            self.target_distance,
        ]
        .into_iter()
    }
}

impl Engine {
    /// Captures deterministic engine state and asks the game for its explicit
    /// JSON state blob. No renderer, socket, or host handle is touched.
    pub fn capture_snapshot(&self) -> Result<EngineSnapshot, SnapshotError> {
        let game_state = self.script.as_ref().map_or(Ok(Value::Null), |script| {
            script.save_state().map_err(SnapshotError::Script)
        })?;
        let snapshot = EngineSnapshot {
            format: ENGINE_SNAPSHOT_FORMAT.to_owned(),
            version: ENGINE_SNAPSHOT_VERSION,
            content_fingerprint: content_fingerprint(&self.package_buffer, &self.script_buffer),
            world_id: self.active_world_id().unwrap_or_default().to_owned(),
            simulation_tick: self.simulation_tick,
            elapsed: self.elapsed,
            random_state: self.random.state(),
            next_spawn_at: self.next_spawn_at,
            player: PlayerSnapshot::from(self.player),
            player_runtime: PlayerRuntimeSnapshot {
                dead: self.player_dead,
                respawn_at: self.player_respawn_at,
                respawn_event_id: self.player_respawn_event_id,
                deaths: self.player_deaths,
                health: self.player_health,
                max_health: self.player_max_health,
                damage_since_event: self.player_damage_since_event,
                heal_since_event: self.player_heal_since_event,
                next_damage_event_at: self.player_next_damage_event_at,
                next_heal_event_at: self.player_next_heal_event_at,
            },
            input: InputSnapshot::from(self.input),
            camera: CameraSnapshot {
                view_yaw: self.view_yaw,
                view_pitch: self.view_pitch,
                target_yaw: self.target_yaw,
                target_pitch: self.target_pitch,
                distance: self.camera_distance,
                target_distance: self.target_camera_distance,
            },
            build_blocks: self
                .build_blocks
                .iter()
                .copied()
                .map(BuildBlockSnapshot::from)
                .collect(),
            launch_pads: self
                .launch_pads
                .iter()
                .copied()
                .map(LaunchPadSnapshot::from)
                .collect(),
            agents: self
                .agents
                .iter()
                .copied()
                .map(AgentSnapshot::from)
                .collect(),
            interactions: InteractionSnapshot {
                event_id: self.interactions.event_id,
                zones: self
                    .interactions
                    .world
                    .iter()
                    .zip(
                        self.interactions
                            .states
                            .iter()
                            .zip(&self.interactions.was_inside),
                    )
                    .map(|(zone, (state, was_inside))| InteractionZoneSnapshot {
                        id: zone.id.clone(),
                        inside: state.inside,
                        players: state.players as u32,
                        event_id: state.event_id,
                        was_inside: *was_inside,
                    })
                    .collect(),
                pending_events: self
                    .interactions
                    .events
                    .iter()
                    .cloned()
                    .map(InteractionEventSnapshot::from)
                    .collect(),
            },
            pending_player_events: self
                .player_events
                .iter()
                .cloned()
                .map(PlayerEventSnapshot::from)
                .collect(),
            pending_network_messages: self.script.as_ref().map_or_else(
                Vec::new,
                crate::scripting::GameScript::pending_network_messages,
            ),
            launch_event_id: self.launch_event_id,
            last_launch_pad: self.last_launch_pad as u32,
            last_launch_occupants: self.last_launch_occupants as u32,
            world_event_id: self.world_event_id,
            last_world_source_pad: self.last_world_source_pad as u32,
            last_world_destination: self.last_world_destination as u32,
            portal_cooldown_until: self.portal_cooldown_until,
            pending_reconciliation: self.pending_reconciliation,
            data_model: self.data_model.snapshot(),
            game_state,
            checkpoint_id: self.checkpoint_id.clone(),
            checkpoint_index: self.checkpoint_index as u64,
            respawn_position: self.respawn_position,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn capture_snapshot_json(&self) -> Result<String, SnapshotError> {
        self.capture_snapshot()?.to_json()
    }

    /// Restores into an engine that has already loaded matching package and
    /// script content. Static geometry comes from that package.
    pub fn restore_snapshot(&mut self, snapshot: &EngineSnapshot) -> Result<(), SnapshotError> {
        snapshot.validate()?;
        let actual_fingerprint = content_fingerprint(&self.package_buffer, &self.script_buffer);
        if snapshot.content_fingerprint != actual_fingerprint {
            return Err(SnapshotError::ContentMismatch {
                expected: snapshot.content_fingerprint,
                actual: actual_fingerprint,
            });
        }
        if self.active_world_id().unwrap_or_default() != snapshot.world_id {
            return Err(SnapshotError::WorldNotLoaded(snapshot.world_id.clone()));
        }
        let mut validated_model = DataModel::new();
        validated_model.restore(&snapshot.data_model)?;
        validate_interactions(self, &snapshot.interactions)?;
        let random = crate::math::Random::from_state(snapshot.random_state)
            .ok_or_else(|| SnapshotError::Invalid("random state must be non-zero".to_owned()))?;
        let agents = snapshot
            .agents
            .iter()
            .map(Agent::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let launch_pads = snapshot
            .launch_pads
            .iter()
            .map(crate::world::LaunchPad::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let pending_player_events = snapshot
            .pending_player_events
            .iter()
            .cloned()
            .map(TryInto::try_into)
            .collect::<Result<std::collections::VecDeque<_>, SnapshotError>>()?;
        let pending_interaction_events = snapshot
            .interactions
            .pending_events
            .iter()
            .cloned()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, SnapshotError>>()?;

        // Run the game hook before mutating engine state so malformed or
        // rejected game-owned state cannot leave the engine half restored.
        if let Some(script) = &self.script {
            script
                .restore_state(&snapshot.game_state)
                .map_err(SnapshotError::Script)?;
            script
                .restore_pending_network_messages(&snapshot.pending_network_messages)
                .map_err(SnapshotError::Script)?;
        } else if !snapshot.game_state.is_null() || !snapshot.pending_network_messages.is_empty() {
            return Err(SnapshotError::Script(
                "snapshot requires a loaded script".to_owned(),
            ));
        }

        self.data_model.restore(&snapshot.data_model)?;
        self.simulation_tick = snapshot.simulation_tick;
        self.elapsed = snapshot.elapsed;
        self.random = random;
        self.next_spawn_at = snapshot.next_spawn_at;
        self.player = snapshot.player.clone().into();
        self.player_dead = snapshot.player_runtime.dead;
        self.player_respawn_at = snapshot.player_runtime.respawn_at;
        self.player_respawn_event_id = snapshot.player_runtime.respawn_event_id;
        self.player_deaths = snapshot.player_runtime.deaths;
        self.player_health = snapshot.player_runtime.health;
        self.player_max_health = snapshot.player_runtime.max_health;
        self.player_damage_since_event = snapshot.player_runtime.damage_since_event;
        self.player_heal_since_event = snapshot.player_runtime.heal_since_event;
        self.player_next_damage_event_at = snapshot.player_runtime.next_damage_event_at;
        self.player_next_heal_event_at = snapshot.player_runtime.next_heal_event_at;
        self.input = snapshot.input.clone().into();
        self.view_yaw = snapshot.camera.view_yaw;
        self.view_pitch = snapshot.camera.view_pitch;
        self.target_yaw = snapshot.camera.target_yaw;
        self.target_pitch = snapshot.camera.target_pitch;
        self.camera_distance = snapshot.camera.distance;
        self.target_camera_distance = snapshot.camera.target_distance;
        self.build_blocks = snapshot
            .build_blocks
            .iter()
            .cloned()
            .map(Into::into)
            .collect();
        self.rebuild_build_obstacles();
        self.launch_pads = launch_pads;
        self.agents = agents;
        self.interactions.event_id = snapshot.interactions.event_id;
        self.interactions.states = snapshot
            .interactions
            .zones
            .iter()
            .map(|zone| crate::types::InteractionRenderState {
                inside: zone.inside,
                players: zone.players as usize,
                event_id: zone.event_id,
            })
            .collect();
        self.interactions.was_inside = snapshot
            .interactions
            .zones
            .iter()
            .map(|zone| zone.was_inside)
            .collect();
        self.interactions.events = pending_interaction_events;
        self.player_events = pending_player_events;
        self.launch_event_id = snapshot.launch_event_id;
        self.last_launch_pad = snapshot.last_launch_pad as usize;
        self.last_launch_occupants = snapshot.last_launch_occupants as usize;
        self.world_event_id = snapshot.world_event_id;
        self.last_world_source_pad = snapshot.last_world_source_pad as usize;
        self.last_world_destination = snapshot.last_world_destination as usize;
        self.portal_cooldown_until = snapshot.portal_cooldown_until;
        self.pending_reconciliation = snapshot.pending_reconciliation;
        self.checkpoint_id.clone_from(&snapshot.checkpoint_id);
        self.checkpoint_index = usize::try_from(snapshot.checkpoint_index).map_err(|_| {
            SnapshotError::Invalid("checkpoint index does not fit this target".to_owned())
        })?;
        self.respawn_position = snapshot.respawn_position;
        if self.script.is_some() {
            self.sync_interaction_script_state();
        }
        self.effects = crate::effects::EffectRuntime::default();
        self.write_snapshot();
        Ok(())
    }

    pub fn restore_snapshot_json(&mut self, source: &str) -> Result<(), SnapshotError> {
        self.restore_snapshot(&EngineSnapshot::from_json(source)?)
    }

    pub(crate) fn prepare_snapshot_buffer(&mut self, length: usize) -> *mut u8 {
        if length > MAX_ENGINE_SNAPSHOT_BYTES {
            self.snapshot_buffer.clear();
            self.snapshot_error_buffer = b"snapshot exceeds the maximum size".to_vec();
            return std::ptr::null_mut();
        }
        self.snapshot_buffer.resize(length, 0);
        self.snapshot_buffer.as_mut_ptr()
    }

    pub(crate) fn load_snapshot_buffer(&mut self) -> bool {
        let source = match std::str::from_utf8(&self.snapshot_buffer) {
            Ok(source) => source.to_owned(),
            Err(error) => {
                self.snapshot_error_buffer = error.to_string().into_bytes();
                return false;
            }
        };
        match self.restore_snapshot_json(&source) {
            Ok(()) => {
                self.snapshot_error_buffer.clear();
                true
            }
            Err(error) => {
                self.snapshot_error_buffer = error.to_string().into_bytes();
                false
            }
        }
    }

    pub(crate) fn capture_snapshot_buffer(&mut self) -> bool {
        match self.capture_snapshot_json() {
            Ok(source) => {
                self.snapshot_output_buffer = source.into_bytes();
                self.snapshot_error_buffer.clear();
                true
            }
            Err(error) => {
                self.snapshot_output_buffer.clear();
                self.snapshot_error_buffer = error.to_string().into_bytes();
                false
            }
        }
    }
}

fn validate_interactions(
    engine: &Engine,
    snapshot: &InteractionSnapshot,
) -> Result<(), SnapshotError> {
    if snapshot.zones.len() != engine.interactions.world.len() {
        return Err(SnapshotError::Invalid(
            "interaction zone count does not match loaded world".to_owned(),
        ));
    }
    for (zone, state) in engine.interactions.world.iter().zip(&snapshot.zones) {
        if zone.id != state.id {
            return Err(SnapshotError::Invalid(format!(
                "interaction zone order differs at {}",
                state.id
            )));
        }
    }
    Ok(())
}

fn content_fingerprint(package: &[u8], script: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for bytes in [package, script] {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

// The legacy eight-float renderer ABI remains separate from EngineSnapshot.
