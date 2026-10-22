// Dust (D305, crates/terra_building/src/loss/dust.rs): a fall's and a break's soft sprites,
// lit as Bevy's standard material is, and faded out where they come near what stands behind
// them in the depth prepass, as shipped games' soft particles are (Unreal's DepthFade, Unity's
// soft particles), so a sprite standing in a wall melts into it rather than cutting it along a
// hard straight line.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    view_transformations::depth_ndc_to_view_z,
};
@if(DEPTH_PREPASS)
import bevy_pbr::prepass::utils::prepass_depth;

// Field order must match DustFade in dust.rs.
struct Dust {
    // How far behind the sprite what is drawn there must stand for it to show whole, metres.
    soft: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> dust: Dust;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var input = pbr_input_from_standard_material(in, is_front);
    var alpha = input.material.base_color.a;
@if(DEPTH_PREPASS)
    {
        let behind = depth_ndc_to_view_z(in.position.z) - depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        alpha *= smoothstep(0.0, dust.soft.x, behind);
    }
    if alpha < 0.004 {
        discard;
    }
    input.material.base_color.a = alpha;
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(input);
    out.color = main_pass_post_lighting_processing(input, out.color);
    out.color.a = alpha;
    return out;
}
