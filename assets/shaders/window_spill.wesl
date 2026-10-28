// A lit window's pool of firelight on the wall round it (crates/terra_folk/src/town/windows.rs):
// drawn as Bevy's StandardMaterial draws an unlit card, its colour the pool's at its brightest
// times the vertex colour's falloff, and flickering with the window's hearth here, from the
// clock and the look's phase, so its material is never written for the flicker.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::main_pass_post_lighting_processing,
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::globals,
};
import super::window_flicker::flicker;

// Field order must match SpillExtension in windows.rs.
struct Spill {
    // x the look's flicker phase.
    phase: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> spill: Spill;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let pbr_input = pbr_input_from_standard_material(in, is_front);
    let base = pbr_input.material.base_color;
    var out: FragmentOutput;
    let lit = vec4<f32>(base.rgb * flicker(globals.time, spill.phase.x), base.a);
    out.color = main_pass_post_lighting_processing(pbr_input, lit);
    return out;
}
