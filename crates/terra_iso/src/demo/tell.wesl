// A foe's ground tell (tell.rs): the standard material's unlit colour (the biome's, times
// the sector's vertex alpha), laid on the ground as light, not paint. The judge of j25 saw
// a flat UI decal: hard straight sides, an even fill and a clean inner rim. So its sides
// soften over the last tenth of the arc (uv.y the angle across it, 0 to 1), the fill glows
// over the litter (premultiplied: its colour added, the ground under it dimmed by 0.6 of its
// alpha, as Diablo IV's cones let the ground show through; at 0.3 it all but vanished, j29) in patches of world
// noise, and the rim's inner border is broken by the same noise (uv.x 0 at its inner edge,
// 1 at the reach), so it reads as embers along the ground's own grain.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::main_pass_post_lighting_processing,
};

// x: 1 the fill, 0 the rim; y: the noise's frequency (1/m); z: how much it breaks (0..1).
@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> look: vec4<f32>;

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

fn fbm(p: vec2<f32>) -> f32 {
    return 0.55 * noise(p) + 0.3 * noise(p * 2.13 + 7.1) + 0.15 * noise(p * 4.37 + 3.3);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let c = pbr_input.material.base_color;
    let across = in.uv.y;
    let sides = smoothstep(0.0, 0.1, across) * smoothstep(0.0, 0.1, 1.0 - across);
    let n = fbm(in.world_position.xz * look.y);
    var out: FragmentOutput;
    if look.x > 0.5 {
        let a = clamp(c.a * sides * mix(1.0, 0.35 + 1.3 * n, look.z), 0.0, 1.0);
        out.color = vec4<f32>(c.rgb * a, 0.6 * a);
    } else {
        // The inner border wanders out by up to half the rim with the noise; the reach's
        // own edge stays clean, the line the blow judges.
        let inner = smoothstep(n * look.z * 0.6, n * look.z * 0.6 + 0.3, in.uv.x);
        out.color = vec4<f32>(c.rgb, c.a * sides * inner);
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
