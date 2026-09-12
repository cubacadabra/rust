//! Renderer-owned character resources. Bundled bodies remain a fixed catalog;
//! authored morph packs use a separate bounded registry.
use super::{
    AvatarStyle, RenderEntity,
    character::{self, Feature, Part},
    character_material::{self, CharacterInstance, CharacterPass, CharacterVertex, Material},
    character_quality::{self, CharacterLod},
    rounded_geometry::RoundedMeshCache,
};
use crate::character::{BodyId, BodyRecipe, JointId, OutfitId, body_recipe};
use cubacadabra_morphs::{
    MAX_MORPH_PACK_SURFACES, MorphAssetId, MorphAssetKind, MorphDiagnostic, MorphPack,
    MorphPackAttachment, MorphPackAttachmentMode, MorphPackLod, MorphPackSurface,
};
use glam::{Mat3, Mat4, Quat, Vec3};
use std::collections::BTreeMap;
use wgpu::util::DeviceExt;

pub(super) const MAX_CHARACTERS: usize = 50;
// Keep the original garment budget plus bounded authored hair geometry.
const MAX_PARTS: usize = 48 + crate::character::hair::MAX_LOCKS;
pub(super) const MAX_MESHES: usize = 384 + 2 * crate::character::hair::MAX_LOCKS * 3;
const MAX_RESIDENCY: usize = 32 * 1024 * 1024;
const MAX_MORPH_PACKS: usize = 32;
// The reusable starter wardrobe includes 20 baked, three-LOD assets. Keep a
// bounded registry large enough for the complete catalog, not just one outfit.
// This is a residency ceiling, not a preallocation or per-frame skinning budget.
const MAX_MORPH_RESIDENCY: usize = 64 * 1024 * 1024;
const MAX_SKINNED_VERTICES: usize = MAX_CHARACTERS * 8192;
const MAX_MORPH_INSTANCES: usize = MAX_CHARACTERS * 16 * MAX_MORPH_PACK_SURFACES;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "character_morph_review.rs"]
mod morph_review;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "character_starter_review.rs"]
mod starter_review;

fn feature_transform(part: Part, entity: RenderEntity) -> Mat4 {
    let face = entity.face.clamped();
    let mut local = part.anchor.local;
    match part.feature {
        Feature::None | Feature::Sole | Feature::GarmentUnderlayer => {}
        Feature::Cloth => {
            let pivot = Vec3::Y * part.spec.size.y * 0.42;
            local = local
                * Mat4::from_translation(pivot)
                * Mat4::from_rotation_x(entity.secondary.cloth_sway)
                * Mat4::from_translation(-pivot);
        }
        Feature::Eye(side) => {
            let look = face.look * if is_hero(entity) { 0.45 } else { 1.0 };
            local = local
                // Keep the eye card just ahead of the rounded head. This is
                // especially important for dark skin, where an occluded
                // sclera would otherwise collapse back to a black slit.
                * Mat4::from_translation(glam::Vec3::new(0.0, 0.0, -0.028))
                * Mat4::from_translation(glam::Vec3::new(look.x, look.y, 0.0))
                * Mat4::from_scale(glam::Vec3::new(
                    1.0,
                    (face.eye_opening + side * face.eye_asymmetry)
                        .clamp(if is_hero(entity) { 0.20 } else { 0.05 }, 1.25),
                    1.0,
                ));
        }
        Feature::Brow(side) => {
            local = local
                * Mat4::from_translation(Vec3::Y * side * face.brow_asymmetry)
                * Mat4::from_quat(Quat::from_rotation_z(side * face.brow_tilt));
        }
        Feature::Mouth | Feature::MouthEdge(_) => {
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
    if let super::hero_geometry::Shape::HairCurve(style, index) = part.shape {
        let sway = crate::character::hair::get(style).locks[index as usize].sway;
        // Curved locks are modeled relative to their buried root, so the
        // cap stays fixed and each lock follows head motion from its root.
        local *= Mat4::from_quat(Quat::from_scaled_axis(entity.secondary.hair_sway * sway));
    } else if is_hero(entity) && part.shape == super::hero_geometry::Shape::HairLock {
        let (_, orientation, _) = local.to_scale_rotation_translation();
        let turn = orientation.conjugate() * entity.secondary.hair_sway;
        let pivot = Vec3::NEG_Y * part.spec.size.y * 0.5;
        local = local
            * Mat4::from_translation(pivot)
            * Mat4::from_quat(Quat::from_scaled_axis(turn))
            * Mat4::from_translation(-pivot);
    } else if is_hero(entity) && part.shape == super::hero_geometry::Shape::Cord {
        let pivot = Vec3::Y * part.spec.size.y * 0.5;
        local = local
            * Mat4::from_translation(pivot)
            * Mat4::from_rotation_x(entity.secondary.cloth_sway * 1.3)
            * Mat4::from_rotation_z(entity.secondary.hair_sway.z * 0.6)
            * Mat4::from_translation(-pivot);
    }
    if part.shape != super::hero_geometry::Shape::Rounded {
        local *= Mat4::from_scale(part.spec.size);
    }
    local
}

fn is_hero(entity: RenderEntity) -> bool {
    entity.body.is_person() && entity.outfit == OutfitId::EverydayHoodie
}

struct Mesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    surfaces: Vec<MorphPackSurface>,
}

struct RegisteredMorph {
    attachment: MorphPackAttachment,
    mode: MorphPackAttachmentMode,
    kind: MorphAssetKind,
    is_base: bool,
    coverage: Vec<String>,
    lods: [Mesh; 3],
    skinned_lods: Option<[MorphPackLod; 3]>,
    canonical_rest: bool,
    authored_static_face: bool,
    textures: Vec<wgpu::BindGroup>,
    bytes: usize,
}

struct MorphDraw {
    asset_id: MorphAssetId,
    lod: usize,
    surface: usize,
    start: usize,
    count: usize,
}

struct SkinnedBatch {
    asset_id: MorphAssetId,
    lod: usize,
    vertices: Vec<CharacterVertex>,
    instances: Vec<CharacterInstance>,
}

struct SkinnedDraw {
    asset_id: MorphAssetId,
    lod: usize,
    vertex_start: usize,
    vertex_count: usize,
    instance_start: usize,
    surface: usize,
}

/// GPU resources for compiled morphs. This registry is intentionally
/// separate from the bundled V1 character catalog: registering content does
/// not rebuild or grow the fixed character batches.
struct MorphRegistry {
    assets: BTreeMap<MorphAssetId, RegisteredMorph>,
    resident_bytes: usize,
    batches: BTreeMap<(MorphAssetId, usize, usize), Vec<CharacterInstance>>,
    instances: Vec<CharacterInstance>,
    draws: Vec<MorphDraw>,
    skinned_batches: Vec<SkinnedBatch>,
    skinned_draws: Vec<SkinnedDraw>,
    skinned_vertices: Vec<CharacterVertex>,
    skinned_buffer: Option<wgpu::Buffer>,
    buffer: wgpu::Buffer,
}

impl MorphRegistry {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            assets: BTreeMap::new(),
            resident_bytes: 0,
            batches: BTreeMap::new(),
            instances: Vec::with_capacity(MAX_MORPH_INSTANCES),
            draws: Vec::with_capacity(MAX_CHARACTERS * 16),
            skinned_batches: Vec::with_capacity(MAX_CHARACTERS),
            skinned_draws: Vec::with_capacity(MAX_CHARACTERS),
            skinned_vertices: Vec::with_capacity(MAX_SKINNED_VERTICES),
            skinned_buffer: None,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("reusable morph instances"),
                size: (MAX_MORPH_INSTANCES * size_of::<CharacterInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }

    fn register(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_layout: &wgpu::BindGroupLayout,
        pack: MorphPack,
    ) -> Result<(), Vec<MorphDiagnostic>> {
        let id = pack.asset.id.clone();
        for (index, lod) in pack.lods.iter().enumerate() {
            if lod.normals.len() != lod.vertices.len()
                || lod.normals.iter().any(|normal| {
                    let n = Vec3::from_array(*normal);
                    !n.is_finite() || !(0.98..=1.02).contains(&n.length_squared())
                })
            {
                return Err(vec![MorphDiagnostic {
                    code: "MORPH_GPU_INVALID_NORMALS".into(),
                    path: format!("lods[{index}].normals"),
                    message: "every vertex requires a finite authored unit normal".into(),
                }]);
            }
        }
        let lod_bytes = pack
            .lods
            .iter()
            .map(morph_lod_bytes)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| vec![morph_error("MORPH_GPU_RESOURCE_OVERFLOW", "geometry")])?;
        let geometry_bytes = lod_bytes
            .iter()
            .try_fold(0usize, |total, value| total.checked_add(*value));
        let texture_bytes = pack.textures.iter().try_fold(0usize, |total, texture| {
            total.checked_add(texture.pixels.len())
        });
        let Some(bytes) = geometry_bytes
            .and_then(|geometry| texture_bytes.and_then(|textures| geometry.checked_add(textures)))
        else {
            return Err(vec![morph_error("MORPH_GPU_RESOURCE_OVERFLOW", "geometry")]);
        };
        let previous_bytes = self.assets.get(&id).map(|asset| asset.bytes).unwrap_or(0);
        let Some(new_resident_bytes) = self
            .resident_bytes
            .checked_sub(previous_bytes)
            .and_then(|value| value.checked_add(bytes))
        else {
            return Err(vec![morph_error("MORPH_GPU_RESOURCE_OVERFLOW", "geometry")]);
        };
        if new_resident_bytes > MAX_MORPH_RESIDENCY {
            return Err(vec![MorphDiagnostic {
                code: "MORPH_GPU_RESOURCE_LIMIT".into(),
                path: "geometry".into(),
                message: format!(
                    "morph registry requires {new_resident_bytes} bytes; limit is {MAX_MORPH_RESIDENCY} bytes"
                ),
            }]);
        }
        if !self.assets.contains_key(&id) && self.assets.len() >= MAX_MORPH_PACKS {
            return Err(vec![morph_error("MORPH_GPU_PACK_LIMIT", "asset.id")]);
        }

        let mode = pack.attachment.mode;
        if mode == MorphPackAttachmentMode::Skinned && self.skinned_buffer.is_none() {
            self.skinned_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("reusable skinned morph vertices"),
                size: (MAX_SKINNED_VERTICES * size_of::<CharacterVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let skinned_lods = (mode == MorphPackAttachmentMode::Skinned).then(|| pack.lods.clone());
        let textures = pack
            .textures
            .iter()
            .map(|texture| {
                super::device::create_world_texture_bind_group(
                    device,
                    queue,
                    texture_layout,
                    u32::from(texture.width),
                    u32::from(texture.height),
                    &texture.pixels,
                )
            })
            .collect();
        let [near, mid, far] = pack.lods;
        let lods = [
            upload_morph_lod(device, &id, "near", near),
            upload_morph_lod(device, &id, "mid", mid),
            upload_morph_lod(device, &id, "far", far),
        ]
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
        let lods: [Mesh; 3] = lods
            .try_into()
            .map_err(|_| vec![morph_error("MORPH_GPU_INVALID_LOD_COUNT", "lods")])?;
        let [near, mid, far] = lods;
        self.batches.retain(|(asset_id, _, _), _| asset_id != &id);
        self.assets.insert(
            id,
            RegisteredMorph {
                attachment: pack.attachment,
                mode,
                kind: pack.asset.kind,
                is_base: pack.asset.kind == cubacadabra_morphs::MorphAssetKind::Base,
                coverage: pack.asset.coverage.clone(),
                lods: [near, mid, far],
                skinned_lods,
                canonical_rest: pack
                    .asset
                    .required_capabilities
                    .iter()
                    .any(|capability| capability.as_str() == "rig.canonical-rest.v1"),
                authored_static_face: pack
                    .asset
                    .required_capabilities
                    .iter()
                    .any(|capability| capability.as_str() == "face.authored-static.v1"),
                textures,
                bytes,
            },
        );
        self.resident_bytes = new_resident_bytes;
        Ok(())
    }

    fn begin(&mut self) {
        self.instances.clear();
        self.draws.clear();
        self.skinned_batches.clear();
        self.skinned_draws.clear();
        self.skinned_vertices.clear();
        for batch in self.batches.values_mut() {
            batch.clear();
        }
    }

    fn add_instance(
        &mut self,
        asset_id: &MorphAssetId,
        lod: CharacterLod,
        root: Mat4,
        joints: [Mat4; 15],
        style: AvatarStyle,
    ) {
        let Some(asset) = self.assets.get(asset_id) else {
            return;
        };
        let existing_instances = self.batches.values().map(Vec::len).sum::<usize>()
            + self
                .skinned_batches
                .iter()
                .map(|batch| batch.instances.len())
                .sum::<usize>();
        let surface_count = asset.lods[lod.index()].surfaces.len();
        if existing_instances.saturating_add(surface_count) > MAX_MORPH_INSTANCES {
            return;
        }
        let attachment = Mat4::from_scale_rotation_translation(
            Vec3::from_array(asset.attachment.scale),
            Quat::from_array(asset.attachment.rotation),
            Vec3::from_array(asset.attachment.translation),
        );
        if asset.mode == MorphPackAttachmentMode::Skinned {
            let Some(lod_mesh) = asset
                .skinned_lods
                .as_ref()
                .and_then(|lods| lods.get(lod.index()))
            else {
                return;
            };
            let Some(skinning) = lod_mesh.skinning.as_ref() else {
                return;
            };
            if skinning.len() != lod_mesh.vertices.len()
                || self
                    .skinned_batches
                    .iter()
                    .map(|batch| batch.vertices.len())
                    .sum::<usize>()
                    .saturating_add(lod_mesh.vertices.len())
                    > MAX_SKINNED_VERTICES
            {
                return;
            }
            let joint_normals = joints.map(Mat3::from_mat4);
            let vertices = lod_mesh
                .vertices
                .iter()
                .zip(skinning)
                .zip(&lod_mesh.normals)
                .zip(&lod_mesh.uvs)
                .map(|(((position, skin), normal), uv)| {
                    let normal = Vec3::from_array(*normal);
                    let mut skinned_position = Vec3::ZERO;
                    let mut skinned_normal = Vec3::ZERO;
                    for (joint, weight) in skin.joints.into_iter().zip(skin.weights) {
                        if weight <= 0.0 {
                            continue;
                        }
                        let matrix = joints[joint as usize];
                        skinned_position +=
                            matrix.transform_point3(Vec3::from_array(*position)) * weight;
                        skinned_normal += joint_normals[joint as usize] * normal * weight;
                    }
                    CharacterVertex {
                        position: skinned_position.to_array(),
                        normal: skinned_normal.try_normalize().unwrap_or(Vec3::Y).to_array(),
                        uv: *uv,
                    }
                })
                .collect();
            let instances = lod_mesh
                .surfaces
                .iter()
                .map(|surface| {
                    let (tint, material) = morph_surface_appearance(asset.kind, *surface, style);
                    CharacterInstance::new(root * attachment, tint, material)
                })
                .collect();
            self.skinned_batches.push(SkinnedBatch {
                asset_id: asset_id.clone(),
                lod: lod.index(),
                vertices,
                instances,
            });
            return;
        }
        let Some(joint) = morph_joint(&asset.attachment.joint) else {
            return;
        };
        for (surface_index, surface) in asset.lods[lod.index()].surfaces.iter().enumerate() {
            let (tint, material) = morph_surface_appearance(asset.kind, *surface, style);
            self.batches
                .entry((asset_id.clone(), lod.index(), surface_index))
                .or_default()
                .push(CharacterInstance::new(
                    root * joints[joint.index()] * attachment,
                    tint,
                    material,
                ));
        }
    }

    fn upload(&mut self, queue: &wgpu::Queue) {
        self.instances.clear();
        self.draws.clear();
        self.skinned_vertices.clear();
        for ((asset_id, lod, surface), batch) in &self.batches {
            if batch.is_empty() {
                continue;
            }
            let start = self.instances.len();
            self.instances.extend_from_slice(batch);
            self.draws.push(MorphDraw {
                asset_id: asset_id.clone(),
                lod: *lod,
                surface: *surface,
                start,
                count: batch.len(),
            });
        }
        for batch in &self.skinned_batches {
            let vertex_start = self.skinned_vertices.len();
            self.skinned_vertices.extend_from_slice(&batch.vertices);
            for (surface, instance) in batch.instances.iter().copied().enumerate() {
                let instance_start = self.instances.len();
                self.instances.push(instance);
                self.skinned_draws.push(SkinnedDraw {
                    asset_id: batch.asset_id.clone(),
                    lod: batch.lod,
                    vertex_start,
                    vertex_count: batch.vertices.len(),
                    instance_start,
                    surface,
                });
            }
        }
        if !self.instances.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.instances));
        }
        if !self.skinned_vertices.is_empty() {
            let Some(skinned_buffer) = self.skinned_buffer.as_ref() else {
                return;
            };
            queue.write_buffer(
                skinned_buffer,
                0,
                bytemuck::cast_slice(&self.skinned_vertices),
            );
        }
    }

    fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        solid_pipeline: &wgpu::RenderPipeline,
        textured_pipeline: &wgpu::RenderPipeline,
    ) {
        if self.draws.is_empty() && self.skinned_draws.is_empty() {
            return;
        }
        for draw in &self.draws {
            let Some(asset) = self.assets.get(&draw.asset_id) else {
                continue;
            };
            let mesh = &asset.lods[draw.lod];
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            let start = (draw.start * size_of::<CharacterInstance>()) as u64;
            let end = start + (draw.count * size_of::<CharacterInstance>()) as u64;
            pass.set_vertex_buffer(1, self.buffer.slice(start..end));
            pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            let Some(surface) = mesh.surfaces.get(draw.surface) else {
                continue;
            };
            let texture = surface
                .texture
                .and_then(|index| asset.textures.get(usize::from(index)));
            if let Some(texture) = texture {
                pass.set_pipeline(textured_pipeline);
                pass.set_bind_group(1, texture, &[]);
            } else {
                pass.set_pipeline(solid_pipeline);
            }
            pass.draw_indexed(
                surface.index_start..surface.index_start + surface.index_count,
                0,
                0..draw.count as u32,
            );
        }
        let Some(skinned_buffer) = self.skinned_buffer.as_ref() else {
            return;
        };
        for draw in &self.skinned_draws {
            let Some(asset) = self.assets.get(&draw.asset_id) else {
                continue;
            };
            let mesh = &asset.lods[draw.lod];
            let vertex_start = (draw.vertex_start * size_of::<CharacterVertex>()) as u64;
            let vertex_end =
                vertex_start + (draw.vertex_count * size_of::<CharacterVertex>()) as u64;
            pass.set_vertex_buffer(0, skinned_buffer.slice(vertex_start..vertex_end));
            let instance_start = (draw.instance_start * size_of::<CharacterInstance>()) as u64;
            let instance_end = instance_start + size_of::<CharacterInstance>() as u64;
            pass.set_vertex_buffer(1, self.buffer.slice(instance_start..instance_end));
            pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            let Some(surface) = mesh.surfaces.get(draw.surface) else {
                continue;
            };
            let texture = surface
                .texture
                .and_then(|index| asset.textures.get(usize::from(index)));
            if let Some(texture) = texture {
                pass.set_pipeline(textured_pipeline);
                pass.set_bind_group(1, texture, &[]);
            } else {
                pass.set_pipeline(solid_pipeline);
            }
            pass.draw_indexed(
                surface.index_start..surface.index_start + surface.index_count,
                0,
                0..1,
            );
        }
    }

    fn is_skinned_base(&self, asset_id: &MorphAssetId) -> bool {
        self.assets
            .get(asset_id)
            .is_some_and(|asset| asset.mode == MorphPackAttachmentMode::Skinned && asset.is_base)
    }

    fn has_skinned_coverage(&self, asset_ids: &[MorphAssetId], coverage: &[&str]) -> bool {
        asset_ids.iter().take(16).any(|asset_id| {
            self.assets.get(asset_id).is_some_and(|asset| {
                asset.mode == MorphPackAttachmentMode::Skinned
                    && coverage
                        .iter()
                        .any(|region| asset.coverage.iter().any(|value| value == region))
            })
        })
    }
}

fn morph_surface_appearance(
    kind: MorphAssetKind,
    surface: MorphPackSurface,
    style: AvatarStyle,
) -> ([f32; 4], Material) {
    let authored = surface.base_color.unwrap_or([0.7, 0.7, 0.7, 1.0]);
    let avatar_tint = match kind {
        MorphAssetKind::Base => style.skin,
        MorphAssetKind::Top | MorphAssetKind::Outerwear => style.shirt,
        MorphAssetKind::Bottom => style.pants,
        MorphAssetKind::Footwear => style.shoes,
        _ => authored,
    };
    let material = if surface.texture.is_some() {
        match kind {
            MorphAssetKind::Base
            | MorphAssetKind::Hair
            | MorphAssetKind::Top
            | MorphAssetKind::Outerwear
            | MorphAssetKind::Bottom
            | MorphAssetKind::Footwear => Material::AuthoredTexture(kind),
            _ => Material::Textured,
        }
    } else {
        match kind {
            MorphAssetKind::Hair => Material::Hair,
            MorphAssetKind::Top | MorphAssetKind::Outerwear => Material::Cloth,
            MorphAssetKind::Bottom => Material::Denim,
            MorphAssetKind::Footwear => Material::Rubber,
            _ => Material::Toy,
        }
    };
    (
        if surface.use_avatar_tint {
            avatar_tint
        } else {
            authored
        },
        material,
    )
}

fn morph_lod_bytes(lod: &MorphPackLod) -> Option<usize> {
    size_of::<CharacterVertex>()
        .checked_mul(lod.vertices.len())?
        .checked_add(size_of::<u32>().checked_mul(lod.indices.len())?)
}

fn upload_morph_lod(
    device: &wgpu::Device,
    id: &MorphAssetId,
    level: &str,
    lod: MorphPackLod,
) -> Result<Mesh, Vec<MorphDiagnostic>> {
    let surfaces = lod.surfaces.clone();
    let vertices = lod
        .vertices
        .into_iter()
        .zip(lod.normals)
        .zip(lod.uvs)
        .map(|((position, normal), uv)| CharacterVertex {
            position,
            normal,
            uv,
        })
        .collect::<Vec<_>>();
    let index_count = u32::try_from(lod.indices.len()).map_err(|_| {
        vec![morph_error(
            "MORPH_GPU_INDEX_COUNT_OVERFLOW",
            &format!("{id}.{level}.indices"),
        )]
    })?;
    Ok(Mesh {
        vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("morph vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }),
        indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("morph indices"),
            contents: bytemuck::cast_slice(&lod.indices),
            usage: wgpu::BufferUsages::INDEX,
        }),
        index_count,
        surfaces,
    })
}

fn morph_error(code: &str, path: &str) -> MorphDiagnostic {
    MorphDiagnostic {
        code: code.to_owned(),
        path: path.to_owned(),
        message: "compiled morph exceeds the renderer resource limit".to_owned(),
    }
}

fn morph_joint(name: &str) -> Option<JointId> {
    Some(match name {
        "root" => JointId::Root,
        "torso" => JointId::Torso,
        "head" => JointId::Head,
        "left-upper-arm" => JointId::LeftUpperArm,
        "left-lower-arm" => JointId::LeftLowerArm,
        "left-hand" => JointId::LeftHand,
        "right-upper-arm" => JointId::RightUpperArm,
        "right-lower-arm" => JointId::RightLowerArm,
        "right-hand" => JointId::RightHand,
        "left-upper-leg" => JointId::LeftUpperLeg,
        "left-lower-leg" => JointId::LeftLowerLeg,
        "left-foot" => JointId::LeftFoot,
        "right-upper-leg" => JointId::RightUpperLeg,
        "right-lower-leg" => JointId::RightLowerLeg,
        "right-foot" => JointId::RightFoot,
        _ => return None,
    })
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
    morphs: MorphRegistry,
    batches: Vec<Batch>,
    instances: Vec<CharacterInstance>,
    buffer: wgpu::Buffer,
    opaque: wgpu::RenderPipeline,
    textured: wgpu::RenderPipeline,
    face: wgpu::RenderPipeline,
    effects: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    pub stats: CharacterStats,
    pub(super) hero_study: super::hero_character::Study,
    #[cfg(feature = "dev-showcase")]
    pub(super) head_only: bool,
}

impl CharacterRenderer {
    pub fn new(device: &wgpu::Device, globals: &wgpu::BindGroupLayout, samples: u32) -> Self {
        let texture_layout = super::device::world_texture_bind_group_layout(device);
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
                    let mut pieces = character::parts_for(&recipe, outfit);
                    pieces.extend(character::garment_underlayer(body));
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
                                    surfaces: Vec::new(),
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
            #[cfg(feature = "dev-showcase")]
            head_only: false,
            bodies,
            meshes,
            morphs: MorphRegistry::new(device),
            batches,
            instances: Vec::with_capacity(MAX_CHARACTERS * MAX_PARTS),
            buffer,
            opaque: character_material::pipeline(device, globals, samples, CharacterPass::Opaque),
            textured: character_material::textured_pipeline(
                device,
                globals,
                &texture_layout,
                samples,
            ),
            face: character_material::pipeline(device, globals, samples, CharacterPass::Face),
            effects: character_material::pipeline(device, globals, samples, CharacterPass::Effect),
            texture_layout,
            stats,
        }
    }

    pub(super) fn register_morph_pack(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pack: MorphPack,
    ) -> Result<(), Vec<MorphDiagnostic>> {
        self.morphs
            .register(device, queue, &self.texture_layout, pack)
    }

    pub fn begin(&mut self) {
        self.morphs.begin();
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
            &[],
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_with_quality(
        &mut self,
        entity: RenderEntity,
        style: AvatarStyle,
        face: [f32; 4],
        lod: CharacterLod,
        effect_rank: usize,
        reduced_effects: bool,
        morph_assets: &[MorphAssetId],
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
                entity.secondary.run_blend,
                entity.secondary.landing_compression,
            ]
            .iter()
            .all(|value| value.is_finite())
            || !entity.secondary.hair_sway.is_finite()
            || [
                entity.secondary.left_foot_target,
                entity.secondary.right_foot_target,
                entity.secondary.left_ankle_target,
                entity.secondary.right_ankle_target,
            ]
            .into_iter()
            .flatten()
            .any(|target| !target.is_finite())
        {
            return;
        }
        let body = self
            .bodies
            .iter()
            .find(|candidate| candidate.body == entity.body && candidate.outfit == entity.outfit)
            .or_else(|| self.bodies.first())
            .expect("bundled character catalog");
        // Authored coordinates already target the declared biped bind pose.
        // The legacy procedural hero refit moves shoulders, hips and ankles;
        // applying it to those coordinates tears a complete authored outfit.
        let canonical_rest = morph_assets.iter().take(16).any(|id| {
            self.morphs
                .assets
                .get(id)
                .is_some_and(|asset| asset.is_base && asset.canonical_rest)
        });
        let pose = if is_hero(entity) && !canonical_rest {
            super::hero_character::fit_pose(entity, self.hero_study, &body.recipe.rig)
        } else {
            entity.pose
        };
        let joints = body.recipe.rig.world_matrices(&pose.transforms);
        let root = Mat4::from_rotation_translation(
            Quat::from_rotation_y(entity.yaw),
            Vec3::from_array(entity.position),
        );
        for morph_asset in morph_assets.iter().take(16) {
            self.morphs
                .add_instance(morph_asset, lod, root, joints, style);
        }
        let authored_base = morph_assets
            .iter()
            .take(16)
            .any(|asset_id| self.morphs.is_skinned_base(asset_id));
        // A native v2 loadout is a complete authored character. Missing packs
        // must render as missing, never as a procedural body/clothing hybrid.
        // Legacy equipment that explicitly registers an authored base gets
        // the same single-path behavior.
        if entity.authored_morph || authored_base {
            return;
        }
        let authored_hair = morph_assets.iter().take(16).any(|id| {
            id.as_str() == "cuba:hair/bald.v1"
                || self
                    .morphs
                    .assets
                    .get(id)
                    .is_some_and(|asset| asset.kind == MorphAssetKind::Hair)
        });
        let authored_top = self
            .morphs
            .has_skinned_coverage(morph_assets, &["torso", "arms"]);
        let authored_bottom = self.morphs.has_skinned_coverage(morph_assets, &["legs"]);
        let authored_footwear = self.morphs.has_skinned_coverage(morph_assets, &["feet"]);
        let authored_static_face = morph_assets.iter().take(16).any(|id| {
            self.morphs.assets.get(id).is_some_and(|asset| {
                asset.is_base
                    && asset.mode == MorphPackAttachmentMode::Skinned
                    && asset.authored_static_face
            })
        });
        let mut effect_count = 0;
        for (part, index) in &body.parts[lod.index()] {
            // An explicitly declared static art-study face owns its complete
            // graphic expression. Animated analytic faces remain the default.
            if authored_static_face
                && matches!(part.tint, character::Tint::Face | character::Tint::Blush)
            {
                continue;
            }
            if authored_hair && part.tint == character::Tint::Hair {
                continue;
            }
            if matches!(part.feature, Feature::GarmentUnderlayer)
                && (!authored_top || authored_base)
            {
                continue;
            }
            if authored_base && part.tint == character::Tint::Skin {
                continue;
            }
            if authored_top
                && (matches!(
                    part.tint,
                    character::Tint::Shirt
                        | character::Tint::Detail
                        | character::Tint::Outer
                        | character::Tint::Armor
                        | character::Tint::Fuzz
                ) || (part.tint == character::Tint::Ivory
                    && matches!(
                        part.anchor.joint,
                        crate::character::JointId::Torso
                            | crate::character::JointId::LeftUpperArm
                            | crate::character::JointId::LeftLowerArm
                            | crate::character::JointId::RightUpperArm
                            | crate::character::JointId::RightLowerArm
                    )))
            {
                continue;
            }
            if authored_bottom && part.tint == character::Tint::Pants {
                continue;
            }
            if authored_footwear
                && (part.tint == character::Tint::Shoes
                    || (part.tint == character::Tint::Ivory
                        && matches!(
                            part.anchor.joint,
                            crate::character::JointId::LeftFoot
                                | crate::character::JointId::RightFoot
                        )))
            {
                continue;
            }
            #[cfg(feature = "dev-showcase")]
            if self.head_only && part.anchor.joint != crate::character::JointId::Head {
                continue;
            }
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
                        let hair = style.body.hair_color();
                        instance.tint = [hair[0], hair[1], hair[2], tint[3]];
                        instance.material = [0.52, 0.11, 0.0, 13.0];
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
                Feature::MouthEdge(side) => {
                    let opening = if is_hero(entity) {
                        (entity.face.mouth_opening - 0.18).max(0.0)
                    } else {
                        entity.face.mouth_opening
                    };
                    instance.material = [side, entity.face.mouth_curve, opening, 18.0];
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
        self.morphs.upload(queue);
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
        if kind == CharacterPass::Opaque {
            self.morphs.draw(pass, &self.opaque, &self.textured);
        }
    }
}
