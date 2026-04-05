use anyhow::Result;

use crate::app::{FrameState, RenderFrameError};
use crate::gpu::context::GpuContext;
use crate::gpu::frame::FrameTargets;
use crate::render::effects::EffectsRenderer;
use crate::render::layers::FrameLayers;
use crate::render::media::MediaRenderer;
use crate::render::vello_renderer::VelloRenderer;
use crate::resources::textures::TextureResource;
use crate::scene::DemoSceneFrame;

const COMPOSITE_SHADER: &str = include_str!("shaders/composite.wgsl");

#[derive(Debug)]
struct CompositeRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
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
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("stellatune-gui-composite-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
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
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        effect_view: &wgpu::TextureView,
        media_view: &wgpu::TextureView,
        vector_view: &wgpu::TextureView,
    ) {
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
                    resource: wgpu::BindingResource::TextureView(vector_view),
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
    ) -> std::result::Result<(), RenderFrameError> {
        let surface_texture = gpu.surface.get_current_texture()?;
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let layers = FrameLayers::new(
            targets.effect_view(),
            targets.media_view(),
            targets.vector_view(),
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
            scene_frame.scene,
            layers.view(crate::render::layers::OffscreenLayer::VectorUi),
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
                &mut encoder,
                &surface_view,
                composite_layers.background,
                composite_layers.media,
                composite_layers.foreground,
            );
            gpu.queue.submit(Some(encoder.finish()));
        }

        surface_texture.present();
        Ok(())
    }
}
