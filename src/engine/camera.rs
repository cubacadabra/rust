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

    /// Apply a server correction to the locally predicted player. The server
    /// validates travel distance rather than simulating rigid-body collisions.
    pub fn reconcile_player(&mut self, position: [f32; 3], yaw: f32) {
        self.player.position = position;
        self.player.velocity = [0.0; 3];
        self.player.grounded = position[1] <= 0.05;
        self.view_yaw = yaw;
        self.target_yaw = yaw;
        self.write_snapshot();
    }

    pub fn camera(&self) -> [f32; 3] {
        [self.view_yaw, self.view_pitch, self.camera_distance]
    }

    pub(super) fn apply_camera_input(&mut self) {
        self.target_yaw -= self.input.look_x * LOOK_SENSITIVITY;
        self.target_pitch =
            (self.target_pitch + self.input.look_y * LOOK_SENSITIVITY).clamp(-MAX_PITCH, MAX_PITCH);
        self.target_camera_distance =
            (self.target_camera_distance + self.input.zoom_delta).clamp(0.0, MAX_CAMERA_DISTANCE);
    }

    pub(super) fn smooth_camera(&mut self, delta: f32) {
        self.view_yaw = damp(self.view_yaw, self.target_yaw, 10.0, delta);
        self.view_pitch = damp(self.view_pitch, self.target_pitch, 10.0, delta);
        self.camera_distance = damp(
            self.camera_distance,
            self.target_camera_distance,
            9.0,
            delta,
        );
    }
}
