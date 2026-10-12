// Moves plants in the wind. Used for the main view, and for the depth and shadow passes,
// so that what is drawn, what is shaded and what casts a shadow all move together.
//
// Each vertex says how much it moves in its first texture coordinate: x is how far it
// bends with the whole plant, y is how much it flutters on its own. Roots and trunks at
// ground level carry zero and stay put.

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
}

// The clock comes from the renderer, which keeps it in a different place for the depth
// and shadow passes than for the main one.
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::{Vertex, VertexOutput}
#import bevy_render::globals::Globals
@group(0) @binding(1) var<uniform> globals: Globals;
#else
#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput},
    mesh_view_bindings::globals,
}
#endif

// Field order must match WindExtension in flora.rs.
struct Wind {
    // Level with the ground at the scene, pointing downwind.
    direction: vec3<f32>,
    strength: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
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
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
#ifdef VERTEX_UVS_A
    world = vec4<f32>(world.xyz + sway(world.xyz, vertex.uv), 1.0);
    out.uv = vertex.uv;
#endif
    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);

#ifdef PREPASS_PIPELINE

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif
#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef MOTION_VECTOR_PREPASS
    let previous_from_local = mesh_functions::get_previous_world_from_local(vertex.instance_index);
    var previous = mesh_functions::mesh_position_local_to_world(previous_from_local, vec4<f32>(vertex.position, 1.0));
#ifdef VERTEX_UVS_A
    previous = vec4<f32>(previous.xyz + sway(previous.xyz, vertex.uv), 1.0);
#endif
    out.previous_world_position = previous;
#endif

#else // PREPASS_PIPELINE

#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#ifdef VERTEX_COLORS
    // Each plant is a slightly different shade, decided by where it stands. Grass is
    // drawn many tufts to a mesh and is shaded tuft by tuft when the mesh is made.
    let origin = world_from_local[3].xyz;
    var shade = 1.0;
    if dot(origin, origin) > 1.0 {
        shade = 0.86 + 0.28 * hash(origin * 7.3);
    }
    out.color = vec4<f32>(vertex.color.rgb * shade, 1.0);
#endif

#endif // PREPASS_PIPELINE

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]);
#endif
    return out;
}
