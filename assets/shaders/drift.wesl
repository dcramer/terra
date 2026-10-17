// What drifts in the air round the eye (crates/terra_world/src/drift.rs): motes, midges,
// falling leaves, down. Each card is moved here from its own luck and the time, in a box
// round the eye that the field wraps in, so the field is endless and fixed in the world: a
// card that leaves the box on one side comes in at the other, faded at its edges so none is
// seen to. The mesh is every card's four corners at the field's origin, whose frame stands
// level under the eye; the first texture coordinate is the corner, the second each card's
// luck and its place in the kind's swarms. A card is lit by the sun as it reaches it, the
// sun's shadow taken where it is, and glows toward the sun as dust in a sunbeam does, so
// motes and midges show where the sun's shafts come through the crowns and not in the shade.

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
    mesh_view_bindings::{globals, lights},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    shadows::fetch_directional_shadow,
};

@if(PREPASS_PIPELINE)
@group(0) @binding(1) var<uniform> globals: Globals;

// Field order must match DriftExtension in drift.rs.
struct Drift {
    // The box's half width and half height, a card's size in metres, how much is seen.
    field: vec4<f32>,
    // How it moves (0 hover, 1 swarm, 2 fall) and its three numbers: hover's wander and
    // sink; a swarm's count, width and beat; a fall's speed, sway and turning.
    motion: vec4<f32>,
    // The wind in the field's frame, metres a second, and how much of it carries the kind.
    wind: vec4<f32>,
    // Its colour, linear, and its glow toward the sun.
    colour: vec4<f32>,
    // How it is drawn (0 speck, 1 leaf, 2 bird), how opaque.
    look: vec4<f32>,
    // A flock's: how high over the eye it flies, lowest and highest; its wings' beat; the
    // share of its crossings it is about on.
    flock: vec4<f32>,
}

fn is_leaf() -> bool {
    return drift.look.x > 0.5 && drift.look.x < 1.5;
}

fn is_bird() -> bool {
    return drift.look.x > 1.5;
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> drift: Drift;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn hash3(x: f32) -> vec3<f32> {
    return vec3<f32>(
        hash(vec2<f32>(x * 811.0, 17.0)),
        hash(vec2<f32>(x * 613.0, 59.0)),
        hash(vec2<f32>(x * 379.0, 101.0)),
    );
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

// A wandering offset, each axis -1 to 1, slowly changing with t.
fn wander(seed: f32, t: f32) -> vec3<f32> {
    return vec3<f32>(
        smooth_noise(vec2<f32>(seed * 13.1, t)),
        smooth_noise(vec2<f32>(seed * 29.7 + 7.0, t)),
        smooth_noise(vec2<f32>(seed * 41.3 + 3.0, t)),
    ) * 2.0 - 1.0;
}

// Where a card is in the field's frame, before it is wrapped into the box.
fn place(luck: f32, order: f32, seconds: f32) -> vec3<f32> {
    let half = vec3<f32>(drift.field.x, drift.field.y, drift.field.x);
    let carried = drift.wind.xyz * drift.wind.w * seconds;
    let mode = drift.motion.x;
    if mode < 0.5 {
        // Hanging in the air, wandering, sinking a little.
        let base = (hash3(luck) * 2.0 - 1.0) * half;
        return base + carried + wander(luck, seconds * 0.12) * drift.motion.y
            - vec3<f32>(0.0, drift.motion.z * seconds, 0.0);
    }
    if mode < 1.5 {
        // In one of a few swarms, each hanging low over the ground, dancing about its
        // middle, the middle itself drifting a little.
        let swarm = floor(order * drift.motion.y);
        var middle = (hash3(swarm * 0.137 + 0.21) * 2.0 - 1.0) * half;
        middle.y = mix(-half.y * 0.55, half.y * 0.1, hash(vec2<f32>(swarm, 5.0)));
        middle += wander(swarm + 0.5, seconds * 0.05) * 1.5 + carried;
        let dance = wander(luck, seconds * drift.motion.w) * drift.motion.z;
        return middle + vec3<f32>(dance.x, dance.y * 0.6, dance.z);
    }
    if mode > 2.5 {
        // In one of a few flocks crossing the sky, each on its own heading, loosely
        // gathered and each bird wandering in it; its height is the eye's own (`vertex`).
        let flock = floor(order * drift.motion.y);
        let heading = hash(vec2<f32>(flock, 11.0)) * 6.283;
        let way = vec3<f32>(cos(heading), 0.0, sin(heading));
        let start = (hash3(flock * 0.173 + 0.37) * 2.0 - 1.0) * half;
        let spread = drift.motion.w;
        let offset = (hash3(luck) * 2.0 - 1.0) * spread * vec3<f32>(1.0, 0.3, 1.0)
            + wander(luck, globals.time * 0.25) * spread * 0.15;
        let high = mix(drift.flock.x, drift.flock.y, hash(vec2<f32>(flock, 23.0)));
        let middle = start + way * drift.motion.z * globals.time;
        return vec3<f32>(middle.x + offset.x, high + offset.y, middle.z + offset.z);
    }
    // Falling, swaying, carried by the wind.
    let base = (hash3(luck) * 2.0 - 1.0) * half;
    let sway = vec3<f32>(
        sin(seconds * 1.3 + luck * 6.283),
        0.0,
        cos(seconds * 0.9 + luck * 12.57) * 0.6,
    ) * drift.motion.z;
    return base + carried + sway - vec3<f32>(0.0, drift.motion.y * seconds * (0.8 + 0.4 * luck), 0.0);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    let origin = world_from_local[3].xyz;
    let frame = mat3x3<f32>(world_from_local[0].xyz, world_from_local[1].xyz, world_from_local[2].xyz);
    let luck = vertex.uv_b.x;
    let order = vertex.uv_b.y;
    let seconds = globals.time + luck * 97.0;
    // The eye in the field's frame, and the card wrapped into the box round it.
    let eye = transpose(frame) * (view.world_position - origin);
    let half = vec3<f32>(drift.field.x, drift.field.y, drift.field.x);
    let free = place(luck, order, seconds) - eye;
    let crossing = round(free / (2.0 * half));
    var rel = free - 2.0 * half * crossing;
    var edge = max(max(abs(rel.x) / half.x, abs(rel.z) / half.x), abs(rel.y) / half.y);
    var about = 1.0;
    if is_bird() {
        // A flock flies at its height over the eye, and is about on some of its crossings
        // of the box only, so the sky is not always full of the same birds.
        let placed = place(luck, order, seconds);
        rel.y = placed.y;
        edge = max(abs(rel.x), abs(rel.z)) / half.x;
        let flock = floor(order * drift.motion.y);
        let luck_of_crossing = hash(vec2<f32>(crossing.x * 7.13 + flock, crossing.z * 3.71 + flock * 0.37));
        about = select(0.0, 1.0, luck_of_crossing < drift.flock.w);
    }
    let middle = origin + frame * (eye + rel);
    // Faded toward the box's sides, so none is seen to come or go, and over the few metres
    // nearest the eye, where a speck would be a big bright fleck across the picture.
    let distance = length(view.world_position - middle);
    var fade = (1.0 - smoothstep(0.65, 1.0, edge)) * smoothstep(1.5, 4.0, distance) * about;
    // Never smaller than a pixel and a half on screen; one drawn bigger is fainter, so a
    // far mote does not flicker in and out.
    let pixel_m = distance * 2.0 / (view.clip_from_view[1][1] * view.viewport.w);
    var size = drift.field.z * (0.7 + 0.6 * hash(vec2<f32>(luck * 71.0, 3.0)));
    let drawn = max(size, 1.5 * pixel_m);
    fade *= (size / drawn) * (size / drawn);
    size = drawn;
    // Turned to the eye; a leaf turns over as it falls, seen edge-on now and then.
    let to_eye = normalize(view.world_position - middle);
    let up = frame[1];
    var right = cross(up, to_eye);
    if dot(right, right) < 1.0e-6 {
        right = cross(frame[0], to_eye);
    }
    right = normalize(right);
    let card_up = cross(to_eye, right);
    var corner = (vertex.uv - vec2<f32>(0.5)) * size;
    var turned = corner;
    var flip = 1.0;
    if is_bird() {
        // Its wings' beat, each bird at its own phase, carried to the fragment.
        flip = sin(globals.time * drift.flock.z * 6.283 + luck * 6.283);
    }
    if is_leaf() {
        let spin = seconds * drift.motion.w * 6.283 * (0.6 + 0.8 * luck) + luck * 6.283;
        flip = cos(spin);
        corner.x *= max(abs(flip), 0.15);
        let roll = luck * 6.283 + seconds * 0.7 * (luck - 0.5);
        turned = vec2<f32>(
            corner.x * cos(roll) - corner.y * sin(roll),
            corner.x * sin(roll) + corner.y * cos(roll),
        );
    }
    let world = middle + right * turned.x + card_up * turned.y;
    out.position = position_world_to_clip(world);
    out.world_position = vec4<f32>(world, 1.0);
    out.uv = vertex.uv;
    out.uv_b = vec2<f32>(fade * drift.field.w, flip);
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

// How much of the card is there: a soft round speck, or a leaf's pointed oval with its
// midrib.
fn coverage(uv: vec2<f32>, flap: f32) -> f32 {
    let p = uv - vec2<f32>(0.5);
    if drift.look.x < 0.5 {
        let r = length(p) * 2.0;
        return pow(clamp(1.0 - r, 0.0, 1.0), 1.5);
    }
    if is_bird() {
        // A body and two wings, each a tapering stroke out to a tip that rises and falls
        // with the beat, bent at the wrist.
        let body = 1.0 - smoothstep(0.8, 1.0, length(p * vec2<f32>(1.0 / 0.07, 1.0 / 0.045)));
        var wings = 0.0;
        for (var side = -1.0; side <= 1.0; side += 2.0) {
            let wrist = vec2<f32>(side * 0.22, 0.05 + 0.12 * flap);
            let tip = vec2<f32>(side * 0.47, -0.02 + 0.3 * flap);
            for (var half_wing = 0; half_wing < 2; half_wing++) {
                let a = select(wrist, vec2<f32>(0.0, 0.02), half_wing == 0);
                let b = select(tip, wrist, half_wing == 0);
                let ab = b - a;
                let s = clamp(dot(p - a, ab) / dot(ab, ab), 0.0, 1.0);
                let along = (f32(half_wing) + s) * 0.5;
                let width = mix(0.07, 0.015, along);
                let d = length(p - (a + ab * s));
                wings = max(wings, 1.0 - smoothstep(width * 0.6, width, d));
            }
        }
        return max(body, wings);
    }
    // Pointed at both ends: the half width falls off as the square of the way along.
    let along = p.y * 2.0;
    let width = 0.42 * (1.0 - along * along);
    let inside = 1.0 - smoothstep(width - 0.06, width, abs(p.x));
    let stalk = (1.0 - smoothstep(0.0, 0.03, abs(p.x))) * step(0.42, -p.y) * step(-p.y, 0.5);
    return max(inside * step(abs(along), 1.0), stalk);
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
    let alpha = coverage(in.uv, in.uv_b.y) * in.uv_b.x * drift.look.y;
    if alpha < 0.004 {
        discard;
    }
    var input = pbr_input_from_standard_material(in, is_front);
    // A leaf's two faces: the underside paler, and a vein down its middle.
    var colour = drift.colour.rgb;
    if is_leaf() {
        let vein = 1.0 - 0.25 * (1.0 - smoothstep(0.0, 0.025, abs(in.uv.x - 0.5)));
        colour *= vein * select(1.25, 1.0, in.uv_b.y > 0.0);
    }
    input.material.base_color = vec4<f32>(colour, alpha);
    let n = normalize(in.world_normal);
    input.N = n;
    input.world_normal = n;
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(input);
    // The sun scattered forward toward the eye where it reaches the card.
    if lights.n_directional_lights > 0u {
        let sun = lights.directional_lights[0];
        let L = sun.direction_to_light;
        let toward = clamp(dot(-input.V, L), 0.0, 1.0);
        let forward = 0.7 * pow(toward, 8.0) + 0.3 * toward * toward;
        let view_z = dot(vec4<f32>(
            view.view_from_world[0].z,
            view.view_from_world[1].z,
            view.view_from_world[2].z,
            view.view_from_world[3].z
        ), input.world_position);
        let shadow = fetch_directional_shadow(0u, input.world_position, L, view_z, in.position.xy);
        out.color = vec4<f32>(
            out.color.rgb + colour * sun.color.rgb * view.exposure * shadow * forward * drift.colour.w,
            out.color.a,
        );
    }
    out.color = main_pass_post_lighting_processing(input, out.color);
    out.color.a = alpha;
    return out;
}
