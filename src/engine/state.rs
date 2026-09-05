use crate::engine::{ENABLE_LOCAL_NPCS, Engine, MAX_AGENTS};
use crate::scripting::GameScript;
use crate::types::{
    CharacterEntityKey, CharacterEntityKind, CharacterMotionSample, CharacterMotionSource,
    CharacterSupport, Input, LaunchPadPhase, Player, RemotePlayer,
};
use crate::ui::{UiPointerPhase, UiViewport};

impl Engine {
    pub(crate) fn set_input(&mut self, input: Input) {
        self.input = input;
    }

    pub(crate) fn set_ui_viewport(&mut self, viewport: UiViewport) {
        self.ui.borrow_mut().set_viewport(viewport);
    }

    pub(crate) fn set_authenticated(&mut self, authenticated: bool) {
        self.ui.borrow_mut().set_authenticated(authenticated);
    }

    pub(crate) fn ui_node_count(&self) -> usize {
        self.ui.borrow().document_node_count()
    }

    pub(crate) fn ui_hit_test(&mut self, x: f32, y: f32) -> bool {
        self.ui.borrow_mut().is_interactive_at(x, y)
    }

    pub(crate) fn ui_external_link_hit_test(&mut self, x: f32, y: f32) -> bool {
        self.ui.borrow_mut().is_external_link_at(x, y)
    }

    pub(crate) fn ui_shared_modal_visible(&self) -> bool {
        self.ui.borrow().shared_modal_visible()
    }

    pub(crate) fn set_ui_document(&mut self, source: &str) -> bool {
        self.ui.borrow_mut().set_document_json(source).is_ok()
    }

    pub(crate) fn prepare_ui_document_buffer(&mut self, length: usize) -> *mut u8 {
        self.ui_document_buffer.resize(length, 0);
        self.ui_document_buffer.as_mut_ptr()
    }

    pub(crate) fn load_ui_document_buffer(&mut self) -> bool {
        let source = String::from_utf8_lossy(&self.ui_document_buffer).into_owned();
        self.set_ui_document(&source)
    }

    pub(crate) fn ui_pointer(
        &mut self,
        pointer_id: u64,
        phase: UiPointerPhase,
        x: f32,
        y: f32,
    ) -> bool {
        self.ui.borrow_mut().pointer(pointer_id, phase, x, y)
    }

    pub fn step(&mut self, delta: f32) {
        let delta = delta.clamp(0.0, 0.05);
        self.elapsed += delta;
        self.motion_sequence = self.motion_sequence.saturating_add(1);
        self.ui.borrow_mut().advance(delta);
        self.tick_script(delta);
        self.apply_camera_input();
        self.smooth_camera(delta);
        self.update_player(delta);
        self.update_portals();
        if ENABLE_LOCAL_NPCS {
            self.spawn_agents();
            self.update_agents(delta);
        }
        for player in &mut self.remote_players {
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
            },
            sequence: self.motion_sequence,
            time: self.elapsed,
            position: self.player.position,
            facing_yaw: player_facing_yaw(self.player, self.view_yaw),
            look_yaw: self.view_yaw,
            planar_velocity: Some([self.player.velocity[0], self.player.velocity[2]]),
            vertical_velocity: Some(self.player.velocity[1]),
            support: player_support(self.player),
            stride_phase: self.player.walk_cycle,
            moving: self.player.moving,
            sprinting: self.player.sprinting,
            source: CharacterMotionSource::Simulation,
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
                        }
                    }),
            )
            .chain(
                self.remote_players
                    .iter()
                    .take(self.remote_player_count())
                    .enumerate()
                    .map(move |(slot, player)| CharacterMotionSample {
                        key: CharacterEntityKey {
                            kind: CharacterEntityKind::RemotePlayer,
                            slot,
                            generation: 0,
                        },
                        sequence,
                        time,
                        position: player.position,
                        facing_yaw: player.yaw,
                        look_yaw: player.yaw,
                        planar_velocity: None,
                        vertical_velocity: None,
                        support: CharacterSupport::Unknown,
                        stride_phase: player.walk_cycle,
                        moving: player.moving,
                        sprinting: player.sprinting,
                        source: CharacterMotionSource::LegacyRemote,
                    }),
            )
    }

    pub(crate) fn set_remote_player_count(&mut self, count: usize) {
        self.remote_players
            .resize(count.min(MAX_AGENTS), RemotePlayer::default());
        self.write_snapshot();
    }

    pub(crate) fn set_remote_player(
        &mut self,
        index: usize,
        position: [f32; 3],
        yaw: f32,
        moving: bool,
        sprinting: bool,
    ) {
        if let Some(player) = self.remote_players.get_mut(index) {
            player.position = position;
            player.yaw = yaw;
            player.moving = moving;
            player.sprinting = sprinting;
        }
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

    pub(crate) fn prepare_script_buffer(&mut self, length: usize) -> *mut u8 {
        self.script_buffer.resize(length, 0);
        self.script_buffer.as_mut_ptr()
    }

    pub(crate) fn prepare_package_buffer(&mut self, length: usize) -> *mut u8 {
        self.package_buffer.resize(length, 0);
        self.package_buffer.as_mut_ptr()
    }

    pub(crate) fn prepare_username_buffer(&mut self, length: usize) -> *mut u8 {
        self.username_buffer.resize(length, 0);
        self.username_buffer.as_mut_ptr()
    }

    pub(crate) fn load_username_buffer(&mut self) -> bool {
        let Ok(source) = std::str::from_utf8(&self.username_buffer) else {
            return false;
        };
        let username = source
            .trim()
            .chars()
            .filter(|character| {
                character.is_ascii_alphanumeric() || matches!(character, ' ' | '_' | '-')
            })
            .take(24)
            .collect::<String>();
        if username.len() < 2 {
            return false;
        }
        self.username = username;
        true
    }

    pub(crate) fn load_script_buffer(&mut self) -> bool {
        let source = String::from_utf8_lossy(&self.script_buffer).into_owned();
        match GameScript::load(&source, std::rc::Rc::clone(&self.ui)) {
            Ok(script) => {
                self.script = Some(script);
                self.script_error_buffer.clear();
                true
            }
            Err(error) => {
                self.script = None;
                self.script_error_buffer = error.into_bytes();
                false
            }
        }
    }

    pub(crate) fn script_error_buffer(&self) -> &[u8] {
        &self.script_error_buffer
    }

    pub(crate) fn script_loaded(&self) -> bool {
        self.script.is_some()
    }
}

fn player_facing_yaw(player: Player, fallback: f32) -> f32 {
    let planar_speed = player.velocity[0].hypot(player.velocity[2]);
    if planar_speed > 0.001 {
        (-player.velocity[0]).atan2(-player.velocity[2])
    } else {
        fallback
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
