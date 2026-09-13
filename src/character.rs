//! CPU-owned character definitions used by both the engine-facing presentation
//! layer and the renderer.  Nothing in this module owns a GPU resource.

pub(crate) mod animation;
pub(crate) mod catalog;
pub(crate) mod definition;
pub(crate) mod face;
pub(crate) mod gait;
pub(crate) mod hair;
pub(crate) mod rig;

pub(crate) const STANCE_ANKLE_HEIGHT: f32 = 0.05;
#[cfg(any(test, feature = "dev-showcase"))]
pub(crate) const GAIT_STANCE_PHASE: f32 = std::f32::consts::PI;

#[cfg(any(test, feature = "dev-showcase"))]
pub(crate) fn foot_is_planted(phase: f32) -> bool {
    phase.rem_euclid(std::f32::consts::TAU) < GAIT_STANCE_PHASE
}

#[cfg(feature = "rendering")]
pub(crate) use animation::{AnimationOutput, CharacterPresentationState, SecondaryMotion};
pub(crate) use definition::{
    AppearanceInput, BodyId, BodyPart, BodyRecipe, CharacterAppearance, CharacterColors, OutfitId,
    body_recipe, resolve_appearance,
};
pub(crate) use face::{FaceParameters, FacePreset};
pub(crate) use rig::{JointId, Pose};
