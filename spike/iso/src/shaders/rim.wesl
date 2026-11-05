// The spike's rim (rim.rs): the standard material, with a fresnel term of the hour's sky
// added to its emission, so a figure's edge catches the light.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> rim: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);
    let facing = clamp(dot(normalize(pbr_input.N), normalize(pbr_input.V)), 0.0, 1.0);
    // Brighter where the edge faces up, as a sky's light would.
    let up = 0.55 + 0.45 * clamp(pbr_input.N.y, 0.0, 1.0);
    let edge = pow(1.0 - facing, rim.w) * up;
    pbr_input.material.emissive = vec4<f32>(pbr_input.material.emissive.rgb + rim.rgb * edge, pbr_input.material.emissive.a);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
