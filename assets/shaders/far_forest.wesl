// The far forest's trees (D273, crates/terra_sandbox/src/experiments/lush/far.rs): each a
// card standing at its foot, turned about its upright to face the eye, as SpeedTree's
// billboards are, its tree drawn here rather than from a picture: an oak's lobed crown on a
// clear bole, a Scots pine's high ragged crown on its long trunk, a Norway spruce's cone of
// drooping whorls, with low undergrowth round the oaks' and pines' feet. Each crown is
// rounded as a mass of leaves, lit by Bevy's own lighting so it sits in the same sun, sky,
// shadows and haze as the near forest, darker low and inside; its leaves' grain is the
// bushes' leaf picture. The mesh holds each card's four corners at its foot: the first
// texture coordinate is the corner, the second its width and height, its colour its kind
// (0, 0.5, 1), its luck and, for a card standing for a grown tree, where it comes in.

import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, VertexOutput, decompress_vertex};
@if(PREPASS_PIPELINE)
import bevy_pbr::render::pbr_prepass_functions;
@if(PREPASS_PIPELINE)
import bevy_render::globals::Globals;
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::{
    forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    mesh_view_bindings::globals,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

@if(PREPASS_PIPELINE)
@group(0) @binding(1) var<uniform> globals: Globals;

// Field order must match FarExtension in far.rs.
struct Far {
    // Each kind's crown colour, linear: oak, pine, spruce.
    crowns: array<vec4<f32>, 3>,
    // Within x of the eye a card is gone, from y on whole. A card whose colour's blue is
    // above 0 stands for a grown tree (terra_world's woods, D366): it comes in where that
    // tree's last level goes, from w of blue times z metres to all of it.
    near: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> far: Far;

// Which way a card faces, at a point on it: toward the eye, about the planet's up there.
struct Facing {
    up: vec3<f32>,
    right: vec3<f32>,
    toward: vec3<f32>,
}

fn facing(at: vec3<f32>) -> Facing {
    var f: Facing;
    f.up = normalize(at);
    let to_eye = view.world_position - at;
    var level = to_eye - f.up * dot(to_eye, f.up);
    if dot(level, level) < 1.0e-6 {
        level = cross(f.up, vec3<f32>(1.0, 0.0, 0.0));
    }
    f.toward = normalize(level);
    f.right = cross(f.up, f.toward);
    return f;
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    let foot = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0)).xyz;
    let f = facing(foot);
    let size = vertex.uv_b;
    let corner = vertex.uv;
    let world = foot + f.right * (corner.x - 0.5) * size.x + f.up * corner.y * size.y;
    out.position = position_world_to_clip(world);
    out.world_position = vec4<f32>(world, 1.0);
    out.uv = corner;
    out.uv_b = size;
    out.color = vertex.color;
@if(PREPASS_PIPELINE) {
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS)
    out.world_normal = f.toward;
@if(MOTION_VECTOR_PREPASS)
    out.previous_world_position = out.world_position;
} @else {
    out.world_normal = f.toward;
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

// Smooth noise, 0 to 1.
fn smooth_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// What a point on a card is: how far inside the tree (above 0 is in it), which way its
// surface faces on the card (right, up, toward the eye), how much light reaches into it,
// and what it is: 0 leaves, 1 bark, 2 undergrowth.
struct Shape {
    inside: f32,
    normal: vec3<f32>,
    light: f32,
    part: u32,
}

// A mass of leaves round a middle with these half sizes, its edge broken into lobes so
// many metres across: its surface rounded as the mass is, and lumped by the lobes.
fn mass(p: vec2<f32>, middle: vec2<f32>, half: vec2<f32>, lobe: f32, ragged: f32, luck: f32) -> Shape {
    var s: Shape;
    let q = (p - middle) / half;
    let seed = vec2<f32>(luck * 37.0, luck * 91.0);
    let n = smooth_noise(p / lobe + seed);
    let fine = smooth_noise(p / (lobe * 0.35) + seed * 1.7);
    // Sprays of leaves at its edge, half a metre across.
    let sprays = smooth_noise(p / 0.5 + seed * 2.3);
    s.inside = (1.0 - length(q)) - ragged * (n - 0.5) - 0.14 * (fine - 0.5) - 0.12 * (sprays - 0.5);
    let round = clamp(q, vec2<f32>(-1.0), vec2<f32>(1.0));
    let lumps = vec2<f32>(
        smooth_noise((p + vec2<f32>(0.3, 0.0)) / lobe + seed) - n,
        smooth_noise((p + vec2<f32>(0.0, 0.3)) / lobe + seed) - n,
    ) / 0.3 * lobe;
    s.normal = normalize(vec3<f32>(round + lumps * 0.5, sqrt(max(1.0 - dot(round, round), 0.05))));
    // Darker low in the crown and in the gaps between its lobes.
    s.light = (0.7 + 0.3 * smoothstep(-0.9, 0.5, q.y)) * (0.75 + 0.25 * n);
    s.part = 0u;
    return s;
}

// Of two masses of leaves, the one a point is deeper inside.
fn deeper(a: Shape, b: Shape) -> Shape {
    if b.inside > a.inside {
        return b;
    }
    return a;
}

// A crown of several masses round a middle: one in the middle and others about it, where
// the great limbs carry them, so its outline is lumped as a real crown's, not one ball.
fn crown(p: vec2<f32>, middle: vec2<f32>, half: vec2<f32>, masses: u32, lobe: f32, ragged: f32, luck: f32) -> Shape {
    var s = mass(p, middle, half * vec2<f32>(0.7, 0.75), lobe, ragged, luck);
    for (var k = 0u; k < masses; k++) {
        let f = f32(k);
        let a = luck * 6.283 + f * 6.283 / f32(masses) + 0.6 * hash(vec2<f32>(luck * 9.0, f));
        let reach = 0.55 + 0.25 * hash(vec2<f32>(f, luck * 5.0));
        let at = middle + vec2<f32>(cos(a), sin(a) * 0.8) * half * reach * 0.62;
        let size = half * (0.42 + 0.18 * hash(vec2<f32>(luck * 3.0 + f, 2.0)));
        s = deeper(s, mass(p, at, size, lobe * 0.8, ragged, luck + f * 0.37));
    }
    // Lit higher in the crown, deeper in its shade low down.
    s.light *= 0.7 + 0.3 * smoothstep(middle.y - half.y, middle.y + half.y, p.y);
    return s;
}

fn tree(p: vec2<f32>, size: vec2<f32>, kind: u32, luck: f32) -> Shape {
    let w = size.x;
    let h = size.y;
    var s: Shape;
    var bole = 0.0;
    var trunk = 0.0;
    if kind == 0u {
        // An oak: a broad crown of several masses from a third of its height.
        s = crown(p, vec2<f32>(0.0, 0.64 * h), vec2<f32>(0.5 * w, 0.34 * h), 5u, 0.13 * w, 0.4, luck);
        bole = 0.42 * h;
        trunk = 0.028 * h;
    } else if kind == 1u {
        // A Scots pine: a high, ragged, flat-topped crown of a few clumps on a long trunk.
        s = crown(p, vec2<f32>(0.0, 0.82 * h), vec2<f32>(0.5 * w, 0.17 * h), 3u, 0.16 * w, 0.5, luck);
        bole = 0.86 * h;
        trunk = 0.022 * h;
    } else {
        // A Norway spruce of the tall stand: a cone of whorls from a third of its height, each
        // drooping wider at its foot, over its bare bole (D318).
        let base = 0.32 * h;
        let up = clamp((p.y - base) / (h - base), 0.0, 1.0);
        let whorl = fract((p.y - base) / (1.3 + 0.03 * h) + luck * 5.0);
        let n = smooth_noise(vec2<f32>(p.y / 0.9, luck * 13.0 + sign(p.x) * 3.0));
        var half = 0.5 * w * pow(1.0 - up, 0.9) * (0.74 + 0.26 * whorl) * (0.85 + 0.3 * n);
        half = max(half, 0.0);
        s.inside = select((half - abs(p.x)) / (0.5 * w), -1.0, p.y < base || p.y > h);
        let across = clamp(p.x / max(half, 0.05), -1.0, 1.0);
        s.normal = normalize(vec3<f32>(across * 0.75, 0.35 + 0.3 * whorl, sqrt(max(1.0 - across * across, 0.05))));
        s.light = (0.5 + 0.5 * up) * (0.65 + 0.35 * whorl);
        s.part = 0u;
        bole = base;
        trunk = 0.02 * h;
    }
    // The trunk, below the leaves.
    if s.inside <= 0.0 && p.y < bole && abs(p.x) < trunk * mix(1.3, 0.8, p.y / max(bole, 1.0)) {
        s.inside = 1.0 - abs(p.x) / trunk;
        let across = clamp(p.x / trunk, -1.0, 1.0);
        s.normal = vec3<f32>(across, 0.0, sqrt(1.0 - across * across));
        // Under a closed stand's canopy, in its shade, darkest at the foot: lit as the crowns,
        // the long boles of a pine wood seen across a lake stood as a lit picket fence.
        s.light = mix(0.22, 0.42, clamp(p.y / max(bole, 1.0), 0.0, 1.0));
        s.part = 1u;
    }
    // Undergrowth round the foot of every tree, one to three and a half metres high, across
    // the card, so a wood's edge is a belt of shrubs as a real one is.
    if s.inside <= 0.0 {
        let n = smooth_noise(vec2<f32>(p.x / 1.4, luck * 29.0));
        let twigs = smooth_noise(p / 0.35 + vec2<f32>(luck * 17.0, 0.0));
        let tall = (1.0 + 2.5 * n * n) * (1.0 - smoothstep(0.3 * w, 0.5 * w, abs(p.x))) * (0.8 + 0.4 * twigs);
        if p.y < tall {
            s.inside = (tall - p.y) / max(tall, 0.1);
            s.normal = normalize(vec3<f32>((n - 0.5) * 0.6, 0.6, 0.8));
            s.light = 0.5 + 0.3 * p.y / max(tall, 0.1);
            s.part = 2u;
        }
    }
    return s;
}

fn noise_at(frag_coord: vec4<f32>) -> f32 {
    let shifted = frag_coord.xy + 5.588238 * f32(globals.frame_count % 64u);
    return fract(52.9829189 * fract(dot(shifted, vec2<f32>(0.06711056, 0.00583715))));
}

// The point on its card a fragment is, in metres across from the middle and up from the
// foot, and its tree there; gone where it is not the tree, its edge dithered over a pixel
// for TAA to smooth, and near the eye, as a walker comes into the far forest.
fn seen(in: VertexOutput) -> Shape {
    let p = vec2<f32>((in.uv.x - 0.5) * in.uv_b.x, in.uv.y * in.uv_b.y);
    let kind = u32(round(in.color.r * 2.0));
    let s = tree(p, in.uv_b, kind, in.color.g);
    let dither = noise_at(in.position);
    let edge = max(fwidth(s.inside), 1.0e-4);
    var gone = far.near.xy;
    if in.color.b > 0.0 {
        let to = in.color.b * far.near.z;
        gone = vec2<f32>(to * far.near.w, to);
    }
    let near = smoothstep(gone.x, gone.y, distance(in.world_position.xyz, view.world_position));
    if s.inside < (dither - 0.5) * edge || dither > near {
        discard;
    }
    return s;
}

fn world_normal(at: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let f = facing(at);
    return normalize(f.right * normal.x + f.up * normal.y + f.toward * normal.z);
}

@if(PREPASS_PIPELINE && PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) -> bevy_pbr::prepass::io::FragmentOutput {
    let s = seen(in);
    var out: bevy_pbr::prepass::io::FragmentOutput;
@if(NORMAL_PREPASS)
    out.normal = vec4<f32>(world_normal(in.world_position.xyz, s.normal) * 0.5 + vec3<f32>(0.5), 1.0);
@if(MOTION_VECTOR_PREPASS)
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);
    return out;
}

@if(PREPASS_PIPELINE && !PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) {
    _ = seen(in);
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let s = seen(in);
    let kind = u32(round(in.color.r * 2.0));
    let luck = in.color.g;
    let p = vec2<f32>((in.uv.x - 0.5) * in.uv_b.x, in.uv.y * in.uv_b.y);
    // The leaves' grain: the leaf picture laid over the card a metre and a half a repeat.
    var grained = in;
    grained.uv = p / 1.5 + vec2<f32>(luck * 3.1, luck * 7.7);
    // Not tinted by the colour, which carries the card's kind and luck.
    grained.color = vec4<f32>(1.0);
    var input = pbr_input_from_standard_material(grained, is_front);
    let picture = input.material.base_color.rgb;
    // The picture is about 0.4 where it is leaf; darker in the gaps between its leaves.
    let leaf = input.material.base_color.a;
    let grain = clamp(dot(picture, vec3<f32>(0.3, 0.59, 0.11)) / 0.4, 0.6, 1.4) * mix(0.65, 1.0, leaf);
    var colour = far.crowns[kind].rgb * (0.85 + 0.3 * luck) * mix(1.0, grain, 0.55);
    if s.part == 1u {
        // Bark, grey-brown, a Scots pine's orange high up.
        colour = select(vec3<f32>(0.075, 0.065, 0.055), vec3<f32>(0.16, 0.08, 0.04), kind == 1u && p.y > 0.5 * in.uv_b.y);
    } else if s.part == 2u {
        colour = far.crowns[0].rgb * vec3<f32>(0.8, 0.9, 0.7) * mix(1.0, grain, 0.5);
    }
    input.material.base_color = vec4<f32>(colour * s.light, 1.0);
    let n = world_normal(in.world_position.xyz, s.normal);
    input.N = n;
    input.world_normal = n;
    input.diffuse_occlusion = vec3<f32>(mix(0.75, 1.0, s.light));
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(input);
    out.color = main_pass_post_lighting_processing(input, out.color);
    return out;
}
