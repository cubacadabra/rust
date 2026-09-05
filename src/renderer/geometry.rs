fn color(value: u32) -> [f32; 4] {
    [
        ((value >> 16) & 0xff) as f32 / 255.0,
        ((value >> 8) & 0xff) as f32 / 255.0,
        (value & 0xff) as f32 / 255.0,
        1.0,
    ]
}

fn faded(mut color: [f32; 4], alpha: f32) -> [f32; 4] {
    color[3] = alpha;
    color
}

fn add_cuboid(vertices: &mut Vec<Vertex>, center: Vec3, size: Vec3, color: [f32; 4]) {
    add_transformed_cuboid(vertices, Mat4::from_translation(center), size, color);
}

fn add_cuboid_outline(
    vertices: &mut Vec<Vertex>,
    center: Vec3,
    size: Vec3,
    thickness: f32,
    color: [f32; 4],
) {
    let half = size * 0.5;
    for y in [-half.y, half.y] {
        for z in [-half.z, half.z] {
            add_cuboid(
                vertices,
                center + Vec3::new(0.0, y, z),
                Vec3::new(size.x + thickness, thickness, thickness),
                color,
            );
        }
    }
    for x in [-half.x, half.x] {
        for z in [-half.z, half.z] {
            add_cuboid(
                vertices,
                center + Vec3::new(x, 0.0, z),
                Vec3::new(thickness, size.y + thickness, thickness),
                color,
            );
        }
    }
    for x in [-half.x, half.x] {
        for y in [-half.y, half.y] {
            add_cuboid(
                vertices,
                center + Vec3::new(x, y, 0.0),
                Vec3::new(thickness, thickness, size.z + thickness),
                color,
            );
        }
    }
}

fn add_transformed_cuboid(
    vertices: &mut Vec<Vertex>,
    transform: Mat4,
    size: Vec3,
    color: [f32; 4],
) {
    let half = size * 0.5;
    let corners = [
        Vec3::new(-half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, -half.z),
        Vec3::new(half.x, half.y, -half.z),
        Vec3::new(-half.x, half.y, -half.z),
        Vec3::new(-half.x, -half.y, half.z),
        Vec3::new(half.x, -half.y, half.z),
        Vec3::new(half.x, half.y, half.z),
        Vec3::new(-half.x, half.y, half.z),
    ]
    .map(|corner| transform.transform_point3(corner));
    let normal = |direction: Vec3| transform.transform_vector3(direction).normalize_or_zero();
    add_quad(
        vertices,
        corners[0],
        corners[1],
        corners[2],
        corners[3],
        normal(Vec3::NEG_Z),
        color,
    );
    add_quad(
        vertices,
        corners[5],
        corners[4],
        corners[7],
        corners[6],
        normal(Vec3::Z),
        color,
    );
    add_quad(
        vertices,
        corners[1],
        corners[5],
        corners[6],
        corners[2],
        normal(Vec3::X),
        color,
    );
    add_quad(
        vertices,
        corners[4],
        corners[0],
        corners[3],
        corners[7],
        normal(Vec3::NEG_X),
        color,
    );
    add_quad(
        vertices,
        corners[3],
        corners[2],
        corners[6],
        corners[7],
        normal(Vec3::Y),
        color,
    );
    add_quad(
        vertices,
        corners[4],
        corners[5],
        corners[1],
        corners[0],
        normal(Vec3::NEG_Y),
        color,
    );
}

fn add_avatar(
    vertices: &mut Vec<Vertex>,
    agent: RenderEntity,
    style: AvatarStyle,
    face_color: [f32; 4],
    rounded_mesh_cache: &mut rounded_geometry::RoundedMeshCache,
) {
    add_avatar_inner(
        vertices,
        agent,
        style,
        face_color,
        Some(rounded_mesh_cache),
    );
}

/// The Phase 0 capture deliberately keeps a hard-cuboid copy of the old
/// avatar path so committed before-images do not silently change when the
/// production comparison path advances.
#[cfg(feature = "dev-showcase")]
fn add_legacy_avatar(
    vertices: &mut Vec<Vertex>,
    agent: RenderEntity,
    style: AvatarStyle,
    face_color: [f32; 4],
) {
    add_avatar_inner(vertices, agent, style, face_color, None);
}

fn add_avatar_inner(
    vertices: &mut Vec<Vertex>,
    agent: RenderEntity,
    style: AvatarStyle,
    face_color: [f32; 4],
    mut rounded_mesh_cache: Option<&mut rounded_geometry::RoundedMeshCache>,
) {
    let mut shadow_color = face_color;
    shadow_color[3] = 0.14;
    add_cylinder(
        vertices,
        Vec3::new(agent.position[0], 0.018, agent.position[2]),
        0.72,
        0.025,
        shadow_color,
    );
    let root = Mat4::from_translation(Vec3::from_array(agent.position))
        * Mat4::from_quat(Quat::from_rotation_y(agent.yaw));
    let stride = if rounded_mesh_cache.is_none() {
        agent.walk_cycle.sin() * 0.5
    } else if agent.moving {
        let amplitude = if agent.sprinting { 0.55 } else { 0.5 };
        agent.walk_cycle.sin() * amplitude
    } else {
        0.0
    };
    let bob = if agent.position[1] <= 0.01 {
        agent.walk_cycle.sin().abs() * 0.025
    } else {
        0.0
    };

    let mut part = |position: Vec3, size: Vec3, pitch: f32, color: [f32; 4]| {
        let transform =
            root * Mat4::from_translation(position) * Mat4::from_quat(Quat::from_rotation_x(pitch));
        if let Some(cache) = rounded_mesh_cache.as_deref_mut() {
            add_rounded_transformed_cuboid(vertices, cache, transform, size, color);
        } else {
            add_transformed_cuboid(vertices, transform, size, color);
        }
    };

    part(
        Vec3::new(0.0, 1.82 + bob, 0.0),
        Vec3::new(1.1, 1.25, 0.64),
        0.0,
        style.shirt,
    );
    part(
        Vec3::new(0.0, 3.01, 0.0),
        Vec3::splat(0.84),
        0.0,
        style.skin,
    );
    part(
        Vec3::new(-0.76, 1.84, 0.0),
        Vec3::new(0.36, 1.15, 0.45),
        stride,
        style.shirt,
    );
    part(
        Vec3::new(0.76, 1.84, 0.0),
        Vec3::new(0.36, 1.15, 0.45),
        -stride,
        style.shirt,
    );
    part(
        Vec3::new(-0.28, 0.62, 0.0),
        Vec3::new(0.47, 1.25, 0.55),
        -stride,
        style.pants,
    );
    part(
        Vec3::new(0.28, 0.62, 0.0),
        Vec3::new(0.47, 1.25, 0.55),
        stride,
        style.pants,
    );
    part(
        Vec3::new(-0.28, 0.11, -0.06),
        Vec3::new(0.56, 0.22, 0.7),
        0.0,
        style.shoes,
    );
    part(
        Vec3::new(0.28, 0.11, -0.06),
        Vec3::new(0.56, 0.22, 0.7),
        0.0,
        style.shoes,
    );
    part(
        Vec3::new(-0.16, 3.04, -0.43),
        Vec3::new(0.1, 0.12, 0.03),
        0.0,
        face_color,
    );
    part(
        Vec3::new(0.16, 3.04, -0.43),
        Vec3::new(0.1, 0.12, 0.03),
        0.0,
        face_color,
    );
}

fn add_rounded_transformed_cuboid(
    vertices: &mut Vec<Vertex>,
    cache: &mut rounded_geometry::RoundedMeshCache,
    transform: Mat4,
    size: Vec3,
    color: [f32; 4],
) {
    let radius = (size.min_element() * 0.16).max(0.01);
    let recipe = rounded_geometry::RoundedBoxRecipe::new(
        size,
        radius,
        2,
        rounded_geometry::TaperProfile::default(),
    );
    let Ok(mesh) = cache.get_or_build(recipe) else {
        add_transformed_cuboid(vertices, transform, size, color);
        return;
    };
    let normal_transform = transform.inverse().transpose();
    for &index in &mesh.indices {
        let source = mesh.vertices[index as usize];
        let position = transform.transform_point3(source.position);
        let normal = normal_transform
            .transform_vector3(source.normal)
            .normalize_or_zero();
        vertices.push(Vertex {
            position: position.to_array(),
            normal: normal.to_array(),
            color,
            tex_coords: source.uv,
            image_invert: 0.0,
        });
    }
}
