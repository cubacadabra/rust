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
    // Fine tessellation plus a transparent fringe keeps rounded surfaces
    // smooth at iPad pixel densities even though the UI pass has no MSAA.
    const CORNER_SEGMENTS: usize = 16;
    const EDGE_ANTIALIAS: f32 = 1.0;
    let perimeter = rounded_perimeter(rect, radius, CORNER_SEGMENTS);
    let fringe = rounded_perimeter(
        UiRect {
            x: rect.x - EDGE_ANTIALIAS,
            y: rect.y - EDGE_ANTIALIAS,
            width: rect.width + EDGE_ANTIALIAS * 2.0,
            height: rect.height + EDGE_ANTIALIAS * 2.0,
        },
        radius + EDGE_ANTIALIAS,
        CORNER_SEGMENTS,
    );

    for index in 0..perimeter.len() {
        let next = (index + 1) % perimeter.len();
        add_ui_gradient_triangle(
            vertices,
            frame,
            [fringe[index], fringe[next], perimeter[next]],
            [
                [color[0], color[1], color[2], 0.0],
                [color[0], color[1], color[2], 0.0],
                color,
            ],
        );
        add_ui_gradient_triangle(
            vertices,
            frame,
            [fringe[index], perimeter[next], perimeter[index]],
            [[color[0], color[1], color[2], 0.0], color, color],
        );
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

fn rounded_perimeter(rect: UiRect, radius: f32, corner_segments: usize) -> Vec<(f32, f32)> {
    let radius = radius.clamp(0.0, rect.width.min(rect.height) * 0.5);
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
    let mut perimeter = Vec::with_capacity(corner_segments * centers.len());
    for (center_x, center_y, start_angle) in centers {
        for segment in 0..corner_segments {
            let angle = start_angle + segment as f32 / (corner_segments - 1) as f32 * FRAC_PI_2;
            perimeter.push((
                center_x + angle.cos() * radius,
                center_y + angle.sin() * radius,
            ));
        }
    }
    perimeter
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

fn add_image(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    rect: UiRect,
    image: UiImage,
    invert: bool,
    opacity: f32,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let [u0, v0, u1, v1] = image_uv(image);
    let top_left = (rect.x, rect.y, [u0, v0]);
    let top_right = (rect.x + rect.width, rect.y, [u1, v0]);
    let bottom_right = (rect.x + rect.width, rect.y + rect.height, [u1, v1]);
    let bottom_left = (rect.x, rect.y + rect.height, [u0, v1]);
    add_ui_image_triangle(
        vertices,
        frame,
        top_left,
        top_right,
        bottom_right,
        opacity,
        invert,
    );
    add_ui_image_triangle(
        vertices,
        frame,
        top_left,
        bottom_right,
        bottom_left,
        opacity,
        invert,
    );
}

fn image_uv(image: UiImage) -> [f32; 4] {
    let padding = UI_ATLAS_PADDING as f32;
    let (x, width, height) = match image {
        UiImage::Logo => (padding, 1206.0, 1206.0),
        UiImage::Cube => (1212.0, 512.0, 512.0),
        UiImage::Chat => (1728.0, 512.0, 512.0),
        UiImage::Voice => (2244.0, 512.0, 512.0),
    };
    [
        x / UI_ATLAS_WIDTH as f32,
        padding / UI_ATLAS_HEIGHT as f32,
        (x + width) / UI_ATLAS_WIDTH as f32,
        (padding + height) / UI_ATLAS_HEIGHT as f32,
    ]
}

fn add_ui_image_triangle(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    a: (f32, f32, [f32; 2]),
    b: (f32, f32, [f32; 2]),
    c: (f32, f32, [f32; 2]),
    opacity: f32,
    invert: bool,
) {
    for (x, y, tex_coords) in [a, b, c] {
        let x = x / frame.viewport.width.max(1.0) * 2.0 - 1.0;
        let y = 1.0 - y / frame.viewport.height.max(1.0) * 2.0;
        vertices.push(Vertex {
            position: [x, y, 0.0],
            normal: Vec3::ZERO.to_array(),
            color: [1.0, 1.0, 1.0, opacity],
            tex_coords,
            image_invert: f32::from(invert),
        });
    }
}

fn add_ui_tinted_image_triangle(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    a: (f32, f32, [f32; 2]),
    b: (f32, f32, [f32; 2]),
    c: (f32, f32, [f32; 2]),
    color: [f32; 4],
) {
    for (x, y, tex_coords) in [a, b, c] {
        let x = x / frame.viewport.width.max(1.0) * 2.0 - 1.0;
        let y = 1.0 - y / frame.viewport.height.max(1.0) * 2.0;
        vertices.push(Vertex {
            position: [x, y, 0.0],
            normal: Vec3::ZERO.to_array(),
            color,
            tex_coords,
            image_invert: 0.0,
        });
    }
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
            tex_coords: [-1.0, -1.0],
            image_invert: 0.0,
        });
    }
}

fn add_ui_gradient_triangle(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    points: [(f32, f32); 3],
    colors: [[f32; 4]; 3],
) {
    for ((x, y), color) in points.into_iter().zip(colors) {
        let x = x / frame.viewport.width.max(1.0) * 2.0 - 1.0;
        let y = 1.0 - y / frame.viewport.height.max(1.0) * 2.0;
        vertices.push(Vertex {
            position: [x, y, 0.0],
            normal: Vec3::ZERO.to_array(),
            color,
            tex_coords: [-1.0, -1.0],
            image_invert: 0.0,
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

