//! Creator-only camera navigation. Never mutates the gameplay camera or snapshots.
use glam::Vec3;

use crate::StudioCameraPreset;

#[derive(Default)]
pub(super) struct StudioCamera {
    yaw: f32,
    pitch: f32,
    distance: Option<f32>,
    target: Option<Vec3>,
}

impl StudioCamera {
    pub(super) fn view(
        &self,
        preset: StudioCameraPreset,
        minimum: Vec3,
        maximum: Vec3,
        aspect: f32,
    ) -> (Vec3, Vec3) {
        let center = (minimum + maximum) * 0.5;
        let base_direction = match preset {
            StudioCameraPreset::Overview => Vec3::new(0.92, 1.18, 0.92),
            _ => Vec3::new(1.35, 0.22, 1.15),
        }
        .normalize();
        let base_yaw = base_direction.x.atan2(base_direction.z);
        let base_pitch = base_direction.y.asin();
        let yaw = base_yaw + self.yaw;
        let pitch = (base_pitch + self.pitch).clamp(-1.5, 1.5);
        let direction = Vec3::new(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            yaw.cos() * pitch.cos(),
        );
        // Fit the projected corners, not a sphere containing all the empty
        // space around a flat island. Use the preset orientation for a stable
        // orbit distance while dragging.
        let distance = self
            .distance
            .unwrap_or_else(|| fit_distance(minimum, maximum, base_direction, aspect));
        let target = self.target.unwrap_or(center);
        (target + direction * distance, target)
    }

    pub(super) fn navigate(
        &mut self,
        orbit: [f32; 2],
        pan: [f32; 2],
        zoom: f32,
        camera: Vec3,
        target: Vec3,
        viewport_height: f32,
    ) {
        if !orbit
            .into_iter()
            .chain(pan)
            .chain([zoom, viewport_height])
            .all(f32::is_finite)
            || viewport_height <= 0.0
        {
            return;
        }
        self.yaw = (self.yaw - orbit[0].clamp(-10000.0, 10000.0) * 0.005)
            .rem_euclid(std::f32::consts::TAU);
        let current_pitch = (camera - target).normalize().y.asin();
        self.pitch += (current_pitch + orbit[1] * 0.005).clamp(-1.5, 1.5) - current_pitch;
        let forward = (target - camera).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward);
        let units_per_point =
            2.0 * camera.distance(target) * 31.0_f32.to_radians().tan() / viewport_height;
        self.target = Some(target + (-right * pan[0] + up * pan[1]) * units_per_point);
        self.distance = Some(
            (camera.distance(target) * (-zoom * 0.1).clamp(-5.0, 4.0).exp()).clamp(0.25, 10_000.0),
        );
    }

    pub(super) fn focus(&mut self, point: Vec3, radius: f32) {
        if !point.is_finite() || !radius.is_finite() {
            return;
        }
        self.target = Some(point);
        let desired_distance = (radius.max(0.25) * 2.4).max(2.0);
        self.distance = Some(desired_distance);
    }

    pub(super) fn ground_axes(
        &self,
        preset: StudioCameraPreset,
        minimum: Vec3,
        maximum: Vec3,
        aspect: f32,
    ) -> ([f32; 2], [f32; 2]) {
        let (camera, target) = self.view(preset, minimum, maximum, aspect);
        let forward = target - camera;
        let ground_forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let ground_right = ground_forward.cross(Vec3::Y).normalize_or_zero();
        (
            [ground_right.x, ground_right.z],
            [ground_forward.x, ground_forward.z],
        )
    }
}

fn fit_distance(minimum: Vec3, maximum: Vec3, direction: Vec3, aspect: f32) -> f32 {
    let center = (minimum + maximum) * 0.5;
    let right = Vec3::Y.cross(direction).normalize();
    let up = direction.cross(right);
    let vertical = 31.0_f32.to_radians().tan();
    let horizontal = vertical * aspect.max(0.1);
    let mut distance: f32 = 8.0;
    for x in [minimum.x, maximum.x] {
        for y in [minimum.y, maximum.y] {
            for z in [minimum.z, maximum.z] {
                let point = Vec3::new(x, y, z) - center;
                let depth = point.dot(direction);
                distance = distance.max(
                    depth
                        + (point.dot(right).abs() / horizontal).max(point.dot(up).abs() / vertical)
                            * 1.08,
                );
            }
        }
    }
    distance
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;

    const MIN: Vec3 = Vec3::new(-64.0, -58.0, -60.0);
    const MAX: Vec3 = Vec3::new(90.0, 34.0, 100.0);

    #[test]
    fn review_presets_fit_all_corners_at_narrow_and_wide_aspects() {
        for aspect in [0.5, 1.0, 16.0 / 9.0, 2.5] {
            for preset in [StudioCameraPreset::Overview, StudioCameraPreset::Showcase] {
                let (eye, target) = StudioCamera::default().view(preset, MIN, MAX, aspect);
                let matrix = Mat4::perspective_rh(62.0_f32.to_radians(), aspect, 0.05, 2000.0)
                    * Mat4::look_at_rh(eye, target, Vec3::Y);
                for x in [MIN.x, MAX.x] {
                    for y in [MIN.y, MAX.y] {
                        for z in [MIN.z, MAX.z] {
                            let clip = matrix * Vec3::new(x, y, z).extend(1.0);
                            let ndc = clip.truncate() / clip.w;
                            assert!(ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0 && clip.w > 0.0);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn navigation_orbits_pans_and_zooms_without_moving_the_subject() {
        let mut camera = StudioCamera::default();
        let view = |camera: &StudioCamera| camera.view(StudioCameraPreset::Showcase, MIN, MAX, 1.5);
        let (eye, target) = view(&camera);
        camera.navigate([80.0, 30.0], [0.0; 2], 0.0, eye, target, 600.0);
        let (orbited, same_target) = view(&camera);
        assert_eq!(target, same_target);
        assert!(orbited.distance(eye) > 10.0);
        assert!((orbited.distance(target) - eye.distance(target)).abs() < 0.001);
        camera.navigate([0.0; 2], [40.0, -20.0], 6.0, orbited, target, 600.0);
        let (zoomed, panned) = view(&camera);
        assert!(zoomed.distance(panned) < eye.distance(target));
        assert!(panned.distance(target) > 1.0);
        camera.navigate(
            [f32::NAN, 0.0],
            [0.0; 2],
            f32::INFINITY,
            zoomed,
            panned,
            600.0,
        );
        assert_eq!(view(&camera), (zoomed, panned));
    }

    #[test]
    fn focus_centers_an_object_without_changing_the_orbit_direction() {
        let mut camera = StudioCamera::default();
        camera.navigate(
            [80.0, 30.0],
            [0.0; 2],
            0.0,
            Vec3::new(20.0, 10.0, 20.0),
            Vec3::ZERO,
            600.0,
        );
        let before = camera.ground_axes(StudioCameraPreset::Showcase, MIN, MAX, 1.5);
        let point = Vec3::new(12.0, 3.0, -8.0);
        camera.focus(point, 2.0);
        let (eye, target) = camera.view(StudioCameraPreset::Showcase, MIN, MAX, 1.5);
        assert_eq!(target, point);
        assert!(eye.distance(target) >= 4.0);
        let after = camera.ground_axes(StudioCameraPreset::Showcase, MIN, MAX, 1.5);
        for (actual, expected) in after
            .0
            .into_iter()
            .chain(after.1)
            .zip(before.0.into_iter().chain(before.1))
        {
            assert!((actual - expected).abs() < 0.0001);
        }
    }

    #[test]
    fn ground_axes_follow_the_visible_camera() {
        let mut camera = StudioCamera::default();
        let before = camera.ground_axes(StudioCameraPreset::Showcase, MIN, MAX, 1.5);
        let (eye, target) = camera.view(StudioCameraPreset::Showcase, MIN, MAX, 1.5);
        camera.navigate([120.0, 0.0], [0.0; 2], 0.0, eye, target, 600.0);
        let after = camera.ground_axes(StudioCameraPreset::Showcase, MIN, MAX, 1.5);
        assert_ne!(before, after);
        for axis in [after.0, after.1] {
            assert!((axis[0].hypot(axis[1]) - 1.0).abs() < 0.001);
        }
    }

    #[test]
    fn navigated_camera_survives_changed_world_bounds() {
        let mut camera = StudioCamera::default();
        let (eye, target) = camera.view(StudioCameraPreset::Showcase, MIN, MAX, 1.5);
        camera.navigate([25.0, -10.0], [18.0, 12.0], 3.0, eye, target, 600.0);
        let preserved = camera.view(StudioCameraPreset::Showcase, MIN, MAX, 1.5);
        let rebuilt = camera.view(
            StudioCameraPreset::Showcase,
            MIN - Vec3::splat(100.0),
            MAX + Vec3::splat(200.0),
            1.5,
        );
        assert_eq!(preserved, rebuilt);
    }
}
