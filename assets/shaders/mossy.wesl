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
    // Moss on the tops and ledges in patches with a ragged edge; bare stone down the sides,
    // where it shows its own relief and grain.
    let grows = smoothstep(0.5, 0.7, facing + (patches - 0.5) * 1.4) * moss.colour.a;
    // Lichen on the bare stone, in pale grey-green crusts and specks (D268).
    let crust = noise(world * 7.0 + 23.0) * 0.6 + noise(world * 29.0 + 9.0) * 0.4;
    let lichen = smoothstep(0.6, 0.74, crust) * (1.0 - grows) * 0.6;
    if lichen > 0.0 {
        let base = pbr_input.material.base_color.rgb;
        let pale = vec3<f32>(0.16, 0.17, 0.12) * (0.8 + 0.4 * noise(world * 61.0));
        pbr_input.material.base_color = vec4<f32>(mix(base, pale, lichen), pbr_input.material.base_color.a);
    }
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
        pbr_input.N = normalize(mix(pbr_input.N, pbr_input.world_normal, grows * 0.3));
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
