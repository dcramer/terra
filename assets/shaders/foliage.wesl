// Moves textured foliage in the wind: leaf cards, fern fronds and the trees they hang on,
// for the lush-forest spike (crates/terra_sandbox/src/experiments/lush.rs). The same as
// wind.wesl, except that how much a vertex moves comes from its second texture coordinate,
// so the first is free for the leaf or bark texture: x is how far it bends with the whole
// plant, y how much it flutters on its own.

import bevy_pbr::render::{
    mesh_functions,
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

// Field order must match FoliageExtension in lush.rs.
struct Wind {
    direction: vec3<f32>,
    strength: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> wind: Wind;

fn sway(world: vec3<f32>, weights: vec2<f32>) -> vec3<f32> {
    let downwind = dot(world, wind.direction);
    let crosswind = dot(world, cross(wind.direction, normalize(world)));
    let gust = sin(globals.time * 0.9 - downwind * 0.07 + crosswind * 0.03) * 0.6
        + sin(globals.time * 1.9 - downwind * 0.23 - crosswind * 0.11) * 0.4;
    let bend = wind.direction * (0.45 + gust) * weights.x * 0.25;
    let flutter = vec3<f32>(
        sin(globals.time * 4.1 + world.x * 2.3 + world.z * 1.1),
        sin(globals.time * 3.3 + world.y * 1.7 + world.x * 0.9),
        sin(globals.time * 3.7 + world.z * 2.1 + world.y * 1.3),
    ) * weights.y * 0.05;
    return (bend + flutter) * wind.strength;
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
@if(VERTEX_UVS_B)
    world = vec4<f32>(world.xyz + sway(world.xyz, vertex.uv_b), 1.0);
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
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
@if(MOTION_VECTOR_PREPASS) {
    let previous_from_local = mesh_functions::get_previous_world_from_local(vertex_in.instance_index);
    var previous = mesh_functions::mesh_position_local_to_world(previous_from_local, vec4<f32>(vertex.position, 1.0));
@if(VERTEX_UVS_B)
    previous = vec4<f32>(previous.xyz + sway(previous.xyz, vertex.uv_b), 1.0);
    out.previous_world_position = previous;
}
} @else {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_COLORS)
    out.color = vertex.color;
}

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_in.instance_index, world_from_local[3]);
    return out;
}
