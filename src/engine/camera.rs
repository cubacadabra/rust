use crate::engine::{Engine, LOOK_SENSITIVITY, MAX_CAMERA_DISTANCE, MAX_PITCH};
use crate::math::damp;

impl Engine {
    pub fn reset_view(&mut self) {
        self.view_yaw = 0.0;
        self.view_pitch = -0.095;
        self.target_yaw = 0.0;
        self.target_pitch = -0.095;
        self.camera_distance = 0.0;
        self.target_camera_distance = 0.0;
    }

    pub fn reset_showcase_view(&mut self) {
        self.view_yaw = 0.0;
        self.view_pitch = crate::engine::DEFAULT_ORBIT_PITCH;
        self.target_yaw = 0.0;
        self.target_pitch = crate::engine::DEFAULT_ORBIT_PITCH;
        self.camera_distance = crate::engine::DEFAULT_ORBIT_DISTANCE;
        self.target_camera_distance = crate::engine::DEFAULT_ORBIT_DISTANCE;
    }

    /// Queue a server correction for the locally predicted player. The server
    /// validates travel distance rather than simulating rigid-body collisions,
    /// so applying its position as an immediate snap can fight local obstacle
    /// collision and look like a random teleport. The player loop blends this
    /// delta over subsequent ticks instead.
    pub fn reconcile_player(&mut self, position: [f32; 3], yaw: f32) {
        for (pending, (&target, &current)) in self
            .pending_reconciliation
            .iter_mut()
            .zip(position.iter().zip(self.player.position.iter()))
        {
            if target.is_finite() && current.is_finite() {
                // Corrections are absolute server positions. Replace the
                // outstanding delta instead of accumulating stale packets.
                *pending = target - current;
            }
        }
        if yaw.is_finite() {
            self.player.facing_yaw = yaw;
        }
    }

    pub fn camera(&self) -> [f32; 3] {
        [self.view_yaw, self.view_pitch, self.camera_distance]
    }

    pub fn player_facing_yaw(&self) -> f32 {
        self.player.facing_yaw
    }

    pub(super) fn apply_camera_input(&mut self) {
        if self.input.look_x.is_finite() {
            self.target_yaw -= self.input.look_x.clamp(-10000.0, 10000.0) * LOOK_SENSITIVITY;
        }
        if self.input.look_y.is_finite() {
            self.target_pitch = (self.target_pitch + self.input.look_y * LOOK_SENSITIVITY)
                .clamp(-MAX_PITCH, MAX_PITCH);
        }
        if self.input.zoom_delta.is_finite() {
            // Distance-scaled zoom: precise near the face, fast across the map.
            // The offset lets the same gesture leave first person at zero.
            let factor = (self.input.zoom_delta / 10.0).clamp(-10.0, 10.0).exp();
            self.target_camera_distance = ((self.target_camera_distance + 2.0) * factor - 2.0)
                .clamp(0.0, MAX_CAMERA_DISTANCE);
        }
    }

    pub(super) fn smooth_camera(&mut self, delta: f32) {
        // Rebase both together, preserving accumulated multi-turn input.
        let turns = (self.view_yaw / std::f32::consts::TAU).trunc() * std::f32::consts::TAU;
        self.view_yaw -= turns;
        self.target_yaw -= turns;
        self.view_yaw = damp(self.view_yaw, self.target_yaw, 18.0, delta);
        self.view_pitch = damp(self.view_pitch, self.target_pitch, 14.0, delta);
        self.camera_distance = damp(
            self.camera_distance,
            self.target_camera_distance,
            16.0,
            delta,
        );
    }
}
