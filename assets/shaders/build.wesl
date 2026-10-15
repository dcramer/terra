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
    pbr_lighting as lighting,
};

@if(SCREEN_SPACE_AMBIENT_OCCLUSION)
import bevy_pbr::render::mesh_view_bindings::screen_space_ambient_occlusion_texture;
@if(SCREEN_SPACE_AMBIENT_OCCLUSION)
import bevy_pbr::ssao::utils::ssao_multibounce;

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

// How much lighter or darker, and warmer or cooler, each piece of a shingled covering is.
const PIECE_TONE: f32 = 0.18;
const PIECE_WARMTH: f32 = 0.04;
// How many of the cells' texels a pixel spans when that is faded, and when it is gone.
const PIECE_FADE: vec2<f32> = vec2(6.0, 14.0);

// How many texels a side the cells are (`arrays::cells::CELLS`).
const CELLS: f32 = 512.0;
const TAU: f32 = 6.2831853;
// The cells' distances, steps a texel (`terra_build::cells::DISTANCE_SCALE`).
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
// round it from their distance fields (`terra_build::cells::standing`): below 0 in a
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
// lost; what is broken out of timber is its geometry's (`terra_build::timber::break_up`).
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
            // (`terra_build::surface`, their wear), each stone judged once, whole or gone: the
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
    pbr_input.material.metallic = arm.b;
    pbr_input.material.reflectance = vec3(entry.z);

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
    pbr_input.diffuse_occlusion = diffuse_occlusion;
    pbr_input.specular_occlusion = specular_occlusion;

    // The normal map's two stored components, its third made up from them.
    var Nt = vec3(mix(normal, crack_n, broken * 0.85) * 2.0 - 1.0 + detail_n * (1.0 - broken), 0.0);
    Nt = vec3(Nt.xy / max(1.0, length(Nt.xy) / 0.98), 0.0);
    Nt.z = sqrt(max(1.0 - dot(Nt.xy, Nt.xy), 0.0));
    pbr_input.N = normalize(Nt.x * T + Nt.y * B + Nt.z * N);
    pbr_input.clearcoat_N = pbr_input.N;

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
