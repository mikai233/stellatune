#![allow(dead_code)]

use anyhow::{Context, Result, anyhow};
use winit::dpi::PhysicalSize;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
    ID3DBlob,
};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BLEND_DESC, D3D11_BLEND_INV_SRC_ALPHA, D3D11_BLEND_ONE, D3D11_BLEND_OP_ADD,
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BUFFER_DESC, D3D11_CPU_ACCESS_WRITE,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_WRITE_DISCARD, D3D11_MAPPED_SUBRESOURCE,
    D3D11_COLOR_WRITE_ENABLE_ALL, D3D11_CULL_NONE, D3D11_FILL_SOLID,
    D3D11_SAMPLER_DESC, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DYNAMIC,
    D3D11_VIEWPORT, D3D11CreateDevice, ID3D11Buffer, ID3D11Device, ID3D11DeviceContext,
    ID3D11BlendState, ID3D11RasterizerState,
    ID3D11InputLayout, ID3D11PixelShader, ID3D11SamplerState,
    ID3D11ShaderResourceView, ID3D11Texture2D, ID3D11VertexShader, D3D11_BIND_SHADER_RESOURCE,
    D3D11_BIND_VERTEX_BUFFER, D3D11_INPUT_ELEMENT_DESC, D3D11_INPUT_PER_VERTEX_DATA,
    D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_TEXTURE_ADDRESS_CLAMP,
    D3D11_RASTERIZER_DESC, D3D11_RENDER_TARGET_BLEND_DESC, D3D11_SUBRESOURCE_DATA,
    D3D11_USAGE_IMMUTABLE,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dwm::{
    DWMSBT_MAINWINDOW, DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWMWCP_ROUNDSMALL, DWM_SYSTEMBACKDROP_TYPE,
    DWM_WINDOW_CORNER_PREFERENCE, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
    DXGI_PRESENT, IDXGIDevice, IDXGIFactory2, IDXGISwapChain1,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::core::{Interface, PCSTR};

use crate::renderer::{EffectFrame, UiFrame};

use super::composition::WindowsCompositionPlan;
use super::presenter::{WindowsPresenterKind, WindowsPresenterProfile};

#[derive(Debug, Clone)]
pub struct CompositionPresenterConfig {
    pub true_window_rounding: bool,
    pub backdrop_visual_enabled: bool,
    pub heavy_fx_visual_enabled: bool,
}

impl Default for CompositionPresenterConfig {
    fn default() -> Self {
        Self {
            true_window_rounding: true,
            backdrop_visual_enabled: true,
            heavy_fx_visual_enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompositionPresenterPlan {
    pub profile: WindowsPresenterProfile,
    pub composition: WindowsCompositionPlan,
    pub notes: &'static [&'static str],
}

impl CompositionPresenterPlan {
    pub fn target() -> Self {
        Self {
            profile: WindowsPresenterProfile::composition_visual_tree(),
            composition: WindowsCompositionPlan::bootstrap(),
            notes: &[
                "target Windows presenter for true rounded transparent window",
                "GPU effects should land in a composition visual layer",
                "system backdrop should provide the primary glass impression first",
            ],
        }
    }
}

pub struct CompositionPresenterRuntime {
    pub d3d11_device: ID3D11Device,
    pub d3d11_context: ID3D11DeviceContext,
    pub dcomp_device: IDCompositionDevice,
    _dxgi_device: IDXGIDevice,
    _dxgi_factory: IDXGIFactory2,
    _target: IDCompositionTarget,
    _root_visual: IDCompositionVisual,
    _backdrop_visual: IDCompositionVisual,
    _effects_visual: IDCompositionVisual,
    _ui_visual: IDCompositionVisual,
    _overlay_visual: IDCompositionVisual,
    pub effect_swapchain: IDXGISwapChain1,
    pub effect_vertex_shader: ID3D11VertexShader,
    pub effect_pixel_shader: ID3D11PixelShader,
    pub effect_uniform_buffer: ID3D11Buffer,
    pub fullscreen_input_layout: ID3D11InputLayout,
    pub fullscreen_vertex_buffer: ID3D11Buffer,
    pub rasterizer_state: ID3D11RasterizerState,
    pub premul_blend_state: ID3D11BlendState,
    pub ui_vertex_shader: ID3D11VertexShader,
    pub ui_pixel_shader: ID3D11PixelShader,
    pub ui_sampler: ID3D11SamplerState,
    ui_texture: Option<ID3D11Texture2D>,
    ui_shader_resource: Option<ID3D11ShaderResourceView>,
    pub backdrop_type: DWM_SYSTEMBACKDROP_TYPE,
    pub corner_preference: DWM_WINDOW_CORNER_PREFERENCE,
    effect_size: PhysicalSize<u32>,
}

impl CompositionPresenterRuntime {
    pub fn bootstrap(hwnd: isize, config: &CompositionPresenterConfig) -> Result<Self> {
        let hwnd = HWND(hwnd as _);
        let (backdrop_type, corner_preference) = apply_dwm_window_attributes(hwnd, config)
            .context("apply DWM composition window attributes")?;

        let (d3d11_device, d3d11_context) =
            create_d3d11_device().context("create D3D11 device for composition")?;
        let dxgi_device: IDXGIDevice = d3d11_device.cast().context("cast ID3D11Device to IDXGIDevice")?;
        let dxgi_factory: IDXGIFactory2 =
            unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)).context("CreateDXGIFactory2")? };
        let dcomp_device: IDCompositionDevice = unsafe {
            DCompositionCreateDevice(&dxgi_device).context("DCompositionCreateDevice")?
        };
        let target = unsafe {
            dcomp_device
                .CreateTargetForHwnd(hwnd, true)
                .context("CreateTargetForHwnd")?
        };
        let root_visual = unsafe { dcomp_device.CreateVisual().context("CreateVisual root")? };
        let backdrop_visual =
            unsafe { dcomp_device.CreateVisual().context("CreateVisual backdrop")? };
        let effects_visual =
            unsafe { dcomp_device.CreateVisual().context("CreateVisual effects")? };
        let ui_visual = unsafe { dcomp_device.CreateVisual().context("CreateVisual ui")? };
        let overlay_visual =
            unsafe { dcomp_device.CreateVisual().context("CreateVisual overlay")? };
        let effect_swapchain =
            create_effect_swapchain(&dxgi_factory, &dxgi_device).context("Create composition swapchain")?;
        let (effect_vertex_shader, effect_pixel_shader, effect_uniform_buffer, effect_vertex_shader_blob) =
            create_effect_pipeline(&d3d11_device).context("Create composition effect pipeline")?;
        let (fullscreen_input_layout, fullscreen_vertex_buffer) =
            create_fullscreen_geometry(&d3d11_device, &effect_vertex_shader_blob)
                .context("Create composition fullscreen geometry")?;
        let (rasterizer_state, premul_blend_state) =
            create_draw_states(&d3d11_device).context("Create composition draw states")?;
        let (ui_vertex_shader, ui_pixel_shader, ui_sampler) =
            create_ui_pipeline(&d3d11_device).context("Create composition ui pipeline")?;

        unsafe {
            effects_visual
                .SetContent(&effect_swapchain)
                .context("Attach swapchain to effects visual")?;
            root_visual
                .AddVisual(&backdrop_visual, false, None)
                .context("Add backdrop visual")?;
            root_visual
                .AddVisual(&effects_visual, true, Some(&backdrop_visual))
                .context("Add effects visual")?;
            root_visual
                .AddVisual(&ui_visual, true, Some(&effects_visual))
                .context("Add ui visual")?;
            root_visual
                .AddVisual(&overlay_visual, true, Some(&ui_visual))
                .context("Add overlay visual")?;
            target
                .SetRoot(&root_visual)
                .context("Set composition root visual")?;
            dcomp_device.Commit().context("Commit composition device")?;
        }

        Ok(Self {
            d3d11_device,
            d3d11_context,
            _dxgi_device: dxgi_device,
            _dxgi_factory: dxgi_factory,
            dcomp_device,
            _target: target,
            _root_visual: root_visual,
            _backdrop_visual: backdrop_visual,
            _effects_visual: effects_visual,
            _ui_visual: ui_visual,
            _overlay_visual: overlay_visual,
            effect_swapchain,
            effect_vertex_shader,
            effect_pixel_shader,
            effect_uniform_buffer,
            fullscreen_input_layout,
            fullscreen_vertex_buffer,
            rasterizer_state,
            premul_blend_state,
            ui_vertex_shader,
            ui_pixel_shader,
            ui_sampler,
            ui_texture: None,
            ui_shader_resource: None,
            backdrop_type,
            corner_preference,
            effect_size: PhysicalSize::new(1, 1),
        })
    }

    pub fn resize_effect_layer(&mut self, size: PhysicalSize<u32>) -> Result<()> {
        if size.width == 0 || size.height == 0 || self.effect_size == size {
            return Ok(());
        }

        unsafe {
            self.effect_swapchain
                .ResizeBuffers(
                    0,
                    size.width,
                    size.height,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
                .context("ResizeBuffers for composition effect swapchain")?;
            self.dcomp_device
                .Commit()
                .context("Commit composition resize")?;
        }

        self.effect_size = size;
        Ok(())
    }

    pub fn present_composed_frame(
        &mut self,
        size: PhysicalSize<u32>,
        effect_frame: &EffectFrame,
        ui_frame: &UiFrame,
    ) -> Result<()> {
        self.resize_effect_layer(size)?;
        self.ensure_ui_texture(PhysicalSize::new(ui_frame.width.max(1), ui_frame.height.max(1)))?;
        self.upload_ui_frame(ui_frame)?;

        let back_buffer: ID3D11Texture2D = unsafe {
            self.effect_swapchain
                .GetBuffer(0)
                .context("GetBuffer for composed effect swapchain")?
        };
        let mut render_target = None;
        unsafe {
            self.d3d11_device
                .CreateRenderTargetView(&back_buffer, None, Some(&mut render_target))
                .context("CreateRenderTargetView for composed swapchain")?;
        }
        let render_target =
            render_target.ok_or_else(|| anyhow!("CreateRenderTargetView returned no composed render target"))?;
        update_effect_uniforms(&self.d3d11_context, &self.effect_uniform_buffer, size, effect_frame)
            .context("Update composed effect uniforms")?;

        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: size.width.max(1) as f32,
            Height: size.height.max(1) as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        let render_targets = [Some(render_target.clone())];
        let constant_buffers = [Some(self.effect_uniform_buffer.clone())];
        let shader_resources = [self.ui_shader_resource.clone()];
        let samplers = [Some(self.ui_sampler.clone())];
        let vertex_buffers = [Some(self.fullscreen_vertex_buffer.clone())];
        let strides = [std::mem::size_of::<FullscreenVertex>() as u32];
        let offsets = [0_u32];
        let blend_factors = [0.0, 0.0, 0.0, 0.0];

        unsafe {
            self.d3d11_context.OMSetRenderTargets(Some(&render_targets), None);
            self.d3d11_context
                .OMSetBlendState(&self.premul_blend_state, Some(&blend_factors), u32::MAX);
            self.d3d11_context
                .RSSetState(&self.rasterizer_state);
            self.d3d11_context.RSSetViewports(Some(&[viewport]));
            self.d3d11_context
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.d3d11_context
                .IASetInputLayout(&self.fullscreen_input_layout);
            self.d3d11_context.IASetVertexBuffers(
                0,
                1,
                Some(vertex_buffers.as_ptr()),
                Some(strides.as_ptr()),
                Some(offsets.as_ptr()),
            );
            self.d3d11_context
                .ClearRenderTargetView(&render_target, &[0.0, 0.0, 0.0, 0.0]);

            self.d3d11_context
                .VSSetShader(&self.effect_vertex_shader, None);
            self.d3d11_context
                .PSSetShader(&self.effect_pixel_shader, None);
            self.d3d11_context
                .PSSetConstantBuffers(0, Some(&constant_buffers));
            self.d3d11_context.Draw(3, 0);

            self.d3d11_context
                .VSSetShader(&self.ui_vertex_shader, None);
            self.d3d11_context
                .PSSetShader(&self.ui_pixel_shader, None);
            self.d3d11_context.PSSetConstantBuffers(0, None);
            self.d3d11_context
                .PSSetShaderResources(0, Some(&shader_resources));
            self.d3d11_context.PSSetSamplers(0, Some(&samplers));
            self.d3d11_context.Draw(3, 0);

            self.effect_swapchain
                .Present(0, DXGI_PRESENT(0))
                .ok()
                .context("Present composed effect swapchain")?;
            self.dcomp_device
                .Commit()
                .context("Commit composed composition frame")?;
        }

        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct EffectUniforms {
    viewport: [f32; 2],
    pointer: [f32; 2],
    clear_color: [f32; 4],
    accent_color: [f32; 4],
    glow_color: [f32; 4],
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FullscreenVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

const EFFECT_SHADER_SOURCE: &str = r#"
cbuffer EffectUniforms : register(b0) {
    float2 viewport;
    float2 pointer;
    float4 clear_color;
    float4 accent_color;
    float4 glow_color;
    float4 params;
};

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

VsOut vs_main(float2 position : POSITION, float2 uv : TEXCOORD0) {
    VsOut output;
    output.position = float4(position, 0.0, 1.0);
    output.uv = uv;
    return output;
}

float circle_mask(float2 uv, float2 center, float radius, float softness) {
    float dist = distance(uv, center);
    return 1.0 - smoothstep(radius, radius + softness, dist);
}

float hash21(float2 p) {
    p = frac(p * float2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return frac(p.x * p.y);
}

float rounded_rect_mask(float2 uv, float2 margin, float radius, float2 view_size) {
    float2 size = float2(1.0 - margin.x * 2.0, 1.0 - margin.y * 2.0);
    float2 center = float2(0.5, 0.5);
    float2 local = abs(uv - center) - size * 0.5 + float2(radius / view_size.x, radius / view_size.y);
    float2 q = max(local, float2(0.0, 0.0));
    float dist = length(q) + min(max(local.x, local.y), 0.0);
    float feather = 1.5 / max(view_size.x, view_size.y);
    return 1.0 - smoothstep(0.0, feather, dist);
}

float4 ps_main(VsOut input) : SV_Target {
    float2 uv = input.uv;
    float time = params.x;
    float intensity = params.y;
    float aspect = viewport.x / max(viewport.y, 1.0);
    float2 centered = float2((uv.x - 0.5) * aspect, uv.y - 0.5);
    float2 p = float2(pointer.x, 1.0 - pointer.y);

    float wave = 0.5 + 0.5 * sin((centered.x * 10.0 + centered.y * 8.0) + time * (0.7 + intensity));
    float bands = 0.5 + 0.5 * sin((uv.y * 22.0 - time * 1.8) + uv.x * 6.0);
    float orb = circle_mask(uv, p, 0.18 + intensity * 0.10, 0.30);
    float secondary = circle_mask(uv, float2(0.18, 0.76), 0.24, 0.42);
    float vignette = smoothstep(0.92, 0.18, length(centered));
    float shell = rounded_rect_mask(uv, float2(6.0 / viewport.x, 6.0 / viewport.y), 18.0, viewport);
    float frost_noise = hash21(floor(uv * viewport * 0.22) + floor(time * 20.0));
    float fine_noise = hash21(floor(uv * viewport * 0.65) + 17.0 + floor(time * 42.0));
    float grain = (frost_noise - 0.5) * 0.10 + (fine_noise - 0.5) * 0.035;
    float sheen = smoothstep(0.96, 0.18, abs(uv.y - 0.14) * 4.1);
    float diagonal_sheen = smoothstep(0.78, 0.06, abs((uv.x - 0.18) - (1.0 - uv.y) * 0.62));
    float vertical_haze = smoothstep(0.0, 0.78, 1.0 - uv.y);
    float edge_glow = pow(saturate(1.0 - abs(uv.x - 0.5) * 1.65), 3.0) * 0.075;
    float soft_bloom = circle_mask(uv, float2(0.74, 0.22), 0.22, 0.42) * 0.06;

    float3 base = clear_color.rgb;
    float3 accent = accent_color.rgb * (0.022 + wave * 0.030 + orb * 0.060 * intensity);
    float3 glow = glow_color.rgb * (secondary * 0.022 + bands * 0.012 * intensity + soft_bloom);
    float3 frost = float3(0.16, 0.20, 0.26) * (0.075 + vertical_haze * 0.05) + grain;
    float3 highlight = float3(0.94, 0.98, 1.0) * (sheen * 0.095 + diagonal_sheen * 0.030 + edge_glow);
    float3 color = base + accent + glow + frost + highlight + float3(0.002, 0.004, 0.008) * vignette;
    float alpha = shell * (0.040 + intensity * 0.016 + sheen * 0.030 + diagonal_sheen * 0.012);
    return float4(color * alpha, alpha);
}
"#;

const UI_SHADER_SOURCE: &str = r#"
Texture2D ui_texture : register(t0);
SamplerState ui_sampler : register(s0);

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

VsOut vs_main(float2 position : POSITION, float2 uv : TEXCOORD0) {
    VsOut output;
    output.position = float4(position, 0.0, 1.0);
    output.uv = uv;
    return output;
}

float4 ps_main(VsOut input) : SV_Target {
    float4 color = ui_texture.Sample(ui_sampler, input.uv);
    return color;
}
"#;

pub fn presenter_kind() -> WindowsPresenterKind {
    WindowsPresenterKind::CompositionVisualTree
}

fn create_d3d11_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device = None;
    let mut context = None;
    let mut feature_level = Default::default();

    let hardware = unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut feature_level),
            Some(&mut context),
        )
    };

    if hardware.is_err() {
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut feature_level),
                Some(&mut context),
            )
            .context("fallback D3D11CreateDevice with WARP")?;
        }
    }

    let device = device.ok_or_else(|| anyhow!("D3D11CreateDevice returned no device"))?;
    let context = context.ok_or_else(|| anyhow!("D3D11CreateDevice returned no immediate context"))?;
    Ok((device, context))
}

fn create_effect_pipeline(
    device: &ID3D11Device,
) -> Result<(ID3D11VertexShader, ID3D11PixelShader, ID3D11Buffer, ID3DBlob)> {
    let vertex_shader_blob =
        compile_shader(EFFECT_SHADER_SOURCE, "vs_main", "vs_5_0").context("Compile composition vertex shader")?;
    let pixel_shader_blob =
        compile_shader(EFFECT_SHADER_SOURCE, "ps_main", "ps_5_0").context("Compile composition pixel shader")?;

    let mut vertex_shader = None;
    let mut pixel_shader = None;
    let mut uniform_buffer = None;
    let uniform_desc = D3D11_BUFFER_DESC {
        ByteWidth: std::mem::size_of::<EffectUniforms>() as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: 0,
        StructureByteStride: 0,
    };

    unsafe {
        device
            .CreateVertexShader(
                std::slice::from_raw_parts(
                    vertex_shader_blob.GetBufferPointer() as *const u8,
                    vertex_shader_blob.GetBufferSize(),
                ),
                None,
                Some(&mut vertex_shader),
            )
            .context("CreateVertexShader for composition effect")?;
        device
            .CreatePixelShader(
                std::slice::from_raw_parts(
                    pixel_shader_blob.GetBufferPointer() as *const u8,
                    pixel_shader_blob.GetBufferSize(),
                ),
                None,
                Some(&mut pixel_shader),
            )
            .context("CreatePixelShader for composition effect")?;
        device
            .CreateBuffer(&uniform_desc, None, Some(&mut uniform_buffer))
            .context("CreateBuffer for composition effect uniforms")?;
    }

    Ok((
        vertex_shader.ok_or_else(|| anyhow!("CreateVertexShader returned no shader"))?,
        pixel_shader.ok_or_else(|| anyhow!("CreatePixelShader returned no shader"))?,
        uniform_buffer.ok_or_else(|| anyhow!("CreateBuffer returned no uniform buffer"))?,
        vertex_shader_blob,
    ))
}

fn create_ui_pipeline(
    device: &ID3D11Device,
) -> Result<(ID3D11VertexShader, ID3D11PixelShader, ID3D11SamplerState)> {
    let vertex_shader_blob =
        compile_shader(UI_SHADER_SOURCE, "vs_main", "vs_5_0").context("Compile composition UI vertex shader")?;
    let pixel_shader_blob =
        compile_shader(UI_SHADER_SOURCE, "ps_main", "ps_5_0").context("Compile composition UI pixel shader")?;

    let mut vertex_shader = None;
    let mut pixel_shader = None;
    let mut sampler = None;
    let sampler_desc = D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
        MinLOD: 0.0,
        MaxLOD: f32::MAX,
        ..Default::default()
    };

    unsafe {
        device
            .CreateVertexShader(
                std::slice::from_raw_parts(
                    vertex_shader_blob.GetBufferPointer() as *const u8,
                    vertex_shader_blob.GetBufferSize(),
                ),
                None,
                Some(&mut vertex_shader),
            )
            .context("CreateVertexShader for composition UI")?;
        device
            .CreatePixelShader(
                std::slice::from_raw_parts(
                    pixel_shader_blob.GetBufferPointer() as *const u8,
                    pixel_shader_blob.GetBufferSize(),
                ),
                None,
                Some(&mut pixel_shader),
            )
            .context("CreatePixelShader for composition UI")?;
        device
            .CreateSamplerState(&sampler_desc, Some(&mut sampler))
            .context("CreateSamplerState for composition UI")?;
    }

    Ok((
        vertex_shader.ok_or_else(|| anyhow!("CreateVertexShader returned no UI vertex shader"))?,
        pixel_shader.ok_or_else(|| anyhow!("CreatePixelShader returned no UI pixel shader"))?,
        sampler.ok_or_else(|| anyhow!("CreateSamplerState returned no UI sampler"))?,
    ))
}

fn compile_shader(source: &str, entry: &str, target: &str) -> Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    let source_name = b"stellatune-composition-effect.hlsl\0";
    let entry = format!("{entry}\0");
    let target = format!("{target}\0");

    let result = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            PCSTR(source_name.as_ptr()),
            None,
            None,
            PCSTR(entry.as_ptr()),
            PCSTR(target.as_ptr()),
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };

    match result {
        Ok(()) => code.ok_or_else(|| anyhow!("D3DCompile returned no shader blob")),
        Err(error) => {
            let detail = errors
                .and_then(|blob| unsafe {
                    let ptr = blob.GetBufferPointer() as *const u8;
                    let len = blob.GetBufferSize();
                    std::str::from_utf8(std::slice::from_raw_parts(ptr, len))
                        .ok()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| error.message().to_string());
            Err(anyhow!("D3DCompile failed for {entry}/{target}: {detail}"))
        },
    }
}

fn create_fullscreen_geometry(
    device: &ID3D11Device,
    vertex_shader_blob: &ID3DBlob,
) -> Result<(ID3D11InputLayout, ID3D11Buffer)> {
    let vertices = [
        FullscreenVertex {
            position: [-1.0, -1.0],
            uv: [0.0, 1.0],
        },
        FullscreenVertex {
            position: [3.0, -1.0],
            uv: [2.0, 1.0],
        },
        FullscreenVertex {
            position: [-1.0, 3.0],
            uv: [0.0, -1.0],
        },
    ];
    let position_semantic = b"POSITION\0";
    let texcoord_semantic = b"TEXCOORD\0";
    let layout_desc = [
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: PCSTR(position_semantic.as_ptr()),
            SemanticIndex: 0,
            Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 0,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: PCSTR(texcoord_semantic.as_ptr()),
            SemanticIndex: 0,
            Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 8,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
    ];
    let buffer_desc = D3D11_BUFFER_DESC {
        ByteWidth: std::mem::size_of_val(&vertices) as u32,
        Usage: D3D11_USAGE_IMMUTABLE,
        BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
        StructureByteStride: std::mem::size_of::<FullscreenVertex>() as u32,
    };
    let initial_data = D3D11_SUBRESOURCE_DATA {
        pSysMem: vertices.as_ptr().cast(),
        ..Default::default()
    };
    let mut input_layout = None;
    let mut vertex_buffer = None;

    unsafe {
        device
            .CreateInputLayout(
                &layout_desc,
                std::slice::from_raw_parts(
                    vertex_shader_blob.GetBufferPointer() as *const u8,
                    vertex_shader_blob.GetBufferSize(),
                ),
                Some(&mut input_layout),
            )
            .context("CreateInputLayout for composition fullscreen geometry")?;
        device
            .CreateBuffer(&buffer_desc, Some(&initial_data), Some(&mut vertex_buffer))
            .context("CreateBuffer for composition fullscreen geometry")?;
    }

    Ok((
        input_layout.ok_or_else(|| anyhow!("CreateInputLayout returned no input layout"))?,
        vertex_buffer.ok_or_else(|| anyhow!("CreateBuffer returned no fullscreen vertex buffer"))?,
    ))
}

fn create_draw_states(
    device: &ID3D11Device,
) -> Result<(ID3D11RasterizerState, ID3D11BlendState)> {
    let raster_desc = D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_NONE,
        FrontCounterClockwise: false.into(),
        DepthClipEnable: true.into(),
        ..Default::default()
    };
    let blend_desc = D3D11_BLEND_DESC {
        AlphaToCoverageEnable: false.into(),
        IndependentBlendEnable: false.into(),
        RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            SrcBlend: D3D11_BLEND_ONE,
            DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D11_BLEND_OP_ADD,
            SrcBlendAlpha: D3D11_BLEND_ONE,
            DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOpAlpha: D3D11_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
        }; 8],
    };
    let mut rasterizer_state = None;
    let mut blend_state = None;
    unsafe {
        device
            .CreateRasterizerState(&raster_desc, Some(&mut rasterizer_state))
            .context("CreateRasterizerState for composition draw states")?;
        device
            .CreateBlendState(&blend_desc, Some(&mut blend_state))
            .context("CreateBlendState for composition draw states")?;
    }
    Ok((
        rasterizer_state.ok_or_else(|| anyhow!("CreateRasterizerState returned no state"))?,
        blend_state.ok_or_else(|| anyhow!("CreateBlendState returned no state"))?,
    ))
}

fn update_effect_uniforms(
    context: &ID3D11DeviceContext,
    buffer: &ID3D11Buffer,
    viewport: PhysicalSize<u32>,
    frame: &EffectFrame,
) -> Result<()> {
    let uniforms = EffectUniforms {
        viewport: [viewport.width.max(1) as f32, viewport.height.max(1) as f32],
        pointer: frame.pointer,
        clear_color: frame.clear_color,
        accent_color: frame.accent_color,
        glow_color: frame.glow_color,
        params: [frame.time, frame.intensity, 0.0, 0.0],
    };
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe {
        context
            .Map(buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))
            .context("Map composition effect uniform buffer")?;
        std::ptr::copy_nonoverlapping(
            (&uniforms as *const EffectUniforms).cast::<u8>(),
            mapped.pData.cast::<u8>(),
            std::mem::size_of::<EffectUniforms>(),
        );
        context.Unmap(buffer, 0);
    }
    Ok(())
}

fn create_ui_texture(
    device: &ID3D11Device,
    size: PhysicalSize<u32>,
) -> Result<(ID3D11Texture2D, ID3D11ShaderResourceView)> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: size.width,
        Height: size.height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: 0,
    };
    let mut texture = None;
    let mut view = None;
    unsafe {
        device
            .CreateTexture2D(&desc, None, Some(&mut texture))
            .context("CreateTexture2D for composition UI")?;
    }
    let texture = texture.ok_or_else(|| anyhow!("CreateTexture2D returned no UI texture"))?;
    unsafe {
        device
            .CreateShaderResourceView(&texture, None, Some(&mut view))
            .context("CreateShaderResourceView for composition UI texture")?;
    }
    let view = view.ok_or_else(|| anyhow!("CreateShaderResourceView returned no UI SRV"))?;
    Ok((texture, view))
}

fn create_effect_swapchain(
    factory: &IDXGIFactory2,
    dxgi_device: &IDXGIDevice,
) -> Result<IDXGISwapChain1> {
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: 1,
        Height: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        Flags: 0,
    };

    unsafe {
        factory
            .CreateSwapChainForComposition(dxgi_device, &desc, None)
            .context("CreateSwapChainForComposition")
    }
}

impl CompositionPresenterRuntime {
    fn ensure_ui_texture(&mut self, size: PhysicalSize<u32>) -> Result<()> {
        let needs_recreate = match self.ui_texture.as_ref() {
            Some(texture) => {
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                unsafe {
                    texture.GetDesc(&mut desc);
                }
                desc.Width != size.width || desc.Height != size.height
            },
            None => true,
        };
        if !needs_recreate && self.ui_shader_resource.is_some() {
            return Ok(());
        }
        let (texture, view) = create_ui_texture(&self.d3d11_device, size)?;
        self.ui_texture = Some(texture);
        self.ui_shader_resource = Some(view);
        Ok(())
    }

    fn upload_ui_frame(&mut self, frame: &UiFrame) -> Result<()> {
        let texture = self
            .ui_texture
            .as_ref()
            .ok_or_else(|| anyhow!("composition UI texture is not initialized"))?;
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.d3d11_context
                .Map(texture, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))
                .context("Map composition UI texture")?;
            let dst_stride = mapped.RowPitch as usize;
            let src_stride = frame.row_bytes;
            let row_bytes = src_stride.min(dst_stride);
            for row in 0..frame.height as usize {
                let src = frame.pixels.as_ptr().add(row * src_stride);
                let dst = mapped.pData.cast::<u8>().add(row * dst_stride);
                std::ptr::copy_nonoverlapping(src, dst, row_bytes);
            }
            self.d3d11_context.Unmap(texture, 0);
        }
        Ok(())
    }
}

fn apply_dwm_window_attributes(
    hwnd: HWND,
    config: &CompositionPresenterConfig,
) -> Result<(DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE)> {
    let backdrop_type = if config.backdrop_visual_enabled {
        if config.heavy_fx_visual_enabled {
            DWMSBT_TRANSIENTWINDOW
        } else {
            DWMSBT_MAINWINDOW
        }
    } else {
        DWMSBT_NONE
    };
    let corner_preference = if config.true_window_rounding {
        DWMWCP_ROUND
    } else {
        DWMWCP_ROUNDSMALL
    };

    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&backdrop_type as *const DWM_SYSTEMBACKDROP_TYPE).cast(),
            std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        )
        .context("DwmSetWindowAttribute system backdrop type")?;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corner_preference as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        )
        .context("DwmSetWindowAttribute corner preference")?;
    }

    Ok((backdrop_type, corner_preference))
}
