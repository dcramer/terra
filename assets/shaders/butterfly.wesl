// Beats a butterfly's wings (`terra_creatures::butterflies`, docs/ambient-life.md, D655):
// the model comes with its wings spread flat in the level, its body along z and its span
// along x, and each side is turned up and down about the body's axis, so a point moves by
// how far out along the wing it is, with no skeleton. It beats in bouts with a glide
// between, wings held in a shallow V. Used for the main view and for the depth pass, so
// what is drawn and what is hidden move together. Each butterfly's bout and beat start
// from its mesh tag, so a group does not beat as one; the flight on the CPU reads the same
// bout from the same clock to lift the body as the wings beat (butterflies/flight.rs).

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

// Must match `Wings` in butterflies/look.rs.
struct Wings {
    // Beats a second, bouts a second, and the share of a bout that is beating.
    beat: vec4<f32>,
    // The body's axis, which the wings turn about, in the model's own units: where it is
    // across and up, and how far out from it a wing is hinged.
    hinge: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> wings: Wings;

// How far up the wings are held in a glide, the middle of a beat and how far it swings
// either way, in radians: from 28 degrees under the level to 80 over it, nearly closed
// over the back, as a butterfly's stroke is.
const GLIDE: f32 = 0.3;
const MIDDLE: f32 = 0.45;
const SWING: f32 = 0.95;

// How far up the wings are turned at a time.
fn raised(tag: u32, time: f32) -> f32 {
    let through = fract(time * wings.beat.y + f32(tag & 0xffffu) / 65536.0);
    let share = wings.beat.z;
    let beating = smoothstep(0.0, 0.08, through) * (1.0 - smoothstep(share - 0.08, share, through));
    let stroke = fract(time * wings.beat.x + f32(tag >> 16u) / 65536.0) * 6.2831853;
    return mix(GLIDE, MIDDLE + SWING * sin(stroke), beating);
}

// A direction of the model, with the wing of a point so far out from the body's axis
// turned up by so much: nothing at the axis, all of it past the hinge.
fn turned(way: vec3<f32>, out_m: f32, up: f32) -> vec3<f32> {
    let side = select(-1.0, 1.0, out_m >= 0.0);
    let by = up * smoothstep(0.0, max(wings.hinge.z, 1.0e-5), abs(out_m)) * side;
    let c = cos(by);
    let s = sin(by);
    return vec3<f32>(way.x * c - way.y * s, way.x * s + way.y * c, way.z);
}

// A point of the model, its wing turned up by so much about the body's axis.
fn beaten(at: vec3<f32>, up: f32) -> vec3<f32> {
    let axis = vec3<f32>(wings.hinge.x, wings.hinge.y, 0.0);
    return axis + turned(at - axis, at.x - axis.x, up);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    let tag = mesh_functions::get_tag(vertex_in.instance_index);
    let up = raised(tag, globals.time);
    let local = beaten(vertex.position, up);
    let out_m = vertex.position.x - wings.hinge.x;
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
    out.world_normal = mesh_functions::mesh_normal_local_to_world(turned(vertex.normal, out_m, up), vertex_in.instance_index);
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(MOTION_VECTOR_PREPASS) {
    let previous_from_local = mesh_functions::get_previous_world_from_local(vertex_in.instance_index);
    let before = beaten(vertex.position, raised(tag, globals.time - globals.delta_time));
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(previous_from_local, vec4<f32>(before, 1.0));
}
} @else {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(turned(vertex.normal, out_m, up), vertex_in.instance_index);
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
