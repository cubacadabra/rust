#![allow(clippy::too_many_arguments)]

use std::f32::consts::PI;

const TOTAL_PLAYERS: usize = 18;
const MAX_AGENTS: usize = TOTAL_PLAYERS - 1;
const SNAPSHOT_STRIDE: usize = 8;

const BODY_HEIGHT: f32 = 3.15;
const PLAYER_RADIUS: f32 = 0.52;
const WALK_SPEED: f32 = 6.4;
const RUN_SPEED: f32 = 11.5;
const ACCELERATION: f32 = 22.0;
const AIR_ACCELERATION: f32 = 9.0;
const GRAVITY: f32 = 28.0;
const JUMP_VELOCITY: f32 = 10.5;
const WORLD_LIMIT: f32 = 57.5;

const LOOK_SENSITIVITY: f32 = 0.0062;
const MAX_PITCH: f32 = 1.1;
const MAX_CAMERA_DISTANCE: f32 = 16.0;

#[derive(Clone, Copy, Debug, Default)]
struct Vec2 {
    x: f32,
    z: f32,
}

impl Vec2 {
    fn length_squared(self) -> f32 {
        self.x * self.x + self.z * self.z
    }

    fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    fn normalized(self) -> Self {
        let length = self.length();
        if length <= f32::EPSILON {
            Self::default()
        } else {
            Self {
                x: self.x / length,
                z: self.z / length,
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Aabb {
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    bottom: f32,
    top: f32,
}

#[derive(Clone, Copy, Debug)]
struct Gate {
    x: f32,
    z: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Input {
    forward: f32,
    strafe: f32,
    sprint: bool,
    jump: bool,
    look_x: f32,
    look_y: f32,
    zoom_delta: f32,
}

#[derive(Clone, Copy, Debug)]
struct Player {
    position: [f32; 3],
    velocity: [f32; 3],
    grounded: bool,
    moving: bool,
    sprinting: bool,
    walk_cycle: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 11.5],
            velocity: [0.0; 3],
            grounded: true,
            moving: false,
            sprinting: false,
            walk_cycle: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AgentPhase {
    Entering,
    Roaming,
    Assembling,
    Assembled,
}

impl AgentPhase {
    fn code(self) -> f32 {
        match self {
            Self::Entering => 0.0,
            Self::Roaming => 1.0,
            Self::Assembling => 2.0,
            Self::Assembled => 3.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Agent {
    position: [f32; 3],
    target: Vec2,
    meeting_target: Vec2,
    meeting_index: usize,
    phase: AgentPhase,
    spawned_at: f32,
    next_decision_at: f32,
    gather_at: f32,
    next_jump_at: f32,
    speed: f32,
    walk_cycle: f32,
    vertical_velocity: f32,
    grounded: bool,
}

#[derive(Clone, Copy, Debug)]
struct Random(u32);

impl Random {
    fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> u32 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 17;
        value ^= value << 5;
        self.0 = value;
        value
    }

    fn unit(&mut self) -> f32 {
        self.next() as f32 / u32::MAX as f32
    }

    fn between(&mut self, min: f32, max: f32) -> f32 {
        min + self.unit() * (max - min)
    }
}

pub struct Engine {
    player: Player,
    agents: Vec<Agent>,
    obstacles: Vec<Aabb>,
    gates: [Gate; 3],
    input: Input,
    elapsed: f32,
    next_spawn_at: f32,
    view_yaw: f32,
    view_pitch: f32,
    target_yaw: f32,
    target_pitch: f32,
    camera_distance: f32,
    target_camera_distance: f32,
    random: Random,
    snapshot: Vec<f32>,
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

    fn set_input(&mut self, input: Input) {
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

    fn update_player(&mut self, delta: f32) {
        let mut forward = self.input.forward.clamp(-1.0, 1.0);
        let mut strafe = self.input.strafe.clamp(-1.0, 1.0);
        let input_length = (forward * forward + strafe * strafe).sqrt();
        if input_length > 1.0 {
            forward /= input_length;
            strafe /= input_length;
        }

        let moving = input_length > 0.01;
        let sprinting = moving && self.input.sprint;
        self.player.moving = moving;
        self.player.sprinting = sprinting;

        let forward_vector = Vec2 {
            x: -self.view_yaw.sin(),
            z: -self.view_yaw.cos(),
        };
        let right_vector = Vec2 {
            x: self.view_yaw.cos(),
            z: -self.view_yaw.sin(),
        };
        let direction = Vec2 {
            x: forward_vector.x * forward + right_vector.x * strafe,
            z: forward_vector.z * forward + right_vector.z * strafe,
        }
        .normalized();
        let speed = if sprinting { RUN_SPEED } else { WALK_SPEED };
        let target_x = direction.x * speed;
        let target_z = direction.z * speed;
        let acceleration = if self.player.grounded {
            ACCELERATION
        } else {
            AIR_ACCELERATION
        };
        self.player.velocity[0] = damp(self.player.velocity[0], target_x, acceleration, delta);
        self.player.velocity[2] = damp(self.player.velocity[2], target_z, acceleration, delta);

        if self.input.jump && self.player.grounded {
            self.player.velocity[1] = JUMP_VELOCITY;
            self.player.grounded = false;
        }
        self.input.jump = false;
        self.input.look_x = 0.0;
        self.input.look_y = 0.0;
        self.input.zoom_delta = 0.0;

        if moving {
            self.player.walk_cycle += delta * if sprinting { 14.0 } else { 10.0 };
        }
        self.move_player_horizontally(delta);
        self.player.velocity[1] -= GRAVITY * delta;
        self.move_player_vertically(delta);
    }

    fn move_player_horizontally(&mut self, delta: f32) {
        let limit = WORLD_LIMIT - PLAYER_RADIUS;
        let mut candidate = self.player.position;
        candidate[0] = (candidate[0] + self.player.velocity[0] * delta).clamp(-limit, limit);
        if self.player_can_occupy(candidate) {
            self.player.position[0] = candidate[0];
        } else {
            self.player.velocity[0] = 0.0;
        }

        candidate = self.player.position;
        candidate[2] = (candidate[2] + self.player.velocity[2] * delta).clamp(-limit, limit);
        if self.player_can_occupy(candidate) {
            self.player.position[2] = candidate[2];
        } else {
            self.player.velocity[2] = 0.0;
        }
    }

    fn move_player_vertically(&mut self, delta: f32) {
        let previous_feet = self.player.position[1];
        let next_feet = previous_feet + self.player.velocity[1] * delta;
        let epsilon = 0.05;

        if self.player.velocity[1] <= 0.0 {
            let mut landing_height = None;
            for obstacle in &self.obstacles {
                if !overlaps_obstacle(self.player.position, obstacle) {
                    continue;
                }
                let crossed_top =
                    previous_feet >= obstacle.top - epsilon && next_feet <= obstacle.top + epsilon;
                if crossed_top && landing_height.is_none_or(|height| obstacle.top > height) {
                    landing_height = Some(obstacle.top);
                }
            }
            if let Some(height) = landing_height {
                self.player.position[1] = height;
                self.player.velocity[1] = 0.0;
                self.player.grounded = true;
                return;
            }
            if next_feet <= 0.0 {
                self.player.position[1] = 0.0;
                self.player.velocity[1] = 0.0;
                self.player.grounded = true;
                return;
            }
        } else {
            let previous_head = previous_feet + BODY_HEIGHT;
            let next_head = next_feet + BODY_HEIGHT;
            let mut ceiling_height = None;
            for obstacle in &self.obstacles {
                if !overlaps_obstacle(self.player.position, obstacle) {
                    continue;
                }
                let hit_bottom = previous_head <= obstacle.bottom + epsilon
                    && next_head >= obstacle.bottom - epsilon;
                if hit_bottom && ceiling_height.is_none_or(|height| obstacle.bottom < height) {
                    ceiling_height = Some(obstacle.bottom);
                }
            }
            if let Some(height) = ceiling_height {
                self.player.position[1] = height - BODY_HEIGHT;
                self.player.velocity[1] = 0.0;
                self.player.grounded = false;
                return;
            }
        }

        self.player.position[1] = next_feet;
        self.player.grounded = false;
    }

    fn player_can_occupy(&self, candidate: [f32; 3]) -> bool {
        let feet = self.player.position[1];
        let head = feet + BODY_HEIGHT;
        self.obstacles.iter().all(|obstacle| {
            if feet >= obstacle.top - 0.05 || head <= obstacle.bottom + 0.05 {
                return true;
            }
            !overlaps_obstacle(candidate, obstacle)
        })
    }

    fn spawn_agents(&mut self) {
        while self.agents.len() < MAX_AGENTS && self.elapsed >= self.next_spawn_at {
            let index = self.agents.len();
            let entry = entry_point(index);
            let meeting_index = entry.2;
            let slot_index = index / 3;
            let gate = self.gates[meeting_index];
            let offset = slot_offset(slot_index);
            let position = [
                entry.0 + self.random.between(-0.7, 0.7),
                0.0,
                entry.1 + self.random.between(-0.4, 0.4),
            ];
            self.agents.push(Agent {
                position,
                target: Vec2 {
                    x: entry.3,
                    z: entry.4,
                },
                meeting_target: Vec2 {
                    x: gate.x + offset.0,
                    z: gate.z + offset.1,
                },
                meeting_index,
                phase: AgentPhase::Entering,
                spawned_at: self.elapsed,
                next_decision_at: self.elapsed + self.random.between(1.3, 2.8),
                gather_at: self.elapsed + self.random.between(7.5, 10.5),
                next_jump_at: self.elapsed + self.random.between(1.4, 3.5),
                speed: self.random.between(0.82, 1.08),
                walk_cycle: self.random.between(0.0, PI * 2.0),
                vertical_velocity: 0.0,
                grounded: true,
            });
            self.next_spawn_at += 3.0;
        }
    }

    fn update_agents(&mut self, delta: f32) {
        for index in 0..self.agents.len() {
            self.update_agent(index, delta);
        }
    }

    fn update_agent(&mut self, index: usize, delta: f32) {
        let mut agent = self.agents[index];
        if agent.phase == AgentPhase::Entering
            && horizontal_distance(
                agent.position[0],
                agent.position[2],
                agent.target.x,
                agent.target.z,
            ) < 1.2
        {
            agent.phase = AgentPhase::Roaming;
            agent.target = self.roam_target(agent.position);
            agent.next_decision_at = self.elapsed + self.random.between(1.2, 2.7);
        }

        if agent.phase == AgentPhase::Roaming {
            if self.elapsed >= agent.gather_at {
                agent.phase = AgentPhase::Assembling;
                agent.target = agent.meeting_target;
            } else if self.elapsed >= agent.next_decision_at
                || horizontal_distance(
                    agent.position[0],
                    agent.position[2],
                    agent.target.x,
                    agent.target.z,
                ) < 1.0
            {
                agent.target = self.roam_target(agent.position);
                agent.next_decision_at = self.elapsed + self.random.between(1.1, 2.4);
            }
        }

        if agent.phase == AgentPhase::Assembling
            && horizontal_distance(
                agent.position[0],
                agent.position[2],
                agent.meeting_target.x,
                agent.meeting_target.z,
            ) < 0.65
        {
            agent.phase = AgentPhase::Assembled;
            agent.target = agent.meeting_target;
        }

        if agent.phase != AgentPhase::Assembled {
            let mut direction = Vec2 {
                x: agent.target.x - agent.position[0],
                z: agent.target.z - agent.position[2],
            }
            .normalized();
            self.add_separation(index, &mut direction);
            self.avoid_obstacles(agent.position, &mut direction);
            let is_running = if agent.phase == AgentPhase::Entering {
                (self.elapsed * 1.35 + agent.spawned_at).sin() > -0.25
            } else {
                (self.elapsed * 1.1 + agent.spawned_at * 2.0).sin() > 0.35
            };
            let speed = (if is_running {
                RUN_SPEED
            } else {
                WALK_SPEED * 0.62
            }) * agent.speed;
            agent.position[0] =
                (agent.position[0] + direction.x * speed * delta).clamp(-WORLD_LIMIT, WORLD_LIMIT);
            agent.position[2] =
                (agent.position[2] + direction.z * speed * delta).clamp(-WORLD_LIMIT, WORLD_LIMIT);
            agent.walk_cycle += delta * if is_running { 13.0 } else { 9.0 };
        } else {
            agent.walk_cycle += delta * 2.2;
        }

        if agent.grounded
            && agent.phase != AgentPhase::Assembled
            && self.elapsed >= agent.next_jump_at
        {
            agent.vertical_velocity = JUMP_VELOCITY * self.random.between(0.78, 0.95);
            agent.grounded = false;
            agent.next_jump_at = self.elapsed + self.random.between(3.8, 7.2);
        }
        if !agent.grounded {
            agent.vertical_velocity -= GRAVITY * delta;
            agent.position[1] += agent.vertical_velocity * delta;
            if agent.position[1] <= 0.0 {
                agent.position[1] = 0.0;
                agent.vertical_velocity = 0.0;
                agent.grounded = true;
            }
        }

        self.agents[index] = agent;
    }

    fn roam_target(&mut self, position: [f32; 3]) -> Vec2 {
        let angle = self.random.between(-PI, PI);
        let distance = self.random.between(2.5, 6.5);
        Vec2 {
            x: (position[0] + angle.cos() * distance).clamp(-22.0, 22.0),
            z: (position[2] + angle.sin() * distance).clamp(-1.0, 14.0),
        }
    }

    fn add_separation(&self, index: usize, direction: &mut Vec2) {
        let agent = self.agents[index];
        for (other_index, other) in self.agents.iter().enumerate() {
            if index == other_index {
                continue;
            }
            let offset = Vec2 {
                x: agent.position[0] - other.position[0],
                z: agent.position[2] - other.position[2],
            };
            let distance = offset.length();
            if !(0.001..1.55).contains(&distance) {
                continue;
            }
            let strength = (1.55 - distance) / 1.55;
            direction.x += offset.x / distance * strength * 1.8;
            direction.z += offset.z / distance * strength * 1.8;
        }
        *direction = direction.normalized();
    }

    fn avoid_obstacles(&self, position: [f32; 3], direction: &mut Vec2) {
        for obstacle in &self.obstacles {
            if obstacle.top > 0.8 {
                continue;
            }
            let closest_x = position[0].clamp(obstacle.min_x, obstacle.max_x);
            let closest_z = position[2].clamp(obstacle.min_z, obstacle.max_z);
            let offset = Vec2 {
                x: position[0] - closest_x,
                z: position[2] - closest_z,
            };
            let distance = offset.length();
            if !(0.001..2.1).contains(&distance) {
                continue;
            }
            let strength = (2.1 - distance) / 2.1;
            direction.x += offset.x / distance * strength * 2.5;
            direction.z += offset.z / distance * strength * 2.5;
        }
        *direction = direction.normalized();
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

fn block_bounds(position: [f32; 3], size: [f32; 3]) -> Aabb {
    Aabb {
        min_x: position[0] - size[0] / 2.0,
        max_x: position[0] + size[0] / 2.0,
        min_z: position[2] - size[2] / 2.0,
        max_z: position[2] + size[2] / 2.0,
        bottom: position[1] - size[1] / 2.0,
        top: position[1] + size[1] / 2.0,
    }
}

fn horizontal_distance(x: f32, z: f32, other_x: f32, other_z: f32) -> f32 {
    ((x - other_x).powi(2) + (z - other_z).powi(2)).sqrt()
}

fn overlaps_obstacle(position: [f32; 3], obstacle: &Aabb) -> bool {
    let closest_x = position[0].clamp(obstacle.min_x, obstacle.max_x);
    let closest_z = position[2].clamp(obstacle.min_z, obstacle.max_z);
    let distance_x = position[0] - closest_x;
    let distance_z = position[2] - closest_z;
    distance_x * distance_x + distance_z * distance_z < PLAYER_RADIUS * PLAYER_RADIUS
}

fn damp(current: f32, target: f32, smoothing: f32, delta: f32) -> f32 {
    current + (target - current) * (1.0 - (-smoothing * delta).exp())
}

fn bool_as_float(value: bool) -> f32 {
    if value { 1.0 } else { 0.0 }
}

fn entry_point(index: usize) -> (f32, f32, usize, f32, f32) {
    match index % 3 {
        0 => (-17.0, 12.0, 0, -14.0, 4.0),
        1 => (0.0, 16.0, 1, 0.0, 3.0),
        _ => (17.0, 12.0, 2, 14.0, 4.0),
    }
}

fn slot_offset(index: usize) -> (f32, f32) {
    match index % 7 {
        0 => (-1.75, 1.35),
        1 => (0.0, 1.65),
        2 => (1.75, 1.35),
        3 => (-2.05, -0.45),
        4 => (2.05, -0.45),
        5 => (-0.8, -1.7),
        _ => (0.8, -1.7),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn engine_create() -> *mut Engine {
    Box::into_raw(Box::new(Engine::new()))
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_input(
    engine: *mut Engine,
    forward: f32,
    strafe: f32,
    sprint: u8,
    jump: u8,
    look_x: f32,
    look_y: f32,
    zoom_delta: f32,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_input(Input {
            forward,
            strafe,
            sprint: sprint != 0,
            jump: jump != 0,
            look_x,
            look_y,
            zoom_delta,
        });
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_step(engine: *mut Engine, delta: f32) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.step(delta);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_reset_view(engine: *mut Engine) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.reset_view();
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_snapshot_ptr(engine: *const Engine) -> *const f32 {
    unsafe { engine.as_ref() }
        .map(|engine| engine.snapshot().as_ptr())
        .unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub extern "C" fn engine_snapshot_len() -> usize {
    (MAX_AGENTS + 1) * SNAPSHOT_STRIDE
}

#[unsafe(no_mangle)]
pub extern "C" fn engine_snapshot_stride() -> usize {
    SNAPSHOT_STRIDE
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_camera_yaw(engine: *const Engine) -> f32 {
    unsafe { engine.as_ref() }.map_or(0.0, |engine| engine.camera()[0])
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_camera_pitch(engine: *const Engine) -> f32 {
    unsafe { engine.as_ref() }.map_or(0.0, |engine| engine.camera()[1])
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_camera_distance(engine: *const Engine) -> f32 {
    unsafe { engine.as_ref() }.map_or(0.0, |engine| engine.camera()[2])
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_agent_count(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.agents.len())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_meeting_count(engine: *const Engine, index: usize) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.meeting_count(index))
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_elapsed(engine: *const Engine) -> f32 {
    unsafe { engine.as_ref() }.map_or(0.0, |engine| engine.elapsed)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`, and it
/// must not be used again after this call.
pub unsafe extern "C" fn engine_destroy(engine: *mut Engine) {
    if !engine.is_null() {
        unsafe { drop(Box::from_raw(engine)) };
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
