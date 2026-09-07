//! Swept elliptical locks with artist-authored curves and tapered sections.
use super::character::{Anchor, Feature, Part, Tint};
use super::hero_geometry::Shape;
use super::rounded_geometry::{IndexedMesh, RoundedVertex};
use crate::character::{BodyId, BodyPart, JointId, hair::{self, HairLock}};
use glam::{Mat4, Vec3};

pub(super) fn add_parts(parts: &mut Vec<Part>, body: BodyId) -> bool {
    let Some((style_id, style)) = hair::for_body(body) else { return false; };
    parts.push(Part {
        anchor: Anchor {
            joint: JointId::Head,
            local: Mat4::from_translation(Vec3::from_array(style.cap.center)),
        },
        spec: BodyPart::new(Vec3::from_array(style.cap.size), 0.0),
        tint: Tint::Hair,
        feature: Feature::None,
        shape: Shape::HairScalp,
    });
    for (index, lock) in style.locks.iter().enumerate() {
        parts.push(Part {
            anchor: Anchor {
                joint: JointId::Head,
                local: Mat4::from_translation(Vec3::from_array(lock.points[0])),
            },
            spec: BodyPart::new(Vec3::ONE, 0.0),
            tint: Tint::Hair,
            feature: Feature::None,
            shape: Shape::HairCurve(style_id, index as u8),
        });
    }
    true
}

fn center_and_tangent(lock: &HairLock, t: f32) -> (Vec3, Vec3) {
    let [a, b, c, d] = lock.points.map(Vec3::from_array);
    let u = 1.0 - t;
    let center = a * u.powi(3) + b * (3.0 * u * u * t)
        + c * (3.0 * u * t * t) + d * t.powi(3);
    let tangent = ((b - a) * (3.0 * u * u) + (c - b) * (6.0 * u * t)
        + (d - c) * (3.0 * t * t)).try_normalize().unwrap_or(Vec3::NEG_Y);
    (center - a, tangent)
}

fn radius(lock: &HairLock, t: f32) -> f32 {
    let at = t * 5.0;
    let i = (at as usize).min(4);
    let f = at - i as f32;
    let [a, b, c, d] = [i.saturating_sub(1), i, i + 1, (i + 2).min(5)]
        .map(|j| lock.profile[j]);
    // Shared tangents make broad locks smooth all the way to the tip.
    (0.5 * ((2.0 * b) + (-a + c) * f
        + (2.0 * a - 5.0 * b + 4.0 * c - d) * f * f
        + (-a + 3.0 * b - 3.0 * c + d) * f * f * f)).clamp(0.005, 0.6)
}

fn surface(lock: &HairLock, t: f32, angle: f32) -> Vec3 {
    let (center, tangent) = center_and_tangent(lock, t);
    let outward = Vec3::from_array(lock.outward).normalize();
    let normal = (outward - tangent * tangent.dot(outward)).try_normalize()
        .unwrap_or_else(|| {
            let axis = if tangent.x.abs() < 0.8 { Vec3::X } else { Vec3::Z };
            (axis - tangent * tangent.dot(axis)).normalize()
        });
    let side = tangent.cross(normal).normalize();
    let (sin, cos) = angle.sin_cos();
    let r = radius(lock, t);
    center + side * (cos * lock.width * r) + normal * (sin * lock.depth * r)
}

/// The Bezier lies in its control hull. Include the maximum permitted
/// section radius so off-skull ponytails also participate in culling/LOD.
pub(super) fn bounds(style: u8, index: u8) -> (Vec3, Vec3) {
    let lock = &hair::get(style).locks[index as usize];
    let root = Vec3::from_array(lock.points[0]);
    let padding = Vec3::splat(lock.width.max(lock.depth) * 0.6);
    let min = lock.points.iter().map(|p| Vec3::from_array(*p) - root)
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let max = lock.points.iter().map(|p| Vec3::from_array(*p) - root)
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    (min - padding, max + padding)
}

pub(super) fn top(body: BodyId) -> Option<f32> {
    let (_, style) = hair::for_body(body)?;
    let mut top = style.cap.center[1] + style.cap.size[1] * 0.5;
    // Used only when the label-height cache initializes, never per frame.
    for lock in &style.locks {
        for row in 0..=32 {
            for col in 0..24 {
                let t = row as f32 / 32.0;
                let angle = col as f32 / 24.0 * std::f32::consts::TAU;
                top = top.max(lock.points[0][1] + surface(lock, t, angle).y);
            }
        }
    }
    Some(top + 0.03)
}

pub(super) fn build(style: u8, index: u8, size: Vec3, subdivisions: u32) -> IndexedMesh {
    let lock = &hair::get(style).locks[index as usize];
    let detail = subdivisions.clamp(1, 4) as usize;
    let rows = 16 + detail * 6;
    let radial = 12 + detail * 4;
    let mut vertices = Vec::with_capacity((rows + 1) * (radial + 1) + 2);
    let mut indices = Vec::with_capacity((rows + 1) * radial * 6);
    for row in 0..=rows {
        let t = row as f32 / rows as f32;
        for col in 0..=radial {
            let angle = col as f32 / radial as f32 * std::f32::consts::TAU;
            let along = (surface(lock, (t + 0.0001).min(1.0), angle)
                - surface(lock, (t - 0.0001).max(0.0), angle)) * size;
            let around = (surface(lock, t, angle + 0.0001)
                - surface(lock, t, angle - 0.0001)) * size;
            vertices.push(RoundedVertex {
                position: surface(lock, t, angle) * size,
                normal: along.cross(around).normalize_or_zero(),
                uv: [col as f32 / radial as f32, t],
            });
            if row < rows && col < radial {
                let a = (row * (radial + 1) + col) as u32;
                let b = a + radial as u32 + 1;
                indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
    }
    for (row, t, sign) in [(0, 0.0, -1.0), (rows, 1.0, 1.0)] {
        let (center, tangent) = center_and_tangent(lock, t);
        let id = vertices.len() as u32;
        vertices.push(RoundedVertex {
            position: center * size,
            normal: (tangent * sign / size).normalize(),
            uv: [0.5, t],
        });
        for col in 0..radial {
            let a = (row * (radial + 1) + col) as u32;
            indices.extend_from_slice(&if row == 0 { [id, a, a + 1] } else { [id, a + 1, a] });
        }
    }
    let bounds_min = vertices.iter().map(|v| v.position)
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let bounds_max = vertices.iter().map(|v| v.position)
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    IndexedMesh { vertices, indices, bounds_min, bounds_max }
}
