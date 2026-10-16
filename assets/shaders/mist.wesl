// A lake's morning mist (D371, crates/terra_world/src/lakes.rs): banks of soft cards standing
// on the water, each turned about the up to face the eye, as games lay mist banks over water
// (soft particles, fading where they meet what stands behind them in the depth prepass, so no
// card's edge shows on the shore or the water). The mesh holds each card's four corners at its
// foot on the water: the first texture coordinate is the corner, the second its width and
// height in metres. What it is made of is a noise laid in the world and drifting downwind, so
// the cards' overlaps read as one bank, thin and torn at its edges; lit by Bevy's own lighting,
// the sun and the sky, so it sits in the same light as everything else.

import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::{view, globals},
    view_transformations::{position_world_to_clip, depth_ndc_to_view_z},
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, VertexOutput, decompress_vertex};
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::{
    forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};
@if(DEPTH_PREPASS && !PREPASS_PIPELINE)
import bevy_pbr::prepass::utils::prepass_depth;

// Field order must match MistExtension in lakes.rs.
struct Mist {
    // Which way and how fast its drifts move, metres a second; and how thick it is, 0 to 1.
    wind: vec4<f32>,
    // A point near the lakes its noise is laid out from, so it reads small numbers.
    origin: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> mist: Mist;

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash(i), hash(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash(i + vec3<f32>(0.0, 1.0, 0.0)), hash(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash(i + vec3<f32>(0.0, 0.0, 1.0)), hash(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash(i + vec3<f32>(0.0, 1.0, 1.0)), hash(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    let foot = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0)).xyz;
    let up = normalize(foot);
    // Turned about the up to face the eye.
    let to_eye = view.world_position - foot;
    var right = cross(up, to_eye);
    if dot(right, right) < 1.0e-6 {
        right = cross(up, vec3<f32>(1.0, 0.0, 0.0));
    }
    right = normalize(right);
    let size = vertex.uv_b;
    let world = foot + right * (vertex.uv.x - 0.5) * size.x + up * (1.0 - vertex.uv.y) * size.y;
    out.position = position_world_to_clip(world);
    out.world_position = vec4<f32>(world, 1.0);
    out.uv = vertex.uv;
    out.uv_b = size;
@if(PREPASS_PIPELINE) {
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS)
    out.world_normal = up;
@if(MOTION_VECTOR_PREPASS)
    out.previous_world_position = out.world_position;
} @else {
    out.world_normal = up;
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

// How far from the eye the mist starts to show, and is full: a card the eye passes through
// would fill the screen.
const NEAR_M: vec2<f32> = vec2<f32>(25.0, 90.0);
// How far behind a card what is drawn there must stand for the card to show fully, metres.
const SOFT_M: f32 = 1.5;
// How far under the water a card's foot stands (lakes.rs MIST_FOOT_M).
const FOOT_M: f32 = 1.5;

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let world = in.world_position.xyz;
    let up = normalize(world);
    let local = world - mist.origin.xyz;
    let drift = local - mist.wind.xyz * globals.time;
    // The bank's stuff: broad drifts tens of metres across, torn finer, the same every way
    // round (the world's axes are not the lake's level here).
    let broad = noise(drift * 0.022);
    let fine = noise(drift * 0.085 + 17.0);
    let stuff = smoothstep(0.45, 0.85, broad * 0.65 + fine * 0.35);
    // Thickest low over the water, thinning all the way up the card, its top torn by the
    // fine noise; soft over a third of its width each side, so no card's edge shows where
    // the banks overlap.
    let height = 1.0 - in.uv.y;
    // Its foot stands under the water, which writes no depth: from the waterline up only,
    // softly, so no card's foot draws a ruler line across the lake.
    let over_water = height * in.uv_b.y - FOOT_M;
    let footed = smoothstep(0.0, 1.5, over_water);
    let thinning = 1.0 - smoothstep(0.0, 1.0, height + (fine - 0.5) * 0.6 + (broad - 0.5) * 0.5);
    let across = 1.0 - abs(in.uv.x - 0.5) * 2.0;
    let profile = thinning * thinning * smoothstep(0.0, 0.7, across) * footed;
    let to_eye = view.world_position - world;
    let distance = length(to_eye);
    // Seen from above a card stands edge-on to the eye's way down: fade it as the eye rises.
    let level = 1.0 - abs(dot(to_eye / distance, up));
    // Far off, the many banks along the eye's way stack: thinned there, so the far shore's
    // feet are veiled, not walled off.
    let stacked = mix(1.0, 0.45, smoothstep(300.0, 900.0, distance));
    var alpha = stuff * profile * mist.wind.w * smoothstep(NEAR_M.x, NEAR_M.y, distance) * level * level * stacked;
@if(DEPTH_PREPASS)
    {
        let behind = depth_ndc_to_view_z(in.position.z) - depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        alpha *= smoothstep(0.0, SOFT_M, behind);
    }
    alpha = clamp(alpha, 0.0, 1.0) * 0.24;
    if alpha < 0.003 {
        discard;
    }
    var input = pbr_input_from_standard_material(in, is_front);
    // A touch warm: water vapour lit by a low sun and a blue sky, white, never the sky's violet.
    input.material.base_color = vec4<f32>(0.92, 0.88, 0.82, alpha);
    // Lit as the water's own skin of air is: by the sky from above and the sun through it.
    let n = normalize(up + normalize(to_eye) * 0.6);
    input.N = n;
    input.world_normal = n;
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(input);
    out.color = main_pass_post_lighting_processing(input, out.color);
    out.color.a = alpha;
    return out;
}
