// Moves plants in the wind. Used for the main view, and for the depth and shadow passes,
// so that what is drawn, what is shaded and what casts a shadow all move together.
//
// Each vertex says how much it moves in its first texture coordinate: x is how far it
// bends with the whole plant, y is how much it flutters on its own. Roots and trunks at
// ground level carry zero and stay put.

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

// Field order must match WindExtension in flora.rs.
struct Wind {
    // Level with the ground at the scene, pointing downwind.
    direction: vec3<f32>,
    strength: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> wind: Wind;

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn sway(world: vec3<f32>, weights: vec2<f32>) -> vec3<f32> {
    let downwind = dot(world, wind.direction);
    let crosswind = dot(world, cross(wind.direction, normalize(world)));
    // Gusts roll downwind as broad waves, with a quicker ripple on top.
    let gust = sin(globals.time * 1.1 - downwind * 0.09 + crosswind * 0.03) * 0.6
        + sin(globals.time * 2.3 - downwind * 0.31 - crosswind * 0.11) * 0.4;
    let bend = wind.direction * (0.45 + gust) * weights.x * 0.13;
    let flutter = vec3<f32>(
        sin(globals.time * 4.1 + world.x * 2.3 + world.z * 1.1),
        sin(globals.time * 3.3 + world.y * 1.7 + world.x * 0.9),
        sin(globals.time * 3.7 + world.z * 2.1 + world.y * 1.3),
    ) * weights.y * 0.035;
    return (bend + flutter) * wind.strength;
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    // Where the point is on the plant before the wind moves it, from the plant's foot,
    // which is what its material is projected from (flora.wesl), so bark and leaves stay
    // put on a swaying tree. Carried in the texture coordinates, which the fragment
    // shader reads as this and not as coordinates.
    let on_plant = world.xyz - world_from_local[3].xyz;
@if(VERTEX_UVS_A) {
    world = vec4<f32>(world.xyz + sway(world.xyz, vertex.uv), 1.0);
    out.uv = on_plant.xy;
}
@if(VERTEX_UVS_B)
    out.uv_b = vec2<f32>(on_plant.z, 0.0);
    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);

@if(PREPASS_PIPELINE) {
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(MOTION_VECTOR_PREPASS) {
    let previous_from_local = mesh_functions::get_previous_world_from_local(vertex_in.instance_index);
    var previous = mesh_functions::mesh_position_local_to_world(previous_from_local, vec4<f32>(vertex.position, 1.0));
@if(VERTEX_UVS_A)
    previous = vec4<f32>(previous.xyz + sway(previous.xyz, vertex.uv), 1.0);
    out.previous_world_position = previous;
}
} @else {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_COLORS) {
    // Each plant is a slightly different shade, decided by where it stands. Grass is
    // drawn many tufts to a mesh and is shaded tuft by tuft when the mesh is made.
    let origin = world_from_local[3].xyz;
    var shade = 1.0;
    if dot(origin, origin) > 1.0 {
        shade = 0.86 + 0.28 * hash(origin * 7.3);
    }
    // The alpha says which material the part is drawn in (flora.wesl).
    out.color = vec4<f32>(vertex.color.rgb * shade, vertex.color.a);
}
}

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_in.instance_index, world_from_local[3]);
    return out;
}
