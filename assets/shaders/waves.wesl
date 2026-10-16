// Waves on water (D322), the lakes' (lake.wesl) and the sea's (water.wesl): trains of plane
// waves summed, as Finch's "Effective Water Simulation from Physical Models" (GPU Gems, 2004)
// sums them, each running one fixed way at its own speed by water's dispersion (waves.rs
// `trains`), the wind making those that run with it stronger, as a spectrum's spreading does
// (Tessendorf, "Simulating Ocean Water", 2001); as steep as Cox and Munk measured the sea's
// slopes by the wind (1954); and filtered by the pixel's footprint, what a pixel cannot hold
// widening the sun's glint, as Bruneton, Neyret and Holzschuch's ocean does ("Real-time
// Realistic Ocean Lighting using Seamless Transitions from Geometry to BRDF", 2010).
//
// A surface sums them itself, from its own uniform, a train at a time (`wave_train`):
//
//     var face = Surface(vec2<f32>(0.0), 0.0, 0.0);
//     let bent = wave_bend(at, anchored, up, t);
//     for (var i = 0u; i < TRAINS; i += 1u) {
//         wave_train(&face, trains[i], i < SWELL, bent, pixel, downwind, spread, slopes, t);
//     }

// How many trains, the first SWELL of them the long ones (waves.rs).
const TRAINS: u32 = 28u;
const SWELL: u32 = 11u;

// The water's surface at a place: its slope east and north, its height (metres), and the mean
// square slope of the waves too short for the pixel to show, which the light spreads instead.
struct Surface {
    slope: vec2<f32>,
    height: f32,
    unseen: f32,
}

// A lattice point's value, by Jarzynski and Olano's pcg3d ("Hash Functions for GPU
// Rendering", 2020) on its whole coordinates: exact at any distance from the water's origin,
// with none of a hash of float products' rounding or patterns.
fn hash(p: vec3<f32>) -> f32 {
    var v = bitcast<vec3<u32>>(vec3<i32>(p));
    v = v * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v = v ^ (v >> vec3<u32>(16u));
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return f32(v.x >> 8u) / 16777216.0;
}

fn noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    // Perlin's quintic fade, smooth in its curvature too: with the cubic the ripples' normals
    // turn sharply at every cell's edge, and still water's reflection draws the lattice.
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let x00 = mix(hash(cell), hash(cell + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash(cell + vec3<f32>(0.0, 1.0, 0.0)), hash(cell + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash(cell + vec3<f32>(0.0, 0.0, 1.0)), hash(cell + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash(cell + vec3<f32>(0.0, 1.0, 1.0)), hash(cell + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// How ruffled the water is, 0 glassy to 1: a light wind ruffles it in patches, cat's-paws a few
// tens of metres across racing downwind as the gusts do and changing as they go, with glassy
// water between that mirrors the shore, as a mountain lake in the morning calm is (Morskie
// Oko's photographs); the stiffer the wind (its w, 0 to 1), the more of it, all of it in a
// breeze. `anchored` is a place on the water fixed in the world, in metres.
fn cats_paws(anchored: vec3<f32>, wind: vec4<f32>, up: vec3<f32>, t: f32) -> f32 {
    let gusting = wind.xyz * (1.5 + 4.0 * wind.w) + up * 0.4;
    let paws = noise(anchored * 0.018 - gusting * t * 0.018 + 5.0) * 0.65
        + noise(anchored * 0.05 - gusting * t * 0.05 + 19.0) * 0.35;
    return smoothstep(0.62 - 0.5 * wind.w, 0.78 - 0.5 * wind.w, paws);
}

// Each set's mean square slope, the swell's and the ripples': glassy water still heaves a
// little with the swell the last breeze left, enough to waver what it mirrors; the ripples
// are as steep as Cox and Munk's sea by the wind, its mean square slope 0.003 + 0.00512 a
// metre a second of it (taken as a gust's, at least 1.5 m/s, in a cat's-paw), on the sea's
// 0.003 of swell, which still water has less of.
fn wave_slopes(wind: f32, ruffled: f32) -> vec2<f32> {
    return vec2<f32>(0.0004 + 0.0012 * wind, 0.0002 + ruffled * 0.00512 * max(8.0 * wind, 1.5));
}

// The place the trains are laid out on (metres east and north) bent a little by a slow noise,
// half a metre over some metres, so no train's crests run dead straight and none cross
// another's in a lattice, as real wave fields wander.
fn wave_bend(at: vec2<f32>, anchored: vec3<f32>, up: vec3<f32>, t: f32) -> vec2<f32> {
    let p = anchored * 0.13 + up * t * 0.05;
    return at + vec2<f32>(noise(p) - 0.5, noise(p + 17.0) - 0.5) * 1.2;
}

// One train added to the surface at a place (bent, metres east and north), the pixel this many
// metres across, downwind a level direction, `spread` how much the wind picks its trains (0 in
// a calm, all ways alike), and the sets' mean square slopes (`wave_slopes`). A train is
// stronger the nearer its way to the wind's, by a cardioid (its mean round the circle a half,
// so the set's slope is kept): broad, as a wind sea's short crests are; its square ran the
// crests in rows. A train shorter than four pixels fades out before it is
// under one and a half, where it would only shimmer, its slope counted as unseen roughness
// instead; the temporal antialiasing smooths what sparkle is left, as a sun's glitter should.
fn wave_train(
    surface: ptr<function, Surface>,
    train: vec4<f32>,
    swell: bool,
    bent: vec2<f32>,
    pixel: f32,
    downwind: vec2<f32>,
    spread: f32,
    slopes: vec2<f32>,
    t: f32,
) {
    let k = train.x;
    let way = vec2<f32>(cos(train.z), sin(train.z));
    let facing = 0.5 + 0.5 * dot(way, downwind);
    let weight = mix(1.0, facing * 2.0, spread);
    var share = slopes.y / f32(TRAINS - SWELL);
    if swell {
        share = slopes.x / f32(SWELL);
    }
    share *= weight;
    // A train's mean square slope is half its steepness squared.
    let steepness = sqrt(2.0 * share);
    let phase = k * dot(way, bent) - train.y * t + train.w;
    let seen = clamp((6.2831853 / k / pixel - 1.5) * 0.4, 0.0, 1.0);
    (*surface).slope += way * (steepness * cos(phase) * seen);
    (*surface).height += steepness / k * sin(phase);
    (*surface).unseen += share * (1.0 - seen * seen);
}

// How sharp the sun's glint is: a normalised Blinn-Phong lobe as sharp as still water's where
// the pixel sees every wave, widened by the slopes of those it cannot (Blinn-Phong's power as
// Beckmann's for the mean square slope, 2/m² − 2), so far off the glitter merges into the
// sun's broad path on the water rather than flickering.
fn glint(normal: vec3<f32>, toward_eye: vec3<f32>, toward_sun: vec3<f32>, unseen: f32) -> f32 {
    let half_way = normalize(toward_sun + toward_eye);
    let lit = max(dot(normal, toward_sun), 0.0);
    let power = 2.0 / (2.0 / 602.0 + unseen) - 2.0;
    return pow(max(dot(normal, half_way), 0.0), power) * ((power + 8.0) / 25.13) * lit;
}
