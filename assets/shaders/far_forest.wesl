// The far forest's trees (D273, crates/terra_sandbox/src/experiments/lush/far.rs): each a
// card standing at its foot, turned about its upright to face the eye, as SpeedTree's
// billboards are, its tree drawn here rather than from a picture: an oak's lobed crown on a
// clear bole, a Scots pine's high ragged crown on its long trunk, a Norway spruce's cone of
// drooping whorls, with low undergrowth round the oaks' and pines' feet. Each crown is
// rounded as a mass of leaves, lit by Bevy's own lighting so it sits in the same sun, sky,
// shadows and haze as the near forest, darker low and inside; its leaves' grain is the
// bushes' leaf picture. The mesh holds each card's corners at its foot, a quad for each band
// of its tree's outline (woods/outline.rs, whose bounds are `tree()`'s: change both): the
// first texture coordinate is the corner on the whole card, the second its width and height,
// its colour its kind (0, 0.5, 1), its luck, for a card standing for a grown tree where it
// comes in, and which way the corner is widened (`widened`).

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
    mesh_view_bindings::{globals, lights},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

// The near trees' gusts and shared sway (D597), so the far forest stirs with them.
import super::gusts::trunk_lean;
@if(!PREPASS_PIPELINE)
import super::horizon::horizon_lit;
@if(!PREPASS_PIPELINE)
import super::hue::hue_turned;

@if(PREPASS_PIPELINE)
@group(0) @binding(1) var<uniform> globals: Globals;

// Field order must match woods::FarExtension in terra_world, its one definition (D396).
struct Far {
    // Each kind's crown colour, linear: oak, pine, spruce.
    crowns: array<vec4<f32>, 3>,
    // Within x of the eye a card is gone, from y on whole. A card whose colour's blue is
    // above 0 stands for a grown tree (terra_world's woods, D366): it comes in where that
    // tree's last level goes, from w of blue times z metres to all of it.
    near: vec4<f32>,
    // The land's horizon round the camera (horizon.wesl, D416), laid out as the ground's.
    horizon_middle: vec4<f32>,
    horizon_east: vec4<f32>,
    horizon_north: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> far: Far;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var horizon_map: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var horizon_sampler: sampler;

// As foliage.wesl's shared buffer (terra_world's parting, D271), read here for its last
// entry alone: the world's look, in x how far round the hue circle its plants are turned.
struct Parting {
    eye: vec4<f32>,
    subject: vec4<f32>,
    bodies: array<vec4<f32>, 32>,
    wind: vec4<f32>,
    climate_from_world: mat4x4<f32>,
    look: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var<storage, read> parting: Parting;
// The penumbra either side of a horizon, radians (ground.wesl's HORIZON_SOFT).
const HORIZON_SOFT: f32 = 0.008;

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

// A corner of one band of a card cut to its tree's outline (terra_world's woods/outline.rs),
// widened by two pixels at the card's distance the way its colour's alpha says (woods.rs's
// `outward`: 2 + 3 (across + 1) + up + 1), so the dithered edge of what the tree can be and
// TAA's jitter stay on it; never off the whole card. An alpha of 1 is a whole card.
fn widened(corner: vec2<f32>, alpha: f32, size: vec2<f32>, foot: vec3<f32>) -> vec2<f32> {
    if alpha < 1.5 {
        return corner;
    }
    let code = u32(round(alpha)) - 2u;
    let out = vec2<f32>(f32(code / 3u), f32(code % 3u)) - vec2<f32>(1.0);
    let pixel = 2.0 * distance(view.world_position, foot) / (view.clip_from_view[1][1] * view.viewport.w);
    return clamp(corner + out * 2.0 * pixel / size, vec2<f32>(0.0), vec2<f32>(1.0));
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    let foot = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0)).xyz;
    let f = facing(foot);
    let size = vertex.uv_b;
    let corner = widened(vertex.uv, vertex.color.a, size, foot);
    var world = foot + f.right * (corner.x - 0.5) * size.x + f.up * corner.y * size.y;
    // The crown sways as its near tree's trunk leans (gusts.wesl's `trunk_lean`, D597): the
    // card sheared downwind by its height squared, its top by the lean times its height. One
    // shear a card, so the cheapest crown-level sway, in every pass alike.
    if parting.wind.w >= 0.0 {
        let direction = normalize(parting.wind.xyz - f.up * dot(parting.wind.xyz, f.up) + vec3<f32>(1.0e-5, 0.0, 0.0));
        let across = cross(f.up, direction);
        let lean = trunk_lean(foot, direction, across, globals.time);
        world += (direction * lean.x + across * lean.y) * corner.y * corner.y * size.y * parting.wind.w;
    }
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
// many metres across.
struct Mass {
    middle: vec2<f32>,
    half: vec2<f32>,
    lobe: f32,
    luck: f32,
}

fn seed_of(m: Mass) -> vec2<f32> {
    return vec2<f32>(m.luck * 37.0, m.luck * 91.0);
}

// How far inside a mass a point is (above 0 is in it), and its lobes' noise there.
fn depth_in(p: vec2<f32>, m: Mass, ragged: f32) -> vec2<f32> {
    let q = (p - m.middle) / m.half;
    let seed = seed_of(m);
    let n = smooth_noise(p / m.lobe + seed);
    let fine = smooth_noise(p / (m.lobe * 0.35) + seed * 1.7);
    // Sprays of leaves at its edge, half a metre across.
    let sprays = smooth_noise(p / 0.5 + seed * 2.3);
    return vec2<f32>((1.0 - length(q)) - ragged * (n - 0.5) - 0.14 * (fine - 0.5) - 0.12 * (sprays - 0.5), n);
}

// The most `depth_in` can give a point: its noises (0 to 1 each) at their most.
fn deepest_in(p: vec2<f32>, m: Mass, ragged: f32) -> f32 {
    return (1.0 - length((p - m.middle) / m.half)) + 0.5 * ragged + 0.07 + 0.06;
}

// The point in the mass it is deepest in (`depth` its `depth_in`): its surface rounded as the
// mass is, and lumped by the lobes. Worked out for that mass alone, not every one.
fn mass(p: vec2<f32>, m: Mass, depth: vec2<f32>) -> Shape {
    var s: Shape;
    let q = (p - m.middle) / m.half;
    let seed = seed_of(m);
    let n = depth.y;
    s.inside = depth.x;
    let round = clamp(q, vec2<f32>(-1.0), vec2<f32>(1.0));
    let lumps = vec2<f32>(
        smooth_noise((p + vec2<f32>(0.3, 0.0)) / m.lobe + seed) - n,
        smooth_noise((p + vec2<f32>(0.0, 0.3)) / m.lobe + seed) - n,
    ) / 0.3 * m.lobe;
    s.normal = normalize(vec3<f32>(round + lumps * 0.5, sqrt(max(1.0 - dot(round, round), 0.05))));
    // Darker low in the crown and in the gaps between its lobes.
    s.light = (0.7 + 0.3 * smoothstep(-0.9, 0.5, q.y)) * (0.75 + 0.25 * n);
    s.part = 0u;
    return s;
}

// A crown of several masses round a middle: one in the middle and others about it, where
// the great limbs carry them, so its outline is lumped as a real crown's, not one ball. A
// point is the mass it is deepest in (the first of equals); a mass that cannot be deeper than
// the deepest yet is passed over without its noise, which changes nothing it shows.
fn crown(p: vec2<f32>, middle: vec2<f32>, half: vec2<f32>, masses: u32, lobe: f32, ragged: f32, luck: f32) -> Shape {
    var best = Mass(middle, half * vec2<f32>(0.7, 0.75), lobe, luck);
    var depth = depth_in(p, best, ragged);
    for (var k = 0u; k < masses; k++) {
        let f = f32(k);
        let a = luck * 6.283 + f * 6.283 / f32(masses) + 0.6 * hash(vec2<f32>(luck * 9.0, f));
        let reach = 0.55 + 0.25 * hash(vec2<f32>(f, luck * 5.0));
        let at = middle + vec2<f32>(cos(a), sin(a) * 0.8) * half * reach * 0.62;
        let size = half * (0.42 + 0.18 * hash(vec2<f32>(luck * 3.0 + f, 2.0)));
        let m = Mass(at, size, lobe * 0.8, luck + f * 0.37);
        if deepest_in(p, m, ragged) > depth.x {
            let d = depth_in(p, m, ragged);
            if d.x > depth.x {
                depth = d;
                best = m;
            }
        }
    }
    var s = mass(p, best, depth);
    // Lit higher in the crown, deeper in its shade low down.
    s.light *= 0.7 + 0.3 * smoothstep(middle.y - half.y, middle.y + half.y, p.y);
    return s;
}

// A card is cut to what this can reach (terra_world's woods/outline.rs): a change to a
// crown's place, size or noise, the trunk or the undergrowth changes its bounds there too.
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
    // A realm's plants in its own colours, as its near trees are (hue.wesl).
    colour = hue_turned(colour, parting.look.x);
    input.material.base_color = vec4<f32>(colour * s.light, 1.0);
    let n = world_normal(in.world_position.xyz, s.normal);
    input.N = n;
    input.world_normal = n;
    input.diffuse_occlusion = vec3<f32>(mix(0.75, 1.0, s.light));
    // A valley's trees go into a range's shadow with its ground (D416, ground.wesl's rule):
    // the sun off where it stands under the land's horizon, past the shadow map's reach.
    if lights.n_directional_lights > 0u {
        let light = lights.directional_lights[0];
        let view_z = dot(vec4<f32>(view.view_from_world[0].z, view.view_from_world[1].z, view.view_from_world[2].z, view.view_from_world[3].z), in.world_position);
        let reach = light.cascades[max(light.num_cascades, 1u) - 1u].far_bound;
        let fade = smoothstep(0.8 * reach, reach, -view_z);
        if fade > 0.0 {
            let lit = horizon_lit(horizon_map, horizon_sampler, far.horizon_middle, far.horizon_east, far.horizon_north, in.world_position.xyz, light.direction_to_light, HORIZON_SOFT);
            input.directional_shadow_factor = mix(1.0, lit, fade);
        }
    }
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(input);
    out.color = main_pass_post_lighting_processing(input, out.color);
    return out;
}
