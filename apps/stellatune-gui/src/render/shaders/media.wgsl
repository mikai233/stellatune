struct MediaUniforms {
  resolution_and_origin: vec4<f32>,
  rect_size_and_radius: vec4<f32>,
};

@group(0) @binding(0)
var linear_sampler: sampler;

@group(0) @binding(1)
var source_texture: texture_2d<f32>;

@group(0) @binding(2)
var<uniform> uniforms: MediaUniforms;

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

fn rounded_rect_mask(local: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
  let q = abs(local - size * 0.5) - (size * 0.5 - vec2<f32>(radius, radius));
  let outside = max(q, vec2<f32>(0.0, 0.0));
  let distance = length(outside) - radius;
  return 1.0 - smoothstep(0.0, 1.5, distance);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  let resolution = uniforms.resolution_and_origin.xy;
  let rect_origin = uniforms.resolution_and_origin.zw;
  let rect_size = uniforms.rect_size_and_radius.xy;
  let corner_radius = uniforms.rect_size_and_radius.z;
  let pixel = in.uv * resolution;
  let local = pixel - rect_origin;

  if any(local < vec2<f32>(0.0, 0.0)) || any(local > rect_size) {
    return vec4<f32>(0.0);
  }

  let sample_uv = local / rect_size;
  let source = textureSample(source_texture, linear_sampler, sample_uv);
  let mask = rounded_rect_mask(local, rect_size, corner_radius);
  return vec4<f32>(source.rgb, source.a * mask);
}
