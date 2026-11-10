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
// middle (below 0 round it, out to its foot), and that wart's own number. One point a cell
// (Worley's cellular noise, as The Book of Shaders' chapter 12 builds it), in space, not on
// a plane, so a bell's steep side wears them round as its crown does; the eight cells
// nearest searched, which hold every wart that can reach, none being wider than 0.34 of a
// cell nor further than 0.35 from its cell's middle.
fn warts(p: vec3<f32>) -> vec2<f32> {
    let cell = floor(p - 0.5);
    var best = vec2<f32>(-1.0, 0.0);
    for (var z = 0; z <= 1; z++) {
        for (var y = 0; y <= 1; y++) {
            for (var x = 0; x <= 1; x++) {
                let other = cell + vec3<f32>(f32(x), f32(y), f32(z));
                let flat = other.xy + other.z * vec2<f32>(37.0, 59.0);
                let own = skin_hash(flat + 17.0);
                let jitter = vec3<f32>(skin_hash(flat), skin_hash(flat + 71.3), skin_hash(flat + 113.7));
                let at = other + 0.5 + 0.7 * (jitter - 0.5);
                let across = 0.1 + 0.24 * own;
                let inside = 1.0 - length(p - at) / across;
                if inside > best.x {
                    best = vec2<f32>(inside, own);
                }
            }
        }
    }
    return best;
}

// One level of gills: thin pale blades `count` round, 1 on a blade's edge and 0 between,
// thinned to their mean where they are finer than a pixel.
fn blades(round: f32, count: f32, pixel: f32) -> f32 {
    let wave = 0.5 + 0.5 * cos(round * count * TURN);
    let blade = wave * wave;
    return mix(blade, 0.375, clamp(pixel * count * 1.5, 0.0, 1.0));
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

    // Its top. Darker at its middle; paler toward its rim, where it dries first; faint
    // rings; streaked along its radius, and lined at its rim over the gills.
    let ringed = 0.5 + 0.5 * cos(out * 34.0 + 5.0 * skin_round(vec2<f32>(round * 9.0, out * 2.0), 9.0));
    let streak = mix(skin_round(vec2<f32>(round * 260.0, out * 7.0), 260.0), 0.5, clamp(pixel * 390.0, 0.0, 1.0));
    let lined = blades(round, GILLS * 2.0, pixel) * smoothstep(0.8, 0.98, out);
    var shade = mix(0.3, 0.52, smoothstep(0.05, 0.6, out)) * (0.72 + 0.56 * broad) * (0.9 + 0.2 * middling);
    shade *= 1.0 + 0.1 * (ringed - 0.5) + 0.3 * (streak - 0.5) * (0.4 + 0.6 * out) - 0.22 * lined;
    let bloom = smoothstep(0.7, 1.0, out) * (0.3 + 0.4 * middling);
    var above = mix(tint * shade, pale * 0.5, bloom * 0.45);
    // Its warts: thick at its middle, few at its rim, shed in patches, each a raised pale
    // scrap with shade at its foot; gone where one is finer than a pixel.
    let wart = warts(stood * 2.3);
    let kept = step(wart.y, mix(1.0, 0.3, out) * smoothstep(0.25, 0.55, broad + 0.25 * middling));
    let raised = smoothstep(0.0, 0.35, wart.x) * kept * (1.0 - smoothstep(0.25, 0.6, metre * 2.3));
    let foot = smoothstep(-0.5, 0.0, wart.x) * kept * (1.0 - raised);
    above = mix(above * (1.0 - 0.3 * foot), pale * (0.42 + 0.2 * wart.y + 0.12 * middling), raised);
    // Wet, as a fresh cap is, where it is not wart.
    let above_rough = mix(mix(0.22, 0.5, broad), 0.9, raised);
    let above_height = raised * 0.04 + streak * 0.006 + middling * 0.03;

    // Its gills: long blades from the stalk out, a short one set between each two past
    // two fifths of the way and again past seven tenths, each wandering a little.
    let wander = (skin_round(vec2<f32>(round * 30.0, out * 5.0), 30.0) - 0.5) * 0.008;
    let turned = round + wander;
    var gill = blades(turned, GILLS, pixel);
    gill = mix(gill, blades(turned, GILLS * 2.0, pixel), smoothstep(0.36, 0.5, out));
    gill = mix(gill, blades(turned, GILLS * 4.0, pixel), smoothstep(0.66, 0.8, out));
    // A blade's edge is ragged and paler; between two it is in shade, deepest by the stalk.
    let ragged = 0.75 + 0.25 * skin_noise(vec2<f32>(turned * GILLS * 4.0, out * 40.0));
    let deep = mix(0.6, 1.0, smoothstep(0.1, 0.9, out));
    let below = gills.rgb * mix(0.2, 0.85 * ragged, gill) * deep;
    // Their light is in the dark between them, by the stalk, in patches: sparing.
    let lit = (1.0 - gill) * pow(1.0 - out, 1.5) * smoothstep(0.3, 0.7, broad) * (0.3 + 0.7 * middling);

    skin.colour = mix(below, above, top);
    skin.rough = mix(0.85, above_rough, top);
    skin.height = mix(gill * 0.06, above_height, top);
    // In their own hue, deeper: twice as far from grey as they are by day.
    let grey = vec3<f32>((gills.r + gills.g + gills.b) / 3.0);
    let shine = max(grey + (gills.rgb - grey) * 2.2, vec3<f32>(0.0));
    skin.glow = shine * gills.a * lit * (1.0 - top);
    skin.open = mix(mix(0.4, 1.0, gill) * deep, 1.0, top);
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
