//! Bounded presentation-quality decisions for the character renderer.
//!
//! These functions are deliberately CPU-only and deterministic. They keep
//! distance/viewport policy out of mesh construction, so a quality change can
//! select another immutable catalog entry without changing simulation or
//! appearance identity.

use super::{character, RenderEntity};
use crate::character::{BodyId, OutfitId};
use glam::{Mat4, Vec3};

pub(super) const LOD_NEAR_PIXELS: f32 = 180.0;
pub(super) const LOD_FAR_PIXELS: f32 = 70.0;
pub(super) const LOD_HYSTERESIS_PIXELS: f32 = 12.0;
pub(super) const CHARACTER_FOV_Y_RADIANS: f32 = 62.0_f32.to_radians();
pub(super) const CHARACTER_NEAR_PLANE: f32 = 0.05;
pub(super) const CHARACTER_FAR_PLANE: f32 = 240.0;
pub(super) const MAX_EFFECTS: usize = 128;
pub(super) const MAX_EFFECTS_PER_CHARACTER: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CharacterLod {
    Near,
    Mid,
    Far,
}

impl CharacterLod {
    pub(super) const ALL: [Self; 3] = [Self::Near, Self::Mid, Self::Far];

    pub(super) const fn subdivisions(self) -> u32 {
        match self {
            Self::Near => 4,
            Self::Mid => 2,
            Self::Far => 1,
        }
    }

    pub(super) const fn index(self) -> usize {
        match self {
            Self::Near => 0,
            Self::Mid => 1,
            Self::Far => 2,
        }
    }
}

/// Select a tier from projected height. The optional previous tier gives
/// hysteresis at the two boundaries, preventing visible flicker while a
/// character hovers around a threshold.
pub(super) fn select_lod(projected_height: f32, previous: Option<CharacterLod>) -> CharacterLod {
    if !projected_height.is_finite() || projected_height <= 0.0 {
        return CharacterLod::Far;
    }
    match previous {
        Some(CharacterLod::Near) if projected_height >= LOD_NEAR_PIXELS - LOD_HYSTERESIS_PIXELS => {
            CharacterLod::Near
        }
        Some(CharacterLod::Mid) if projected_height >= LOD_NEAR_PIXELS - LOD_HYSTERESIS_PIXELS => {
            CharacterLod::Near
        }
        Some(CharacterLod::Mid) if projected_height >= LOD_FAR_PIXELS - LOD_HYSTERESIS_PIXELS => {
            CharacterLod::Mid
        }
        Some(CharacterLod::Far) if projected_height >= LOD_FAR_PIXELS - LOD_HYSTERESIS_PIXELS => {
            CharacterLod::Mid
        }
        _ if projected_height >= LOD_NEAR_PIXELS => CharacterLod::Near,
        _ if projected_height >= LOD_FAR_PIXELS => CharacterLod::Mid,
        _ => CharacterLod::Far,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CharacterBounds {
    pub(super) center: Vec3,
    pub(super) radius: f32,
}

pub(super) fn bounds(body: BodyId, outfit: OutfitId) -> CharacterBounds {
    let (center, radius) = character::bounds(body, outfit);
    CharacterBounds { center, radius }
}

fn view_center(entity: RenderEntity, view: Mat4, body: BodyId, outfit: OutfitId) -> Vec3 {
    let local = bounds(body, outfit).center;
    let world = Mat4::from_rotation_y(entity.yaw).transform_point3(local)
        + Vec3::from_array(entity.position);
    view.transform_point3(world)
}

pub(super) fn is_visible(entity: RenderEntity, view: Mat4, aspect: f32) -> bool {
    let bounds = bounds(entity.body, entity.outfit);
    let center = view_center(entity, view, entity.body, entity.outfit);
    let radius = bounds.radius;
    if !center.is_finite() || !radius.is_finite() || radius <= 0.0 {
        return false;
    }
    let depth = -center.z;
    if depth + radius < CHARACTER_NEAR_PLANE || depth - radius > CHARACTER_FAR_PLANE {
        return false;
    }

    // A sphere tested against the camera-space frustum is conservative for
    // animated tails, hats and seam travel. No visible geometry is culled by
    // a moving joint because the bounds already include the authored margin.
    let aspect = aspect.clamp(0.1, 10.0);
    let half_height = depth.max(CHARACTER_NEAR_PLANE) * (CHARACTER_FOV_Y_RADIANS * 0.5).tan();
    let half_width = half_height * aspect;
    center.x.abs() - radius <= half_width && center.y.abs() - radius <= half_height
}

pub(super) fn projected_height(entity: RenderEntity, view: Mat4, viewport_height: f32) -> f32 {
    if !viewport_height.is_finite() || viewport_height <= 0.0 {
        return 0.0;
    }
    let bounds = bounds(entity.body, entity.outfit);
    let center = view_center(entity, view, entity.body, entity.outfit);
    let depth = -center.z;
    if !depth.is_finite() || depth <= CHARACTER_NEAR_PLANE * 0.25 {
        return f32::INFINITY;
    }
    viewport_height * bounds.radius * 2.0 / (2.0 * depth * (CHARACTER_FOV_Y_RADIANS * 0.5).tan())
}

/// Returns whether an effect should be admitted to the fixed live-effect
/// budget. Local/nearby characters are added first by the draw path, so the
/// rank is deterministic and naturally prioritizes them under pressure.
pub(super) fn admit_effect(
    effect_rank: usize,
    effect_count: usize,
    lod: CharacterLod,
    reduced_effects: bool,
) -> bool {
    !reduced_effects
        && lod != CharacterLod::Far
        && effect_rank < MAX_EFFECTS
        && effect_count < MAX_EFFECTS_PER_CHARACTER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projected_size_thresholds_are_hysteretic_and_bounded() {
        assert_eq!(select_lod(220.0, None), CharacterLod::Near);
        assert_eq!(select_lod(100.0, None), CharacterLod::Mid);
        assert_eq!(select_lod(20.0, None), CharacterLod::Far);
        assert_eq!(
            select_lod(LOD_NEAR_PIXELS - 1.0, Some(CharacterLod::Near)),
            CharacterLod::Near
        );
        assert_eq!(
            select_lod(LOD_FAR_PIXELS - 1.0, Some(CharacterLod::Mid)),
            CharacterLod::Mid
        );
        assert_eq!(select_lod(f32::NAN, None), CharacterLod::Far);
    }

    #[test]
    fn effect_budget_prioritizes_nearby_effects_and_reduced_mode() {
        assert!(admit_effect(0, 0, CharacterLod::Near, false));
        assert!(!admit_effect(MAX_EFFECTS, 0, CharacterLod::Near, false));
        assert!(!admit_effect(
            0,
            MAX_EFFECTS_PER_CHARACTER,
            CharacterLod::Near,
            false
        ));
        assert!(!admit_effect(0, 0, CharacterLod::Far, false));
        assert!(!admit_effect(0, 0, CharacterLod::Near, true));
    }

    #[test]
    fn invalid_bounds_are_not_visible() {
        let mut entity = RenderEntity::default();
        entity.position = [f32::NAN, 0.0, 0.0];
        assert!(!is_visible(entity, Mat4::IDENTITY, 16.0 / 9.0));
    }

    #[test]
    fn every_bundled_fit_has_a_conservative_positive_bound() {
        for body in BodyId::ALL {
            for outfit in OutfitId::ALL {
                let bound = bounds(body, outfit);
                assert!(bound.center.is_finite());
                assert!(bound.radius.is_finite() && bound.radius > 0.0);
            }
        }
    }
}
