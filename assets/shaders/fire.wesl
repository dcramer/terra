// Fire's flames, embers and jets (D377, crates/terra_world/src/fire.rs): cards placed here
// from each blaze's slot of a buffer, the card's seed and the time, as smoke's are (D325).
// Every corner of the mesh is at the origin; its first texture coordinate is the corner, its
// second the slot and the card's place among the slot's.
//
// A blaze's flame is a body sheet standing where it burns and flamelets breaking off its top
// (research/2026-10-16-real-time-fire-rendering.md), upright and turned to the eye about the
// up: its shape a mask eroded by gradient-noise turbulence scrolled up through it, as games'
// procedural fire is drawn, its colour glowing soot's at the temperature of that part of it,
// from 1050 K at its edges to 1650 K in a fierce one's heart (`soot`). An ember is a spark
// thrown up on the plume, slowing, carried downwind, stretched along its flight and winking
// out. A jet is a stream of flame breathed out along a way, widening in a cone, hottest at
// the mouth. All are light, premultiplied over what is behind them and covering a little of
// it as soot does, in the camera's exposure; faded in where they meet what stands behind.

import bevy_pbr::render::{
    mesh_view_bindings::view,
    view_transformations::{position_world_to_clip, depth_ndc_to_view_z},
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, VertexOutput, decompress_vertex};
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex};
@if(DEPTH_PREPASS && !PREPASS_PIPELINE)
import bevy_pbr::prepass::utils::prepass_depth;
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::mesh_view_bindings::{view_transmission_texture, view_transmission_sampler};

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
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var flipbook: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var flipbook_sampler: sampler;
// What breaths have let go: soot, then drops, each where and when, then its going and luck.
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var<storage, read> puffs: array<vec4<f32>>;
// How many soot puffs come before the drops; must match SOOT_PUFFS in fire.rs.
const SOOT_PUFFS: u32 = 320u;
// How long a drop burns as it falls, in seconds.
const DROP_S: f32 = 2.0;

// The flipbook's frames, how many across its sheet, and how many it plays a second for a
// flame two and a half metres tall (`tools/textures/flames.ts`).
const FLIPBOOK_FRAMES: f32 = 64.0;
const FLIPBOOK_COLUMNS: f32 = 8.0;
const FLIPBOOK_FPS: f32 = 30.0;

// A frame of the flipbook at a place on its card, x across and y up, kept a hair inside the
// frame so its neighbours never bleed in.
fn flipbook_at(frame: f32, x: f32, y: f32) -> vec4<f32> {
    let column = frame % FLIPBOOK_COLUMNS;
    let row = floor(frame / FLIPBOOK_COLUMNS);
    let rows = FLIPBOOK_FRAMES / FLIPBOOK_COLUMNS;
    let at = vec2((column + clamp(x, 0.01, 0.99)) / FLIPBOOK_COLUMNS, (row + 1.0 - clamp(y, 0.01, 0.99)) / rows);
    return textureSampleLevel(flipbook, flipbook_sampler, at, 0.0);
}

// A card's corner placed: where on screen, the corner across and up the card, how far
// through its life, how hot, its luck and what it is, and how tall it is, in metres.
struct Out {
    clip: vec4<f32>,
    uv: vec2<f32>,
    life: vec4<f32>,
    size: f32,
};

// How bright glowing soot is taken to be, on the luminances `soot` works from.
const LUMINANCE: f32 = 1.0;
// How far over the tonemapper's knee glowing soot is kept, by the camera's exposure. Eye
// adaptation is applied after the scene is drawn (Bevy's auto exposure, in the tonemapping
// pass), so a flame kept under the knee of `view.exposure` alone was darkened with the rest
// of a sunlit frame by up to the stop the day lets it darken (scene.rs, ADAPT_DARKER): a
// shouldered yellow at half its brightness, the olive of castle film r5 and r6 (flame hearts
// at sRGB 162,149,89). One stop of headroom keeps its heart a full yellow then.
const HEADROOM: f32 = 2.0;
// How far in front of what is behind it a card fades in, in metres, so it never shows a
// straight edge where it goes into a wall or the ground: a flame's, an ember's, a jet's.
const SOFT_M: vec3<f32> = vec3<f32>(0.35, 0.1, 0.5);

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

// A gradient for a lattice point, and gradient noise from them, about -0.7 to 0.7: smoother
// than value noise, without its blocky grain.
fn gradient(p: vec2<f32>) -> vec2<f32> {
    let q = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
    return -1.0 + 2.0 * fract(sin(q) * 43758.5453);
}

fn gnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = dot(gradient(i), f);
    let b = dot(gradient(i + vec2(1.0, 0.0)), f - vec2(1.0, 0.0));
    let c = dot(gradient(i + vec2(0.0, 1.0)), f - vec2(0.0, 1.0));
    let d = dot(gradient(i + vec2(1.0, 1.0)), f - vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}


// Glowing soot at a temperature and in an amount (how much of it there is along the eye's
// ray, 0 to 1), as the camera sees it: its colour from soot's emission at that temperature
// (red, with green rising from 0.04 at 1000 K to 0.21 at 1600 K, never the yellow or white of
// the 3000 K colours a wood flame has not got), its luminance from about 50 nits at 1100 K to
// 1150 at 1400 and 5700 at 1650, a sunlit white's 0.005 to 0.6, times the amount; then kept
// under the tonemapper's knee per channel, as film shoulders it, with a white-hot lift only
// where it is far brighter than the exposure, so a fire by day stays orange with a yellow
// heart and never goes salmon pink. The amount is taken before the shoulder, as a camera
// takes light: a thin or dim flame is dull red, since only red is near the knee, and goes
// yellow only where there is light enough to lift its green there. Shouldered first and then
// thinned, a dim flame kept its hot one's yellow at a fraction of its brightness, which over
// charred timber and smoke is olive (castle film r5: hue 50-60 at sRGB 153,148,92).
fn soot(kelvin: f32, amount: f32) -> vec3<f32> {
    let g = clamp(0.042 + 0.000278 * (kelvin - 1000.0), 0.03, 0.26);
    let slope = select(0.0105, 0.0064, kelvin > 1400.0);
    let nits = exp(7.05 + (kelvin - 1400.0) * slope) * LUMINANCE * amount;
    let unit = 0.2126 + 0.7152 * g + 0.0722 * 0.02;
    let exposed = vec3(1.0, g, 0.02) * (nits / unit) * view.exposure;
    let kept = HEADROOM * (vec3(1.0) - exp(-exposed / HEADROOM));
    let lift = 0.5 * saturate((nits * view.exposure - 3.0) / 10.0);
    return kept + (vec3(HEADROOM) - kept) * lift;
}

// How much of what is behind it a flame covers, for what it gives and its alpha. Its light is
// already shouldered, so its red is near one wherever it is hot and only its green grows: a
// hot yellow over a blue sky at half cover summed to more green than red, a pale olive. So
// the yellower it glows the more it covers, keeping red over green over blue as a camera
// sees fire; a dull red flame stays thin, as it is.
fn covers(light: vec3<f32>, alpha: f32) -> f32 {
    let yellow = light.g / max(light.r, 1.0e-4);
    return alpha * mix(0.4, 0.97, smoothstep(0.2, 0.75, yellow));
}

fn nowhere() -> Out {
    var out: Out;
    out.clip = vec4(0.0, 0.0, -1.0, 1.0);
    out.uv = vec2(0.0);
    out.life = vec4(0.0);
    out.size = 0.0;
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
    // Upright cards lean back to face an eye looking down on them, as they would show their
    // edges otherwise: from over a burning roof, a wall of crossed edges.
    let facing_eye = cross(to_eye, right);
    var card_up = normalize(mix(up, facing_eye, smoothstep(0.3, 0.75, abs(dot(to_eye, up)))));
    if !upright {
        card_up = facing_eye;
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
    if kind == 4 {
        // A burning drop of the breath's fuel, falling from where it left the stream: going
        // on as the stream went, slowed by the air, and falling, stretched along its flight
        // as a streak (research/2026-10-18-dragon-breath.md).
        let i = (SOOT_PUFFS + slot) * 2u;
        let born = puffs[i];
        let going = puffs[i + 1u];
        let s = time - born.w;
        if s < 0.0 || s > DROP_S {
            return nowhere();
        }
        let up = normalize(born.xyz);
        let drag = (1.0 - exp(-s * 1.2)) / 1.2;
        let at = born.xyz + going.xyz * drag - up * 4.9 * s * s;
        let flight = going.xyz * exp(-s * 1.2) - up * 9.8 * s;
        let to_eye = normalize(view.world_position - at);
        var along = flight - to_eye * dot(flight, to_eye);
        if dot(along, along) < 1.0e-6 {
            along = up;
        }
        // Long enough to read as a falling streak forty metres off, as drops of burning fuel
        // do in footage, smeared by the camera's shutter.
        let streak = clamp(length(along) * 0.09, 0.6, 3.2);
        along = normalize(along);
        let across = normalize(cross(along, to_eye));
        let size = 0.28 + 0.2 * going.w;
        let world = at + across * (corner.x - 0.5) * size + along * (corner.y - 0.8) * streak;
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(s / DROP_S, 1.0, going.w, 5.0);
        out.size = size;
        return out;
    }
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
        // Slower than the 34 m/s a flamethrower's fuel leaves at, so each puff lives long
        // enough on screen for the stream to read as a thick rope of fire, not a wisp.
        let speed = 24.0;
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
        // turbulent about its line, slowing and lifting as they burn out; where it strikes it
        // splashes, its last puffs stopped short of what it struck and thrown out across it
        // (as Unreal's and Frostbite's flamethrowers end in an impact burst on the surface).
        let splash = smoothstep(0.72, 1.0, t);
        let r = spread * d * sqrt(fract(luck * 13.7)) * (0.6 + 0.6 * t) + splash * (1.0 + 0.18 * reach) * sqrt(fract(luck * 3.9));
        let swirl = (vec2(noise(vec2(luck * 31.0, time * 3.0 + t * 4.0)), noise(vec2(luck * 57.0 + 3.0, time * 3.0 + t * 4.0))) - 0.5) * d * 0.12;
        let at = mouth + way * min(d * (1.0 - 0.08 * t * t), reach - 1.0) + (side * (cos(a) * r + swirl.x) + lift * (sin(a) * r + swirl.y)) + up * t * t * reach * 0.1 + wind * t * t * life_s;
        // The rod at the mouth thin and long, a rope of fire, opening into the rolling body.
        let rod = 1.0 - smoothstep(0.0, 0.3, t);
        let size = (mix(0.9, 1.3 + spread * reach * 0.7, pow(t, 0.7)) + splash * 1.2) * (0.75 + 0.5 * fract(luck * 5.3)) * (1.0 - 0.45 * rod);
        // Streaked along the way near the mouth, round as it opens.
        let to_eye = normalize(view.world_position - at);
        var across = cross(way, to_eye);
        if dot(across, across) < 1.0e-6 {
            across = side;
        }
        across = normalize(across);
        let along = normalize(cross(to_eye, across));
        let stretch = mix(5.5, 1.0, smoothstep(0.0, 0.5, t));
        let spin = (luck - 0.5) * 0.6 * t;
        let c = corner - 0.5;
        let turned = vec2(c.x * cos(spin) - c.y * sin(spin), c.x * sin(spin) + c.y * cos(spin));
        let world = at + across * turned.x * size + along * turned.y * size * stretch;
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        // Once it is off, what was breathed burns out in a moment where it is, rather than
        // flying on its course as a cloud cut loose from the dragon (castle film r5, 12.75 s).
        let gone = select(saturate(1.0 - (time - since) / 0.3), 1.0, on > 0.5);
        out.life = vec4(t, gone, luck, 1.0);
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
    let broad = b.z;
    if tall <= 0.0 {
        return nowhere();
    }
    let up = normalize(foot);
    let luck = fract(seed * 0.618 + phase * 3.71);
    if kind == 3 {
        // Heat haze: a card over the flame, from half its height to twice it, through which
        // what is behind shimmers.
        let w = tall * 1.5;
        let h = tall * 1.6;
        let world = card(foot + up * tall * 0.45, up, corner, w, h, true, 0.0);
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(0.0, heat, luck, 4.0);
        out.size = h;
        return out;
    }
    if kind == 0 {
        // A burning surface's flame (research/2026-10-16-real-time-fire-rendering.md): a few
        // cards of a simulated fire's flipbook standing where it burns, a big one over the
        // blaze and two smaller either side of it, each its own fire and its own moment. The
        // flipbook's flame fills about the lower two thirds of its card, so the cards stand
        // half again as tall as the flame. Its height breathes at the puffing rate of a fire
        // that wide, 1.5 / sqrt(D) Hz.
        // Blazes stand on a two-metre grid: each is put off its cell's middle by up to most
        // of a metre and made taller or shorter by its own seed, so a burning roof is a fire
        // and not rows of the same flame.
        let own = vec3(fract(seed * 12.9898), fract(seed * 78.233), fract(seed * 37.719));
        // A broad surface burning as one blaze is a sheet of fire its own width.
        let wide = select(clamp(tall * 0.8, 0.8, 2.8), broad, broad > 0.0);
        let puff_hz = 1.5 / sqrt(wide);
        let breathe = 1.0 + 0.15 * sin(time * puff_hz * 6.283 + seed * 40.0) * (0.6 + 0.4 * sin(time * puff_hz * 2.7 + seed * 9.0));
        // A kept fire is as tall as it was made, its flames sized to clear its own logs; a
        // grid's blazes each a little taller or shorter.
        let flame = tall * breathe * mix(0.65 + 0.7 * own.z, 1.0, b.w);
        let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
        let north = cross(east, up);
        let lean = wind * 0.05 * flame;
        var w = wide * 1.9;
        var h = flame * 1.75;
        // A kept fire (a campfire) stands just where it is, not off a grid cell's middle.
        var at = foot + (east * (own.x - 0.5) + north * (own.y - 0.5)) * max(1.5, wide * 0.45) * (1.0 - b.w);
        if phase > 0.01 {
            let side = select(-1.0, 1.0, phase > 0.5);
            w = wide * 1.2;
            h = flame * 1.3;
            at = foot + east * side * wide * (0.25 + 0.2 * luck) + north * (fract(luck * 3.1) - 0.5) * wide * 0.5;
        }
        let world = card(at - up * h * 0.04 + lean * 0.2, up, corner, w, h, true, 0.0);
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(0.5, heat, luck, 1.0);
        out.size = h;
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

// `soft` is how far it has faded in front of what is behind it: taken as less soot glowing,
// so a flame meeting the roof it stands on goes dull red there rather than a dim yellow.
fn shade(uv_in: vec2<f32>, life: vec4<f32>, size: f32, soft: f32) -> vec4<f32> {
    let kind = i32(fire.kind.x + 0.5);
    let time = fire.kind.z;
    let t = life.x;
    let heat = life.y;
    let luck = life.z;
    let uv = uv_in;
    // What it gives, already exposed, and how much of what is behind it it covers.
    var light = vec3(0.0);
    var cover = 0.0;
    if kind == 0 {
        // A simulated fire's frame from the flipbook (`tools/textures/flames.ts`): one of its
        // three fires by the card's luck, mirrored for half of them, played at the rate a fire
        // this tall moves (a flame's time goes as the root of its size, Froude's scaling),
        // blended frame to frame. Its heat is how hot the soot there glows and how much of the
        // card it covers.
        let channel = u32(luck * 3.0) % 3u;
        let x = select(uv.x, 1.0 - uv.x, fract(luck * 7.3) > 0.5);
        let rate = FLIPBOOK_FPS * sqrt(2.5 / max(size, 0.5));
        let f = fract(time * rate / FLIPBOOK_FRAMES + luck * 5.17) * FLIPBOOK_FRAMES;
        let f0 = floor(f);
        let f1 = (f0 + 1.0) % FLIPBOOK_FRAMES;
        let a = flipbook_at(f0, x, uv.y)[channel];
        let b = flipbook_at(f1, x, uv.y)[channel];
        let heat_here = mix(a, b, fract(f)) * smoothstep(0.0, 0.03, uv.y);
        // The flipbook's body is near its full heat, so only its hottest cores go yellow: the
        // rest orange, its rims and licks a dull red.
        let alpha = smoothstep(0.08, 0.4, heat_here) * soft;
        let kelvin = 950.0 + 700.0 * pow(heat_here, 1.5) * (0.72 + 0.28 * heat);
        light = soot(kelvin, alpha);
        cover = covers(light, alpha);
    } else if kind == 1 {
        let p = (uv - 0.5) * vec2(2.0, 2.0);
        let spark = 1.0 - smoothstep(0.2, 1.0, length(p));
        let wink = 0.6 + 0.4 * sin(time * (9.0 + luck * 14.0) + luck * 50.0);
        let alpha = spark * pow(1.0 - t, 1.5) * wink * soft;
        light = soot(1600.0 - t * 300.0, alpha);
    } else if kind == 4 {
        // A drop: a hot bright streak, its head hottest, cooling to red as it burns out.
        let x = abs(uv.x - 0.5) * 2.0;
        let streak = (1.0 - smoothstep(0.3, 1.0, x)) * smoothstep(0.0, 0.7, uv.y) * (1.0 - smoothstep(0.85, 1.0, uv.y));
        let wink = 0.75 + 0.25 * sin(time * (11.0 + luck * 9.0) + luck * 40.0);
        let alpha = streak * (1.0 - smoothstep(0.6, 1.0, t)) * wink * soft;
        light = soot(mix(1640.0, 1300.0, t) + 80.0 * uv.y, alpha);
        cover = covers(light, alpha);
    } else {
        // A puff of breath, as a flamethrower's stream is in footage and in games that do it
        // well (research/2026-10-18-dragon-breath.md): what reads is the contrast inside it.
        // A frame of the simulated fire (`tools/textures/flames.ts`), its tongues laid along
        // the stream, the card's foot toward the mouth, under a soft round mask broken by
        // noise at its rim. Its heat there sets both how hot it glows and how much it covers:
        // a rod near-white at the mouth; yellow cores with orange round them and dull red at
        // the rims as it rolls open; pockets of black soot in it where it burns rich, more as
        // it burns out; and white-hot again where it strikes and splashes.
        let p = uv - 0.5;
        let n = fbm(uv * 3.0 + vec2(luck * 7.0 - time * 1.5, luck * 13.0 + time * 0.8));
        let edge = length(p) + (n - 0.5) * 0.35;
        // Nothing at the card's own edges, so its straight sides never show.
        let window = 1.0 - smoothstep(0.32, 0.5, max(abs(p.x), abs(p.y)));
        let body = (1.0 - smoothstep(0.1, 0.46, edge)) * window;
        let channel = u32(luck * 3.0) % 3u;
        let x = select(uv.x, 1.0 - uv.x, fract(luck * 7.3) > 0.5);
        let f = fract(time * FLIPBOOK_FPS * 1.6 / FLIPBOOK_FRAMES + luck * 5.17) * FLIPBOOK_FRAMES;
        let f0 = floor(f);
        let f1 = (f0 + 1.0) % FLIPBOOK_FRAMES;
        // The flame's column above its fuel bed, its middle half across.
        let fx = 0.5 + (x - 0.5) * 0.6;
        let fy = 0.1 + uv.y * 0.62;
        let tongue = mix(flipbook_at(f0, fx, fy)[channel], flipbook_at(f1, fx, fy)[channel], fract(f));
        let mouth = 1.0 - smoothstep(0.0, 0.3, t);
        let struck = smoothstep(0.7, 0.92, t) * (1.0 - smoothstep(0.92, 1.0, t));
        let heat_here = body * saturate(mix(0.2, 0.02, smoothstep(0.0, 0.5, t)) + 0.45 * mouth + 0.2 * struck + 0.85 * tongue);
        let pocket = body * smoothstep(0.2, 0.8, t) * (1.0 - smoothstep(0.04, 0.3, tongue)) * smoothstep(0.42, 0.62, n);
        let fade = smoothstep(0.0, 0.02, t) * (1.0 - smoothstep(0.82, 1.0, t));
        let alpha = smoothstep(0.05, 0.4, heat_here) * fade * heat * soft;
        let kelvin = 1060.0 + 640.0 * pow(heat_here, 1.3) + 200.0 * mouth - 110.0 * t + 90.0 * struck;
        light = soot(kelvin, alpha) * (1.0 - 0.9 * pocket);
        // The rod overexposed, white-yellow as a flamethrower's is on film, and solid.
        let rod = mouth * alpha;
        light = mix(light, vec3(1.0, 0.92, 0.66) * HEADROOM, rod * 0.85);
        // A stream of thickened fuel is dense: it covers more than a building's thin flames.
        cover = max(saturate(mix(covers(light, alpha), alpha, 0.5) + pocket * fade * heat * soft * 0.8), rod * 0.95);
    }
    if max(light.r, cover) < 0.002 {
        discard;
    }
    return vec4(light, cover);
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
    out.uv_b = vec2(placed.life.w, placed.size);
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

// What is behind a heat haze card, shimmering: the opaque scene as Bevy copies it for
// transmissive materials, read a few pixels off (0.2 to 0.5% of the screen's width) by
// noise rising at a metre or two a second, as games draw heat haze; the most a third of the
// way up the card, none at its edges.
@if(!PREPASS_PIPELINE)
fn haze(in: VertexOutput) -> vec4<f32> {
    let time = fire.kind.z;
    let heat = in.world_normal.y;
    let rise = 1.5 / max(in.uv_b.y, 0.5);
    let p = in.uv - vec2(0.5, 0.35);
    let mask = (1.0 - smoothstep(0.15, 0.5, length(p * vec2(1.0, 0.75)))) * (0.4 + 0.6 * heat);
    if mask < 0.01 {
        discard;
    }
    let q = in.uv * vec2(5.0, 4.0) - vec2(0.0, time * rise * 4.0);
    let off = vec2(gnoise(q), gnoise(q + vec2(5.2, 1.3))) * 0.006 * mask;
    let screen = in.position.xy / view.viewport.zw;
    let behind = textureSampleLevel(view_transmission_texture, view_transmission_sampler, screen + off, 0.0).rgb;
    return vec4(behind, mask);
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    if i32(fire.kind.x + 0.5) == 3 {
        out.color = haze(in);
        return out;
    }
    // Faded in over the last of its way in front of what is behind it (soft particles).
    var soft = 1.0;
@if(DEPTH_PREPASS)
    {
        let kind = i32(fire.kind.x + 0.5);
        let reach = select(select(SOFT_M.z, SOFT_M.y, kind == 1 || kind == 4), SOFT_M.x, kind == 0);
        let behind = depth_ndc_to_view_z(in.position.z) - depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        soft = smoothstep(0.0, reach, behind);
    }
    out.color = shade(in.uv, vec4(in.world_normal, in.uv_b.x), in.uv_b.y, soft);
    return out;
}
