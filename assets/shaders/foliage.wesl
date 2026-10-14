// Moves textured foliage in the wind: leaf cards, fronds, flowers and the trees they hang
// on, grown plants (crates/terra_world/src/grown) and the lush forest spike's
// (crates/terra_sandbox/src/experiments/lush.rs). The same as
// wind.wesl, except that how much a vertex moves comes from its second texture coordinate,
// so the first is free for the leaf or bark texture: x is how far it bends with the whole
// plant, y how much it flutters on its own.

import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, VertexOutput, decompress_vertex};
@if(PREPASS_PIPELINE)
import bevy_render::globals::Globals;
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::{
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    mesh_view_bindings::globals,
};

@if(PREPASS_PIPELINE)
@group(0) @binding(1) var<uniform> globals: Globals;

// Field order must match FoliageExtension in grown/mod.rs and lush.rs.
struct Wind {
    direction: vec3<f32>,
    strength: f32,
    // 1 for bark.
    bark: f32,
    // For a crown whose cards drop out with distance: from where, to where, and 1 (D223).
    cards: vec3<f32>,
    // A shadow stand-in shrinks to its middles between x and y and grows from them between
    // z and w (D234).
    shrink: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> wind: Wind;

// A crown's cards drop out with distance, as SpeedTree's leaves do (D223): each vertex
// knows its offset from its card's middle, and its card's rank and how many its clump has
// (the tangent's w: the rank, plus a hundredth of the count). From cards.x to cards.y the
// clump keeps fewer, down to two: the next card to go shrinks to its middle while those that
// stay grow to cover as much. The tree's distance from the camera is the same in its shadow's
// pass, which Bevy measures from the camera too.
fn dropped(local: vec3<f32>, tangent: vec4<f32>, tree: vec3<f32>) -> vec3<f32> {
    let rank = floor(tangent.w);
    let count = round(fract(tangent.w) * 100.0);
    let far = distance(tree, view.lod_view_world_position);
    let t = smoothstep(wind.cards.x, wind.cards.y, far);
    let keep = mix(count, min(2.0, count), t);
    let stays = clamp(keep - rank, 0.0, 1.0);
    let grow = pow(count / max(keep, 1.0), 0.55);
    let middle = local - tangent.xyz;
    return middle + tangent.xyz * grow * stays;
}

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn sway(world: vec3<f32>, weights: vec2<f32>, time: f32) -> vec3<f32> {
    let downwind = dot(world, wind.direction);
    let crosswind = dot(world, cross(wind.direction, normalize(world)));
    let gust = sin(time * 0.9 - downwind * 0.07 + crosswind * 0.03) * 0.6
        + sin(time * 1.9 - downwind * 0.23 - crosswind * 0.11) * 0.4;
    let bend = wind.direction * (0.45 + gust) * weights.x * 0.25;
    let flutter = vec3<f32>(
        sin(time * 4.1 + world.x * 2.3 + world.z * 1.1),
        sin(time * 3.3 + world.y * 1.7 + world.x * 0.9),
        sin(time * 3.7 + world.z * 2.1 + world.y * 1.3),
    ) * weights.y * 0.05;
    return (bend + flutter) * wind.strength;
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var local = vertex.position;
@if(VERTEX_TANGENTS) {
    if wind.cards.z > 0.5 {
        local = dropped(local, vertex.tangent, world_from_local[3].xyz);
    }
}
@if(VERTEX_COLORS) {
    // A shadow stand-in shrinks into its middles rather than ending, since Bevy's shadow
    // passes do not dither; the offset from its middle is in its colour, which only the sun
    // sees and the shadow pass is given, unlike tangents (D234).
    if wind.shrink.y > 0.0 || wind.shrink.w > 0.0 {
        let far = distance(world_from_local[3].xyz, view.lod_view_world_position);
        var keep = 1.0;
        if wind.shrink.y > 0.0 {
            keep *= 1.0 - smoothstep(wind.shrink.x, wind.shrink.y, far);
        }
        if wind.shrink.w > 0.0 {
            keep *= smoothstep(wind.shrink.z, wind.shrink.w, far);
        }
        local = local - vertex.color.rgb * (1.0 - keep);
    }
}
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(local, 1.0));
@if(VERTEX_UVS_B)
    world = vec4<f32>(world.xyz + sway(world.xyz, vertex.uv_b, globals.time), 1.0);
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
    // Each tree's bark shifted by where it stands, so no two trunks of a kind show the same
    // bark at the same place (D218).
    if wind.bark > 0.5 {
        let foot = world_from_local[3].xyz;
        out.uv += fract(vec2<f32>(sin(dot(foot, vec3<f32>(12.9898, 78.233, 37.719))), sin(dot(foot, vec3<f32>(39.346, 11.135, 83.155)))) * 43758.547);
    }
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;
    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);

@if(PREPASS_PIPELINE) {
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(MOTION_VECTOR_PREPASS) {
    let previous_from_local = mesh_functions::get_previous_world_from_local(vertex_in.instance_index);
    var previous = mesh_functions::mesh_position_local_to_world(previous_from_local, vec4<f32>(local, 1.0));
@if(VERTEX_UVS_B)
    previous = vec4<f32>(previous.xyz + sway(previous.xyz, vertex.uv_b, globals.time - globals.delta_time), 1.0);
    out.previous_world_position = previous;
}
} @else {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
// The bark's normal map and relief need the surface's tangents.
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
@if(VERTEX_COLORS) {
    // Each plant is a slightly different shade and hue, decided by where it stands, so no
    // two of a variant look alike. The alpha, how much of the sky it sees, is kept.
    let origin = world_from_local[3].xyz;
    let shade = 0.9 + 0.2 * hash(origin * 7.3);
    let warm = (hash(origin * 3.1 + 17.0) - 0.5) * 0.12;
    out.color = vec4<f32>(vertex.color.rgb * shade * vec3<f32>(1.0 + warm, 1.0, 1.0 - warm), vertex.color.a);
}
}

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_in.instance_index, world_from_local[3]);
    return out;
}
