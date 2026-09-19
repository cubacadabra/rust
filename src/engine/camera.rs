use crate::engine::{Engine, LOOK_SENSITIVITY, MAX_CAMERA_DISTANCE, MAX_PITCH};
use crate::math::damp;
use crate::world::Aabb;
use glam::Vec3;

const CAMERA_COLLISION_RADIUS: f32 = 0.35;
// Reaching 95% of the requested distance takes about 250 ms. Obstruction
// clamps do not use this response and therefore still move inward immediately.
const CAMERA_OUTWARD_RESPONSE: f32 = 12.0;
const CAMERA_ZOOM_RESPONSE: f32 = 16.0;

impl Engine {
    /// Moves the local preview player near a selected Studio scene object.
    ///
    /// This is an editor-only presentation action. It updates local runtime
    /// state immediately so it also works while the preview is stopped; it
    /// does not change authored spawn data.
    /// `object_radius` is the selected object's horizontal half-extent.
    #[cfg(feature = "studio-ui")]
    pub fn studio_move_player_near(&mut self, target: [f32; 3], object_radius: f32) {
        if !target.iter().all(|value| value.is_finite()) {
            return;
        }

        const PLAYER_CLEARANCE: f32 = 1.75;
        let preview_distance = object_radius
            .is_finite()
            .then_some(object_radius.max(0.0) + PLAYER_CLEARANCE)
            .unwrap_or(4.0)
            .max(4.0);
        let current = self.player.position;
        let delta_x = current[0] - target[0];
        let delta_z = current[2] - target[2];
        let distance = delta_x.hypot(delta_z);
        let (direction_x, direction_z) = if distance > 0.001 {
            (delta_x / distance, delta_z / distance)
        } else {
            (0.0, 1.0)
        };
        let position = self
            .studio_preview_position(
                target,
                current[1],
                direction_x,
                direction_z,
                preview_distance,
            )
            .unwrap_or([
                target[0] + direction_x * preview_distance,
                current[1],
                target[2] + direction_z * preview_distance,
            ]);
        let look_x = target[0] - position[0];
        let look_z = target[2] - position[2];
        let yaw = (-look_x).atan2(-look_z);

        self.player.position = position;
        self.player.velocity = [0.0; 3];
        self.player.grounded = true;
        self.player.climbing = false;
        self.player.moving = false;
        self.player.sprinting = false;
        self.player.facing_yaw = yaw;
        self.view_yaw = yaw;
        self.target_yaw = yaw;
        self.pending_reconciliation = [0.0; 3];
        self.write_snapshot();
    }

    #[cfg(feature = "studio-ui")]
    fn studio_preview_position(
        &self,
        target: [f32; 3],
        feet_y: f32,
        direction_x: f32,
        direction_z: f32,
        preview_distance: f32,
    ) -> Option<[f32; 3]> {
        // The first candidate follows the direction the player came from.
        // If that side is occupied by a table, wall, or other runtime
        // collision, walk around the object until a clear side is found.
        const ANGLE_OFFSETS: [f32; 15] = [
            0.0, 0.35, -0.35, 0.7, -0.7, 1.05, -1.05, 1.4, -1.4, 1.75, -1.75, 2.1, -2.1, 2.6, -2.6,
        ];
        for extra_distance in [0.0, 0.75, 1.5, 2.25, 3.0, 4.0] {
            let distance = preview_distance + extra_distance;
            for angle in ANGLE_OFFSETS {
                let (sin, cos) = angle.sin_cos();
                let x = direction_x * cos - direction_z * sin;
                let z = direction_x * sin + direction_z * cos;
                let candidate = [target[0] + x * distance, feet_y, target[2] + z * distance];
                if self.player_can_occupy(candidate) {
                    return Some(candidate);
                }
            }
        }
        None
    }

    /// Restores the normal classic third-person orbit.
    pub fn reset_view(&mut self) {
        if !self.apply_authored_world_camera() {
            self.view_yaw = 0.0;
            self.view_pitch = crate::engine::DEFAULT_ORBIT_PITCH;
            self.target_yaw = 0.0;
            self.target_pitch = crate::engine::DEFAULT_ORBIT_PITCH;
            self.camera_distance = crate::engine::DEFAULT_ORBIT_DISTANCE;
            self.target_camera_distance = crate::engine::DEFAULT_ORBIT_DISTANCE;
        }
    }

    /// Applies a package-authored gameplay camera for the active world.
    /// Returns false when the world intentionally uses the legacy camera.
    pub(crate) fn apply_authored_world_camera(&mut self) -> bool {
        let Some(camera) = self
            .worlds
            .get(self.active_world)
            .and_then(|world| world.camera)
        else {
            return false;
        };
        self.view_yaw = camera.yaw;
        self.view_pitch = camera.pitch;
        self.target_yaw = camera.yaw;
        self.target_pitch = camera.pitch;
        self.camera_distance = camera.distance;
        self.target_camera_distance = camera.distance;
        true
    }

    pub fn reset_showcase_view(&mut self) {
        self.reset_view();
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
        if self.input.zoom_delta.is_finite() && self.input.zoom_delta != 0.0 {
            // Distance-scaled zoom: precise near the face, fast across the map.
            // The offset lets the same gesture leave first person at zero.
            let factor = (self.input.zoom_delta / 10.0).clamp(-10.0, 10.0).exp();
            self.target_camera_distance = ((self.target_camera_distance + 2.0) * factor - 2.0)
                .clamp(0.0, MAX_CAMERA_DISTANCE);
        }
    }

    pub(super) fn smooth_camera_orientation(&mut self, delta: f32) {
        // Rebase both together, preserving accumulated multi-turn input.
        let turns = (self.view_yaw / std::f32::consts::TAU).trunc() * std::f32::consts::TAU;
        self.view_yaw -= turns;
        self.target_yaw -= turns;
        self.view_yaw = damp(self.view_yaw, self.target_yaw, 18.0, delta);
        self.view_pitch = damp(self.view_pitch, self.target_pitch, 14.0, delta);
    }

    pub(super) fn resolve_camera_distance(&mut self, delta: f32) {
        let response = if self.target_camera_distance > self.camera_distance {
            CAMERA_OUTWARD_RESPONSE
        } else {
            CAMERA_ZOOM_RESPONSE
        };
        let preferred = damp(
            self.camera_distance,
            self.target_camera_distance,
            response,
            delta,
        );
        self.camera_distance = self.occlusion_distance(preferred);
    }

    fn occlusion_distance(&self, preferred: f32) -> f32 {
        if preferred <= crate::camera::FIRST_PERSON_DISTANCE {
            return preferred;
        }

        let player = Vec3::from_array(self.player.position);
        let body = self.player_appearance.body;
        let mut resolved = preferred;
        // Near the first-person transition the orbit target also eases from
        // the eyes toward the torso. A few bounded refinements keep the final
        // camera sphere clear without putting that presentation curve into
        // terrain or obstacle collision code.
        for _ in 0..4 {
            if resolved <= crate::camera::FIRST_PERSON_DISTANCE {
                return resolved;
            }
            let (position, target) =
                crate::camera::orbit(player, body, self.view_yaw, self.view_pitch, resolved);
            let segment = position - target;
            let length = segment.length();
            if length <= f32::EPSILON || !length.is_finite() {
                return resolved;
            }
            let allowed = self.camera_sweep_distance(target, position, length);
            if allowed + 0.001 >= length {
                return resolved;
            }
            resolved *= (allowed / length).clamp(0.0, 1.0);
        }
        resolved
    }

    fn camera_sweep_distance(&self, start: Vec3, end: Vec3, length: f32) -> f32 {
        let mut nearest = length;
        for obstacle in &self.obstacles {
            if let Some(distance) =
                segment_expanded_aabb_distance(start, end, obstacle, CAMERA_COLLISION_RADIUS)
            {
                nearest = nearest.min(distance);
            }
        }
        if let Some(distance) = self.terrain.as_ref().and_then(|terrain| {
            terrain.sweep_sphere(start.to_array(), end.to_array(), CAMERA_COLLISION_RADIUS)
        }) {
            nearest = nearest.min(distance);
        }
        if let Some(distance) = self.static_collision.as_ref().and_then(|collision| {
            collision.sweep_sphere(start.to_array(), end.to_array(), CAMERA_COLLISION_RADIUS)
        }) {
            nearest = nearest.min(distance);
        }
        nearest
    }
}

fn segment_expanded_aabb_distance(
    start: Vec3,
    end: Vec3,
    obstacle: &Aabb,
    radius: f32,
) -> Option<f32> {
    let minimum = Vec3::new(
        obstacle.min_x - radius,
        obstacle.bottom - radius,
        obstacle.min_z - radius,
    );
    let maximum = Vec3::new(
        obstacle.max_x + radius,
        obstacle.top + radius,
        obstacle.max_z + radius,
    );
    let direction = end - start;
    let length = direction.length();
    if length <= f32::EPSILON || !length.is_finite() {
        return None;
    }
    let mut entry = 0.0_f32;
    let mut exit = 1.0_f32;
    for axis in 0..3 {
        let origin = start[axis];
        let delta = direction[axis];
        if delta.abs() <= f32::EPSILON {
            if origin < minimum[axis] || origin > maximum[axis] {
                return None;
            }
            continue;
        }
        let inverse = delta.recip();
        let mut near = (minimum[axis] - origin) * inverse;
        let mut far = (maximum[axis] - origin) * inverse;
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        entry = entry.max(near);
        exit = exit.min(far);
        if entry > exit {
            return None;
        }
    }
    (exit >= 0.0 && entry <= 1.0).then(|| entry.max(0.0) * length)
}
