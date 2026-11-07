// The kit's effects (kit/effects.rs, D634): one shader for every quad an effect draws, as the
// shipped ARPGs build theirs from a few soft sprites over bloom (Diablo IV's and Path of
// Exile 2's hits and slashes; Unreal's Niagara soft particles). Values are nits, times the
// view's exposure, so a preset reads the same under any biome's light.
//
// Mode 0, a glow: a white-hot core (a narrow Gaussian) inside a tinted halo (a wide one),
// windowed to nothing well inside the quad, added over the frame (premultiplied, alpha 0).
// Mode 1, smoke or steam: a soft noisy puff, lit by what the CPU summed of the lights at it
// (kit/effects.rs `lit_at`), drawn over the frame (premultiplied).
// Mode 2, a trail (u the age, v across from the hilt to the edge): a blade's motion streak,
// a thin hot glint along the edge's path in a feathered glow, a faint blur of the blade
// inside it, fading from the blade back, nearly additive.
// Mode 3, a streak (a projectile's trail, u the age from the head, v across): a hot line
// down the middle in a narrow halo narrowing down the trail, the tail eroded into wisps.
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
        // A lit blade's motion streak, as a camera's shutter draws one (iso-kit j43; the
        // judges' "flat, hard-edged crescent" was a ribbon of solid body with a cut border):
        // a thin hot glint along the path of the edge (`edge`, a Gaussian `width` across), a
        // feathered falloff about it three times as wide, a faint blur of the blade inward
        // (`smear`), all fading from the blade back down its age and gone before either
        // border of the strip, so no edge of it shows.
        // Fine streaks along the swing (noise long in u, fine across v), as a motion blur's.
        let n = 0.6 * noise(vec2(age * 2.0 + s, v * 23.0)) + 0.4 * noise(vec2(age * 5.0, v * 47.0 + s));
        let streak = 0.85 + 0.3 * n;
        let along = pow(max(1.0 - age, 0.0), look.lit.x);
        // No cut where the blade stands now: the blur's leading edge is soft too.
        let lead = smoothstep(0.0, 0.04, age);
        let window = smoothstep(0.0, 0.15, v) * (1.0 - smoothstep(0.86, 1.0, v));
        let w = max(look.rim.w, 0.001);
        let d = (v - look.core.w) / w;
        let f = (v - look.core.w) / (3.0 * w);
        // The glint cools first: hot only over the newest part of the streak.
        let hot = exp(-d * d) * (1.0 - smoothstep(0.15, 0.6, age)) * streak;
        let feather = exp(-f * f);
        // The blade's own blur, rising from the hilt to the edge, the feather's beyond it.
        let blade = select(smoothstep(0.0, look.core.w, v), feather, v > look.core.w);
        let glow = (0.6 * feather + look.shape.w * 0.4 * blade) * streak;
        // The glow cools down the streak from the rim's colour toward a dull red.
        let tint = look.rim.rgb * mix(vec3(1.0), vec3(0.6, 0.32, 0.25), age);
        let k = along * lead * window * strength;
        let rgb = (look.core.rgb * hot + tint * glow) * k;
        // Nearly additive: a light, not a ribbon; a little cover so it holds over a sunlit
        // floor.
        let cover = 0.12 * glow * k;
        return vec4(rgb * view.exposure, cover);
    }
    if mode == 3 {
        let age = in.uv.x;
        let v = in.uv.y;
        let x = abs(v * 2.0 - 1.0);
        let s = look.lit.w;
        let n = 0.62 * noise(vec2(age * 4.0 + s, v * 7.0)) + 0.38 * noise(vec2(age * 13.0, v * 19.0 + s));
        let t = 0.88 * pow(age, 1.25) - 0.08;
        // Eroded softly (iso-kit j43: the narrow ramp cut blotches with hard rims).
        let kept = smoothstep(t - 0.15, t + 0.3, n);
        let along = pow(max(1.0 - age, 0.0), look.lit.x);
        // The halo narrows down the trail as it cools, a narrow Gaussian across (a glow about
        // the line, not a band: iso-kit j43's "hard crescent"), long gone at the border.
        let xh = x / mix(1.0, 0.45, age);
        let halo = exp(-7.0 * xh * xh) * (1.0 - smoothstep(0.8, 1.0, xh)) * kept;
        // The hot line down the middle, thinning and cooling first.
        let c = x / max(look.rim.w * mix(1.0, 0.5, age), 0.001);
        let hot = exp(-c * c) * (1.0 - smoothstep(0.15, 0.6, age)) * min(0.75 + 0.5 * n, 1.0);
        let rgb = look.rim.rgb * halo * look.shape.w * (0.8 + 0.4 * n) + look.core.rgb * hot;
        let cover = 0.1 * halo * along * strength;
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
