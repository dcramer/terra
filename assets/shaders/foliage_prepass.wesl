// The foliage's prepass fragment shader, for depth, normals, motion vectors and shadows:
// Bevy 0.20's standard one (bevy_pbr src/render/pbr_prepass.wesl, MIT OR Apache-2.0, by
// the Bevy contributors), without the meshlet path, with one change: the crossfade between
// levels of detail dithers in a pattern that moves every frame, as in
// foliage_fragment.wesl, so depth, shadows and colour agree on which pixels are drawn
// (D214).

import bevy_pbr::{
    render::{
        pbr_prepass_functions,
        pbr_bindings,
        pbr_types,
        pbr_functions,
        pbr_functions::SampleBias,
        mesh_bindings::mesh,
        mesh_view_bindings::view,
    },
    prepass::io as prepass_io,
};
import bevy_render::{bindless::{bindless_samplers_filtering, bindless_textures_2d}, globals::Globals};

@if(BINDLESS)
import bevy_pbr::render::pbr_bindings::material_indices;

@group(0) @binding(1) var<uniform> globals: Globals;

fn noise_at(frag_coord: vec4<f32>) -> f32 {
    let shifted = frag_coord.xy + 5.588238 * f32(globals.frame_count % 64u);
    return fract(52.9829189 * fract(dot(shifted, vec2<f32>(0.06711056, 0.00583715))));
}

// As foliage_fragment.wesl: what is right at the camera fades out. A shadow view stands far
// off, so shadows keep it.
fn near_fade(frag_coord: vec4<f32>, world: vec3<f32>) {
    let near = smoothstep(0.3, 0.9, distance(world, view.world_position));
    if near < 1.0 && noise_at(frag_coord) > near {
        discard;
    }
}

// As foliage.wesl's: the camera's eye and the figure it follows (D271).
struct Parting {
    eye: vec4<f32>,
    subject: vec4<f32>,
    bodies: array<vec4<f32>, 32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var<storage, read> parting: Parting;

// As foliage_fragment.wesl: the third-person camera's view kept clear of leaves (D271).
// Only in the main camera's own view, so shadows keep them.
fn clear_view(frag_coord: vec4<f32>, world: vec3<f32>) {
    let eye = parting.eye;
    if eye.w < 0.5 || distance(view.world_position, eye.xyz) > 0.01 {
        return;
    }
    let to = parting.subject.xyz - eye.xyz;
    let far = max(length(to), 0.1);
    let along = to / far;
    let from_eye = world - eye.xyz;
    // Most leaves are beyond both the lens's reach and the tube's: one test, before the rest.
    let seen = dot(from_eye, from_eye);
    if seen > 9.0 && seen > (far + 0.15) * (far + 0.15) + 1.0 {
        return;
    }
    let s = dot(from_eye, along);
    let off = length(from_eye - along * s);
    let lens = min(3.0, far * 0.65);
    var keep = smoothstep(lens * 0.55, lens, length(from_eye));
    let radius = mix(1.0, 0.8, clamp(s / far, 0.0, 1.0));
    let tube = (1.0 - smoothstep(radius * 0.65, radius, off)) * (1.0 - smoothstep(far - 0.1, far + 0.15, s));
    keep = min(keep, 1.0 - tube * step(0.0, s));
    if keep < 1.0 && noise_at(frag_coord) > keep {
        discard;
    }
}

// As foliage_fragment.wesl: a card fades out as it turns edge-on to the eye (D235). Not in
// the shadow pass, which keeps the whole card.
fn edge_fade(frag_coord: vec4<f32>, world: vec3<f32>) {
    let face = normalize(cross(dpdx(world), dpdy(world)));
    let towards = normalize(view.world_position - world);
    let keep = smoothstep(0.04, 0.16, abs(dot(face, towards)));
    if noise_at(frag_coord) > keep {
        discard;
    }
}

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

@if(PREPASS_FRAGMENT)
@fragment
fn fragment(in: prepass_io::VertexOutput, @builtin(front_facing) is_front: bool) -> prepass_io::FragmentOutput {
@if(BINDLESS)
    let slot = mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
@if(BINDLESS)
    let flags = pbr_bindings::material_array[material_indices[slot].material].flags;
@if(BINDLESS)
    let uv_transform = pbr_bindings::material_array[material_indices[slot].material].uv_transform;
@if(!BINDLESS)
    let flags = pbr_bindings::material.flags;
@if(!BINDLESS)
    let uv_transform = pbr_bindings::material.uv_transform;

@if(VISIBILITY_RANGE_DITHER)
    temporal_dither(in.position, in.visibility_range_dither);
    near_fade(in.position, in.world_position.xyz);
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS) == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_MASK {
        edge_fade(in.position, in.world_position.xyz);
        clear_view(in.position, in.world_position.xyz);
    }

    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);

    var out: prepass_io::FragmentOutput;

@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;

@if(NORMAL_PREPASS)
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
        let mesh_flags = mesh[in.instance_index].flags;
        let is_logically_front = pbr_functions::winding_corrected_front_facing(mesh_flags, is_front);
        let world_normal = pbr_functions::prepare_world_normal(in.world_normal, double_sided, is_logically_front);
        var normal = world_normal;

@if(VERTEX_UVS) {
@if(VERTEX_TANGENTS) {
// Not for a modelled trunk, which has no wind weights: its texture coordinates are not its
// bark's, which the main pass lays on from three sides (D264).
@if(STANDARD_MATERIAL_NORMAL_MAP && VERTEX_UVS_B) {
@if(STANDARD_MATERIAL_NORMAL_MAP_UV_B)
        let uv = (uv_transform * vec3(in.uv_b, 1.0)).xy;
@else
        let uv = (uv_transform * vec3(in.uv, 1.0)).xy;
        var bias: SampleBias;
        bias.mip_bias = view.mip_bias;
@if(BINDLESS)
        let texture = bindless_textures_2d[material_indices[slot].normal_map_texture];
@else
        let texture = pbr_bindings::normal_map_texture;
@if(BINDLESS)
        let sampler_ = bindless_samplers_filtering[material_indices[slot].normal_map_sampler];
@else
        let sampler_ = pbr_bindings::normal_map_sampler;
        let Nt = textureSampleBias(texture, sampler_, uv, bias.mip_bias).rgb;
        let TBN = pbr_functions::calculate_tbn_mikktspace(normal, in.world_tangent);
        normal = pbr_functions::apply_normal_mapping(flags, TBN, double_sided, is_logically_front, Nt);
}
}
}

        out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
    } else {
        out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
    }

@if(MOTION_VECTOR_PREPASS)
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);

    return out;
}
@else
@fragment
fn fragment(in: prepass_io::VertexOutput) {
@if(VISIBILITY_RANGE_DITHER)
    temporal_dither(in.position, in.visibility_range_dither);
    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);
}
