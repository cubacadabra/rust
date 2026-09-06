//! Fixed, renderer-owned catalog. All three bundled bodies are compiled and
//! uploaded before first use; changing colors/worlds cannot grow this cache.
use super::{
    AvatarStyle, RenderEntity,
    character::{self, Feature, Part},
    character_material::{self, CharacterInstance, CharacterPass, CharacterVertex, Material},
    character_quality::{self, CharacterLod},
    rounded_geometry::RoundedMeshCache,
};
use crate::character::{BodyId, BodyRecipe, OutfitId, body_recipe};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

pub(super) const MAX_CHARACTERS: usize = 50;
const MAX_PARTS: usize = 48;
pub(super) const MAX_MESHES: usize = 384;
const MAX_RESIDENCY: usize = 32 * 1024 * 1024;

fn feature_transform(part: Part, entity: RenderEntity) -> Mat4 {
    let face = entity.face.clamped();
    let mut local = part.anchor.local;
    match part.feature {
        Feature::None | Feature::Sole => {}
        Feature::Cloth => {
            local *= Mat4::from_rotation_x(entity.secondary.cloth_sway);
        }
        Feature::Eye(side) => {
            let look = face.look * if is_hero(entity) { 0.45 } else { 1.0 };
            local = local
                * Mat4::from_translation(glam::Vec3::new(look.x, look.y, 0.0))
                * Mat4::from_scale(glam::Vec3::new(
                    1.0,
                    (face.eye_opening + side * face.eye_asymmetry).clamp(0.05, 1.25),
                    1.0,
                ));
        }
        Feature::Brow(side) => {
            local = local
                * Mat4::from_translation(Vec3::Y * side * face.brow_asymmetry)
                * Mat4::from_quat(Quat::from_rotation_z(side * face.brow_tilt));
        }
        Feature::Mouth => {
            local = local
                * Mat4::from_scale(glam::Vec3::new(1.0 + face.mouth_opening * 0.22, 1.0, 1.0));
        }
        Feature::Cheek(_side) => {
            local = local
                * Mat4::from_scale(glam::Vec3::new(
                    1.0 + face.mouth_opening * 0.12,
                    1.0 + face.mouth_opening * 0.18,
                    1.0,
                ));
        }
        Feature::Ear(side) => {
            local =
                local * Mat4::from_quat(Quat::from_rotation_z(side * entity.secondary.ear_tilt));
        }
        Feature::Tail(progress) => {
            local = local
                * Mat4::from_quat(Quat::from_rotation_y(
                    entity.secondary.tail_sway * (0.55 + progress * 0.8),
                ));
        }
        Feature::Wing(side) => {
            local =
                local * Mat4::from_quat(Quat::from_rotation_z(side * entity.secondary.wing_flap));
        }
        Feature::Seam(phase) => {
            let intensity = entity.secondary.gap_expansion.clamp(0.0, 0.72);
            local = local
                * Mat4::from_scale(Vec3::splat(1.0 + intensity * (0.38 + phase.abs() * 0.10)));
        }
        Feature::Spark(side) => {
            let intensity = 1.0 - (entity.secondary.spark_life / 0.36).clamp(0.0, 1.0);
            local = local
                * Mat4::from_translation(Vec3::new(
                    side * intensity * 0.40,
                    intensity * 0.50,
                    -intensity * 0.05,
                ))
                * Mat4::from_rotation_z(side * (0.7 + intensity * 1.4))
                * Mat4::from_scale(Vec3::new(
                    1.0 + intensity * 1.8,
                    1.0 + intensity * 0.6,
                    1.0 + intensity * 1.8,
                ));
        }
    }
    if part.shape != super::hero_geometry::Shape::Rounded {
        local *= Mat4::from_scale(part.spec.size);
    }
    local
}

fn is_hero(entity: RenderEntity) -> bool {
    entity.body == BodyId::Person && entity.outfit == OutfitId::EverydayHoodie
}

struct Mesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}
struct CompiledBody {
    body: BodyId,
    outfit: OutfitId,
    recipe: BodyRecipe,
    parts: [Vec<(Part, usize)>; 3],
}
struct Batch {
    mesh: usize,
    material: Material,
    instances: Vec<CharacterInstance>,
    start: usize,
}

fn part_visible(part: Part, lod: CharacterLod) -> bool {
    match lod {
        CharacterLod::Near | CharacterLod::Mid => true,
        // Preserve the head's eyes and mouth at the far tier while dropping
        // subpixel brows and seam cores. Silhouette appendages remain present.
        CharacterLod::Far => {
            !matches!(part.feature, Feature::Brow(_))
                && !matches!(part.feature, Feature::Cheek(_))
                && !matches!(part.tint, character::Tint::Seam)
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "dev-showcase", derive(serde::Serialize))]
#[cfg_attr(not(feature = "dev-showcase"), allow(dead_code))]
pub(super) struct CharacterStats {
    pub characters: usize,
    pub instances: usize,
    pub draws: usize,
    pub triangles: usize,
    pub upload_bytes: usize,
    pub mesh_count: usize,
    pub mesh_uploads: usize,
    pub mesh_bytes: usize,
    pub resident_bytes: usize,
    pub staging_capacity_bytes: usize,
    pub culled: usize,
    pub lod_near: usize,
    pub lod_mid: usize,
    pub lod_far: usize,
    pub effects: usize,
}

pub(super) struct CharacterRenderer {
    bodies: Vec<CompiledBody>,
    meshes: Vec<Mesh>,
    batches: Vec<Batch>,
    instances: Vec<CharacterInstance>,
    buffer: wgpu::Buffer,
    opaque: wgpu::RenderPipeline,
    face: wgpu::RenderPipeline,
    effects: wgpu::RenderPipeline,
    pub stats: CharacterStats,
    pub(super) hero_study: super::hero_character::Study,
}

impl CharacterRenderer {
    pub fn new(device: &wgpu::Device, globals: &wgpu::BindGroupLayout, samples: u32) -> Self {
        let mut meshes = Vec::new();
        let mut recipes = Vec::new();
        let mut batches: Vec<Batch> = Vec::new();
        let mut bodies = Vec::new();
        let mut mesh_bytes = 0;
        let mut cache = RoundedMeshCache::new(MAX_MESHES);
        for body in BodyId::ALL {
            for outfit in OutfitId::ALL {
                let recipe = body_recipe(body);
                recipe.rig.validate().expect("bundled rig");
                let mut parts: [Vec<(Part, usize)>; 3] = std::array::from_fn(|_| Vec::new());
                for lod in CharacterLod::ALL {
                    let pieces = character::parts_for(&recipe, outfit);
                    assert!(
                        pieces.len() <= MAX_PARTS,
                        "character catalog entry exceeds MAX_PARTS: body={:?} outfit={:?} parts={}",
                        body,
                        outfit,
                        pieces.len()
                    );
                    for part in pieces.into_iter().filter(|part| part_visible(*part, lod)) {
                        let mesh_recipe = character::mesh_recipe_with_subdivisions(
                            if part.shape == super::hero_geometry::Shape::Rounded {
                                part.spec
                            } else {
                                crate::character::BodyPart::new(Vec3::ONE, 0.0)
                            },
                            lod.subdivisions(),
                        );
                        let mesh_index = recipes
                            .iter()
                            .position(|key| *key == (mesh_recipe, part.shape))
                            .unwrap_or_else(|| {
                                assert!(meshes.len() < MAX_MESHES);
                                let mesh = if part.shape == super::hero_geometry::Shape::Rounded {
                                    cache.get_or_build(mesh_recipe).expect("bundled mesh")
                                } else {
                                    std::sync::Arc::new(super::hero_geometry::build(
                                        part.shape,
                                        Vec3::ONE,
                                        lod.subdivisions(),
                                    ))
                                };
                                let vertices: Vec<_> = mesh
                                    .vertices
                                    .iter()
                                    .map(|v| CharacterVertex {
                                        position: v.position.to_array(),
                                        normal: v.normal.to_array(),
                                        uv: v.uv,
                                    })
                                    .collect();
                                mesh_bytes += size_of_val(vertices.as_slice())
                                    + size_of_val(mesh.indices.as_slice());
                                assert!(mesh_bytes < MAX_RESIDENCY);
                                meshes.push(Mesh {
                                    vertices: device.create_buffer_init(
                                        &wgpu::util::BufferInitDescriptor {
                                            label: Some("immutable character vertices"),
                                            contents: bytemuck::cast_slice(&vertices),
                                            usage: wgpu::BufferUsages::VERTEX,
                                        },
                                    ),
                                    indices: device.create_buffer_init(
                                        &wgpu::util::BufferInitDescriptor {
                                            label: Some("immutable character indices"),
                                            contents: bytemuck::cast_slice(&mesh.indices),
                                            usage: wgpu::BufferUsages::INDEX,
                                        },
                                    ),
                                    index_count: mesh.indices.len() as u32,
                                });
                                recipes.push((mesh_recipe, part.shape));
                                meshes.len() - 1
                            });
                        let material = part.tint.material();
                        let batch = batches
                            .iter()
                            .position(|b| b.mesh == mesh_index && b.material == material)
                            .unwrap_or_else(|| {
                                batches.push(Batch {
                                    mesh: mesh_index,
                                    material,
                                    instances: Vec::new(),
                                    start: 0,
                                });
                                batches.len() - 1
                            });
                        parts[lod.index()].push((part, batch));
                    }
                }
                bodies.push(CompiledBody {
                    body,
                    outfit,
                    recipe,
                    parts,
                });
            }
        }
        // Reserve each batch for the worst single-body crowd, not for an
        // arbitrary float recipe or unbounded stream of appearance changes.
        for (index, batch) in batches.iter_mut().enumerate() {
            let per_body = bodies
                .iter()
                .flat_map(|b| b.parts.iter())
                .map(|parts| parts.iter().filter(|(_, i)| *i == index).count())
                .max()
                .unwrap_or(0);
            batch.instances.reserve_exact(per_body * MAX_CHARACTERS);
        }
        let buffer_bytes = MAX_CHARACTERS * MAX_PARTS * size_of::<CharacterInstance>();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("reusable character instances"),
            size: buffer_bytes as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let stats = CharacterStats {
            mesh_bytes,
            mesh_count: meshes.len(),
            mesh_uploads: meshes.len(),
            resident_bytes: mesh_bytes + buffer_bytes,
            staging_capacity_bytes: buffer_bytes
                + batches
                    .iter()
                    .map(|b| b.instances.capacity() * size_of::<CharacterInstance>())
                    .sum::<usize>(),
            ..Default::default()
        };
        assert!(stats.resident_bytes < MAX_RESIDENCY);
        Self {
            hero_study: super::hero_character::Study::Everyday,
            bodies,
            meshes,
            batches,
            instances: Vec::with_capacity(MAX_CHARACTERS * MAX_PARTS),
            buffer,
            opaque: character_material::pipeline(device, globals, samples, CharacterPass::Opaque),
            face: character_material::pipeline(device, globals, samples, CharacterPass::Face),
            effects: character_material::pipeline(device, globals, samples, CharacterPass::Effect),
            stats,
        }
    }

    pub fn begin(&mut self) {
        self.instances.clear();
        for batch in &mut self.batches {
            batch.instances.clear();
        }
        self.stats.characters = 0;
        self.stats.instances = 0;
        self.stats.draws = 0;
        self.stats.triangles = 0;
        self.stats.upload_bytes = 0;
        self.stats.culled = 0;
        self.stats.lod_near = 0;
        self.stats.lod_mid = 0;
        self.stats.lod_far = 0;
        self.stats.effects = 0;
    }

    #[allow(dead_code)]
    pub fn add(&mut self, entity: RenderEntity, style: AvatarStyle, face: [f32; 4]) {
        self.add_with_quality(
            entity,
            style,
            face,
            // Direct validation/capture callers do not have a camera
            // projection. Mid is the stable compatibility tier; the gameplay
            // draw path always supplies the projected-size decision.
            CharacterLod::Mid,
            0,
            false,
        );
    }

    pub fn add_with_quality(
        &mut self,
        entity: RenderEntity,
        style: AvatarStyle,
        face: [f32; 4],
        lod: CharacterLod,
        effect_rank: usize,
        reduced_effects: bool,
    ) {
        if self.stats.characters >= MAX_CHARACTERS
            || !entity.camera_fade.is_finite()
            || entity.camera_fade >= 1.0
            || !Vec3::from_array(entity.position).is_finite()
            || !entity.yaw.is_finite()
            || !entity.walk_cycle.is_finite()
            || ![
                entity.face.eye_opening,
                entity.face.eye_asymmetry,
                entity.face.brow_asymmetry,
                entity.face.look.x,
                entity.face.look.y,
                entity.face.brow_tilt,
                entity.face.mouth_curve,
                entity.face.mouth_opening,
                entity.secondary.tail_sway,
                entity.secondary.ear_tilt,
                entity.secondary.wing_flap,
                entity.secondary.gap_expansion,
                entity.secondary.spark_life,
                entity.secondary.cloth_sway,
                entity.secondary.stride_blend,
                entity.secondary.landing_compression,
            ]
            .iter()
            .all(|value| value.is_finite())
        {
            return;
        }
        let body = self
            .bodies
            .iter()
            .find(|candidate| candidate.body == entity.body && candidate.outfit == entity.outfit)
            .or_else(|| self.bodies.first())
            .expect("bundled character catalog");
        let pose = if is_hero(entity) {
            super::hero_character::fit_pose(entity, self.hero_study, &body.recipe.rig)
        } else {
            entity.pose
        };
        let joints = body.recipe.rig.world_matrices(&pose.transforms);
        let root = Mat4::from_rotation_translation(
            Quat::from_rotation_y(entity.yaw),
            Vec3::from_array(entity.position),
        );
        let mut effect_count = 0;
        for (part, index) in &body.parts[lod.index()] {
            let part = &if is_hero(entity) {
                super::hero_character::study_part(*part, self.hero_study)
            } else {
                *part
            };
            if is_hero(entity)
                && self.hero_study == super::hero_character::Study::Everyday
                && matches!(part.feature, Feature::Brow(_))
            {
                continue;
            }
            if matches!(part.feature, Feature::Spark(_)) && entity.secondary.spark_life <= 0.0 {
                continue;
            }
            if matches!(part.tint, character::Tint::Seam) {
                if self.stats.effects >= character_quality::MAX_EFFECTS
                    || !character_quality::admit_effect(
                        effect_rank,
                        effect_count,
                        lod,
                        reduced_effects,
                    )
                {
                    continue;
                }
                effect_count += 1;
                self.stats.effects += 1;
            }
            let batch = &mut self.batches[*index];
            let transform =
                root * joints[part.anchor.joint.index()] * feature_transform(*part, entity);
            let mut tint = part.tint.color(style, face).map(|v| {
                if v.is_finite() {
                    v.clamp(0.0, 1.0)
                } else {
                    0.5
                }
            });
            if matches!(part.feature, Feature::Spark(_)) {
                tint[3] *= (entity.secondary.spark_life / 0.36).clamp(0.0, 1.0);
            } else if matches!(part.feature, Feature::Seam(_)) {
                tint[3] *= 0.62 + entity.secondary.gap_expansion.clamp(0.0, 0.72) * 0.42;
            }
            if batch.material != Material::Seam {
                tint[3] = 1.0;
            }
            tint[3] *= 1.0 - entity.camera_fade.clamp(0.0, 1.0);
            let mut instance = CharacterInstance::new(transform, tint, batch.material);
            if is_hero(entity) {
                match part.tint {
                    character::Tint::Shirt => {
                        instance.material = [
                            0.9,
                            0.035,
                            0.0,
                            if part.shape == super::hero_geometry::Shape::Sleeve {
                                17.0
                            } else if part.shape == super::hero_geometry::Shape::Hood {
                                12.0
                            } else if part.shape == super::hero_geometry::Shape::Pocket {
                                14.0
                            } else if part.shape == super::hero_geometry::Shape::Rib {
                                16.0
                            } else {
                                8.0
                            },
                        ]
                    }
                    character::Tint::Skin => instance.material = [0.65, 0.06, 0.0, 11.0],
                    character::Tint::Shoes => instance.material = [0.8, 0.04, 0.0, 15.0],
                    character::Tint::Blush => {
                        instance.tint = [
                            style.skin[0] * 0.96,
                            style.skin[1] * 0.85,
                            style.skin[2] * 0.80,
                            tint[3],
                        ]
                    }
                    character::Tint::Hair => {
                        instance.tint = [0.31, 0.14, 0.065, tint[3]];
                        instance.material = [0.74, 0.045, 0.0, 13.0];
                    }
                    _ => {}
                }
                if part.shape == super::hero_geometry::Shape::Sleeve {
                    let elbow = if part.anchor.joint == crate::character::JointId::LeftUpperArm {
                        crate::character::JointId::LeftLowerArm
                    } else {
                        crate::character::JointId::RightLowerArm
                    };
                    let turn = pose.transforms[elbow.index()].rotation.to_scaled_axis();
                    // Unused normal-row w lanes carry a bounded elbow axis-angle.
                    // Layout and inverse-transpose xyz lanes stay unchanged.
                    instance.normal[0][3] = turn.x;
                    instance.normal[1][3] = turn.y;
                    instance.normal[2][3] = turn.z;
                }
            }
            match part.feature {
                Feature::Eye(side) => {
                    instance.material = if is_hero(entity) {
                        [
                            if self.hero_study == super::hero_character::Study::SoftShoulders {
                                1.0
                            } else {
                                0.0
                            },
                            0.0,
                            -(entity.face.eye_opening + side * entity.face.eye_asymmetry)
                                .clamp(0.05, 1.25),
                            9.0,
                        ]
                    } else {
                        [1.0, 0.0, 0.0, 4.0]
                    }
                }
                Feature::Mouth => {
                    let opening = if is_hero(entity) {
                        (entity.face.mouth_opening - 0.18).max(0.0)
                    } else {
                        entity.face.mouth_opening
                    };
                    instance.material = [entity.face.mouth_curve, opening, 0.0, 5.0];
                    if is_hero(entity) {
                        instance.tint = [0.13, 0.085, 0.065, tint[3]];
                    }
                }
                Feature::Brow(_) => {
                    instance.material = [1.0, 0.0, 0.0, if is_hero(entity) { 10.0 } else { 6.0 }];
                    if is_hero(entity) {
                        instance.tint = [0.19, 0.095, 0.04, tint[3]];
                    }
                }
                Feature::Cheek(_) => instance.material = [1.0, 0.0, 0.0, 7.0],
                _ => {}
            }
            batch.instances.push(instance);
        }
        self.stats.characters += 1;
        match lod {
            CharacterLod::Near => self.stats.lod_near += 1,
            CharacterLod::Mid => self.stats.lod_mid += 1,
            CharacterLod::Far => self.stats.lod_far += 1,
        }
    }

    #[cfg(all(feature = "dev-showcase", not(target_arch = "wasm32")))]
    pub(super) fn make_silhouette(&mut self) {
        // Review the opaque contour without faces, material highlights, or
        // additive magic. Emissive black avoids even the shader's rim light.
        for batch in &mut self.batches {
            if matches!(batch.material, Material::Face | Material::Seam) {
                batch.instances.clear();
            } else {
                for instance in &mut batch.instances {
                    instance.tint = [0.0, 0.0, 0.0, 1.0];
                    let shape = if instance.material[3] == 17.0 {
                        17.0
                    } else {
                        0.0
                    };
                    instance.material = [1.0, 0.0, 1.0, shape];
                }
            }
        }
        self.stats.effects = 0;
    }

    pub fn upload(&mut self, queue: &wgpu::Queue) {
        for batch in &mut self.batches {
            batch.start = self.instances.len();
            self.instances.extend_from_slice(&batch.instances);
            if !batch.instances.is_empty() {
                self.stats.draws += 1;
                self.stats.triangles +=
                    self.meshes[batch.mesh].index_count as usize / 3 * batch.instances.len();
            }
        }
        self.stats.instances = self.instances.len();
        self.stats.staging_capacity_bytes = (self.instances.capacity()
            + self
                .batches
                .iter()
                .map(|b| b.instances.capacity())
                .sum::<usize>())
            * size_of::<CharacterInstance>();
        self.stats.upload_bytes = size_of_val(self.instances.as_slice());
        if !self.instances.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.instances));
        }
    }

    #[cfg(feature = "dev-showcase")]
    pub(super) fn add_effect_probe(&mut self) {
        // An enlarged copy of a bundled seam, deliberately in front of the
        // middle head, makes depth-write/occlusion tests non-vacuous.
        let batch = self
            .batches
            .iter_mut()
            .find(|b| b.material == Material::Seam)
            .unwrap();
        batch.instances.push(CharacterInstance::new(
            Mat4::from_scale_rotation_translation(
                Vec3::splat(4.0),
                Quat::IDENTITY,
                Vec3::new(0.0, 1.72, -0.9),
            ),
            [0.28, 0.95, 0.87, 1.0],
            Material::Seam,
        ));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, kind: CharacterPass) {
        pass.set_pipeline(match kind {
            CharacterPass::Opaque => &self.opaque,
            CharacterPass::Face => &self.face,
            CharacterPass::Effect => &self.effects,
        });
        for batch in &self.batches {
            if batch.instances.is_empty() || batch.material.pass() != kind {
                continue;
            }
            let mesh = &self.meshes[batch.mesh];
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            // Slice instead of nonzero first_instance for WebGL/downlevel.
            let start = (batch.start * size_of::<CharacterInstance>()) as u64;
            let end = start + size_of_val(batch.instances.as_slice()) as u64;
            pass.set_vertex_buffer(1, self.buffer.slice(start..end));
            pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..batch.instances.len() as u32);
        }
    }
}
