use crate::engine::{ENABLE_LOCAL_NPCS, Engine, MAX_AGENTS};
use crate::scripting::GameScript;
use crate::types::{Input, LaunchPadPhase, RemotePlayer};
use crate::ui::{UiPointerPhase, UiViewport};

impl Engine {
    pub(crate) fn set_input(&mut self, input: Input) {
        self.input = input;
    }

    pub(crate) fn set_ui_viewport(&mut self, viewport: UiViewport) {
        self.ui.borrow_mut().set_viewport(viewport);
    }

    pub(crate) fn ui_node_count(&self) -> usize {
        self.ui.borrow().document_node_count()
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
                true
            }
            Err(_) => {
                self.script = None;
                false
            }
        }
    }

    pub(crate) fn script_loaded(&self) -> bool {
        self.script.is_some()
    }
}
