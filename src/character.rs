//! CPU-owned character definitions used by both the engine-facing presentation
//! layer and the renderer.  Nothing in this module owns a GPU resource.

pub(crate) mod animation;
pub(crate) mod catalog;
pub(crate) mod definition;
pub(crate) mod face;
pub(crate) mod rig;

pub(crate) use animation::{AnimationOutput, CharacterPresentationState, SecondaryMotion};
pub(crate) use definition::{
    AppearanceInput, BodyId, BodyPart, BodyRecipe, CharacterAppearance, CharacterColors,
    OutfitId, body_recipe, resolve_appearance,
};
pub(crate) use face::{FaceParameters, FacePreset};
pub(crate) use rig::{JointId, Pose};
