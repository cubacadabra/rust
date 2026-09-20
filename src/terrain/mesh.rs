use super::math::{cross, dot, normalize, subtract, trilinear};
use super::{CHUNK_CELLS, TerrainChunk, TerrainGrid};

#[derive(Clone, Copy, Debug)]
pub(crate) struct TerrainVertex {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) material: u8,
}

impl TerrainGrid {
    pub(crate) fn for_each_chunk_triangle(
        &self,
        mut visit: impl FnMut([i32; 3], [TerrainVertex; 3]),
    ) {
        const CUBE_CORNERS: [[usize; 3]; 8] = [
            [0, 0, 0],
            [1, 0, 0],
            [0, 1, 0],
            [1, 1, 0],
            [0, 0, 1],
            [1, 0, 1],
            [0, 1, 1],
            [1, 1, 1],
        ];
        const TETRAHEDRA: [[usize; 4]; 6] = [
            [0, 1, 3, 7],
            [0, 3, 2, 7],
            [0, 2, 6, 7],
            [0, 6, 4, 7],
            [0, 4, 5, 7],
            [0, 5, 1, 7],
        ];
        const TETRA_EDGES: [[usize; 2]; 6] = [[0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3]];

        for chunk in self.chunks.values() {
            for z in 0..CHUNK_CELLS {
                for y in 0..CHUNK_CELLS {
                    for x in 0..CHUNK_CELLS {
                        let mut positions = [[0.0; 3]; 8];
                        let mut distances = [0.0; 8];
                        let mut materials = [0; 8];
                        let mut negative = false;
                        let mut positive = false;
                        for (index, [dx, dy, dz]) in CUBE_CORNERS.iter().enumerate() {
                            let sx = x + dx;
                            let sy = y + dy;
                            let sz = z + dz;
                            let sample = TerrainChunk::sample_index(sx, sy, sz);
                            distances[index] = chunk.distances[sample];
                            materials[index] = chunk.materials[sample];
                            negative |= distances[index] < 0.0;
                            positive |= distances[index] >= 0.0;
                            positions[index] = [
                                (chunk.coordinate[0] * CHUNK_CELLS as i32 + sx as i32) as f32
                                    * self.cell_size,
                                (chunk.coordinate[1] * CHUNK_CELLS as i32 + sy as i32) as f32
                                    * self.cell_size,
                                (chunk.coordinate[2] * CHUNK_CELLS as i32 + sz as i32) as f32
                                    * self.cell_size,
                            ];
                        }
                        if !negative || !positive {
                            continue;
                        }
                        for tetra in TETRAHEDRA {
                            let mut intersections = [None; 4];
                            let mut count = 0;
                            for [edge_a, edge_b] in TETRA_EDGES {
                                let a = tetra[edge_a];
                                let b = tetra[edge_b];
                                if (distances[a] < 0.0) == (distances[b] < 0.0) {
                                    continue;
                                }
                                intersections[count] = Some(interpolate_vertex(
                                    positions[a],
                                    positions[b],
                                    distances[a],
                                    distances[b],
                                    materials[a],
                                    materials[b],
                                    positions[0],
                                    distances,
                                    self.cell_size,
                                ));
                                count += 1;
                            }
                            if count >= 3 {
                                emit_polygon(intersections, count, chunk.coordinate, &mut visit);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn interpolate_vertex(
    a: [f32; 3],
    b: [f32; 3],
    distance_a: f32,
    distance_b: f32,
    material_a: u8,
    material_b: u8,
    cell_origin: [f32; 3],
    cube_distances: [f32; 8],
    cell_size: f32,
) -> TerrainVertex {
    let amount = (distance_a / (distance_a - distance_b)).clamp(0.0, 1.0);
    let position = std::array::from_fn(|axis| a[axis] + (b[axis] - a[axis]) * amount);
    let local = [
        (position[0] - cell_origin[0]) / cell_size,
        (position[1] - cell_origin[1]) / cell_size,
        (position[2] - cell_origin[2]) / cell_size,
    ];
    let epsilon = 0.01;
    let mut normal = [0.0; 3];
    for axis in 0..3 {
        let mut before = local;
        let mut after = local;
        before[axis] = (before[axis] - epsilon).clamp(0.0, 1.0);
        after[axis] = (after[axis] + epsilon).clamp(0.0, 1.0);
        let span = (after[axis] - before[axis]).max(0.0001) * cell_size;
        normal[axis] =
            (trilinear(cube_distances, after) - trilinear(cube_distances, before)) / span;
    }
    TerrainVertex {
        position,
        normal: normalize(normal),
        material: if distance_a < 0.0 {
            material_a
        } else {
            material_b
        },
    }
}

fn emit_polygon(
    mut vertices: [Option<TerrainVertex>; 4],
    count: usize,
    chunk: [i32; 3],
    visit: &mut impl FnMut([i32; 3], [TerrainVertex; 3]),
) {
    let mut polygon = vertices[..count]
        .iter_mut()
        .filter_map(Option::take)
        .collect::<Vec<_>>();
    if polygon.len() < 3 {
        return;
    }
    let center = polygon.iter().fold([0.0; 3], |mut sum, vertex| {
        for axis in 0..3 {
            sum[axis] += vertex.position[axis] / polygon.len() as f32;
        }
        sum
    });
    let normal = normalize(polygon.iter().fold([0.0; 3], |mut sum, vertex| {
        for axis in 0..3 {
            sum[axis] += vertex.normal[axis];
        }
        sum
    }));
    let reference = if normal[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let tangent = normalize(cross(reference, normal));
    let bitangent = cross(normal, tangent);
    polygon.sort_by(|left, right| {
        let angle = |vertex: &TerrainVertex| {
            let offset =
                std::array::from_fn::<_, 3, _>(|axis| vertex.position[axis] - center[axis]);
            dot(offset, bitangent).atan2(dot(offset, tangent))
        };
        angle(left).total_cmp(&angle(right))
    });
    let orientation = cross(
        subtract(polygon[1].position, polygon[0].position),
        subtract(polygon[2].position, polygon[0].position),
    );
    if dot(orientation, normal) < 0.0 {
        polygon.reverse();
    }
    for index in 1..polygon.len() - 1 {
        visit(chunk, [polygon[0], polygon[index], polygon[index + 1]]);
    }
}
