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
    pub(super) color: [u8; 4],
}

impl WorldMeshVertex {
    pub(super) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 24,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Unorm8x4,
                offset: 32,
                shader_location: 11,
            },
        ],
    };
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(super) struct WorldMeshInstance {
    pub(super) transform: [[f32; 4]; 3],
    pub(super) normal: [[f32; 4]; 3],
    pub(super) tint: [f32; 4],
    pub(super) texture_bounds: [f32; 4],
}

impl WorldMeshInstance {
    pub(super) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
            6 => Float32x4, 7 => Float32x4, 8 => Float32x4,
            9 => Float32x4, 10 => Float32x4
        ],
    };

    fn new(transform: Mat4, tint: [f32; 4], texture_bounds: [f32; 4]) -> Self {
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
            texture_bounds,
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

fn generated_normals(positions: &[[f32; 3]], indices: &[u32]) -> Result<Vec<[f32; 3]>, String> {
    if indices.len() % 3 != 0 {
        return Err("a world mesh primitive without normals must contain triangle indices".into());
    }
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] = *triangle else { unreachable!() };
        let pa = positions.get(a as usize).copied().ok_or_else(|| {
            "a world mesh primitive contains an out-of-range normal index".to_owned()
        })?;
        let pb = positions.get(b as usize).copied().ok_or_else(|| {
            "a world mesh primitive contains an out-of-range normal index".to_owned()
        })?;
        let pc = positions.get(c as usize).copied().ok_or_else(|| {
            "a world mesh primitive contains an out-of-range normal index".to_owned()
        })?;
        let face = (Vec3::from_array(pb) - Vec3::from_array(pa))
            .cross(Vec3::from_array(pc) - Vec3::from_array(pa));
        if !face.is_finite() || face.length_squared() <= 1e-12 {
            return Err(
                "a world mesh primitive contains a degenerate triangle without normals".into(),
            );
        }
        normals[a as usize] += face;
        normals[b as usize] += face;
        normals[c as usize] += face;
    }
    normals
        .into_iter()
        .map(|normal| {
            normal
                .try_normalize()
                .map(|normal| normal.to_array())
                .ok_or_else(|| "a world mesh vertex has no computable normal".to_owned())
        })
        .collect()
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
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    return Err(format!(
                        "world mesh asset {id:?} only supports triangle-list primitives"
                    ));
                }
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
                let primitive_indices = if let Some(values) = reader.read_indices() {
                    values.into_u32().collect::<Vec<_>>()
                } else {
                    if positions.len() % 3 != 0 {
                        return Err(format!(
                            "world mesh asset {id:?} has an unindexed primitive with a non-triangular vertex count"
                        ));
                    }
                    (0..u32::try_from(positions.len())
                        .map_err(|_| format!("world mesh asset {id:?} has too many vertices"))?)
                        .collect()
                };
                let normals = reader
                    .read_normals()
                    .map(|values| values.collect::<Vec<_>>())
                    .map(Ok)
                    .unwrap_or_else(|| generated_normals(&positions, &primitive_indices))?;
                let uvs = reader
                    .read_tex_coords(0)
                    .map(|values| values.into_f32().collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
                let colors = reader
                    .read_colors(0)
                    .map(|values| {
                        values
                            .into_rgba_f32()
                            .map(|value| value.map(color_channel))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|| vec![[u8::MAX; 4]; positions.len()]);
                if normals.len() != positions.len()
                    || uvs.len() != positions.len()
                    || colors.len() != positions.len()
                {
                    return Err(format!(
                        "world mesh asset {id:?} has mismatched vertex attribute lengths"
                    ));
                }

                let base = u32::try_from(vertices.len())
                    .map_err(|_| format!("world mesh asset {id:?} has too many vertices"))?;
                vertices.extend(positions.into_iter().zip(normals).zip(uvs).zip(colors).map(
                    |(((position, normal), uv), color)| WorldMeshVertex {
                        position,
                        normal,
                        uv,
                        color,
                    },
                ));
                indices.extend(primitive_indices.into_iter().map(|index| base + index));
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

    pub(super) fn replace(
        &mut self,
        device: &wgpu::Device,
        models: &[(&str, &[u8])],
        instances: &[crate::renderer::RenderMeshInstance],
        image_regions: &BTreeMap<String, [f32; 4]>,
    ) -> Result<(), String> {
        let mut replacement = Self::default();
        for (id, bytes) in models {
            replacement.register(device, id, bytes)?;
        }
        replacement.rebuild_instances(device, instances, image_regions);
        *self = replacement;
        Ok(())
    }

    pub(super) fn rebuild_instances(
        &mut self,
        device: &wgpu::Device,
        instances: &[crate::renderer::RenderMeshInstance],
        image_regions: &BTreeMap<String, [f32; 4]>,
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
            let texture_bounds = instance
                .texture_image
                .as_ref()
                .and_then(|image| image_regions.get(image))
                .copied()
                .unwrap_or([0.0; 4]);
            list.push(WorldMeshInstance::new(
                transform,
                instance.color,
                texture_bounds,
            ));
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
        shadow_bind_group: &'a wgpu::BindGroup,
    ) {
        pass.set_pipeline(pipeline);
        pass.set_bind_group(3, shadow_bind_group, &[]);
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

    pub(super) fn draw_shadow<'a>(
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

fn color_channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::{color_channel, generated_normals};

    #[test]
    fn generates_normals_for_triangle_geometry() {
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let normals = generated_normals(&positions, &[0, 1, 2]).expect("valid triangle");
        assert_eq!(normals.len(), 3);
        for normal in normals {
            assert_eq!(normal, [0.0, 0.0, 1.0]);
        }
    }

    #[test]
    fn rejects_degenerate_or_out_of_range_triangle_geometry() {
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert!(generated_normals(&positions, &[0, 1]).is_err());
        assert!(generated_normals(&positions, &[0, 1, 3]).is_err());
        assert!(generated_normals(&positions, &[0, 1, 1]).is_err());
    }

    #[test]
    fn packs_gltf_vertex_colors_for_the_gpu() {
        assert_eq!(color_channel(-1.0), 0);
        assert_eq!(color_channel(0.5), 128);
        assert_eq!(color_channel(2.0), 255);
    }
}
