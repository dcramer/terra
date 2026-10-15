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
    // The climate's ground map round the camera, as grass.wesl reads it (D327).
    climate_from_world: mat4x4<f32>,
    climate_on: u32,
    climate_painting: u32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> litter: Litter;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var climate_ground: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var climate_ground_sampler: sampler;

// As ground.wesl's.
const SNOW_FULL_M: f32 = 0.5;

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

// As ground.wesl's, so the snow's edge falls in the same places.
fn noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash(cell), hash(cell + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash(cell + vec3<f32>(0.0, 1.0, 0.0)), hash(cell + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash(cell + vec3<f32>(0.0, 0.0, 1.0)), hash(cell + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash(cell + vec3<f32>(0.0, 1.0, 1.0)), hash(cell + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// Whether snow covers the floor where a piece lies, as ground.wesl lays it: buried pieces
// sink out of sight under it (D327).
fn under_snow(world: vec3<f32>) -> f32 {
    if litter.climate_on == 0u {
        return 0.0;
    }
    let at = clamp((litter.climate_from_world * vec4<f32>(world, 1.0)).xy, vec2<f32>(0.0), vec2<f32>(1.0));
    let depth = textureSampleLevel(climate_ground, climate_ground_sampler, at, 0.0).r * SNOW_FULL_M;
    let edge = noise(world * 0.35 + 91.0) * 0.6 + noise(world * 1.9 + 23.0) * 0.4;
    return smoothstep(0.3, 0.6, (depth - 0.004) / 0.09 + (edge - 0.5) * 0.9);
}

// A point sunk under the ground as the camera draws away from it, by its piece's rank. The
// planet's centre is the world's origin, so up is away from it.
fn sunk(world: vec3<f32>, rank: f32) -> vec3<f32> {
    let far = distance(world, view.world_position);
    let gone = mix(litter.fade.y, litter.fade.x + 1.5, rank);
    let sink = max(smoothstep(gone - 1.5, gone, far), under_snow(world));
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
