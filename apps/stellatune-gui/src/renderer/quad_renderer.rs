use anyhow::Result;
use wgpu::util::DeviceExt;

use super::frame::UiRect;

const QUAD_SHADER: &str = r#"
struct VsIn {
  @location(0) position: vec2<f32>,
  @location(1) color: vec4<f32>,
};

struct VsOut {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
  var out: VsOut;
  out.position = vec4<f32>(input.position, 0.0, 1.0);
  out.color = input.color;
  return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  return input.color;
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct QuadVertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl QuadVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct QuadRenderer {
    pipeline: wgpu::RenderPipeline,
}

impl QuadRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stellatune-gui-quad-shader"),
            source: wgpu::ShaderSource::Wgsl(QUAD_SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("stellatune-gui-quad-pipeline-layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stellatune-gui-quad-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[QuadVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline }
    }

    pub fn draw_rects(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        viewport: winit::dpi::PhysicalSize<u32>,
        rects: &[UiRect],
    ) -> Result<()> {
        if rects.is_empty() || viewport.width == 0 || viewport.height == 0 {
            return Ok(());
        }

        let vertices = build_vertices(rects, viewport.width as f32, viewport.height as f32);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("stellatune-gui-quad-vertices"),
            contents: bytemuck::cast_slice(vertices.as_slice()),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("stellatune-gui-ui-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
        Ok(())
    }
}

fn build_vertices(rects: &[UiRect], viewport_width: f32, viewport_height: f32) -> Vec<QuadVertex> {
    let mut vertices = Vec::with_capacity(rects.len() * 6);
    for rect in rects {
        let x0 = normalize_x(rect.rect[0], viewport_width);
        let y0 = normalize_y(rect.rect[1], viewport_height);
        let x1 = normalize_x(rect.rect[0] + rect.rect[2], viewport_width);
        let y1 = normalize_y(rect.rect[1] + rect.rect[3], viewport_height);

        let color = rect.color;
        vertices.extend_from_slice(&[
            QuadVertex {
                position: [x0, y0],
                color,
            },
            QuadVertex {
                position: [x1, y0],
                color,
            },
            QuadVertex {
                position: [x1, y1],
                color,
            },
            QuadVertex {
                position: [x0, y0],
                color,
            },
            QuadVertex {
                position: [x1, y1],
                color,
            },
            QuadVertex {
                position: [x0, y1],
                color,
            },
        ]);
    }
    vertices
}

fn normalize_x(x: f32, width: f32) -> f32 {
    (x / width) * 2.0 - 1.0
}

fn normalize_y(y: f32, height: f32) -> f32 {
    1.0 - (y / height) * 2.0
}
