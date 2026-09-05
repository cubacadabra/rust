//! Character-only layouts. Eleven attributes, two vertex buffers, no optional
//! GPU features: fits the 16-attribute downlevel device requested by all hosts.
use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct CharacterVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl CharacterVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
    };
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(super) struct CharacterInstance {
    // Affine rows pack translation into w; normal rows contain inverse transpose.
    pub transform: [[f32; 4]; 3],
    pub normal: [[f32; 4]; 3],
    pub tint: [f32; 4],
    // Roughness, specular strength, emission, reserved for material detail.
    pub material: [f32; 4],
}

impl CharacterInstance {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
            6 => Float32x4, 7 => Float32x4, 8 => Float32x4,
            9 => Float32x4, 10 => Float32x4
        ],
    };

    pub fn new(transform: Mat4, tint: [f32; 4], material: Material) -> Self {
        let normal = Mat3::from_mat4(transform).inverse().transpose();
        Self {
            transform: [
                transform.row(0).to_array(),
                transform.row(1).to_array(),
                transform.row(2).to_array(),
            ],
            normal: [
                normal.row(0).extend(0.0).to_array(),
                normal.row(1).extend(0.0).to_array(),
                normal.row(2).extend(0.0).to_array(),
            ],
            tint,
            material: material.parameters(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Material {
    Toy,
    Cloth,
    Denim,
    Rubber,
    Face,
    Seam,
}

impl Material {
    pub fn parameters(self) -> [f32; 4] {
        match self {
            Self::Toy => [0.42, 0.18, 0.0, 0.0],
            Self::Cloth => [0.92, 0.025, 0.0, 0.0],
            Self::Denim => [0.85, 0.04, 0.0, 0.0],
            Self::Rubber => [0.72, 0.08, 0.0, 0.0],
            Self::Face => [1.0, 0.0, 0.0, 0.0],
            Self::Seam => [1.0, 0.0, 1.4, 0.0],
        }
    }
    pub fn pass(self) -> CharacterPass {
        match self {
            Self::Face => CharacterPass::Face,
            Self::Seam => CharacterPass::Effect,
            _ => CharacterPass::Opaque,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CharacterPass {
    Opaque,
    Face,
    Effect,
}

pub(super) fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sample_count: u32,
    pass: CharacterPass,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("character materials"),
        source: wgpu::ShaderSource::Wgsl(include_str!("character.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("character layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(match pass {
            CharacterPass::Opaque => "character solids",
            CharacterPass::Face => "character face",
            CharacterPass::Effect => "character seam emission",
        }),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[CharacterVertex::LAYOUT, CharacterInstance::LAYOUT],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: super::DEPTH_FORMAT,
            depth_write_enabled: Some(pass != CharacterPass::Effect),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: super::targets::SCENE_FORMAT,
                blend: (pass == CharacterPass::Effect).then_some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    #[test]
    fn portable_layout_and_inverse_transpose() {
        let limits = wgpu::Limits::downlevel_defaults();
        assert!(11 <= limits.max_vertex_attributes);
        assert!(size_of::<CharacterInstance>() as u32 <= limits.max_vertex_buffer_array_stride);
        assert_eq!(size_of::<CharacterInstance>(), 128);
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::new(2.0, 0.3, 1.5),
            Quat::from_rotation_y(0.7),
            Vec3::new(3.0, 2.0, 1.0),
        );
        let instance = CharacterInstance::new(transform, [1.0; 4], Material::Toy);
        let source = Vec3::new(1.0, 1.0, 0.0).normalize();
        let normal = Vec3::from_array(
            instance
                .normal
                .map(|row| Vec3::from_array([row[0], row[1], row[2]]).dot(source)),
        );
        let tangent = transform.transform_vector3(Vec3::new(1.0, -1.0, 0.0));
        assert!(normal.dot(tangent).abs() < 1e-5);
    }
}
