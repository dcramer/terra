// A gate's veil (crates/terra_game/src/play/portal/veil.rs): on the flat sheet in the gate's
// opening, light drawn inward in slow arms: bright against the stone, deep toward the middle,
// seen through a little.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::globals,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::main_pass_post_lighting_processing,
};

// Field order must match Veil in veil.rs.
struct Veil {
    // rgb its colour, linear; w how fast its arms turn.
    glow: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> veil: Veil;

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

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    // The sheet's middle is the opening's: how far out, 0 to 1, and which way round.
    let at = in.uv * 2.0 - 1.0;
    let out_r = length(at);
    let round = atan2(at.y, at.x);
    let turn = globals.time * veil.glow.w * 6.2832;
    // Arms wound tighter toward the middle, turning, and drifting inward: noise read in that
    // wound frame, two sizes of it.
    let wound = round + 2.6 * (1.0 - out_r) + turn;
    let inward = out_r * 3.2 + globals.time * 0.22;
    let p = vec3(cos(wound) * inward, sin(wound) * inward, globals.time * 0.15);
    let n = 0.62 * noise3(p) + 0.38 * noise3(p * 2.6 + vec3(11.0, 3.0, 7.0));
    let arms = smoothstep(0.38, 0.78, n);
    // Deep in the middle, bright where it meets the stone.
    let depth = mix(0.18, 1.0, smoothstep(0.0, 0.95, out_r));
    let rim = smoothstep(0.8, 1.0, out_r);
    let light = veil.glow.rgb * ((0.3 + 1.7 * arms) * depth + 2.2 * rim * rim);
    // Seen through between the arms, more so at the middle; its edge soft against the ring.
    let alpha = (0.55 + 0.4 * arms) * mix(0.75, 1.0, out_r) * (1.0 - smoothstep(0.96, 1.0, out_r));

    var out: FragmentOutput;
    // Its own light, as an emissive not weighed by the exposure is: as bright by day as by night.
    out.color = vec4(light, alpha);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    out.color.a = alpha;
    return out;
}
