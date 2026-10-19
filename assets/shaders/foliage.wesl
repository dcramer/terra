// Moves textured foliage in the wind: leaf cards, fronds, flowers and the trees they hang
// on, grown plants (crates/terra_world/src/grown) and the lush forest spike's
// (crates/terra_sandbox/src/experiments/lush.rs). The same as
// wind.wesl, except that how much a vertex moves comes from its second texture coordinate,
// so the first is free for the leaf or bark texture: x is how far it bends with the whole
// plant, y how much it flutters on its own.

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

// Field order must match FoliageExtension in grown/mod.rs and lush.rs.
// What the climate does to foliage (grown/mod.rs's FoliageClimate, D327): whether there are
// maps, how wide the card atlas's cells are, which kind's layer of the leaves map each cell's
// card is (-1 none), and each layer's autumn colour, linear. Where the maps lie is in the
// shared buffer (`parting.climate_from_world`).
struct FoliageClimate {
    on: u32,
    across: f32,
    layers: array<vec4<i32>, 4>,
    autumn: array<vec4<f32>, 12>,
}

struct Wind {
    direction: vec3<f32>,
    strength: f32,
    // 1 for bark.
    bark: f32,
    // For a crown whose cards drop out with distance: from where, to where, and 1 (D223).
    cards: vec3<f32>,
    // A shadow stand-in shrinks to its middles between x and y and grows from them between
    // z and w (D234).
    shrink: vec4<f32>,
    // For leaves: how much their colour is greyed and darkened, how brightly they glow lit
    // through, and how dark the inside of a crown goes at least (D247).
    leaf: vec4<f32>,
    // For leaves: how far the atlas's normal map turns each leaf from the crown's normal
    // (D269); 0 for none.
    relief: f32,
    // For a crown's solid shadow stand-in: how wide the gaps in its shade are, in metres
    // (`dapple` in foliage_prepass.wesl, D359); 0 for none.
    dapple: f32,
    climate: FoliageClimate,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> wind: Wind;

// What plants make way for (D271), as terra_world's parting.rs writes it: the main camera's
// eye (w 1 if it follows a subject), the subject (w how many bodies), and each body's feet
// (w its radius) and head, now, then a frame ago.
struct Parting {
    eye: vec4<f32>,
    subject: vec4<f32>,
    bodies: array<vec4<f32>, 32>,
    // The world's wind (wind.rs, D255): which way it blows, and in w how hard, or -1 until
    // there is one.
    wind: vec4<f32>,
    // From a point in the world to its place on the climate's maps (climate.rs, D327).
    climate_from_world: mat4x4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var<storage, read> parting: Parting;
// Each kind's leaves round the camera (climate.rs): how many a plant has early and late in its
// kind's year in red and green, how far turned in blue and alpha.
@group(constants::MATERIAL_BIND_GROUP) @binding(106)
var climate_leaves: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(107)
var climate_leaves_sampler: sampler;

// Which kind's layer of the leaves map a card is, by the atlas cell its picture is in.
fn card_layer(uv: vec2<f32>) -> i32 {
    if wind.climate.on == 0u {
        return -1;
    }
    let cell = vec2<i32>(floor(uv / wind.climate.across));
    if any(cell < vec2<i32>(0)) || any(cell > vec2<i32>(3)) {
        return -1;
    }
    let index = cell.y * 4 + cell.x;
    return wind.climate.layers[index / 4][index % 4];
}

// How many leaves a plant standing at `origin` has, of its kind's: between its kind's early
// and late, by where it stands (climate.md: a stand turns over days).
fn leaves_on(origin: vec3<f32>, layer: i32) -> f32 {
    let at = clamp((parting.climate_from_world * vec4<f32>(origin, 1.0)).xy, vec2<f32>(0.0), vec2<f32>(1.0));
    let year = textureSampleLevel(climate_leaves, climate_leaves_sampler, at, layer, 0.0);
    return mix(year.r, year.g, hash(origin * 1.7 + 0.3));
}

// What a card that fell by its rank carries in its second texture coordinate's y, for the
// fragment shaders: far below any flutter's weight.
const RANKED: f32 = -1000.0;

// A card falls with the leaves it carries (climate.md, D327), the highest ranks first, as cards
// drop with distance (D223): those past how many the plant still has shrink to their middles,
// and are gone. None if the card has no rank, or all of it on.
fn fallen(local: vec3<f32>, tangent: vec4<f32>, on: f32) -> vec4<f32> {
    // Taken whichever way the tangent's sign turned it, as a mirrored card's is.
    let rank = floor(abs(tangent.w));
    let count = floor(fract(abs(tangent.w)) * 100.0 + 0.02);
    if count < 1.0 || on >= 0.999 {
        return vec4<f32>(local, 1.0);
    }
    let stays = clamp(count * on - rank, 0.0, 1.0);
    let middle = local - tangent.xyz;
    return vec4<f32>(middle + tangent.xyz * stays, stays);
}

// Plants yield to bodies walking through them (D271), as Ghost of Tsushima's grass takes
// characters' displacement: what is within a body's reach, between its feet and head, is
// pushed out round it, level with the ground and a little down, by as much as it bends
// freely (`give`, 0 at a root). `then` is 0 for where bodies are now, 2 for where they were
// a frame ago. Plants lean aside, brushing the body, rather than part round it like a door
// (D274): within BEND_REACH_M of its side, by at most BEND of its radius.
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

// How freely a vertex bends, from its wind weight: tips of shrubs and ferns freely, a
// tree's trunk and low limbs hardly.
fn give(weights: vec2<f32>) -> f32 {
    return clamp(sqrt(max(weights.x, 0.0)) * 3.0, 0.0, 1.0);
}

// A crown's cards drop out with distance, as SpeedTree's leaves do (D223): each vertex
// knows its offset from its card's middle, and its card's rank and how many its clump has
// (the tangent's w: the rank, plus a hundredth of the count and a thousandth of how many it
// keeps). From cards.x to cards.y the clump keeps fewer, down to those: the next card to go
// shrinks to its middle while those that stay grow to cover as much. The tree's distance from the camera is the same in its shadow's
// pass, which Bevy measures from the camera too.
fn dropped(local: vec3<f32>, tangent: vec4<f32>, tree: vec3<f32>) -> vec3<f32> {
    let rank = floor(tangent.w);
    // Its count in hundredths, and in thousandths how many its clump keeps far off (two,
    // D318).
    let counted = fract(tangent.w) * 100.0;
    let count = floor(counted + 0.02);
    let least = max(round((counted - count) * 10.0), 1.0);
    let far = distance(tree, view.lod_view_world_position);
    let t = smoothstep(wind.cards.x, wind.cards.y, far);
    let keep = mix(count, min(least, count), t);
    let stays = clamp(keep - rank, 0.0, 1.0);
    let grow = pow(count / max(keep, 1.0), 0.55);
    let middle = local - tangent.xyz;
    return middle + tangent.xyz * grow * stays;
}

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
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
    // The world's wind, with how much this plant sways in it; its own way until there is one.
    let blowing = parting.wind.w >= 0.0;
    let direction = select(wind.direction, parting.wind.xyz, blowing);
    let downwind = dot(world, direction);
    let crosswind = dot(world, cross(direction, normalize(world)));
    let gust = sin(time * 0.9 - downwind * 0.07 + crosswind * 0.03) * 0.6
        + sin(time * 1.9 - downwind * 0.23 - crosswind * 0.11) * 0.4;
    let strength = gusts(downwind, crosswind, time);
    let bend = direction * (0.45 + gust) * strength * weights.x * 0.25;
    let flutter = vec3<f32>(
        // Its phase from where it is at the scale of a branch, not of a leaf: from each
        // vertex's own place, a card's corners shook apart and every card wobbled like jelly
        // (D282); now a spray flutters as one and its neighbours a little differently.
        sin(time * 4.1 + world.x * 0.29 + world.z * 0.14),
        sin(time * 3.3 + world.y * 0.21 + world.x * 0.11),
        sin(time * 3.7 + world.z * 0.26 + world.y * 0.16),
    ) * weights.y * 0.015 * (0.5 + 0.5 * strength);
    // A card is a cluster of leaves, which rustles, not bounces: at 0.05 every clump in a crown
    // shook by its own few centimetres several times a second, three quarters of all the motion
    // a still view of a crown showed (D282).
    return (bend + flutter) * wind.strength * select(1.0, parting.wind.w, blowing);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var local = vertex.position;
    // Whether this card falls by its rank here; if not, the fragment shaders thin it (`bare`).
    var ranked = false;
@if(VERTEX_TANGENTS) {
    // A leaf card of a kind whose leaves fall, falling, or gone (D327); otherwise dropping
    // with distance as before.
    var falling = 1.0;
@if(VERTEX_UVS_A) {
    if wind.bark < 0.5 {
        let layer = card_layer(vertex.uv);
        if layer >= 0 {
            ranked = floor(fract(abs(vertex.tangent.w)) * 100.0 + 0.02) >= 1.0;
            let fell = fallen(local, vertex.tangent, leaves_on(world_from_local[3].xyz, layer));
            local = fell.xyz;
            falling = fell.w;
        }
    }
}
    if wind.cards.z > 0.5 && falling >= 1.0 {
        local = dropped(local, vertex.tangent, world_from_local[3].xyz);
    }
}
@if(VERTEX_COLORS) {
    // A shadow stand-in shrinks into its middles rather than ending, since Bevy's shadow
    // passes do not dither; the offset from its middle is in its colour, which only the sun
    // sees and the shadow pass is given, unlike tangents (D234).
    if wind.shrink.y > 0.0 || wind.shrink.w > 0.0 {
        let far = distance(world_from_local[3].xyz, view.lod_view_world_position);
        var keep = 1.0;
        if wind.shrink.y > 0.0 {
            keep *= 1.0 - smoothstep(wind.shrink.x, wind.shrink.y, far);
        }
        if wind.shrink.w > 0.0 {
            keep *= smoothstep(wind.shrink.z, wind.shrink.w, far);
        }
        local = local - vertex.color.rgb * (1.0 - keep);
    }
}
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(local, 1.0));
@if(VERTEX_UVS_B)
    world = vec4<f32>(world.xyz + sway(world.xyz, vertex.uv_b, globals.time) + parted(world.xyz, give(vertex.uv_b), 0u), 1.0);
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
    // Each tree's bark shifted by where it stands, so no two trunks of a kind show the same
    // bark at the same place (D218).
    if wind.bark > 0.5 {
        let foot = world_from_local[3].xyz;
        out.uv += fract(vec2<f32>(sin(dot(foot, vec3<f32>(12.9898, 78.233, 37.719))), sin(dot(foot, vec3<f32>(39.346, 11.135, 83.155)))) * 43758.547);
    }
@if(VERTEX_UVS_B) {
    out.uv_b = vertex.uv_b;
    // A leaf card that fell by its rank is marked so, its flutter's weight being of no more
    // use past here, so the fragment shaders do not thin what is left of it again.
    if ranked {
        out.uv_b.y = RANKED;
    }
}
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
@if(VERTEX_COLORS) {
    out.color = vertex.color;
    // A dappled shadow stand-in's offset from its lump's middle, turned into the world, for
    // the shadow pass to find its rim as the sun sees it (`dapple`, D359).
    if wind.dapple > 0.0 {
        out.color = vec4<f32>(mesh_functions::mesh_normal_local_to_world(vertex.color.rgb, vertex_in.instance_index), 1.0);
    }
}
@if(MOTION_VECTOR_PREPASS) {
    let previous_from_local = mesh_functions::get_previous_world_from_local(vertex_in.instance_index);
    var previous = mesh_functions::mesh_position_local_to_world(previous_from_local, vec4<f32>(local, 1.0));
@if(VERTEX_UVS_B)
    previous = vec4<f32>(previous.xyz + sway(previous.xyz, vertex.uv_b, globals.time - globals.delta_time) + parted(previous.xyz, give(vertex.uv_b), 2u), 1.0);
    out.previous_world_position = previous;
}
} @else {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
// The bark's normal map and relief need the surface's tangents.
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
@if(VERTEX_COLORS) {
    // Each plant is a slightly different shade and hue, decided by where it stands, so no
    // two of a variant look alike. The alpha, how much of the sky it sees, is kept.
    let origin = world_from_local[3].xyz;
    let shade = 0.9 + 0.2 * hash(origin * 7.3);
    let warm = (hash(origin * 3.1 + 17.0) - 0.5) * 0.12;
    out.color = vec4<f32>(vertex.color.rgb * shade * vec3<f32>(1.0 + warm, 1.0, 1.0 - warm), vertex.color.a);
}
}

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_in.instance_index, world_from_local[3]);
    return out;
}
