//! Bounded package-authored triangle collision shared by movement and cameras.
//!
//! Collision geometry is deliberately independent of renderer assets. Packages
//! contain world-space triangles, and loading builds a compact XZ broadphase so
//! headless and rendered hosts use identical collision semantics.

use std::collections::BTreeMap;

use glam::Vec3;
use serde::Deserialize;

pub(crate) const STATIC_COLLISION_FORMAT_VERSION: u32 = 1;
pub(crate) const MAX_STATIC_COLLISION_TRIANGLES: usize = 200_000;
const MAX_STATIC_COLLISION_COORDINATE: f32 = 4096.0;
const BUCKET_SIZE: f32 = 8.0;
const MAX_BUCKETS_PER_TRIANGLE: usize = 256;
const MAX_BUCKET_REFERENCES: usize = 2_000_000;
const MIN_TRIANGLE_AREA_SQUARED: f32 = 1.0e-12;
pub(crate) const MIN_WALKABLE_NORMAL_Y: f32 = 0.45;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StaticCollisionDefinition {
    pub(crate) format_version: u32,
    #[serde(default)]
    pub(crate) triangles: Vec<[[f32; 3]; 3]>,
}

impl StaticCollisionDefinition {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.format_version != STATIC_COLLISION_FORMAT_VERSION {
            return Err(format!(
                "unsupported collision formatVersion {}; runtime supports {}",
                self.format_version, STATIC_COLLISION_FORMAT_VERSION
            ));
        }
        if self.triangles.len() > MAX_STATIC_COLLISION_TRIANGLES {
            return Err(format!(
                "collision cannot contain more than {MAX_STATIC_COLLISION_TRIANGLES} triangles"
            ));
        }
        for triangle in &self.triangles {
            if triangle
                .iter()
                .flatten()
                .any(|value| !value.is_finite() || value.abs() > MAX_STATIC_COLLISION_COORDINATE)
            {
                return Err(
                    "collision triangle coordinates must be finite and inside supported world bounds"
                        .into(),
                );
            }
            let [a, b, c] = triangle.map(Vec3::from_array);
            if (b - a).cross(c - a).length_squared() <= MIN_TRIANGLE_AREA_SQUARED {
                return Err("collision triangles must have non-zero area".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct Triangle {
    vertices: [Vec3; 3],
    normal: Vec3,
    minimum: Vec3,
    maximum: Vec3,
}

impl Triangle {
    fn new(vertices: [[f32; 3]; 3]) -> Self {
        let vertices = vertices.map(Vec3::from_array);
        let normal = (vertices[1] - vertices[0])
            .cross(vertices[2] - vertices[0])
            .normalize();
        let minimum = vertices[0].min(vertices[1]).min(vertices[2]);
        let maximum = vertices[0].max(vertices[1]).max(vertices[2]);
        Self {
            vertices,
            normal,
            minimum,
            maximum,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StaticCollision {
    triangles: Vec<Triangle>,
    buckets: BTreeMap<[i32; 2], Vec<usize>>,
    large_triangles: Vec<usize>,
}

impl StaticCollision {
    pub(crate) fn build(definition: &StaticCollisionDefinition) -> Result<Option<Self>, String> {
        definition.validate()?;
        if definition.triangles.is_empty() {
            return Ok(None);
        }

        let triangles = definition
            .triangles
            .iter()
            .copied()
            .map(Triangle::new)
            .collect::<Vec<_>>();
        let mut buckets = BTreeMap::<[i32; 2], Vec<usize>>::new();
        let mut large_triangles = Vec::new();
        let mut references = 0usize;
        for (index, triangle) in triangles.iter().enumerate() {
            let minimum = bucket_coordinate(triangle.minimum);
            let maximum = bucket_coordinate(triangle.maximum);
            let width = (maximum[0] - minimum[0] + 1) as usize;
            let depth = (maximum[1] - minimum[1] + 1) as usize;
            let count = width.saturating_mul(depth);
            if count > MAX_BUCKETS_PER_TRIANGLE {
                large_triangles.push(index);
                continue;
            }
            references = references
                .checked_add(count)
                .ok_or_else(|| "collision broadphase size overflow".to_owned())?;
            if references > MAX_BUCKET_REFERENCES {
                return Err(format!(
                    "collision exceeds the {MAX_BUCKET_REFERENCES}-reference broadphase limit"
                ));
            }
            for z in minimum[1]..=maximum[1] {
                for x in minimum[0]..=maximum[0] {
                    buckets.entry([x, z]).or_default().push(index);
                }
            }
        }
        Ok(Some(Self {
            triangles,
            buckets,
            large_triangles,
        }))
    }

    /// Whether a vertical character capsule can occupy `feet` without
    /// penetrating any authored triangle. Resting tangency remains clear.
    pub(crate) fn capsule_clear(&self, feet: [f32; 3], radius: f32, height: f32) -> bool {
        if !valid_capsule(feet, radius, height) {
            return false;
        }
        let feet = Vec3::from_array(feet);
        let bottom = feet + Vec3::Y * radius;
        let top = feet + Vec3::Y * (height - radius).max(radius);
        let clearance = (radius - 0.02).max(0.0);
        let minimum = Vec3::new(feet.x - radius, feet.y, feet.z - radius);
        let maximum = Vec3::new(feet.x + radius, feet.y + height, feet.z + radius);
        self.query_indices(minimum, maximum)
            .into_iter()
            .all(|index| {
                let triangle = &self.triangles[index];
                !aabb_overlaps(minimum, maximum, triangle.minimum, triangle.maximum)
                    || segment_triangle_distance_squared(bottom, top, triangle.vertices)
                        >= clearance * clearance
            })
    }

    /// Highest walkable surface below/above a capsule center within the given
    /// vertical interval. Winding is irrelevant; collision faces are two-sided.
    pub(crate) fn support_height(
        &self,
        x: f32,
        z: f32,
        minimum_y: f32,
        maximum_y: f32,
    ) -> Option<f32> {
        self.horizontal_surface_height(x, z, minimum_y, maximum_y, true)
    }

    /// Lowest horizontal-enough surface crossed by an upward-moving head.
    pub(crate) fn ceiling_height(
        &self,
        x: f32,
        z: f32,
        minimum_y: f32,
        maximum_y: f32,
    ) -> Option<f32> {
        self.horizontal_surface_height(x, z, minimum_y, maximum_y, false)
    }

    fn horizontal_surface_height(
        &self,
        x: f32,
        z: f32,
        minimum_y: f32,
        maximum_y: f32,
        highest: bool,
    ) -> Option<f32> {
        if !x.is_finite()
            || !z.is_finite()
            || !minimum_y.is_finite()
            || !maximum_y.is_finite()
            || minimum_y > maximum_y
        {
            return None;
        }
        let minimum = Vec3::new(x, minimum_y, z);
        let maximum = Vec3::new(x, maximum_y, z);
        let mut result: Option<f32> = None;
        for index in self.query_indices(minimum, maximum) {
            let triangle = &self.triangles[index];
            if triangle.normal.y.abs() < MIN_WALKABLE_NORMAL_Y
                || x < triangle.minimum.x - 0.001
                || x > triangle.maximum.x + 0.001
                || z < triangle.minimum.z - 0.001
                || z > triangle.maximum.z + 0.001
            {
                continue;
            }
            let Some(y) = projected_height(triangle, x, z) else {
                continue;
            };
            if y < minimum_y - 0.001 || y > maximum_y + 0.001 {
                continue;
            }
            result = Some(match result {
                Some(current) if highest => current.max(y),
                Some(current) => current.min(y),
                None => y,
            });
        }
        result
    }

    /// Distance from `start` to the first contact of a swept camera sphere.
    /// Faces, edges, and vertices are tested analytically after the broadphase,
    /// avoiding per-triangle sampling work in dense imported worlds.
    pub(crate) fn sweep_sphere(&self, start: [f32; 3], end: [f32; 3], radius: f32) -> Option<f32> {
        let start = Vec3::from_array(start);
        let end = Vec3::from_array(end);
        let delta = end - start;
        let length = delta.length();
        if !start.is_finite()
            || !end.is_finite()
            || !radius.is_finite()
            || radius < 0.0
            || !length.is_finite()
            || length <= f32::EPSILON
        {
            return None;
        }
        let expansion = Vec3::splat(radius);
        let minimum = start.min(end) - expansion;
        let maximum = start.max(end) + expansion;
        let candidates = self.query_indices(minimum, maximum);
        if candidates.is_empty() {
            return None;
        }
        candidates
            .into_iter()
            .filter_map(|index| {
                let triangle = &self.triangles[index];
                aabb_overlaps(minimum, maximum, triangle.minimum, triangle.maximum)
                    .then(|| swept_sphere_triangle_fraction(start, delta, radius, triangle))
                    .flatten()
            })
            .min_by(f32::total_cmp)
            .map(|fraction| fraction * length)
    }

    fn query_indices(&self, minimum: Vec3, maximum: Vec3) -> Vec<usize> {
        let first = bucket_coordinate(minimum);
        let last = bucket_coordinate(maximum);
        let mut indices = self.large_triangles.clone();
        for z in first[1]..=last[1] {
            for x in first[0]..=last[0] {
                if let Some(bucket) = self.buckets.get(&[x, z]) {
                    indices.extend_from_slice(bucket);
                }
            }
        }
        indices.sort_unstable();
        indices.dedup();
        indices
    }

    #[cfg(test)]
    pub(crate) fn triangles_len(&self) -> usize {
        self.triangles.len()
    }
}

fn valid_capsule(feet: [f32; 3], radius: f32, height: f32) -> bool {
    feet.iter().all(|value| value.is_finite())
        && radius.is_finite()
        && height.is_finite()
        && radius > 0.0
        && height >= radius * 2.0
}

fn bucket_coordinate(position: Vec3) -> [i32; 2] {
    [
        (position.x / BUCKET_SIZE).floor() as i32,
        (position.z / BUCKET_SIZE).floor() as i32,
    ]
}

fn aabb_overlaps(a_min: Vec3, a_max: Vec3, b_min: Vec3, b_max: Vec3) -> bool {
    a_min.x <= b_max.x
        && a_max.x >= b_min.x
        && a_min.y <= b_max.y
        && a_max.y >= b_min.y
        && a_min.z <= b_max.z
        && a_max.z >= b_min.z
}

fn projected_height(triangle: &Triangle, x: f32, z: f32) -> Option<f32> {
    let [a, b, c] = triangle.vertices;
    let v0 = Vec3::new(b.x - a.x, 0.0, b.z - a.z);
    let v1 = Vec3::new(c.x - a.x, 0.0, c.z - a.z);
    let v2 = Vec3::new(x - a.x, 0.0, z - a.z);
    let denominator = v0.x * v1.z - v1.x * v0.z;
    if denominator.abs() <= 1.0e-8 {
        return None;
    }
    let u = (v2.x * v1.z - v1.x * v2.z) / denominator;
    let v = (v0.x * v2.z - v2.x * v0.z) / denominator;
    let epsilon = 0.001;
    if u < -epsilon || v < -epsilon || u + v > 1.0 + epsilon {
        return None;
    }
    Some(a.y + (b.y - a.y) * u + (c.y - a.y) * v)
}

fn segment_triangle_distance_squared(start: Vec3, end: Vec3, triangle: [Vec3; 3]) -> f32 {
    if segment_intersects_triangle(start, end, triangle) {
        return 0.0;
    }
    let mut distance = point_triangle_distance_squared(start, triangle)
        .min(point_triangle_distance_squared(end, triangle));
    for [a, b] in [[0, 1], [1, 2], [2, 0]] {
        distance = distance.min(segment_segment_distance_squared(
            start,
            end,
            triangle[a],
            triangle[b],
        ));
    }
    distance
}

fn segment_intersects_triangle(start: Vec3, end: Vec3, [a, b, c]: [Vec3; 3]) -> bool {
    let direction = end - start;
    let edge1 = b - a;
    let edge2 = c - a;
    let p = direction.cross(edge2);
    let determinant = edge1.dot(p);
    if determinant.abs() <= 1.0e-8 {
        return false;
    }
    let inverse = determinant.recip();
    let offset = start - a;
    let u = offset.dot(p) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = offset.cross(edge1);
    let v = direction.dot(q) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    let amount = edge2.dot(q) * inverse;
    (0.0..=1.0).contains(&amount)
}

fn swept_sphere_triangle_fraction(
    start: Vec3,
    direction: Vec3,
    radius: f32,
    triangle: &Triangle,
) -> Option<f32> {
    if point_triangle_distance_squared(start, triangle.vertices) <= radius * radius {
        return Some(0.0);
    }

    let mut earliest = None;
    let plane_distance = (start - triangle.vertices[0]).dot(triangle.normal);
    let plane_velocity = direction.dot(triangle.normal);
    if plane_velocity.abs() > 1.0e-8 {
        for target_distance in [-radius, radius] {
            let fraction = (target_distance - plane_distance) / plane_velocity;
            if !(0.0..=1.0).contains(&fraction) {
                continue;
            }
            let center = start + direction * fraction;
            let contact = center - triangle.normal * target_distance;
            if point_inside_triangle(contact, triangle.vertices, triangle.normal) {
                earliest = earlier_fraction(earliest, fraction);
            }
        }
    }

    for [a, b] in [[0, 1], [1, 2], [2, 0]] {
        if let Some(fraction) = moving_point_edge_cylinder_fraction(
            start,
            direction,
            triangle.vertices[a],
            triangle.vertices[b],
            radius,
        ) {
            earliest = earlier_fraction(earliest, fraction);
        }
    }
    for vertex in triangle.vertices {
        if let Some(fraction) = moving_point_sphere_fraction(start, direction, vertex, radius) {
            earliest = earlier_fraction(earliest, fraction);
        }
    }
    earliest
}

fn earlier_fraction(current: Option<f32>, candidate: f32) -> Option<f32> {
    Some(current.map_or(candidate, |current| current.min(candidate)))
}

fn point_inside_triangle(point: Vec3, [a, b, c]: [Vec3; 3], normal: Vec3) -> bool {
    const EPSILON: f32 = -1.0e-5;
    (b - a).cross(point - a).dot(normal) >= EPSILON
        && (c - b).cross(point - b).dot(normal) >= EPSILON
        && (a - c).cross(point - c).dot(normal) >= EPSILON
}

fn moving_point_edge_cylinder_fraction(
    start: Vec3,
    direction: Vec3,
    a: Vec3,
    b: Vec3,
    radius: f32,
) -> Option<f32> {
    let edge = b - a;
    let edge_length_squared = edge.length_squared();
    if edge_length_squared <= 1.0e-12 {
        return None;
    }
    let offset = start - a;
    let direction_perpendicular = direction - edge * (direction.dot(edge) / edge_length_squared);
    let offset_perpendicular = offset - edge * (offset.dot(edge) / edge_length_squared);
    let qa = direction_perpendicular.length_squared();
    let qb = 2.0 * offset_perpendicular.dot(direction_perpendicular);
    let qc = offset_perpendicular.length_squared() - radius * radius;
    for fraction in quadratic_roots(qa, qb, qc) {
        if !(0.0..=1.0).contains(&fraction) {
            continue;
        }
        let along_edge = (offset + direction * fraction).dot(edge) / edge_length_squared;
        if (0.0..=1.0).contains(&along_edge) {
            return Some(fraction);
        }
    }
    None
}

fn moving_point_sphere_fraction(
    start: Vec3,
    direction: Vec3,
    center: Vec3,
    radius: f32,
) -> Option<f32> {
    let offset = start - center;
    quadratic_roots(
        direction.length_squared(),
        2.0 * offset.dot(direction),
        offset.length_squared() - radius * radius,
    )
    .into_iter()
    .find(|fraction| (0.0..=1.0).contains(fraction))
}

fn quadratic_roots(a: f32, b: f32, c: f32) -> [f32; 2] {
    if a.abs() <= 1.0e-12 {
        return [f32::INFINITY; 2];
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return [f32::INFINITY; 2];
    }
    let root = discriminant.sqrt();
    let inverse = 0.5 / a;
    let first = (-b - root) * inverse;
    let second = (-b + root) * inverse;
    if first <= second {
        [first, second]
    } else {
        [second, first]
    }
}

fn point_triangle_distance_squared(point: Vec3, [a, b, c]: [Vec3; 3]) -> f32 {
    let ab = b - a;
    let ac = c - a;
    let ap = point - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return ap.length_squared();
    }
    let bp = point - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return bp.length_squared();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let amount = d1 / (d1 - d3);
        return (point - (a + ab * amount)).length_squared();
    }
    let cp = point - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return cp.length_squared();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let amount = d2 / (d2 - d6);
        return (point - (a + ac * amount)).length_squared();
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let amount = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (point - (b + (c - b) * amount)).length_squared();
    }
    let denominator = (va + vb + vc).recip();
    let v = vb * denominator;
    let w = vc * denominator;
    (point - (a + ab * v + ac * w)).length_squared()
}

fn segment_segment_distance_squared(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> f32 {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let epsilon = 1.0e-8;
    let (mut s, mut t);
    if a <= epsilon && e <= epsilon {
        return r.length_squared();
    }
    if a <= epsilon {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e <= epsilon {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denominator = a * e - b * b;
            s = if denominator.abs() > epsilon {
                ((b * f - c * e) / denominator).clamp(0.0, 1.0)
            } else {
                0.0
            };
            t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
        }
    }
    let closest1 = p1 + d1 * s;
    let closest2 = p2 + d2 * t;
    (closest1 - closest2).length_squared()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition(triangles: Vec<[[f32; 3]; 3]>) -> StaticCollisionDefinition {
        StaticCollisionDefinition {
            format_version: STATIC_COLLISION_FORMAT_VERSION,
            triangles,
        }
    }

    #[test]
    fn validates_version_coordinates_and_degenerate_triangles() {
        let mut invalid_version = definition(vec![]);
        invalid_version.format_version = 2;
        assert!(invalid_version.validate().is_err());

        let outside = definition(vec![[[0.0, 0.0, 0.0], [5000.0, 0.0, 0.0], [0.0, 0.0, 1.0]]]);
        assert!(outside.validate().is_err());

        let degenerate = definition(vec![[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]]);
        assert!(degenerate.validate().is_err());
    }

    #[test]
    fn floor_wall_ceiling_and_void_queries_are_distinct() {
        let collision = StaticCollision::build(&definition(vec![
            [[-3.0, 0.0, -3.0], [3.0, 0.0, -3.0], [3.0, 0.0, 3.0]],
            [[-3.0, 0.0, -3.0], [3.0, 0.0, 3.0], [-3.0, 0.0, 3.0]],
            [[2.0, 0.0, -2.0], [2.0, 4.0, -2.0], [2.0, 4.0, 2.0]],
            [[-2.0, 4.0, -2.0], [2.0, 4.0, 2.0], [2.0, 4.0, -2.0]],
        ]))
        .unwrap()
        .unwrap();

        assert_eq!(collision.support_height(0.0, 0.0, -1.0, 1.0), Some(0.0));
        assert_eq!(collision.support_height(8.0, 0.0, -1.0, 1.0), None);
        assert!(!collision.capsule_clear([1.7, 0.0, 0.0], 0.52, 3.15));
        assert_eq!(collision.ceiling_height(0.0, 0.0, 3.0, 5.0), Some(4.0));
    }

    #[test]
    fn ramp_support_and_camera_sphere_use_authored_triangles() {
        let collision = StaticCollision::build(&definition(vec![
            [[-2.0, 0.0, -2.0], [2.0, 2.0, -2.0], [2.0, 2.0, 2.0]],
            [[-2.0, 0.0, -2.0], [2.0, 2.0, 2.0], [-2.0, 0.0, 2.0]],
        ]))
        .unwrap()
        .unwrap();
        assert!((collision.support_height(0.0, 0.0, -1.0, 2.0).unwrap() - 1.0).abs() < 0.001);
        let distance = collision
            .sweep_sphere([-4.0, 1.0, 0.0], [4.0, 1.0, 0.0], 0.35)
            .unwrap();
        assert!(distance > 3.0 && distance < 4.0);
    }

    #[test]
    fn camera_sphere_detects_triangle_edges_missed_by_its_center_ray() {
        let collision = StaticCollision::build(&definition(vec![[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ]]))
        .unwrap()
        .unwrap();
        let distance = collision
            .sweep_sphere([-0.2, 0.5, -1.0], [-0.2, 0.5, 1.0], 0.25)
            .expect("the sphere should catch the triangle edge");
        assert!(distance > 0.8 && distance < 1.0);
    }
}
