use super::super::Renderer;
use crate::Engine;
use crate::renderer::targets::{PostProcessor, SCENE_FORMAT, SceneTargets};

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
    // The full player uses Renderer::targets. The smaller views render at
    // their own resolution instead of clearing and grading the whole window.
    render_targets: Option<SceneTargets>,
}

pub(crate) struct StudioTileCompositor {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    tiles: Vec<TileTexture>,
    next_preview: usize,
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
            next_preview: 0,
        }
    }

    fn ensure_tiles(
        &mut self,
        device: &wgpu::Device,
        rects: &[TileRect],
        samples: u32,
        post: &PostProcessor,
        present_layout: &wgpu::BindGroupLayout,
        full_index: usize,
    ) -> bool {
        if self.tiles.len() == rects.len()
            && self
                .tiles
                .iter()
                .zip(rects)
                .enumerate()
                .all(|(index, (tile, rect))| {
                    tile.size == (rect.width, rect.height)
                        && tile.render_targets.is_none() == (index == full_index)
                })
        {
            return false;
        }
        self.next_preview = 0;
        self.tiles = rects
            .iter()
            .enumerate()
            .map(|(index, rect)| {
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
                    render_targets: (index != full_index).then(|| {
                        SceneTargets::new(
                            device,
                            rect.width,
                            rect.height,
                            samples,
                            post,
                            present_layout,
                        )
                    }),
                }
            })
            .collect();
        true
    }

    fn refresh_indices(&mut self, full_index: usize, refreshed_all: bool) -> Vec<usize> {
        preview_refresh_indices(
            self.tiles.len(),
            full_index,
            &mut self.next_preview,
            refreshed_all,
        )
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
        // Draw the full player view first, then place the smaller live views
        // over it. Equal-sized previews retain their player order.
        let mut order: Vec<_> = (0..rects.len()).collect();
        order.sort_by_key(|&index| {
            std::cmp::Reverse(rects[index].width as u64 * rects[index].height as u64)
        });
        for index in order {
            let tile = &self.tiles[index];
            let rect = &rects[index];
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

fn preview_refresh_indices(
    count: usize,
    full_index: usize,
    cursor: &mut usize,
    refreshed_all: bool,
) -> Vec<usize> {
    let mut indices = vec![full_index];
    let previews: Vec<_> = (0..count).filter(|&index| index != full_index).collect();
    if refreshed_all || previews.len() <= 2 {
        indices.extend(previews);
    } else {
        // Simulation still advances every client on every frame. Rotate two
        // rendered previews per frame so the full view remains responsive.
        for _ in 0..2 {
            indices.push(previews[*cursor % previews.len()]);
            *cursor += 1;
        }
    }
    indices
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
        let full_index = (0..rects.len())
            .max_by_key(|&index| rects[index].width as u64 * rects[index].height as u64)
            .expect("at least one play view");
        let refreshed_all = self
            .studio_tiles
            .as_mut()
            .expect("tile compositor")
            .ensure_tiles(
                &self.device,
                &rects,
                self.sample_count,
                &self.post_processor,
                &self.presenter.layout,
                full_index,
            );
        let refresh_indices = self
            .studio_tiles
            .as_mut()
            .expect("tile compositor")
            .refresh_indices(full_index, refreshed_all);
        let previous_viewport = self.studio_viewport;
        let previous_size = (self.width, self.height);
        for index in refresh_indices {
            let engine = engines[index];
            let rect = &rects[index];
            self.sync_engine(engine);
            let is_full = index == full_index;
            if !is_full {
                let tile = &mut self.studio_tiles.as_mut().expect("tile compositor").tiles[index];
                std::mem::swap(
                    &mut self.targets,
                    tile.render_targets.as_mut().expect("preview targets"),
                );
                self.width = rect.width as f32;
                self.height = rect.height as f32;
            }
            let origin = if is_full {
                self.set_studio_viewport(Some(rect.viewport()));
                wgpu::Origin3d {
                    x: rect.x,
                    y: rect.y,
                    z: 0,
                }
            } else {
                self.set_studio_viewport(Some([0.0, 0.0, rect.width as f32, rect.height as f32]));
                wgpu::Origin3d::ZERO
            };
            if let Some((_, mut encoder, _)) = self.encode_frame(true) {
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.targets.color_texture,
                        mip_level: 0,
                        origin,
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
            if !is_full {
                let tile = &mut self.studio_tiles.as_mut().expect("tile compositor").tiles[index];
                std::mem::swap(
                    &mut self.targets,
                    tile.render_targets.as_mut().expect("preview targets"),
                );
                (self.width, self.height) = previous_size;
            }
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
    use super::{TileRect, preview_refresh_indices};

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

    #[test]
    fn every_preview_refreshes_while_the_controlled_view_updates_each_frame() {
        let mut cursor = 0;
        let mut refreshed = Vec::new();
        for _ in 0..4 {
            let indices = preview_refresh_indices(9, 4, &mut cursor, false);
            assert_eq!(indices[0], 4);
            assert_eq!(indices.len(), 3);
            refreshed.extend_from_slice(&indices[1..]);
        }
        refreshed.sort_unstable();
        assert_eq!(refreshed, vec![0, 1, 2, 3, 5, 6, 7, 8]);
        assert_eq!(preview_refresh_indices(9, 4, &mut cursor, true).len(), 9);
    }
}
