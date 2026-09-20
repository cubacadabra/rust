use super::character_material::CharacterPass;
use super::character_quality;
#[cfg(feature = "studio-ui")]
use glam::Vec4;
use glam::{Mat4, Vec3};
#[cfg(test)]
use std::collections::BTreeMap;

use super::CharacterRenderMode;
#[cfg(debug_assertions)]
use super::add_floor_pixel_text;
use super::{
    Globals, RenderEntity, Renderer, Vertex, add_billboard, add_cloud, add_cuboid,
    add_cuboid_outline, add_decoration, add_ladder, add_launch_pad, add_pixel_text, add_spawn_pad,
    add_textured_cuboid, faded,
};
#[cfg(test)]
use crate::terrain::TerrainVertex;

mod terrain {
    include!("draw/terrain.rs");
}
mod frame {
    include!("draw/frame.rs");
}

#[cfg(test)]
use terrain::TerrainMeshBuilder;
mod camera;
mod geometry;

fn shadow_light_direction(sun_direction: [f32; 3]) -> Vec3 {
    let light_direction = (-Vec3::from_array(sun_direction)).normalize_or_zero();
    if light_direction.length_squared() > 0.001 {
        light_direction
    } else {
        Vec3::new(0.45, 0.82, -0.32).normalize()
    }
}

fn screen_sun(
    view_projection: Mat4,
    camera: Vec3,
    sun_direction: [f32; 3],
    viewport: (f32, f32, f32, f32),
    output: (f32, f32),
) -> ([f32; 4], [f32; 4]) {
    let toward_sun = shadow_light_direction(sun_direction);
    let clip = view_projection * (camera + toward_sun * 180.0).extend(1.0);
    let ndc = (clip.w.abs() > 0.0001)
        .then(|| clip.truncate() / clip.w)
        .unwrap_or(Vec3::ZERO);
    let visible = f32::from(
        clip.w > 0.0001
            && ndc.x.abs() < 1.08
            && ndc.y.abs() < 1.08
            && ndc.z >= 0.0
            && ndc.z <= 1.0
            && toward_sun.y > -0.08,
    );
    let pixel = [
        viewport.0 + (ndc.x * 0.5 + 0.5) * viewport.2,
        viewport.1 + (0.5 - ndc.y * 0.5) * viewport.3,
    ];
    let normalized = [
        pixel[0] / output.0.max(1.0),
        pixel[1] / output.1.max(1.0),
        visible,
        0.0,
    ];
    ([pixel[0], pixel[1], toward_sun.y, visible], normalized)
}

fn vertex_capacity_for(required: usize, max_buffer_size: u64) -> Option<usize> {
    if required == 0 {
        return Some(0);
    }
    let vertex_size = std::mem::size_of::<Vertex>() as u64;
    let required_bytes = (required as u64).checked_mul(vertex_size)?;
    if required_bytes > max_buffer_size {
        return None;
    }

    let rounded = required.checked_next_power_of_two().unwrap_or(required);
    let rounded_bytes = (rounded as u64).checked_mul(vertex_size)?;
    Some(if rounded_bytes <= max_buffer_size {
        rounded
    } else {
        // Power-of-two growth is only an optimization. Near the device limit,
        // allocate the exact mesh size rather than crossing wgpu's hard cap.
        required
    })
}

impl Renderer {
    pub fn draw(&mut self) {
        let Some((frame, mut encoder, view)) = self.encode_frame(false) else {
            return;
        };
        self.presenter.draw(&mut encoder, &self.targets, &view);
        self.queue.submit(Some(encoder.finish()));
        frame.expect("presented frame").present();
    }

    pub(crate) fn draw_with_overlay<F>(&mut self, overlay: F)
    where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    {
        let Some((frame, mut encoder, view)) = self.encode_frame(false) else {
            return;
        };
        overlay(&self.device, &self.queue, &mut encoder, &self.targets.color);
        self.presenter.draw(&mut encoder, &self.targets, &view);
        self.queue.submit(Some(encoder.finish()));
        frame.expect("presented frame").present();
    }

    /// Runs the production scene and overlay passes into the app-owned target
    /// when a development capture cannot acquire an on-screen drawable.
    #[cfg(all(feature = "studio-ui", debug_assertions))]
    pub(crate) fn capture_studio_frame<F>(&mut self, overlay: F)
    where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    {
        let Some((_, mut encoder, _)) = self.encode_frame(true) else {
            return;
        };
        overlay(&self.device, &self.queue, &mut encoder, &self.targets.color);
        self.queue.submit(Some(encoder.finish()));
    }

    fn review_fog_range(&self) -> (f32, f32) {
        #[cfg(feature = "studio-ui")]
        if self.studio_camera_preset != crate::StudioCameraPreset::Gameplay {
            return (1_000_000.0, 1_000_001.0);
        }
        (self.scene.world.fog_start, self.scene.world.fog_end)
    }

    /// Embedded Studio views fill their editor pane. Player hosts retain their
    /// landscape composition when a window becomes portrait-ish; the UI pass
    /// still covers the full scene so touch controls can adapt.
    fn world_viewport(&self) -> (f32, f32, f32, f32) {
        const LANDSCAPE_ASPECT: f32 = 16.0 / 9.0;
        #[cfg(feature = "studio-ui")]
        if self.studio_viewport.is_some() {
            return self.output_viewport();
        }
        #[cfg(feature = "studio-ui")]
        let (x, y, width, height) = self.output_viewport();
        #[cfg(not(feature = "studio-ui"))]
        let (x, y, width, height) = (0.0, 0.0, self.width.max(1.0), self.height.max(1.0));
        let aspect = width / height;
        if aspect >= 1.25 {
            return (x, y, width, height);
        }
        let viewport_height = (width / LANDSCAPE_ASPECT).min(height);
        (
            x,
            y + (height - viewport_height) * 0.5,
            width,
            viewport_height,
        )
    }

    fn shadow_view_projection(&self, center: Vec3) -> Mat4 {
        // Keep one stable, player-centered orthographic cascade for the first
        // shadow milestone. It covers the playable maze and nearby dressing
        // on mobile/WebGL without requiring a second shadow cascade.
        Self::shadow_view_projection_for(center, self.scene.world.sun_direction)
    }

    fn shadow_view_projection_for(center: Vec3, sun_direction: [f32; 3]) -> Mat4 {
        let light_direction = shadow_light_direction(sun_direction);
        let up = if light_direction.dot(Vec3::Y).abs() > 0.92 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        // Keep the light view itself fixed. Only translate its orthographic
        // projection in light space, then snap that translation to whole shadow
        // texels. Rebuilding look_at around `center` would reintroduce swimming.
        let base_view = Mat4::look_at_rh(light_direction * 180.0, Vec3::ZERO, up);
        let light_space_center = base_view.transform_point3(center);
        let texel_world_size = 240.0 / crate::renderer::device::SHADOW_MAP_SIZE as f32;
        let snapped_center = Vec3::new(
            (light_space_center.x / texel_world_size).round() * texel_world_size,
            (light_space_center.y / texel_world_size).round() * texel_world_size,
            light_space_center.z,
        );
        // The translation must be derived only from the quantized center. Using
        // `snapped_center - light_space_center` would still track sub-texel
        // movement and merely hide the swimming behind a rounded target.
        let snapped_view =
            Mat4::from_translation(Vec3::new(-snapped_center.x, -snapped_center.y, 0.0))
                * base_view;
        Mat4::orthographic_rh(-120.0, 120.0, -120.0, 120.0, 0.1, 420.0) * snapped_view
    }

    #[cfg(feature = "studio-ui")]
    fn output_viewport(&self) -> (f32, f32, f32, f32) {
        if let Some([x, y, width, height]) = self.studio_viewport {
            return (x, y, width, height);
        }
        (0.0, 0.0, self.width.max(1.0), self.height.max(1.0))
    }
}

fn project_world_label_to_ui(
    position: Vec3,
    view_projection: Mat4,
    world_viewport: (f32, f32, f32, f32),
    output_viewport: (f32, f32, f32, f32),
    ui_size: (f32, f32),
) -> Option<(f32, f32)> {
    if output_viewport.2 <= 0.0 || output_viewport.3 <= 0.0 || ui_size.0 <= 0.0 || ui_size.1 <= 0.0
    {
        return None;
    }
    let clip = view_projection * position.extend(1.0);
    if !clip.is_finite() || clip.w <= 0.01 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !ndc.is_finite()
        || ndc.z < 0.0
        || ndc.z > 1.0
        || !(-1.0..=1.0).contains(&ndc.x)
        || !(-1.0..=1.0).contains(&ndc.y)
    {
        return None;
    }

    let surface_x = world_viewport.0 + (ndc.x + 1.0) * 0.5 * world_viewport.2;
    let surface_y = world_viewport.1 + (1.0 - (ndc.y + 1.0) * 0.5) * world_viewport.3;
    Some((
        (surface_x - output_viewport.0) * ui_size.0 / output_viewport.2,
        (surface_y - output_viewport.1) * ui_size.1 / output_viewport.3,
    ))
}

fn support_receiver(_world: &super::RenderWorld, entity: RenderEntity) -> Option<(f32, f32)> {
    match entity.support {
        crate::types::CharacterSupport::Grounded { height } if height.is_finite() => {
            Some((height, 0.18))
        }
        crate::types::CharacterSupport::Unknown
            if entity.position[1].is_finite() && entity.position[1].abs() <= 0.08 =>
        {
            // Legacy remotes do not report support. The ground fallback is
            // intentionally faint and is omitted at any raised height.
            Some((0.0, 0.08))
        }
        _ => None,
    }
}

pub(super) fn split_world_vertices(
    source: &[Vertex],
    opaque: &mut Vec<Vertex>,
    translucent: &mut Vec<Vertex>,
) {
    for triangle in source.chunks_exact(3) {
        if triangle.iter().all(|vertex| vertex.color[3] >= 1.0) {
            opaque.extend_from_slice(triangle);
        } else {
            translucent.extend_from_slice(triangle);
        }
    }
}

pub(super) fn sort_translucent(vertices: &mut [Vertex], camera: Vec3, target: Vec3) {
    let forward = (target - camera).normalize_or_zero();
    let depth = |triangle: &[Vertex; 3]| {
        triangle
            .iter()
            .map(|v| (Vec3::from_array(v.position) - camera).dot(forward))
            .sum::<f32>()
    };
    vertices
        .as_chunks_mut::<3>()
        .0
        .sort_unstable_by(|a, b| depth(b).total_cmp(&depth(a)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain_vertex(position: [f32; 3]) -> TerrainVertex {
        TerrainVertex {
            position,
            normal: [0.0, 1.0, 0.0],
            material: 1,
        }
    }

    #[test]
    fn terrain_chunk_mesh_reuses_vertices_and_keeps_triangle_indices() {
        let a = terrain_vertex([0.0, 0.0, 0.0]);
        let b = terrain_vertex([1.0, 0.0, 0.0]);
        let c = terrain_vertex([0.0, 0.0, 1.0]);
        let d = terrain_vertex([1.0, 0.0, 1.0]);
        let mut mesh = TerrainMeshBuilder::default();

        mesh.push_triangle([a, b, c], true);
        mesh.push_triangle([b, d, c], true);

        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices, [0, 1, 2, 1, 3, 2]);
    }

    #[test]
    fn terrain_surface_is_built_as_indexed_chunk_meshes() {
        let definition: crate::terrain::TerrainDefinition = serde_json::from_str(
            r#"{
                "cellSize":0.5,
                "operations":[{
                    "shape":"block","operation":"fill","position":[0,0,0],
                    "size":[8,8,8],"material":"grass"
                }]
            }"#,
        )
        .unwrap();
        let terrain = crate::terrain::TerrainGrid::build(&definition)
            .unwrap()
            .unwrap();
        let mut chunks = BTreeMap::<[i32; 3], TerrainMeshBuilder>::new();
        terrain.for_each_chunk_triangle(|coordinate, triangle| {
            chunks
                .entry(coordinate)
                .or_default()
                .push_triangle(triangle, true);
        });

        let indexed_vertices = chunks
            .values()
            .map(|chunk| chunk.vertices.len())
            .sum::<usize>();
        let index_count = chunks
            .values()
            .map(|chunk| chunk.indices.len())
            .sum::<usize>();
        assert!(!chunks.is_empty());
        assert!(index_count > 0 && index_count % 3 == 0);
        assert!(indexed_vertices < index_count);
        let indexed_bytes = indexed_vertices * std::mem::size_of::<Vertex>()
            + index_count * std::mem::size_of::<u32>();
        let triangle_soup_bytes = index_count * std::mem::size_of::<Vertex>();
        assert!(indexed_bytes < triangle_soup_bytes);
    }

    #[test]
    fn vertex_capacity_does_not_round_past_device_limit() {
        let max_buffer_size = 256 * 1024 * 1024;
        // Maze 101 currently emits this many terrain vertices. Rounding its
        // capacity to 2^22 used to request 285,212,672 bytes and panic.
        let maze_vertices = 2_511_408;
        assert_eq!(
            vertex_capacity_for(maze_vertices, max_buffer_size),
            Some(maze_vertices)
        );
        assert_eq!(maze_vertices * std::mem::size_of::<Vertex>(), 170_775_744);
        assert_eq!(vertex_capacity_for(100, max_buffer_size), Some(128));
        assert_eq!(vertex_capacity_for(4_000_000, max_buffer_size), None);
    }

    #[test]
    fn world_labels_convert_render_pixels_to_ui_points_and_cull_offscreen() {
        let viewport = (0.0, 0.0, 2_000.0, 1_000.0);
        let center = project_world_label_to_ui(
            Vec3::ZERO,
            Mat4::IDENTITY,
            viewport,
            viewport,
            (1_000.0, 500.0),
        );
        assert_eq!(center, Some((500.0, 250.0)));
        assert!(
            project_world_label_to_ui(
                Vec3::new(1.01, 0.0, 0.0),
                Mat4::IDENTITY,
                viewport,
                viewport,
                (1_000.0, 500.0),
            )
            .is_none()
        );
    }

    #[test]
    fn world_labels_are_local_to_an_embedded_output_viewport() {
        let output = (240.0, 80.0, 1_000.0, 600.0);
        assert_eq!(
            project_world_label_to_ui(Vec3::ZERO, Mat4::IDENTITY, output, output, (1_000.0, 600.0),),
            Some((500.0, 300.0))
        );
    }

    #[test]
    fn world_alpha_is_separated_and_sorted_back_to_front() {
        let triangle = |z, alpha| {
            [Vertex {
                position: [0.0, 0.0, z],
                normal: [0.0, 1.0, 0.0],
                color: [1.0, 1.0, 1.0, alpha],
                tex_coords: [0.0; 2],
                image_invert: 0.0,
                texture_bounds: [0.0, 0.0, 1.0, 1.0],
            }; 3]
        };
        let mut source = Vec::new();
        source.extend(triangle(-2.0, 0.4));
        source.extend(triangle(-1.0, 1.0));
        source.extend(triangle(-5.0, 0.5));
        let (mut opaque, mut alpha) = (Vec::new(), Vec::new());
        split_world_vertices(&source, &mut opaque, &mut alpha);
        sort_translucent(&mut alpha, Vec3::ZERO, -Vec3::Z);
        assert_eq!(opaque.len(), 3);
        assert_eq!(alpha.len(), 6);
        assert_eq!(alpha[0].position[2], -5.0);
    }

    #[test]
    fn support_shadow_fallback_is_conservative() {
        let mut entity = RenderEntity::default();
        entity.support = crate::types::CharacterSupport::Grounded { height: 2.0 };
        assert_eq!(
            support_receiver(&crate::renderer::RenderWorld::default(), entity),
            Some((2.0, 0.18))
        );
        entity.support = crate::types::CharacterSupport::Airborne;
        assert_eq!(
            support_receiver(&crate::renderer::RenderWorld::default(), entity),
            None
        );
        entity.support = crate::types::CharacterSupport::Unknown;
        entity.position[1] = 2.0;
        assert_eq!(
            support_receiver(&crate::renderer::RenderWorld::default(), entity),
            None
        );
    }

    #[test]
    fn shadow_camera_is_above_the_maze_and_light_space_center_snaps() {
        let sun = [-0.45, -0.82, 0.32];
        let light = shadow_light_direction(sun);
        let eye = Vec3::ZERO + light * 180.0;
        assert!(eye.y > 0.0, "shadow camera must sit toward the sun");

        let first = Renderer::shadow_view_projection_for(Vec3::ZERO, sun);
        let texel = 240.0 / crate::renderer::device::SHADOW_MAP_SIZE as f32;
        let view = Mat4::look_at_rh(light * 180.0, Vec3::ZERO, Vec3::Y);
        let sub_texel_world =
            view.inverse()
                .transform_vector3(Vec3::new(texel * 0.40, texel * 0.30, 0.0));
        let sub_texel = Renderer::shadow_view_projection_for(sub_texel_world, sun);
        assert_eq!(
            first, sub_texel,
            "sub-texel motion must not move the shadow map"
        );

        let light_space_step = view
            .inverse()
            .transform_vector3(Vec3::new(texel * 0.60, 0.0, 0.0));
        let moved = Renderer::shadow_view_projection_for(light_space_step, sun);
        assert_ne!(
            first, moved,
            "crossing a shadow texel must update the projection"
        );
    }
}
