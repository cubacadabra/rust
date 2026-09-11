//! Opt-in visual regression captures using the production character pipelines.
//! CUBA_MORPH_REVIEW_PACK=/path/to/file.morphpack cargo test capture_morph -- --ignored
use super::super::{DEPTH_FORMAT, Globals};
use super::*;

#[test]
#[ignore = "requires a GPU and CUBA_MORPH_REVIEW_PACK"]
fn capture_morph() {
    let input = std::env::var("CUBA_MORPH_REVIEW_PACK").expect("morph pack path");
    let output = std::path::PathBuf::from(
        std::env::var("CUBA_MORPH_REVIEW_OUTPUT").unwrap_or("/tmp/morph-review".into()),
    );
    std::fs::create_dir_all(&output).unwrap();
    let pack = cubacadabra_morphs::decode_morph_pack(&std::fs::read(input).unwrap()).unwrap();
    let asset = pack.asset.id.clone();
    let is_top = pack.asset.kind == MorphAssetKind::Top;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        ..Default::default()
    }))
    .unwrap();
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("morph review globals"),
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
    renderer.register_morph_pack(&device, &queue, pack).unwrap();
    let recipe = body_recipe(BodyId::Person);
    let entity = RenderEntity {
        body: BodyId::Person,
        outfit: OutfitId::EverydayHoodie,
        pose: crate::character::Pose::rest(&recipe.rig),
        face: crate::character::FaceParameters::preset(crate::character::FacePreset::Happy),
        ..Default::default()
    };
    // Loading and removing a top must restore exactly the four arm segments,
    // without changing the default bundled appearance.
    if is_top {
        let skin_count = |renderer: &CharacterRenderer| -> usize {
            renderer
                .batches
                .iter()
                .flat_map(|batch| &batch.instances)
                .filter(|instance| instance.material[3] == 11.0)
                .count()
        };
        renderer.begin();
        renderer.add(entity, AvatarStyle::default(), [0.08, 0.04, 0.025, 1.]);
        let baseline = skin_count(&renderer);
        renderer.begin();
        renderer.add_with_quality(
            entity,
            AvatarStyle::default(),
            [0.08, 0.04, 0.025, 1.],
            CharacterLod::Mid,
            0,
            true,
            std::slice::from_ref(&asset),
        );
        assert_eq!(skin_count(&renderer), baseline + 4);
        renderer.begin();
        renderer.add(entity, AvatarStyle::default(), [0.08, 0.04, 0.025, 1.]);
        assert_eq!(skin_count(&renderer), baseline);
    }
    let mut assets = vec![asset];
    if let Ok(path) = std::env::var("CUBA_MORPH_REVIEW_BASE") {
        let base = cubacadabra_morphs::decode_morph_pack(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(base.asset.kind, MorphAssetKind::Base);
        assets.push(base.asset.id.clone());
        renderer.register_morph_pack(&device, &queue, base).unwrap();
        renderer.begin();
        renderer.add_with_quality(
            entity,
            AvatarStyle::default(),
            [0.08, 0.04, 0.025, 1.],
            CharacterLod::Mid,
            0,
            true,
            &assets,
        );
        assert!(
            renderer
                .batches
                .iter()
                .flat_map(|batch| &batch.instances)
                .all(|instance| instance.material[3] != 11.0),
            "authored base must not draw fallback skin"
        );
    }
    let width = 640;
    let height = 800;
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    for (tier, lod) in [
        ("near", CharacterLod::Near),
        ("mid", CharacterLod::Mid),
        ("far", CharacterLod::Far),
    ] {
        for (name, angle) in [
            ("front", 0.0_f32),
            ("three-quarter", 0.7),
            ("side", 1.57),
            ("back", std::f32::consts::PI),
            ("walk", 0.7),
        ] {
            let entity = if name == "walk" {
                RenderEntity {
                    moving: true,
                    walk_cycle: 1.0,
                    pose: crate::character::Pose::locomotion(&recipe.rig, 1.0, true, false),
                    ..entity
                }
            } else {
                entity
            };
            renderer.begin();
            renderer.add_with_quality(
                entity,
                AvatarStyle::default(),
                [0.08, 0.04, 0.025, 1.0],
                lod,
                0,
                true,
                &assets,
            );
            renderer.upload(&queue);
            let camera = Vec3::new(angle.sin() * 6.5, 2.8, -angle.cos() * 6.5);
            let globals = Globals {
                view_projection: (Mat4::perspective_rh(
                    0.65,
                    width as f32 / height as f32,
                    0.1,
                    100.0,
                ) * Mat4::look_at_rh(camera, Vec3::new(0., 1.65, 0.), Vec3::Y))
                .to_cols_array_2d(),
                camera_position: [camera.x, camera.y, camera.z, 1.],
                sun_direction: [-0.45, -0.82, 0.32, 0.],
                fog_color: [0.025, 0.027, 0.045, 1.],
            };
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&globals),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            let color = device.create_texture(&wgpu::TextureDescriptor {
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
            let color_view = color.create_view(&Default::default());
            let depth_view = depth.create_view(&Default::default());
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &color_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.025,
                                g: 0.027,
                                b: 0.045,
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
                color.as_image_copy(),
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
            let (sender, receiver) = std::sync::mpsc::channel();
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    sender.send(result).unwrap();
                });
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            receiver.recv().unwrap().unwrap();
            let pixels = readback.slice(..).get_mapped_range();
            let file = std::fs::File::create(output.join(format!("{tier}-{name}.png"))).unwrap();
            let mut png = png::Encoder::new(file, width, height);
            png.set_color(png::ColorType::Rgba);
            png.set_depth(png::BitDepth::Eight);
            png.write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
    }
}
