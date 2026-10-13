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
    mesh_view_bindings::{globals, view},
};

// Sky light under a canopy is dimmed by the leaves above and warmed by what they and the
// ground bounce: what a surface that sees none of the sky is lit by instead, as a share of
// the sky's light (D216). Crysis bakes the same into its trees' vertices.
const BOUNCE: vec3<f32> = vec3<f32>(0.55, 0.6, 0.4);

fn noise_at(frag_coord: vec4<f32>) -> f32 {
    let shifted = frag_coord.xy + 5.588238 * f32(globals.frame_count % 64u);
    return fract(52.9829189 * fract(dot(shifted, vec2<f32>(0.06711056, 0.00583715))));
}

// Leaves and twigs right at the camera fade out, a pixel at a time and a different pixel
// each frame, so TAA blends them away rather than the camera pushing through them (D216).
fn near_fade(frag_coord: vec4<f32>, world: vec3<f32>) {
    let near = smoothstep(0.5, 1.6, distance(world, view.world_position));
    if near < 1.0 && noise_at(frag_coord) > near {
        discard;
    }
}

// Discards this fragment if it is outside its crossfade's share at this pixel this frame.
// `dither` is Bevy's level: -16 to 0 fading in, 0 to 16 fading out.
fn temporal_dither(frag_coord: vec4<f32>, dither: i32) {
    if dither == 0 {
        return;
    }
    if dither <= -16 || dither >= 16 {
        discard;
    }
    let threshold = i32(noise_at(frag_coord) * 16.0);
    if (dither >= 0 && dither + threshold >= 16) || (dither < 0 && 1 + dither + threshold <= 0) {
        discard;
    }
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
@if(VISIBILITY_RANGE_DITHER)
    temporal_dither(in.position, in.visibility_range_dither);

    near_fade(in.position, in.world_position.xyz);

    var pbr_input = pbr_input_from_standard_material(in, is_front);
@if(VERTEX_COLORS) {
    // Alpha is how much of the sky this point sees.
    let sky = in.color.a;
    pbr_input.diffuse_occlusion *= mix(BOUNCE, vec3<f32>(1.0), sky);
    pbr_input.specular_occlusion *= mix(0.35, 1.0, sky);
}
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
