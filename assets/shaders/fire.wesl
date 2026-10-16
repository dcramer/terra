// Fire's flames, embers and jets (D377, crates/terra_world/src/fire.rs): cards placed here
// from each blaze's slot of a buffer, the card's seed and the time, as smoke's are (D325).
// Every corner of the mesh is at the origin; its first texture coordinate is the corner, its
// second the slot and the card's place among the slot's.
//
// A flame card is born at its blaze's foot, rolls up and narrows as it rises, standing
// upright and turned to the eye about the up; its shape a soft tongue broken by noise that
// climbs it, its colour a glowing soot's by how hot that part of it is: white-yellow at the
// root, orange, dark red at its tips, gone into nothing above. An ember is a spark thrown up
// on the plume, slowing, carried downwind, stretched along its flight and winking out. A jet
// is a stream of flame breathed out along a way, widening in a cone, hottest at the mouth,
// rolling into dark red and soot at its end. All are light: added to what is behind them, in
// the same units the sun's light is, so the camera's exposure takes them as it takes it.

import bevy_pbr::render::{
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, VertexOutput, decompress_vertex};
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex};

struct Fire {
    // x which (0 flames, 1 embers, 2 jets), y how many slots are filled, z the time.
    kind: vec4<f32>,
    // The wind, metres a second, and its strength.
    wind: vec4<f32>,
    // Each jet's from and reach; way and spread; on, and since when.
    jets: array<vec4<f32>, 6>,
}

// Field order must match FireExtension in crates/terra_world/src/fire.rs.
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> fire: Fire;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> slots: array<vec4<f32>>;

// A card's corner placed: where on screen, the corner across and up the card, and how far
// through its life, how hot, its luck, and how bright.
struct Out {
    clip: vec4<f32>,
    uv: vec2<f32>,
    life: vec4<f32>,
};

// How bright the hottest flame is, in the units the sun's light is in (a flame's luminance,
// some thousands of nits), before the camera's exposure.
const NITS: f32 = 9000.0;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    return 0.55 * noise(p) + 0.3 * noise(p * 2.1 + 3.7) + 0.15 * noise(p * 4.3 + 9.1);
}

// A colour of glowing soot by how hot, 0 to 1: dark red, orange, yellow, near white.
fn glow(t: f32) -> vec3<f32> {
    let c = clamp(t, 0.0, 1.0);
    let red = vec3(0.55, 0.06, 0.01);
    let orange = vec3(1.0, 0.36, 0.05);
    let yellow = vec3(1.0, 0.72, 0.28);
    let white = vec3(1.0, 0.93, 0.78);
    if c < 0.4 {
        return mix(red * 0.3, red, c / 0.4);
    }
    if c < 0.7 {
        return mix(red, orange, (c - 0.4) / 0.3);
    }
    if c < 0.9 {
        return mix(orange, yellow, (c - 0.7) / 0.2);
    }
    return mix(yellow, white, (c - 0.9) / 0.1);
}

fn nowhere() -> Out {
    var out: Out;
    out.clip = vec4(0.0, 0.0, -1.0, 1.0);
    out.uv = vec2(0.0);
    out.life = vec4(0.0);
    return out;
}

// A card at a point, `w` wide and `h` tall, its foot or middle there, upright about `up`
// or turned wholly to the eye.
fn card(at: vec3<f32>, up: vec3<f32>, corner: vec2<f32>, w: f32, h: f32, upright: bool, spin: f32) -> vec3<f32> {
    let to_eye = normalize(view.world_position - at);
    var right = cross(up, to_eye);
    if dot(right, right) < 1.0e-6 {
        right = cross(vec3(1.0, 0.0, 0.0), to_eye);
    }
    right = normalize(right);
    var card_up = up;
    if !upright {
        card_up = cross(to_eye, right);
    }
    let c = vec2(corner.x - 0.5, select(corner.y - 0.5, corner.y, upright));
    let turned = vec2(c.x * cos(spin) - c.y * sin(spin), c.x * sin(spin) + c.y * cos(spin));
    return at + right * turned.x * w + card_up * turned.y * h;
}

fn place(corner: vec2<f32>, id: vec2<f32>) -> Out {
    let kind = i32(fire.kind.x + 0.5);
    let time = fire.kind.z;
    let slot = u32(id.x + 0.5);
    let phase = id.y;
    let wind = fire.wind.xyz;
    var out: Out;
    if kind == 2 {
        // A jet: each card a puff flying out along the way from the mouth.
        let j = slot * 3u;
        let mouth = fire.jets[j].xyz;
        let reach = fire.jets[j].w;
        let way = normalize(fire.jets[j + 1u].xyz);
        let spread = fire.jets[j + 1u].w;
        let on = fire.jets[j + 2u].x;
        let since = fire.jets[j + 2u].y;
        if reach <= 0.0 {
            return nowhere();
        }
        let speed = 34.0;
        let life_s = reach / speed;
        let luck = fract(phase * 7.31 + 0.13);
        let t = fract(time / life_s + phase);
        let born = time - t * life_s;
        // Only what was breathed while it was on: born after it came on, or before it went off.
        let breathed = select((born < since), (born >= since), (on > 0.5));
        if !breathed {
            return nowhere();
        }
        let d = reach * t;
        let up = normalize(mouth);
        var side = normalize(cross(way, up));
        if dot(side, side) < 0.5 {
            side = vec3(1.0, 0.0, 0.0);
        }
        let lift = normalize(cross(side, way));
        let a = luck * 6.283 + t * 2.0;
        // A flamethrower's stream: tight at the mouth, opening into a rolling cone, the puffs
        // turbulent about its line, slowing and lifting as they burn out.
        let r = spread * d * sqrt(fract(luck * 13.7)) * (0.6 + 0.6 * t);
        let swirl = (vec2(noise(vec2(luck * 31.0, time * 3.0 + t * 4.0)), noise(vec2(luck * 57.0 + 3.0, time * 3.0 + t * 4.0))) - 0.5) * d * 0.12;
        let at = mouth + way * d * (1.0 - 0.18 * t * t) + (side * (cos(a) * r + swirl.x) + lift * (sin(a) * r + swirl.y)) + up * t * t * reach * 0.1 + wind * t * t * life_s;
        let size = mix(0.7, 0.9 + spread * reach * 0.38, pow(t, 0.8)) * (0.7 + 0.6 * fract(luck * 5.3));
        // Streaked along the way near the mouth, round as it opens.
        let to_eye = normalize(view.world_position - at);
        var across = cross(way, to_eye);
        if dot(across, across) < 1.0e-6 {
            across = side;
        }
        across = normalize(across);
        let along = normalize(cross(to_eye, across));
        let stretch = mix(3.2, 1.0, smoothstep(0.0, 0.45, t));
        let spin = (luck - 0.5) * 0.6 * t;
        let c = corner - 0.5;
        let turned = vec2(c.x * cos(spin) - c.y * sin(spin), c.x * sin(spin) + c.y * cos(spin));
        let world = at + across * turned.x * size + along * turned.y * size * stretch;
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(t, 1.0 - t * 0.85, luck, 1.0);
        return out;
    }
    let count = u32(fire.kind.y + 0.5);
    if slot >= count {
        return nowhere();
    }
    let a = slots[slot * 2u];
    let b = slots[slot * 2u + 1u];
    let foot = a.xyz;
    let tall = a.w;
    let heat = b.x;
    let seed = b.y;
    if tall <= 0.0 {
        return nowhere();
    }
    let up = normalize(foot);
    let luck = fract(seed * 0.618 + phase * 3.71);
    if kind == 0 {
        // A flame tongue: lives as long as a fire that tall puffs (about 1.5 / sqrt(D) Hz).
        let life_s = 0.45 + 0.35 * sqrt(tall) * (0.8 + 0.4 * luck);
        let t = fract(time / life_s + phase + seed);
        let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
        let north = cross(east, up);
        let sway = (vec2(noise(vec2(luck * 17.0, time * 1.3)), noise(vec2(luck * 29.0 + 5.0, time * 1.3))) - 0.5) * tall * 0.35 * t;
        let lean = wind * 0.06 * tall * t;
        // Spread over the blaze's metre, each card its own: some a wide body low down, some a
        // tall tongue licking up.
        let spot = (vec2(hash(vec2(luck * 9.1, seed)), hash(vec2(seed, luck * 7.7))) - 0.5) * 0.9;
        let tongue = fract(luck * 4.7) > 0.45;
        let w = tall * select(0.95, 0.5, tongue) * (1.0 - 0.35 * t) * (0.75 + 0.5 * fract(luck * 2.3));
        let h = tall * select(0.6, 1.0, tongue) * (0.55 + 0.45 * t) * (0.8 + 0.4 * fract(luck * 6.1));
        let at = foot + up * tall * 0.5 * pow(t, 0.9) + east * (sway.x + spot.x) + north * (sway.y + spot.y) + lean;
        let world = card(at - up * h * 0.15, up, corner, w, h, true, (fract(luck * 3.3) - 0.5) * 0.5);
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(t, heat, luck, 1.0);
        return out;
    }
    // An ember: only some blazes throw them, the hotter the more.
    if luck > heat * 0.9 {
        return nowhere();
    }
    let life_s = 1.6 + 2.2 * luck;
    let t = fract(time / life_s + phase * 1.37 + seed);
    let seconds = t * life_s;
    let rise = (2.5 + 3.5 * luck) * (1.0 - exp(-seconds * 0.9)) / 0.9;
    let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
    let north = cross(east, up);
    let wander = vec2(noise(vec2(luck * 41.0, seconds * 0.8)), noise(vec2(luck * 73.0, seconds * 0.8))) - 0.5;
    let out_from = (vec2(hash(vec2(luck, 1.0)), hash(vec2(luck, 2.0))) - 0.5) * tall * 0.8;
    let at = foot + up * (rise + tall * 0.3) + east * (out_from.x + wander.x * 2.0 * t) + north * (out_from.y + wander.y * 2.0 * t) + wind * seconds * 0.8;
    // Stretched along its flight: up and downwind.
    let flight = normalize(up * (3.0 * exp(-seconds * 0.9) + 0.4) + wind * 0.8);
    let to_eye = normalize(view.world_position - at);
    var across = cross(flight, to_eye);
    if dot(across, across) < 1.0e-6 {
        across = cross(up, to_eye);
    }
    across = normalize(across);
    let size = 0.035 + 0.03 * luck;
    let world = at + across * (corner.x - 0.5) * size + flight * (corner.y - 0.5) * size * 5.0;
    out.clip = position_world_to_clip(world);
    out.uv = corner;
    out.life = vec4(t, heat, luck, 2.0);
    return out;
}

fn shade(uv_in: vec2<f32>, life: vec4<f32>) -> vec4<f32> {
    let kind = i32(fire.kind.x + 0.5);
    let time = fire.kind.z;
    let t = life.x;
    let heat = life.y;
    let luck = life.z;
    let uv = uv_in;
    var colour = vec3(0.0);
    var alpha = 0.0;
    if kind == 0 {
        // A tongue: wide at its foot, narrowing to a point, its edge eaten by noise that climbs
        // it and warped sideways, so it licks rather than standing as a tooth.
        let x = (uv.x - 0.5) * 2.0;
        let y = uv.y;
        let warp = (noise(vec2(uv.y * 3.0 - time * 3.1, luck * 19.0)) - 0.5) * 0.5 * y;
        let n = fbm(vec2((uv.x + warp) * 3.4 + luck * 11.0, uv.y * 2.6 - time * 3.2 - luck * 5.0));
        let fine = noise(vec2((uv.x + warp) * 9.0 + luck * 3.0, uv.y * 7.0 - time * 5.0));
        let width = (1.0 - y) * (0.7 + 0.3 * (1.0 - y)) + 0.04;
        let edge = abs(x + warp * 2.0) / max(width, 0.02) + (n - 0.5) * 1.1 + (fine - 0.5) * 0.35 + y * 0.3;
        let body = 1.0 - smoothstep(0.25, 1.05, edge);
        let foot = smoothstep(0.0, 0.1, y);
        let life = smoothstep(0.0, 0.12, t) * pow(1.0 - t, 1.2);
        alpha = body * body * foot * life * 0.9;
        let hot = (1.0 - y * 0.9) * (0.6 + 0.4 * heat) * (1.0 - 0.45 * t) + (n - 0.5) * 0.35 - abs(x) * 0.2;
        colour = glow(hot) * (0.35 + 0.65 * heat);
    } else if kind == 1 {
        let p = (uv - 0.5) * vec2(2.0, 2.0);
        let spark = 1.0 - smoothstep(0.2, 1.0, length(p));
        let wink = 0.6 + 0.4 * sin(time * (9.0 + luck * 14.0) + luck * 50.0);
        alpha = spark * pow(1.0 - t, 1.5) * wink;
        colour = glow(0.85 - t * 0.4) * 1.6;
    } else {
        // A puff of breath: rolling, broken by noise, white-hot near the mouth, orange as it
        // opens, a dull red at its end as it burns out.
        let p = uv - 0.5;
        let n = fbm(uv * 3.0 + vec2(luck * 7.0 - time * 1.5, luck * 13.0 + time * 0.8));
        let fine = noise(uv * 8.0 + vec2(luck * 3.0, time * 2.0));
        let body = 1.0 - smoothstep(0.1, 0.5, length(p) + (n - 0.5) * 0.45 + (fine - 0.5) * 0.15);
        let life = smoothstep(0.0, 0.02, t) * pow(1.0 - t, 1.8);
        alpha = body * life * mix(0.9, 0.35, t);
        colour = glow(1.1 - t * 1.0 + (n - 0.5) * 0.35) * (2.2 - t * 1.6);
    }
    if alpha < 0.003 {
        discard;
    }
    // Added to what is behind it, as light.
    return vec4(colour * alpha * NITS * view.exposure, alpha);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let placed = place(vertex.uv, vertex.uv_b);
    var out: VertexOutput;
    out.position = placed.clip;
@if(!PREPASS_PIPELINE) {
    out.world_position = vec4(0.0, 0.0, 0.0, 1.0);
    // The card's life rides where a normal would.
    out.world_normal = placed.life.xyz;
    out.uv = placed.uv;
    out.uv_b = vec2(placed.life.w, 0.0);
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

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    out.color = shade(in.uv, vec4(in.world_normal, in.uv_b.x));
    return out;
}
