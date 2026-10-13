// Grass blades (crates/terra_world/src/cover): moved in the wind as foliage.wesl moves
// leaves, and shrunk into their roots with distance from the camera, so the grass fades into
// the ground rather than ending at an edge. Each vertex carries the way back to its root:
// across in its first texture coordinate (east and south, in the square's frame), and up in
// its colour's alpha. Its second texture coordinate is how far up its blade it is, which is
// how much it moves in the wind, and its blade's rank among its neighbours, which says how
// far off it thins out: the grass thins one blade at a time, as Ghost of Tsushima's does, so
// that a blade a farther ring of detail does not have is gone before its square changes.

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

// Field order must match GrassExtension in cover/mod.rs.
struct Grass {
    // The way the wind blows, and how hard.
    wind: vec4<f32>,
    // From how far the blades start to shrink, and where they are gone.
    fade: vec4<f32>,
    // The share of blades the second ring keeps and where the rest are gone; the same for
    // the third.
    thinning: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> grass: Grass;

fn sway(world: vec3<f32>, weights: vec2<f32>) -> vec3<f32> {
    let direction = grass.wind.xyz;
    let downwind = dot(world, direction);
    let crosswind = dot(world, cross(direction, normalize(world)));
    let gust = sin(globals.time * 0.9 - downwind * 0.07 + crosswind * 0.03) * 0.6
        + sin(globals.time * 1.9 - downwind * 0.23 - crosswind * 0.11) * 0.4;
    let bend = direction * (0.45 + gust) * weights.x * 0.25;
    let flutter = vec3<f32>(
        sin(globals.time * 4.1 + world.x * 2.3 + world.z * 1.1),
        sin(globals.time * 3.3 + world.y * 1.7 + world.x * 0.9),
        sin(globals.time * 3.7 + world.z * 2.1 + world.y * 1.3),
    ) * weights.y * 0.04;
    return (bend + flutter) * grass.wind.w;
}

// How much a vertex this far up its blade moves: with the blade, and on its own.
fn weights(up: f32) -> vec2<f32> {
    return vec2<f32>(up * up * 0.5, up * 0.6);
}

// The vertex where it stands now: drawn toward its root as the camera draws away.
fn shrunk(local: vec3<f32>, to_root: vec2<f32>, rise: f32, rank: f32, world_from_local: mat4x4<f32>) -> vec3<f32> {
    let root = local + vec3<f32>(to_root.x, -rise, to_root.y);
    let root_world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(root, 1.0)).xyz;
    let far = distance(root_world, view.world_position);
    var keep = 1.0 - smoothstep(grass.fade.x, grass.fade.y, far);
    // Where this blade thins out: those only the near ring has are gone by its end, the
    // highest ranks first; those the second ring has but not the third, by the second's.
    let t = grass.thinning;
    var gone = 1.0e6;
    if rank > t.x {
        gone = mix(t.y, t.y - 4.0, (rank - t.x) / (1.0 - t.x));
    } else if rank > t.z {
        gone = mix(t.w, t.y, (rank - t.z) / (t.x - t.z));
    }
    keep *= 1.0 - smoothstep(gone - 2.0, gone, far);
    return root + (local - root) * keep;
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var local = vertex.position;
@if(VERTEX_UVS_A && VERTEX_UVS_B && VERTEX_COLORS)
    local = shrunk(local, vertex.uv, vertex.color.a, vertex.uv_b.y, world_from_local);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(local, 1.0));
@if(VERTEX_UVS_B)
    world = vec4<f32>(world.xyz + sway(world.xyz, weights(vertex.uv_b.x)), 1.0);
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
    var previous = mesh_functions::mesh_position_local_to_world(previous_from_local, vec4<f32>(local, 1.0));
@if(VERTEX_UVS_B)
    previous = vec4<f32>(previous.xyz + sway(previous.xyz, weights(vertex.uv_b.x)), 1.0);
    out.previous_world_position = previous;
}
} @else {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_COLORS)
    // Alpha carries the blade's rise, not transparency; the blades are solid.
    out.color = vec4<f32>(vertex.color.rgb, 1.0);
}

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_in.instance_index, world_from_local[3]);
    return out;
}
