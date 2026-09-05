//! Fixed, renderer-owned catalog. All three bundled bodies are compiled and
//! uploaded before first use; changing colors/worlds cannot grow this cache.
use super::{
    AvatarStyle, RenderEntity,
    character::{self, Feature, Part},
    character_material::{self, CharacterInstance, CharacterPass, CharacterVertex, Material},
    rounded_geometry::RoundedMeshCache,
};
use crate::character::{BodyId, BodyRecipe, OutfitId, body_recipe};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

pub(super) const MAX_CHARACTERS: usize = 50;
const MAX_PARTS: usize = 48;
const MAX_MESHES: usize = 256;
const MAX_RESIDENCY: usize = 32 * 1024 * 1024;

fn feature_transform(part: Part, entity: RenderEntity) -> Mat4 {
    let face = entity.face.clamped();
    let mut local = part.anchor.local;
    match part.feature {
        Feature::None => {}
        Feature::Eye(_side) => {
            local = local
                * Mat4::from_translation(glam::Vec3::new(face.look.x, face.look.y, 0.0))
                * Mat4::from_scale(glam::Vec3::new(1.0, face.eye_opening, 1.0));
        }
        Feature::Brow(side) => {
            local = local * Mat4::from_quat(Quat::from_rotation_z(side * face.brow_tilt));
        }
        Feature::Mouth => {
            local = local
                * Mat4::from_translation(glam::Vec3::new(0.0, face.mouth_curve * 0.025, 0.0))
                * Mat4::from_quat(Quat::from_rotation_z(face.mouth_curve * 0.18))
                * Mat4::from_scale(glam::Vec3::new(
                    1.0 + face.mouth_opening * 0.22,
                    1.0 + face.mouth_opening * 1.6,
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
    }
    if matches!(part.tint, character::Tint::Seam) {
        let gap = 1.0 + entity.secondary.gap_expansion.clamp(0.0, 0.72) * 0.35;
        local = local * Mat4::from_scale(Vec3::splat(gap));
    }
    local
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
    parts: Vec<(Part, usize)>,
}
struct Batch {
    mesh: usize,
    material: Material,
    instances: Vec<CharacterInstance>,
    start: usize,
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
                let pieces = character::parts_for(&recipe, outfit);
                assert!(pieces.len() <= MAX_PARTS);
                let mut parts = Vec::with_capacity(pieces.len());
                for part in pieces {
                    let mesh_recipe = character::mesh_recipe(part.spec);
                    let mesh_index = recipes
                        .iter()
                        .position(|key| *key == mesh_recipe)
                        .unwrap_or_else(|| {
                            assert!(meshes.len() < MAX_MESHES);
                            let mesh = cache.get_or_build(mesh_recipe).expect("bundled mesh");
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
                            recipes.push(mesh_recipe);
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
                    parts.push((part, batch));
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
                .map(|b| b.parts.iter().filter(|(_, i)| *i == index).count())
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
    }

    pub fn add(&mut self, entity: RenderEntity, style: AvatarStyle, face: [f32; 4]) {
        if self.stats.characters >= MAX_CHARACTERS
            || !Vec3::from_array(entity.position).is_finite()
            || !entity.yaw.is_finite()
            || !entity.walk_cycle.is_finite()
            || ![
                entity.face.eye_opening,
                entity.face.look.x,
                entity.face.look.y,
                entity.face.brow_tilt,
                entity.face.mouth_curve,
                entity.face.mouth_opening,
                entity.secondary.tail_sway,
                entity.secondary.ear_tilt,
                entity.secondary.wing_flap,
                entity.secondary.gap_expansion,
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
        let pose = entity.pose;
        let joints = body.recipe.rig.world_matrices(&pose.transforms);
        let root = Mat4::from_rotation_translation(
            Quat::from_rotation_y(entity.yaw),
            Vec3::from_array(entity.position),
        );
        for (part, index) in &body.parts {
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
            if batch.material != Material::Seam {
                tint[3] = 1.0;
            }
            batch
                .instances
                .push(CharacterInstance::new(transform, tint, batch.material));
        }
        self.stats.characters += 1;
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
