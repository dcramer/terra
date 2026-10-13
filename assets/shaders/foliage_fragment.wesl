// The foliage's main-pass fragment shader: Bevy 0.20's standard one (bevy_pbr
// src/render/pbr.wesl, MIT OR Apache-2.0, by the Bevy contributors), cut to what foliage
// uses, with one change: the crossfade between levels of detail dithers in a pattern that
// moves every frame, so temporal anti-aliasing blends it into a fade (D214). Bevy's own
// pattern is the same 4x4 grid every frame, which TAA cannot resolve and shows as a
// crawling screen door. The pattern is Jimenez's interleaved gradient noise (Call of Duty:
// Advanced Warfare), shifted each frame, as Unreal dithers its foliage's crossfades.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
    mesh_view_bindings::globals,
};

// Discards this fragment if it is outside its crossfade's share at this pixel this frame.
// `dither` is Bevy's level: -16 to 0 fading in, 0 to 16 fading out.
fn temporal_dither(frag_coord: vec4<f32>, dither: i32) {
    if dither == 0 {
        return;
    }
    if dither <= -16 || dither >= 16 {
        discard;
    }
    let shifted = frag_coord.xy + 5.588238 * f32(globals.frame_count % 64u);
    let noise = fract(52.9829189 * fract(dot(shifted, vec2<f32>(0.06711056, 0.00583715))));
    let threshold = i32(noise * 16.0);
    if (dither >= 0 && dither + threshold >= 16) || (dither < 0 && 1 + dither + threshold <= 0) {
        discard;
    }
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
@if(VISIBILITY_RANGE_DITHER)
    temporal_dither(in.position, in.visibility_range_dither);

    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );

    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
