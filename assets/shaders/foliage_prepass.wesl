// The foliage's prepass fragment shader, for depth, normals, motion vectors and shadows:
// Bevy 0.20's standard one (bevy_pbr src/render/pbr_prepass.wesl, MIT OR Apache-2.0, by
// the Bevy contributors), without the meshlet path, with one change: the crossfade between
// levels of detail dithers in a pattern that moves every frame, as in
// foliage_fragment.wesl, so depth, shadows and colour agree on which pixels are drawn
// (D214).

import bevy_pbr::{
    render::{
        pbr_prepass_functions,
        pbr_bindings,
        pbr_types,
        pbr_functions,
        pbr_functions::SampleBias,
        mesh_bindings::mesh,
        mesh_functions,
        mesh_view_bindings::view,
    },
    prepass::io as prepass_io,
};
import bevy_render::{bindless::{bindless_samplers_filtering, bindless_textures_2d}, globals::Globals};

@if(BINDLESS)
import bevy_pbr::render::pbr_bindings::material_indices;

@group(0) @binding(1) var<uniform> globals: Globals;

// What the climate does to foliage (grown/mod.rs's FoliageClimate, D327): whether there are
// maps, how wide the card atlas's cells are, which kind's layer of the leaves map each cell's
// card is (-1 none), and each layer's autumn colour, linear. Where the maps lie is in the
// shared buffer (`parting.climate_from_world`).
struct FoliageClimate {
    on: u32,
    across: f32,
    layers: array<vec4<i32>, 4>,
    autumn: array<vec4<f32>, 8>,
}

struct Wind {
    direction: vec3<f32>,
    strength: f32,
    bark: f32,
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
@group(constants::MATERIAL_BIND_GROUP) @binding(106)
var climate_leaves: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(107)
var climate_leaves_sampler: sampler;

// Where a plant stands in its kind's year, 0 early to 1 late: foliage.wesl's `hash`, so its
// leaves fall and turn alike.
fn plant_share(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn climate_place(world: vec3<f32>) -> vec2<f32> {
    return clamp((parting.climate_from_world * vec4<f32>(world, 1.0)).xy, vec2<f32>(0.0), vec2<f32>(1.0));
}

// Which kind's layer of the leaves map a card is, by its atlas cell (foliage.wesl's).
fn card_layer(uv: vec2<f32>) -> i32 {
    let cell = vec2<i32>(floor(uv / wind.climate.across));
    if any(cell < vec2<i32>(0)) || any(cell > vec2<i32>(3)) {
        return -1;
    }
    let index = cell.y * 4 + cell.x;
    return wind.climate.layers[index / 4][index % 4];
}

// Smooth noise in clumps a hand or two across, stretched to about uniform from 0 to 1: where a
// plant's leaves have fallen, those left lie in rounded clumps, not cubes. The same in the main
// pass and the prepass.
fn clump_hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn clumps(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(clump_hash(i), clump_hash(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(clump_hash(i + vec3<f32>(0.0, 1.0, 0.0)), clump_hash(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(clump_hash(i + vec3<f32>(0.0, 0.0, 1.0)), clump_hash(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(clump_hash(i + vec3<f32>(0.0, 1.0, 1.0)), clump_hash(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    let n = mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
    return clamp((n - 0.2) / 0.6, 0.0, 1.0);
}

// Leaves its kind's year says have fallen, on a plant whose cards carry no rank to fall by, as
// a bush's or a fern's do not (foliage.wesl's `fallen` takes the crowns', and marks them
// `ranked`, its second texture coordinate's y far below nothing): gone in clumps a hand or
// two across (`clumps`), so it thins and stands bare (D327). The same in the
// main pass and the prepass, so depth and colour agree on what is left.
fn bare(uv: vec2<f32>, ranked: bool, world: vec3<f32>) {
    if wind.climate.on == 0u || wind.bark > 0.5 || ranked {
        return;
    }
    let layer = card_layer(uv);
    if layer < 0 {
        return;
    }
    // Taken where the leaf is, not where its plant stands, which a pass may not be told: a
    // bush's leaves agree, being near each other, and a patch of bushes a few metres across
    // turns as one plant.
    let year = textureSampleLevel(climate_leaves, climate_leaves_sampler, climate_place(world), layer, 0.0);
    let on = mix(year.r, year.g, plant_share(floor(world / 3.0) * 1.7 + 0.3));
    if on < 0.999 && clumps(world * 2.5 + 0.37) > on {
        discard;
    }
}

fn noise_at(frag_coord: vec4<f32>) -> f32 {
    let shifted = frag_coord.xy + 5.588238 * f32(globals.frame_count % 64u);
    return fract(52.9829189 * fract(dot(shifted, vec2<f32>(0.06711056, 0.00583715))));
}

// As foliage_fragment.wesl: what is right at the camera fades out. A shadow view stands far
// off, so shadows keep it.
fn near_fade(frag_coord: vec4<f32>, world: vec3<f32>) {
    let near = smoothstep(0.3, 0.9, distance(world, view.world_position));
    if near < 1.0 && noise_at(frag_coord) > near {
        discard;
    }
}

// As foliage.wesl's: the camera's eye and the figure it follows (D271).
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

// As foliage_fragment.wesl: the third-person camera's view kept clear, a plant in the way
// fading as a whole and leaves at the lens (D271, D274). Only in the main camera's own view,
// so shadows keep them.
fn clear_view(frag_coord: vec4<f32>, world: vec3<f32>, instance: u32, leaf: bool) {
    let eye = parting.eye;
    if eye.w < 0.5 || distance(view.world_position, eye.xyz) > 0.01 {
        return;
    }
    var keep = 1.0 - f32(mesh[instance].tag & 0xffu) / 255.0;
    if leaf {
        let lens = min(LENS_M, distance(parting.subject.xyz, eye.xyz) * 0.6);
        keep = min(keep, smoothstep(lens - LENS_BAND_M, lens, distance(world, eye.xyz)));
    } else {
        // Bark the lens slides past, as it does round a trunk, dithers out within a stride
        // of it rather than sweeping across the screen as a smear TAA cannot resolve (D289).
        keep = min(keep, smoothstep(BARK_LENS_M - LENS_BAND_M, BARK_LENS_M, distance(world, eye.xyz)));
    }
    if keep < 1.0 && noise_at(frag_coord) > keep {
        discard;
    }
}

const LENS_M: f32 = 1.4;
const LENS_BAND_M: f32 = 0.35;
// Bark is whole from this far from a third-person lens (D289).
const BARK_LENS_M: f32 = 0.8;

// As foliage_fragment.wesl: a card fades out as it turns edge-on to the eye (D235). Not in
// the shadow pass, which keeps the whole card.
fn edge_fade(frag_coord: vec4<f32>, world: vec3<f32>) {
    let face = normalize(cross(dpdx(world), dpdy(world)));
    let towards = normalize(view.world_position - world);
    let keep = smoothstep(0.04, 0.16, abs(dot(face, towards)));
    if noise_at(frag_coord) > keep {
        discard;
    }
}

fn temporal_dither(frag_coord: vec4<f32>, dither: i32) {
    if dither == 0 {
        return;
    }
    if dither <= -16 || dither >= 16 {
        discard;
    }
    let threshold = i32(noise_at(frag_coord) * 16.0);
    if (dither >= 0 && dither + threshold >= 16) || (dither < 0 && 1 + dither + threshold <= 0) {
        discard;
    }
}

fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453);
}

// Smooth noise in the world, 0 to 1 (foliage_fragment.wesl's `wander`).
fn wander(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// A crown's solid shadow stand-in (`dapple` over 0, its offsets from its lumps' middles in the
// world in its colour) casts the shade a crown does from tens of metres up, not a disc: broken
// into gaps `dapple` metres across, as a pine's pads are tufts with sky between, and thinning
// to nothing toward its rim as the sun sees it, as the sun's disc, half a degree across, blurs
// a shade cast from 30 m into a penumbra a quarter of a metre wide and from 100 m nearly a
// metre (D359). What is part shade is a share of the shadow map's texels, a different share
// every frame, which the shadow's temporal filter and TAA average, as a masked material's
// dithered opacity shadows it in Unreal.
fn dapple(frag_coord: vec4<f32>, world: vec3<f32>, offset: vec3<f32>) {
    if wind.dapple <= 0.0 {
        return;
    }
    let toward_light = normalize(view.world_from_view[2].xyz);
    let facing = abs(dot(normalize(offset), toward_light));
    let p = world / wind.dapple;
    let gaps = 0.65 * wander(p) + 0.35 * wander(p * 2.3 + vec3<f32>(7.1, 3.3, 5.9));
    let shade = smoothstep(DAPPLE_GAPS.x, DAPPLE_GAPS.y, gaps) * smoothstep(DAPPLE_RIM.x, DAPPLE_RIM.y, facing);
    if noise_at(frag_coord) > shade {
        discard;
    }
}

// Where a stand-in's noise turns from gap to shade, and how far in from its rim, as the cosine
// of the sun's angle to its surface, its shade thins from whole to none.
const DAPPLE_GAPS: vec2<f32> = vec2<f32>(0.36, 0.6);
const DAPPLE_RIM: vec2<f32> = vec2<f32>(0.0, 0.75);

@if(PREPASS_FRAGMENT)
@fragment
fn fragment(in: prepass_io::VertexOutput, @builtin(front_facing) is_front: bool) -> prepass_io::FragmentOutput {
@if(BINDLESS)
    let slot = mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
@if(BINDLESS)
    let flags = pbr_bindings::material_array[material_indices[slot].material].flags;
@if(BINDLESS)
    let uv_transform = pbr_bindings::material_array[material_indices[slot].material].uv_transform;
@if(!BINDLESS)
    let flags = pbr_bindings::material.flags;
@if(!BINDLESS)
    let uv_transform = pbr_bindings::material.uv_transform;

@if(VISIBILITY_RANGE_DITHER)
    temporal_dither(in.position, in.visibility_range_dither);
    // A shadow pass emulating unclipped depth runs this variant.
@if(VERTEX_COLORS)
    dapple(in.position, in.world_position.xyz, in.color.rgb);
    near_fade(in.position, in.world_position.xyz);
    let leaf = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS) == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_MASK;
    if leaf {
        edge_fade(in.position, in.world_position.xyz);
    }
    clear_view(in.position, in.world_position.xyz, in.instance_index, leaf);
@if(VERTEX_UVS_A && VERTEX_UVS_B)
    bare(in.uv, in.uv_b.y < -500.0, in.world_position.xyz);
@if(VERTEX_UVS_A && !VERTEX_UVS_B)
    bare(in.uv, false, in.world_position.xyz);

    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);

    var out: prepass_io::FragmentOutput;

@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;

@if(NORMAL_PREPASS)
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
        let mesh_flags = mesh[in.instance_index].flags;
        let is_logically_front = pbr_functions::winding_corrected_front_facing(mesh_flags, is_front);
        let world_normal = pbr_functions::prepare_world_normal(in.world_normal, double_sided, is_logically_front);
        var normal = world_normal;

@if(VERTEX_UVS) {
@if(VERTEX_TANGENTS) {
// Not for a modelled trunk, which has no wind weights: its texture coordinates are not its
// bark's, which the main pass lays on from three sides (D264).
@if(STANDARD_MATERIAL_NORMAL_MAP && VERTEX_UVS_B) {
@if(STANDARD_MATERIAL_NORMAL_MAP_UV_B)
        let uv = (uv_transform * vec3(in.uv_b, 1.0)).xy;
@else
        let uv = (uv_transform * vec3(in.uv, 1.0)).xy;
        var bias: SampleBias;
        bias.mip_bias = view.mip_bias;
@if(BINDLESS)
        let texture = bindless_textures_2d[material_indices[slot].normal_map_texture];
@else
        let texture = pbr_bindings::normal_map_texture;
@if(BINDLESS)
        let sampler_ = bindless_samplers_filtering[material_indices[slot].normal_map_sampler];
@else
        let sampler_ = pbr_bindings::normal_map_sampler;
        let Nt = textureSampleBias(texture, sampler_, uv, bias.mip_bias).rgb;
        let TBN = pbr_functions::calculate_tbn_mikktspace(normal, in.world_tangent);
        normal = pbr_functions::apply_normal_mapping(flags, TBN, double_sided, is_logically_front, Nt);
}
}
}

        out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
    } else {
        out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
    }

@if(MOTION_VECTOR_PREPASS)
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);

    return out;
}
@else
@fragment
fn fragment(in: prepass_io::VertexOutput) {
@if(VISIBILITY_RANGE_DITHER)
    temporal_dither(in.position, in.visibility_range_dither);
@if(VERTEX_COLORS)
    dapple(in.position, in.world_position.xyz, in.color.rgb);
@if(VERTEX_UVS_A && VERTEX_UVS_B)
    bare(in.uv, in.uv_b.y < -500.0, in.world_position.xyz);
@if(VERTEX_UVS_A && !VERTEX_UVS_B)
    bare(in.uv, false, in.world_position.xyz);
    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);
}
