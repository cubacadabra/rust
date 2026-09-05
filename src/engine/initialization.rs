use crate::engine::{DEFAULT_LAUNCH_COUNTDOWN, Engine, MAX_AGENTS, SNAPSHOT_STRIDE};
use crate::math::Random;
use crate::types::{Input, Player};
use crate::ui::UiRuntime;
use crate::world::{LaunchPad, block_bounds};
use std::cell::RefCell;
use std::rc::Rc;

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
            remote_players: Vec::with_capacity(MAX_AGENTS),
            obstacles: obstacles.clone(),
            base_obstacles: obstacles.clone(),
            build_blocks: Vec::new(),
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
            motion_sequence: 0,
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
            script_error_buffer: Vec::new(),
            package: None,
            package_generation: 0,
            package_buffer: Vec::new(),
            authoritative_launch: false,
            world_ids: Vec::new(),
            username: "PLAYER".to_owned(),
            username_buffer: Vec::new(),
            portal_cooldown_until: 0.0,
            ui: Rc::new(RefCell::new(UiRuntime::default())),
            ui_document_buffer: Vec::new(),
        };
        engine.write_snapshot();
        engine
    }
}
