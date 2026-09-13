use crate::engine::{ENABLE_LOCAL_NPCS, Engine, MAX_AGENTS};
use crate::types::{
    CharacterEmote, CharacterEntityKey, CharacterEntityKind, CharacterMotionEvent,
    CharacterMotionSample, CharacterMotionSource, CharacterSupport, LaunchPadPhase, Player,
};

impl Engine {
    pub fn step(&mut self, delta: f32) {
        let delta = delta.clamp(0.0, 0.05);
        self.elapsed += delta;
        self.motion_sequence = self.motion_sequence.saturating_add(1);
        self.player_motion_event = CharacterMotionEvent::None;
        self.ui.borrow_mut().advance(delta);
        self.apply_camera_input();
        self.smooth_camera(delta);
        self.update_player(delta);
        self.update_hazards(delta);
        self.update_player_checkpoints();
        self.update_interactions();
        self.sync_interaction_script_state();
        self.tick_script(delta);
        self.update_portals();
        if ENABLE_LOCAL_NPCS {
            self.spawn_agents();
            self.update_agents(delta);
        }
        for player in &mut self.remote_players {
            // Emotes are edge-triggered presentation inputs. Keep them alive
            // until the next simulation tick so a host can apply a packet and
            // sync immediately, then clear them before the next sample.
            player.emote = CharacterEmote::None;
            if player.moving {
                let speed = if player.sprinting { 13.0 } else { 9.0 };
                player.walk_cycle += delta * speed;
            } else {
                player.walk_cycle = 0.0;
            }
        }
        self.update_launch_pads();
        self.write_snapshot();
    }

    pub fn snapshot(&self) -> &[f32] {
        &self.snapshot
    }

    /// Returns a stable hash of the observable runtime state.
    ///
    /// This is intentionally a diagnostic/conformance surface, not a
    /// persistence format. It gives headless hosts a cheap way to compare two
    /// runs without depending on renderer state or pointer identity.
    pub fn state_hash(&self) -> u64 {
        let mut hash = StableHasher::default();
        hash.bytes(b"cubacadabra-engine-state-v1");
        hash.f32(self.elapsed);
        hash.usize(self.active_world);
        hash.string(self.active_world_id().unwrap_or_default());
        hash.u32(self.random.state());
        hash.f32(self.next_spawn_at);
        hash.f32(self.portal_cooldown_until);
        hash.bool(self.player_dead);
        hash.u32(self.player_deaths);
        hash.u32(self.player_respawn_event_id);
        hash.f32(self.player_respawn_at);
        hash.f32(self.player_health);
        hash.f32(self.player_max_health);
        hash.string(&self.checkpoint_id);
        hash.usize(self.checkpoint_index);
        hash.u32(self.launch_event_id);
        hash.u32(self.world_event_id);
        hash.usize(self.last_launch_pad);
        hash.usize(self.last_launch_occupants);
        hash.usize(self.last_world_source_pad);
        hash.usize(self.last_world_destination);
        hash.player(&self.player);
        hash.input(&self.input);
        hash.array3(self.pending_reconciliation);

        for value in &self.snapshot {
            hash.f32(*value);
        }
        for pad in &self.launch_pads {
            hash.f32(pad.x);
            hash.f32(pad.z);
            hash.f32(pad.radius);
            hash.f32(pad.countdown);
            hash.u8(pad.phase.code());
            hash.f32(pad.launch_at);
            hash.usize(pad.occupants);
            hash.bool(pad.enabled);
        }
        for agent in &self.agents {
            hash.array3(agent.position);
            hash.f32(agent.target.x);
            hash.f32(agent.target.z);
            hash.f32(agent.meeting_target.x);
            hash.f32(agent.meeting_target.z);
            hash.usize(agent.meeting_index);
            hash.u8(agent.phase.code() as u8);
            hash.f32(agent.spawned_at);
            hash.f32(agent.next_decision_at);
            hash.f32(agent.gather_at);
            hash.f32(agent.next_jump_at);
            hash.f32(agent.speed);
            hash.f32(agent.walk_cycle);
            hash.f32(agent.vertical_velocity);
            hash.bool(agent.grounded);
        }
        for player in &self.remote_players {
            hash.array3(player.position);
            hash.f32(player.yaw);
            hash.f32(player.look_yaw);
            hash.optional_array2(player.planar_velocity);
            hash.optional_f32(player.vertical_velocity);
            hash.bool(player.moving);
            hash.bool(player.sprinting);
            hash.f32(player.walk_cycle);
            hash.string(&player.stable_id);
            hash.string(&player.display_name);
            hash.u64(player.identity);
            hash.u32(player.generation);
            hash.u64(player.motion_sequence);
        }
        for (zone, state) in self
            .interactions
            .world
            .iter()
            .zip(self.interactions.states())
        {
            hash.string(&zone.id);
            hash.string(&zone.kind);
            hash.string(&zone.label);
            hash.array3(zone.position);
            hash.f32(zone.radius);
            hash.bool(state.inside);
            hash.usize(state.players);
            hash.u32(state.event_id);
        }
        hash.u32(self.interactions.event_id());
        for (key, value) in &self.effects.states {
            hash.usize(key.0);
            hash.string(&key.1);
            hash.string(value);
        }
        for instance in &self.effects.instances {
            hash.string(&instance.template);
            hash.array3(instance.position);
            hash.f32(instance.started_at);
            hash.usize(instance.world);
        }
        for event in &self.player_events {
            hash.player_event(event);
        }

        if let Some(script) = &self.script {
            let state = script.state();
            let state = state.borrow();
            hash.string(&state.lobby_status);
            hash.optional_bool(state.lobby_enabled);
            hash.optional_string(state.session_name.as_deref());
            hash.optional_string(state.last_error.as_deref());
            hash.u32(state.interactions.event_id);
            for zone in &state.interactions.zones {
                hash.string(&zone.id);
                hash.string(&zone.kind);
                hash.string(&zone.label);
                hash.bool(zone.inside);
                hash.bool(zone.nearby);
                hash.usize(zone.players);
            }
            for message in &state.network_outbox {
                hash.string(message);
            }
            for message in &state.network_inbox {
                hash.string(message);
            }
            for message in &state.audio_outbox {
                hash.string(message);
            }
            for command in &state.effect_outbox {
                match command {
                    crate::effects::EffectCommand::SetState { target, state } => {
                        hash.u8(0);
                        hash.string(target);
                        hash.string(state);
                    }
                    crate::effects::EffectCommand::Play { template, position } => {
                        hash.u8(1);
                        hash.string(template);
                        hash.array3(*position);
                    }
                }
            }
        }
        if let Ok(model) = serde_json::to_vec(&self.data_model.snapshot_state()) {
            hash.bytes(&model);
        }
        hash.finish()
    }

    pub fn set_reduced_effects(&mut self, reduced: bool) {
        self.reduced_effects = reduced;
    }

    pub(crate) fn reduced_effects(&self) -> bool {
        self.reduced_effects
    }

    pub(crate) fn trigger_local_wave(&mut self) {
        self.player_emote = CharacterEmote::Wave;
        self.player_emote_sequence = self.player_emote_sequence.wrapping_add(1).max(1);
    }

    pub(crate) fn agent_count(&self) -> usize {
        self.local_agent_count() + self.remote_player_count()
    }

    pub(crate) fn local_agent_count(&self) -> usize {
        self.agents
            .len()
            .min(MAX_AGENTS.saturating_sub(self.remote_player_count()))
    }

    pub(crate) fn remote_player_count(&self) -> usize {
        self.remote_players.len().min(MAX_AGENTS)
    }

    pub(crate) fn character_motion_samples(
        &self,
    ) -> impl Iterator<Item = CharacterMotionSample> + '_ {
        let local = CharacterMotionSample {
            key: CharacterEntityKey {
                kind: CharacterEntityKind::LocalPlayer,
                slot: 0,
                generation: 0,
                identity: 0,
            },
            sequence: self.motion_sequence,
            time: self.elapsed,
            position: self.player.position,
            facing_yaw: self.player.facing_yaw,
            look_yaw: if self.camera_distance <= 0.75 {
                self.view_yaw
            } else if self.player.velocity[0].hypot(self.player.velocity[2]) > 0.15 {
                // Let the gaze lead travel through a turn. Orbiting the
                // camera must not pull a running person's head sideways.
                (-self.player.velocity[0]).atan2(-self.player.velocity[2])
            } else {
                self.player.facing_yaw
            },
            planar_velocity: Some([self.player.velocity[0], self.player.velocity[2]]),
            vertical_velocity: Some(self.player.velocity[1]),
            support: player_support(self.player),
            stride_phase: self.player.walk_cycle,
            moving: self.player.moving,
            sprinting: self.player.sprinting,
            source: CharacterMotionSource::Simulation,
            event: self.player_motion_event,
            emote: self.player_emote,
            emote_sequence: self.player_emote_sequence,
            appearance_revision: self.player_appearance.revision,
        };
        let sequence = self.motion_sequence;
        let time = self.elapsed;
        std::iter::once(local)
            .chain(
                self.agents
                    .iter()
                    .take(self.local_agent_count())
                    .enumerate()
                    .map(move |(slot, agent)| {
                        let facing_yaw = (agent.target.x - agent.position[0])
                            .atan2(agent.target.z - agent.position[2]);
                        CharacterMotionSample {
                            key: CharacterEntityKey {
                                kind: CharacterEntityKind::LocalNpc,
                                slot,
                                generation: 0,
                                identity: 0,
                            },
                            sequence,
                            time,
                            position: agent.position,
                            facing_yaw,
                            look_yaw: facing_yaw,
                            planar_velocity: None,
                            vertical_velocity: Some(agent.vertical_velocity),
                            support: if agent.grounded {
                                CharacterSupport::Grounded {
                                    height: agent.position[1],
                                }
                            } else {
                                CharacterSupport::Airborne
                            },
                            stride_phase: agent.walk_cycle,
                            moving: agent.phase != crate::types::AgentPhase::Assembled,
                            sprinting: false,
                            source: CharacterMotionSource::Simulation,
                            event: CharacterMotionEvent::None,
                            emote: CharacterEmote::None,
                            emote_sequence: 0,
                            appearance_revision: 0,
                        }
                    }),
            )
            .chain(
                self.remote_players
                    .iter()
                    .filter(move |_| self.remote_world_matches_active())
                    .take(self.remote_player_count())
                    .enumerate()
                    .map(move |(slot, player)| CharacterMotionSample {
                        key: CharacterEntityKey {
                            kind: CharacterEntityKind::RemotePlayer,
                            slot: super::remote::stable_remote_slot(player, slot),
                            generation: player.generation,
                            identity: super::remote::remote_identity(player, slot),
                        },
                        sequence,
                        time,
                        position: player.position,
                        facing_yaw: player.yaw,
                        look_yaw: player.look_yaw,
                        planar_velocity: player.planar_velocity,
                        vertical_velocity: player.vertical_velocity,
                        support: player.support,
                        stride_phase: player.walk_cycle,
                        moving: player.moving,
                        sprinting: player.sprinting,
                        source: if player.stable_id.is_empty() {
                            CharacterMotionSource::LegacyRemote
                        } else {
                            CharacterMotionSource::VersionedRemote
                        },
                        event: CharacterMotionEvent::None,
                        emote: player.emote,
                        emote_sequence: player.emote_sequence,
                        appearance_revision: player.appearance.revision,
                    }),
            )
    }

    pub(crate) fn remote_world_matches_active(&self) -> bool {
        self.remote_world_id.as_deref().is_none_or(|remote_world| {
            self.world_ids
                .get(self.active_world)
                .is_none_or(|active_world| active_world == remote_world)
        })
    }

    pub(crate) fn elapsed(&self) -> f32 {
        self.elapsed
    }

    pub fn meeting_count(&self, index: usize) -> usize {
        self.count_launch_pad_occupants(index)
    }

    pub(crate) fn launch_pad_occupants(&self, index: usize) -> usize {
        self.launch_pads.get(index).map_or(0, |pad| pad.occupants)
    }

    pub(crate) fn launch_pad_count(&self) -> usize {
        self.launch_pads.len()
    }

    pub(crate) fn launch_pad_seconds(&self, index: usize) -> f32 {
        self.launch_pads.get(index).map_or(0.0, |pad| {
            if pad.phase == LaunchPadPhase::Countdown {
                (pad.launch_at - self.elapsed).max(0.0)
            } else {
                0.0
            }
        })
    }

    pub(crate) fn launch_pad_phase(&self, index: usize) -> u8 {
        self.launch_pads
            .get(index)
            .map_or(0, |pad| pad.phase.code())
    }

    pub(crate) fn player_launch_pad(&self) -> i32 {
        self.launch_pads
            .iter()
            .position(|pad| self.player_is_on_pad(pad))
            .map_or(-1, |index| index as i32)
    }

    pub(crate) fn launch_event_id(&self) -> u32 {
        self.launch_event_id
    }

    pub fn player_respawn_event_id(&self) -> u32 {
        self.player_respawn_event_id
    }

    pub(crate) fn last_launch_pad(&self) -> usize {
        self.last_launch_pad
    }

    pub(crate) fn last_launch_occupants(&self) -> usize {
        self.last_launch_occupants
    }

    pub(crate) fn active_world(&self) -> usize {
        self.active_world
    }

    pub fn active_world_id(&self) -> Option<&str> {
        self.world_ids.get(self.active_world).map(String::as_str)
    }

    pub(crate) fn settings_room_state(&self) -> u8 {
        let Some(room) = self
            .package
            .as_ref()
            .and_then(|package| package.settings_room.as_ref())
        else {
            return 0;
        };
        if self.world_ids.get(self.active_world).map(String::as_str) != Some(room.world_id.as_str())
        {
            return 0;
        }

        let distance = (self.player.position[0] - room.username_station_x())
            .hypot(self.player.position[2] - room.username_station_z());
        if distance <= room.interaction_radius.max(0.0) {
            2
        } else {
            1
        }
    }

    pub(crate) fn world_event_id(&self) -> u32 {
        self.world_event_id
    }

    pub(crate) fn last_world_source_pad(&self) -> usize {
        self.last_world_source_pad
    }

    pub(crate) fn last_world_destination(&self) -> usize {
        self.last_world_destination
    }
}

fn player_support(player: Player) -> CharacterSupport {
    if player.grounded {
        CharacterSupport::Grounded {
            height: player.position[1],
        }
    } else {
        CharacterSupport::Airborne
    }
}

#[derive(Default)]
struct StableHasher(u64);

impl StableHasher {
    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    fn finish(self) -> u64 {
        self.0
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    fn f32(&mut self, value: f32) {
        self.u32(value.to_bits());
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn string(&mut self, value: &str) {
        self.usize(value.len());
        self.bytes(value.as_bytes());
    }

    fn optional_string(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.string(value);
            }
            None => self.bool(false),
        }
    }

    fn optional_bool(&mut self, value: Option<bool>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.bool(value);
            }
            None => self.bool(false),
        }
    }

    fn optional_f32(&mut self, value: Option<f32>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.f32(value);
            }
            None => self.bool(false),
        }
    }

    fn array3(&mut self, value: [f32; 3]) {
        for value in value {
            self.f32(value);
        }
    }

    fn optional_array2(&mut self, value: Option<[f32; 2]>) {
        match value {
            Some(value) => {
                self.bool(true);
                for value in value {
                    self.f32(value);
                }
            }
            None => self.bool(false),
        }
    }

    fn player(&mut self, player: &Player) {
        self.array3(player.position);
        self.f32(player.facing_yaw);
        self.array3(player.velocity);
        self.bool(player.grounded);
        self.bool(player.climbing);
        self.bool(player.moving);
        self.bool(player.sprinting);
        self.f32(player.walk_cycle);
    }

    fn input(&mut self, input: &crate::types::Input) {
        self.f32(input.forward);
        self.f32(input.strafe);
        self.bool(input.sprint);
        self.bool(input.jump);
        self.bool(input.climb);
        self.f32(input.look_x);
        self.f32(input.look_y);
        self.f32(input.zoom_delta);
    }

    fn player_event(&mut self, event: &crate::types::PlayerEvent) {
        match event {
            crate::types::PlayerEvent::Spawn {
                health,
                max_health,
                deaths,
            } => {
                self.u8(0);
                self.f32(*health);
                self.f32(*max_health);
                self.u32(*deaths);
            }
            crate::types::PlayerEvent::Checkpoint { id, position } => {
                self.u8(1);
                self.string(id);
                self.array3(*position);
            }
            crate::types::PlayerEvent::Death {
                cause,
                checkpoint,
                deaths,
                health,
                max_health,
            } => {
                self.u8(2);
                self.string(cause);
                self.string(checkpoint);
                self.u32(*deaths);
                self.f32(*health);
                self.f32(*max_health);
            }
            crate::types::PlayerEvent::Respawn {
                checkpoint,
                deaths,
                health,
                max_health,
            } => {
                self.u8(3);
                self.string(checkpoint);
                self.u32(*deaths);
                self.f32(*health);
                self.f32(*max_health);
            }
            crate::types::PlayerEvent::Damage {
                source,
                amount,
                health,
                max_health,
            } => {
                self.u8(4);
                self.string(source);
                self.f32(*amount);
                self.f32(*health);
                self.f32(*max_health);
            }
            crate::types::PlayerEvent::Heal {
                source,
                amount,
                health,
                max_health,
            } => {
                self.u8(5);
                self.string(source);
                self.f32(*amount);
                self.f32(*health);
                self.f32(*max_health);
            }
        }
    }
}
