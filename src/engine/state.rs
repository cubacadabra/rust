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

    pub(crate) fn last_launch_pad(&self) -> usize {
        self.last_launch_pad
    }

    pub(crate) fn last_launch_occupants(&self) -> usize {
        self.last_launch_occupants
    }

    pub(crate) fn active_world(&self) -> usize {
        self.active_world
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
