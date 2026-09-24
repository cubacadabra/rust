mod about_preview;
#[cfg(all(feature = "dev-showcase", not(target_arch = "wasm32")))]
pub(crate) mod capture;
mod character;
mod character_gpu;
mod character_material;
mod character_quality;
mod device;
mod draw;
mod effects;
mod hair_geometry;
mod hero_character;
mod hero_geometry;
#[cfg(all(feature = "dev-showcase", not(target_arch = "wasm32")))]
pub(crate) mod reference_capture;
mod rounded_geometry;
mod scene;
#[cfg(feature = "studio-ui")]
mod studio_camera;
mod targets;
mod ui;
#[cfg(feature = "dev-showcase")]
pub(crate) mod validation;
mod world_mesh;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};

use crate::types::BuildBlock;
use crate::ui::UiFrame;

#[cfg(debug_assertions)]
const DEBUG_GIT_SHA: &str = env!("CUBACADABRA_GIT_SHA");

include!("renderer/types.rs");
include!("renderer/geometry.rs");
include!("renderer/world_geometry.rs");

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct ShadowGlobals {
    pub(super) view_projection: [[f32; 4]; 4],
    pub(super) texel_size: [f32; 4],
}
