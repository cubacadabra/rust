fn add_atlas_rect(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    rect: UiRect,
    uv: [f32; 4],
    color: [f32; 4],
) {
    let [u0, v0, u1, v1] = uv;
    let points = [
        (rect.x, rect.y, [u0, v0]),
        (rect.x + rect.width, rect.y, [u1, v0]),
        (rect.x + rect.width, rect.y + rect.height, [u1, v1]),
        (rect.x, rect.y + rect.height, [u0, v1]),
    ];
    add_ui_tinted_image_triangle(vertices, frame, points[0], points[1], points[2], color);
    add_ui_tinted_image_triangle(vertices, frame, points[0], points[2], points[3], color);
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

fn add_line(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    start: (f32, f32),
    end: (f32, f32),
    width: f32,
    color: [f32; 4],
) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let length = dx.hypot(dy).max(0.001);
    let half_width = width.max(0.5) * 0.5;
    let nx = -dy / length * half_width;
    let ny = dx / length * half_width;
    let points = [
        (start.0 + nx, start.1 + ny),
        (end.0 + nx, end.1 + ny),
        (end.0 - nx, end.1 - ny),
        (start.0 - nx, start.1 - ny),
    ];
    add_ui_triangle(vertices, frame, points[0], points[1], points[2], color);
    add_ui_triangle(vertices, frame, points[0], points[2], points[3], color);
}

fn add_icon(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    name: &str,
    rect: UiRect,
    color: [f32; 4],
) {
    let shadow_rect = UiRect {
        x: rect.x + 1.0,
        y: rect.y + 1.5,
        ..rect
    };
    draw_icon(
        vertices,
        frame,
        name,
        shadow_rect,
        [0.01, 0.04, 0.05, color[3] * 0.68],
    );
    draw_icon(vertices, frame, name, rect, color);
}

fn draw_cube_mark(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    left: f32,
    top: f32,
    size: f32,
    color: [f32; 4],
) {
    let top_point = (left + size * 0.50, top + size * 0.06);
    let upper_right = (left + size * 0.88, top + size * 0.26);
    let center = (left + size * 0.50, top + size * 0.48);
    let upper_left = (left + size * 0.12, top + size * 0.26);
    let lower_left = (left + size * 0.12, top + size * 0.70);
    let bottom = (left + size * 0.50, top + size * 0.93);
    let lower_right = (left + size * 0.88, top + size * 0.70);
    let top_face = [color[0], color[1], color[2], color[3]];
    let left_face = [color[0] * 0.82, color[1] * 0.82, color[2] * 0.82, color[3]];
    let right_face = [color[0] * 0.64, color[1] * 0.64, color[2] * 0.64, color[3]];
    add_ui_triangle(vertices, frame, top_point, upper_right, center, top_face);
    add_ui_triangle(vertices, frame, top_point, center, upper_left, top_face);
    add_ui_triangle(vertices, frame, upper_left, center, bottom, left_face);
    add_ui_triangle(vertices, frame, upper_left, bottom, lower_left, left_face);
    add_ui_triangle(
        vertices,
        frame,
        center,
        upper_right,
        lower_right,
        right_face,
    );
    add_ui_triangle(vertices, frame, center, lower_right, bottom, right_face);
}

fn draw_icon(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    name: &str,
    rect: UiRect,
    color: [f32; 4],
) {
    let size = rect.width.min(rect.height).clamp(20.0, 38.0);
    let left = rect.x + (rect.width - size) * 0.5;
    let top = rect.y + (rect.height - size) * 0.5;
    let right = left + size;
    let bottom = top + size;
    let mid_x = (left + right) * 0.5;
    let mid_y = (top + bottom) * 0.5;
    let stroke = (size * 0.13).max(2.2);

    match name.to_ascii_lowercase().as_str() {
        "plus" | "add" => {
            add_line(
                vertices,
                frame,
                (mid_x, top + 4.0),
                (mid_x, bottom - 4.0),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (left + 4.0, mid_y),
                (right - 4.0, mid_y),
                stroke,
                color,
            );
        }
        "place" => {
            let cube_size = size * 0.72;
            draw_cube_mark(vertices, frame, left, top + size * 0.20, cube_size, color);
            let badge_x = right - size * 0.18;
            let badge_y = top + size * 0.22;
            add_circle(
                vertices,
                frame,
                badge_x,
                badge_y,
                size * 0.22,
                [0.02, 0.12, 0.18, color[3]],
            );
            add_line(
                vertices,
                frame,
                (badge_x, badge_y - size * 0.11),
                (badge_x, badge_y + size * 0.11),
                stroke * 0.72,
                color,
            );
            add_line(
                vertices,
                frame,
                (badge_x - size * 0.11, badge_y),
                (badge_x + size * 0.11, badge_y),
                stroke * 0.72,
                color,
            );
        }
        "rotate" => {
            let radius = size * 0.34;
            let mut previous = None;
            for step in 0..=9 {
                let angle = PI * 0.22 + step as f32 / 9.0 * PI * 1.48;
                let point = (mid_x + angle.cos() * radius, mid_y + angle.sin() * radius);
                if let Some(previous) = previous {
                    add_line(vertices, frame, previous, point, stroke, color);
                }
                previous = Some(point);
            }
            let tip = (mid_x + radius * 0.76, mid_y - radius * 0.76);
            add_ui_triangle(
                vertices,
                frame,
                tip,
                (tip.0 - size * 0.25, tip.1 - size * 0.02),
                (tip.0 - size * 0.03, tip.1 + size * 0.24),
                color,
            );
        }
        "remove" | "trash" | "delete" => {
            add_rounded_rect(
                vertices,
                frame,
                UiRect {
                    x: left + size * 0.24,
                    y: top + size * 0.31,
                    width: size * 0.52,
                    height: size * 0.53,
                },
                size * 0.07,
                color,
            );
            add_rounded_rect(
                vertices,
                frame,
                UiRect {
                    x: left + size * 0.18,
                    y: top + size * 0.22,
                    width: size * 0.64,
                    height: size * 0.12,
                },
                size * 0.05,
                color,
            );
            add_rounded_rect(
                vertices,
                frame,
                UiRect {
                    x: left + size * 0.38,
                    y: top + size * 0.12,
                    width: size * 0.24,
                    height: size * 0.13,
                },
                size * 0.05,
                color,
            );
            let cutout = [0.02, 0.12, 0.18, color[3] * 0.72];
            add_line(
                vertices,
                frame,
                (mid_x - size * 0.10, top + size * 0.43),
                (mid_x - size * 0.08, bottom - size * 0.25),
                stroke * 0.48,
                cutout,
            );
            add_line(
                vertices,
                frame,
                (mid_x + size * 0.10, top + size * 0.43),
                (mid_x + size * 0.08, bottom - size * 0.25),
                stroke * 0.48,
                cutout,
            );
        }
        "cube" | "build" => {
            draw_cube_mark(vertices, frame, left, top, size, color);
        }
        "beam" => {
            add_line(
                vertices,
                frame,
                (left + 4.0, top + 7.0),
                (right - 4.0, top + 7.0),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (right - 4.0, top + 7.0),
                (right - 4.0, bottom - 7.0),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (right - 4.0, bottom - 7.0),
                (left + 4.0, bottom - 7.0),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (left + 4.0, bottom - 7.0),
                (left + 4.0, top + 7.0),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (left + 6.0, mid_y),
                (right - 6.0, mid_y),
                stroke,
                color,
            );
        }
        "slab" => {
            add_line(
                vertices,
                frame,
                (left + 4.0, mid_y),
                (mid_x, top + 5.0),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (mid_x, top + 5.0),
                (right - 4.0, mid_y),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (right - 4.0, mid_y),
                (mid_x, bottom - 5.0),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (mid_x, bottom - 5.0),
                (left + 4.0, mid_y),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (left + 7.0, mid_y),
                (right - 7.0, mid_y),
                stroke,
                color,
            );
        }
        "palette" | "color" | "colorswap" => {
            add_circle(vertices, frame, left + 8.0, mid_y, 5.5, color);
            add_circle(vertices, frame, right - 8.0, mid_y, 5.5, color);
            add_line(
                vertices,
                frame,
                (left + 10.0, top + 7.0),
                (right - 7.0, top + 7.0),
                stroke * 0.7,
                color,
            );
            add_ui_triangle(
                vertices,
                frame,
                (right - 5.0, top + 7.0),
                (right - 11.0, top + 2.0),
                (right - 11.0, top + 12.0),
                color,
            );
            add_line(
                vertices,
                frame,
                (right - 10.0, bottom - 7.0),
                (left + 7.0, bottom - 7.0),
                stroke * 0.7,
                color,
            );
            add_ui_triangle(
                vertices,
                frame,
                (left + 5.0, bottom - 7.0),
                (left + 11.0, bottom - 12.0),
                (left + 11.0, bottom - 2.0),
                color,
            );
        }
        "save" => {
            add_rounded_rect(
                vertices,
                frame,
                UiRect {
                    x: left + 4.0,
                    y: top + 3.0,
                    width: size - 8.0,
                    height: size - 6.0,
                },
                2.5,
                color,
            );
            add_rect(
                vertices,
                frame,
                UiRect {
                    x: left + 8.0,
                    y: top + 5.0,
                    width: size - 16.0,
                    height: 5.0,
                },
                [0.01, 0.04, 0.05, color[3] * 0.75],
            );
            add_rounded_rect(
                vertices,
                frame,
                UiRect {
                    x: left + 8.0,
                    y: bottom - 9.0,
                    width: size - 16.0,
                    height: 5.0,
                },
                1.5,
                [0.01, 0.04, 0.05, color[3] * 0.75],
            );
        }
        "close" | "exit" => {
            add_line(
                vertices,
                frame,
                (left + 5.0, top + 5.0),
                (right - 5.0, bottom - 5.0),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (right - 5.0, top + 5.0),
                (left + 5.0, bottom - 5.0),
                stroke,
                color,
            );
        }
        "chevrondown" | "down" => {
            add_line(
                vertices,
                frame,
                (left + 5.0, top + 8.0),
                (mid_x, bottom - 7.0),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (mid_x, bottom - 7.0),
                (right - 5.0, top + 8.0),
                stroke,
                color,
            );
        }
        "check" => {
            add_line(
                vertices,
                frame,
                (left + 4.0, mid_y),
                (mid_x - 1.0, bottom - 5.0),
                stroke,
                color,
            );
            add_line(
                vertices,
                frame,
                (mid_x - 1.0, bottom - 5.0),
                (right - 4.0, top + 5.0),
                stroke,
                color,
            );
        }
        _ => {
            add_circle(vertices, frame, mid_x, mid_y, size * 0.36, color);
            add_circle(
                vertices,
                frame,
                mid_x,
                mid_y,
                size * 0.18,
                [0.01, 0.04, 0.05, color[3] * 0.8],
            );
        }
    }
}


