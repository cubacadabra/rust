//! Sparse, package-authored solid terrain shared by rendering and collision.
//!
//! The package stores ordered CSG operations. Loading rasterizes their signed
//! distance field into sparse chunks; the same samples drive surface meshing
//! and the character controller's terrain queries.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

pub(crate) const CHUNK_CELLS: usize = 16;
pub(crate) const MAX_TERRAIN_OPERATIONS: usize = 512;
pub(crate) const MAX_TERRAIN_CHUNKS: usize = 512;
const SAMPLE_EDGE: usize = CHUNK_CELLS + 1;
const SAMPLE_COUNT: usize = SAMPLE_EDGE * SAMPLE_EDGE * SAMPLE_EDGE;
const MAX_TERRAIN_SAMPLES: usize = 2_000_000;
const MAX_TERRAIN_COORDINATE: f32 = 4096.0;

fn default_cell_size() -> f32 {
    0.5
}

fn default_shape() -> String {
    "block".to_owned()
}

fn default_operation() -> String {
    "fill".to_owned()
}

fn default_hide_default_ground() -> bool {
    false
}

fn default_material_art() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerrainDefinition {
    #[serde(default = "default_cell_size")]
    pub(crate) cell_size: f32,
    #[serde(default = "default_hide_default_ground")]
    pub(crate) hide_default_ground: bool,
    #[serde(default = "default_material_art")]
    pub(crate) material_art: bool,
    #[serde(default)]
    pub(crate) operations: Vec<TerrainOperationDefinition>,
}

impl Default for TerrainDefinition {
    fn default() -> Self {
        Self {
            cell_size: default_cell_size(),
            hide_default_ground: false,
            material_art: true,
            operations: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerrainOperationDefinition {
    #[serde(default = "default_shape")]
    shape: String,
    #[serde(default = "default_operation")]
    operation: String,
    #[serde(default)]
    position: Vec<f32>,
    #[serde(default)]
    size: Vec<f32>,
    #[serde(default)]
    radius: f32,
    #[serde(default)]
    material: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum TerrainMaterial {
    Grass = 1,
    Ground = 2,
    Rock = 3,
    Sand = 4,
    Mud = 5,
    Snow = 6,
}

impl TerrainMaterial {
    fn parse(name: &str) -> Option<Self> {
        match name
            .strip_prefix("builtin:")
            .unwrap_or(name)
            .to_ascii_lowercase()
            .as_str()
        {
            "grass" => Some(Self::Grass),
            "ground" | "dirt" => Some(Self::Ground),
            "rock" => Some(Self::Rock),
            "sand" => Some(Self::Sand),
            "mud" => Some(Self::Mud),
            "snow" => Some(Self::Snow),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum TerrainOperationKind {
    Fill(TerrainMaterial),
    Carve,
    Paint(TerrainMaterial),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TerrainShape {
    Block,
    Ball,
}

#[derive(Clone, Copy, Debug)]
struct TerrainOperation {
    kind: TerrainOperationKind,
    shape: TerrainShape,
    position: [f32; 3],
    size: [f32; 3],
    radius: f32,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
}

impl TerrainOperation {
    fn parse(definition: &TerrainOperationDefinition) -> Result<Self, String> {
        if definition.position.len() != 3
            || definition.position.iter().any(|value| !value.is_finite())
        {
            return Err("terrain operation position must contain three finite numbers".into());
        }
        let position = [
            definition.position[0],
            definition.position[1],
            definition.position[2],
        ];
        if position
            .iter()
            .any(|value| value.abs() > MAX_TERRAIN_COORDINATE)
        {
            return Err("terrain operation position is outside supported world bounds".into());
        }
        let shape = match definition.shape.to_ascii_lowercase().as_str() {
            "block" | "box" => TerrainShape::Block,
            "ball" | "sphere" => TerrainShape::Ball,
            _ => return Err(format!("unsupported terrain shape: {}", definition.shape)),
        };
        let kind = match definition.operation.to_ascii_lowercase().as_str() {
            "fill" => TerrainOperationKind::Fill(
                TerrainMaterial::parse(&definition.material).ok_or_else(|| {
                    format!(
                        "unsupported built-in terrain material: {}",
                        definition.material
                    )
                })?,
            ),
            "carve" => TerrainOperationKind::Carve,
            "paint" => TerrainOperationKind::Paint(
                TerrainMaterial::parse(&definition.material).ok_or_else(|| {
                    format!(
                        "unsupported built-in terrain material: {}",
                        definition.material
                    )
                })?,
            ),
            _ => {
                return Err(format!(
                    "unsupported terrain operation: {}",
                    definition.operation
                ));
            }
        };
        let (size, radius, bounds_min, bounds_max) = match shape {
            TerrainShape::Block => {
                if definition.size.len() != 3
                    || definition
                        .size
                        .iter()
                        .any(|value| !value.is_finite() || *value <= 0.0)
                {
                    return Err(
                        "terrain block size must contain three positive finite numbers".into(),
                    );
                }
                let size = [definition.size[0], definition.size[1], definition.size[2]];
                let half = size.map(|value| value * 0.5);
                let low = std::array::from_fn(|axis| position[axis] - half[axis]);
                let high = std::array::from_fn(|axis| position[axis] + half[axis]);
                (size, 0.0, low, high)
            }
            TerrainShape::Ball => {
                let radius = definition.radius;
                if !radius.is_finite() || radius <= 0.0 {
                    return Err("terrain ball radius must be positive and finite".into());
                }
                let extent = [radius; 3];
                (
                    [0.0; 3],
                    radius,
                    std::array::from_fn(|axis| position[axis] - extent[axis]),
                    std::array::from_fn(|axis| position[axis] + extent[axis]),
                )
            }
        };
        if bounds_min
            .iter()
            .chain(bounds_max.iter())
            .any(|value| !value.is_finite() || value.abs() > MAX_TERRAIN_COORDINATE)
        {
            return Err("terrain operation bounds are outside supported world bounds".into());
        }
        Ok(Self {
            kind,
            shape,
            position,
            size,
            radius,
            bounds_min,
            bounds_max,
        })
    }

    fn signed_distance(&self, point: [f32; 3]) -> f32 {
        match self.shape {
            TerrainShape::Ball => {
                let delta = [
                    point[0] - self.position[0],
                    point[1] - self.position[1],
                    point[2] - self.position[2],
                ];
                (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt()
                    - self.radius
            }
            TerrainShape::Block => {
                let q = std::array::from_fn::<_, 3, _>(|axis| {
                    (point[axis] - self.position[axis]).abs() - self.size[axis] * 0.5
                });
                let outside = [q[0].max(0.0), q[1].max(0.0), q[2].max(0.0)];
                let outside_distance =
                    (outside[0] * outside[0] + outside[1] * outside[1] + outside[2] * outside[2])
                        .sqrt();
                outside_distance + q[0].max(q[1]).max(q[2]).min(0.0)
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TerrainVertex {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) material: u8,
}

#[derive(Clone, Debug)]
struct TerrainChunk {
    coordinate: [i32; 3],
    operations: Vec<usize>,
    distances: Vec<f32>,
    materials: Vec<u8>,
}

impl TerrainChunk {
    fn sample_index(x: usize, y: usize, z: usize) -> usize {
        x + SAMPLE_EDGE * (y + SAMPLE_EDGE * z)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TerrainGrid {
    cell_size: f32,
    chunks: BTreeMap<[i32; 3], TerrainChunk>,
}

impl TerrainGrid {
    /// Returns the conservative world-space bounds of the allocated terrain
    /// volume. Review cameras use this to frame authored terrain without
    /// changing gameplay camera state.
    pub(crate) fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut low = [i32::MAX; 3];
        let mut high = [i32::MIN; 3];
        for coordinate in self.chunks.keys() {
            for axis in 0..3 {
                low[axis] = low[axis].min(coordinate[axis]);
                high[axis] = high[axis].max(coordinate[axis] + 1);
            }
        }
        if low[0] == i32::MAX {
            return None;
        }
        let low = low.map(|value| value as f32 * CHUNK_CELLS as f32 * self.cell_size);
        let high = high.map(|value| value as f32 * CHUNK_CELLS as f32 * self.cell_size);
        Some((low, high))
    }

    pub(crate) fn build(definition: &TerrainDefinition) -> Result<Option<Self>, String> {
        if definition.operations.is_empty() {
            return Ok(None);
        }
        if definition.operations.len() > MAX_TERRAIN_OPERATIONS {
            return Err(format!(
                "terrain cannot contain more than {MAX_TERRAIN_OPERATIONS} operations"
            ));
        }
        let cell_size = definition.cell_size;
        if !cell_size.is_finite() || !(0.5..=8.0).contains(&cell_size) {
            return Err("terrain.cellSize must be finite and between 0.5 and 8".into());
        }
        let operations = definition
            .operations
            .iter()
            .map(TerrainOperation::parse)
            .collect::<Result<Vec<_>, _>>()?;
        for operation in &operations {
            let minimum_feature = match operation.shape {
                TerrainShape::Block => operation.size.into_iter().fold(f32::INFINITY, f32::min),
                TerrainShape::Ball => operation.radius * 2.0,
            };
            if minimum_feature < cell_size {
                return Err(format!(
                    "terrain operation feature size must be at least cellSize ({cell_size})"
                ));
            }
        }

        let mut chunk_coordinates = BTreeSet::new();
        for operation in &operations {
            if !matches!(operation.kind, TerrainOperationKind::Fill(_)) {
                continue;
            }
            let low = operation.bounds_min.map(|value| value - cell_size);
            let high = operation.bounds_max.map(|value| value + cell_size);
            let first = low.map(|value| (value / (cell_size * CHUNK_CELLS as f32)).floor() as i32);
            let end = high.map(|value| (value / (cell_size * CHUNK_CELLS as f32)).ceil() as i32);
            for z in first[2]..end[2] {
                for y in first[1]..end[1] {
                    for x in first[0]..end[0] {
                        chunk_coordinates.insert([x, y, z]);
                        if chunk_coordinates.len() > MAX_TERRAIN_CHUNKS {
                            return Err(format!(
                                "terrain exceeds the {MAX_TERRAIN_CHUNKS}-chunk package limit"
                            ));
                        }
                    }
                }
            }
        }
        let sample_count = chunk_coordinates
            .len()
            .checked_mul(SAMPLE_COUNT)
            .ok_or_else(|| "terrain sample dimensions overflow".to_owned())?;
        if sample_count > MAX_TERRAIN_SAMPLES {
            return Err(format!(
                "terrain exceeds the {MAX_TERRAIN_SAMPLES}-sample package limit"
            ));
        }
        if chunk_coordinates.is_empty() {
            return Err("terrain operations must contain at least one fill".into());
        }

        let mut chunks = BTreeMap::new();
        for coordinate in chunk_coordinates {
            let chunk_low = coordinate
                .map(|value| value as f32 * CHUNK_CELLS as f32 * cell_size - cell_size * 2.0);
            let chunk_high = chunk_low.map(|value| value + (CHUNK_CELLS as f32 + 4.0) * cell_size);
            let relevant = operations
                .iter()
                .enumerate()
                .filter_map(|(index, operation)| {
                    let intersects = (0..3).all(|axis| {
                        operation.bounds_max[axis] >= chunk_low[axis]
                            && operation.bounds_min[axis] <= chunk_high[axis]
                    });
                    intersects.then_some(index)
                })
                .collect::<Vec<_>>();
            let mut chunk = TerrainChunk {
                coordinate,
                operations: relevant,
                distances: vec![1.0e6; SAMPLE_COUNT],
                materials: vec![0; SAMPLE_COUNT],
            };
            for z in 0..SAMPLE_EDGE {
                for y in 0..SAMPLE_EDGE {
                    for x in 0..SAMPLE_EDGE {
                        let global = [
                            coordinate[0] * CHUNK_CELLS as i32 + x as i32,
                            coordinate[1] * CHUNK_CELLS as i32 + y as i32,
                            coordinate[2] * CHUNK_CELLS as i32 + z as i32,
                        ];
                        let point = global.map(|value| value as f32 * cell_size);
                        let (distance, material) =
                            evaluate_field(&operations, &chunk.operations, point, cell_size);
                        let sample = TerrainChunk::sample_index(x, y, z);
                        chunk.distances[sample] = distance;
                        chunk.materials[sample] = material;
                    }
                }
            }
            chunks.insert(coordinate, chunk);
        }
        Ok(Some(Self { cell_size, chunks }))
    }

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

    /// Signed distance in world units. Negative values are inside solid terrain.
    pub(crate) fn signed_distance(&self, position: [f32; 3]) -> f32 {
        let grid = position.map(|value| value / self.cell_size);
        let cell = grid.map(|value| value.floor() as i32);
        let fraction = [
            grid[0] - cell[0] as f32,
            grid[1] - cell[1] as f32,
            grid[2] - cell[2] as f32,
        ];
        let coordinate = cell.map(|value| value.div_euclid(CHUNK_CELLS as i32));
        let Some(chunk) = self.chunks.get(&coordinate) else {
            return 1.0e6;
        };
        let local = cell.map(|value| value.rem_euclid(CHUNK_CELLS as i32) as usize);
        let mut values = [0.0; 8];
        for (index, [dx, dy, dz]) in [
            [0, 0, 0],
            [1, 0, 0],
            [0, 1, 0],
            [1, 1, 0],
            [0, 0, 1],
            [1, 0, 1],
            [0, 1, 1],
            [1, 1, 1],
        ]
        .iter()
        .enumerate()
        {
            values[index] = chunk.distances
                [TerrainChunk::sample_index(local[0] + dx, local[1] + dy, local[2] + dz)];
        }
        trilinear(values, fraction)
    }

    pub(crate) fn surface_normal(&self, position: [f32; 3]) -> [f32; 3] {
        let delta = self.cell_size * 0.25;
        let mut gradient = [0.0; 3];
        for axis in 0..3 {
            let mut before = position;
            let mut after = position;
            before[axis] -= delta;
            after[axis] += delta;
            gradient[axis] = self.signed_distance(after) - self.signed_distance(before);
        }
        normalize(gradient)
    }

    pub(crate) fn capsule_clear(&self, feet: [f32; 3], radius: f32, height: f32) -> bool {
        let bottom = feet[1] + radius;
        let top = feet[1] + height - radius;
        let interval = (self.cell_size * 0.5).min(radius * 0.5).max(0.1);
        let steps = (((top - bottom).max(0.0) / interval).ceil() as usize).max(1);
        (0..=steps).all(|step| {
            let fraction = step as f32 / steps as f32;
            let point = [feet[0], bottom + (top - bottom) * fraction, feet[2]];
            self.signed_distance(point) >= radius - 0.02
        })
    }

    #[cfg(test)]
    pub(crate) fn chunks_len(&self) -> usize {
        self.chunks.len()
    }

    #[cfg(test)]
    fn sample_count(&self) -> usize {
        self.chunks.len() * SAMPLE_COUNT
    }
}

fn evaluate_field(
    operations: &[TerrainOperation],
    operation_indices: &[usize],
    position: [f32; 3],
    cell_size: f32,
) -> (f32, u8) {
    let smoothing = cell_size * 0.55;
    let mut distance = f32::INFINITY;
    let mut material = 0;
    let mut has_fill = false;
    for index in operation_indices {
        let operation = &operations[*index];
        let shape_distance = operation.signed_distance(position);
        match operation.kind {
            TerrainOperationKind::Fill(id) => {
                distance = if has_fill {
                    smooth_min(distance, shape_distance, smoothing)
                } else {
                    shape_distance
                };
                has_fill = true;
                if shape_distance <= smoothing {
                    material = id as u8;
                }
            }
            TerrainOperationKind::Carve if has_fill => {
                distance = smooth_max(distance, -shape_distance, smoothing);
                if shape_distance < 0.0 {
                    material = 0;
                }
            }
            TerrainOperationKind::Paint(id) if has_fill && shape_distance <= 0.0 => {
                material = id as u8;
            }
            _ => {}
        }
    }
    if distance >= 0.0 {
        material = 0;
    }
    (distance, material)
}

fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    let h = ((k - (a - b).abs()).max(0.0)) / k;
    a.min(b) - h * h * k * 0.25
}

fn smooth_max(a: f32, b: f32, k: f32) -> f32 {
    -smooth_min(-a, -b, k)
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

fn trilinear(values: [f32; 8], point: [f32; 3]) -> f32 {
    let [x, y, z] = point;
    let x00 = values[0] + (values[1] - values[0]) * x;
    let x10 = values[2] + (values[3] - values[2]) * x;
    let x01 = values[4] + (values[5] - values[4]) * x;
    let x11 = values[6] + (values[7] - values[6]) * x;
    let y0 = x00 + (x10 - x00) * y;
    let y1 = x01 + (x11 - x01) * y;
    y0 + (y1 - y0) * z
}

fn normalize(vector: [f32; 3]) -> [f32; 3] {
    let length = dot(vector, vector).sqrt();
    if length <= 1.0e-6 {
        [0.0, 1.0, 0.0]
    } else {
        vector.map(|value| value / length)
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn subtract(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(
        kind: &str,
        shape: &str,
        position: [f32; 3],
        size: [f32; 3],
        radius: f32,
        material: &str,
    ) -> TerrainOperationDefinition {
        TerrainOperationDefinition {
            shape: shape.into(),
            operation: kind.into(),
            position: position.to_vec(),
            size: size.to_vec(),
            radius,
            material: material.into(),
        }
    }

    fn terrain(operations: Vec<TerrainOperationDefinition>) -> TerrainGrid {
        TerrainGrid::build(&TerrainDefinition {
            operations,
            ..TerrainDefinition::default()
        })
        .unwrap()
        .unwrap()
    }

    #[test]
    fn material_art_is_optional_and_does_not_change_terrain_storage() {
        assert!(TerrainDefinition::default().material_art);
        let definition: TerrainDefinition = serde_json::from_str(
            r##"{
                "materialArt":false,
                "operations":[{
                    "shape":"block","operation":"fill","position":[0,0,0],
                    "size":[2,2,2],"material":"builtin:grass"
                }]
            }"##,
        )
        .expect("materialArt should be a visual-only terrain option");
        assert!(!definition.material_art);
        let terrain = TerrainGrid::build(&definition)
            .expect("terrain mechanics must work without art")
            .expect("the fill should produce terrain");
        assert!(terrain.signed_distance([0.0; 3]) < 0.0);
    }

    #[test]
    fn sparse_chunks_share_one_signed_field_for_surface_and_collision() {
        let terrain = terrain(vec![operation(
            "fill",
            "block",
            [0.0, -1.0, 0.0],
            [20.0, 2.0, 20.0],
            0.0,
            "builtin:grass",
        )]);
        assert!(terrain.chunks_len() < 40);
        assert!(terrain.sample_count() < 40 * SAMPLE_COUNT);
        assert!(terrain.signed_distance([0.0, -1.0, 0.0]) < 0.0);
        assert!(terrain.signed_distance([0.0, 0.2, 0.0]) > 0.0);
        assert!(terrain.capsule_clear([0.0, 0.0, 0.0], 0.5, 2.0));
        assert!(!terrain.capsule_clear([0.0, -0.4, 0.0], 0.5, 2.0));
    }

    #[test]
    fn carve_and_paint_are_ordered_volume_operations() {
        let terrain = terrain(vec![
            operation("fill", "block", [0.0; 3], [12.0; 3], 0.0, "ground"),
            operation("carve", "ball", [0.0; 3], [0.0; 3], 2.5, "air"),
            operation("paint", "ball", [4.0, 0.0, 0.0], [0.0; 3], 1.5, "rock"),
        ]);
        assert!(terrain.signed_distance([0.0; 3]) > 0.0);
        assert!(terrain.signed_distance([4.0, 0.0, 0.0]) < 0.0);
        let mut triangles = 0;
        terrain.for_each_chunk_triangle(|_, triangle| {
            triangles += 1;
            assert!(
                triangle
                    .iter()
                    .all(|vertex| vertex.normal.iter().all(|v| v.is_finite()))
            );
        });
        assert!(triangles > 0);
    }

    #[test]
    fn rejects_unknown_materials_and_unbounded_chunk_grids() {
        let unknown = TerrainDefinition {
            operations: vec![operation("fill", "block", [0.0; 3], [2.0; 3], 0.0, "lava")],
            ..TerrainDefinition::default()
        };
        assert!(
            TerrainGrid::build(&unknown)
                .unwrap_err()
                .contains("unsupported")
        );

        let too_large = TerrainDefinition {
            operations: vec![operation(
                "fill",
                "block",
                [0.0; 3],
                [4096.0; 3],
                0.0,
                "grass",
            )],
            ..TerrainDefinition::default()
        };
        assert!(
            TerrainGrid::build(&too_large)
                .unwrap_err()
                .contains("chunk")
        );
    }

    #[test]
    fn fill_and_sphere_carve_form_a_walkable_tunnel_across_chunk_edges() {
        let terrain = terrain(vec![
            operation(
                "fill",
                "block",
                [0.0, -2.0, 0.0],
                [64.0, 4.0, 64.0],
                0.0,
                "grass",
            ),
            operation("carve", "ball", [15.5, 0.0, 0.0], [0.0; 3], 2.0, "air"),
        ]);
        assert!(terrain.chunks_len() > 1);
        assert!(terrain.signed_distance([15.5, 0.0, 0.0]) > 0.0);
        assert!(terrain.signed_distance([15.5, -3.9, 0.0]) < 0.0);
        let mut crossing_triangles = 0;
        terrain.for_each_chunk_triangle(|_, triangle| {
            if triangle
                .iter()
                .any(|vertex| (vertex.position[0] - 16.0).abs() < 0.02)
            {
                crossing_triangles += 1;
            }
        });
        assert!(crossing_triangles > 0);
    }

    #[test]
    fn rejects_features_smaller_than_the_voxel_sampling_resolution() {
        let undersampled = TerrainDefinition {
            cell_size: 1.0,
            operations: vec![operation(
                "fill",
                "block",
                [0.0; 3],
                [2.0, 0.5, 2.0],
                0.0,
                "grass",
            )],
            ..TerrainDefinition::default()
        };
        assert!(
            TerrainGrid::build(&undersampled)
                .unwrap_err()
                .contains("cellSize")
        );
    }
}
