// Construction materials from texture arrays (D290, docs/proposals/construction-002.md): every
// texture set a layer of four arrays (colour, normal, occlusion-roughness-metal, height), each
// corner carrying the layer of its material in its second texture place (uv_b.x), so a
// section's surfaces in any number of materials are one draw. A table maps each layer to the
// slot of the arrays it is resident in and gives its parallax depth and reflectance.
//
// What it computes is what Bevy's StandardMaterial computes with the same textures (its
// colour times the corner's, roughness and metal from ARM, occlusion the least of ARM's and
// the screen's, the normal map in tangent space, relief mapping by the height), so a material
// looks as it did when it had a StandardMaterial of its own. The relief mapping is ported
// from Bevy's `parallax_mapping.wesl` (MIT or Apache-2.0), reading a layer of the array.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing, calculate_tbn_mikktspace},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
    mesh_view_bindings::globals,
    pbr_lighting as lighting,
};

@if(SCREEN_SPACE_AMBIENT_OCCLUSION)
import bevy_pbr::render::mesh_view_bindings::screen_space_ambient_occlusion_texture;
@if(SCREEN_SPACE_AMBIENT_OCCLUSION)
import bevy_pbr::ssao::utils::ssao_multibounce;
// A debug view (D385, build/views.rs), compiled in only while one is on.
@if(BUILD_VIEW)
import super::build_views::{Seen, shown, BUILT, LIGHT_ALONE, GREY_CARD};

// Field order must match BuildArrays in crates/terra_game/src/build/arrays/mod.rs.
struct Layers {
    // For each layer: x its slot in the arrays, y its parallax depth (0 for none), z its
    // reflectance, w how many layers relief mapping steps through at the preset (0: none).
    table: array<vec4<f32>, 512>,
    // The shared damage mask (D290, D305): x its slot, y whether it is drawn (else a damaged
    // surface is darkened by what it has lost, as far off), z how much darker all but gone.
    damage: vec4<f32>,
    // For each layer, four to an entry: the slot of the texture set it wears down to (a
    // log's bark to its bare wood) and one, or 0 for none.
    worn: array<vec4<f32>, 128>,
    // For each layer, how it breaks (D305): x which way (0 the shared mask, 1 at its
    // joints, 2 along its grain, 3 a sawn end, 4 a shingle at a time), y its splits along
    // the grain or its end's pictures across.
    breaks: array<vec4<f32>, 512>,
    // For each layer, its detail texture (D362): x the slot of its set and one (0: none), y
    // and z how many copies of it a copy of the layer covers across and down, w how strongly.
    detail: array<vec4<f32>, 512>,
    // Where detail fades: x from how far, y by how far, in metres; z 1 where it is drawn.
    near: vec4<f32>,
    // The debug view on (D385): x which (0 none), y texels across a copy of a layer's colour,
    // z the screen pixels a texel should cover a metre off.
    view: vec4<f32>,
    // One build weathered (crates/terra_bases/src/weathering.rs): rows 0-2 the world-to-build
    // affine in cubes, 3 (its least cube, 1 if on), 4 (its box, maps a row of the atlas).
    weather: array<vec4<f32>, 6>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> layers: Layers;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var colour_array: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var array_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var normal_array: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(104)
var arm_array: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(105)
var height_array: texture_2d_array<f32>;
// Each texture set's cells (D305): its stones or shingles, each a number of its own, its
// joints 0; read texel by texel, unfiltered.
@group(constants::MATERIAL_BIND_GROUP) @binding(106)
var cells_array: texture_2d_array<f32>;
// The fire lights (crates/terra_world/src/fire/light.rs `FireGlows`, GLOWS of them): xyz
// where, w its flicker now, a share of its mean, 0 for none.
@group(constants::MATERIAL_BIND_GROUP) @binding(107)
var<storage, read> fire_glows: array<vec4<f32>, 16>;
// The weathered build's faces: each cube's six, its map's slot and one, or 0.
@group(constants::MATERIAL_BIND_GROUP) @binding(108)
var weather_lookup: texture_2d<u32>;
// Its maps, three layers: crack, loss, moss, streak; grime, wear, lichen, damp; soot.
@group(constants::MATERIAL_BIND_GROUP) @binding(109)
var weather_atlas: texture_2d_array<f32>;

struct Weather {
    a: vec4<f32>,
    b: vec4<f32>,
    c: vec4<f32>,
    on: bool,
}

// The weathering maps where a point of the weathered build is (terra_weathering): the cube
// whose face it is, by the normal's strongest axis, else on a log's round its strongest
// level one, a little inside; that face's map at where the point is across it, its u along
// z on the x faces and x otherwise, its v up on walls and along z on floors and roofs.
// A roof's covering lies sloped over its cubes, so its cubes' faces are steps, a map each, and
// banded it; it takes its moss and lichen from noise instead, the same in either way: moss's
// want rising toward the eaves and drifting broadly across the slope (the fragment lays it in
// the laps under each course's butt, by the covering's height), lichen's crusts a few
// centimetres across, both by the building's age; c.y how far down toward the eaves.
fn weather_at(wp: vec3<f32>, wn: vec3<f32>, roof: bool) -> Weather {
    var out = Weather(vec4(0.0), vec4(0.0), vec4(0.0), false);
    let w = layers.weather;
    if w[3].w < 0.5 {
        return out;
    }
    let p = vec4(wp, 1.0);
    let local = vec3(dot(w[0], p), dot(w[1], p), dot(w[2], p));
    let n = vec3(dot(w[0].xyz, wn), dot(w[1].xyz, wn), dot(w[2].xyz, wn));
    let a = abs(n);
    let low = vec3<i32>(w[3].xyz);
    let dims = vec3<i32>(w[4].xyz);
    if roof {
        let box = local - vec3<f32>(low);
        if any(box < vec3(-1.0)) || any(box > vec3<f32>(dims) + 1.0) {
            return out;
        }
        let aged = clamp(w[5].y / 60.0, 0.0, 1.0);
        let down = 1.0 - clamp(box.y / max(f32(dims.y), 1.0), 0.0, 1.0);
        let q = local.xz + local.y * 0.37;
        let drift = noise21(q * 0.45) * 0.7 + noise21(q * 1.6) * 0.3;
        out.a.z = aged * (0.08 + 0.92 * down * down) * (0.55 + 0.9 * drift);
        let crust = noise21(q * 9.0 + 13.0) * 0.6 + noise21(q * 27.0) * 0.4;
        out.b.z = smoothstep(0.56, 0.7, crust + 0.1 * down) * aged;
        out.b.x = 0.15 * aged * down;
        out.c.y = down;
        out.on = true;
        return out;
    }
    let strongest = select(select(0, 2, a.z > a.x), 1, a.y > a.x && a.y > a.z);
    let level = select(0, 2, a.z > a.x);
    var slot = 0u;
    var axis = 0;
    for (var k = 0; k < 4 && slot == 0u; k++) {
        axis = select(strongest, level, k >= 2);
        var d = vec3(0.0);
        d[axis] = select(1.0, -1.0, n[axis] < 0.0);
        let face = axis * 2 + select(0, 1, n[axis] < 0.0);
        let cube = vec3<i32>(floor(local - d * select(0.24, 0.7, (k % 2) == 1))) - low;
        if all(cube >= vec3(0)) && all(cube < dims) {
            slot = textureLoad(weather_lookup, vec2(cube.x * 6 + face, cube.z * dims.y + cube.y), 0).r;
        }
    }
    if slot == 0u {
        return out;
    }
    let u = select(0, 2, axis == 0);
    let v = select(1, 2, axis == 1);
    let f = vec2(fract(local[u]), 1.0 - fract(local[v]));
    let per_row = u32(w[4].w);
    let s = slot - 1u;
    let texel = vec2(f32(s % per_row), f32(s / per_row)) * 32.0 + clamp(f * 32.0, vec2(0.5), vec2(31.5));
    let uv = texel / (f32(per_row) * 32.0);
    out.a = textureSampleLevel(weather_atlas, array_sampler, uv, 0, 0.0);
    out.b = textureSampleLevel(weather_atlas, array_sampler, uv, 1, 0.0);
    out.c = textureSampleLevel(weather_atlas, array_sampler, uv, 2, 0.0);
    out.on = true;
    return out;
}

// How bright the fires round a place burn now, a share of their mean: each fire light's
// flicker by how near it is, out to 12 m; 1 where none is near. One flicker per fire (D415):
// burning char flares as the fire's light does, never on a pulse of its own.
fn fire_glow(wp: vec3<f32>) -> f32 {
    var sum = 0.0;
    var weight = 0.0;
    for (var i = 0u; i < 16u; i++) {
        let fire = fire_glows[i];
        if fire.w <= 0.0 {
            continue;
        }
        let d = distance(fire.xyz, wp);
        let w = max(0.0, 1.0 - d / 12.0) / (1.0 + d * d);
        sum += fire.w * w;
        weight += w;
    }
    return select(1.0, sum / max(weight, 1.0e-6), weight > 1.0e-6);
}

// How much lighter or darker, and warmer or cooler, each piece of a shingled covering is.
const PIECE_TONE: f32 = 0.18;
const PIECE_WARMTH: f32 = 0.04;
// How many of the cells' texels a pixel spans when that is faded, and when it is gone.
const PIECE_FADE: vec2<f32> = vec2(6.0, 14.0);

// How many texels a side the cells are (`arrays::cells::CELLS`).
const CELLS: f32 = 512.0;
const TAU: f32 = 6.2831853;
// The cells' distances, steps a texel (`terra_build_mesh::cells::DISTANCE_SCALE`).
const DISTANCE_SCALE: f32 = 16.0;

// A number from 0 to 1 for a point of a lattice, by integer hashing (a sine's hash loses
// its precision far from the origin and draws in blocks on some GPUs).
fn hash21(p: vec2<f32>) -> f32 {
    let q = vec2<u32>(vec2<i32>(floor(p)) + vec2(4096));
    var h = (q.x * 1597334677u) ^ (q.y * 3812015801u);
    h = (h ^ (h >> 15u)) * 0x2C1B3C6Du;
    h = h ^ (h >> 12u);
    return f32(h & 0xFFFFFFu) / 16777215.0;
}

// Moss's cushions (weathering): lumps a few centimetres across, 0 to 1, at a point of the
// surface in metres along its tangent and bitangent.
fn moss_lumps(s: vec2<f32>) -> f32 {
    return noise21(s * 16.0) * 0.6 + noise21(s * 41.0 + 7.0) * 0.4;
}

// Smooth value noise.
fn noise21(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash21(i), hash21(i + vec2(1.0, 0.0)), u.x);
    let b = mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), u.x);
    return mix(a, b, u.y);
}

// A set's cell at a place on its texture, wrapping: 0 a joint, else its number.
fn cell_at(uv: vec2<f32>, slot: i32) -> f32 {
    let at = vec2<i32>(floor(fract(uv) * CELLS)) % vec2<i32>(i32(CELLS));
    return textureLoad(cells_array, at, slot, 0).r;
}

// Where the middle of the cell at a point is on its texture: the cells hold the way to it.
fn middle_at(uv: vec2<f32>, slot: i32) -> vec2<f32> {
    let at = vec2<i32>(floor(fract(uv) * CELLS)) % vec2<i32>(i32(CELLS));
    let way = textureLoad(cells_array, at, slot, 0).gb * 255.0 - 128.0;
    return (floor(uv * CELLS) + 0.5 + way) / CELLS;
}

// Of the four cells' texels round a point, weighed by how near each is: how much of it is
// joint, and how much of it is a cell that has given way, a cell giving way whole when what
// was lost at its middle passes `begin` and its number's share of `span` more; what was lost
// at its middle is what was lost here (`lost`) carried on by how it changes across the
// texture (`slope`).
fn given(uv: vec2<f32>, slot: i32, lost: f32, slope: vec2<f32>, begin: f32, span: f32) -> vec2<f32> {
    let p = uv * CELLS - 0.5;
    let f = fract(p);
    let o = floor(p);
    var out = vec2(0.0);
    for (var k = 0; k < 4; k++) {
        let d = vec2(f32(k % 2), f32(k / 2));
        let w = mix(1.0 - f.x, f.x, d.x) * mix(1.0 - f.y, f.y, d.y);
        let tap = (o + d + 0.5) / CELLS;
        let cell = cell_at(tap, slot);
        let joint = select(0.0, 1.0, cell < 0.01);
        let there = clamp(lost + dot(slope, middle_at(tap, slot) - uv), 0.0, 1.0);
        out += w * vec2(joint, (1.0 - joint) * step(begin + span * cell, there));
    }
    return out;
}

// How far inside its cell a point is, in texels of the cells, read between the four texels
// round it from their distance fields (`terra_build_mesh::cells::standing`): below 0 in a
// joint, so a stone's edge runs smooth however near.
fn in_cell(uv: vec2<f32>, slot: i32) -> f32 {
    let p = uv * CELLS - 0.5;
    let f = fract(p);
    let o = floor(p);
    var out = 0.0;
    for (var k = 0; k < 4; k++) {
        let d = vec2(f32(k % 2), f32(k / 2));
        let w = mix(1.0 - f.x, f.x, d.x) * mix(1.0 - f.y, f.y, d.y);
        let tap = (o + d + 0.5) / CELLS;
        let at = vec2<i32>(floor(fract(tap) * CELLS)) % vec2<i32>(i32(CELLS));
        let texel = textureLoad(cells_array, at, slot, 0);
        let far = texel.a * 255.0 / DISTANCE_SCALE;
        out += w * select(-far, far, texel.r >= 0.01);
    }
    return out;
}

// How what a cube has lost changes across its texture at a pixel, from the pixels beside it:
// worked out where every pixel runs.
fn slope_of(lost: f32, uv: vec2<f32>) -> vec2<f32> {
    let dl = vec2(dpdx(lost), dpdy(lost));
    let dx = dpdx(uv);
    let dy = dpdy(uv);
    let det = dx.x * dy.y - dx.y * dy.x;
    if abs(det) < 1e-14 {
        return vec2(0.0);
    }
    let g = vec2(dl.x * dy.y - dl.y * dx.y, dx.x * dl.y - dy.x * dl.x) / det;
    return clamp(g, vec2(-6.0), vec2(6.0));
}

// As StandardMaterial's: up to the preset's layers, steeper views fewer, then 5 halvings.
const RELIEF_STEPS: u32 = 5u;
// The view's cosine to the surface below which relief mapping is gone, and above which it
// is whole: about 75 and 55 degrees from head on.
const RELIEF_GRAZING: vec2<f32> = vec2(0.25, 0.6);
// How fast the surface may turn, in radians a metre, before relief mapping fades, and by
// when it is gone: a log 30 cm across turns about 7, a block's rounded edge about 25.
const RELIEF_CURVED: vec2<f32> = vec2(10.0, 18.0);

// Timber's damage at a point (D305): how much of it is a split along the grain, how deep
// it goes as a share of the relief's depth, and its torn lips. A split is a straight crack
// down the grain, its sides parallel, tapering shut at its ends, longer and wider as more is
// lost; what is broken out of timber is its geometry's (`terra_build_mesh::timber::break_up`).
fn grain_cut(uv: vec2<f32>, splits: f32, l: f32, aa: f32) -> vec4<f32> {
    let a = uv.x * splits + 0.37;
    let lane = floor(a);
    let dist = min(fract(a), 1.0 - fract(a));
    let reach = noise21(vec2(lane * 7.3 + 1.7, uv.y * 1.2));
    let open = smoothstep(1.0 - l * 1.4, 1.25 - l * 1.4, reach);
    let width = (0.012 + 0.07 * l) * open * (0.6 + 0.6 * hash21(vec2(lane, 5.0)));
    let split = (1.0 - smoothstep(width, width + aa, dist)) * step(0.02, open);
    // Its lips torn up pale, the fibres pulled out of either side.
    let lip = (1.0 - smoothstep(width + aa, width * 1.5 + 0.01 + aa, dist)) * step(0.02, open) * (1.0 - split);
    return vec4(split, 0.0, split, lip);
}

// The relief's depth at a point: its material's, and where timber is damaged, its splits'
// and gouges' (`splits` none where it is not).
fn sample_depth(uv: vec2<f32>, slot: i32, splits: f32, l: f32) -> f32 {
    // A fixed level, as Bevy's does: a level from derivatives cannot be taken in a loop.
    var d = textureSampleLevel(height_array, array_sampler, uv, slot, 0.0).r;
    if splits > 0.0 {
        d = max(d, grain_cut(uv, splits, l, 0.004).z);
    }
    return d;
}

/// Relief mapping (Bevy's `parallaxed_uv`): where the view ray meets the height field.
fn parallaxed_uv(depth_scale: f32, max_layers: f32, original_uv: vec2<f32>, Vt: vec3<f32>, slot: i32, splits: f32, l: f32) -> vec2<f32> {
    var uv = original_uv;
    let view_steepness = abs(Vt.z);
    let layer_count = mix(max_layers, 1.0, view_steepness);
    let layer_depth = 1.0 / layer_count;
    var delta_uv = depth_scale * layer_depth * Vt.xy * vec2(1.0, -1.0) / view_steepness;
    var current_layer_depth = 0.0;
    var texture_depth = sample_depth(uv, slot, splits, l);
    for (var i: i32 = 0; texture_depth > current_layer_depth && i <= i32(layer_count); i++) {
        current_layer_depth += layer_depth;
        uv += delta_uv;
        texture_depth = sample_depth(uv, slot, splits, l);
    }
    delta_uv *= 0.5;
    var delta_depth = 0.5 * layer_depth;
    uv -= delta_uv;
    current_layer_depth -= delta_depth;
    for (var i: u32 = 0u; i < RELIEF_STEPS; i++) {
        texture_depth = sample_depth(uv, slot, splits, l);
        delta_uv *= 0.5;
        delta_depth *= 0.5;
        if (texture_depth > current_layer_depth) {
            uv += delta_uv;
            current_layer_depth += delta_depth;
        } else {
            uv -= delta_uv;
            current_layer_depth -= delta_depth;
        }
    }
    return uv;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    // How the texture and the world change across the pixel, for a debug view, taken before
    // anything may be discarded.
    var view_uv_d = vec4(0.0);
    var view_p_dx = vec3(0.0);
    var view_p_dy = vec3(0.0);
@if(BUILD_VIEW) {
    view_uv_d = vec4(dpdx(in.uv), dpdy(in.uv));
    view_p_dx = dpdx(in.world_position.xyz);
    view_p_dy = dpdy(in.world_position.xyz);
}
    let layer = min(u32(round(in.uv_b.x)), 511u);
    let entry = layers.table[layer];
    // A set not resident yet (its slot below 0) is drawn plain, in its corners' colours,
    // undamaged: never in the pictures of the slot it would read, the damage mask's.
    let missing = entry.x < 0.0;
    let slot = max(i32(entry.x), 0);
    let depth = select(entry.y, 0.0, missing);

    let TBN = calculate_tbn_mikktspace(pbr_input.world_normal, in.world_tangent);
    let T = TBN[0];
    let B = TBN[1];
    let N = TBN[2];
    var uv = in.uv;
    // How much its cube has lost, spreading in patches across it (D305); and timber's
    // splits, which the relief steps into where it is damaged.
    let lost = clamp(in.uv_b.y, 0.0, 1.0);
    let slope = slope_of(lost, in.uv);
    let wp = in.world_position.xyz;
    let patchy = noise21(vec2(wp.x + wp.z * 0.71, wp.y * 1.3) * 1.6);
    let l = clamp(lost * (0.6 + 0.8 * patchy), 0.0, 1.0);
    let way = select(i32(layers.breaks[layer].x), 0, missing);
    let cut_wood = way == 2 && lost > 0.001 && layers.damage.y > 0.5;
    let splits = select(0.0, max(layers.breaks[layer].y, 1.0), cut_wood);
    let relief_depth = select(depth, max(depth, 0.02), cut_wood);
    // How sharply the surface turns here, in radians a metre: its normal's change across the
    // pixel against its position's. Relief mapping takes the surface as a plane; across a
    // block's rounded edge, a few centimetres round, the march ran down it as streaks, so it
    // is faded out there, and kept on a log's round, which turns a fifth as fast.
    let turn = length(fwidth(pbr_input.world_normal)) / max(length(fwidth(wp)), 1e-5);
    let flat_enough = 1.0 - smoothstep(RELIEF_CURVED.x, RELIEF_CURVED.y, turn);
    // None at the low preset: its frame is its pixels' shading, and a step costs a fetch each.
    if relief_depth > 0.0 && entry.w > 0.0 {
        let V = pbr_input.V;
        let Vt = vec3(dot(V, T), dot(V, B), dot(V, N));
        // Faded out as the view grazes the surface: relief mapping takes the surface as a
        // plane, and a ray along a rounded edge or a face built up round a sill marched so far
        // across the texture that it drew streaks and fins (research/kit-review-2026-10-14.md);
        // as Unreal's parallax occlusion fades by view angle.
        let grazing = smoothstep(RELIEF_GRAZING.x, RELIEF_GRAZING.y, abs(Vt.z)) * flat_enough;
        if grazing > 0.0 {
            uv = parallaxed_uv(relief_depth * grazing, entry.w, uv, -Vt, slot, splits, l);
        }
    }

    let bias = view.mip_bias;
    var colour = textureSampleBias(colour_array, array_sampler, uv, slot, bias);
    var arm = textureSampleBias(arm_array, array_sampler, uv, slot, bias);
    var normal = textureSampleBias(normal_array, array_sampler, uv, slot, bias).rg;
    if missing {
        colour = vec4(1.0);
        arm = vec4(1.0, 0.85, 0.0, 1.0);
        normal = vec2(0.5);
    }

    // Worn (D290): where its corners say its face is worn (the vertex colour's alpha, one
    // less how worn), the set its layer wears down to shows through, through the low places
    // of its height first, as engines blend a material's layers by their heights: a log's
    // bark lost toward its sawn ends and on its weather side to the bare wood under it.
    // Taken by gradients worked out where every pixel runs, as it is sampled in a branch.
    var wear = 0.0;
@if(VERTEX_COLORS) {
    wear = 1.0 - in.color.a;
}
    let scale = exp2(bias);
    let ddx = dpdx(uv) * scale;
    let ddy = dpdy(uv) * scale;
    let under = select(i32(layers.worn[layer / 4u][layer % 4u]) - 1, -1, missing);
    if under >= 0 && wear > 0.002 {
        let high = textureSampleGrad(height_array, array_sampler, uv, slot, ddx, ddy).r;
        let bare = smoothstep(0.0, 0.25, wear * 1.4 - high);
        let under_colour = textureSampleGrad(colour_array, array_sampler, uv, under, ddx, ddy);
        let under_arm = textureSampleGrad(arm_array, array_sampler, uv, under, ddx, ddy);
        let under_normal = textureSampleGrad(normal_array, array_sampler, uv, under, ddx, ddy).rg;
        colour = mix(colour, under_colour, bare);
        arm = mix(arm, under_arm, bare);
        normal = mix(normal, under_normal, bare);
    }

    // Detail (D362): its material's fine grain, wood's or stone's, a small texture tiled a
    // few times a metre over it near and faded out a few metres off, finer than its own
    // texels, as shipped games lay a detail map: its colour grey about the middle, so it
    // brightens and darkens as Unity's detail albedo does (times 2 in sRGB, 4.59 in linear
    // light), and its normal added to the surface's (the partial derivatives' sum, UDN).
    var detail_n = vec2(0.0);
    let fine = layers.detail[layer];
    let fine_slot = i32(fine.x) - 1;
    let reach = 1.0 - smoothstep(layers.near.x, layers.near.y, length(view.world_position - wp));
    let fine_k = select(0.0, fine.w * reach, fine_slot >= 0 && layers.near.z > 0.5 && !missing);
    if fine_k > 0.001 {
        let fuv = uv * fine.yz;
        let fdx = ddx * fine.yz;
        let fdy = ddy * fine.yz;
        let fine_colour = textureSampleGrad(colour_array, array_sampler, fuv, fine_slot, fdx, fdy).rgb;
        let fine_normal = textureSampleGrad(normal_array, array_sampler, fuv, fine_slot, fdx, fdy).rg * 2.0 - 1.0;
        colour = vec4(colour.rgb * mix(vec3(1.0), fine_colour * 4.59479, fine_k), colour.a);
        detail_n = fine_normal * fine_k;
    }

    // Its colour on average, for what is behind it where a piece of it has gone.
    let mean = pbr_input.material.base_color.rgb
        * textureSampleLevel(colour_array, array_sampler, uv, slot, 8.0).rgb;
    pbr_input.material.base_color = vec4(pbr_input.material.base_color.rgb * colour.rgb, 1.0);
    pbr_input.material.perceptual_roughness = arm.g;
    // A covering laid a shingle at a time (D305's cells, as its damage finds them): each
    // slate or shake a little lighter or darker, warmer or cooler than the next, as no two
    // from a quarry or a tree are alike, so the courses read as pieces, not boards.
    // Read unfiltered, so faded out before a piece is a few pixels across, where it would
    // sparkle instead.
    let cell_px = max(length(dpdx(uv)), length(dpdy(uv))) * CELLS;
    let piece_shown = 1.0 - smoothstep(PIECE_FADE.x, PIECE_FADE.y, cell_px);
    if way == 4 && piece_shown > 0.0 {
        let cell = cell_at(uv, slot);
        if cell > 0.0 {
            let n = cell * 255.0;
            let tone = 1.0 + piece_shown * PIECE_TONE * (2.0 * hash21(vec2(n, 7.0)) - 1.0);
            let warm = piece_shown * PIECE_WARMTH * (2.0 * hash21(vec2(n, 13.0)) - 1.0);
            pbr_input.material.base_color = vec4(
                pbr_input.material.base_color.rgb * tone * vec3(1.0 + warm, 1.0, 1.0 - warm),
                1.0,
            );
        }
    }

    // Damage (D305), by the structure of what breaks, one way a pattern: a wall's mortar
    // washes out of its joints, its stones' edges chip back to their fresh face and then
    // whole stones fall out, leaving their hollows; timber splits along its grain, chunks
    // gouge out of it and its sawn ends check from the rim; a covering's shingles crack and
    // then go missing, showing the dark under them; anything else by the shared crack and
    // chip mask. What a cube has lost spreads in patches across it (where the hits landed
    // are the cubes they landed on), so a damaged wall is broken here and there, more as it
    // loses more. Where none of it is drawn, darkened as far off.
    let fw = fwidth(uv);
    var broken = 0.0;
    var crack_n = vec2(0.5, 0.5);
    if layers.damage.y > 0.5 && !missing {
        let mask = i32(layers.damage.x);
        let threshold = textureSampleBias(height_array, array_sampler, uv, mask, bias).r;
        let flake = textureSampleBias(colour_array, array_sampler, uv, mask, bias).r;
        let mask_n = textureSampleBias(normal_array, array_sampler, uv, mask, bias).rg;
        let base = pbr_input.material.base_color.rgb;
        let grey = vec3(dot(base, vec3(0.3, 0.59, 0.11)));

        var colour_out = base;
        var rough = arm.g;
        if way == 1 {
            // Laid stones: the joints first, then the stones' edges, then whole stones, each
            // leaving its hollow: the mortar bed and rubble behind it, in shade at its rim.
            // Where a stone has gone the wall's surface is sunk, its corners marked so
            // (`terra_build_mesh::surface`, their wear), each stone judged once, whole or gone: the
            // hollow is drawn where it is sunk and nowhere else. Every edge's shape is read
            // from the cells' distance field, so it runs smooth however near.
            // How wide a pixel is in texels of the cells, to soften an edge over.
            let aa = max(max(fw.x, fw.y) * CELLS * 0.7, 0.15);
            let own = in_cell(uv, slot);
            let joint = 1.0 - smoothstep(-aa, aa, own);
            // Anywhere sunk at all is the hole: its floor all sunk, its sides between the
            // face and its floor, where nothing of the stone's face is drawn, stretched.
            let fall = step(0.0001, wear);
            let side = fall * (1.0 - step(0.999, wear));
            let erode = joint * smoothstep(0.02, 0.3, l) * (1.0 - fall);
            // A standing stone's edge, chipped back to its fresh face only once much is lost.
            let edge = (1.0 - joint) * (1.0 - smoothstep(0.5, 3.0, own));
            let bite = noise21(uv * 34.0) * 0.6 + noise21(uv * 9.0) * 0.4;
            let chip = edge * smoothstep(bite - 0.05, bite, smoothstep(0.5, 0.95, l) * 0.5)
                * (1.0 - fall);
            let fresh = mix(base, grey, 0.3) * 1.05;
            // A hollow: the rubble and mortar bed behind, darker up its sides.
            let rubble = noise21(uv * 30.0) * 0.6 + noise21(uv * 8.0) * 0.4;
            let shade = 1.0 - 0.35 * (1.0 - smoothstep(0.6, 1.0, wear));
            // Behind it, the wall's core: more of its stones, set back in shadow, their
            // joints dark.
            let behind_uv = uv + vec2(0.37, 0.53);
            let behind = textureSampleGrad(colour_array, array_sampler, behind_uv, slot, ddx, ddy).rgb;
            let behind_joint = 1.0 - smoothstep(-aa, aa, in_cell(behind_uv, slot));
            let core = pbr_input.material.base_color.rgb / max(colour.rgb, vec3(0.01)) * behind;
            // Its floor deep in the wall, a stone's thickness and more back, occluded: the
            // core's stones barely lit, near black in their joints.
            let hollow_floor = core * (0.2 - 0.14 * behind_joint) * (0.7 + 0.4 * rubble) * shade;
            // Its sides plain mortar and rubble, by where they are in the world, as their
            // texture would run down them stretched.
            let grit = noise21(vec2(wp.x + wp.z, wp.y) * 40.0) * 0.6 + noise21(vec2(wp.x - wp.z, wp.y) * 11.0) * 0.4;
            let mortar = mean * (0.2 + 0.3 * grit * grit);
            let hollow = mix(hollow_floor, mortar, side);
            colour_out = mix(base, base * 0.3, erode);
            colour_out = mix(colour_out, fresh, chip);
            colour_out = mix(colour_out, hollow, fall);
            broken = max(max(erode * 0.6, chip), fall);
            crack_n = mix(mask_n, vec2(0.5, 0.5), fall);
            rough = mix(arm.g, 0.95, broken);
        } else if way == 2 {
            // Timber: splits along the grain, longer and wider as it loses more, dark to
            // their floor; chunks gouged out, the fresh wood under them; their walls in the
            // normal, from the slope of how deep they go.
            let splits = max(layers.breaks[layer].y, 1.0);
            let aa = max(fw.x * splits, 0.002);
            let cut = grain_cut(uv, splits, l, aa);
            let e = 1.5 / 512.0;
            let du = grain_cut(uv + vec2(e, 0.0), splits, l, aa).z - grain_cut(uv - vec2(e, 0.0), splits, l, aa).z;
            let dv = grain_cut(uv + vec2(0.0, e), splits, l, aa).z - grain_cut(uv - vec2(0.0, e), splits, l, aa).z;
            let fresh = mix(base * 1.25, vec3(0.5, 0.36, 0.22), 0.4);
            let floor_shade = 1.0 - 0.45 * cut.z;
            colour_out = mix(base, fresh * floor_shade, cut.y);
            colour_out = mix(colour_out, fresh * 1.1, cut.w * 0.8);
            colour_out = mix(colour_out, base * 0.03, cut.x);
            broken = max(cut.x, cut.y);
            crack_n = clamp(vec2(0.5) + vec2(du, -dv) * 1.2, vec2(0.0), vec2(1.0));
            rough = mix(arm.g, 0.95, broken);
        } else if way == 3 {
            // A sawn end: checks from its rim in toward its heart, its rim splintered.
            let cells = max(layers.breaks[layer].y, 1.0);
            let local = fract(uv * cells) - 0.5;
            let r = length(local) * 2.0;
            let turn = atan2(local.y, local.x) / TAU;
            let a = turn * 9.0 + 0.3 * noise21(vec2(r * 4.0, turn * 9.0));
            let dist = min(fract(a), 1.0 - fract(a));
            let reach = noise21(vec2(floor(a) * 3.1, 0.5));
            let inward = 1.0 - l * (0.5 + 0.6 * reach);
            let check = (1.0 - smoothstep(0.02 + 0.06 * l, 0.05 + 0.06 * l, dist))
                * smoothstep(inward, inward + 0.05, r);
            let splinter = smoothstep(1.0 - 0.3 * l * noise21(vec2(turn * 40.0, 0.0)), 1.0, r);
            colour_out = mix(base, mix(base, grey, 0.3) * 1.3, splinter);
            colour_out = mix(colour_out, base * 0.15, check);
            broken = max(check, splinter);
            rough = mix(arm.g, 0.95, broken);
        } else if way == 4 {
            // Shingles: one cracks across, then whole ones are gone, and what is under them,
            // its battens and rafters, shows through the holes (as build_prepass.wesl leaves
            // them out of depth and shadow); all but gone, the frame alone.
            let cell = cell_at(uv, slot);
            let gap = select(0.0, 1.0, cell < 0.01);
            let hit = lost;
            let shingle = given(uv, slot, hit, slope, 0.3, 0.65);
            if shingle.y + shingle.x * step(0.9, hit) > 0.5 && lost > 0.001 {
                discard;
            }
            // Round a hole, the shingles' edges broken and lifted, in shade.
            let near_hole = given(uv + vec2(0.0, 2.0 / CELLS), slot, hit, slope, 0.3, 0.65).y;
            let cracked = (1.0 - gap) * step(0.08 + 0.55 * cell, l);
            let dir = vec2(cos(cell * 40.0), sin(cell * 40.0) * 0.3 + 0.9);
            let c = dot(uv * 9.0, normalize(dir)) + cell * 13.0;
            let dist = min(fract(c), 1.0 - fract(c));
            let line = (1.0 - smoothstep(0.03, 0.03 + max(fw.x, fw.y) * 9.0, dist)) * cracked;
            colour_out = mix(base, base * 0.2, line);
            colour_out = mix(colour_out, base * 0.45, near_hole);
            broken = line;
            crack_n = mask_n;
            rough = mix(arm.g, 0.95, broken);
        } else if way == 5 {
            // Daub and mud: crumbling, darker and rougher in patches, bits of it gone.
            let crumble = smoothstep(0.75 - 0.5 * l, 0.8 - 0.5 * l, noise21(uv * 11.0) * 0.7 + noise21(uv * 37.0) * 0.3);
            colour_out = mix(base * (1.0 - 0.25 * l), base * 0.4, crumble);
            broken = crumble;
            crack_n = mask_n;
            rough = mix(arm.g, 0.95, max(broken, l * 0.5));
        } else {
            broken = smoothstep(threshold - 0.03, threshold + 0.03, l);
            let deep = clamp((l - threshold) * 4.0, 0.0, 1.0);
            let crack = base * (0.32 - 0.12 * deep);
            let rim = 1.0 - 0.35 * (1.0 - smoothstep(0.0, 0.12, l - threshold));
            let chip = mix(base, grey, 0.45) * 1.18 * rim;
            colour_out = mix(base, mix(crack, chip, flake), broken);
            crack_n = mask_n;
            rough = mix(arm.g, 0.95, broken);
        }
        // Nothing of it where nothing is lost.
        let shown = step(0.001, lost);
        broken *= shown;
        pbr_input.material.base_color = vec4(mix(base, colour_out, shown), 1.0);
        pbr_input.material.perceptual_roughness = mix(arm.g, rough, shown);
    } else {
        let shade = 1.0 - layers.damage.z * lost;
        pbr_input.material.base_color = vec4(pbr_input.material.base_color.rgb * shade, 1.0);
    }
    // Weathering (the spike, research/2026-10-25-wfc-weathering.md): the weathered build's
    // layers over its material, as engines blend a building's weathering layers: grime down
    // its low places and under its ledges, damp at its foot, its edges worn to fresh stone,
    // streaks down from its sills, soot over its openings, lichen and moss where it is
    // sheltered and damp, its cracks dark, its stones spalled or gone.
    let weather = weather_at(wp, normalize(in.world_normal), way == 4);
    // Moss's cover, its cushions' tilt of the normal and their shade, for the lighting below.
    var moss_cover = 0.0;
    // The tilt of the normal its weathering adds: moss's cushions, aged timber's raised grain.
    var moss_n = vec2(0.0);
    var moss_shade = 1.0;
    // How much of its reflectance an aged surface keeps.
    var reflect_k = 1.0;
    if weather.on {
        var c = pbr_input.material.base_color.rgb;
        var r = pbr_input.material.perceptual_roughness;
        let lum = dot(c, vec3(0.3, 0.59, 0.11));
        let age = clamp(layers.weather[5].y / 60.0, 0.0, 1.0);
        let grain = noise21(vec2(wp.x + wp.z, wp.y) * 9.0) * 0.6 + noise21(vec2(wp.x - wp.z, wp.y) * 31.0) * 0.4;
        // The material's height against its own mean round about (a far mip): its joints, laps
        // and grooves below nought, its stones' tops, butts and ridges above, whatever range a
        // material's height map spans.
        let high = textureSampleGrad(height_array, array_sampler, uv, slot, ddx, ddy).r;
        let high_mean = textureSampleLevel(height_array, array_sampler, uv, slot, 6.0).r;
        let rel = select(high - high_mean, 0.0, missing);
        let st = vec2(dot(wp, T), dot(wp, B));
        let roof = way == 4;
        if roof {
            // An aged covering goes matte and chalky: the sheen of new slate or shakes gone to
            // a dry bloom, lighter and greyer, mottled, so no course mirrors the sky.
            let chalk = noise21(st * 1.7 + 3.0) * 0.6 + noise21(st * 6.0) * 0.4;
            let bloom = mix(vec3(lum), c, 0.4) * (1.25 + 0.3 * chalk) + vec3(0.008);
            c = mix(c, bloom, 0.75 * age);
            r = mix(r, 0.86 + 0.08 * chalk, age);
            reflect_k = 1.0 - 0.35 * age;
        }
        let timber = way == 2 || way == 3;
        let grime = weather.b.x * (0.75 + 0.5 * grain);
        // Timber's grime and damp below, its own: dark already, it would go near black here.
        c = mix(c, mix(vec3(lum), c, 0.5) * vec3(0.42, 0.38, 0.31), clamp(grime, 0.0, 1.0) * select(1.0, 0.0, timber));
        c *= 1.0 - select(0.45, 0.0, timber) * weather.b.w;
        c = mix(c, mix(c, vec3(lum), 0.35) * 1.3, weather.b.y * 0.55);
        if timber {
            // Exposed timber silvers: sun and rain take its brown out to a light-mid grey over
            // decades, the figure of its grain kept, less where it is sheltered (its grime, under
            // a ledge or the eaves) and wet (its damp, the bottom log), which stay browner and
            // darker, the wet greener. Its grain raised as the soft earlywood wears from between
            // the harder latewood: the ridges of its height lighter, its grooves darker, and in
            // the normal; fine checks split along it.
            let shelter = smoothstep(0.1, 0.5, weather.b.x);
            let wet = clamp(weather.b.w, 0.0, 1.0);
            let mean_lum = dot(mean, vec3(0.3, 0.59, 0.11));
            let figure = clamp(lum / max(mean_lum, 0.003), 0.5, 1.6);
            let ridge = clamp(rel * 2.5, -0.5, 0.5);
            let silver = vec3(0.235, 0.228, 0.215) * mix(1.0, figure, 0.45) * (1.0 + ridge * age);
            c = mix(c, silver, age * (1.0 - 0.55 * shelter) * (1.0 - 0.5 * wet));
            c *= 1.0 - 0.3 * clamp(grime, 0.0, 1.0);
            let lanes = uv.x * 23.0 + 0.13;
            let lane = floor(lanes);
            let across = min(fract(lanes), 1.0 - fract(lanes));
            let open = smoothstep(0.6, 0.72, noise21(vec2(lane * 5.7, uv.y * 2.5))) * step(0.45, hash21(vec2(lane, 9.0)));
            let width = 0.035 * open;
            let check = (1.0 - smoothstep(width, width + fw.x * 23.0 + 0.004, across)) * open * age * select(1.0, 0.0, way == 3);
            c = mix(c, c * 0.3, check * 0.85);
            c = mix(c, c * vec3(0.5, 0.58, 0.42), wet * 0.8);
            r = mix(r, 0.82 + 0.08 * grain, age);
            r = mix(r, 0.72, wet * 0.5);
            reflect_k = 1.0 - 0.3 * age;
            let eg = max(1.5 / 512.0, max(fw.x, fw.y));
            let hu = textureSampleGrad(height_array, array_sampler, uv + vec2(eg, 0.0), slot, ddx, ddy).r
                - textureSampleGrad(height_array, array_sampler, uv - vec2(eg, 0.0), slot, ddx, ddy).r;
            let hv = textureSampleGrad(height_array, array_sampler, uv + vec2(0.0, eg), slot, ddx, ddy).r
                - textureSampleGrad(height_array, array_sampler, uv - vec2(0.0, eg), slot, ddx, ddy).r;
            moss_n = select(vec2(-hu, hv) * 1.5 * age, vec2(0.0), missing);
        }
        // Streaks: a faint wash down from a sill or a wall head, a little darker and greyer, never
        // a black line.
        c = mix(c, mix(c, vec3(lum), 0.2) * vec3(0.77, 0.76, 0.74), clamp(weather.a.w, 0.0, 1.0));
        // Soot: a soft smoky plume over an opening, darkening by at most about two thirds.
        c = mix(c, mix(c, vec3(lum), 0.4) * vec3(0.34, 0.32, 0.3), weather.c.x * 0.9);
        var lichen = 0.0;
        if roof {
            // Crustose lichen on a covering: pale grey-green crusts, now and then ochre, on the
            // faces toward each course's butt (its height's high part) and toward the eaves.
            lichen = weather.b.z * (0.3 + 0.7 * smoothstep(-0.05, 0.2, rel));
            let ochre = smoothstep(0.7, 0.85, noise21(st * 3.1 + 21.0));
            let crust = mix(vec3(0.3, 0.31, 0.26), vec3(0.4, 0.3, 0.11), ochre) * (0.85 + 0.3 * grain);
            c = mix(c, crust, lichen * 0.75);
        } else {
            lichen = smoothstep(0.2, 0.6, weather.b.z * (0.6 + 0.8 * grain));
            c = mix(c, c * 0.4 + vec3(0.16, 0.16, 0.1), lichen * 0.7);
        }
        // Moss as cushions in the joints, blended by the material's height as engines blend a
        // layer by height (the Worn block's way): a thin field fills the mortar's low places
        // first and only a strong one climbs over the stones' tops; its edge ragged with noise,
        // never cut out; olive to brown, lumpy, matte, a damp darkening just round it. On a
        // covering, in the laps under each course's butt and the gaps between its pieces, the
        // lowest of its height, spreading over the pieces only toward the eaves.
        let e = 0.004;
        let lump = moss_lumps(st);
        let lump_d = vec2(moss_lumps(st + vec2(e, 0.0)), moss_lumps(st + vec2(0.0, e))) - lump;
        let field = smoothstep(0.12, 0.5, weather.a.z * (0.7 + 0.6 * grain));
        let ragged = (noise21(st * 23.0 + 5.0) - 0.5) * 0.4 + (lump - 0.5) * 0.3;
        var moss = smoothstep(0.0, 0.25, field * 1.6 - 0.8 - rel * 3.0 + ragged);
        if roof {
            let reach = -0.45 + 0.6 * clamp(weather.a.z, 0.0, 1.0);
            moss = smoothstep(-0.04, 0.06, reach - rel + ragged * 0.15);
        }
        let tone = noise21(st * 2.3 + 11.0);
        let olive_brown = mix(vec3(0.08, 0.095, 0.035), vec3(0.08, 0.066, 0.034), smoothstep(0.4, 0.8, tone));
        let fresh = smoothstep(0.55, 0.9, noise21(st * 5.0 + 3.0)) * 0.6;
        let moss_colour = mix(olive_brown, vec3(0.11, 0.125, 0.04), fresh) * (0.7 + 0.5 * lump);
        let damp_band = smoothstep(0.0, 0.25, field) * (1.0 - moss);
        c *= 1.0 - select(0.28, 0.1, roof) * damp_band;
        c = mix(c, moss_colour, moss * 0.94);
        r = mix(r, 0.97, max(moss, lichen));
        moss_cover = moss;
        moss_n = mix(moss_n, -lump_d / e * 0.01, moss);
        moss_shade = 1.0 - 0.35 * moss * (1.0 - lump);
        // Loss: 1 a stone gone, its hollow dark; a half a spalled face, fresh and rough.
        let gone = smoothstep(0.72, 0.88, weather.a.y);
        let spalled = smoothstep(0.3, 0.45, weather.a.y) * (1.0 - gone);
        c = mix(c, mix(c, vec3(lum), 0.4) * 1.2, spalled);
        c = mix(c, c * (0.12 + 0.12 * grain), gone);
        r = mix(r, 0.95, max(spalled, gone));
        c = mix(c, c * 0.1, smoothstep(0.3, 0.7, weather.a.x));
        pbr_input.material.base_color = vec4(c, 1.0);
        pbr_input.material.perceptual_roughness = r;
    }
    // Fire (D377, terra_build::fire): how blackened the corner is rides on its layer number,
    // under a half. What burns chars to charcoal, black, matte and cracked across and along
    // its grain into blocks (alligatoring, as burnt timber checks), its cracks glowing while it
    // burns; what does not is sooted, a smooth dark film thickest where the flame licked.
    let char = clamp((in.uv_b.x - f32(layer)) / 0.4, 0.0, 1.0);
    if char > 0.001 {
        let burns = layers.breaks[layer].z > 0.5;
        let base = pbr_input.material.base_color.rgb;
        if burns {
            let p = vec2(dot(wp, T), dot(wp, B)) * vec2(26.0, 15.0);
            let row = floor(p.y);
            let q = vec2(p.x + hash21(vec2(row, 3.0)) * 0.9, p.y);
            let f = fract(q);
            let edge = min(min(f.x, 1.0 - f.x) * 1.6, min(f.y, 1.0 - f.y));
            let crack = 1.0 - smoothstep(0.03, 0.11, edge);
            let block = hash21(floor(q) + vec2(17.0, 5.0));
            // Grain still showing through the first char, black blocks once it is deep.
            let coal = vec3(0.028, 0.024, 0.021) * (0.7 + 0.6 * block);
            let black = mix(base * 0.35, coal, smoothstep(0.0, 0.55, char));
            let charred = mix(black, vec3(0.006, 0.005, 0.004), crack * smoothstep(0.3, 0.8, char));
            pbr_input.material.base_color = vec4(mix(base, charred, smoothstep(0.0, 0.35, char)), 1.0);
            pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.92, char);
            // Embers in the cracks, hotter in the deeper char, some blocks more than others,
            // flaring by three times the flicker of the fire burning them.
            let flicker = (0.6 + 0.4 * block) * max(1.0 + 3.0 * (fire_glow(wp) - 1.0), 0.15);
            let heat = crack * smoothstep(0.35, 0.9, char) * flicker;
            let ember = vec3(1.0, 0.32, 0.06) * heat * 14.0 + vec3(0.6, 0.12, 0.02) * smoothstep(0.6, 1.0, char) * (1.0 - crack) * block * 1.5;
            pbr_input.material.emissive = vec4(ember, 1.0);
        } else {
            let soot = vec3(0.035, 0.032, 0.03);
            pbr_input.material.base_color = vec4(mix(base, soot, char * 0.88), 1.0);
        }
    }
    pbr_input.material.metallic = arm.b;
    pbr_input.material.reflectance = vec3(entry.z * reflect_k);
    // The light view (D385): an 18% grey lit as it is, as the grey card beside it.
@if(BUILD_VIEW) {
    if u32(layers.view.x + 0.5) == LIGHT_ALONE {
        pbr_input.material.base_color = vec4(vec3(GREY_CARD), 1.0);
        pbr_input.material.emissive = vec4(0.0);
    }
}

    // Occlusion: the least of the texture's and the screen's, as StandardMaterial's.
    var diffuse_occlusion = vec3(arm.r);
    var specular_occlusion = 1.0;
@if(SCREEN_SPACE_AMBIENT_OCCLUSION) {
    let ssao_tex_pos = vec2<i32>(in.position.xy - view.viewport.xy);
    let ssao = textureLoad(screen_space_ambient_occlusion_texture, ssao_tex_pos, 0i).r;
    diffuse_occlusion = min(diffuse_occlusion, ssao_multibounce(ssao, pbr_input.material.base_color.rgb));
    let NdotV = max(dot(pbr_input.N, pbr_input.V), 0.0001);
    let roughness = lighting::perceptualRoughnessToRoughness(arm.g);
    specular_occlusion = saturate(pow(NdotV + ssao, exp2(-16.0 * roughness - 1.0)) - 1.0 + ssao);
}
    pbr_input.diffuse_occlusion = diffuse_occlusion * moss_shade;
    pbr_input.specular_occlusion = specular_occlusion;

    // The normal map's two stored components, its third made up from them.
    var Nt = vec3(mix(normal, crack_n, broken * 0.85) * 2.0 - 1.0 + detail_n * (1.0 - broken), 0.0);
    // Moss's cushions over the stone: its relief softened under them, their lumps raised.
    Nt = vec3(Nt.xy * (1.0 - 0.6 * moss_cover) + moss_n, 0.0);
    Nt = vec3(Nt.xy / max(1.0, length(Nt.xy) / 0.98), 0.0);
    Nt.z = sqrt(max(1.0 - dot(Nt.xy, Nt.xy), 0.0));
    pbr_input.N = normalize(Nt.x * T + Nt.y * B + Nt.z * N);
    pbr_input.clearcoat_N = pbr_input.N;

    // A debug view: the surface coloured by what it shows, unlit (D385, build_views.wesl);
    // but the light view, lit below.
@if(BUILD_VIEW) {
    if layers.view.x > 0.5 && u32(layers.view.x + 0.5) != LIGHT_ALONE {
        var seen: Seen;
        seen.view = u32(layers.view.x + 0.5);
        seen.uv = in.uv;
        seen.uv_dx = view_uv_d.xy;
        seen.uv_dy = view_uv_d.zw;
        seen.position = wp;
        seen.p_dx = view_p_dx;
        seen.p_dy = view_p_dy;
        seen.pixel = in.position.xy;
        seen.side = select(layers.view.y, 0.0, missing);
        seen.aim = layers.view.z;
        seen.geometric = N;
        seen.mapped = pbr_input.N;
        seen.tangent = T;
        seen.bitangent = B;
        seen.framed = 1.0;
        seen.lost = lost;
        seen.broken = broken;
        seen.masked = layers.damage.y;
        seen.texture_occlusion = select(arm.r, 1.0, diffuse_occlusion.r < arm.r - 1e-4);
        seen.screen_occlusion = select(1.0, diffuse_occlusion.r, diffuse_occlusion.r < arm.r - 1e-4);
        var corners = 1.0;
@if(VERTEX_COLORS) {
        corners = min(dot(in.color.rgb, vec3(0.2126, 0.7152, 0.0722)), 1.0);
}
        // Where the damage is not drawn, a damaged surface is darkened instead.
        seen.baked = corners * select(1.0 - layers.damage.z * lost, 1.0, layers.damage.y > 0.5);
        seen.albedo = clamp(dot(colour.rgb, vec3(0.2126, 0.7152, 0.0722)) * 2.0, 0.0, 1.0);
        seen.flat = BUILT;
        var shown_out: FragmentOutput;
        shown_out.color = vec4(shown(seen), 1.0);
        return shown_out;
    }
}

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
