// The kit's effects (kit/effects.rs, D634): one shader for every quad an effect draws, as the
// shipped ARPGs build theirs from a few soft sprites over bloom (Diablo IV's and Path of
// Exile 2's hits and slashes; Unreal's Niagara soft particles). Values are nits, times the
// view's exposure, so a preset reads the same under any biome's light.
//
// Mode 0, a glow: a white-hot core (a narrow Gaussian) inside a tinted halo (a wide one),
// windowed to nothing well inside the quad, added over the frame (premultiplied, alpha 0).
// Mode 1, smoke or steam: a soft noisy puff, lit by what the CPU summed of the lights at it
// (kit/effects.rs `lit_at`), drawn over the frame (premultiplied).
// Mode 2, a trail (u the age, v across from the hilt to the edge): a hot edge, a smear of
// motion inside it streaked along the swing, fading back from the blade, added.
// Every mode fades where it nears what is behind it (a soft particle, the depth prepass's
// view depth against its own), so a puff or a trail never cuts a hard line into the floor.

import bevy_pbr::render::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
    view_transformations::depth_ndc_to_view_z,
};
@if(DEPTH_PREPASS) import bevy_pbr::prepass::utils::prepass_depth;

// Must match effects.rs `Look`, field for field.
struct Look {
    // rgb the core's nits; w the core's radius (a fraction of the half-size), or the
    // trail's edge (v).
    core: vec4<f32>,
    // rgb the halo's (the rim's) nits; w its falloff (exp(-k r^2)), or the trail's width.
    rim: vec4<f32>,
    // x the strength (the fade, 0 to 1), y the soft distance (m), z the mode, w the grain
    // (noise's share), or the trail's smear.
    shape: vec4<f32>,
    // rgb the light on a puff (nits); w a seed. A trail: x its tail's exponent.
    lit: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var<uniform> look: Look;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2(1.0, 0.0));
    let c = hash(i + vec2(0.0, 1.0));
    let d = hash(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var soft = 1.0;
    @if(DEPTH_PREPASS) {
        let behind = depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        let here = depth_ndc_to_view_z(in.position.z);
        soft = saturate((here - behind) / max(look.shape.y, 0.001));
    }
    let mode = i32(look.shape.z + 0.5);
    let strength = look.shape.x * soft;
    if mode == 2 {
        let age = in.uv.x;
        let v = in.uv.y;
        let d = (v - look.core.w) / max(look.rim.w, 0.001);
        let edge = exp(-d * d);
        // The smear: from nothing at the hilt to full at the edge, then gone past it.
        let smear = smoothstep(0.0, look.core.w, v) * (1.0 - smoothstep(look.core.w, 1.0, v));
        // Streaks along the swing (noise stretched along u), as a motion blur's.
        let streak = 0.55 + 0.9 * noise(vec2(v * 22.0, age * 3.0 + look.lit.w));
        let along = pow(max(1.0 - age, 0.0), look.lit.x);
        let head = pow(max(1.0 - age, 0.0), 6.0);
        let hot = mix(look.rim.rgb, look.core.rgb, saturate(edge * head));
        let rgb = (hot * edge + look.rim.rgb * smear * look.shape.w * streak) * along;
        return vec4(rgb * strength * view.exposure, 0.0);
    }
    let p = in.uv * 2.0 - 1.0;
    let r2 = dot(p, p);
    if mode == 1 {
        let s = look.lit.w;
        let n = 0.6 * noise(p * 2.3 + vec2(s, s * 1.7)) + 0.4 * noise(p * 5.1 + vec2(s * 2.3, s));
        let body = saturate(1.0 - r2);
        let a = saturate(strength * body * body * mix(1.0, n * 1.8, look.shape.w));
        return vec4(look.lit.rgb * view.exposure * a, a);
    }
    let window = 1.0 - smoothstep(0.55, 1.0, r2);
    let halo = exp(-look.rim.w * r2) * window;
    let c = max(look.core.w, 0.01);
    let core = exp(-r2 / (c * c));
    let rgb = look.rim.rgb * halo + look.core.rgb * core;
    return vec4(rgb * strength * view.exposure, 0.0);
}
