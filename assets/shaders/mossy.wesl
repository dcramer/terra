// Rocks of the forest floor (D246): their scanned colour, relief and roughness, with moss
// grown over what faces the sky, as it does on every stone left lying in a damp wood. Where
// it grows is the surface's own facing, its relief included, so moss fills the tops and
// the ledges and gives out down the sides in a ragged edge, broken into patches by noise in
// the world.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
};

// Field order must match MossExtension in lush/floor.rs.
struct Moss {
    // Its colour in rgb, and how much grows in a.
    colour: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> moss: Moss;

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash(cell), hash(cell + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash(cell + vec3<f32>(0.0, 1.0, 0.0)), hash(cell + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash(cell + vec3<f32>(0.0, 0.0, 1.0)), hash(cell + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash(cell + vec3<f32>(0.0, 1.0, 1.0)), hash(cell + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let world = in.world_position.xyz;
    // The planet's centre is the origin, so up is away from it.
    let up = normalize(world);
    let facing = dot(pbr_input.N, up);
    let patches = noise(world * 1.6) * 0.5 + noise(world * 4.7 + 11.0) * 0.3 + noise(world * 17.0 + 3.0) * 0.2;
    let grows = smoothstep(0.35, 0.6, facing + (patches - 0.5) * 1.2) * moss.colour.a;
    if grows > 0.0 {
        // Cushions of it, darker between them, yellower where it is thin.
        let cushions = noise(world * 9.0 + 5.0) * 0.6 + noise(world * 31.0) * 0.4;
        let base = pbr_input.material.base_color.rgb;
        let stone = dot(base, vec3<f32>(0.33));
        let thin = 1.0 - smoothstep(0.35, 0.9, grows);
        let colour = moss.colour.rgb * (0.45 + 0.9 * cushions) * (0.6 + 0.8 * stone / max(stone + 0.1, 0.02))
            * mix(vec3<f32>(1.0), vec3<f32>(1.25, 1.1, 0.7), thin);
        pbr_input.material.base_color = vec4<f32>(mix(base, colour, grows), pbr_input.material.base_color.a);
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 1.0, grows);
        // A cushion of moss is softer than the stone's relief.
        pbr_input.N = normalize(mix(pbr_input.N, pbr_input.world_normal, grows * 0.5));
    }
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
