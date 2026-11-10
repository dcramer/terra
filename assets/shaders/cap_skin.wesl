// A cap tree's skin (`terra_world::grown::caps`, docs/realm-two.md), where bark would be:
// its cap's top, its gills, its stalk and its ring, each made here from what a great
// fungus's are (research/2026-11-09-great-fungi.md): a top darker at its middle and paler
// and lined at its rim, streaked along its radius, mottled, with pale warts, the scraps of
// the veil it broke through, thick at its middle and shed in patches; true gills under it,
// short ones set between the long toward the rim, two for one and then four, dark between;
// a stalk of long fibres banded in darker scales as a parasol's is; a pleated ring.
//
// What a point is, its mesh says in its texture coordinates (`caps::cap_mesh`,
// `caps::stalk_mesh`): x how far round, 0 to 1; y below zero, -1 to -2 a cap's gills from
// its neck to its rim, -2 to -3 its top from its rim to its crown, from -4 down a stalk by
// its metres up, from -100 a ring out along its skirt and back under it.

import super::hue::hue_turned;

// How many long gills round a cap; the short ones double them twice.
const GILLS: f32 = 96.0;
const TURN: f32 = 6.2831853;

struct CapSkin {
    colour: vec3<f32>,
    rough: f32,
    // What its gills shine, to be added to its light.
    glow: vec3<f32>,
    // How far its surface stands out, in metres, for its relief (`cap_relief`).
    height: f32,
    // How much of the sky's light reaches into it.
    open: f32,
}

fn is_cap_skin(uv: vec2<f32>) -> bool {
    return uv.y < -0.5;
}

fn skin_hash(p: vec2<f32>) -> f32 {
    let q = fract(vec3<f32>(p.xyx) * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

// Smooth noise, 0 to 1, that comes round on itself every `period` in x: a cap's or a
// stalk's turn has no seam.
fn skin_round(p: vec2<f32>, period: f32) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x0 = i.x - period * floor(i.x / period);
    let x1 = (i.x + 1.0) - period * floor((i.x + 1.0) / period);
    let a = mix(skin_hash(vec2<f32>(x0, i.y)), skin_hash(vec2<f32>(x1, i.y)), u.x);
    let b = mix(skin_hash(vec2<f32>(x0, i.y + 1.0)), skin_hash(vec2<f32>(x1, i.y + 1.0)), u.x);
    return mix(a, b, u.y);
}

// Smooth noise, 0 to 1, over a plane.
fn skin_noise(p: vec2<f32>) -> f32 {
    return skin_round(p, 4096.0);
}

// Warts: how far inside the nearest one of a scatter a point is, 0 outside to 1 at its
// middle, and that wart's own number. One point a cell, the nine cells round searched
// (Worley's cellular noise, as The Book of Shaders' chapter 12 builds it).
fn warts(p: vec2<f32>) -> vec2<f32> {
    let cell = floor(p);
    var best = vec2<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let other = cell + vec2<f32>(f32(x), f32(y));
            let own = skin_hash(other + 17.0);
            let at = other + 0.5 + 0.7 * (vec2<f32>(skin_hash(other), skin_hash(other + 71.3)) - 0.5);
            // Each its own size, a third of a cell across at most.
            let across = 0.1 + 0.24 * own;
            let inside = 1.0 - length(p - at) / across;
            if inside > best.x {
                best = vec2<f32>(inside, own);
            }
        }
    }
    return best;
}

// One level of gills: thin pale blades `count` round, 1 on a blade's edge and 0 between,
// thinned to their mean where they are finer than a pixel.
fn blades(round: f32, count: f32, pixel: f32) -> f32 {
    let wave = 0.5 + 0.5 * cos(round * count * TURN);
    let blade = wave * wave * wave;
    return mix(blade, 0.31, clamp(pixel * count * 1.5, 0.0, 1.0));
}

// `tint` its vertex's colour, a cap's or a stalk's; `stood` the point from the tree's foot,
// in metres; `gills` their colour and how brightly they shine; `turn` how far its realm
// turns its plants' hues after this, which what is ivory or brown here is turned back by,
// to be seen so.
fn cap_skin(tint: vec3<f32>, uv: vec2<f32>, stood: vec3<f32>, gills: vec4<f32>, turn: f32) -> CapSkin {
    var skin: CapSkin;
    let pale = hue_turned(vec3<f32>(0.86, 0.8, 0.7), -turn);
    let brown = hue_turned(vec3<f32>(0.5, 0.42, 0.36), -turn);
    skin.glow = vec3<f32>(0.0);
    skin.open = 1.0;
    let round = uv.x;
    // How much of a turn and of a metre a pixel covers.
    let pixel = fwidth(round);
    let metre = length(fwidth(stood));
    let broad = skin_noise(stood.xz * 0.3 + stood.y * 0.21 + 5.0);
    let middling = skin_noise(stood.xz * 1.9 + stood.y * 1.3);

    if uv.y < -50.0 {
        // Its ring: a pale pleated membrane, browned and ragged at its edge, darker under.
        let along = -uv.y - 100.0;
        let edge = smoothstep(0.55, 1.0, 1.0 - abs(along - 1.0));
        let pleat = mix(0.5 + 0.5 * cos(round * 56.0 * TURN + 3.0 * middling), 0.5, clamp(pixel * 84.0, 0.0, 1.0));
        let under = smoothstep(1.02, 1.1, along);
        skin.colour = tint * (0.62 + 0.3 * pleat) * mix(1.0, 0.55, edge * (0.5 + 0.5 * middling)) * mix(1.0, 0.6, under);
        skin.rough = 0.85;
        skin.height = pleat * 0.02;
        return skin;
    }
    if uv.y < -3.5 {
        // Its stalk: long fibres, banded below in darker scales torn apart as it grew (a
        // parasol's snakeskin), soiled at its foot.
        let up = -uv.y - 4.0;
        let coarse = skin_round(vec2<f32>(round * 46.0, up * 0.35), 46.0);
        let fibre = mix(skin_round(vec2<f32>(round * 150.0, up * 0.9 + coarse), 150.0), 0.5, clamp(pixel * 220.0, 0.0, 1.0));
        let torn = skin_round(vec2<f32>(round * 16.0, up * 2.3 + 2.5 * coarse), 16.0);
        let scale = smoothstep(0.52, 0.62, torn) * (0.35 + 0.65 * broad);
        let soiled = 1.0 - smoothstep(0.0, 1.6, up + broad);
        skin.colour = tint * (0.5 + 0.22 * coarse + 0.2 * fibre) * mix(vec3<f32>(1.0), brown, scale * 0.75) * (1.0 - 0.5 * soiled);
        skin.rough = 0.8;
        skin.height = fibre * 0.012 + coarse * 0.02 + scale * 0.015;
        return skin;
    }

    let along = -uv.y - 1.0;
    // Its top, where past its rim's lip; how far out from its middle a point is.
    let top = smoothstep(0.975, 0.995, along);
    let out = 1.0 - abs(along - 1.0);

    // Its top: darker and lighter in broad patches, with pale warts, fewer toward its rim.
    let wart = smoothstep(0.8, 0.86, skin_noise(stood.xz * 2.6));
    let above = mix(tint * (0.3 + 0.3 * broad), mix(tint, vec3<f32>(1.0), 0.45) * 0.7, wart);
    // Its gills, from the stalk to the rim, each a ridge with shade between.
    let gill = mix(0.5 + 0.5 * cos(round * 140.0 * TURN), 0.5, clamp(pixel * 210.0, 0.0, 1.0));
    skin.colour = mix(gills.rgb * (0.25 + 0.75 * gill), above, top);
    skin.rough = mix(0.9, 0.5, top);
    skin.height = 0.0;
    skin.glow = gills.rgb * gills.a * (0.2 + 0.8 * gill) * (1.0 - top);
    return skin;
}

// A surface's normal with a height's relief on it, where the surface has no tangents to
// carry a normal map: from the height's and the point's change across the screen (Morten
// Mikkelsen, "Bump Mapping Unparametrized Surfaces on the GPU", 2010). `height` and `at` in
// one measure, metres; gone where a pixel covers more than the relief is across.
fn cap_relief(normal: vec3<f32>, at: vec3<f32>, height: f32) -> vec3<f32> {
    let across = dpdx(at);
    let down = dpdy(at);
    let r1 = cross(down, normal);
    let r2 = cross(normal, across);
    let det = dot(across, r1);
    let slope = sign(det) * (dpdx(height) * r1 + dpdy(height) * r2);
    let fade = 1.0 - smoothstep(0.02, 0.08, length(fwidth(at)));
    return normalize(abs(det) * normal - slope * fade);
}
