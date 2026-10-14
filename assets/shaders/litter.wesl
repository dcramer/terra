// Litter on the forest floor (crates/terra_world/src/cover/litter.rs): fallen leaves,
// needles, twigs and cones, cards lying on the ground near the camera, sunk into it with
// distance one piece at a time, so the litter thins out rather than ending at an edge. Each
// vertex's second texture coordinate holds its piece's rank: the highest ranks sink first.
// Otherwise drawn as Bevy's own meshes are (bevy_pbr's mesh.wesl and prepass.wesl).

import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, VertexOutput, decompress_vertex};
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::forward_io::{Vertex, VertexOutput, decompress_vertex};

// Field order must match LitterExtension in cover/litter.rs.
struct Litter {
    // From how far pieces start to sink, and where the last is gone.
    fade: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> litter: Litter;

// A point sunk under the ground as the camera draws away from it, by its piece's rank. The
// planet's centre is the world's origin, so up is away from it.
fn sunk(world: vec3<f32>, rank: f32) -> vec3<f32> {
    let far = distance(world, view.world_position);
    let gone = mix(litter.fade.y, litter.fade.x + 1.5, rank);
    let sink = smoothstep(gone - 1.5, gone, far);
    return world - normalize(world) * sink * 0.1;
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
@if(VERTEX_UVS_B)
    world = vec4<f32>(sunk(world.xyz, vertex.uv_b.x), 1.0);
    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

@if(PREPASS_PIPELINE) {
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
}
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(MOTION_VECTOR_PREPASS) {
    // The litter does not move; where it is sinking, last frame's place is close enough.
    out.previous_world_position = world;
}
} @else {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
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
