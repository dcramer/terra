// The construction materials' prepass fragment shader, for depth, normals, motion vectors
// and shadows (D290, D305): Bevy 0.20's standard one (bevy_pbr src/render/pbr_prepass.wesl,
// MIT OR Apache-2.0, by the Bevy contributors) cut to what the texture arrays' surfaces
// need, with one addition: a covering's shingles that damage has taken are not there, so
// what is under them (its battens and rafters) shows and casts its shadows through the
// holes. What decides which are gone is build.wesl's, kept the same here; the main pass
// discards the same pixels.

import bevy_pbr::{
    render::{
        pbr_prepass_functions,
        pbr_functions,
        mesh_bindings::mesh,
    },
    prepass::io as prepass_io,
};

// As build.wesl's Layers; only `breaks` is read here.
struct Layers {
    table: array<vec4<f32>, 512>,
    damage: vec4<f32>,
    worn: array<vec4<f32>, 128>,
    breaks: array<vec4<f32>, 512>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> layers: Layers;
@group(constants::MATERIAL_BIND_GROUP) @binding(106)
var cells_array: texture_2d_array<f32>;

const CELLS: f32 = 512.0;

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

// How what a cube has lost changes across the texture here; none without the places.
fn slope(in: prepass_io::VertexOutput) -> vec2<f32> {
    var out = vec2(0.0);
@if(VERTEX_UVS_A && VERTEX_UVS_B) {
    out = slope_of(clamp(in.uv_b.y, 0.0, 1.0), in.uv);
}
    return out;
}

// Whether a covering's shingle is gone here, as build.wesl's shingles decide.
fn gone(in: prepass_io::VertexOutput, slope: vec2<f32>) -> bool {
    var out = false;
@if(VERTEX_UVS_A && VERTEX_UVS_B) {
    let layer = min(u32(round(in.uv_b.x)), 511u);
    let lost = clamp(in.uv_b.y, 0.0, 1.0);
    if lost > 0.001 && i32(layers.breaks[layer].x) == 4 && layers.damage.y > 0.5 && layers.table[layer].x >= 0.0 {
        let slot = i32(layers.table[layer].x);
        let hit = lost;
        let shingle = given(in.uv, slot, hit, slope, 0.3, 0.65);
        out = shingle.y + shingle.x * step(0.9, hit) > 0.5;
    }
}
    return out;
}

@if(PREPASS_FRAGMENT)
@fragment
fn fragment(in: prepass_io::VertexOutput, @builtin(front_facing) is_front: bool) -> prepass_io::FragmentOutput {
    if gone(in, slope(in)) {
        discard;
    }
    var out: prepass_io::FragmentOutput;
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;
@if(NORMAL_PREPASS) {
    let mesh_flags = mesh[in.instance_index].flags;
    let is_logically_front = pbr_functions::winding_corrected_front_facing(mesh_flags, is_front);
    let normal = pbr_functions::prepare_world_normal(in.world_normal, false, is_logically_front);
    out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
}
@if(MOTION_VECTOR_PREPASS)
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);
    return out;
}
@else
@fragment
fn fragment(in: prepass_io::VertexOutput) {
    if gone(in, slope(in)) {
        discard;
    }
}
