use super::super::Renderer;
use crate::Engine;
use crate::renderer::targets::SCENE_FORMAT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TileRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl TileRect {
    fn from_viewport(viewport: [f32; 4], surface: (u32, u32)) -> Option<Self> {
        let [x, y, width, height] = viewport;
        if !viewport.iter().all(|value| value.is_finite()) || width <= 0.0 || height <= 0.0 {
            return None;
        }
        let left = x.round().clamp(0.0, surface.0 as f32) as u32;
        let top = y.round().clamp(0.0, surface.1 as f32) as u32;
        let right = (x + width).round().clamp(0.0, surface.0 as f32) as u32;
        let bottom = (y + height).round().clamp(0.0, surface.1 as f32) as u32;
        (right > left && bottom > top).then_some(Self {
            x: left,
            y: top,
            width: right.saturating_sub(left),
            height: bottom.saturating_sub(top),
        })
    }

    fn viewport(self) -> [f32; 4] {
        [
            self.x as f32,
            self.y as f32,
            self.width as f32,
            self.height as f32,
        ]
    }
}

struct TileTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
}

pub(crate) struct StudioTileCompositor {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    tiles: Vec<TileTexture>,
}

impl StudioTileCompositor {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Studio play tiles"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Studio play tile shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("studio_tiles.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Studio play tile pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Studio play tile pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SCENE_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            layout,
            pipeline,
            tiles: Vec::new(),
        }
    }

    fn ensure_tiles(&mut self, device: &wgpu::Device, rects: &[TileRect]) {
        if self.tiles.len() == rects.len()
            && self
                .tiles
                .iter()
                .zip(rects)
                .all(|(tile, rect)| tile.size == (rect.width, rect.height))
        {
            return;
        }
        self.tiles = rects
            .iter()
            .map(|rect| {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Studio play tile"),
                    size: wgpu::Extent3d {
                        width: rect.width,
                        height: rect.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: SCENE_FORMAT,
                    usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let view = texture.create_view(&Default::default());
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Studio play tile binding"),
                    layout: &self.layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    }],
                });
                TileTexture {
                    texture,
                    bind_group,
                    size: (rect.width, rect.height),
                }
            })
            .collect();
    }

    fn compose(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::TextureView,
        rects: &[TileRect],
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Studio play tile composition"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: destination,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        for (tile, rect) in self.tiles.iter().zip(rects) {
            pass.set_viewport(
                rect.x as f32,
                rect.y as f32,
                rect.width as f32,
                rect.height as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(rect.x, rect.y, rect.width, rect.height);
            pass.set_bind_group(0, &tile.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

impl Renderer {
    pub(crate) fn draw_studio_tiles_with_overlay<F>(
        &mut self,
        engines: &[&Engine],
        viewports: &[[f32; 4]],
        offscreen: bool,
        overlay: F,
    ) where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    {
        let rects: Vec<_> = viewports
            .iter()
            .filter_map(|viewport| {
                TileRect::from_viewport(*viewport, (self.width as u32, self.height as u32))
            })
            .collect();
        if rects.len() != engines.len() || rects.is_empty() {
            return;
        }
        if self.studio_tiles.is_none() {
            self.studio_tiles = Some(StudioTileCompositor::new(&self.device));
        }
        self.studio_tiles
            .as_mut()
            .expect("tile compositor")
            .ensure_tiles(&self.device, &rects);
        let previous_viewport = self.studio_viewport;
        for (index, (engine, rect)) in engines.iter().zip(&rects).enumerate() {
            self.sync_engine(engine);
            self.set_studio_viewport(Some(rect.viewport()));
            let Some((_, mut encoder, _)) = self.encode_frame(true) else {
                continue;
            };
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.targets.color_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: rect.x,
                        y: rect.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &self.studio_tiles.as_ref().expect("tile compositor").tiles[index]
                        .texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: rect.width,
                    height: rect.height,
                    depth_or_array_layers: 1,
                },
            );
            self.queue.submit(Some(encoder.finish()));
        }
        self.studio_viewport = previous_viewport;

        let frame = if offscreen {
            None
        } else {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    self.resize(self.width, self.height);
                    return;
                }
                _ => return,
            }
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Studio play tiles frame"),
            });
        self.studio_tiles
            .as_ref()
            .expect("tile compositor")
            .compose(&mut encoder, &self.targets.color, &rects);
        overlay(&self.device, &self.queue, &mut encoder, &self.targets.color);
        if let Some(frame) = &frame {
            let view = frame.texture.create_view(&Default::default());
            self.presenter.draw(&mut encoder, &self.targets, &view);
        }
        self.queue.submit(Some(encoder.finish()));
        if let Some(frame) = frame {
            frame.present();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TileRect;

    #[test]
    fn tile_rects_stay_inside_the_target_after_scale_rounding() {
        let rect = TileRect::from_viewport([100.4, 20.6, 450.2, 300.2], (500, 250)).unwrap();
        assert_eq!(
            rect,
            TileRect {
                x: 100,
                y: 21,
                width: 400,
                height: 229
            }
        );
        assert!(TileRect::from_viewport([600.0, 0.0, 10.0, 10.0], (500, 250)).is_none());
    }
}
