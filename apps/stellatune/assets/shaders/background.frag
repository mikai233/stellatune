#version 460 core

#include <flutter/runtime_effect.glsl>

uniform vec2 uSize;
uniform float uTime;
uniform vec4 uColor1;
uniform vec4 uColor2;
uniform vec4 uColor3;
uniform vec4 uColor4;

out vec4 fragColor;

float hash(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * .1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// 2D Noise function with smoother interpolation
float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i + vec2(0.0, 0.0)), 
                   hash(i + vec2(1.0, 0.0)), u.x),
               mix(hash(i + vec2(0.0, 1.0)), 
                   hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

// Fractional Brownian Motion with 5 octaves for high detail
float fbm(vec2 p) {
    float v = 0.0;
    float a = 0.5;
    mat2 m = mat2(1.6, 1.2, -1.2, 1.6);
    for (int i = 0; i < 5; i++) {
        v += a * noise(p);
        p = m * p;
        a *= 0.5;
    }
    return v;
}

void main() {
    vec2 uv = FlutterFragCoord().xy / uSize;
    float time = uTime * 0.12; // Slightly slower for more elegance

    // Domain Warping for organic color flow
    vec2 q = vec2(fbm(uv + time * 0.1), fbm(uv + vec2(5.2, 1.3) + time * 0.15));
    vec2 r = vec2(fbm(uv + 3.0 * q + vec2(1.7, 9.2) + time * 0.08), 
                  fbm(uv + 3.0 * q + vec2(8.3, 2.8) + time * 0.04));
    
    float f = fbm(uv + r * 0.5);

    // Color mixing centers
    vec2 c1 = vec2(0.2, 0.2) + 0.2 * q;
    vec2 c2 = vec2(0.8, 0.2) + 0.2 * r;
    vec2 c3 = vec2(0.2, 0.8) - 0.2 * q;
    vec2 c4 = vec2(0.8, 0.8) - 0.2 * r;

    float d1 = 1.0 / pow(max(distance(uv, c1), 0.15), 1.8);
    float d2 = 1.0 / pow(max(distance(uv, c2), 0.15), 1.8);
    float d3 = 1.0 / pow(max(distance(uv, c3), 0.15), 1.8);
    float d4 = 1.0 / pow(max(distance(uv, c4), 0.15), 1.8);

    float sum = d1 + d2 + d3 + d4;
    vec3 baseColor = (uColor1.rgb * d1 + uColor2.rgb * d2 + uColor3.rgb * d3 + uColor4.rgb * d4) / sum;

    // Advanced color blending
    vec3 color = mix(baseColor, uColor2.rgb, f * 0.4);
    color = mix(color, uColor4.rgb, dot(q, r) * 0.3);
    color = mix(color, uColor3.rgb, q.y * 0.2);
    
    // Triangular Dither to eliminate banding
    // Using two hashes to create a triangular distribution of noise
    float rand1 = hash(uv + fract(uTime));
    float rand2 = hash(uv - fract(uTime));
    float dither = (rand1 + rand2 - 1.0) / 255.0;
    color += dither;

    fragColor = vec4(color, 1.0);
}
