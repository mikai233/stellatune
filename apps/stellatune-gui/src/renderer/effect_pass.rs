use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

use super::frame::EffectFrame;

const EFFECT_SHADER: &str = r#"
struct EffectUniforms {
  viewport: vec2<f32>,
  pointer: vec2<f32>,
  clear_color: vec4<f32>,
  accent_color: vec4<f32>,
  glow_color: vec4<f32>,
  params: vec4<f32>,
};

struct VsOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: EffectUniforms;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
  var positions = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>( 1.0,  1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0,  1.0),
    vec2<f32>(-1.0,  1.0),
  );
  var uvs = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 0.0),
  );

  var out: VsOut;
  out.position = vec4<f32>(positions[index], 0.0, 1.0);
  out.uv = uvs[index];
  return out;
}

fn circle(uv: vec2<f32>, center: vec2<f32>, radius: f32, softness: f32) -> f32 {
  let dist = distance(uv, center);
  return 1.0 - smoothstep(radius, radius + softness, dist);
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  let uv = input.uv;
  let time = uniforms.params.x;
  let intensity = uniforms.params.y;
  let aspect = uniforms.viewport.x / max(uniforms.viewport.y, 1.0);
  let centered = vec2<f32>((uv.x - 0.5) * aspect, uv.y - 0.5);
  let pointer = vec2<f32>(uniforms.pointer.x, 1.0 - uniforms.pointer.y);

  let wave = 0.5 + 0.5 * sin((centered.x * 10.0 + centered.y * 8.0) + time * (0.7 + intensity));
  let bands = 0.5 + 0.5 * sin((uv.y * 22.0 - time * 1.8) + uv.x * 6.0);
  let orb = circle(uv, pointer, 0.18 + intensity * 0.10, 0.30);
  let secondary = circle(uv, vec2<f32>(0.18, 0.76), 0.24, 0.42);
  let vignette = smoothstep(0.92, 0.18, length(centered));
  let base = uniforms.clear_color.rgb;
  let accent = uniforms.accent_color.rgb * (0.035 + wave * 0.045 + orb * 0.11 * intensity);
  let glow = uniforms.glow_color.rgb * (secondary * 0.04 + bands * 0.02 * intensity);
  let color = base + accent + glow + vec3<f32>(0.003, 0.007, 0.012) * vignette;
  let alpha = 0.010 + intensity * 0.009;

  return vec4<f32>(color * alpha, alpha);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct EffectUniforms {
    viewport: [f32; 2],
    pointer: [f32; 2],
    clear_color: [f32; 4],
    accent_color: [f32; 4],
    glow_color: [f32; 4],
    params: [f32; 4],
}

pub struct EffectPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}

impl EffectPass {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stellatune-gui-effect-shader"),
            source: wgpu::ShaderSource::Wgsl(EFFECT_SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stellatune-gui-effect-bind-group-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("stellatune-gui-effect-uniforms"),
            contents: bytemuck::bytes_of(&EffectUniforms::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stellatune-gui-effect-bind-group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("stellatune-gui-effect-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stellatune-gui-effect-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
        }
    }

    pub fn update(
        &self,
        queue: &wgpu::Queue,
        viewport: PhysicalSize<u32>,
        frame: &EffectFrame,
    ) {
        let uniforms = EffectUniforms {
            viewport: [viewport.width.max(1) as f32, viewport.height.max(1) as f32],
            pointer: frame.pointer,
            clear_color: frame.clear_color,
            accent_color: frame.accent_color,
            glow_color: frame.glow_color,
            params: [frame.time, frame.intensity, 0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) -> Result<()> {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("stellatune-gui-effect-pass"),
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
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..6, 0..1);
        Ok(())
    }
}
