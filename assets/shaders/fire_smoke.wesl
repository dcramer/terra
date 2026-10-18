// The smoke over a fire (D377, crates/terra_world/src/fire.rs): dark soft cards rising from
// each blaze's slot of the fire's buffer, as a chimney's smoke rises (smoke.wesl, D325), but
// thick and dark as burning timber's is, rolling up in a column that swells as it climbs and
// leans downwind, lit by the sun and the sky as anything is, and glowing from under where the
// fire is, near its foot. The mesh's corners are all at the origin: the first texture
// coordinate the corner, the second the slot and the card's place among the slot's.

import bevy_pbr::render::{
    mesh_view_bindings::view,
    view_transformations::{position_world_to_clip, depth_ndc_to_view_z},
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, VertexOutput, decompress_vertex};
@if(PREPASS_PIPELINE)
import bevy_render::globals::Globals;
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::{
    forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    mesh_view_bindings::globals,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};
@if(DEPTH_PREPASS && !PREPASS_PIPELINE)
import bevy_pbr::prepass::utils::prepass_depth;

@if(PREPASS_PIPELINE)
@group(0) @binding(1) var<uniform> globals: Globals;

// The wind, metres a second, and how many slots are filled; which smoke (0 over blazes, 1 a
// breath's soot) and the time its puffs were let go by. Must match FireSmokeUniform.
struct Smoke {
    wind: vec4<f32>,
    kind: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> smoke: Smoke;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var<storage, read> slots: array<vec4<f32>>;
// What breaths have let go, the soot first: where and when, then its going and its luck.
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var<storage, read> puffs: array<vec4<f32>>;
// How long a breath's soot hangs, in seconds.
const SOOT_S: f32 = 5.5;
// How many kept fires are drawn as hearths (crates/terra_world/src/fire.rs HEARTHS).
const HEARTHS: u32 = 16u;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn smooth_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

// A puff's normal at one of its corners, `turned` from its middle in a puff `size` across.
fn puffed(right: vec3<f32>, card_up: vec3<f32>, to_eye: vec3<f32>, turned: vec2<f32>, size: f32) -> vec3<f32> {
    let out = turned / max(size * 0.5, 1.0e-3);
    return right * out.x + card_up * out.y + to_eye * 0.7;
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let slot = u32(vertex.uv_b.x + 0.5);
    let phase = vertex.uv_b.y;
    let wind = smoke.wind;
    if smoke.kind.x > 0.5 {
        // A breath's soot (research/2026-10-18-dragon-breath.md): black smoke rolling off
        // the stream where it was let go, carried on a little by it, then rising as hot
        // smoke does and swelling, leaning downwind, hanging a while after the breath stops.
        let born = puffs[slot * 2u];
        let going = puffs[slot * 2u + 1u];
        let s = smoke.kind.y - born.w;
        if s < 0.0 || s > SOOT_S {
            out.position = vec4(0.0, 0.0, -1.0, 1.0);
            return out;
        }
        let luck = going.w;
        let t = s / SOOT_S;
        let up = normalize(born.xyz);
        let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
        let north = cross(east, up);
        let roll = luck * 6.283 + s * (0.5 + 0.6 * luck);
        let billow = (east * cos(roll) + north * sin(roll)) * (0.4 + 1.2 * t);
        let middle = born.xyz + going.xyz * (1.0 - exp(-s * 1.4)) / 1.4 + up * (1.2 * s + 0.25 * s * s) + billow + wind.xyz * s * 0.8;
        let size = mix(1.4, 7.5, sqrt(t)) * (0.6 + 0.8 * luck);
        let to_eye = normalize(view.world_position - middle);
        var right = cross(up, to_eye);
        if dot(right, right) < 1.0e-6 {
            right = cross(vec3(1.0, 0.0, 0.0), to_eye);
        }
        right = normalize(right);
        let card_up = cross(to_eye, right);
        let spin = luck * 6.283 + s * (luck - 0.5) * 0.4;
        let corner = (vertex.uv - vec2(0.5)) * size;
        let turned = vec2(corner.x * cos(spin) - corner.y * sin(spin), corner.x * sin(spin) + corner.y * cos(spin));
        let world = middle + right * turned.x + card_up * turned.y;
        out.position = position_world_to_clip(world);
        out.world_position = vec4(world, 1.0);
        out.uv = vertex.uv;
        // Its heat past any blaze's, so the fragment knows a breath's soot.
        out.uv_b = vec2(t, luck + 64.0);
@if(PREPASS_PIPELINE) {
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS)
        out.world_normal = to_eye;
@if(MOTION_VECTOR_PREPASS)
        out.previous_world_position = out.world_position;
} @else {
        out.world_normal = puffed(right, card_up, to_eye, turned, size);
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
        out.instance_index = vertex_in.instance_index;
        return out;
    }
    let count = u32(wind.w + 0.5);
    let a = slots[slot * 2u];
    let b = slots[slot * 2u + 1u];
    let tall = a.w;
    // A kept fire's smoke is a hearth's, of cards of its own.
    let is_hearth = b.w > 0.5 && slot < HEARTHS;
    if slot >= count || tall <= 0.0 || (phase >= 1.0) != is_hearth {
        out.position = vec4(0.0, 0.0, -1.0, 1.0);
        return out;
    }
    let foot = a.xyz;
    let heat = b.x;
    let seed = b.y;
    let up = normalize(foot);
    if is_hearth {
        // A campfire's smoke, as in dawn footage of one and in the games that draw one well:
        // dry wood burning clean gives a thin pale wisp, not a building's black column; its
        // puffs let go close behind one another off the flames' tips, so they rise as one
        // thread, slow, swaying a little low down and more as they climb and spread, bent by
        // the air. Every puff of a hearth lives as long, so they stay evenly spaced.
        let j = phase - 1.0;
        let luck = fract(seed * 0.618 + j * 7.37);
        // Gone four or five metres up: lived nine seconds its puffs climbed past eight
        // metres, swaying apart, and hung among the trunks as patches of fog (lake film r5).
        let life_s = 4.5 + 1.0 * fract(seed * 3.3);
        let t = fract(globals.time / life_s + j + seed * 0.37);
        let seconds = t * life_s;
        let rise = (0.8 + 0.5 * heat) * seconds * (1.0 - 0.35 * t);
        let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
        let north = cross(east, up);
        let sway = (vec2(smooth_noise(vec2(seed * 13.0, seconds * 0.35 - globals.time * 0.1)), smooth_noise(vec2(seed * 29.0 + 7.0, seconds * 0.35 - globals.time * 0.1))) - 0.5) * (0.15 + 1.2 * t);
        let middle = foot + up * (tall * 0.7 + rise) + east * sway.x + north * sway.y + wind.xyz * seconds * (0.25 + 0.45 * t);
        let size = mix(0.15 + tall * 0.12, 0.9 + tall * 0.4, sqrt(t)) * (0.75 + 0.5 * luck);
        let to_eye = normalize(view.world_position - middle);
        var right = cross(up, to_eye);
        if dot(right, right) < 1.0e-6 {
            right = cross(vec3(1.0, 0.0, 0.0), to_eye);
        }
        right = normalize(right);
        let card_up = cross(to_eye, right);
        let spin = (luck - 0.5) * 0.6 + seconds * (luck - 0.5) * 0.15;
        // Drawn out along its rise, a wisp more than a ball.
        let corner = (vertex.uv - vec2(0.5)) * vec2(size, size * 1.7);
        let turned = vec2(corner.x * cos(spin) - corner.y * sin(spin), corner.x * sin(spin) + corner.y * cos(spin));
        let world = middle + right * turned.x + card_up * turned.y;
        out.position = position_world_to_clip(world);
        out.world_position = vec4(world, 1.0);
        out.uv = vertex.uv;
        // Its age past 2, so the fragment knows a hearth's.
        out.uv_b = vec2(t + 2.0, luck + floor(heat * 15.0 + 0.5) * 2.0);
@if(PREPASS_PIPELINE) {
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS)
        out.world_normal = to_eye;
@if(MOTION_VECTOR_PREPASS)
        out.previous_world_position = out.world_position;
} @else {
        // Thin smoke is lit through, not as a ball: its normal mostly toward the eye.
        out.world_normal = normalize(mix(to_eye, puffed(right, card_up, to_eye, turned, size), 0.35));
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
        out.instance_index = vertex_in.instance_index;
        return out;
    }
    let luck = fract(seed * 0.618 + phase * 2.37);
    // Longer lived and taller from a bigger fire.
    let life_s = 9.0 + 6.0 * luck;
    let t = fract(globals.time / life_s + phase + seed * 0.37);
    let seconds = t * life_s;
    let rise = (1.6 + 1.6 * heat + 0.25 * tall) * seconds * (1.0 - 0.3 * t);
    let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
    let north = cross(east, up);
    let wander = (vec2(smooth_noise(vec2(luck * 13.0, seconds * 0.3)), smooth_noise(vec2(luck * 29.0 + 7.0, seconds * 0.3))) - 0.5) * (1.0 + 7.0 * t);
    // Rolling as it climbs: each puff turned about the column as it goes.
    let roll = luck * 6.283 + seconds * (0.25 + 0.3 * luck);
    let billow = (east * cos(roll) + north * sin(roll)) * (0.5 + 1.5 * t) * (0.5 + tall * 0.3);
    let middle = foot + up * (rise + tall * 0.8) + east * wander.x + north * wander.y + billow + wind.xyz * seconds * (0.3 + 0.7 * t);
    let size = mix(0.8 + tall * 0.9, 5.0 + tall * 3.0, sqrt(t)) * (0.6 + 0.8 * luck);
    let to_eye = normalize(view.world_position - middle);
    var right = cross(up, to_eye);
    if dot(right, right) < 1.0e-6 {
        right = cross(vec3(1.0, 0.0, 0.0), to_eye);
    }
    right = normalize(right);
    let card_up = cross(to_eye, right);
    let spin = luck * 6.283 + seconds * (luck - 0.5) * 0.25;
    let corner = (vertex.uv - vec2(0.5)) * size;
    let turned = vec2(corner.x * cos(spin) - corner.y * sin(spin), corner.x * sin(spin) + corner.y * cos(spin));
    let world = middle + right * turned.x + card_up * turned.y;
    out.position = position_world_to_clip(world);
    out.world_position = vec4(world, 1.0);
    out.uv = vertex.uv;
    out.uv_b = vec2(t, luck + floor(heat * 15.0 + 0.5) * 2.0);
@if(PREPASS_PIPELINE) {
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS)
    out.world_normal = to_eye;
@if(MOTION_VECTOR_PREPASS)
    out.previous_world_position = out.world_position;
} @else {
    // Each puff lit as a ball, its normal at a corner leaning out toward that corner, so its
    // sunward side is lit and its far side and underside dark, as a billow is: lit flat
    // toward the eye, a column of puffs was one even tan.
    out.world_normal = puffed(right, card_up, to_eye, turned, size);
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
    return out;
}

fn density(uv: vec2<f32>, t: f32, luck: f32) -> f32 {
    let p = uv - vec2(0.5);
    let n = smooth_noise(uv * 3.5 + vec2(luck * 41.0, luck * 17.0));
    let fine = smooth_noise(uv * 9.0 + vec2(luck * 7.0, luck * 63.0));
    // Its rim cauliflowered by the finer noise, so a column's edge billows, not blurs.
    let round = 1.0 - smoothstep(0.2, 0.44, length(p) + (n - 0.5) * 0.3 + (fine - 0.5) * 0.16);
    let window = 1.0 - smoothstep(0.36, 0.5, max(abs(p.x), abs(p.y)));
    let life = smoothstep(0.0, 0.06, t) * pow(1.0 - t, 1.3);
    return round * window * life * (0.7 + 0.6 * n);
}

@if(PREPASS_PIPELINE && PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) -> bevy_pbr::prepass::io::FragmentOutput {
    discard;
}

@if(PREPASS_PIPELINE && !PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) {
    discard;
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let hearth = in.uv_b.x > 1.5;
    let t = in.uv_b.x - select(0.0, 2.0, hearth);
    let luck = fract(in.uv_b.y);
    let heat = floor(in.uv_b.y * 0.5) / 15.0;
    let soot = heat > 1.5;
    var alpha = density(in.uv, t, luck) * select(mix(0.95, 0.4, t), mix(0.97, 0.55, t), soot);
    if hearth {
        // A wisp: its rim eaten away by finer noise turning as it rises, thin, and thinner
        // as it spreads.
        let p = (in.uv - vec2(0.5)) * vec2(1.0, 0.85);
        let n = smooth_noise(in.uv * vec2(3.0, 2.0) + vec2(luck * 41.0, -t * 3.0));
        let fine = smooth_noise(in.uv * vec2(8.0, 5.0) + vec2(luck * 7.0, -t * 6.0));
        let wisp = 1.0 - smoothstep(0.05, 0.42, length(p) + (n - 0.5) * 0.4 + (fine - 0.5) * 0.25);
        // Thinning out as it climbs, so its top fades into the air rather than ending.
        // Clear of its flames before it shows: born over them it lay a pale smear across
        // them and the troll sat by them (lake film r6's first still).
        let life = smoothstep(0.03, 0.14, t) * pow(1.0 - t, 2.2);
        alpha = wisp * life * (0.5 + 0.7 * fine) * mix(0.3, 0.12, t);
    }
    // Faded in over its last metre and a half in front of what is behind it.
@if(DEPTH_PREPASS)
    {
        let behind = depth_ndc_to_view_z(in.position.z) - depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        alpha *= smoothstep(0.0, 1.5, behind);
    }
    if alpha < 0.004 {
        discard;
    }
    var input = pbr_input_from_standard_material(in, is_front);
    // Burning timber's smoke: a dark brown-grey, thick and darker in its middle.
    let shade = mix(1.0, 0.55, alpha);
    // Young smoke from a big timber fire near black (soot's albedo 0.03 to 0.05,
    // research/2026-10-16-real-time-fire-rendering.md), thinning to a grey as it climbs, never
    // the tan its old 0.25 albedo turned in the morning sun with the fire's glow on it.
    var grey = mix(vec3(0.03, 0.028, 0.026), vec3(0.15, 0.145, 0.14), smoothstep(0.2, 0.9, t));
    // A breath's soot is fuel burnt rich: black, and greying only a little as it thins.
    if soot {
        grey = mix(vec3(0.025, 0.022, 0.02), vec3(0.12, 0.11, 0.1), smoothstep(0.2, 1.0, t));
    }
    // Dry wood burning clean: a pale grey with a little blue from its fine ash, scattering
    // the sky's light (research/2026-10-16-real-time-fire-rendering.md).
    var dark = shade;
    if hearth {
        grey = mix(vec3(0.34, 0.36, 0.4), vec3(0.46, 0.48, 0.52), smoothstep(0.1, 0.7, t));
        dark = 1.0;
    }
    input.material.base_color = vec4(grey * dark, alpha);
    // Lit from under by the fire just over the flames; a breath's soot glows from the stream it
    // leaves for its first moments. Most on a puff's underside, which faces the flames, and
    // little on the rest: glowing up to a third of the way up, a column's base was a brown fog
    // (castle film r7, sRGB 104,75,55) where young smoke over a fire is black.
    let n = normalize(in.world_normal);
    let down = saturate(-dot(n, normalize(in.world_position.xyz)));
    var under = select(pow(1.0 - t, 24.0) * heat, pow(1.0 - t, 18.0) * 1.5, soot) * (0.15 + 1.5 * down);
    // A hearth's wisp warm only its first half metre over the flames, and dimly.
    if hearth {
        under = heat * (1.0 - smoothstep(0.0, 0.06, t)) * 0.04;
    }
    input.material.emissive = vec4(vec3(1.0, 0.3, 0.06) * under * 900.0, 1.0);
    input.N = n;
    input.world_normal = n;
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(input);
    out.color = main_pass_post_lighting_processing(input, out.color);
    out.color.a = alpha;
    return out;
}
