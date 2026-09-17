//! Shared gameplay camera geometry.

use crate::character::definition::{BodyId, camera_anchors};
use glam::Vec3;

pub(crate) const FIRST_PERSON_DISTANCE: f32 = 0.75;
const FULL_AVATAR_DISTANCE: f32 = 2.2;

pub(crate) fn fade(distance: f32) -> f32 {
    1.0 - ((distance - FIRST_PERSON_DISTANCE) / (FULL_AVATAR_DISTANCE - FIRST_PERSON_DISTANCE))
        .clamp(0.0, 1.0)
}

pub(crate) fn orbit(
    player: Vec3,
    body: BodyId,
    yaw: f32,
    pitch: f32,
    distance: f32,
) -> (Vec3, Vec3) {
    // Pitch describes the orbit elevation. First person looks along the same
    // ray as third person, including the yaw sign and vertical drag direction.
    let outward = Vec3::new(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        yaw.cos() * pitch.cos(),
    );
    let (anchor, third_person_target) = camera_anchors(body);
    let eye = player + anchor;
    if distance <= FIRST_PERSON_DISTANCE {
        return (eye, eye - outward);
    }
    let t = ((distance - FIRST_PERSON_DISTANCE) / 4.25).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    let target = eye.lerp(player + third_person_target, t);
    // Ease out from the eye instead of jumping behind the head on entry.
    let radius =
        distance * ((distance - FIRST_PERSON_DISTANCE) / FIRST_PERSON_DISTANCE).clamp(0.0, 1.0);
    let mut position = target + outward * radius;
    // Low-angle orbits also stay above a raised support under the character.
    position.y = position.y.max(player.y + 0.15);
    (position, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_orbit_and_first_person_share_the_same_view_direction() {
        for yaw in [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, std::f32::consts::TAU] {
            let (eye, aim) = orbit(Vec3::ZERO, BodyId::Person, yaw, 0.2, 0.0);
            let (camera, target) = orbit(Vec3::ZERO, BodyId::Person, yaw, 0.2, 8.0);
            assert!((aim - eye).normalize().dot((target - camera).normalize()) > 0.999);
        }
    }

    #[test]
    fn zoom_transition_is_continuous_and_low_views_stay_above_ground() {
        let (eye, _) = orbit(Vec3::ZERO, BodyId::Cat, 1.0, 0.0, FIRST_PERSON_DISTANCE);
        let (near, _) = orbit(
            Vec3::ZERO,
            BodyId::Cat,
            1.0,
            0.0,
            FIRST_PERSON_DISTANCE + 0.001,
        );
        assert!(eye.distance(near) < 0.005);
        for pitch in [-1.45, -0.5, 0.0, 1.45] {
            let (position, target) = orbit(Vec3::ZERO, BodyId::Dragon, 2.0, pitch, 120.0);
            assert!(position.is_finite() && target.is_finite() && position.y >= 0.15);
        }
    }

    #[test]
    fn fade_has_hidden_blended_and_full_avatar_zones() {
        assert_eq!(fade(FIRST_PERSON_DISTANCE), 1.0);
        assert!(fade(1.75) > 0.0 && fade(1.75) < 1.0);
        assert_eq!(fade(FULL_AVATAR_DISTANCE), 0.0);
    }
}
