// The rim material's prepass (rim.rs): Bevy 0.20's StandardMaterial prepass fragment
// (bevy_pbr render/pbr_prepass.wesl, MIT or Apache-2.0, its non-bindless, non-meshlet path)
// with ground cover's cut before the hero (rim.rs `Cut`, rim.wesl's `cut`) in the camera's
// prepass, never the sun's shadow pass, so a fern still casts its whole shade. Without it a
// fern dropped in the main pass still wrote its depth here (the prepass is on for SSAO's
// normals and TAA's motion vectors), and the hero failed the depth test behind the hole.

import bevy_pbr::{
    prepass::io::{VertexOutput, FragmentOutput},
    render::{
        pbr_bindings,
        pbr_functions,
        pbr_prepass_functions::{prepass_sample_color_and_alpha_discard, calculate_motion_vector},
        pbr_types,
        mesh_bindings::mesh,
        mesh_view_bindings::view,
        utils::interleaved_gradient_noise,
    },
};

@group(constants::MATERIAL_BIND_GROUP) @binding(109)
var<uniform> near: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(110)
var<uniform> hole: vec4<f32>;

fn cut(world: vec3<f32>, frag: vec2<f32>) {
    if near.w <= 0.0 {
        return;
    }
    let d = world - near.xyz;
    let s = length(vec2<f32>(dot(d, view.world_from_view[0].xyz), dot(d, view.world_from_view[1].xyz)));
    let before = dot(d, view.world_from_view[2].xyz);
    let fade = (1.0 - smoothstep(near.w * 0.72, near.w, s)) * smoothstep(0.05, 0.45, before);
    if fade > interleaved_gradient_noise(frag, u32(hole.x)) {
        discard;
    }
}

@if(PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
@if(NORMAL_PREPASS)
    cut(in.world_position.xyz, in.position.xy);
    prepass_sample_color_and_alpha_discard(in);
    var out: FragmentOutput;
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;
@if(NORMAL_PREPASS) {
    let flags = pbr_bindings::material.flags;
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
        let mesh_flags = mesh[in.instance_index].flags;
        let is_logically_front = pbr_functions::winding_corrected_front_facing(mesh_flags, is_front);
        let world_normal = pbr_functions::prepare_world_normal(in.world_normal, double_sided, is_logically_front);
        out.normal = vec4<f32>(world_normal * 0.5 + vec3<f32>(0.5), 1.0);
    } else {
        out.normal = vec4<f32>(in.world_normal * 0.5 + vec3<f32>(0.5), 1.0);
    }
}
@if(MOTION_VECTOR_PREPASS)
    out.motion_vector = calculate_motion_vector(in.world_position, in.previous_world_position);
    return out;
}
// The shadow pass of an alpha-masked material (Bevy runs a material's own prepass fragment
// there without PREPASS_FRAGMENT): its cutout only, as foliage.wesl's.
@else
@fragment
fn fragment(in: VertexOutput) {
    prepass_sample_color_and_alpha_discard(in);
}
