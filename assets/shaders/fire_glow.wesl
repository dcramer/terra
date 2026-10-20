// A burning prop's glowing part (crates/terra_world/src/fire/glow.rs): drawn as Bevy's
// StandardMaterial draws it, the share of its light its glow gives added to the flames' mask
// in the alpha (crates/terra_world/src/fire/film.rs), so the tonemapper takes that share
// through the film, ungraded, as it takes a flame.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
};

// Field order must match Glow in glow.rs.
struct Glow {
    // x how much of the glow's share goes to the mask.
    mask: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> glow: Glow;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);
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
