// A burning prop's glowing part (crates/terra_world/src/fire/glow.rs): drawn as Bevy's
// StandardMaterial draws it, the share of its light its glow gives added to the flames' mask
// in the alpha (crates/terra_world/src/fire/film.rs), so the tonemapper takes that share
// through the film, ungraded, as it takes a flame.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::{view, globals},
};

// Field order must match Glow in glow.rs.
struct Glow {
    // x how much of the glow's share goes to the mask; y how deep its embers' crawl (0 none,
    // a campfire's logs' cracks), z the crawl's patches a metre, w their rate a second.
    mask: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> glow: Glow;

fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3(127.1, 311.7, 74.7))) * 43758.5453);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash3(i), hash3(i + vec3(1.0, 0.0, 0.0)), u.x), mix(hash3(i + vec3(0.0, 1.0, 0.0)), hash3(i + vec3(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(hash3(i + vec3(0.0, 0.0, 1.0)), hash3(i + vec3(1.0, 0.0, 1.0)), u.x), mix(hash3(i + vec3(0.0, 1.0, 1.0)), hash3(i + vec3(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

// How hot a log's embers are here and now, about 1 on the whole: a burning log's cracks crawl,
// patches flaring and dulling over a second or two as the air reaches them (RDR2's logs' crackle;
// g66: ours glowed evenly, so the cracks read as a dark painted pattern).
fn crawl(at: vec3<f32>) -> f32 {
    let p = at * glow.mask.z;
    let s = globals.time * glow.mask.w;
    let n = 0.65 * noise3(p + vec3(0.0, s, 0.37 * s)) + 0.35 * noise3(p * 2.3 - vec3(0.8 * s, 0.0, 0.0));
    return mix(1.0, 0.3 + 1.9 * smoothstep(0.32, 0.72, n), glow.mask.y);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);
    if glow.mask.y > 0.0 {
        pbr_input.material.emissive = vec4(pbr_input.material.emissive.rgb * crawl(in.world_position.xyz), pbr_input.material.emissive.a);
    }
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    // Its glow as apply_pbr_lighting adds it: the emissive, exposed by its alpha's share.
    let emissive = pbr_input.material.emissive;
    let own = emissive.rgb * pbr_input.material.base_color.a * mix(1.0, view.exposure, emissive.a);
    let luma = vec3(0.2126, 0.7152, 0.0722);
    let share = clamp(dot(own, luma) / max(dot(out.color.rgb, luma), 1e-6), 0.0, 1.0);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    out.color.a = 1.0 + share * glow.mask.x;
    return out;
}
