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
// motion inside it streaked along the swing, its tail dissolved into the streaks, drawn
// premultiplied (the body hides a little of what is behind it).
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
        let s = look.lit.w;
        // Streaks along the swing (noise stretched along u, fine across v), as a motion blur's.
        let n = 0.62 * noise(vec2(age * 3.0 + s, v * 11.0)) + 0.38 * noise(vec2(age * 9.0, v * 29.0 + s));
        // The dissolve a ribbon's tail breaks into wisps by (Niagara's ribbon examples): the
        // threshold climbs with age, so the old trail is eroded into its streaks, never faded
        // whole.
        let t = 0.88 * pow(age, 1.25) - 0.08;
        let kept = smoothstep(t - 0.06, t + 0.14, n);
        let along = pow(max(1.0 - age, 0.0), look.lit.x);
        // The body: from nothing at the hilt, thickening outward, fading out past the edge
        // (a motion blur's soft border, not a ribbon's cut).
        let out = 1.0 - smoothstep(look.core.w, 1.0, v);
        let body = smoothstep(0.04, look.core.w * 0.72, v) * out * kept;
        // The hot edge near the tip, and a hot head at the blade.
        let d = (v - look.core.w) / max(look.rim.w, 0.001);
        let rim = exp(-d * d) * (1.0 - smoothstep(0.25, 0.85, age)) * min(0.75 + 0.5 * n, 1.0);
        let head = exp(-(age / 0.05) * (age / 0.05)) * smoothstep(0.25, 0.85, v) * out;
        let hot = min(rim + 0.7 * head, 1.0);
        // The body darkens to a deep red down the trail.
        let deep = look.rim.rgb * mix(vec3(1.0), vec3(0.55, 0.18, 0.45), age);
        let rgb = deep * body * look.shape.w * (0.8 + 0.4 * n) + look.core.rgb * hot;
        // Premultiplied: the body hides some of what is behind it, so it stays orange-red over
        // a sunlit floor rather than washing to peach as a purely additive strip did.
        let cover = max(0.7 * body, 0.3 * hot) * along * strength;
        return vec4(rgb * along * strength * view.exposure, cover);
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
