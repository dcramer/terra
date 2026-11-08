// The spike's foliage (foliage.rs): the standard material, its vertices swayed by the wind
// (blades bend from the root, leaf cards flutter, the crown rocks slowly) and, for the wood
// before the party, a dithered hole cut about each of them, as modern isometric games do
// (Pillars of Eternity II, Pathfinder: Kingmaker). The same file serves the main pass and
// the prepass; the hole is cut in the camera's prepass (which has normals, for SSAO) and
// main pass alike, never in the sun's shadow pass, so a crown still casts its whole shade.

import bevy_pbr::render::{
    mesh_functions,
    utils::interleaved_gradient_noise,
    view_transformations::position_world_to_clip,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::{
    prepass::io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    render::pbr_prepass_functions::{prepass_sample_color_and_alpha_discard, calculate_motion_vector},
};
@else
import bevy_pbr::render::{
    forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
};

struct Foliage {
    // xz: the wind's direction, z: how far a blade's tip bends (m), w: the time (s).
    wind: vec4<f32>,
    // x: how far a leaf card flutters (m), y: 1 to cut the hole, z: its radius (m), w: the
    // frame (0..64), turning the hole's dither.
    leaf: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    forward: vec4<f32>,
    // The camp fire and the party's middles; w is 1 for one standing there.
    party: array<vec4<f32>, 6>,
    // x: how far each texel of the card goes to its own grey before the vertex colour tints
    // it (a plant set's `grey`, 0 leaves the card as it is).
    tone: vec4<f32>,
    // xyz: the camera; w: play's near band (m), 0 off (foliage.rs `Near`): over 0, play.
    eye: vec4<f32>,
    // Ground cover's hole (leaf.y 2, foliage.rs `cover`): xyz the hero's middle, w its
    // radius (m), set in play alone; 0 cuts nothing.
    cover: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> foliage: Foliage;

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

/// Where the wind moves a vertex, from its weight (`uv_b`: x how far up the blade or card,
/// y 1 for a leaf card) and the mesh's root.
fn sway(world: vec3<f32>, root: vec3<f32>, w: vec2<f32>) -> vec3<f32> {
    let t = foliage.wind.w;
    let dir = vec3<f32>(foliage.wind.x, 0.0, foliage.wind.y);
    // Gusts roll across the land along the wind.
    let along = dot(world.xz, foliage.wind.xy);
    let gust = 0.55 + 0.45 * sin(along * 0.18 - t * 1.3) * sin(along * 0.07 - t * 0.5 + 1.7);
    var d = vec3<f32>(0.0);
    if w.y < 0.5 {
        // A blade or a stem: it bends from the root, more at the tip, and springs back.
        let bend = w.x * w.x * foliage.wind.z * gust;
        let wave = 0.6 + 0.4 * sin(t * 2.3 + along * 0.9 + hash3(floor(world * 4.0)) * 6.0);
        d = dir * bend * wave;
        d.y = -0.35 * bend * wave * w.x;
    } else {
        // A leaf card: each flutters on its own; the whole crown rocks on its trunk.
        let h = hash3(floor(world * 2.0));
        let f = foliage.leaf.x * (0.4 + 0.6 * gust);
        d = vec3<f32>(
            sin(t * 3.1 + h * 6.28),
            sin(t * 4.3 + h * 11.0) * 0.6,
            cos(t * 2.7 + h * 9.0),
        ) * f;
        let rise = max(world.y - root.y, 0.0);
        d += dir * rise * 0.012 * gust * (0.7 + 0.3 * sin(t * 0.8 + root.x * 0.3 + root.z * 0.2));
    }
    return d;
}

/// Cuts the hole: a fragment before one of the party, near them on the screen, is dropped
/// through a dither that thins to nothing at the hole's middle. The dither is interleaved
/// gradient noise (Jimenez 2014, Bevy's `utils::interleaved_gradient_noise`) turned each
/// frame (`leaf.w`), as Unreal's DitherTemporalAA: TAA averages it to a smooth fade, where a
/// fixed 4 by 4 Bayer matrix showed as a checker at 1:1 (the forest's roots, iso-art j39).
/// In play a tree near the lens fades whole (foliage.rs `Tree`): its share gone rides in the
/// mesh's tag, 0 to `TAG`, so at rest it is drawn whole or not at all, never a screen door.
const TAG: f32 = 1000.0;

fn cut(world: vec3<f32>, frag: vec2<f32>, instance: u32) {
    if foliage.leaf.y < 0.5 {
        return;
    }
    // Ground cover (foliage.rs `Hole::Cover`): its own smaller hole about the hero alone,
    // opening as play's crowns do from a hand's breadth before his middle, as rim.wesl's
    // `cut` opens the cover's models; nothing in a still (`cover.w` 0).
    if foliage.leaf.y > 1.5 {
        let c = foliage.cover;
        if c.w <= 0.0 {
            return;
        }
        let d = world - c.xyz;
        let s = length(vec2<f32>(dot(d, foliage.right.xyz), dot(d, foliage.up.xyz)));
        let before = -dot(d, foliage.forward.xyz);
        let f = (1.0 - smoothstep(c.w * 0.72, c.w, s)) * smoothstep(0.05, 0.45, before);
        if f > interleaved_gradient_noise(frag, u32(foliage.leaf.w)) {
            discard;
        }
        return;
    }
    let r = foliage.leaf.z;
    // Play (foliage.rs `fade`, its twin, and `PLAY_BEFORE`): open from a hand's breadth
    // before the hero.
    let play = foliage.eye.w > 0.0;
    let lo = select(0.4, 0.05, play);
    let hi = select(1.4, 0.45, play);
    var fade = min(f32(mesh_functions::get_tag(instance)) / TAG, 1.0);
    for (var i = 0u; i < 6u; i++) {
        let m = foliage.party[i];
        if m.w < 0.5 {
            continue;
        }
        let d = world - m.xyz;
        let s = length(vec2<f32>(dot(d, foliage.right.xyz), dot(d, foliage.up.xyz)));
        let before = -dot(d, foliage.forward.xyz);
        let f = (1.0 - smoothstep(r * 0.72, r, s)) * smoothstep(lo, hi, before);
        fade = max(fade, f);
    }
    if fade > interleaved_gradient_noise(frag, u32(foliage.leaf.w)) {
        discard;
    }
}

@if(PREPASS_PIPELINE)
@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
@if(VERTEX_UVS_B)
    world = vec4<f32>(world.xyz + sway(world.xyz, world_from_local[3].xyz, vertex.uv_b), 1.0);
    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(MOTION_VECTOR_PREPASS)
    out.previous_world_position = world;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
    return out;
}
@else
@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
@if(VERTEX_UVS_B)
    world = vec4<f32>(world.xyz + sway(world.xyz, world_from_local[3].xyz, vertex.uv_b), 1.0);
    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
    return out;
}

@if(PREPASS_PIPELINE && PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
@if(NORMAL_PREPASS)
    cut(in.world_position.xyz, in.position.xy, in.instance_index);
    prepass_sample_color_and_alpha_discard(in);
    var out: FragmentOutput;
@if(NORMAL_PREPASS) {
    var n = normalize(in.world_normal);
    if !is_front {
        n = -n;
    }
    out.normal = vec4<f32>(n * 0.5 + vec3<f32>(0.5), 1.0);
}
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;
    // The camera's own motion, for the TAA (the sway's is left out).
@if(MOTION_VECTOR_PREPASS)
    out.motion_vector = calculate_motion_vector(in.world_position, in.previous_world_position);
    return out;
}
@elif(PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOutput) {
    prepass_sample_color_and_alpha_discard(in);
}
@else
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    cut(in.world_position.xyz, in.position.xy, in.instance_index);
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    // The card's texel taken toward its own grey, then tinted again: the base colour is the
    // texel times the vertex colour, so the texel is that over the colour. A multiplied
    // tint alone cannot grey a card whose blades are green and tips tan.
@if(VERTEX_COLORS) {
    if foliage.tone.x > 0.0 {
        let tint = max(in.color.rgb, vec3<f32>(1e-5));
        let texel = pbr_input.material.base_color.rgb / tint;
        let grey = vec3<f32>(dot(texel, vec3<f32>(0.2126, 0.7152, 0.0722)));
        let rgb = mix(texel, grey, foliage.tone.x) * tint;
        pbr_input.material.base_color = vec4<f32>(rgb, pbr_input.material.base_color.a);
    }
}
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
