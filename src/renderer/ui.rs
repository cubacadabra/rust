use std::f32::consts::{FRAC_PI_2, PI};

use glam::Vec3;

use crate::ui::{UiAlignment, UiFrame, UiNodeKind, UiRect, UiRenderNode};

use super::{Vertex, glyph};

pub(super) fn build_ui_vertices(frame: &UiFrame) -> Vec<Vertex> {
    if frame.viewport.width <= 0.0 || frame.viewport.height <= 0.0 {
        return Vec::new();
    }
    let mut vertices = Vec::with_capacity(frame.nodes.len() * 96);
    for node in &frame.nodes {
        add_node(&mut vertices, frame, node);
    }
    vertices
}

fn add_node(vertices: &mut Vec<Vertex>, frame: &UiFrame, node: &UiRenderNode) {
    let mut opacity = if node.disabled { 0.48 } else { 1.0 };
    if node.pressed {
        opacity *= 0.78;
    }
    if let Some(border) = node.border_color {
        add_rounded_rect(
            vertices,
            frame,
            node.rect,
            node.corner_radius,
            faded(border, opacity),
        );
    }
    if let Some(background) = node.background {
        let inset = node
            .border_width
            .min(node.rect.width.min(node.rect.height) * 0.5);
        add_rounded_rect(
            vertices,
            frame,
            inset_rect(node.rect, inset),
            (node.corner_radius - inset).max(0.0),
            faded(background, opacity),
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
                width: (node.rect.width - 60.0).max(0.0),
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
        (radius - 1.5).max(0.0),
        faded([0.08, 0.14, 0.15, 0.68], opacity),
    );
    let travel = radius * 0.46;
    add_circle(
        vertices,
        frame,
        center_x + node.value_x.clamp(-1.0, 1.0) * travel,
        center_y + node.value_y.clamp(-1.0, 1.0) * travel,
        radius * 0.23,
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
    let pixel = (font_size / 7.0).max(1.0);
    let line_height = pixel * 9.0;
    let lines = text.lines().take(8).collect::<Vec<_>>();
    let block_height = lines.len() as f32 * line_height - pixel * 2.0;
    let first_y = rect.y + (rect.height - block_height).max(0.0) * 0.5;
    for (line_index, line) in lines.into_iter().enumerate() {
        let characters = line
            .chars()
            .filter(|character| character.is_ascii())
            .map(|character| character.to_ascii_uppercase())
            .take(96)
            .collect::<Vec<_>>();
        let line_width = characters.len().saturating_mul(6).saturating_sub(1) as f32 * pixel;
        let start_x = match alignment {
            UiAlignment::Center | UiAlignment::Stretch => {
                rect.x + (rect.width - line_width).max(0.0) * 0.5
            }
            UiAlignment::End => rect.x + (rect.width - line_width).max(0.0),
            UiAlignment::Start => rect.x,
        };
        for (character_index, character) in characters.into_iter().enumerate() {
            for (row, bits) in glyph(character).into_iter().enumerate() {
                for column in 0..5 {
                    if bits & (1 << (4 - column)) == 0 {
                        continue;
                    }
                    let pixel_rect = UiRect {
                        x: start_x + (character_index * 6 + column) as f32 * pixel,
                        y: first_y + line_index as f32 * line_height + row as f32 * pixel,
                        width: pixel * 0.84,
                        height: pixel * 0.84,
                    };
                    add_rect(vertices, frame, pixel_rect, color);
                }
            }
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
