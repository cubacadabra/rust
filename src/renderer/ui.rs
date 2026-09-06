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

pub(super) fn add_world_label(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    center_x: f32,
    anchor_y: f32,
    text: &str,
    font_size: f32,
) {
    let font_size = font_size.clamp(10.0, 20.0);
    let text = text
        .chars()
        .filter(|character| character.is_ascii())
        .take(24)
        .collect::<String>();
    if text.trim().is_empty() || frame.viewport.width <= 0.0 || frame.viewport.height <= 0.0 {
        return;
    }

    let scale = font_size / UI_FONT_ATLAS_SIZE;
    let text_width = text
        .chars()
        .map(|character| {
            let character = character.to_ascii_uppercase();
            ui_atlas_glyphs()
                .iter()
                .find(|glyph| glyph.character == character)
                .or_else(|| ui_atlas_glyphs().iter().find(|glyph| glyph.character == '?'))
                .map(|glyph| glyph.metrics.advance_width * scale)
                .unwrap_or(font_size * 0.55)
        })
        .sum::<f32>();
    let available_width = (frame.viewport.width - 8.0).max(1.0);
    let bubble_width = (text_width + 24.0)
        .clamp(58.0, 250.0)
        .min(available_width);
    let bubble_height = (font_size + 13.0).clamp(25.0, 34.0);
    let half_width = bubble_width * 0.5;
    let x = center_x.clamp(half_width + 4.0, frame.viewport.width - half_width - 4.0);
    let y = (anchor_y - bubble_height - 10.0).max(4.0);
    let bubble = UiRect {
        x: x - half_width,
        y,
        width: bubble_width,
        height: bubble_height,
    };

    let tail_center = x;
    add_ui_triangle(
        vertices,
        frame,
        (tail_center - 9.0, y + bubble_height - 1.0),
        (tail_center + 9.0, y + bubble_height - 1.0),
        (tail_center, y + bubble_height + 10.0),
        [0.01, 0.03, 0.04, 0.26],
    );
    add_rounded_rect(
        vertices,
        frame,
        UiRect {
            x: bubble.x + 1.5,
            y: bubble.y + 2.0,
            ..bubble
        },
        10.0,
        [0.01, 0.03, 0.04, 0.24],
    );
    add_ui_triangle(
        vertices,
        frame,
        (tail_center - 7.0, y + bubble_height - 1.0),
        (tail_center + 7.0, y + bubble_height - 1.0),
        (tail_center, y + bubble_height + 7.0),
        [0.98, 0.98, 0.94, 0.98],
    );
    add_rounded_rect(
        vertices,
        frame,
        bubble,
        10.0,
        [0.98, 0.98, 0.94, 0.98],
    );
    add_text(
        vertices,
        frame,
        &text,
        UiRect {
            x: bubble.x + 7.0,
            y: bubble.y + 7.0,
            width: bubble.width - 14.0,
            height: bubble.height - 14.0,
        },
        font_size,
        UiAlignment::Center,
        [0.06, 0.09, 0.11, 1.0],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{UiNodeKind, UiViewport};

    include!("ui/tests.rs");
}
