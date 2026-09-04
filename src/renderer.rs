mod device;
mod draw;
mod scene;
mod ui;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};

use crate::types::BuildBlock;
use crate::ui::UiFrame;

include!("renderer/types.rs");
include!("renderer/geometry.rs");
include!("renderer/world_geometry.rs");
