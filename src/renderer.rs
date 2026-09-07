#[cfg(all(feature = "dev-showcase", not(target_arch = "wasm32")))]
pub(crate) mod capture;
#[cfg(feature = "dev-showcase")]
pub(crate) mod validation;
mod device;
mod draw;
mod character;
mod camera;
mod character_gpu;
mod character_material;
mod character_quality;
mod targets;
mod rounded_geometry;
mod hero_geometry;
mod hair_geometry;
mod hero_character;
mod scene;
mod ui;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};

use crate::types::BuildBlock;
use crate::ui::UiFrame;

#[cfg(debug_assertions)]
const DEBUG_GIT_SHA: &str = env!("CUBACADABRA_GIT_SHA");

include!("renderer/types.rs");
include!("renderer/geometry.rs");
include!("renderer/world_geometry.rs");
