struct EffectUniforms {
  resolution: vec2<f32>,
  time_seconds: f32,
  beat_phase: f32,
  delta_seconds: f32,
  scale_factor: f32,
  frame_phase: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: EffectUniforms;

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
  let centered = uv - vec2<f32>(0.5, 0.5);
  let radial = length(centered);
  let time_scale = 1.0 + clamp((uniforms.scale_factor - 1.0) * 0.06, -0.1, 0.1);
  let drift = vec2<f32>(
    sin(uniforms.time_seconds * 0.31 * time_scale + uv.y * 7.0),
    cos(uniforms.time_seconds * 0.27 * time_scale + uv.x * 5.0)
  ) * 0.08;

  let glow = smoothstep(0.58, 0.0, length(centered + drift));
  let sweep = 0.5 + 0.5 * sin(uniforms.time_seconds * 0.8 + uv.x * 12.0 - uv.y * 6.0 + uniforms.frame_phase * 0.015625);
  let pulse = 0.5 + 0.5 * uniforms.beat_phase;
  let frame_pulse = 0.5 + 0.5 * sin(uniforms.time_seconds * (0.9 + uniforms.delta_seconds * 24.0));

  let deep = vec3<f32>(0.04, 0.06, 0.12);
  let teal = vec3<f32>(0.05, 0.42, 0.42);
  let coral = vec3<f32>(0.92, 0.36, 0.38);
  let gold = vec3<f32>(0.98, 0.74, 0.25);

  let gradient = mix(deep, teal, uv.y);
  let ribbon = mix(coral, gold, sweep);
  let color = gradient
    + ribbon * glow * (0.35 + pulse * 0.25)
    + vec3<f32>(0.0, 0.03, 0.05) * (1.0 - radial)
    + vec3<f32>(0.02, 0.015, 0.01) * frame_pulse * glow;
  return vec4<f32>(color, 1.0);
}
