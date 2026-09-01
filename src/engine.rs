use crate::math::{Random, bool_as_float, damp, horizontal_distance};
use crate::types::{Agent, AgentPhase, Input, Player};
use crate::world::{Aabb, Gate, block_bounds};

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

const LOOK_SENSITIVITY: f32 = 0.0062;
const MAX_PITCH: f32 = 1.1;
const MAX_CAMERA_DISTANCE: f32 = 16.0;

pub struct Engine {
    pub(crate) player: Player,
    pub(crate) agents: Vec<Agent>,
    pub(crate) obstacles: Vec<Aabb>,
    pub(crate) gates: [Gate; 3],
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
            gates: [
                Gate { x: -10.0, z: -3.0 },
                Gate { x: 0.0, z: -7.0 },
                Gate { x: 10.0, z: -3.0 },
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
        };
        engine.write_snapshot();
        engine
    }

    pub(crate) fn set_input(&mut self, input: Input) {
        self.input = input;
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
        let Some(gate) = self.gates.get(index) else {
            return 0;
        };
        let agents = self
            .agents
            .iter()
            .filter(|agent| agent.meeting_index == index && agent.phase == AgentPhase::Assembled)
            .count();
        let player_is_here = horizontal_distance(
            self.player.position[0],
            self.player.position[2],
            gate.x,
            gate.z,
        ) <= 2.7;
        agents + usize::from(player_is_here)
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
}
