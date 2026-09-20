use std::collections::HashMap;

use super::super::{Renderer, TerrainRenderChunk, Vertex};
use crate::terrain::TerrainVertex;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct TerrainVertexKey {
    position: [u32; 3],
    normal: [u32; 3],
    material: u8,
}

#[derive(Default)]
pub struct TerrainMeshBuilder {
    pub(super) vertices: Vec<Vertex>,
    pub(super) indices: Vec<u32>,
    vertex_indices: HashMap<TerrainVertexKey, u32>,
}

impl TerrainMeshBuilder {
    pub(super) fn push_triangle(&mut self, triangle: [TerrainVertex; 3], material_art: bool) {
        for terrain_vertex in triangle {
            let key = TerrainVertexKey {
                position: terrain_vertex.position.map(f32::to_bits),
                normal: terrain_vertex.normal.map(f32::to_bits),
                material: terrain_vertex.material,
            };
            let index = if let Some(index) = self.vertex_indices.get(&key) {
                *index
            } else {
                let index = u32::try_from(self.vertices.len())
                    .expect("a terrain chunk mesh must fit in u32 vertex indices");
                self.vertices.push(Vertex {
                    position: terrain_vertex.position,
                    normal: terrain_vertex.normal,
                    color: [1.0; 4],
                    tex_coords: [
                        terrain_vertex.material as f32,
                        if material_art { 1.0 } else { 0.0 },
                    ],
                    image_invert: 2.0,
                    texture_bounds: [0.0, 0.0, 1.0, 1.0],
                });
                self.vertex_indices.insert(key, index);
                index
            };
            self.indices.push(index);
        }
    }
}

impl Renderer {
    pub(super) fn build_terrain_meshes(&self) -> Vec<TerrainMeshBuilder> {
        let Some(terrain) = &self.scene.world.terrain else {
            return Vec::new();
        };
        let mut chunks = std::collections::BTreeMap::<[i32; 3], TerrainMeshBuilder>::new();
        terrain.for_each_chunk_triangle(|coordinate, triangle| {
            chunks
                .entry(coordinate)
                .or_default()
                .push_triangle(triangle, self.scene.world.terrain_material_art);
        });
        chunks.into_values().collect()
    }

    pub(super) fn upload_terrain_meshes(
        &self,
        meshes: Vec<TerrainMeshBuilder>,
    ) -> Vec<TerrainRenderChunk> {
        let mut uploaded = Vec::with_capacity(meshes.len());
        for mesh in meshes {
            if mesh.indices.is_empty() {
                continue;
            }
            let vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("cubacadabra terrain chunk vertices"),
                size: std::mem::size_of_val(mesh.vertices.as_slice()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&mesh.vertices));
            let index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("cubacadabra terrain chunk indices"),
                size: std::mem::size_of_val(mesh.indices.as_slice()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&index_buffer, 0, bytemuck::cast_slice(&mesh.indices));
            uploaded.push(TerrainRenderChunk {
                vertex_buffer,
                index_buffer,
                index_count: mesh.indices.len() as u32,
            });
        }
        uploaded
    }
}
