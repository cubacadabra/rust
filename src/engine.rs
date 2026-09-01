use crate::math::{Random, bool_as_float, damp, horizontal_distance};
use crate::types::{Agent, AgentPhase, Input, LaunchPadPhase, Player};
use crate::world::{Aabb, LaunchPad, block_bounds};

pub(crate) const TOTAL_PLAYERS: usize = 18;
pub(crate) const MAX_AGENTS: usize = TOTAL_PLAYERS - 1;
pub(crate) const SNAPSHOT_STRIDE: usize = 8;

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
const MAX_PITCH: f32 = 1.1;
const MAX_CAMERA_DISTANCE: f32 = 16.0;

pub struct Engine {
    pub(crate) player: Player,
    pub(crate) agents: Vec<Agent>,
    pub(crate) obstacles: Vec<Aabb>,
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
    pub(crate) launch_event_id: u32,
    pub(crate) last_launch_pad: usize,
    pub(crate) last_launch_occupants: usize,
}

impl Engine {
    pub fn new() -> Self {
        let obstacles = vec![
            block_bounds([0.15, 1.35, -13.5], [2.8, 2.7, 2.8]),
            block_bounds([-8.1, 0.8, -22.0], [3.8, 1.6, 3.8]),
            block_bounds([8.4, 1.5, -27.0], [2.2, 3.0, 2.2]),
            block_bounds([13.0, 0.5, -17.0], [1.0, 1.0, 1.0]),
        ];
        let mut engine = Self {
            player: Player::default(),
            agents: Vec::with_capacity(MAX_AGENTS),
            obstacles,
            launch_pads: vec![
                LaunchPad::new(-10.0, -3.0, 2.7, DEFAULT_LAUNCH_COUNTDOWN),
                LaunchPad::new(0.0, -7.0, 2.7, DEFAULT_LAUNCH_COUNTDOWN),
                LaunchPad::new(10.0, -3.0, 2.7, DEFAULT_LAUNCH_COUNTDOWN),
            ],
            input: Input::default(),
            elapsed: 0.0,
            next_spawn_at: 3.0,
            view_yaw: 0.0,
            view_pitch: -0.095,
            target_yaw: 0.0,
            target_pitch: -0.095,
            camera_distance: 8.0,
            target_camera_distance: 8.0,
            random: Random::new(0xC0BA_CAFE),
            snapshot: vec![0.0; (MAX_AGENTS + 1) * SNAPSHOT_STRIDE],
            launch_event_id: 0,
            last_launch_pad: 0,
            last_launch_occupants: 0,
        };
        engine.write_snapshot();
        engine
    }

    pub(crate) fn set_input(&mut self, input: Input) {
        self.input = input;
    }

    pub(crate) fn set_launch_pad(
        &mut self,
        index: usize,
        x: f32,
        z: f32,
        radius: f32,
        countdown: f32,
    ) {
        if index >= self.launch_pads.len() {
            self.launch_pads.resize(index + 1, LaunchPad::default());
        }
        if let Some(pad) = self.launch_pads.get_mut(index) {
            *pad = LaunchPad::new(x, z, radius, countdown);
        }
    }

    pub(crate) fn set_launch_pad_count(&mut self, count: usize) {
        self.launch_pads.resize(count.min(64), LaunchPad::default());
    }

    pub fn reset_view(&mut self) {
        self.view_yaw = 0.0;
        self.view_pitch = -0.095;
        self.target_yaw = 0.0;
        self.target_pitch = -0.095;
        self.camera_distance = 0.0;
        self.target_camera_distance = 0.0;
    }

    pub fn step(&mut self, delta: f32) {
        let delta = delta.clamp(0.0, 0.05);
        self.elapsed += delta;
        self.apply_camera_input();
        self.smooth_camera(delta);
        self.update_player(delta);
        self.spawn_agents();
        self.update_agents(delta);
        self.update_launch_pads();
        self.write_snapshot();
    }

    pub fn snapshot(&self) -> &[f32] {
        &self.snapshot
    }

    pub(crate) fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub(crate) fn elapsed(&self) -> f32 {
        self.elapsed
    }

    pub fn camera(&self) -> [f32; 3] {
        [self.view_yaw, self.view_pitch, self.camera_distance]
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

    fn update_launch_pads(&mut self) {
        for index in 0..self.launch_pads.len() {
            let occupants = self.count_launch_pad_occupants(index);
            let pad = &mut self.launch_pads[index];
            pad.occupants = occupants;

            match pad.phase {
                LaunchPadPhase::Idle if occupants > 0 => {
                    pad.phase = LaunchPadPhase::Countdown;
                    pad.launch_at = self.elapsed + pad.countdown;
                }
                LaunchPadPhase::Countdown if occupants == 0 => {
                    pad.phase = LaunchPadPhase::Idle;
                    pad.launch_at = 0.0;
                }
                LaunchPadPhase::Countdown if self.elapsed >= pad.launch_at => {
                    pad.phase = LaunchPadPhase::Launched;
                    self.launch_event_id = self.launch_event_id.wrapping_add(1);
                    self.last_launch_pad = index;
                    self.last_launch_occupants = occupants;
                }
                LaunchPadPhase::Launched if occupants == 0 => {
                    pad.phase = LaunchPadPhase::Idle;
                    pad.launch_at = 0.0;
                }
                _ => {}
            }
        }
    }

    fn count_launch_pad_occupants(&self, index: usize) -> usize {
        let Some(pad) = self.launch_pads.get(index) else {
            return 0;
        };
        let agents = self
            .agents
            .iter()
            .filter(|agent| {
                agent.meeting_index == index
                    && agent.phase == AgentPhase::Assembled
                    && horizontal_distance(agent.position[0], agent.position[2], pad.x, pad.z)
                        <= pad.radius
            })
            .count();
        agents + usize::from(self.player_is_on_pad(pad))
    }

    fn player_is_on_pad(&self, pad: &LaunchPad) -> bool {
        self.player.grounded
            && horizontal_distance(
                self.player.position[0],
                self.player.position[2],
                pad.x,
                pad.z,
            ) <= pad.radius
    }

    fn apply_camera_input(&mut self) {
        self.target_yaw -= self.input.look_x * LOOK_SENSITIVITY;
        self.target_pitch =
            (self.target_pitch + self.input.look_y * LOOK_SENSITIVITY).clamp(-MAX_PITCH, MAX_PITCH);
        self.target_camera_distance =
            (self.target_camera_distance + self.input.zoom_delta).clamp(0.0, MAX_CAMERA_DISTANCE);
    }

    fn smooth_camera(&mut self, delta: f32) {
        self.view_yaw = damp(self.view_yaw, self.target_yaw, 10.0, delta);
        self.view_pitch = damp(self.view_pitch, self.target_pitch, 10.0, delta);
        self.camera_distance = damp(
            self.camera_distance,
            self.target_camera_distance,
            9.0,
            delta,
        );
    }

    fn write_snapshot(&mut self) {
        self.snapshot.fill(0.0);
        self.snapshot[0..SNAPSHOT_STRIDE].copy_from_slice(&[
            self.player.position[0],
            self.player.position[1],
            self.player.position[2],
            self.view_yaw,
            self.player.walk_cycle,
            bool_as_float(self.player.grounded),
            bool_as_float(self.player.moving),
            bool_as_float(self.player.sprinting),
        ]);
        for (index, agent) in self.agents.iter().enumerate() {
            let offset = (index + 1) * SNAPSHOT_STRIDE;
            let yaw =
                (agent.target.x - agent.position[0]).atan2(agent.target.z - agent.position[2]);
            self.snapshot[offset..offset + SNAPSHOT_STRIDE].copy_from_slice(&[
                agent.position[0],
                agent.position[1],
                agent.position[2],
                yaw,
                agent.walk_cycle,
                agent.phase.code(),
                agent.meeting_index as f32,
                bool_as_float(agent.phase == AgentPhase::Assembled),
            ]);
        }
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_the_spawn_pad() {
        let engine = Engine::new();
        assert_eq!(engine.player.position, [0.0, 0.0, 11.5]);
        assert_eq!(engine.snapshot.len(), 18 * SNAPSHOT_STRIDE);
    }

    #[test]
    fn movement_accelerates_in_view_direction() {
        let mut engine = Engine::new();
        engine.set_input(Input {
            forward: 1.0,
            ..Input::default()
        });
        engine.step(1.0 / 60.0);
        assert!(engine.player.position[2] < 11.5);
        assert!(engine.player.moving);
    }

    #[test]
    fn jump_returns_to_ground() {
        let mut engine = Engine::new();
        engine.set_input(Input {
            jump: true,
            ..Input::default()
        });
        engine.step(1.0 / 60.0);
        assert!(!engine.player.grounded);
        for _ in 0..120 {
            engine.set_input(Input::default());
            engine.step(1.0 / 60.0);
        }
        assert!(engine.player.grounded);
        assert_eq!(engine.player.position[1], 0.0);
    }

    #[test]
    fn agents_spawn_deterministically() {
        let mut engine = Engine::new();
        for _ in 0..181 {
            engine.step(1.0 / 60.0);
        }
        assert_eq!(engine.agents.len(), 1);
        assert_eq!(engine.snapshot[SNAPSHOT_STRIDE + 5], 0.0);
    }

    #[test]
    fn occupied_launch_pad_counts_down_and_emits_event() {
        let mut engine = Engine::new();
        engine.player.position = [-10.0, 0.0, -3.0];

        engine.step(1.0 / 60.0);
        assert_eq!(engine.launch_pad_phase(0), LaunchPadPhase::Countdown.code());
        assert_eq!(engine.launch_pad_occupants(0), 1);
        assert!(engine.launch_pad_seconds(0) > 7.9);

        for _ in 0..480 {
            engine.step(1.0 / 60.0);
        }

        assert_eq!(engine.launch_event_id, 1);
        assert_eq!(engine.last_launch_pad, 0);
        assert_eq!(engine.last_launch_occupants, 1);
        assert_eq!(engine.launch_pad_phase(0), LaunchPadPhase::Launched.code());
    }

    #[test]
    fn empty_launch_pad_cancels_countdown() {
        let mut engine = Engine::new();
        engine.player.position = [-10.0, 0.0, -3.0];
        engine.step(1.0 / 60.0);
        engine.player.position = [0.0, 0.0, 11.5];
        engine.step(1.0 / 60.0);

        assert_eq!(engine.launch_pad_phase(0), LaunchPadPhase::Idle.code());
        assert_eq!(engine.launch_event_id, 0);
    }

    #[test]
    fn launch_pad_registry_accepts_world_defined_counts() {
        let mut engine = Engine::new();
        engine.set_launch_pad_count(1);
        engine.set_launch_pad(0, 4.0, -2.0, 2.0, 4.0);

        assert_eq!(engine.launch_pad_count(), 1);
        engine.player.position = [4.0, 0.0, -2.0];
        engine.step(1.0 / 60.0);

        assert_eq!(engine.launch_pad_occupants(0), 1);
        assert_eq!(engine.launch_pad_phase(0), LaunchPadPhase::Countdown.code());
    }
}
