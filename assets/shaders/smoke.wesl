// Smoke (D325, crates/terra_world/src/smoke.rs): a column of soft cards rising from where
// something burns, each card turned to face the eye, moved here from its own seed, the time
// and the wind rather than on the CPU, as fragments are drawn instanced (D305). The mesh holds
// each card's four corners at the emitter: the first texture coordinate is the corner, the
// second the card's phase through its life and its luck. A card is born small and thick at
// the emitter, rises, swells, leans with the wind and thins away; its puff is a soft round
// broken by noise of its own, lit by Bevy's own lighting so it sits in the same sun and haze.

import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
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

@if(PREPASS_PIPELINE)
@group(0) @binding(1) var<uniform> globals: Globals;

// Field order must match SmokeExtension in smoke.rs.
struct Smoke {
    // Where the wind carries it, metres a second in the world; and how thick it is, 0 to 1.
    wind: vec4<f32>,
    // How long a card lives, in seconds; how fast it rises, metres a second; how wide it is
    // born and how wide it ends, in metres.
    shape: vec4<f32>,
    // Its colour, linear, and how opaque a card is at its thickest.
    colour: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> smoke: Smoke;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

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

// How far through its life a card is, 0 to 1, from its phase and the time.
fn age(phase: f32) -> f32 {
    return fract(globals.time / smoke.shape.x + phase);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    let foot = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0)).xyz;
    let up = normalize(foot);
    let phase = vertex.uv_b.x;
    let luck = vertex.uv_b.y;
    let t = age(phase);
    let seconds = t * smoke.shape.x;
    // Rising, slowing as it cools; leaning with the wind more the higher it gets; wandering.
    let rise = smoke.shape.y * seconds * (1.0 - 0.35 * t);
    let wander = vec3<f32>(
        smooth_noise(vec2<f32>(luck * 13.0, seconds * 0.4)) - 0.5,
        0.0,
        smooth_noise(vec2<f32>(luck * 29.0 + 7.0, seconds * 0.4)) - 0.5,
    ) * (0.4 + 2.2 * t);
    let wind = smoke.wind.xyz * seconds * (0.4 + 0.6 * t);
    let middle = foot + up * rise + wind + wander;
    let size = mix(smoke.shape.z, smoke.shape.w, sqrt(t)) * (0.8 + 0.4 * luck);
    // Turned to the eye.
    let to_eye = normalize(view.world_position - middle);
    var right = cross(up, to_eye);
    if dot(right, right) < 1.0e-6 {
        right = cross(vec3<f32>(1.0, 0.0, 0.0), to_eye);
    }
    right = normalize(right);
    let card_up = cross(to_eye, right);
    // Each card turned about the eye's line by its own amount, so no two match.
    let spin = luck * 6.283 + seconds * (luck - 0.5) * 0.3;
    let corner = (vertex.uv - vec2<f32>(0.5)) * size;
    let turned = vec2<f32>(
        corner.x * cos(spin) - corner.y * sin(spin),
        corner.x * sin(spin) + corner.y * cos(spin),
    );
    let world = middle + right * turned.x + card_up * turned.y;
    out.position = position_world_to_clip(world);
    out.world_position = vec4<f32>(world, 1.0);
    out.uv = vertex.uv;
    out.uv_b = vec2<f32>(t, luck);
@if(PREPASS_PIPELINE) {
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS)
    out.world_normal = to_eye;
@if(MOTION_VECTOR_PREPASS)
    out.previous_world_position = out.world_position;
} @else {
    out.world_normal = to_eye;
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
    return out;
}

// How thick a card is at a point on it: a soft round, its edge broken by noise of its own,
// thick at once after it is born and thinning away as it ages.
fn density(uv: vec2<f32>, t: f32, luck: f32) -> f32 {
    let p = uv - vec2<f32>(0.5);
    let n = smooth_noise(uv * 3.5 + vec2<f32>(luck * 41.0, luck * 17.0));
    let fine = smooth_noise(uv * 9.0 + vec2<f32>(luck * 7.0, luck * 63.0));
    let round = 1.0 - smoothstep(0.18, 0.5, length(p) + (n - 0.5) * 0.28 + (fine - 0.5) * 0.08);
    let life = smoothstep(0.0, 0.08, t) * pow(1.0 - t, 1.6);
    return round * life * (0.7 + 0.6 * n);
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
    let alpha = density(in.uv, in.uv_b.x, in.uv_b.y) * smoke.colour.a * smoke.wind.w;
    if alpha < 0.004 {
        discard;
    }
    var input = pbr_input_from_standard_material(in, is_front);
    // Thicker smoke darker in its middle, as it is seen against the light.
    let shade = mix(1.0, 0.78, alpha);
    input.material.base_color = vec4<f32>(smoke.colour.rgb * shade, alpha);
    let n = normalize(in.world_normal);
    input.N = n;
    input.world_normal = n;
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(input);
    out.color = main_pass_post_lighting_processing(input, out.color);
    out.color.a = alpha;
    return out;
}
