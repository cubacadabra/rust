fn add_spawn_pad(vertices: &mut Vec<Vertex>, origin: Vec3, palette: RenderPalette, elapsed: f32) {
    add_cylinder(
        vertices,
        origin + Vec3::new(0.0, 0.08, 0.0),
        2.35,
        0.16,
        palette.ink,
    );
    add_cylinder(
        vertices,
        origin + Vec3::new(0.0, 0.18, 0.0),
        1.9,
        0.08,
        faded(palette.paper, 0.46),
    );
    add_ring(
        vertices,
        origin + Vec3::new(0.0, 0.24, 0.0),
        1.55,
        0.13,
        palette.ground_edge,
    );
    let rotation = Quat::from_rotation_y(elapsed * 0.22);
    for angle in [0.0_f32, std::f32::consts::FRAC_PI_2] {
        let transform = Mat4::from_translation(origin + Vec3::new(0.0, 0.30, 0.0))
            * Mat4::from_quat(rotation * Quat::from_rotation_y(angle));
        add_transformed_cuboid(
            vertices,
            transform,
            Vec3::new(2.45, 0.045, 0.12),
            faded(palette.paper, 0.62),
        );
    }
}

fn add_cloud(
    vertices: &mut Vec<Vertex>,
    cloud: &RenderCloud,
    index: usize,
    paper: [f32; 4],
    elapsed: f32,
) {
    let drift =
        (elapsed * (0.02 + index as f32 * 0.003) + index as f32).sin() * (1.2 + index as f32 * 0.2);
    let origin = Vec3::from_array(cloud.position) + Vec3::new(drift, 0.0, 0.0);
    let cloud_color = faded(paper, 0.52);
    for (offset, radius) in [
        (Vec3::new(-1.1, 0.0, 0.0), 1.1),
        (Vec3::new(0.0, 0.24, 0.0), 1.45),
        (Vec3::new(1.1, 0.02, 0.0), 0.92),
        (Vec3::new(0.42, -0.15, 0.08), 1.05),
    ] {
        add_sphere(
            vertices,
            origin + offset * cloud.scale,
            radius * cloud.scale,
            cloud_color,
        );
    }
}

fn add_launch_pad(
    vertices: &mut Vec<Vertex>,
    pad: &RenderPad,
    seconds: f32,
    palette: RenderPalette,
    elapsed: f32,
    index: usize,
) {
    let origin = Vec3::new(pad.x, 0.0, pad.z);
    add_cylinder(
        vertices,
        origin + Vec3::new(0.0, 0.10, 0.0),
        pad.radius + 0.45,
        0.20,
        palette.ink,
    );

    let enabled = pad.enabled;
    let disabled_color = faded(palette.ink, 0.32);
    let pad_color = if enabled { pad.color } else { disabled_color };
    let mut inner = pad_color;
    inner[3] = 0.30;
    add_cylinder(
        vertices,
        origin + Vec3::new(0.0, 0.215, 0.0),
        pad.radius,
        0.025,
        inner,
    );
    let pulse = 1.0 + (elapsed * 2.2 + index as f32 * 1.7).sin() * 0.045;
    let mut ring_color = pad_color;
    if seconds > 0.0 {
        ring_color[3] = 0.82 + (elapsed * 5.0).sin().abs() * 0.18;
    }
    add_ring(
        vertices,
        origin + Vec3::new(0.0, 0.28, 0.0),
        (pad.radius - 0.15).max(0.2) * pulse,
        0.11,
        ring_color,
    );

    for (beacon_index, x) in [-2.35, 2.35].into_iter().enumerate() {
        let y = 1.35 + (elapsed * 2.6 + index as f32 + beacon_index as f32).sin() * 0.12;
        add_cylinder(
            vertices,
            origin + Vec3::new(x, y, -0.35),
            0.19,
            2.70,
            pad_color,
        );
    }
    add_cuboid(
        vertices,
        origin + Vec3::new(0.0, 2.62, -0.35),
        Vec3::new(5.05, 0.32, 0.38),
        pad_color,
    );
    add_cuboid(
        vertices,
        origin + Vec3::new(0.0, 4.0, -0.35),
        Vec3::new(5.05, 0.82, 0.22),
        palette.ink,
    );
    add_cuboid(
        vertices,
        origin + Vec3::new(-2.42, 4.0, -0.23),
        Vec3::new(0.12, 0.82, 0.08),
        pad_color,
    );
    let label = if enabled {
        format!("{} {}", pad.code, pad.label)
    } else {
        pad.availability_label.clone()
    };
    add_pixel_text(
        vertices,
        label.trim(),
        origin + Vec3::new(0.08, 4.0, -0.225),
        0.0,
        4.55,
        palette.paper,
    );
    add_pixel_text(
        vertices,
        label.trim(),
        origin + Vec3::new(-0.08, 4.0, -0.475),
        std::f32::consts::PI,
        4.55,
        palette.paper,
    );
}

fn add_pixel_text(
    vertices: &mut Vec<Vertex>,
    text: &str,
    origin: Vec3,
    yaw: f32,
    max_width: f32,
    color: [f32; 4],
) {
    let characters = text
        .chars()
        .filter(|character| character.is_ascii())
        .map(|character| character.to_ascii_uppercase())
        .take(24)
        .collect::<Vec<_>>();
    if characters.is_empty() {
        return;
    }
    let columns = characters.len() * 6 - 1;
    let pixel = (max_width / columns as f32).min(0.072);
    let text_width = columns as f32 * pixel;
    let root = Mat4::from_translation(origin) * Mat4::from_quat(Quat::from_rotation_y(yaw));
    for (character_index, character) in characters.into_iter().enumerate() {
        let glyph = glyph(character);
        for (row, bits) in glyph.into_iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                let x =
                    character_index as f32 * pixel * 6.0 + column as f32 * pixel - text_width * 0.5;
                let y = (3.0 - row as f32) * pixel;
                add_transformed_cuboid(
                    vertices,
                    root * Mat4::from_translation(Vec3::new(x, y, 0.0)),
                    Vec3::new(pixel * 0.82, pixel * 0.82, 0.035),
                    color,
                );
            }
        }
    }
}

pub(super) fn glyph(character: char) -> [u8; 7] {
    match character {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [31, 4, 4, 4, 4, 4, 31],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        ':' => [0, 12, 12, 0, 12, 12, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        '&' => [12, 18, 20, 8, 21, 18, 13],
        '!' => [4, 4, 4, 4, 4, 0, 4],
        '?' => [14, 17, 1, 2, 4, 0, 4],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 31],
        _ => [0; 7],
    }
}

fn add_ring(
    vertices: &mut Vec<Vertex>,
    center: Vec3,
    radius: f32,
    thickness: f32,
    color: [f32; 4],
) {
    let inner_radius = (radius - thickness).max(0.05);
    let segments = 32;
    for index in 0..segments {
        let next = (index + 1) % segments;
        let a = index as f32 / segments as f32 * std::f32::consts::TAU;
        let b = next as f32 / segments as f32 * std::f32::consts::TAU;
        let outer_a = center + Vec3::new(a.cos() * radius, 0.0, a.sin() * radius);
        let outer_b = center + Vec3::new(b.cos() * radius, 0.0, b.sin() * radius);
        let inner_a = center + Vec3::new(a.cos() * inner_radius, 0.0, a.sin() * inner_radius);
        let inner_b = center + Vec3::new(b.cos() * inner_radius, 0.0, b.sin() * inner_radius);
        add_quad(vertices, outer_a, outer_b, inner_b, inner_a, Vec3::Y, color);
    }
}

fn add_cylinder(
    vertices: &mut Vec<Vertex>,
    center: Vec3,
    radius: f32,
    height: f32,
    color: [f32; 4],
) {
    let segments = 24;
    for index in 0..segments {
        let next = (index + 1) % segments;
        let a = index as f32 / segments as f32 * std::f32::consts::TAU;
        let b = next as f32 / segments as f32 * std::f32::consts::TAU;
        let bottom_a = center + Vec3::new(a.cos() * radius, -height / 2.0, a.sin() * radius);
        let bottom_b = center + Vec3::new(b.cos() * radius, -height / 2.0, b.sin() * radius);
        let top_a = center + Vec3::new(a.cos() * radius, height / 2.0, a.sin() * radius);
        let top_b = center + Vec3::new(b.cos() * radius, height / 2.0, b.sin() * radius);
        add_quad(
            vertices,
            bottom_a,
            bottom_b,
            top_b,
            top_a,
            Vec3::new(a.cos(), 0.0, a.sin()),
            color,
        );
        add_triangle(
            vertices,
            center + Vec3::new(0.0, height / 2.0, 0.0),
            top_a,
            top_b,
            Vec3::Y,
            color,
        );
    }
}

fn add_sphere(vertices: &mut Vec<Vertex>, center: Vec3, radius: f32, color: [f32; 4]) {
    let latitude_segments = 6;
    let longitude_segments = 12;
    let point = |latitude: usize, longitude: usize| {
        let vertical = latitude as f32 / latitude_segments as f32;
        let horizontal = longitude as f32 / longitude_segments as f32;
        let phi = vertical * std::f32::consts::PI;
        let theta = horizontal * std::f32::consts::TAU;
        Vec3::new(theta.cos() * phi.sin(), phi.cos(), theta.sin() * phi.sin())
    };
    for latitude in 0..latitude_segments {
        for longitude in 0..longitude_segments {
            let next = (longitude + 1) % longitude_segments;
            let normal_a = point(latitude, longitude);
            let normal_b = point(latitude + 1, longitude);
            let normal_c = point(latitude + 1, next);
            let normal_d = point(latitude, next);
            add_triangle(
                vertices,
                center + normal_a * radius,
                center + normal_b * radius,
                center + normal_c * radius,
                (normal_a + normal_b + normal_c).normalize_or_zero(),
                color,
            );
            add_triangle(
                vertices,
                center + normal_a * radius,
                center + normal_c * radius,
                center + normal_d * radius,
                (normal_a + normal_c + normal_d).normalize_or_zero(),
                color,
            );
        }
    }
}

fn add_quad(
    vertices: &mut Vec<Vertex>,
    a: Vec3,
    b: Vec3,
    c: Vec3,
    d: Vec3,
    normal: Vec3,
    color: [f32; 4],
) {
    add_triangle(vertices, a, b, c, normal, color);
    add_triangle(vertices, a, c, d, normal, color);
}

fn add_triangle(
    vertices: &mut Vec<Vertex>,
    a: Vec3,
    b: Vec3,
    c: Vec3,
    normal: Vec3,
    color: [f32; 4],
) {
    let normal = normal.to_array();
    vertices.extend([
        Vertex {
            position: a.to_array(),
            normal,
            color,
            tex_coords: [0.0, 0.0],
            image_invert: 0.0,
        },
        Vertex {
            position: b.to_array(),
            normal,
            color,
            tex_coords: [0.0, 0.0],
            image_invert: 0.0,
        },
        Vertex {
            position: c.to_array(),
            normal,
            color,
            tex_coords: [0.0, 0.0],
            image_invert: 0.0,
        },
    ]);
}

