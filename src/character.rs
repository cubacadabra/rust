//! CPU-owned character definitions used by both the engine-facing presentation
//! layer and the renderer.  Nothing in this module owns a GPU resource.

pub(crate) mod definition;
pub(crate) mod face;
pub(crate) mod rig;

pub(crate) use definition::{
    BodyId, BodyPart, BodyRecipe, body_recipe,
};
pub(crate) use face::{FaceParameters, FacePreset};
pub(crate) use rig::{JointId, Pose};
