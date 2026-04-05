@group(0) @binding(0)
var linear_sampler: sampler;

@group(0) @binding(1)
var effect_texture: texture_2d<f32>;

@group(0) @binding(2)
var media_texture: texture_2d<f32>;

@group(0) @binding(3)
var vector_texture: texture_2d<f32>;

struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

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
  let vector = textureSample(vector_texture, linear_sampler, uv);
  let media_mix = effect.rgb * (1.0 - media.a) + media.rgb * media.a;
  let color = media_mix * (1.0 - vector.a) + vector.rgb;
  return vec4<f32>(color, 1.0);
}
