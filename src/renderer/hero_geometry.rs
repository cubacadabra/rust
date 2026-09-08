//! Authored surface profiles for the green-hoodie hero. These immutable meshes
//! are sampled at three bounded LODs when the renderer catalog is created.
use super::rounded_geometry::{IndexedMesh, RoundedVertex};
use glam::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Shape {
    #[default]
    Rounded,
    Head,
    Torso,
    Sleeve,
    Hood,
    Pebble,
    Pocket,
    HairCap,
    /// Skull-following foundation for data-authored hairstyles.
    HairScalp,
    HairLock,
    /// Finite asset/lock indices; mesh identity includes the authored curve.
    HairCurve(u8, u8),
    Shoe,
    Rib,
    Cord,
    Shorts,
    Limb,
    Laces,
}

/// The shoe profile's lowest generated vertex is at the bottom of its
/// normalized y range. Sole placement derives its local center from this
/// geometry contract instead of from a guessed box extent.
pub(super) const SHOE_MIN_NORMALIZED_Y: f32 = -0.5;

// Height, half-width, half-depth. The profiles describe cheek planes,
// dropped shoulders, gathered fabric, and broad toes instead of box fillets.
const HEAD: &[[f32; 3]] = &[
    [0.0, 0.18, 0.22],
    [0.06, 0.33, 0.34],
    [0.22, 0.46, 0.46],
    [0.45, 0.49, 0.49],
    [0.72, 0.48, 0.48],
    [0.91, 0.36, 0.36],
    [1.0, 0.025, 0.025],
];
const TORSO: &[[f32; 3]] = &[
    [0.0, 0.37, 0.36],
    [0.09, 0.44, 0.44],
    [0.25, 0.48, 0.47],
    [0.55, 0.47, 0.45],
    [0.74, 0.48, 0.40],
    [0.91, 0.40, 0.31],
    [1.0, 0.19, 0.23],
];
// Hair wraps the back/sides of the skull with an uneven lower edge. It is
// deliberately not another copy of the cheek/jaw profile.
const HAIR: &[[f32; 3]] = &[
    [0.0, 0.40, 0.39],
    [0.22, 0.47, 0.46],
    [0.48, 0.49, 0.49],
    [0.74, 0.43, 0.43],
    [0.91, 0.28, 0.29],
    [1.0, 0.025, 0.025],
];
const SLEEVE: &[[f32; 3]] = &[
    [0.0, 0.31, 0.32],
    [0.12, 0.37, 0.38],
    [0.35, 0.40, 0.41],
    [0.57, 0.43, 0.43],
    [0.80, 0.42, 0.42],
    [0.93, 0.35, 0.35],
    [1.0, 0.12, 0.14],
];
const POCKET: &[[f32; 3]] = &[
    [0.0, 0.34, 0.20],
    [0.13, 0.48, 0.43],
    [0.38, 0.48, 0.48],
    [0.72, 0.37, 0.45],
    [0.94, 0.29, 0.34],
    [1.0, 0.27, 0.20],
];
const LOCK: &[[f32; 3]] = &[
    [0.0, 0.39, 0.40],
    [0.16, 0.48, 0.47],
    [0.38, 0.46, 0.44],
    [0.62, 0.36, 0.37],
    [0.82, 0.23, 0.26],
    [0.94, 0.12, 0.15],
    [1.0, 0.035, 0.055],
];
const SHOE: &[[f32; 3]] = &[
    [0.0, 0.43, 0.46],
    [0.12, 0.49, 0.49],
    [0.38, 0.48, 0.48],
    [0.65, 0.41, 0.40],
    [0.85, 0.31, 0.29],
    [1.0, 0.22, 0.22],
];

fn profile(points: &[[f32; 3]], t: f32) -> (f32, f32) {
    let i = points.windows(2).position(|p| t <= p[1][0]).unwrap();
    let a = points[i];
    let b = points[i + 1];
    let dt = b[0] - a[0];
    let f = ((t - a[0]) / dt).clamp(0.0, 1.0);
    let interpolate = |axis: usize| {
        let slope = |j: usize| {
            let lo = j.saturating_sub(1);
            let hi = (j + 1).min(points.len() - 1);
            (points[hi][axis] - points[lo][axis]) / (points[hi][0] - points[lo][0])
        };
        // Shared tangents keep shading continuous across authored rings.
        (2.0 * f * f * f - 3.0 * f * f + 1.0) * a[axis]
            + (f * f * f - 2.0 * f * f + f) * dt * slope(i)
            + (-2.0 * f * f * f + 3.0 * f * f) * b[axis]
            + (f * f * f - f * f) * dt * slope(i + 1)
    };
    (interpolate(1), interpolate(2))
}

fn signed_power(x: f32, p: f32) -> f32 {
    x.signum() * x.abs().powf(p)
}

fn surface(shape: Shape, t: f32, angle: f32) -> Vec3 {
    let (sin, cos) = angle.sin_cos();
    if shape == Shape::HairScalp {
        // Sample the skull at the actual hairline height. Raising the old
        // cap's vertices without changing their radii exposed skin at temples.
        let front = (-sin).max(0.0);
        let bottom = 0.13 + 0.56 * front.powi(2)
            + 0.19 * cos.abs().powi(6)
            + 0.025 * (angle * 5.0 + 0.4).sin() * (1.0 - front).powi(2);
        let height = bottom + (1.0 - bottom) * t;
        let (rx, rz) = profile(HEAD, height);
        let comb = 0.006 * (angle * 7.0 + t * 3.0).cos()
            * (t * std::f32::consts::PI).sin();
        return Vec3::new(
            (rx + comb) * signed_power(cos, 0.70),
            height - 0.5,
            (rz + comb) * signed_power(sin, 0.70),
        );
    }
    if shape == Shape::Hood {
        let (v, u) = (t * std::f32::consts::TAU).sin_cos();
        return Vec3::new(
            (0.33 + 0.15 * u) * cos,
            0.22 * v + 0.26 * sin,
            (0.32 + 0.15 * u) * sin,
        );
    }
    let (rx, rz, exponent) = match shape {
        Shape::Head => {
            let (x, z) = profile(HEAD, t);
            (x, z, 0.58)
        }
        Shape::HairCap => {
            let (x, z) = profile(HAIR, t);
            (x, z, 0.82)
        }
        Shape::Torso => {
            let (x, z) = profile(TORSO, t);
            (x, z, 0.66)
        }
        Shape::Sleeve => {
            let (x, z) = profile(SLEEVE, t);
            (x, z, 0.90)
        }
        Shape::Pocket => {
            let (x, z) = profile(POCKET, t);
            (x, z, 0.58)
        }
        Shape::HairLock => {
            let (x, z) = profile(LOCK, t);
            (x, z, 0.85)
        }
        Shape::Shoe => {
            let (x, z) = profile(SHOE, t);
            (x, z, 0.65)
        }
        Shape::Rib => (
            0.47 - (t * std::f32::consts::PI).cos().abs() * 0.025,
            0.46,
            0.70,
        ),
        Shape::Cord => (0.32 + (t * std::f32::consts::PI).sin() * 0.04, 0.34, 1.0),
        Shape::Shorts => (0.40 + 0.07 * (t * std::f32::consts::PI).sin(), 0.43, 0.65),
        Shape::Limb => (0.34 + 0.10 * (t * std::f32::consts::PI).sin(), 0.40, 0.86),
        _ => {
            let r = (1.0 - (t * 2.0 - 1.0).powi(2)).max(0.001).sqrt() * 0.49;
            (r, r, 0.95)
        }
    };
    let mut p = Vec3::new(
        rx * signed_power(cos, exponent),
        t - 0.5,
        rz * signed_power(sin, exponent),
    );
    match shape {
        Shape::Torso | Shape::Sleeve => {
            // Broad modeled folds die away before cuffs/neck. They change
            // surface normals as well as silhouette, with no texture noise.
            let hem = (-((t - 0.18) / 0.16).powi(2)).exp();
            let elbow = (-((t - 0.68) / 0.19).powi(2)).exp();
            let fold = (angle * 5.0 + t * 19.0).sin() * hem * 0.007
                + (angle * 3.0 - t * 23.0).sin() * elbow * 0.004;
            p.x += cos * fold;
            p.z += sin * fold;
            if shape == Shape::Sleeve {
                p.x += (t * std::f32::consts::PI).sin() * 0.025;
            }
        }
        Shape::HairCap => {
            p.y += (-sin).max(0.0).powi(3) * (1.0 - t) * 0.56;
            p.y += (1.0 + (angle * 5.0 + 0.6).sin()) * 0.045 * (1.0 - t).powi(3);
            // Broad combed lobes are geometry, so the hair catches light and
            // retains an irregular silhouette from the rear and low camera.
            let sweep = angle * 7.0 + t * 3.2 + 0.7;
            let lobe = sweep.cos() * 0.018 * (t * std::f32::consts::PI).sin();
            p.x += cos * lobe + (t * std::f32::consts::PI).sin() * 0.020;
            p.z += sin * lobe;
            p.y += (angle * 3.0 + t * 2.0).sin() * 0.018 * t * (1.0 - t);
        }
        Shape::HairLock => {
            let arch = (t * std::f32::consts::PI).sin();
            p.x += arch * 0.07;
            p.z += arch * 0.045;
            let groove = (angle * 3.0 + t * 1.3).cos() * 0.012 * arch;
            p.x += cos * groove;
            p.z += sin * groove;
        }
        Shape::Shoe => {
            p.z = (p.z + t * 0.14 - 0.03) * 0.95;
            p.y -= (-sin).max(0.0) * t * 0.15;
        }
        // Fine ribbing is filtered in the material; modeling it below the
        // angular sampling rate would alias the cuff silhouette.
        _ => {}
    }
    if shape == Shape::HairCap {
        p.x = p.x.clamp(-0.51, 0.51);
        p.z = p.z.clamp(-0.51, 0.51);
    }
    p
}

pub(super) fn build(shape: Shape, size: Vec3, subdivisions: u32) -> IndexedMesh {
    if let Shape::HairCurve(style, index) = shape {
        return super::hair_geometry::build(style, index, size, subdivisions);
    }
    if shape == Shape::Laces {
        let mut mesh = IndexedMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            bounds_min: Vec3::splat(f32::INFINITY),
            bounds_max: Vec3::splat(f32::NEG_INFINITY),
        };
        for z in [-0.32, 0.0, 0.32] {
            let band = build(Shape::Pebble, Vec3::new(1.0, 0.28, 0.15), 1);
            let offset = mesh.vertices.len() as u32;
            mesh.indices
                .extend(band.indices.into_iter().map(|i| i + offset));
            for mut vertex in band.vertices {
                vertex.position = (vertex.position + Vec3::new(0.0, z * 0.7, z)) * size;
                vertex.normal = (vertex.normal / size).normalize();
                mesh.bounds_min = mesh.bounds_min.min(vertex.position);
                mesh.bounds_max = mesh.bounds_max.max(vertex.position);
                mesh.vertices.push(vertex);
            }
        }
        return mesh;
    }
    let detail = subdivisions.clamp(1, 4) as usize;
    // Spend vertices on the large head and silhouette, not on a drawstring
    // or three tiny laces. All counts remain fixed per catalog LOD.
    let (radial, rows) = match shape {
        Shape::HairScalp => (28 + detail * 6, 10 + detail * 4),
        Shape::Head | Shape::HairCap => (16 + detail * 4, 8 + detail * 4),
        Shape::Hood => (16 + detail * 4, 8 + detail),
        Shape::Rib => (16 + detail * 4, 2 + detail / 2),
        Shape::Cord => (8 + detail, 4),
        Shape::Shorts | Shape::Limb => (12 + detail * 3, 4 + detail),
        Shape::Pebble => (12 + detail * 3, 6 + detail * 2),
        _ => (12 + detail * 3, 6 + detail * 3),
    };
    let closed = shape == Shape::Hood;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for row in 0..=rows {
        let t = row as f32 / rows as f32;
        for col in 0..=radial {
            let angle = col as f32 / radial as f32 * std::f32::consts::TAU;
            let sample_t = |t: f32| {
                if closed {
                    t.rem_euclid(1.0)
                } else {
                    t.clamp(0.0, 1.0)
                }
            };
            let dy = (surface(shape, sample_t(t + 0.0001), angle)
                - surface(shape, sample_t(t - 0.0001), angle))
                * size;
            let dx = (surface(shape, t, angle + 0.0001) - surface(shape, t, angle - 0.0001)) * size;
            vertices.push(RoundedVertex {
                position: surface(shape, t, angle) * size,
                normal: dy.cross(dx).normalize_or_zero(),
                uv: [col as f32 / radial as f32, t],
            });
            if row < rows && col < radial {
                let a = (row * (radial + 1) + col) as u32;
                let b = a + radial as u32 + 1;
                indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
    }
    if !closed {
        for (row, top) in [(0, false), (rows, true)] {
            let start = row * (radial + 1);
            let center = vertices[start..start + radial]
                .iter()
                .map(|v| v.position)
                .sum::<Vec3>()
                / radial as f32;
            let id = vertices.len() as u32;
            vertices.push(RoundedVertex {
                position: center,
                normal: if top { Vec3::Y } else { -Vec3::Y },
                uv: [0.5, if top { 1.0 } else { 0.0 }],
            });
            for col in 0..radial {
                let a = (start + col) as u32;
                indices.extend_from_slice(&if top { [id, a + 1, a] } else { [id, a, a + 1] });
            }
        }
    }
    let min = vertices
        .iter()
        .fold(Vec3::splat(f32::INFINITY), |a, v| a.min(v.position));
    let max = vertices
        .iter()
        .fold(Vec3::splat(f32::NEG_INFINITY), |a, v| a.max(v.position));
    IndexedMesh {
        vertices,
        indices,
        bounds_min: min,
        bounds_max: max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_meshes_are_finite_indexed_and_bounded_at_each_lod() {
        for shape in [
            Shape::Head,
            Shape::Torso,
            Shape::Sleeve,
            Shape::Hood,
            Shape::Pebble,
            Shape::Pocket,
            Shape::HairCap,
            Shape::HairLock,
            Shape::Shoe,
            Shape::Rib,
            Shape::Cord,
            Shape::Shorts,
            Shape::Limb,
            Shape::Laces,
        ] {
            for lod in [1, 2, 4] {
                let mesh = build(shape, Vec3::ONE, lod);
                assert!(
                    mesh.vertices
                        .iter()
                        .all(|v| v.position.is_finite() && v.normal.is_normalized()),
                    "{shape:?} {lod}"
                );
                assert!(
                    mesh.indices
                        .iter()
                        .all(|i| (*i as usize) < mesh.vertices.len())
                );
                assert!(mesh.bounds_min.cmpge(Vec3::splat(-0.51)).all());
                let maximum_extent = if shape == Shape::HairLock {
                    0.55
                } else {
                    0.51
                };
                assert!(
                    mesh.bounds_max
                        .cmple(Vec3::splat(maximum_extent))
                        .all(),
                    "{shape:?}: {:?}",
                    mesh.bounds_max
                );
                assert!(mesh.indices.len() / 3 < 5000);
            }
        }
    }
}
