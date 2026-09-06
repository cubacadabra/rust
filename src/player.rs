use crate::engine::{
    ACCELERATION, AIR_ACCELERATION, BODY_HEIGHT, Engine, GRAVITY, JUMP_VELOCITY, PLAYER_RADIUS,
    RUN_SPEED, WALK_SPEED, WORLD_LIMIT,
};
use crate::math::{Vec2, damp};
use crate::world::overlaps_obstacle;

#[allow(dead_code)]
pub(crate) const WALK_CYCLE_DISTANCE: f32 = crate::character::gait::WALK_DISTANCE;

#[allow(dead_code)]
pub(crate) fn walk_cycle_delta(distance: f32) -> f32 {
    distance.max(0.0) / WALK_CYCLE_DISTANCE * std::f32::consts::TAU
}

impl Engine {
    pub(crate) fn update_player(&mut self, delta: f32) {
        let was_grounded = self.player.grounded;
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
        // Orbit is independent of the body. Movement turns the character;
        // releasing the stick retains its last heading, even after coasting.
        if self.camera_distance <= 0.75 {
            self.player.facing_yaw = self.view_yaw;
        } else if moving {
            let heading = (-direction.x).atan2(-direction.z);
            let turn = (heading - self.player.facing_yaw).sin()
                .atan2((heading - self.player.facing_yaw).cos());
            self.player.facing_yaw += turn * (1.0 - (-16.0 * delta).exp());
        }
        let speed = if sprinting { RUN_SPEED } else { WALK_SPEED };
        let input_amount = if moving { input_length.min(1.0) } else { 0.0 };
        let target_x = direction.x * speed * input_amount;
        let target_z = direction.z * speed * input_amount;
        let acceleration = if self.player.grounded {
            ACCELERATION
        } else {
            AIR_ACCELERATION
        };
        self.player.velocity[0] = damp(self.player.velocity[0], target_x, acceleration, delta);
        self.player.velocity[2] = damp(self.player.velocity[2], target_z, acceleration, delta);

        let takeoff = self.input.jump && self.player.grounded;
        if takeoff {
            self.player.velocity[1] = JUMP_VELOCITY;
            self.player.grounded = false;
        }
        self.input.jump = false;
        self.input.look_x = 0.0;
        self.input.look_y = 0.0;
        self.input.zoom_delta = 0.0;

        let previous_position = self.player.position;
        self.move_player_horizontally(delta);
        // The stride phase follows distance actually travelled. If an input
        // is held against a wall, collision zeros the velocity and the feet
        // stop cycling instead of running on a treadmill.
        let travelled = (self.player.position[0] - previous_position[0])
            .hypot(self.player.position[2] - previous_position[2]);
        if travelled > 0.0 && self.player.grounded {
            let actual_speed = travelled / delta.max(0.0001);
            let run = crate::character::gait::run_amount(actual_speed);
            self.player.walk_cycle += travelled / crate::character::gait::cycle_distance(run)
                * std::f32::consts::TAU;
        }
        self.player.velocity[1] -= GRAVITY * delta;
        self.move_player_vertically(delta);
        self.player_motion_event = if !was_grounded && self.player.grounded {
            crate::types::CharacterMotionEvent::Landing
        } else if takeoff {
            crate::types::CharacterMotionEvent::Takeoff
        } else {
            crate::types::CharacterMotionEvent::None
        };
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
                if !overlaps_obstacle(self.player.position, obstacle, PLAYER_RADIUS) {
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
                if !overlaps_obstacle(self.player.position, obstacle, PLAYER_RADIUS) {
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
            !overlaps_obstacle(candidate, obstacle, PLAYER_RADIUS)
        })
    }
}
