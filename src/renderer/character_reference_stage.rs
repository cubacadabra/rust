//! Capture-only set dressing using the existing opaque character pipeline.
//! No production lighting, material, or asset-format behavior lives here.
use super::*;
use crate::renderer::rounded_geometry::{RoundedBoxRecipe, TaperProfile};

pub(super) fn add(renderer: &mut CharacterRenderer, device: &wgpu::Device) {
    let mut cache = RoundedMeshCache::new(8);
    // center, dimensions, bevel, linear tint. Platform top touches the soles.
    let blocks = [
        (
            [0., -0.26, 1.],
            [18., 0.16, 18.],
            0.04,
            [0.17, 0.20, 0.28, 1.],
        ),
        (
            [0., 2.2, 4.5],
            [16., 9., 0.3],
            0.05,
            [0.105, 0.135, 0.21, 1.],
        ),
        (
            [0., -0.08, 0.],
            [2.55, 0.214, 2.05],
            0.10,
            [0.10, 0.135, 0.22, 1.],
        ),
        (
            [-1.8, 0.47, 1.3],
            [0.72, 1.30, 0.8],
            0.05,
            [0.13, 0.18, 0.29, 1.],
        ),
        (
            [-2.3, 1.3, 2.7],
            [0.88, 2.96, 0.9],
            0.06,
            [0.11, 0.15, 0.24, 1.],
        ),
        (
            [1.6, 0.22, 1.8],
            [0.95, 0.8, 0.9],
            0.05,
            [0.17, 0.21, 0.32, 1.],
        ),
        (
            [1.95, 1.6, 3.],
            [0.82, 3.56, 0.9],
            0.05,
            [0.12, 0.155, 0.25, 1.],
        ),
    ];
    for (center, size, radius, tint) in blocks {
        let geometry = cache
            .get_or_build(RoundedBoxRecipe::new(
                Vec3::from_array(size),
                radius,
                4,
                TaperProfile::default(),
            ))
            .unwrap();
        let vertices: Vec<_> = geometry
            .vertices
            .iter()
            .map(|v| CharacterVertex {
                position: v.position.to_array(),
                normal: v.normal.to_array(),
                uv: v.uv,
            })
            .collect();
        let mesh = renderer.meshes.len();
        renderer.meshes.push(Mesh {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("reference stage vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("reference stage indices"),
                contents: bytemuck::cast_slice(&geometry.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: geometry.indices.len() as u32,
            surfaces: Vec::new(),
        });
        renderer.batches.push(Batch {
            mesh,
            material: Material::Rubber,
            start: 0,
            instances: vec![CharacterInstance::new(
                Mat4::from_translation(Vec3::from_array(center)),
                tint,
                Material::Rubber,
            )],
        });
    }
}
