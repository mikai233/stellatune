use anyhow::Result;
use bytemuck::{Pod, Zeroable};

use crate::app::{FrameState, RenderFrameError};
use crate::gpu::context::GpuContext;
use crate::gpu::frame::FrameTargets;
use crate::page_transition::ResolvedPageTransition;
use crate::render::effects::EffectsRenderer;
use crate::render::layers::FrameLayers;
use crate::render::media::MediaRenderer;
use crate::render::vello_renderer::VelloRenderer;
use crate::resources::textures::TextureResource;
use crate::scene::DemoSceneFrame;
use crate::ui::node::UiLayer;

const COMPOSITE_SHADER: &str = include_str!("shaders/composite.wgsl");

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct CompositeSettings {
    source_opacity: f32,
    destination_opacity: f32,
    source_translate_x: f32,
    source_translate_y: f32,
    source_scale: f32,
    destination_translate_x: f32,
    destination_translate_y: f32,
    destination_scale: f32,
    media_on_top: f32,
    _padding: [f32; 3],
}

#[derive(Debug)]
struct CompositeRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    settings_buffer: wgpu::Buffer,
}

impl CompositeRenderer {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stellatune-gui-composite-shader"),
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stellatune-gui-composite-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(
                                std::mem::size_of::<CompositeSettings>() as u64
                            )
                            .expect("composite settings should have a non-zero size"),
                        ),
                    },
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("stellatune-gui-composite-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let settings_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("stellatune-gui-composite-settings-buffer"),
            size: std::mem::size_of::<CompositeSettings>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("stellatune-gui-composite-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stellatune-gui-composite-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            settings_buffer,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        effect_view: &wgpu::TextureView,
        media_view: &wgpu::TextureView,
        source_vector_view: &wgpu::TextureView,
        destination_vector_view: &wgpu::TextureView,
        page_transition: ResolvedPageTransition,
    ) {
        let settings = CompositeSettings {
            source_opacity: page_transition.source.opacity,
            destination_opacity: page_transition.destination.opacity,
            source_translate_x: page_transition.source.translation_uv[0],
            source_translate_y: page_transition.source.translation_uv[1],
            source_scale: page_transition.source.scale,
            destination_translate_x: page_transition.destination.translation_uv[0],
            destination_translate_y: page_transition.destination.translation_uv[1],
            destination_scale: page_transition.destination.scale,
            media_on_top: if page_transition.media_on_top {
                1.0
            } else {
                0.0
            },
            _padding: [0.0; 3],
        };
        queue.write_buffer(&self.settings_buffer, 0, bytemuck::bytes_of(&settings));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stellatune-gui-composite-bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(effect_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(media_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(source_vector_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(destination_vector_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.settings_buffer.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("stellatune-gui-composite-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

pub struct FrameComposer {
    effects: EffectsRenderer,
    media: MediaRenderer,
    vello: VelloRenderer,
    composite: CompositeRenderer,
}

impl FrameComposer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Result<Self> {
        Ok(Self {
            effects: EffectsRenderer::new(device, wgpu::TextureFormat::Rgba8Unorm),
            media: MediaRenderer::new(device, wgpu::TextureFormat::Rgba8Unorm),
            vello: VelloRenderer::new(device)?,
            composite: CompositeRenderer::new(device, surface_format),
        })
    }

    pub fn render(
        &mut self,
        gpu: &GpuContext,
        targets: &FrameTargets,
        scene_frame: DemoSceneFrame<'_>,
        cover_texture: &TextureResource,
        frame: &FrameState,
        page_transition: ResolvedPageTransition,
    ) -> std::result::Result<(), RenderFrameError> {
        let surface_texture = gpu.surface.get_current_texture()?;
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let layers = FrameLayers::new(
            targets.effect_view(),
            targets.media_view(),
            targets.source_vector_view(),
            targets.destination_vector_view(),
        );

        {
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("stellatune-gui-effect-encoder"),
                });
            self.effects.render(
                &gpu.queue,
                &mut encoder,
                layers.view(crate::render::layers::OffscreenLayer::Effects),
                frame,
            );
            gpu.queue.submit(Some(encoder.finish()));
        }

        {
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("stellatune-gui-media-encoder"),
                });
            self.media.render(
                &gpu.device,
                &gpu.queue,
                &mut encoder,
                layers.view(crate::render::layers::OffscreenLayer::Media),
                cover_texture,
                scene_frame.cover_rect,
                frame,
            );
            gpu.queue.submit(Some(encoder.finish()));
        }

        self.vello.render(
            &gpu.device,
            &gpu.queue,
            scene_frame.source_scene,
            layers.view(crate::render::layers::OffscreenLayer::SourceVectorUi),
            gpu.surface_size(),
        )?;

        self.vello.render(
            &gpu.device,
            &gpu.queue,
            scene_frame.destination_scene,
            layers.view(crate::render::layers::OffscreenLayer::DestinationVectorUi),
            gpu.surface_size(),
        )?;

        let composite_layers = layers.composite_layers();

        {
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("stellatune-gui-composite-encoder"),
                });
            self.composite.render(
                &gpu.device,
                &gpu.queue,
                &mut encoder,
                &surface_view,
                composite_layers.background,
                composite_layers.media,
                composite_layers.source_foreground,
                composite_layers.destination_foreground,
                ResolvedPageTransition {
                    media_on_top: scene_frame.cover_layer == UiLayer::Overlay
                        || page_transition.media_on_top,
                    ..page_transition
                },
            );
            gpu.queue.submit(Some(encoder.finish()));
        }

        surface_texture.present();
        Ok(())
    }
}
