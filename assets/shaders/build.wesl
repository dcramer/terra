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

// How many texels a side the cells are (`arrays::cells::CELLS`).
const CELLS: f32 = 512.0;
const TAU: f32 = 6.2831853;

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

// Of the four cells' texels round a point, weighed by how near each is: how much of it is
// joint, and how much of it is a cell that has given way, a cell giving way when what is
// lost passes `begin` and its number's share of `span` more.
fn given(uv: vec2<f32>, slot: i32, lost: f32, begin: f32, span: f32) -> vec2<f32> {
    let p = uv * CELLS - 0.5;
    let f = fract(p);
    let o = floor(p);
    var out = vec2(0.0);
    for (var k = 0; k < 4; k++) {
        let d = vec2(f32(k % 2), f32(k / 2));
        let w = mix(1.0 - f.x, f.x, d.x) * mix(1.0 - f.y, f.y, d.y);
        let cell = cell_at((o + d + 0.5) / CELLS, slot);
        let joint = select(0.0, 1.0, cell < 0.01);
        out += w * vec2(joint, (1.0 - joint) * step(begin + span * cell, lost));
    }
    return out;
}

// How far round a point its cell gives way to another or a joint, from 0 (deep in its
// cell) to 1 (at its edge), over `reach` texels of the cells.
fn edge_of(uv: vec2<f32>, slot: i32, cell: f32, reach: f32) -> f32 {
    let d = reach / CELLS;
    var out = 0.0;
    for (var k = 0; k < 8; k++) {
        let a = f32(k) * TAU / 8.0;
        let other = cell_at(uv + vec2(cos(a), sin(a)) * d, slot);
        out = max(out, select(0.0, 1.0, abs(other - cell) > 0.002));
    }
    let near = cell_at(uv + vec2(d, d) * 0.5, slot);
    return max(out * 0.6, select(0.0, 1.0, abs(near - cell) > 0.002));
}

// As StandardMaterial's: up to the preset's layers, steeper views fewer, then 5 halvings.
const RELIEF_STEPS: u32 = 5u;

fn sample_depth(uv: vec2<f32>, slot: i32) -> f32 {
    // A fixed level, as Bevy's does: a level from derivatives cannot be taken in a loop.
    return textureSampleLevel(height_array, array_sampler, uv, slot, 0.0).r;
}

/// Relief mapping (Bevy's `parallaxed_uv`): where the view ray meets the height field.
fn parallaxed_uv(depth_scale: f32, max_layers: f32, original_uv: vec2<f32>, Vt: vec3<f32>, slot: i32) -> vec2<f32> {
    var uv = original_uv;
    let view_steepness = abs(Vt.z);
    let layer_count = mix(max_layers, 1.0, view_steepness);
    let layer_depth = 1.0 / layer_count;
    var delta_uv = depth_scale * layer_depth * Vt.xy * vec2(1.0, -1.0) / view_steepness;
    var current_layer_depth = 0.0;
    var texture_depth = sample_depth(uv, slot);
    for (var i: i32 = 0; texture_depth > current_layer_depth && i <= i32(layer_count); i++) {
        current_layer_depth += layer_depth;
        uv += delta_uv;
        texture_depth = sample_depth(uv, slot);
    }
    delta_uv *= 0.5;
    var delta_depth = 0.5 * layer_depth;
    uv -= delta_uv;
    current_layer_depth -= delta_depth;
    for (var i: u32 = 0u; i < RELIEF_STEPS; i++) {
        texture_depth = sample_depth(uv, slot);
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
    let slot = i32(entry.x);
    let depth = entry.y;

    let TBN = calculate_tbn_mikktspace(pbr_input.world_normal, in.world_tangent);
    let T = TBN[0];
    let B = TBN[1];
    let N = TBN[2];
    var uv = in.uv;
    // None at the low preset: its frame is its pixels' shading, and a step costs a fetch each.
    if depth > 0.0 && entry.w > 0.0 {
        let V = pbr_input.V;
        let Vt = vec3(dot(V, T), dot(V, B), dot(V, N));
        uv = parallaxed_uv(depth, entry.w, uv, -Vt, slot);
    }

    let bias = view.mip_bias;
    var colour = textureSampleBias(colour_array, array_sampler, uv, slot, bias);
    var arm = textureSampleBias(arm_array, array_sampler, uv, slot, bias);
    var normal = textureSampleBias(normal_array, array_sampler, uv, slot, bias).rg;

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
    let under = i32(layers.worn[layer / 4u][layer % 4u]) - 1;
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

    // Its colour on average, for what is behind it where a piece of it has gone.
    let mean = pbr_input.material.base_color.rgb
        * textureSampleLevel(colour_array, array_sampler, uv, slot, 8.0).rgb;
    pbr_input.material.base_color = vec4(pbr_input.material.base_color.rgb * colour.rgb, 1.0);
    pbr_input.material.perceptual_roughness = arm.g;

    // Damage (D305), by the structure of what breaks, one way a pattern: a wall's mortar
    // washes out of its joints, its stones' edges chip back to their fresh face and then
    // whole stones fall out, leaving their hollows; timber splits along its grain, chunks
    // gouge out of it and its sawn ends check from the rim; a covering's shingles crack and
    // then go missing, showing the dark under them; anything else by the shared crack and
    // chip mask. What a cube has lost spreads in patches across it (where the hits landed
    // are the cubes they landed on), so a damaged wall is broken here and there, more as it
    // loses more. Where none of it is drawn, darkened as far off.
    let lost = clamp(in.uv_b.y, 0.0, 1.0);
    let fw = fwidth(uv);
    var broken = 0.0;
    var crack_n = vec2(0.5, 0.5);
    if layers.damage.y > 0.5 {
        let mask = i32(layers.damage.x);
        let threshold = textureSampleBias(height_array, array_sampler, uv, mask, bias).r;
        let flake = textureSampleBias(colour_array, array_sampler, uv, mask, bias).r;
        let mask_n = textureSampleBias(normal_array, array_sampler, uv, mask, bias).rg;
        let base = pbr_input.material.base_color.rgb;
        let grey = vec3(dot(base, vec3(0.3, 0.59, 0.11)));
        let wp = in.world_position.xyz;
        let patchy = noise21(vec2(wp.x + wp.z * 0.71, wp.y * 1.3) * 1.6);
        let l = clamp(lost * (0.6 + 0.8 * patchy), 0.0, 1.0);
        let way = i32(layers.breaks[layer].x);
        var colour_out = base;
        var rough = arm.g;
        if way == 1 {
            // Laid stones: the joints first, then the stones' edges, then whole stones, each
            // leaving its hollow: the mortar bed and rubble behind it, in shade at its rim.
            let cell = cell_at(uv, slot);
            let given_here = given(uv, slot, l, 0.5, 0.5);
            let joint = given_here.x;
            // The mortar round a stone that has gone goes with it.
            let r = 3.0 / CELLS;
            var round_fall = 0.0;
            for (var k = 0; k < 4; k++) {
                let a = f32(k) * TAU / 4.0 + 0.4;
                round_fall = max(round_fall, given(uv + vec2(cos(a), sin(a)) * r, slot, l, 0.5, 0.5).y);
            }
            let fall = max(given_here.y, joint * round_fall);
            let erode = joint * smoothstep(0.02, 0.3, l);
            let edge = edge_of(uv, slot, cell, 2.0) * (1.0 - joint);
            let bite = noise21(uv * 34.0) * 0.6 + noise21(uv * 9.0) * 0.4;
            let chip = edge * smoothstep(bite - 0.05, bite, smoothstep(0.15, 0.8, l) * 0.7)
                * (1.0 - fall);
            let fresh = mix(base, grey, 0.4) * 1.25;
            // A hollow: the rubble and mortar bed behind, its top in the shadow of the stone
            // over it and its foot lit where the stone under it shows its top (a wall's
            // texture runs down its face).
            let rubble = noise21(uv * 30.0) * 0.6 + noise21(uv * 8.0) * 0.4;
            let d = 4.0 / CELLS;
            let over = select(0.0, 1.0, abs(cell_at(uv - vec2(0.0, d), slot) - cell) > 0.002);
            let under = select(0.0, 1.0, abs(cell_at(uv + vec2(0.0, d), slot) - cell) > 0.002);
            let rim = edge_of(uv, slot, cell, 3.0);
            let shade = (1.0 - 0.7 * over) * (1.0 + 0.6 * under * (1.0 - over)) * (1.0 - 0.35 * rim);
            // Behind it, the wall's core: more of its stones, set back in shadow, their
            // joints dark.
            let behind_uv = uv + vec2(0.37, 0.53);
            let behind = textureSampleGrad(colour_array, array_sampler, behind_uv, slot, ddx, ddy).rgb;
            let behind_joint = select(0.0, 1.0, cell_at(behind_uv, slot) < 0.01);
            let core = pbr_input.material.base_color.rgb / max(colour.rgb, vec3(0.01)) * behind;
            let hollow = core * (0.42 - 0.25 * behind_joint) * (0.75 + 0.35 * rubble) * shade;
            colour_out = mix(base, base * 0.3, erode);
            colour_out = mix(colour_out, fresh, chip);
            colour_out = mix(colour_out, hollow, fall);
            broken = max(max(erode * 0.6, chip), fall);
            crack_n = mix(mask_n, vec2(0.5, 0.5), fall);
            rough = mix(arm.g, 0.95, broken);
        } else if way == 2 {
            // Timber: splits along the grain, longer and wider as it loses more; chunks
            // gouged out, the fresh wood under them pale.
            let splits = max(layers.breaks[layer].y, 1.0);
            let a = uv.x * splits + 0.45 * noise21(vec2(uv.x * 3.0, uv.y * 1.1));
            let lane = floor(a);
            let dist = min(fract(a), 1.0 - fract(a));
            let reach = noise21(vec2(lane * 7.3 + 1.7, uv.y * 1.6));
            let open = smoothstep(1.0 - l * 1.4, 1.1 - l * 1.4, reach);
            let width = (0.02 + 0.1 * l) * open * (0.4 + 0.9 * noise21(vec2(lane * 3.7, uv.y * 6.0)));
            let aa = max(fw.x * splits, 0.002);
            let split = (1.0 - smoothstep(width, width + aa, dist)) * open;
            let g = noise21(uv * vec2(6.0, 2.2)) * 0.7 + noise21(uv * 23.0) * 0.3;
            let reach_g = 0.82 - 0.35 * smoothstep(0.35, 1.0, l);
            let gouge = smoothstep(reach_g, reach_g + 0.03, g);
            let rim = smoothstep(reach_g - 0.03, reach_g, g) * (1.0 - gouge);
            let fresh = mix(base * 1.45, vec3(0.62, 0.45, 0.28), 0.4);
            colour_out = mix(base, base * 0.55, rim);
            colour_out = mix(colour_out, fresh, gouge);
            let lip = (1.0 - smoothstep(width + aa, width * 2.2 + aa * 2.0, dist)) * open * (1.0 - split);
            colour_out = mix(colour_out, fresh, lip * 0.6);
            colour_out = mix(colour_out, base * 0.08, split);
            broken = max(split, gouge) * 0.5;
            crack_n = vec2(0.5, 0.5);
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
            // Shingles: one cracks across, then whole ones are gone, the dark under them.
            let cell = cell_at(uv, slot);
            let gap = select(0.0, 1.0, cell < 0.01);
            let missing = given(uv, slot, l, 0.3, 0.65).y;
            let cracked = (1.0 - gap) * step(0.08 + 0.55 * cell, l) * (1.0 - missing);
            let dir = vec2(cos(cell * 40.0), sin(cell * 40.0) * 0.3 + 0.9);
            let c = dot(uv * 9.0, normalize(dir)) + cell * 13.0;
            let dist = min(fract(c), 1.0 - fract(c));
            let line = (1.0 - smoothstep(0.03, 0.03 + max(fw.x, fw.y) * 9.0, dist)) * cracked;
            let under = mean * 0.06 + vec3(0.012, 0.01, 0.008);
            colour_out = mix(base, base * 0.2, line);
            colour_out = mix(colour_out, under, missing);
            broken = max(line, missing);
            crack_n = select(mask_n, vec2(0.5, 0.5), missing > 0.5);
            rough = mix(arm.g, 0.95, broken);
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
    var Nt = vec3(mix(normal, crack_n, broken * 0.85) * 2.0 - 1.0, 0.0);
    Nt.z = sqrt(max(1.0 - dot(Nt.xy, Nt.xy), 0.0));
    pbr_input.N = normalize(Nt.x * T + Nt.y * B + Nt.z * N);
    pbr_input.clearcoat_N = pbr_input.N;

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
