use super::*;

pub(super) struct Mesh {
    pub(super) vertices: wgpu::Buffer,
    pub(super) indices: wgpu::Buffer,
    pub(super) index_count: u32,
    pub(super) surfaces: Vec<MorphPackSurface>,
}

pub(super) struct RegisteredMorph {
    attachment: MorphPackAttachment,
    pub(super) mode: MorphPackAttachmentMode,
    pub(super) kind: MorphAssetKind,
    pub(super) is_base: bool,
    coverage: Vec<String>,
    lods: [Mesh; 3],
    pub(super) skinned_lods: Option<[MorphPackLod; 3]>,
    pub(super) canonical_rest: bool,
    pub(super) authored_static_face: bool,
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

pub(super) struct SkinnedBatch {
    pub(super) asset_id: MorphAssetId,
    lod: usize,
    pub(super) vertices: Vec<CharacterVertex>,
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
pub(super) struct MorphRegistry {
    pub(super) assets: BTreeMap<MorphAssetId, RegisteredMorph>,
    pub(super) resident_bytes: usize,
    batches: BTreeMap<(MorphAssetId, usize, usize), Vec<CharacterInstance>>,
    instances: Vec<CharacterInstance>,
    draws: Vec<MorphDraw>,
    pub(super) skinned_batches: Vec<SkinnedBatch>,
    skinned_draws: Vec<SkinnedDraw>,
    skinned_vertices: Vec<CharacterVertex>,
    skinned_buffer: Option<wgpu::Buffer>,
    buffer: wgpu::Buffer,
}

impl MorphRegistry {
    pub(super) fn new(device: &wgpu::Device) -> Self {
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

    pub(super) fn register(
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
                super::super::device::create_world_texture_bind_group(
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

    pub(super) fn begin(&mut self) {
        self.instances.clear();
        self.draws.clear();
        self.skinned_batches.clear();
        self.skinned_draws.clear();
        self.skinned_vertices.clear();
        for batch in self.batches.values_mut() {
            batch.clear();
        }
    }

    pub(super) fn add_instance(
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

    pub(super) fn upload(&mut self, queue: &wgpu::Queue) {
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

    pub(super) fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        solid_pipeline: &wgpu::RenderPipeline,
        textured_pipeline: &wgpu::RenderPipeline,
        shadow_bind_group: &wgpu::BindGroup,
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
                pass.set_bind_group(2, shadow_bind_group, &[]);
            } else {
                pass.set_pipeline(solid_pipeline);
                pass.set_bind_group(1, shadow_bind_group, &[]);
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
                pass.set_bind_group(2, shadow_bind_group, &[]);
            } else {
                pass.set_pipeline(solid_pipeline);
                pass.set_bind_group(1, shadow_bind_group, &[]);
            }
            pass.draw_indexed(
                surface.index_start..surface.index_start + surface.index_count,
                0,
                0..1,
            );
        }
    }

    pub(super) fn draw_shadow(&self, pass: &mut wgpu::RenderPass<'_>, pipeline: &wgpu::RenderPipeline) {
        if self.draws.is_empty() && self.skinned_draws.is_empty() {
            return;
        }
        pass.set_pipeline(pipeline);
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
            pass.draw_indexed(
                surface.index_start..surface.index_start + surface.index_count,
                0,
                0..1,
            );
        }
    }

    pub(super) fn is_skinned_base(&self, asset_id: &MorphAssetId) -> bool {
        self.assets
            .get(asset_id)
            .is_some_and(|asset| asset.mode == MorphPackAttachmentMode::Skinned && asset.is_base)
    }

    pub(super) fn has_skinned_coverage(&self, asset_ids: &[MorphAssetId], coverage: &[&str]) -> bool {
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
