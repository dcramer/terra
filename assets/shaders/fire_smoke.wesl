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
    pbr_functions::main_pass_post_lighting_processing,
    mesh_view_bindings as view_bindings,
    mesh_view_types,
    pbr_lighting as lighting,
    clustered_forward as clustering,
    shadows,
};
@if(DEPTH_PREPASS && !PREPASS_PIPELINE)
import bevy_pbr::prepass::utils::prepass_depth;

@if(PREPASS_PIPELINE)
@group(0) @binding(1) var<uniform> globals: Globals;

// The wind, metres a second, and how many slots are filled; which smoke (0 over blazes, 1 a
// breath's soot) and the time its puffs were let go by; the open sky's light from each side of
// the sky's frame (+X, -X, +Y, -Y, +Z, -Z) and that frame's axes in the world; each smoke set's
// light scale. Must match FireSmokeUniform (crates/terra_world/src/fire/smoke.rs).
struct Smoke {
    wind: vec4<f32>,
    kind: vec4<f32>,
    sky: array<vec4<f32>, 6>,
    sky_frame: array<vec4<f32>, 3>,
    sets: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> smoke: Smoke;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var<storage, read> slots: array<vec4<f32>>;
// What breaths have let go, the soot first: where and when, then its going and its luck.
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var<storage, read> puffs: array<vec4<f32>>;
// The six-way lightmaps of the baked puffs (tools/fire/bake/smoke.ts), a layer a set in
// books.rs's SMOKE_SETS order: map 0 R right, G top, B back, A alpha; map 1 R left, G bottom,
// B front, A glow.
@if(!PREPASS_PIPELINE)
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var near_maps: texture_2d_array<f32>;
@if(!PREPASS_PIPELINE)
@group(constants::MATERIAL_BIND_GROUP) @binding(104)
var maps_sampler: sampler;
@if(!PREPASS_PIPELINE)
@group(constants::MATERIAL_BIND_GROUP) @binding(105)
var far_maps: texture_2d_array<f32>;
const SMOKE_CAMPFIRE: i32 = 0;
const SMOKE_HOUSE: i32 = 1;
// Which of a baked puff's 64 frames a card plays over its life: born a little open, and done
// before the puff outgrows its frame (by frame 35 both sets' reach the frame's edge; the card
// grows itself as it climbs). And how thin a hearth's wisp is against its baked puff.
const PUFF_FROM: f32 = 4.0;
const PUFF_TO: f32 = 34.0;
const HEARTH_THIN: f32 = 0.14;
// A hearth's baked puff's alpha opened out (to this power) and mixed so far with a soft round
// fall-off over its card, so each puff's edge reaches the next ones' and the wisp is one
// thread: the bake's own alpha is a small ball in its card, and the puffs read as beads with
// sky between them although the cards overlap (fire round 22). Higher up it frays: the soft
// fall-off gives way to the bake's own broken alpha, opened less (to 1.25), so the thread breaks
// into wisps as it climbs, thinner (0.32 against 0.4), narrower and fading sooner (fire round
// 24: blind 5.5/10 against 5, the old column a dense tan tube to the top of the frame).
const HEARTH_OPEN: f32 = 0.6;
const HEARTH_SOFT: f32 = 0.7;
// How many times a hearth's wisp gives back the sun and sky its single-scattering bake does.
const HEARTH_SCATTER: f32 = 6.0;
const PI: f32 = 3.14159265;
// How long a breath's soot hangs, in seconds.
const SOOT_S: f32 = 5.5;
// How far a breath's soot puff's edge is softened toward a round falloff.
const SOOT_SOFT: f32 = 0.6;
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
        // Born a little wider, so its first puffs merge into one rolling cloud: at 1.4 m they
        // stood as separate black balls along the stream (castle fall r31's review, 5.75-7.5 s).
        let size = mix(2.2, 7.5, sqrt(t)) * (0.6 + 0.8 * luck);
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
    let is_hearth = slot < min(u32(smoke.kind.w + 0.5), HEARTHS);
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
        // A haze, not a ribbon (campfire notes #5): its puffs live three seconds, so it has
        // thinned to nothing a metre or two over the flames, as the photographs' does.
        let life_s = 2.8 + 0.6 * fract(seed * 3.3);
        let t = fract(globals.time / life_s + j + seed * 0.37);
        let seconds = t * life_s;
        let rise = (0.8 + 0.5 * heat) * seconds * (1.0 - 0.35 * t);
        let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
        let north = cross(east, up);
        let sway = (vec2(smooth_noise(vec2(seed * 13.0, seconds * 0.35 - globals.time * 0.1)), smooth_noise(vec2(seed * 29.0 + 7.0, seconds * 0.35 - globals.time * 0.1))) - 0.5) * (0.3 + 1.8 * t);
        let middle = foot + up * (tall * 0.55 + rise) + east * sway.x + north * sway.y + wind.xyz * seconds * (0.25 + 0.45 * t);
        // Wide and soft: born as wide as the flames' tips and opening to twice that.
        let size = mix(0.35 + tall * 0.25, 1.0 + tall * 0.5, sqrt(t)) * (0.9 + 0.2 * luck);
        let to_eye = normalize(view.world_position - middle);
        var right = cross(up, to_eye);
        if dot(right, right) < 1.0e-6 {
            right = cross(vec3(1.0, 0.0, 0.0), to_eye);
        }
        right = normalize(right);
        let card_up = cross(to_eye, right);
        let spin = (luck - 0.5) * 0.25 + seconds * (luck - 0.5) * 0.06;
        // Drawn out along its rise, a wisp more than a ball.
        let corner = (vertex.uv - vec2(0.5)) * vec2(size, size * 1.3);
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

// A frame of a baked puff's maps, both maps, at a card's uv: frame `f` of the set's 8x8.
@if(!PREPASS_PIPELINE)
fn frame_maps(layer: i32, f: f32, uv: vec2<f32>) -> array<vec4<f32>, 2> {
    let cell = vec2(f % 8.0, floor(f / 8.0));
    // Kept half a texel in from the frame's edge so a neighbour never bleeds in.
    let inside = clamp(vec2(uv.x, 1.0 - uv.y), vec2(0.5 / 128.0), vec2(1.0 - 0.5 / 128.0));
    let at = (cell + inside) / 8.0;
    return array(
        textureSampleLevel(near_maps, maps_sampler, at, layer, 0.0),
        textureSampleLevel(far_maps, maps_sampler, at, layer, 0.0),
    );
}

// The baked response to light from `l`, in the card's frame (right, up the card, toward the
// eye; unit length): each axis's map by the side the light is on, weighted by l squared, in a
// white Lambert surface's units (tools/fire/bake/smoke.ts).
@if(!PREPASS_PIPELINE)
fn response(l: vec3<f32>, near: vec4<f32>, far: vec4<f32>) -> f32 {
    let w = l * l;
    let x = select(far.r, near.r, l.x > 0.0);
    let y = select(far.g, near.g, l.y > 0.0);
    // Map 0's B is lit from behind, map 1's from the eye's side.
    let z = select(near.b, far.b, l.z > 0.0);
    return w.x * x + w.y * y + w.z * z;
}

// The open sky's light arriving from world direction `d` (toward where it comes from), from
// its six sides as skylight.rs's OpenSky gives them, already times the environment's brightness.
@if(!PREPASS_PIPELINE)
fn sky_from(d: vec3<f32>) -> vec3<f32> {
    let s = vec3(dot(d, smoke.sky_frame[0].xyz), dot(d, smoke.sky_frame[1].xyz), dot(d, smoke.sky_frame[2].xyz));
    let w = s * s;
    return w.x * select(smoke.sky[1].rgb, smoke.sky[0].rgb, s.x > 0.0)
        + w.y * select(smoke.sky[3].rgb, smoke.sky[2].rgb, s.y > 0.0)
        + w.z * select(smoke.sky[5].rgb, smoke.sky[4].rgb, s.z > 0.0);
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let hearth = in.uv_b.x > 1.5;
    let t = in.uv_b.x - select(0.0, 2.0, hearth);
    let luck = fract(in.uv_b.y);
    let heat = floor(in.uv_b.y * 0.5) / 15.0;
    let soot = heat > 1.5;
    let P = in.world_position.xyz;
    // The card's own frame from how its position runs with its uv, before anything is
    // discarded (derivatives need every pixel of a quad): right, up the card, toward the eye.
    // So a puff turned or mirrored is lit from where its light really is.
    let dp_x = dpdx(P);
    let dp_y = dpdy(P);
    let du_x = dpdx(in.uv);
    let du_y = dpdy(in.uv);
    let det = du_x.x * du_y.y - du_x.y * du_y.x;
    let flip = select(-1.0, 1.0, det >= 0.0);
    let T = normalize((dp_x * du_y.y - dp_y * du_x.y) * flip + vec3(1.0e-7));
    let B = normalize((dp_y * du_x.x - dp_x * du_y.x) * flip + vec3(1.0e-7));
    let F = normalize(view.world_position - P);
    // Which baked puff, and where in its life: a hearth's thin wisp, or a blaze's (and a
    // breath's) thick soot; played from PUFF_FROM to PUFF_TO, the frames crossfaded.
    let book = select(SMOKE_HOUSE, SMOKE_CAMPFIRE, hearth);
    let f = mix(PUFF_FROM, PUFF_TO, t);
    let a = frame_maps(book, floor(f), in.uv);
    let b = frame_maps(book, floor(f) + 1.0, in.uv);
    let k = fract(f);
    let near = mix(a[0], b[0], k);
    let far = mix(a[1], b[1], k);
    // Its life: a blaze's puff in quickly and thinning as it opens; a hearth's clear of its
    // flames before it shows (born over them it lay a pale smear across them, lake film r6)
    // and thinning to nothing as it climbs, so its top fades into the air rather than ending.
    var life = smoothstep(0.0, 0.06, t) * pow(1.0 - t, 1.3) * select(mix(1.0, 0.5, t), mix(1.0, 0.65, t), soot);
    // A breath's soot thickens only once it has risen off the flame, and never as a wall:
    // at once and whole it stood black in front of the flame (breath r28's review).
    if soot {
        life = smoothstep(0.03, 0.2, t) * pow(1.0 - t, 1.3) * mix(0.6, 0.4, t);
    }
    if hearth {
        life = smoothstep(0.012, 0.07, t) * pow(1.0 - t, 2.4) * HEARTH_THIN;
    }
    let p = abs(in.uv - vec2(0.5));
    let window = 1.0 - smoothstep(0.42, 0.5, max(p.x, p.y));
    var cover = near.a;
    if hearth {
        let r = (in.uv - vec2(0.5)) * 2.0;
        let wisp = smoothstep(0.08, 0.55, t);
        cover = mix(pow(near.a, mix(HEARTH_OPEN, 1.25, wisp)), exp(-3.0 * dot(r, r)), HEARTH_SOFT * (1.0 - wisp));
    }
    if soot {
        // Its puff's baked edge softened toward a round falloff, so overlapping puffs sum to
        // a cloud rather than to stacked balls each with its own rim (castle fall r32).
        let r = (in.uv - vec2(0.5)) * 2.0;
        cover = mix(cover, cover * exp(-1.5 * dot(r, r)), SOOT_SOFT);
    }
    var alpha = cover * life * window;
    // Faded in over its last metre and a half in front of what is behind it.
@if(DEPTH_PREPASS)
    {
        let behind = depth_ndc_to_view_z(in.position.z) - depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        alpha *= smoothstep(0.0, 1.5, behind);
    }
    if alpha < 0.004 {
        discard;
    }
    let scale = select(smoke.sets.y, smoke.sets.x, hearth);
    let near_m = vec4(near.rgb / scale, near.a);
    let far_m = vec4(far.rgb / scale, far.a);
    // Young smoke from a big timber fire near black (soot's albedo 0.03 to 0.05,
    // research/2026-10-16-real-time-fire-rendering.md), greying as it climbs and thins; a
    // breath's is fuel burnt rich, black and greying only a little; a hearth's dry wood burns
    // clean, a mid grey with a little blue from its fine ash, never the pale grey that the fire's
    // light turned salmon.
    var albedo = mix(vec3(0.03, 0.028, 0.026), vec3(0.15, 0.145, 0.14), smoothstep(0.2, 0.9, t));
    if soot {
        // Warm, brown going black: a neutral grey by the fire read olive at dusk (breath r30/a).
        albedo = mix(vec3(0.03, 0.019, 0.012), vec3(0.1, 0.07, 0.05), smoothstep(0.2, 1.0, t));
    }
    // By the sun and the sky a hearth's smoke is wood smoke's pale blue-grey (its fine tar and ash
    // scatter nearly all they take, single-scattering albedo near 0.9): at 0.14 to 0.42 the
    // column by day read as dark beads across the sky (fire round 20). Its fire's light reaches
    // only its young, sooty underside, so takes the grey, never the pale that firelight turned
    // salmon.
    var albedo_sky = albedo;
    // And what its wisp gives back of the sun and sky past single scattering: the bake is single
    // scattering, which leaves a wisp of a near-white medium as dark as soot, where its light
    // scattered again and again is several times as much (Wrenninge 2013's octaves, Hillaire
    // 2016's Frostbite clouds). Measured: at single scattering its light was a quarter of the
    // sky behind it, so it read as dark beads (fire round 21). By day only, from the camera's
    // EV100 (night's about 9, noon's 13 or more): by night our moon is drawn some stops brighter
    // than a real one, and the plume went milky lavender, brighter than the stones (round 21).
    var scatter = 1.0;
    if hearth {
        let ev100 = -log2(view.exposure * 1.2);
        scatter = mix(1.0, HEARTH_SCATTER, smoothstep(10.5, 12.5, ev100));
        albedo = mix(vec3(0.14, 0.14, 0.145), vec3(0.42, 0.42, 0.43), smoothstep(0.05, 0.5, t));
        // Grey-brown, not blue-grey (campfire notes #5: at dusk the column read as a blue shaft).
        albedo_sky = mix(vec3(0.5, 0.49, 0.47), vec3(0.8, 0.78, 0.75), smoothstep(0.05, 0.5, t));
    }
    var light = vec3(0.0);
    var fire_light = vec3(0.0);
    // The sun (and moon), through the atmosphere and the shadows, as Bevy lights a surface
    // facing it: directional_light gives colour times transmittance over pi for a white one.
    let view_z = dot(vec4(view.view_from_world[0].z, view.view_from_world[1].z, view.view_from_world[2].z, view.view_from_world[3].z), vec4(P, 1.0));
    for (var i: u32 = 0u; i < view_bindings::lights.n_directional_lights; i = i + 1u) {
        let L = view_bindings::lights.directional_lights[i].direction_to_light.xyz;
        var li: lighting::LightingInput;
        li.layers[0].N = L;
        li.layers[0].R = L;
        li.layers[0].NdotV = 1.0;
        li.layers[0].perceptual_roughness = 1.0;
        li.layers[0].roughness = 1.0;
        li.P = P;
        li.V = L;
        li.diffuse_color = vec3(1.0);
        var shadow = 1.0;
        if (view_bindings::lights.directional_lights[i].flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            shadow = shadows::fetch_directional_shadow(i, vec4(P, 1.0), F, view_z, in.position.xy);
        }
        let m = response(vec3(dot(L, T), dot(L, B), dot(L, F)), near_m, far_m);
        light += lighting::directional_light(i, &li, true) * shadow * m;
    }
    // The fires' lights from the clusters, as Bevy walks them, each held at its own radius as
    // the fog's are (scatter.rs): one under a young puff lights only its underside.
    let ortho = view.clip_from_view[3].w == 1.0;
    let cluster = clustering::view_fragment_cluster_index(in.position.xy, view_z, ortho);
    let ranges = clustering::unpack_clusterable_object_index_ranges(cluster);
    for (var i: u32 = ranges.first_point_light_index_offset; i < ranges.first_spot_light_index_offset; i = i + 1u) {
        let id = clustering::get_clusterable_object_id(i);
        let lamp = &view_bindings::clustered_lights.data[id];
        let to = (*lamp).position_radius.xyz - P;
        let d2 = dot(to, to);
        let r = (*lamp).position_radius.w;
        let e = (*lamp).color_inverse_square_range.rgb * lighting::getRangeFalloff(d2, (*lamp).color_inverse_square_range.w) / max(d2, max(r * r, 1.0e-4));
        let L = to * inverseSqrt(max(d2, 1.0e-8));
        // A hearth's light reaches only its smoke's first half metre (`t` 0.02-0.08 of its
        // five seconds): its fill hangs in the smoke, and lit the column orange to the top
        // (fire round 24); at 0.08-0.22 the night's column was still a tan-brown sock lit
        // evenly 1.5 m up, where the references' smoke is a faint warm haze over the flames
        // dying into the dark (fire round 27, blind 6/10 against 5).
        let warm_reach = select(1.0, 1.0 - smoothstep(0.02, 0.08, t), hearth);
        fire_light += warm_reach * e * (response(vec3(dot(L, T), dot(L, B), dot(L, F)), near_m, far_m) / PI);
    }
    // The open sky from all round: thin smoke at a quarter a side gives back the sky itself.
    var sky = (2.0 / 3.0) * (near_m.r * sky_from(T) + far_m.r * sky_from(-T)
        + near_m.g * sky_from(B) + far_m.g * sky_from(-B)
        + far_m.b * sky_from(F) + near_m.b * sky_from(-F));
    // A hearth's smoke takes the sky's brightness, not its blue: at dusk the open sky is most of
    // its light and deep blue, so its pale wisp gave it back as a blue shaft over the warm fire
    // (campfire notes #5), where the references' wood smoke stays a dull grey-brown haze.
    if hearth {
        sky = vec3(dot(sky, vec3(0.2126, 0.7152, 0.0722)));
    }
    light += sky;
    // Its hot core's own light, where it is young over the fire: dimly in a hearth's first half
    // metre, a blaze's column at its foot, a breath's soot its first moments.
    // A breath's soot glows only just off the flame: at the 14th power it still glowed orange
    // over the wall 1.5 s after the breath stopped (breath r30/b).
    var glow = select(pow(1.0 - t, 18.0) * heat, pow(1.0 - t, 30.0) * 1.5, soot) * 900.0;
    if hearth {
        glow = heat * (1.0 - smoothstep(0.0, 0.06, t)) * 36.0;
    }
    let lit = (albedo_sky * light * scatter + albedo * fire_light + vec3(1.0, 0.3, 0.06) * glow * far.a) * view.exposure;
    var input = pbr_input_from_standard_material(in, is_front);
    input.N = F;
    input.world_normal = F;
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(input, vec4(lit, alpha));
    out.color.a = alpha;
    return out;
}
