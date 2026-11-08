// The spike's pond (water.rs): dark where deep, clear over silt in the shallows and gone
// at the bank, its face rippled by noise drifting with the wind so the sky and the sun
// break on it.

import bevy_pbr::render::{
    mesh_view_bindings::{globals, lights},
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> deep: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var<uniform> shallow: vec4<f32>;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// The ripples' height: three scales of noise drifting downwind at different speeds.
fn ripples(p: vec2<f32>, t: f32) -> f32 {
    let wind = vec2<f32>(0.8, 0.35);
    return noise(p * 0.9 + wind * t * 0.25) * 0.6
        + noise(p * 2.3 - wind.yx * t * 0.4) * 0.3
        + noise(p * 5.1 + wind * t * 0.7) * 0.1;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let p = in.uv;
    let depth = in.uv_b.x;
    let t = globals.time;

    let e = 0.04;
    let h = ripples(p, t);
    let gx = (ripples(p + vec2<f32>(e, 0.0), t) - h) / e;
    let gz = (ripples(p + vec2<f32>(0.0, e), t) - h) / e;
    // Calmer at the margin, where the reeds and the bank break the wind.
    let calm = smoothstep(0.02, 0.35, depth);
    let bend = shallow.a * mix(0.25, 1.0, calm);
    pbr_input.N = normalize(vec3<f32>(-gx * bend, 1.0, -gz * bend));

    // The colour: the silt seen through the shallows, the dark of the deep beyond.
    let k = smoothstep(0.0, deep.a, depth);
    var colour = mix(shallow.rgb, deep.rgb, k);
    // A wet dark line where it meets the bank, then nothing above.
    let alpha = smoothstep(-0.02, 0.10, depth) * mix(0.55, 0.97, smoothstep(0.0, 0.4, depth));
    pbr_input.material.base_color = vec4<f32>(colour, alpha);
    // What it mirrors, faked as a game would: the sky (the ambient's colour) out in the
    // open water, the dark of the wood near the banks and in broad drifting blots where the
    // crowns stand over it, all wobbled by the ripples. The lit reflectance is low, so this
    // is most of the sky in it; the sun's glints still come from the lights.
    let warp = vec2<f32>(gx, gz) * bend * 0.6;
    let open = smoothstep(0.08, 0.45, depth) * smoothstep(0.25, 0.65, noise(p * 0.28 + warp + 3.0));
    let sky = lights.ambient_color.rgb * mix(0.01, 0.075, open);
    let tilt = clamp(length(vec2<f32>(gx, gz)) * bend * 0.5, 0.0, 1.0);
    pbr_input.material.emissive = vec4<f32>(sky * (0.7 + 1.2 * tilt), 1.0);
    pbr_input.material.perceptual_roughness = 0.05 + 0.08 * (1.0 - calm);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
