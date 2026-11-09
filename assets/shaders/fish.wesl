// Swims a fish (`terra_creatures::fish`, docs/ambient-life.md item 3): a wave runs down
// its body from head to tail, widest at the tail, as shipped games move their schools,
// with no skeleton. Used for the main view and for the depth pass, so what is drawn and
// what is hidden move together. Each fish's stroke starts from its mesh tag, so a school
// does not beat as one.

import bevy_pbr::render::{
    mesh_functions,
    view_transformations::position_world_to_clip,
};

// The clock comes from the renderer, which keeps it in a different place for the depth
// and shadow passes than for the main one.
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

// Must match `Swim` in fish/look.rs.
struct Swim {
    // Where its nose is along the model's z, how long it is, how many strokes a second,
    // and how far its tail swings, as a share of its length.
    body: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> swim: Swim;

// A point of the model, swung: nothing at the nose but a little yaw, most at the tail.
fn swum(at: vec3<f32>, tag: u32, time: f32) -> vec3<f32> {
    let along = clamp((swim.body.x - at.z) / max(swim.body.y, 1.0e-4), 0.0, 1.0);
    let stroke = time * swim.body.z * 6.2831853 - along * 4.0 + f32(tag) * 0.001;
    let swing = sin(stroke) * swim.body.w * swim.body.y * (0.12 + 0.88 * along * along);
    return at + vec3<f32>(swing, 0.0, 0.0);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    let tag = mesh_functions::get_tag(vertex_in.instance_index);
    let local = swum(vertex.position, tag, globals.time);
    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(local, 1.0));
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
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(MOTION_VECTOR_PREPASS) {
    let previous_from_local = mesh_functions::get_previous_world_from_local(vertex_in.instance_index);
    let before = swum(vertex.position, tag, globals.time - globals.delta_time);
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(previous_from_local, vec4<f32>(before, 1.0));
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
