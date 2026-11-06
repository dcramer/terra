// The ruin's wind (ambient/grass.rs): the standard material, its vertices moved in the
// vertex shader as the glade's foliage.wesl moves its blades. A mesh without a second uv
// (the dead grass) bends by its height above the floor, more at the tip, gusts rolling
// across the court; a mesh with one (a banner) hangs from its top edge, uv_b.x how far
// down the cloth, and ripples in waves running along the wind.

import bevy_pbr::render::{
    mesh_functions,
    view_transformations::position_world_to_clip,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, VertexOutput, decompress_vertex};
@else
import bevy_pbr::render::forward_io::{Vertex, VertexOutput, decompress_vertex};

struct Wind {
    // xy: the wind's direction (x, z), z: how far a blade's tip bends (m), w: the time (s).
    wind: vec4<f32>,
    // x: how far the cloth flutters (m), y: the blades' height for full bend (m).
    cloth: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> wind: Wind;

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn gust(world: vec3<f32>, t: f32) -> f32 {
    let along = dot(world.xz, wind.wind.xy);
    return 0.55 + 0.45 * sin(along * 0.35 - t * 1.4) * sin(along * 0.13 - t * 0.55 + 1.7);
}

fn blade(world: vec3<f32>) -> vec3<f32> {
    let t = wind.wind.w;
    let dir = vec3<f32>(wind.wind.x, 0.0, wind.wind.y);
    let up = clamp((world.y + 0.03) / wind.cloth.y, 0.0, 1.0);
    let along = dot(world.xz, wind.wind.xy);
    let wave = 0.6 + 0.4 * sin(t * 2.3 + along * 0.9 + hash3(floor(world * 6.0)) * 6.0);
    let bend = up * up * wind.wind.z * gust(world, t) * wave;
    var d = dir * bend;
    d.y = -0.4 * bend * up;
    return d;
}

fn cloth(world: vec3<f32>, down: f32) -> vec3<f32> {
    let t = wind.wind.w;
    let dir = vec3<f32>(wind.wind.x, 0.0, wind.wind.y);
    let side = vec3<f32>(-wind.wind.y, 0.0, wind.wind.x);
    let along = dot(world.xz, side.xz);
    let g = gust(world, t);
    // Waves run down and across the cloth; the free end moves most.
    let wave = sin(t * 3.2 - down * 5.0 + along * 4.0) * 0.6 + sin(t * 5.1 - down * 9.0 + along * 7.0) * 0.25;
    let f = wind.cloth.x * down * (0.4 + 0.6 * g);
    return dir * f * (0.6 + wave) + side * f * 0.25 * wave + vec3<f32>(0.0, abs(wave) * f * 0.15, 0.0);
}

fn moved(world: vec3<f32>, uv_b: vec2<f32>, has_b: bool) -> vec3<f32> {
    if has_b {
        return world + cloth(world, uv_b.x);
    }
    return world + blade(world);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    var uv_b = vec2<f32>(0.0);
    var has_b = false;
@if(VERTEX_UVS_B) {
    uv_b = vertex.uv_b;
    has_b = true;
}
    world = vec4<f32>(moved(world.xyz, uv_b, has_b), 1.0);
    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);
@if(PREPASS_PIPELINE && UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;
@if(VERTEX_NORMALS && (!PREPASS_PIPELINE || NORMAL_PREPASS_OR_DEFERRED_PREPASS))
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_TANGENTS && (!PREPASS_PIPELINE || NORMAL_PREPASS_OR_DEFERRED_PREPASS))
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(PREPASS_PIPELINE && MOTION_VECTOR_PREPASS)
    out.previous_world_position = world;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
    return out;
}
