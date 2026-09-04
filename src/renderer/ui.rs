use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::OnceLock;

#[cfg(target_os = "ios")]
use std::sync::atomic::{AtomicUsize, Ordering};

use fontdue::{Font, FontSettings};
use glam::Vec3;

use crate::ui::{UiAlignment, UiFrame, UiImage, UiNodeKind, UiRect, UiRenderNode};

use super::Vertex;

include!("ui/atlas.rs");
include!("ui/nodes.rs");
include!("ui/icons.rs");
include!("ui/primitives.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{UiNodeKind, UiViewport};

    include!("ui/tests.rs");
}
