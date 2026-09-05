use glam::Vec3;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

const MAX_SUBDIVISIONS: u32 = 8;
const MAX_DIMENSION: f32 = 256.0;
const MIN_TAPER: f32 = 0.75;
const MAX_TAPER: f32 = 1.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TaperProfile {
    pub(super) bottom: f32,
    pub(super) top: f32,
}

impl Default for TaperProfile {
    fn default() -> Self {
        Self {
            bottom: 1.0,
            top: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RoundedBoxRecipe {
    pub(super) size: Vec3,
    pub(super) radius: f32,
    pub(super) subdivisions: u32,
    pub(super) taper: TaperProfile,
}

impl RoundedBoxRecipe {
    pub(super) const fn new(
        size: Vec3,
        radius: f32,
        subdivisions: u32,
        taper: TaperProfile,
    ) -> Self {
        Self {
            size,
            radius,
            subdivisions,
            taper,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RoundedVertex {
    pub(super) position: Vec3,
    pub(super) normal: Vec3,
    pub(super) uv: [f32; 2],
}

#[derive(Clone, Debug)]
pub(super) struct IndexedMesh {
    pub(super) vertices: Vec<RoundedVertex>,
    pub(super) indices: Vec<u32>,
    pub(super) bounds_min: Vec3,
    pub(super) bounds_max: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RoundedGeometryError {
    NonFiniteSize,
    NonPositiveSize,
    SizeTooLarge,
    NonFiniteRadius,
    NegativeRadius,
    NonFiniteTaper,
    TaperOutOfRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct RoundedBoxKey {
    size: [u32; 3],
    radius: u32,
    subdivisions: u8,
    taper_bottom: u32,
    taper_top: u32,
}

impl RoundedBoxKey {
    fn from_recipe(recipe: NormalizedRecipe) -> Self {
        Self {
            size: recipe.size.to_array().map(quantize),
            radius: quantize(recipe.radius),
            subdivisions: recipe.subdivisions as u8,
            taper_bottom: quantize(recipe.taper.bottom),
            taper_top: quantize(recipe.taper.top),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct NormalizedRecipe {
    size: Vec3,
    radius: f32,
    subdivisions: u32,
    taper: TaperProfile,
}

/// A bounded recipe cache. Meshes are immutable once built, so the draw path
/// can expand a cached indexed mesh into the existing non-indexed buffer while
/// the renderer migration is still in progress.
pub(super) struct RoundedMeshCache {
    meshes: HashMap<RoundedBoxKey, Arc<IndexedMesh>>,
    order: VecDeque<RoundedBoxKey>,
    capacity: usize,
}

impl Default for RoundedMeshCache {
    fn default() -> Self {
        Self::new(64)
    }
}

impl RoundedMeshCache {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            meshes: HashMap::new(),
            order: VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    pub(super) fn get_or_build(
        &mut self,
        recipe: RoundedBoxRecipe,
    ) -> Result<Arc<IndexedMesh>, RoundedGeometryError> {
        let normalized = normalize(recipe)?;
        let key = RoundedBoxKey::from_recipe(normalized);
        if let Some(mesh) = self.meshes.get(&key) {
            return Ok(Arc::clone(mesh));
        }

        let mesh = Arc::new(build_normalized(normalized));
        if self.meshes.len() >= self.capacity && let Some(oldest) = self.order.pop_front() {
            self.meshes.remove(&oldest);
        }
        self.order.push_back(key);
        self.meshes.insert(key, Arc::clone(&mesh));
        Ok(mesh)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.meshes.len()
    }
}

fn normalize(recipe: RoundedBoxRecipe) -> Result<NormalizedRecipe, RoundedGeometryError> {
    if !recipe.size.is_finite() {
        return Err(RoundedGeometryError::NonFiniteSize);
    }
    if recipe.size.min_element() <= 0.0 {
        return Err(RoundedGeometryError::NonPositiveSize);
    }
    if recipe.size.max_element() > MAX_DIMENSION {
        return Err(RoundedGeometryError::SizeTooLarge);
    }
    if !recipe.radius.is_finite() {
        return Err(RoundedGeometryError::NonFiniteRadius);
    }
    if recipe.radius < 0.0 {
        return Err(RoundedGeometryError::NegativeRadius);
    }
    if !recipe.taper.bottom.is_finite() || !recipe.taper.top.is_finite() {
        return Err(RoundedGeometryError::NonFiniteTaper);
    }
    if !(MIN_TAPER..=MAX_TAPER).contains(&recipe.taper.bottom)
        || !(MIN_TAPER..=MAX_TAPER).contains(&recipe.taper.top)
    {
        return Err(RoundedGeometryError::TaperOutOfRange);
    }

    // Keep a small positive planar region even when an author supplies a
    // radius at or above half the shortest dimension.
    let radius_limit = recipe.size.min_element() * 0.5 * (1.0 - f32::EPSILON * 16.0);
    Ok(NormalizedRecipe {
        size: recipe.size,
        radius: recipe.radius.min(radius_limit),
        subdivisions: recipe.subdivisions.clamp(1, MAX_SUBDIVISIONS),
        taper: recipe.taper,
    })
}

fn build_normalized(recipe: NormalizedRecipe) -> IndexedMesh {
    let half = recipe.size * 0.5;
    let mut mesh = IndexedMesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        bounds_min: Vec3::splat(f32::INFINITY),
        bounds_max: Vec3::splat(f32::NEG_INFINITY),
    };

    if recipe.radius == 0.0 {
        add_hard_faces(&mut mesh, half);
    } else {
        // Planar face interiors.
        for axis in 0..3 {
            let tangent = [(axis + 1) % 3, (axis + 2) % 3];
            for sign in [-1.0, 1.0] {
                add_face(
                    &mut mesh,
                    half,
                    recipe.radius,
                    axis,
                    tangent[0],
                    tangent[1],
                    sign,
                );
            }
        }

        // Quarter-cylinder edge strips.
        for first_axis in 0..3 {
            for second_axis in (first_axis + 1)..3 {
                let tangent = 3 - first_axis - second_axis;
                for first_sign in [-1.0, 1.0] {
                    for second_sign in [-1.0, 1.0] {
                        add_edge(
                            &mut mesh,
                            half,
                            recipe.radius,
                            recipe.subdivisions,
                            EdgeSpec {
                                first_axis,
                                second_axis,
                                tangent_axis: tangent,
                                first_sign,
                                second_sign,
                            },
                        );
                    }
                }
            }
        }

        // Eight spherical corner patches close the edge strips without
        // turning the primitive into a stretched unit sphere.
        for x_sign in [-1.0, 1.0] {
            for y_sign in [-1.0, 1.0] {
                for z_sign in [-1.0, 1.0] {
                    add_corner(
                        &mut mesh,
                        half,
                        recipe.radius,
                        recipe.subdivisions,
                        x_sign,
                        y_sign,
                        z_sign,
                    );
                }
            }
        }
    }

    apply_taper(&mut mesh, recipe.size.y, recipe.taper);
    recompute_normals(&mut mesh);
    for vertex in &mesh.vertices {
        mesh.bounds_min = mesh.bounds_min.min(vertex.position);
        mesh.bounds_max = mesh.bounds_max.max(vertex.position);
    }
    mesh
}

fn add_hard_faces(mesh: &mut IndexedMesh, half: Vec3) {
    for axis in 0..3 {
        let tangent = [(axis + 1) % 3, (axis + 2) % 3];
        for sign in [-1.0, 1.0] {
            let mut map = |u: f32, v: f32| {
                let mut point = Vec3::ZERO;
                point[axis] = sign * half[axis];
                point[tangent[0]] = (u * 2.0 - 1.0) * half[tangent[0]];
                point[tangent[1]] = (v * 2.0 - 1.0) * half[tangent[1]];
                (point, Vec3::ZERO, [u, v])
            };
            add_grid(mesh, 1, 1, &mut map);
            let base = mesh.vertices.len() - 4;
            for vertex in &mut mesh.vertices[base..] {
                vertex.normal[axis] = sign;
            }
        }
    }
}

fn add_face(
    mesh: &mut IndexedMesh,
    half: Vec3,
    radius: f32,
    normal_axis: usize,
    u_axis: usize,
    v_axis: usize,
    sign: f32,
) {
    let mut map = |u: f32, v: f32| {
        let mut point = Vec3::ZERO;
        point[normal_axis] = sign * half[normal_axis];
        point[u_axis] = lerp(-half[u_axis] + radius, half[u_axis] - radius, u);
        point[v_axis] = lerp(-half[v_axis] + radius, half[v_axis] - radius, v);
        let mut normal = Vec3::ZERO;
        normal[normal_axis] = sign;
        (point, normal, [u, v])
    };
    add_grid(mesh, 1, 1, &mut map);
}

struct EdgeSpec {
    first_axis: usize,
    second_axis: usize,
    tangent_axis: usize,
    first_sign: f32,
    second_sign: f32,
}

fn add_edge(
    mesh: &mut IndexedMesh,
    half: Vec3,
    radius: f32,
    subdivisions: u32,
    edge: EdgeSpec,
) {
    let mut map = |u: f32, v: f32| {
        let angle = v * std::f32::consts::FRAC_PI_2;
        let mut point = Vec3::ZERO;
        point[edge.tangent_axis] = lerp(
            -half[edge.tangent_axis] + radius,
            half[edge.tangent_axis] - radius,
            u,
        );
        point[edge.first_axis] =
            edge.first_sign * (half[edge.first_axis] - radius + radius * angle.cos());
        point[edge.second_axis] =
            edge.second_sign * (half[edge.second_axis] - radius + radius * angle.sin());
        let mut normal = Vec3::ZERO;
        normal[edge.first_axis] = edge.first_sign * angle.cos();
        normal[edge.second_axis] = edge.second_sign * angle.sin();
        (point, normal.normalize(), [u, v])
    };
    add_grid(mesh, subdivisions, 1, &mut map);
}

fn add_corner(
    mesh: &mut IndexedMesh,
    half: Vec3,
    radius: f32,
    subdivisions: u32,
    x_sign: f32,
    y_sign: f32,
    z_sign: f32,
) {
    let mut map = |u: f32, v: f32| {
        let theta = u * std::f32::consts::FRAC_PI_2;
        let phi = v * std::f32::consts::FRAC_PI_2;
        let radial = Vec3::new(theta.cos() * phi.cos(), theta.sin() * phi.cos(), phi.sin());
        let signs = Vec3::new(x_sign, y_sign, z_sign);
        let center = signs * (half - Vec3::splat(radius));
        let point = center + signs * radial * radius;
        (point, (signs * radial).normalize(), [u, v])
    };
    add_grid(mesh, subdivisions, subdivisions, &mut map);
}

fn add_grid(
    mesh: &mut IndexedMesh,
    u_steps: u32,
    v_steps: u32,
    map: &mut impl FnMut(f32, f32) -> (Vec3, Vec3, [f32; 2]),
) {
    let base = mesh.vertices.len() as u32;
    for v in 0..=v_steps {
        for u in 0..=u_steps {
            let (position, normal, uv) = map(
                u as f32 / u_steps.max(1) as f32,
                v as f32 / v_steps.max(1) as f32,
            );
            mesh.vertices.push(RoundedVertex {
                position,
                normal,
                uv,
            });
        }
    }
    let row = u_steps + 1;
    for v in 0..v_steps {
        for u in 0..u_steps {
            let a = base + v * row + u;
            let b = a + 1;
            let d = base + (v + 1) * row + u;
            let c = d + 1;
            let pa = mesh.vertices[a as usize].position;
            let pb = mesh.vertices[b as usize].position;
            let pc = mesh.vertices[c as usize].position;
            let expected = mesh.vertices[a as usize].normal;
            let first = (pb - pa).cross(pc - pa);
            if first.length_squared() > 1.0e-12 {
                if first.dot(expected) >= 0.0 {
                    mesh.indices.extend_from_slice(&[a, b, c]);
                } else {
                    mesh.indices.extend_from_slice(&[a, c, b]);
                }
            }
            let pa = mesh.vertices[a as usize].position;
            let pb = mesh.vertices[c as usize].position;
            let pc = mesh.vertices[d as usize].position;
            let second = (pb - pa).cross(pc - pa);
            if second.length_squared() > 1.0e-12 {
                if second.dot(expected) >= 0.0 {
                    mesh.indices.extend_from_slice(&[a, c, d]);
                } else {
                    mesh.indices.extend_from_slice(&[a, d, c]);
                }
            }
        }
    }
}

fn apply_taper(mesh: &mut IndexedMesh, height: f32, taper: TaperProfile) {
    if taper == TaperProfile::default() {
        return;
    }
    for vertex in &mut mesh.vertices {
        let y = (vertex.position.y / height + 0.5).clamp(0.0, 1.0);
        let scale = lerp(taper.bottom, taper.top, y);
        vertex.position.x *= scale;
        vertex.position.z *= scale;
    }
}

fn recompute_normals(mesh: &mut IndexedMesh) {
    let mut normals = vec![Vec3::ZERO; mesh.vertices.len()];
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let a = triangle[0] as usize;
        let b = triangle[1] as usize;
        let c = triangle[2] as usize;
        let normal = (mesh.vertices[b].position - mesh.vertices[a].position)
            .cross(mesh.vertices[c].position - mesh.vertices[a].position);
        normals[a] += normal;
        normals[b] += normal;
        normals[c] += normal;
    }
    for (vertex, normal) in mesh.vertices.iter_mut().zip(normals) {
        vertex.normal = if normal.length_squared() > 1.0e-12 {
            normal.normalize()
        } else {
            vertex.normal.normalize_or_zero()
        };
    }
}

fn quantize(value: f32) -> u32 {
    (value * 1000.0).round() as u32
}

fn lerp(start: f32, end: f32, amount: f32) -> f32 {
    start + (end - start) * amount
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(radius: f32, subdivisions: u32) -> RoundedBoxRecipe {
        RoundedBoxRecipe::new(
            Vec3::new(2.0, 3.0, 1.5),
            radius,
            subdivisions,
            TaperProfile::default(),
        )
    }

    #[test]
    fn rounded_mesh_is_indexed_and_has_valid_attributes() {
        let mesh = build_normalized(normalize(recipe(0.2, 4)).unwrap());
        assert!(!mesh.indices.is_empty());
        assert!(mesh.indices.len().is_multiple_of(3));
        for index in &mesh.indices {
            assert!((*index as usize) < mesh.vertices.len());
        }
        for vertex in &mesh.vertices {
            assert!(vertex.position.is_finite());
            assert!(vertex.normal.is_finite());
            assert!(
                (vertex.normal.length() - 1.0).abs() < 0.001,
                "zero/invalid normal at position {:?}, uv {:?}",
                vertex.position,
                vertex.uv
            );
            assert!(vertex.uv.iter().all(|value| value.is_finite()));
        }
    }

    #[test]
    fn rounded_mesh_has_planar_faces_and_curved_edges() {
        let mesh = build_normalized(normalize(recipe(0.2, 4)).unwrap());
        assert!(mesh.vertices.iter().any(|vertex| {
            (vertex.position.z - 0.75).abs() < 0.0001
                && (vertex.position.x.abs() <= 0.8)
                && (vertex.position.y.abs() <= 1.3)
        }));
        assert!(mesh.vertices.iter().any(|vertex| {
            vertex.position.x.abs() > 0.8
                && vertex.position.y.abs() > 1.3
                && vertex.position.z.abs() > 0.55
        }));
    }

    #[test]
    fn bounds_radius_clamping_and_lod_are_bounded() {
        let clamped = normalize(recipe(100.0, 100)).unwrap();
        assert!(clamped.radius < 0.75);
        assert_eq!(clamped.subdivisions, MAX_SUBDIVISIONS);
        let low = build_normalized(normalize(recipe(0.2, 1)).unwrap());
        let high = build_normalized(normalize(recipe(0.2, 4)).unwrap());
        assert!(high.vertices.len() > low.vertices.len());
        assert!(low.bounds_min.x < low.bounds_max.x);
        assert!(low.bounds_min.y < low.bounds_max.y);
        assert!(low.bounds_min.z < low.bounds_max.z);
    }

    #[test]
    fn taper_recomputes_outward_normals_and_bounds() {
        let recipe = RoundedBoxRecipe::new(
            Vec3::splat(2.0),
            0.2,
            2,
            TaperProfile {
                bottom: 0.8,
                top: 1.2,
            },
        );
        let mesh = build_normalized(normalize(recipe).unwrap());
        assert!(mesh.bounds_min.x < -1.0 && mesh.bounds_max.x > 1.0);
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let a = &mesh.vertices[triangle[0] as usize];
            let b = &mesh.vertices[triangle[1] as usize];
            let c = &mesh.vertices[triangle[2] as usize];
            let face = (b.position - a.position).cross(c.position - a.position);
            assert!(face.dot(a.normal) >= -0.0001);
        }
    }

    #[test]
    fn invalid_recipes_are_rejected_and_cache_is_bounded() {
        assert_eq!(
            normalize(recipe(-0.1, 1)),
            Err(RoundedGeometryError::NegativeRadius)
        );
        assert_eq!(
            normalize(RoundedBoxRecipe::new(
                Vec3::new(f32::NAN, 1.0, 1.0),
                0.1,
                1,
                TaperProfile::default(),
            )),
            Err(RoundedGeometryError::NonFiniteSize)
        );

        let mut cache = RoundedMeshCache::new(2);
        cache.get_or_build(recipe(0.1, 1)).unwrap();
        cache.get_or_build(recipe(0.2, 1)).unwrap();
        cache.get_or_build(recipe(0.3, 1)).unwrap();
        assert_eq!(cache.len(), 2);
        assert!(Arc::ptr_eq(
            &cache.get_or_build(recipe(0.2, 1)).unwrap(),
            &cache.get_or_build(recipe(0.2, 1)).unwrap()
        ));
    }
}
