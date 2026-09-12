//! Opt-in complete starter capture through the production GPU pipelines.
//! CUBA_STARTER_CATALOG=<catalog.lock.json> CUBA_STARTER_THUMBNAILS=<directory>
//! cargo test --features studio-ui capture_starters -- --ignored
use super::super::{DEPTH_FORMAT, Globals};
use super::*;

#[path = "character_reference_stage.rs"]
mod reference_stage;

#[test]
#[ignore = "requires a GPU and a compiled starter catalog"]
fn capture_starters() {
    let catalog_path = std::path::PathBuf::from(std::env::var("CUBA_STARTER_CATALOG").unwrap());
    let output = std::path::PathBuf::from(std::env::var("CUBA_STARTER_THUMBNAILS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let lock: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&catalog_path).unwrap()).unwrap();
    let catalog: cubacadabra_morphs::MorphCatalog = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1, "contentVersion": "starter-capture",
        "assets": lock["assets"].as_array().unwrap().iter().map(|a| &a["definition"]).collect::<Vec<_>>(),
        "presets": lock["presets"],
    })).unwrap();
    assert!(catalog.validate().is_empty());
    assert!(
        !catalog.presets.is_empty(),
        "capture needs at least one complete preset"
    );
    let reference = std::env::var_os("CUBA_STARTER_REFERENCE").is_some();
    if reference {
        assert_eq!(
            catalog.presets.len(),
            1,
            "reference capture is an isolated study"
        );
        assert_eq!(
            catalog.presets[0].id.as_str(),
            "cuba:preset/mockup-person.v1"
        );
        assert_eq!(
            catalog.presets[0].loadout().base.as_str(),
            "cuba:base/study-person.v1"
        );
        assert_eq!(
            catalog.presets[0].loadout().parts.len(),
            4,
            "no extra equipment"
        );
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        ..Default::default()
    }))
    .unwrap();
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("starter capture globals"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let mut renderer = CharacterRenderer::new(&device, &layout, 1);
    for asset in lock["assets"].as_array().unwrap() {
        if let Some(hash) = asset["artifact"]["sha256"].as_str() {
            let path = catalog_path.parent().unwrap().join(format!(
                "runtime/morphs/sha256/{}/{}.morphpack",
                &hash[..2],
                hash
            ));
            let pack =
                cubacadabra_morphs::decode_morph_pack(&std::fs::read(path).unwrap()).unwrap();
            let mut invalid = pack.clone();
            invalid.lods[0].normals.pop();
            assert_eq!(
                renderer
                    .register_morph_pack(&device, &queue, invalid)
                    .unwrap_err()[0]
                    .code,
                "MORPH_GPU_INVALID_NORMALS"
            );
            renderer.register_morph_pack(&device, &queue, pack).unwrap();
        }
    }
    for preset in &catalog.presets {
        let loadout = preset.loadout();
        let legacy = cubacadabra_morphs::project_v2_to_v1(&catalog, &loadout).unwrap();
        let resolved = crate::character::resolve_appearance(crate::character::AppearanceInput {
            version: Some(1),
            body: legacy.body.as_deref(),
            face: legacy.face.as_deref(),
            outfit: legacy.outfit.as_deref(),
            equipment: &legacy.equipment,
            colors: &legacy.colors,
            legacy_colors: Default::default(),
            revision: 0,
        });
        assert!(
            resolved.issues.is_empty(),
            "{}: {:?}",
            preset.id,
            resolved.issues
        );
        let body = BodyId::from_stable_id(legacy.body.as_deref().unwrap()).unwrap();
        let recipe = body_recipe(body);
        let style = AvatarStyle {
            body,
            skin: color(&legacy.colors["skin"]),
            shirt: color(&legacy.colors["primary"]),
            pants: color(&legacy.colors["secondary"]),
            shoes: color(&legacy.colors["sole"]),
            face: resolved.appearance.face,
            ..Default::default()
        };
        if reference {
            for (key, expected) in [
                ("skin", "#b87b4e"),
                ("primary", "#8354b5"),
                ("secondary", "#24384e"),
                ("sole", "#f1ebdf"),
            ] {
                assert_eq!(legacy.colors[key], expected, "reference palette changed");
            }
            let mut parts: Vec<_> = loadout.parts.iter().map(|id| id.as_str()).collect();
            parts.sort_unstable();
            assert_eq!(
                parts,
                [
                    "cuba:bottom/study-denim.v1",
                    "cuba:footwear/study-sneakers.v1",
                    "cuba:hair/study-swept.v1",
                    "cuba:top/study-hoodie.v1"
                ]
            );
        }
        let pose_name = if reference {
            "rest".into()
        } else {
            std::env::var("CUBA_STARTER_POSE").unwrap_or_else(|_| "rest".into())
        };
        let mut pose = match pose_name.as_str() {
            "rest" => crate::character::Pose::rest(&recipe.rig),
            "walk" | "bend" | "jump" => {
                crate::character::Pose::locomotion(&recipe.rig, 1.0, true, false)
            }
            _ => panic!("unsupported review pose {pose_name}"),
        };
        if pose_name == "bend" || pose_name == "jump" {
            // Deliberate joint stress poses, not a substitute for a gameplay
            // animation review. They expose rigid sleeve/knee attachment gaps.
            for joint in [JointId::LeftLowerArm, JointId::RightLowerArm] {
                pose.transforms[joint.index()].rotation = Quat::from_rotation_x(-1.1);
            }
            if pose_name == "jump" {
                for joint in [JointId::LeftUpperLeg, JointId::RightUpperLeg] {
                    pose.transforms[joint.index()].rotation = Quat::from_rotation_x(-0.6);
                }
                for joint in [JointId::LeftLowerLeg, JointId::RightLowerLeg] {
                    pose.transforms[joint.index()].rotation = Quat::from_rotation_x(1.0);
                }
            }
        }
        let lod = if reference {
            CharacterLod::Near
        } else {
            match std::env::var("CUBA_STARTER_LOD")
                .as_deref()
                .unwrap_or("near")
            {
                "near" => CharacterLod::Near,
                "mid" => CharacterLod::Mid,
                "far" => CharacterLod::Far,
                other => panic!("unsupported review LOD {other}"),
            }
        };
        let entity = RenderEntity {
            body,
            outfit: OutfitId::EverydayHoodie,
            pose,
            face: crate::character::FaceParameters::preset(resolved.appearance.face),
            ..Default::default()
        };
        let assets: Vec<_> = legacy
            .equipment
            .values()
            .map(|id| MorphAssetId::parse(id).unwrap())
            .collect();
        renderer.begin();
        renderer.add_with_quality(
            entity,
            style,
            [0.08, 0.04, 0.025, 1.],
            lod,
            0,
            true,
            &assets,
        );
        if reference {
            reference_stage::add(&mut renderer, &device);
        }
        renderer.upload(&queue);
        if assets.iter().any(|id| {
            renderer
                .morphs
                .assets
                .get(id)
                .is_some_and(|a| a.is_base && a.canonical_rest)
        }) {
            let matrices = recipe.rig.world_matrices(&pose.transforms);
            let expected_batches = assets
                .iter()
                .filter(|id| {
                    renderer
                        .morphs
                        .assets
                        .get(id)
                        .is_some_and(|asset| asset.mode == MorphPackAttachmentMode::Skinned)
                })
                .count();
            assert_eq!(
                renderer.morphs.skinned_batches.len(),
                expected_batches,
                "all study parts must be admitted"
            );
            for batch in &renderer.morphs.skinned_batches {
                let asset = &renderer.morphs.assets[&batch.asset_id];
                let mesh = &asset.skinned_lods.as_ref().unwrap()[lod.index()];
                for (index, actual) in batch.vertices.iter().enumerate() {
                    let skin = &mesh.skinning.as_ref().unwrap()[index];
                    let mut expected_position = Vec3::ZERO;
                    let mut expected_normal = Vec3::ZERO;
                    for (joint, weight) in skin.joints.into_iter().zip(skin.weights) {
                        expected_position += matrices[joint as usize]
                            .transform_point3(Vec3::from_array(mesh.vertices[index]))
                            * weight;
                        expected_normal += Mat3::from_mat4(matrices[joint as usize])
                            * Vec3::from_array(mesh.normals[index])
                            * weight;
                    }
                    assert!(Vec3::from_array(actual.position).distance(expected_position) < 1e-5);
                    assert!(
                        Vec3::from_array(actual.normal).distance(expected_normal.normalize())
                            < 1e-5
                    );
                }
            }
            if assets.iter().any(|id| {
                renderer
                    .morphs
                    .assets
                    .get(id)
                    .is_some_and(|a| a.is_base && a.authored_static_face)
            }) {
                assert!(
                    renderer
                        .batches
                        .iter()
                        .filter(|b| b.material == Material::Face)
                        .all(|b| b.instances.is_empty()),
                    "static face must own its expression on every target"
                );
            }
        }
        let name = preset
            .id
            .as_str()
            .split('/')
            .next_back()
            .unwrap()
            .trim_end_matches(".v1");
        render_thumbnail(
            &renderer,
            &device,
            &queue,
            &layout,
            loadout.parts.iter().any(|id| {
                catalog
                    .asset(id)
                    .unwrap()
                    .occupied_slots
                    .iter()
                    .any(|slot| slot == "headwear")
            }),
            &output.join(format!("{name}.png")),
        );
    }
}

fn color(hex: &str) -> [f32; 4] {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap();
    [
        ((value >> 16) & 255) as f32 / 255.,
        ((value >> 8) & 255) as f32 / 255.,
        (value & 255) as f32 / 255.,
        1.,
    ]
}

fn render_thumbnail(
    renderer: &CharacterRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    tall_headwear: bool,
    path: &std::path::Path,
) {
    let reference = std::env::var_os("CUBA_STARTER_REFERENCE").is_some();
    let portrait = !reference && std::env::var_os("CUBA_STARTER_PORTRAIT").is_some();
    // Optional inspection resolution uses the same meshes, lighting and draw
    // path as thumbnails. Keep the row width aligned for GPU readback.
    let scale: u32 = if reference {
        3
    } else {
        std::env::var("CUBA_STARTER_SCALE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1)
            .clamp(1, 4)
    };
    let width = if portrait { 512 } else { 256 } * scale;
    let height = if portrait { 640 } else { 320 } * scale;
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let (center, half_height) = if reference {
        (1.62, 2.28)
    } else if portrait {
        (2.82, 1.03)
    } else if tall_headwear {
        (1.95, 2.30)
    } else {
        (1.9, 2.25)
    };
    let yaw: f32 = if reference {
        0.26
    } else {
        std::env::var("CUBA_STARTER_YAW")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0713)
    };
    let pitch: f32 = if reference {
        0.12
    } else {
        std::env::var("CUBA_STARTER_PITCH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.6_f32.atan2(7.))
    };
    let camera = Vec3::new(
        yaw.sin() * 7. * pitch.cos(),
        center + 7. * pitch.sin(),
        -yaw.cos() * 7. * pitch.cos(),
    );
    let globals = Globals {
        view_projection: (Mat4::orthographic_rh(
            -half_height * 0.8,
            half_height * 0.8,
            -half_height,
            half_height,
            0.1,
            100.,
        ) * Mat4::look_at_rh(camera, Vec3::new(0., center, 0.), Vec3::Y))
        .to_cols_array_2d(),
        camera_position: [camera.x, camera.y, camera.z, 1.],
        sun_direction: [-0.45, -0.82, 0.32, 0.],
        fog_color: [0.035, 0.035, 0.035, 1.],
    };
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::bytes_of(&globals),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (width * height * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let view = texture.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.035,
                        g: 0.035,
                        b: 0.035,
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &group, &[]);
        for kind in [CharacterPass::Opaque, CharacterPass::Face] {
            renderer.draw(&mut pass, kind);
        }
    }
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
        },
        extent,
    );
    queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let pixels = readback.slice(..).get_mapped_range();
    let mut png = png::Encoder::new(std::fs::File::create(path).unwrap(), width, height);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .unwrap()
        .write_image_data(&pixels)
        .unwrap();
}
