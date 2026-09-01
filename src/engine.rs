use crate::game_package::GamePackageDefinition;
use crate::math::{Random, bool_as_float, damp, horizontal_distance};
use crate::scripting::GameScript;
use crate::types::{Agent, AgentPhase, Input, LaunchPadPhase, Player};
use crate::world::{Aabb, LaunchPad, RuntimeWorld, block_bounds, slot_offset};

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
    pub(crate) worlds: Vec<RuntimeWorld>,
    pub(crate) active_world: usize,
    pub(crate) world_event_id: u32,
    pub(crate) last_world_source_pad: usize,
    pub(crate) last_world_destination: usize,
    pub(crate) script: Option<GameScript>,
    pub(crate) script_buffer: Vec<u8>,
    pub(crate) package: Option<GamePackageDefinition>,
    pub(crate) package_generation: u32,
    pub(crate) package_buffer: Vec<u8>,
    pub(crate) world_ids: Vec<String>,
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
            worlds: Vec::new(),
            active_world: 0,
            world_event_id: 0,
            last_world_source_pad: 0,
            last_world_destination: 0,
            script: None,
            script_buffer: Vec::new(),
            package: None,
            package_generation: 0,
            package_buffer: Vec::new(),
            world_ids: Vec::new(),
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

    pub(crate) fn set_obstacle(&mut self, index: usize, position: [f32; 3], size: [f32; 3]) {
        if index >= self.obstacles.len() {
            self.obstacles
                .resize(index + 1, block_bounds([0.0; 3], [0.0; 3]));
        }
        if let Some(obstacle) = self.obstacles.get_mut(index) {
            *obstacle = block_bounds(position, size);
        }
    }

    pub(crate) fn set_obstacle_count(&mut self, count: usize) {
        self.obstacles
            .resize(count.min(256), block_bounds([0.0; 3], [0.0; 3]));
    }

    pub(crate) fn set_world_count(&mut self, count: usize) {
        self.worlds.resize(count.min(64), RuntimeWorld::default());
    }

    pub(crate) fn set_world_spawn(&mut self, world: usize, spawn: [f32; 3]) {
        if let Some(world) = self.worlds.get_mut(world) {
            world.spawn = spawn;
        }
    }

    pub(crate) fn set_world_launch_pad_count(&mut self, world: usize, count: usize) {
        if let Some(world) = self.worlds.get_mut(world) {
            let count = count.min(64);
            world.launch_pads.resize(count, LaunchPad::default());
            world.launch_destinations.resize(count, None);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn set_world_launch_pad(
        &mut self,
        world: usize,
        index: usize,
        x: f32,
        z: f32,
        radius: f32,
        countdown: f32,
    ) {
        let Some(world) = self.worlds.get_mut(world) else {
            return;
        };
        if index >= world.launch_pads.len() {
            world.launch_pads.resize(index + 1, LaunchPad::default());
            world.launch_destinations.resize(index + 1, None);
        }
        world.launch_pads[index] = LaunchPad::new(x, z, radius, countdown);
    }

    pub(crate) fn set_world_launch_destination(
        &mut self,
        world: usize,
        pad: usize,
        destination: i32,
    ) {
        let world_count = self.worlds.len();
        let Some(world) = self.worlds.get_mut(world) else {
            return;
        };
        if pad >= world.launch_destinations.len() {
            world.launch_destinations.resize(pad + 1, None);
        }
        world.launch_destinations[pad] = usize::try_from(destination)
            .ok()
            .filter(|index| *index < world_count);
    }

    pub(crate) fn set_world_obstacle_count(&mut self, world: usize, count: usize) {
        if let Some(world) = self.worlds.get_mut(world) {
            world
                .obstacles
                .resize(count.min(256), block_bounds([0.0; 3], [0.0; 3]));
        }
    }

    pub(crate) fn set_world_obstacle(
        &mut self,
        world: usize,
        index: usize,
        position: [f32; 3],
        size: [f32; 3],
    ) {
        let Some(world) = self.worlds.get_mut(world) else {
            return;
        };
        if index >= world.obstacles.len() {
            world
                .obstacles
                .resize(index + 1, block_bounds([0.0; 3], [0.0; 3]));
        }
        world.obstacles[index] = block_bounds(position, size);
    }

    pub(crate) fn start_world(&mut self, index: usize) -> bool {
        let Some(world) = self.worlds.get(index).cloned() else {
            return false;
        };
        self.active_world = index;
        self.launch_pads = world.launch_pads;
        self.obstacles = world.obstacles;
        self.player.position = world.spawn;
        self.player.velocity = [0.0; 3];
        self.player.grounded = true;
        self.agents.clear();
        self.next_spawn_at = self.elapsed + 3.0;
        self.write_snapshot();
        true
    }

    pub(crate) fn enter_session(&mut self, launch_pad_index: usize, spawn: [f32; 3]) -> usize {
        let Some(launch_pad) = self.launch_pads.get(launch_pad_index).copied() else {
            return 0;
        };

        let mut selected_agents = self
            .agents
            .iter()
            .copied()
            .filter(|agent| {
                agent.meeting_index == launch_pad_index
                    && agent.phase == AgentPhase::Assembled
                    && horizontal_distance(
                        agent.position[0],
                        agent.position[2],
                        launch_pad.x,
                        launch_pad.z,
                    ) <= launch_pad.radius
            })
            .collect::<Vec<_>>();

        for (index, agent) in selected_agents.iter_mut().enumerate() {
            let offset = slot_offset(index);
            agent.position = [spawn[0] + offset.0, spawn[1], spawn[2] + offset.1];
            agent.meeting_index = 0;
            agent.meeting_target.x = spawn[0];
            agent.meeting_target.z = spawn[2];
            agent.target = agent.meeting_target;
        }
        self.agents = selected_agents;
        self.launch_pads.clear();
        self.next_spawn_at = f32::MAX;
        self.player.position = spawn;
        self.player.velocity = [0.0; 3];
        self.player.grounded = true;
        self.player.moving = false;
        self.player.sprinting = false;
        self.write_snapshot();
        self.agents.len() + 1
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
        self.tick_script(delta);
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

    pub(crate) fn active_world(&self) -> usize {
        self.active_world
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

    pub(crate) fn load_package_buffer(&mut self) -> bool {
        let source = String::from_utf8_lossy(&self.package_buffer);
        let Ok(package) = GamePackageDefinition::parse(&source) else {
            return false;
        };
        let entries = package.world_entries();
        let world_indices = entries
            .iter()
            .enumerate()
            .map(|(index, (id, _))| (id.as_str(), index))
            .collect::<std::collections::BTreeMap<_, _>>();
        let worlds = entries
            .iter()
            .map(|(id, definition)| {
                let launch_pads = definition
                    .launch_pads
                    .iter()
                    .map(|pad| LaunchPad::new(pad.x(), pad.z(), pad.radius, pad.countdown))
                    .collect::<Vec<_>>();
                let launch_destinations = definition
                    .launch_pads
                    .iter()
                    .map(|pad| {
                        pad.destination_world
                            .as_deref()
                            .or_else(|| {
                                (id == "lobby")
                                    .then_some(package.launch.destination_world.as_deref())
                                    .flatten()
                            })
                            .and_then(|destination| world_indices.get(destination).copied())
                    })
                    .collect::<Vec<_>>();
                let obstacles = definition
                    .blocks
                    .iter()
                    .map(|block| block_bounds(block.position(), block.size()))
                    .collect::<Vec<_>>();
                RuntimeWorld {
                    spawn: definition.world.spawn(),
                    launch_pads,
                    launch_destinations,
                    obstacles,
                }
            })
            .collect::<Vec<_>>();
        let Some(start_world) = world_indices.get(package.start_world.as_str()).copied() else {
            return false;
        };

        self.worlds = worlds;
        self.world_ids = entries.into_iter().map(|(id, _)| id).collect();
        self.package = Some(package);
        self.package_generation = self.package_generation.wrapping_add(1).max(1);
        self.start_world(start_world)
    }

    pub(crate) fn load_script_buffer(&mut self) -> bool {
        let source = String::from_utf8_lossy(&self.script_buffer).into_owned();
        match GameScript::load(&source) {
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

    fn tick_script(&mut self, delta: f32) {
        if let Some(script) = &self.script
            && let Err(error) = script.tick(delta)
        {
            script.state().borrow_mut().last_error = Some(error);
        }
    }

    fn update_launch_pads(&mut self) {
        let mut launched = None;
        for index in 0..self.launch_pads.len() {
            let occupants = self.count_launch_pad_occupants(index);
            let local_player_selected = self
                .launch_pads
                .get(index)
                .is_some_and(|pad| self.player_is_on_pad(pad));
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
                    launched = Some((index, occupants, local_player_selected));
                }
                LaunchPadPhase::Launched if occupants == 0 => {
                    pad.phase = LaunchPadPhase::Idle;
                    pad.launch_at = 0.0;
                }
                _ => {}
            }
        }

        if let Some((index, occupants, local_player_selected)) = launched {
            let pad_id = format!("pad-{index}");
            let player_ids = (0..occupants as u32).collect::<Vec<_>>();
            if let Some(script) = &self.script
                && let Err(error) = script.launch(&pad_id, &player_ids)
            {
                script.state().borrow_mut().last_error = Some(error);
            }

            let destination = self
                .worlds
                .get(self.active_world)
                .and_then(|world| world.launch_destinations.get(index))
                .copied()
                .flatten();
            if local_player_selected && let Some(destination) = destination {
                self.enter_registered_world(index, destination);
            }
        }
    }

    fn enter_registered_world(&mut self, source_pad: usize, destination: usize) {
        let Some(world) = self.worlds.get(destination).cloned() else {
            return;
        };
        self.enter_session(source_pad, world.spawn);
        self.launch_pads = world.launch_pads;
        self.obstacles = world.obstacles;
        self.active_world = destination;
        self.world_event_id = self.world_event_id.wrapping_add(1);
        self.last_world_source_pad = source_pad;
        self.last_world_destination = destination;
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

    #[test]
    fn entering_session_keeps_only_players_from_launched_pad() {
        let mut engine = Engine::new();
        engine.agents.push(Agent {
            position: [-10.0, 0.0, -3.0],
            target: crate::math::Vec2 { x: -10.0, z: -3.0 },
            meeting_target: crate::math::Vec2 { x: -10.0, z: -3.0 },
            meeting_index: 0,
            phase: AgentPhase::Assembled,
            spawned_at: 0.0,
            next_decision_at: 0.0,
            gather_at: 0.0,
            next_jump_at: 0.0,
            speed: 1.0,
            walk_cycle: 0.0,
            vertical_velocity: 0.0,
            grounded: true,
        });
        engine.agents.push(Agent {
            meeting_index: 1,
            ..engine.agents[0]
        });
        engine.player.position = [-10.0, 0.0, -3.0];

        let player_count = engine.enter_session(0, [0.0, 0.0, 8.0]);

        assert_eq!(player_count, 2);
        assert_eq!(engine.agents.len(), 1);
        assert_eq!(engine.launch_pad_count(), 0);
        assert_eq!(engine.player.position, [0.0, 0.0, 8.0]);
    }

    #[test]
    fn registered_world_route_transitions_selected_player_in_engine() {
        let mut engine = Engine::new();
        engine.set_world_count(2);
        engine.set_world_spawn(0, [0.0, 0.0, 6.0]);
        engine.set_world_launch_pad_count(0, 1);
        engine.set_world_launch_pad(0, 0, 4.0, -2.0, 2.0, 0.1);
        engine.set_world_launch_destination(0, 0, 1);
        engine.set_world_spawn(1, [0.0, 0.0, 8.0]);
        engine.set_world_obstacle_count(1, 1);
        engine.set_world_obstacle(1, 0, [0.0, 1.0, -7.0], [4.0, 2.0, 4.0]);
        assert!(engine.start_world(0));

        engine.player.position = [4.0, 0.0, -2.0];
        for _ in 0..8 {
            engine.step(1.0 / 60.0);
        }

        assert_eq!(engine.active_world(), 1);
        assert_eq!(engine.world_event_id(), 1);
        assert_eq!(engine.last_world_source_pad(), 0);
        assert_eq!(engine.last_world_destination(), 1);
        assert_eq!(engine.player.position, [0.0, 0.0, 8.0]);
        assert_eq!(engine.launch_pad_count(), 0);
        assert_eq!(engine.obstacles.len(), 1);
    }
}
