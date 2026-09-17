//! Indexed, instanced world meshes loaded from the package's GLB assets.
//!
//! This is deliberately separate from the character/MorphPack renderer. World
//! meshes are reusable scene geometry: a rock, tree, crate, or landmark can be
//! uploaded once and drawn with many independent transforms.

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Quat, Vec3};
use std::collections::BTreeMap;
use wgpu::util::DeviceExt;

const MAX_ASSETS: usize = 64;
const MAX_ASSET_BYTES: usize = 16 * 1024 * 1024;
const MAX_INSTANCES_PER_ASSET: usize = 512;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(super) struct WorldMeshVertex {
    pub(super) position: [f32; 3],
    pub(super) normal: [f32; 3],
    pub(super) uv: [f32; 2],
}

impl WorldMeshVertex {
    pub(super) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
    };
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(super) struct WorldMeshInstance {
    pub(super) transform: [[f32; 4]; 3],
    pub(super) normal: [[f32; 4]; 3],
    pub(super) tint: [f32; 4],
}

impl WorldMeshInstance {
    pub(super) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
            6 => Float32x4, 7 => Float32x4, 8 => Float32x4,
            9 => Float32x4
        ],
    };

    fn new(transform: Mat4, tint: [f32; 4]) -> Self {
        let normal = Mat3::from_mat4(transform).inverse().transpose();
        Self {
            transform: [
                transform.row(0).to_array(),
                transform.row(1).to_array(),
                transform.row(2).to_array(),
            ],
            normal: [
                normal.row(0).extend(0.0).to_array(),
                normal.row(1).extend(0.0).to_array(),
                normal.row(2).extend(0.0).to_array(),
            ],
            tint,
        }
    }
}

struct MeshAsset {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

struct InstanceBatch {
    buffer: wgpu::Buffer,
    count: u32,
}

#[derive(Default)]
pub(crate) struct WorldMeshRegistry {
    assets: BTreeMap<String, MeshAsset>,
    batches: BTreeMap<String, InstanceBatch>,
}

impl WorldMeshRegistry {
    pub(super) fn register(
        &mut self,
        device: &wgpu::Device,
        id: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        if id.is_empty() || id.len() > 64 || self.assets.len() >= MAX_ASSETS {
            return Err("world mesh asset registry is full or the id is invalid".to_owned());
        }
        if bytes.is_empty() || bytes.len() > MAX_ASSET_BYTES {
            return Err(format!(
                "world mesh asset {id:?} exceeds the {} MiB limit",
                MAX_ASSET_BYTES / (1024 * 1024)
            ));
        }

        let gltf = gltf::Gltf::from_slice(bytes)
            .map_err(|error| format!("world mesh asset {id:?} is not valid GLB: {error}"))?;
        let blob = gltf.blob.as_deref().ok_or_else(|| {
            format!("world mesh asset {id:?} must contain an embedded GLB buffer")
        })?;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for mesh in gltf.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| match buffer.source() {
                    gltf::buffer::Source::Bin => Some(blob),
                    gltf::buffer::Source::Uri(_) => None,
                });
                let positions = reader
                    .read_positions()
                    .ok_or_else(|| {
                        format!("world mesh asset {id:?} has a primitive without positions")
                    })?
                    .collect::<Vec<_>>();
                if positions.is_empty() {
                    continue;
                }
                let normals = reader
                    .read_normals()
                    .map(|values| values.collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
                let uvs = reader
                    .read_tex_coords(0)
                    .map(|values| values.into_f32().collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
                if normals.len() != positions.len() || uvs.len() != positions.len() {
                    return Err(format!(
                        "world mesh asset {id:?} has mismatched vertex attribute lengths"
                    ));
                }

                let base = u32::try_from(vertices.len())
                    .map_err(|_| format!("world mesh asset {id:?} has too many vertices"))?;
                vertices.extend(positions.into_iter().zip(normals).zip(uvs).map(
                    |((position, normal), uv)| WorldMeshVertex {
                        position,
                        normal,
                        uv,
                    },
                ));
                if let Some(values) = reader.read_indices() {
                    indices.extend(values.into_u32().map(|index| base + index));
                } else {
                    indices.extend(
                        (0..u32::try_from(vertices.len()).unwrap() - base)
                            .map(|index| base + index),
                    );
                }
            }
        }

        if vertices.is_empty() || indices.is_empty() {
            return Err(format!(
                "world mesh asset {id:?} contains no drawable primitives"
            ));
        }
        if indices
            .iter()
            .any(|index| *index as usize >= vertices.len())
        {
            return Err(format!(
                "world mesh asset {id:?} contains an out-of-range index"
            ));
        }

        let asset = MeshAsset {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cubacadabra world mesh vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cubacadabra world mesh indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: u32::try_from(indices.len())
                .map_err(|_| format!("world mesh asset {id:?} has too many indices"))?,
        };
        self.assets.insert(id.to_owned(), asset);
        self.batches.remove(id);
        Ok(())
    }

    pub(super) fn clear(&mut self) {
        self.assets.clear();
        self.batches.clear();
    }

    pub(super) fn rebuild_instances(
        &mut self,
        device: &wgpu::Device,
        instances: &[crate::renderer::RenderMeshInstance],
    ) {
        self.batches.clear();
        let mut grouped: BTreeMap<&str, Vec<WorldMeshInstance>> = BTreeMap::new();
        for instance in instances {
            if !self.assets.contains_key(&instance.asset) {
                continue;
            }
            let list = grouped.entry(instance.asset.as_str()).or_default();
            if list.len() >= MAX_INSTANCES_PER_ASSET {
                continue;
            }
            let transform = Mat4::from_translation(Vec3::from_array(instance.position))
                * Mat4::from_quat(Quat::from_rotation_y(instance.yaw))
                * Mat4::from_scale(Vec3::splat(instance.scale));
            list.push(WorldMeshInstance::new(transform, instance.color));
        }
        for (id, values) in grouped {
            let count = values.len() as u32;
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cubacadabra world mesh instances"),
                contents: bytemuck::cast_slice(&values),
                usage: wgpu::BufferUsages::VERTEX,
            });
            self.batches
                .insert(id.to_owned(), InstanceBatch { buffer, count });
        }
    }

    pub(super) fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        pipeline: &'a wgpu::RenderPipeline,
    ) {
        pass.set_pipeline(pipeline);
        for (id, batch) in &self.batches {
            let Some(asset) = self.assets.get(id) else {
                continue;
            };
            pass.set_vertex_buffer(0, asset.vertices.slice(..));
            pass.set_vertex_buffer(1, batch.buffer.slice(..));
            pass.set_index_buffer(asset.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..asset.index_count, 0, 0..batch.count);
        }
    }
}
