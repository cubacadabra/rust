use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::OnceLock;

#[cfg(target_os = "ios")]
use std::sync::atomic::{AtomicUsize, Ordering};

use fontdue::{Font, FontSettings};
use glam::Vec3;

use crate::ui::{UiAlignment, UiFrame, UiNodeKind, UiRect, UiRenderNode};

use super::Vertex;

const UI_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/RobotoCondensed-Light.ttf");

fn ui_font() -> &'static Font {
    static FONT: OnceLock<Font> = OnceLock::new();
    FONT.get_or_init(|| {
        #[cfg(target_os = "ios")]
        eprintln!(
            "[RustRenderer] loading bundled UI font RobotoCondensed-Light.ttf ({} bytes)",
            UI_FONT_BYTES.len()
        );
        Font::from_bytes(UI_FONT_BYTES, FontSettings::default())
            .expect("bundled Lilita One font should be valid")
    })
}

#[cfg(target_os = "ios")]
static LAST_UI_DRAW_VERTEX_COUNT: AtomicUsize = AtomicUsize::new(usize::MAX);

#[cfg(target_os = "ios")]
fn log_ui_draw_vertex_count(count: usize) {
    if LAST_UI_DRAW_VERTEX_COUNT.swap(count, Ordering::Relaxed) != count {
        eprintln!("[RustRenderer] UI draw vertices={count}");
    }
}

pub(super) fn build_ui_vertices(frame: &UiFrame) -> Vec<Vertex> {
    if frame.viewport.width <= 0.0 || frame.viewport.height <= 0.0 {
        #[cfg(target_os = "ios")]
        log_ui_draw_vertex_count(0);
        return Vec::new();
    }
    let mut vertices = Vec::with_capacity(frame.nodes.len() * 96);
    for node in &frame.nodes {
        add_node(&mut vertices, frame, node);
    }
    #[cfg(target_os = "ios")]
    log_ui_draw_vertex_count(vertices.len());
    vertices
}

fn add_node(vertices: &mut Vec<Vertex>, frame: &UiFrame, node: &UiRenderNode) {
    let mut opacity = if node.disabled { 0.48 } else { 1.0 };
    if node.pressed {
        opacity *= 0.78;
    }

    let has_surface = node.background.is_some() || node.border_color.is_some();
    let is_control = matches!(
        node.kind,
        UiNodeKind::Button | UiNodeKind::Toggle | UiNodeKind::Slider
    );
    let automatic_border = is_control && node.border_color.is_none() && node.background.is_some();
    let border_width = if node.border_color.is_some() || automatic_border {
        node.border_width.max(1.0)
    } else {
        node.border_width
    };

    // A quiet, close shadow keeps translucent controls legible against the 3D
    // world. It is deliberately tight so the HUD still feels light on mobile.
    if has_surface {
        add_rounded_rect(
            vertices,
            frame,
            UiRect {
                y: node.rect.y + 3.0,
                ..node.rect
            },
            node.corner_radius,
            faded([0.01, 0.04, 0.05, 0.25], opacity),
        );
    }

    if let Some(border) = node
        .border_color
        .or_else(|| automatic_border.then_some([0.82, 0.94, 0.94, 0.12]))
    {
        add_rounded_rect(
            vertices,
            frame,
            node.rect,
            node.corner_radius,
            faded(border, opacity),
        );
    }
    if let Some(background) = node.background {
        let inset = border_width.min(node.rect.width.min(node.rect.height) * 0.5);
        add_rounded_rect(
            vertices,
            frame,
            inset_rect(node.rect, inset),
            (node.corner_radius - inset).max(0.0),
            faded(background, opacity),
        );

        // A narrow upper sheen gives the dark surfaces a material edge and
        // makes the primary controls read as intentional interactive objects.
        if node.rect.height >= 28.0 {
            let highlight = inset_rect(node.rect, border_width + 1.0);
            add_rounded_rect(
                vertices,
                frame,
                UiRect {
                    height: (highlight.height * 0.34).max(2.0),
                    ..highlight
                },
                (node.corner_radius - border_width - 1.0).max(0.0),
                faded([0.92, 0.98, 0.97, 0.075], opacity),
            );
        }
    }

    if node.pressed && has_surface {
        add_rounded_rect(
            vertices,
            frame,
            inset_rect(node.rect, border_width),
            (node.corner_radius - border_width).max(0.0),
            faded([0.01, 0.03, 0.04, 0.10], opacity),
        );
    }

    match node.kind {
        UiNodeKind::Toggle => add_toggle(vertices, frame, node, opacity),
        UiNodeKind::Slider => add_slider(vertices, frame, node, opacity),
        UiNodeKind::Joystick => add_joystick(vertices, frame, node, opacity),
        _ => {}
    }
    if !node.text.is_empty() {
        let text_rect = match node.kind {
            UiNodeKind::Toggle => UiRect {
                width: (node.rect.width - 68.0).max(0.0),
                ..node.rect
            },
            _ => node.rect,
        };
        add_text(
            vertices,
            frame,
            &node.text,
            text_rect,
            node.font_size,
            node.text_align,
            faded(node.foreground, opacity),
        );
    }
}

fn add_joystick(vertices: &mut Vec<Vertex>, frame: &UiFrame, node: &UiRenderNode, opacity: f32) {
    let radius = node.rect.width.min(node.rect.height) * 0.5;
    let center_x = node.rect.x + node.rect.width * 0.5;
    let center_y = node.rect.y + node.rect.height * 0.5;
    add_circle(
        vertices,
        frame,
        center_x,
        center_y,
        radius,
        faded([0.04, 0.08, 0.09, 0.52], opacity),
    );
    add_circle(
        vertices,
        frame,
        center_x,
        center_y,
        (radius - 2.0).max(0.0),
        faded([0.08, 0.14, 0.15, 0.76], opacity),
    );
    add_circle(
        vertices,
        frame,
        center_x,
        center_y,
        (radius - 7.0).max(0.0),
        faded([0.11, 0.19, 0.20, 0.28], opacity),
    );
    let travel = radius * 0.46;
    let knob_x = center_x + node.value_x.clamp(-1.0, 1.0) * travel;
    let knob_y = center_y + node.value_y.clamp(-1.0, 1.0) * travel;
    add_circle(
        vertices,
        frame,
        knob_x,
        knob_y + 2.0,
        radius * 0.235,
        faded([0.01, 0.04, 0.05, 0.32], opacity),
    );
    add_circle(
        vertices,
        frame,
        knob_x,
        knob_y,
        radius * 0.21,
        faded(node.foreground, opacity),
    );
}

fn add_toggle(vertices: &mut Vec<Vertex>, frame: &UiFrame, node: &UiRenderNode, opacity: f32) {
    let track = UiRect {
        x: node.rect.x + node.rect.width - 50.0,
        y: node.rect.y + (node.rect.height - 28.0) * 0.5,
        width: 48.0,
        height: 28.0,
    };
    let off = [0.34, 0.38, 0.40, 0.9];
    add_rounded_rect(
        vertices,
        frame,
        track,
        14.0,
        faded(if node.value >= 0.5 { node.accent } else { off }, opacity),
    );
    let thumb_x = if node.value >= 0.5 {
        track.x + track.width - 14.0
    } else {
        track.x + 14.0
    };
    add_circle(
        vertices,
        frame,
        thumb_x,
        track.y + track.height * 0.5 + 1.0,
        10.5,
        faded([0.01, 0.04, 0.05, 0.30], opacity),
    );
    add_circle(
        vertices,
        frame,
        thumb_x,
        track.y + track.height * 0.5,
        10.0,
        faded([1.0, 1.0, 1.0, 1.0], opacity),
    );
}

fn add_slider(vertices: &mut Vec<Vertex>, frame: &UiFrame, node: &UiRenderNode, opacity: f32) {
    let track = UiRect {
        x: node.rect.x + 12.0,
        y: node.rect.y + node.rect.height * 0.5 - 3.0,
        width: (node.rect.width - 24.0).max(1.0),
        height: 6.0,
    };
    add_rounded_rect(
        vertices,
        frame,
        track,
        3.0,
        faded([0.35, 0.39, 0.41, 0.9], opacity),
    );
    let fill = UiRect {
        width: track.width * node.value.clamp(0.0, 1.0),
        ..track
    };
    add_rounded_rect(vertices, frame, fill, 3.0, faded(node.accent, opacity));
    add_circle(
        vertices,
        frame,
        track.x + track.width * node.value.clamp(0.0, 1.0),
        track.y + track.height * 0.5 + 1.0,
        10.5,
        faded([0.01, 0.04, 0.05, 0.30], opacity),
    );
    add_circle(
        vertices,
        frame,
        track.x + track.width * node.value.clamp(0.0, 1.0),
        track.y + track.height * 0.5,
        10.0,
        faded([1.0, 1.0, 1.0, 1.0], opacity),
    );
}

fn add_text(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    text: &str,
    rect: UiRect,
    font_size: f32,
    alignment: UiAlignment,
    color: [f32; 4],
) {
    let font_size = font_size.max(7.0);
    let lines = text
        .lines()
        .take(8)
        .map(|line| {
            let glyphs = line
                .chars()
                .filter(|character| character.is_ascii())
                .map(|character| {
                    let character = character.to_ascii_uppercase();
                    let (metrics, bitmap) = ui_font().rasterize(character, font_size);
                    UiGlyph { metrics, bitmap }
                })
                .take(96)
                .collect::<Vec<_>>();
            let width = glyphs.iter().map(|glyph| glyph.metrics.advance_width).sum();
            UiTextLine { glyphs, width }
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return;
    }

    let line_height = ui_font()
        .horizontal_line_metrics(font_size)
        .map(|metrics| metrics.new_line_size)
        .unwrap_or(font_size * 1.2)
        .max(font_size * 1.1);
    let block_height = lines.len() as f32 * line_height;
    let first_baseline = rect.y
        + (rect.height - block_height).max(0.0) * 0.5
        + ui_font()
            .horizontal_line_metrics(font_size)
            .map_or(font_size * 0.82, |metrics| metrics.ascent);

    // Shadow, then a broad game-style outline, then the anti-aliased face.
    add_raster_text(
        vertices,
        frame,
        &lines,
        rect,
        first_baseline,
        line_height,
        alignment,
        [0.01, 0.04, 0.05, color[3] * 0.42],
        [2.0, 3.0],
    );
    for offset in [
        [-1.7, -1.3],
        [0.0, -1.9],
        [1.7, -1.3],
        [-1.9, 0.0],
        [1.9, 0.0],
        [-1.5, 1.4],
        [0.0, 1.8],
        [1.5, 1.4],
    ] {
        add_raster_text(
            vertices,
            frame,
            &lines,
            rect,
            first_baseline,
            line_height,
            alignment,
            [0.01, 0.025, 0.03, color[3] * 0.90],
            offset,
        );
    }
    add_raster_text(
        vertices,
        frame,
        &lines,
        rect,
        first_baseline,
        line_height,
        alignment,
        color,
        [0.0, 0.0],
    );
}

struct UiGlyph {
    metrics: fontdue::Metrics,
    bitmap: Vec<u8>,
}

struct UiTextLine {
    glyphs: Vec<UiGlyph>,
    width: f32,
}

#[allow(clippy::too_many_arguments)]
fn add_raster_text(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    lines: &[UiTextLine],
    rect: UiRect,
    first_baseline: f32,
    line_height: f32,
    alignment: UiAlignment,
    color: [f32; 4],
    offset: [f32; 2],
) {
    for (line_index, line) in lines.iter().enumerate() {
        let line_width = line.width;
        let start_x = match alignment {
            UiAlignment::Center | UiAlignment::Stretch => {
                rect.x + (rect.width - line_width).max(0.0) * 0.5
            }
            UiAlignment::End => rect.x + (rect.width - line_width).max(0.0),
            UiAlignment::Start => rect.x,
        };
        let mut cursor_x = start_x + offset[0];
        let baseline = first_baseline + line_index as f32 * line_height + offset[1];
        for glyph in &line.glyphs {
            let top = baseline - glyph.metrics.ymin as f32 - glyph.metrics.height as f32;
            let left = cursor_x + glyph.metrics.xmin as f32;
            for row in 0..glyph.metrics.height {
                for column in 0..glyph.metrics.width {
                    let coverage = glyph.bitmap[row * glyph.metrics.width + column];
                    if coverage == 0 {
                        continue;
                    }
                    let mut pixel_color = color;
                    pixel_color[3] *= coverage as f32 / 255.0;
                    add_rect(
                        vertices,
                        frame,
                        UiRect {
                            x: left + column as f32,
                            y: top + row as f32,
                            width: 1.05,
                            height: 1.05,
                        },
                        pixel_color,
                    );
                }
            }
            cursor_x += glyph.metrics.advance_width;
        }
    }
}

fn add_circle(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    x: f32,
    y: f32,
    radius: f32,
    color: [f32; 4],
) {
    add_rounded_rect(
        vertices,
        frame,
        UiRect {
            x: x - radius,
            y: y - radius,
            width: radius * 2.0,
            height: radius * 2.0,
        },
        radius,
        color,
    );
}

fn add_rounded_rect(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    rect: UiRect,
    radius: f32,
    color: [f32; 4],
) {
    if rect.width <= 0.0 || rect.height <= 0.0 || color[3] <= 0.0 {
        return;
    }
    let radius = radius.clamp(0.0, rect.width.min(rect.height) * 0.5);
    if radius <= 0.5 {
        add_rect(vertices, frame, rect, color);
        return;
    }
    let centers = [
        (rect.x + rect.width - radius, rect.y + radius, -FRAC_PI_2),
        (
            rect.x + rect.width - radius,
            rect.y + rect.height - radius,
            0.0,
        ),
        (rect.x + radius, rect.y + rect.height - radius, FRAC_PI_2),
        (rect.x + radius, rect.y + radius, PI),
    ];
    let mut perimeter = Vec::with_capacity(20);
    const CORNER_SEGMENTS: usize = 5;
    for (center_x, center_y, start_angle) in centers {
        for segment in 0..CORNER_SEGMENTS {
            let angle = start_angle + segment as f32 / (CORNER_SEGMENTS - 1) as f32 * FRAC_PI_2;
            perimeter.push((
                center_x + angle.cos() * radius,
                center_y + angle.sin() * radius,
            ));
        }
    }
    let center = (rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
    for index in 0..perimeter.len() {
        let next = (index + 1) % perimeter.len();
        add_ui_triangle(
            vertices,
            frame,
            center,
            perimeter[index],
            perimeter[next],
            color,
        );
    }
}

fn add_rect(vertices: &mut Vec<Vertex>, frame: &UiFrame, rect: UiRect, color: [f32; 4]) {
    if rect.width <= 0.0 || rect.height <= 0.0 || color[3] <= 0.0 {
        return;
    }
    let top_left = (rect.x, rect.y);
    let top_right = (rect.x + rect.width, rect.y);
    let bottom_right = (rect.x + rect.width, rect.y + rect.height);
    let bottom_left = (rect.x, rect.y + rect.height);
    add_ui_triangle(vertices, frame, top_left, top_right, bottom_right, color);
    add_ui_triangle(vertices, frame, top_left, bottom_right, bottom_left, color);
}

fn add_ui_triangle(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    a: (f32, f32),
    b: (f32, f32),
    c: (f32, f32),
    color: [f32; 4],
) {
    for (x, y) in [a, b, c] {
        let x = x / frame.viewport.width.max(1.0) * 2.0 - 1.0;
        let y = 1.0 - y / frame.viewport.height.max(1.0) * 2.0;
        vertices.push(Vertex {
            position: [x, y, 0.0],
            normal: Vec3::ZERO.to_array(),
            color,
        });
    }
}

fn inset_rect(rect: UiRect, inset: f32) -> UiRect {
    UiRect {
        x: rect.x + inset,
        y: rect.y + inset,
        width: (rect.width - inset * 2.0).max(0.0),
        height: (rect.height - inset * 2.0).max(0.0),
    }
}

fn faded(mut color: [f32; 4], opacity: f32) -> [f32; 4] {
    color[3] *= opacity;
    color
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{UiNodeKind, UiViewport};

    #[test]
    fn ui_vertices_are_in_clip_space() {
        let frame = UiFrame {
            viewport: UiViewport {
                width: 390.0,
                height: 844.0,
                scale: 1.0,
                safe_area: Default::default(),
            },
            nodes: vec![UiRenderNode {
                id: "button".to_owned(),
                kind: UiNodeKind::Button,
                rect: UiRect {
                    x: 20.0,
                    y: 20.0,
                    width: 100.0,
                    height: 44.0,
                },
                text: "GO".to_owned(),
                background: Some([0.0, 0.5, 1.0, 1.0]),
                foreground: [1.0; 4],
                border_color: None,
                border_width: 0.0,
                corner_radius: 12.0,
                font_size: 14.0,
                text_align: UiAlignment::Center,
                accent: [0.0, 0.5, 1.0, 1.0],
                value: 0.0,
                value_x: 0.0,
                value_y: 0.0,
                pressed: false,
                disabled: false,
            }],
        };
        let vertices = build_ui_vertices(&frame);
        assert!(!vertices.is_empty());
        assert!(vertices.iter().all(|vertex| {
            (-1.0..=1.0).contains(&vertex.position[0]) && (-1.0..=1.0).contains(&vertex.position[1])
        }));
    }
}
