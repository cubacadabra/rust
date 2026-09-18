//! Creator-only camera navigation. Never mutates the gameplay camera or snapshots.
use glam::Vec3;

use crate::StudioCameraPreset;

#[derive(Default)]
pub(super) struct StudioCamera {
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: Vec3,
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
        let distance = fit_distance(minimum, maximum, base_direction, aspect) * self.zoom.exp();
        let target = center + self.pan;
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
        self.zoom = (self.zoom - zoom * 0.1).clamp(-5.0, 4.0);
        let forward = (target - camera).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward);
        let units_per_point =
            2.0 * camera.distance(target) * 31.0_f32.to_radians().tan() / viewport_height;
        self.pan += (-right * pan[0] + up * pan[1]) * units_per_point;
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
}
