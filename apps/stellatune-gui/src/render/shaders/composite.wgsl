@group(0) @binding(0)
var linear_sampler: sampler;

@group(0) @binding(1)
var effect_texture: texture_2d<f32>;

@group(0) @binding(2)
var media_texture: texture_2d<f32>;

@group(0) @binding(3)
var source_vector_texture: texture_2d<f32>;

@group(0) @binding(4)
var destination_vector_texture: texture_2d<f32>;

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
  _padding0: f32,
  _padding1: f32,
  _padding2: f32,
}

@group(0) @binding(5)
var<uniform> settings: CompositeSettings;

struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

fn composite(base: vec4<f32>, overlay: vec4<f32>) -> vec4<f32> {
  let out_a = overlay.a + base.a * (1.0 - overlay.a);
  let out_rgb = base.rgb * (1.0 - overlay.a) + overlay.rgb * overlay.a;
  return vec4<f32>(out_rgb, out_a);
}

fn sample_page(
  page_texture: texture_2d<f32>,
  uv: vec2<f32>,
  opacity: f32,
  translation_uv: vec2<f32>,
  scale: f32,
) -> vec4<f32> {
  let safe_scale = max(scale, 0.0001);
  let centered = uv - vec2<f32>(0.5, 0.5) - translation_uv;
  let sample_uv = centered / safe_scale + vec2<f32>(0.5, 0.5);
  let inside = all(sample_uv >= vec2<f32>(0.0, 0.0)) && all(sample_uv <= vec2<f32>(1.0, 1.0));
  if !inside {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
  }

  return textureSample(page_texture, linear_sampler, sample_uv) * opacity;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
  var positions = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -3.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>( 3.0,  1.0),
  );

  let clip = positions[vertex_index];
  var out: VertexOutput;
  out.clip_position = vec4<f32>(clip, 0.0, 1.0);
  out.uv = clip * 0.5 + vec2<f32>(0.5, 0.5);
  return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  let uv = in.uv;
  let effect = textureSample(effect_texture, linear_sampler, uv);
  let media = textureSample(media_texture, linear_sampler, uv);
  let source_vector = sample_page(
    source_vector_texture,
    uv,
    settings.source_opacity,
    vec2<f32>(settings.source_translate_x, settings.source_translate_y),
    settings.source_scale,
  );
  let destination_vector = sample_page(
    destination_vector_texture,
    uv,
    settings.destination_opacity,
    vec2<f32>(settings.destination_translate_x, settings.destination_translate_y),
    settings.destination_scale,
  );

  var color = effect;

  if settings.media_on_top > 0.5 {
    color = composite(color, source_vector);
    color = composite(color, destination_vector);
    color = composite(color, media);
  } else {
    color = composite(color, media);
    color = composite(color, source_vector);
    color = composite(color, destination_vector);
  }

  return vec4<f32>(color.rgb, 1.0);
}
