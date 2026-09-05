//! Compatibility color contract: encoded world/UI values in an unorm target;
//! linear character lighting explicitly encodes into it. Present once, decoding
//! only when the surface hardware will encode sRGB. UI blends before present.
pub(super) const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub(super) fn select_samples(adapter: &wgpu::Adapter, allow_msaa: bool) -> u32 {
    let color = adapter.get_texture_format_features(SCENE_FORMAT);
    let depth = adapter.get_texture_format_features(super::DEPTH_FORMAT);
    samples_from_flags(color.flags, depth.flags, allow_msaa)
}
fn samples_from_flags(
    color: wgpu::TextureFormatFeatureFlags,
    depth: wgpu::TextureFormatFeatureFlags,
    allow: bool,
) -> u32 {
    use wgpu::TextureFormatFeatureFlags as F;
    if allow
        && color.contains(F::MULTISAMPLE_X4 | F::MULTISAMPLE_RESOLVE)
        && depth.contains(F::MULTISAMPLE_X4)
    {
        4
    } else {
        1
    }
}

pub(super) struct SceneTargets {
    pub color: wgpu::TextureView,
    pub multisample: Option<wgpu::TextureView>,
    pub depth: wgpu::TextureView,
    pub present_bind_group: wgpu::BindGroup,
}
impl SceneTargets {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        samples: u32,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let texture = |label, format, sample_count, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: width.max(1),
                        height: height.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let color = texture(
            "resolved scene and UI",
            SCENE_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let multisample = (samples > 1).then(|| {
            texture(
                "multisample scene",
                SCENE_FORMAT,
                samples,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
        });
        let depth = texture(
            "scene depth",
            super::DEPTH_FORMAT,
            samples,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let present_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present scene"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&color),
            }],
        });
        Self {
            color,
            multisample,
            depth,
            present_bind_group,
        }
    }
    pub fn attachment(&self, clear: wgpu::Color) -> wgpu::RenderPassColorAttachment<'_> {
        wgpu::RenderPassColorAttachment {
            view: self.multisample.as_ref().unwrap_or(&self.color),
            depth_slice: None,
            resolve_target: self.multisample.as_ref().map(|_| &self.color),
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: if self.multisample.is_some() {
                    wgpu::StoreOp::Discard
                } else {
                    wgpu::StoreOp::Store
                },
            },
        }
    }
}

pub(super) struct Presenter {
    pub layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}
impl Presenter {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present layout"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("surface color conversion"),
            source: wgpu::ShaderSource::Wgsl(include_str!("present.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("surface presentation"),
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
                entry_point: Some(if format.is_srgb() {
                    "fs_srgb"
                } else {
                    "fs_unorm"
                }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self { layout, pipeline }
    }
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        targets: &SceneTargets,
        destination: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present encoded scene"),
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
        pass.set_bind_group(0, &targets.present_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn msaa_requires_color_depth_and_resolve_and_can_be_disabled() {
        use wgpu::TextureFormatFeatureFlags as F;
        let color = F::MULTISAMPLE_X4 | F::MULTISAMPLE_RESOLVE;
        assert_eq!(samples_from_flags(color, F::MULTISAMPLE_X4, true), 4);
        assert_eq!(samples_from_flags(color, F::MULTISAMPLE_X4, false), 1);
        assert_eq!(samples_from_flags(color, F::empty(), true), 1);
        assert_eq!(
            samples_from_flags(F::MULTISAMPLE_X4, F::MULTISAMPLE_X4, true),
            1
        );
        assert_eq!(samples_from_flags(F::empty(), F::MULTISAMPLE_X4, true), 1);
    }
}
