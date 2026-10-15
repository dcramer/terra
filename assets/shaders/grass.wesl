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
    // The climate's maps round the camera (climate.rs, D327): from a point in the world to
    // its place on them, whether there are any, which painting it is, and grass's layer in
    // the leaves map, or -1.
    climate_from_world: mat4x4<f32>,
    climate_on: u32,
    climate_painting: u32,
    grass_layer: i32,
    // The colour grass browns to, linear, as its year says.
    grass_autumn: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> grass: Grass;

// As foliage.wesl's: the camera's eye and the figure it follows, and the bodies that part
// plants (D271).
struct Parting {
    eye: vec4<f32>,
    subject: vec4<f32>,
    bodies: array<vec4<f32>, 32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var<storage, read> parting: Parting;
// As ground.wesl's: snow's depth, the ground's wetness, rain now and frost; and each kind's
// leaves, grass's how far it has browned in blue and alpha.
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var climate_ground: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var climate_ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(104)
var climate_leaves: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(105)
var climate_leaves_sampler: sampler;

// As ground.wesl's.
const SNOW_FULL_M: f32 = 0.5;
// Rime on grass's blades in a hard frost, linear.
const RIME: vec3<f32> = vec3<f32>(0.5, 0.53, 0.57);

fn climate_place(world: vec3<f32>) -> vec2<f32> {
    return clamp((grass.climate_from_world * vec4<f32>(world, 1.0)).xy, vec2<f32>(0.0), vec2<f32>(1.0));
}

// How deep the snow lies over a blade's root, as ground.wesl lays it, so the blades sink
// into the white the ground is painted: none where its broken edge leaves the ground bare.
fn snow_over(root: vec3<f32>) -> f32 {
    if grass.climate_on == 0u {
        return 0.0;
    }
    let depth = textureSampleLevel(climate_ground, climate_ground_sampler, climate_place(root), 0.0).r * SNOW_FULL_M;
    let edge = noise(root * 0.35 + 91.0) * 0.6 + noise(root * 1.9 + 23.0) * 0.4;
    let cover = smoothstep(0.0, 1.0, (depth - 0.004) / 0.09 + (edge - 0.5) * 0.9);
    return depth * cover * 1.2;
}

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

// As ground.wesl's, so the snow's edge falls in the same places on both.
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

// As foliage.wesl: blades bend away from bodies walking through them, the more the higher
// up the blade (D271), leaning rather than parting (D274).
const BEND_REACH_M: f32 = 0.3;
const BEND: f32 = 0.6;
fn parted(world: vec3<f32>, give: f32, then: u32) -> vec3<f32> {
    var moved = vec3<f32>(0.0);
    let count = min(u32(parting.subject.w), 8u);
    for (var i = 0u; i < count; i++) {
        let feet = parting.bodies[i * 4u + then];
        let head = parting.bodies[i * 4u + then + 1u].xyz;
        let from_feet = world - feet.xyz;
        let reach = feet.w + BEND_REACH_M;
        // Most vertices are nowhere near a body: one test, before the rest.
        let near = reach + distance(head, feet.xyz) + 0.4;
        if give <= 0.0 || dot(from_feet, from_feet) > near * near {
            continue;
        }
        let up = normalize(feet.xyz);
        let rise = dot(from_feet, up);
        let height = dot(head - feet.xyz, up);
        let within = smoothstep(-0.2, 0.1, rise) * (1.0 - smoothstep(height, height + 0.4, rise));
        let across = from_feet - up * rise;
        let apart = length(across);
        if within > 0.0 && apart < reach {
            let push = feet.w * BEND * (1.0 - apart / reach) * within * give;
            moved += across / max(apart, 0.001) * push - up * push * 0.25;
        }
    }
    return moved;
}

// Blades right at a third-person camera's lens shrink into their roots, as leaves there
// fade (foliage_fragment.wesl's `clear_view`, D271): only in the main camera's own view, so
// shadows keep them.
fn cleared(root: vec3<f32>) -> f32 {
    let eye = parting.eye;
    if eye.w < 0.5 || distance(view.world_position, eye.xyz) > 0.01 {
        return 1.0;
    }
    return smoothstep(0.4, 1.2, distance(root, eye.xyz));
}

// Gusts (D255): one wind for the grass, the bushes and the trees, as Ghost of Tsushima has it.
// Its strength rises and falls as fronts roll downwind at about 6 m/s, waves 40 and 90 m long
// (a gust every 7 to 15 s), so they never line up the same way twice, sharpened so calm spells
// are still and gusts come through as fronts; the whole breathes slowly. From under a half to
// one and a half times the old steady wind, about the same on average: at two and a half, as
// it was, a gust swung an oak's crown a metre (D282).
fn gusts(downwind: f32, crosswind: f32, time: f32) -> f32 {
    let front = downwind - time * 6.0;
    let a = sin(front * 0.157 + crosswind * 0.05);
    let b = sin(front * 0.07 - crosswind * 0.083 + 1.7);
    let g = 0.5 + 0.5 * (0.55 * a + 0.45 * b);
    let breath = 0.8 + 0.2 * sin(time * 0.13 + 0.4);
    return (0.45 + 1.1 * g * g * g) * breath;
}

fn sway(world: vec3<f32>, weights: vec2<f32>, time: f32) -> vec3<f32> {
    let direction = grass.wind.xyz;
    let downwind = dot(world, direction);
    let crosswind = dot(world, cross(direction, normalize(world)));
    let gust = sin(time * 0.9 - downwind * 0.07 + crosswind * 0.03) * 0.6
        + sin(time * 1.9 - downwind * 0.23 - crosswind * 0.11) * 0.4;
    let strength = gusts(downwind, crosswind, time);
    let bend = direction * (0.45 + gust) * strength * weights.x * 0.25;
    let flutter = vec3<f32>(
        sin(time * 4.1 + world.x * 2.3 + world.z * 1.1),
        sin(time * 3.3 + world.y * 1.7 + world.x * 0.9),
        sin(time * 3.7 + world.z * 2.1 + world.y * 1.3),
    ) * weights.y * 0.04 * (0.5 + 0.5 * strength);
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
    var keep = (1.0 - smoothstep(grass.fade.x, grass.fade.y, far)) * cleared(root_world);
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
    // Under snow only what stands above it shows (D327).
    let sunk = min(rise * keep, snow_over(root_world));
    return root + (local - root) * keep - vec3<f32>(0.0, sunk, 0.0);
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
    world = vec4<f32>(world.xyz + sway(world.xyz, weights(vertex.uv_b.x), globals.time) + parted(world.xyz, vertex.uv_b.x, 0u), 1.0);
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
    previous = vec4<f32>(previous.xyz + sway(previous.xyz, weights(vertex.uv_b.x), globals.time - globals.delta_time) + parted(previous.xyz, vertex.uv_b.x, 2u), 1.0);
    out.previous_world_position = previous;
}
} @else {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_COLORS) {
    // Alpha carries the blade's rise, not transparency; the blades are solid. Browned to
    // straw in the long cold as grass's year says, keeping each blade's light and dark, and
    // white with rime toward its tip in a hard frost (D327).
    var colour = vertex.color.rgb;
    if grass.climate_on != 0u {
        let at = climate_place(world.xyz);
        if grass.grass_layer >= 0 {
            let year = textureSampleLevel(climate_leaves, climate_leaves_sampler, at, grass.grass_layer, 0.0);
            let browned = mix(year.b, year.a, hash(floor(world.xyz * 2.0)));
            let light = dot(colour, vec3<f32>(0.333)) / 0.06;
            // Its year's colour at a blade's own brightness, darker as straw is than a lawn.
            let straw = grass.grass_autumn.rgb * 0.35;
            colour = mix(colour, straw * clamp(light, 0.5, 1.6), browned);
        }
@if(VERTEX_UVS_B) {
        let frost = textureSampleLevel(climate_ground, climate_ground_sampler, at, 0.0).a;
        colour = mix(colour, RIME, frost * 0.6 * vertex.uv_b.x);
}
    }
    out.color = vec4<f32>(colour, 1.0);
}
}

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_in.instance_index, world_from_local[3]);
    return out;
}
