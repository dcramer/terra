// The shape of the land beyond the world's edge (D273), shared by the sky, which draws it
// (sky.wesl `far_land`), and the volumetric clouds, which stop at it (cloud_volume.wesl): the
// sky is see-through and writes no depth, so the clouds' march ran on past the far ranges and
// laid the clouds behind them over them, the ranges standing as pale ghosts with clouds seen
// through (mountains round 3).

import super::cloud_noise::noise;

// Each ridge of the far land, nearest first: where it stands between the planet's own
// horizon below and the air's above (0 at the planet's, 1 at the air's), how high its hills
// rise as a share of the farthest's.
const RIDGES: u32 = 4u;
const RIDGE_AT: array<f32, 4> = array<f32, 4>(0.12, 0.5, 0.8, 1.0);
const RIDGE_HILLS: array<f32, 4> = array<f32, 4>(0.45, 0.55, 0.75, 1.0);
// How much taller a range's far peaks stand than a forest's hills of the same `hills`: as
// tall, they rose a degree past the lake's far treeline and barely registered (r16 review).
const PEAK_LIFT: f32 = 1.9;

// Billowing noise that stands still: the far land's ridges.
fn billow(p: vec3<f32>) -> f32 {
    return noise(p) * 0.5 + noise(p * 2.1 + 7.0) * 0.27 + noise(p * 4.3 + 19.0) * 0.15
        + noise(p * 8.9 + 5.0) * 0.08;
}

// How far the planet's horizon lies under the air's, radians, from an eye at `eye` over a
// planet of radius `r`: the stand-in planet is small, so its ground falls away below the air's
// horizon, by more the higher the eye.
fn horizon_dip(eye: vec3<f32>, r: f32) -> f32 {
    return acos(clamp(r / (r + max(length(eye) - r, 0.3)), 0.0, 1.0));
}

// A direction round the horizon as a point on a circle, so the noise along it has no seam.
fn around(direction: vec3<f32>, up: vec3<f32>) -> vec2<f32> {
    let east = normalize(cross(up, vec3<f32>(0.0, 0.0, 1.0)));
    let north = cross(east, up);
    let level = normalize(direction - up * dot(direction, up) + east * 1.0e-5);
    return vec2<f32>(dot(level, east), dot(level, north));
}

// One ridge `k` at the bearing `round` (`around`): where its foot stands over the air's
// horizon (`base`, radians, negative under it), its top, and its crest's sharpness (`crest`,
// which the sky's snow keys on). `hills` is the farthest ridge's height, radians, `peaks` 1 for
// a range's horns and 0 for a forest's hills.
struct Ridge {
    base: f32,
    top: f32,
    crest: f32,
}

fn ridge(k: u32, round: vec2<f32>, dip: f32, hills: f32, peaks: f32) -> Ridge {
    var at = RIDGE_AT;
    var rising = RIDGE_HILLS;
    let f = f32(k);
    // A range's nearer ridges stand over the horizon too, behind the world's own slopes,
    // where a forest's lie between the planet's horizon and the air's: with them below it
    // only the farthest showed, as pale as the sky.
    let base = -dip * (1.0 - at[k]) * (1.0 - 0.85 * peaks);
    // Hills along the ridge, and the crowns on them, finer and lower the farther.
    let p = vec3<f32>(round * (5.0 + 4.0 * f), f * 13.7);
    let shape = billow(p) * 0.85 + billow(p * 3.7 + 3.0) * 0.15;
    let crowns = noise(vec3<f32>(round * (260.0 + 240.0 * f), f * 5.1));
    // A range's crests, sharp where two slopes meet, never down to the horizon, so peak
    // stands behind peak along all of it (a ridged noise, as terrain generators make them).
    // Bent along the bearing, so a crest leans and its two flanks differ, and in massifs
    // and saddles a few tens of degrees wide: one crest frequency at one height read as a
    // row of identical pale triangles (lake film r16, the far right half).
    let bend = vec3<f32>((noise(p * 0.9 + 41.0) - 0.5) * 1.4, (noise(p * 0.9 + 47.0) - 0.5) * 1.4, 0.0);
    let q = p + bend;
    // Half as many crests as round 18's, which stood as an even row of needles.
    let crest = 1.0 - abs(noise(q * 1.8 + 11.0) * 2.0 - 1.0);
    let fine = 1.0 - abs(noise(q * 4.3 + 23.0) * 2.0 - 1.0);
    let jag = 1.0 - abs(noise(q * 14.3 + 31.0) * 2.0 - 1.0);
    let grit = 1.0 - abs(noise(q * 33.7 + 37.0) * 2.0 - 1.0);
    let massif = smoothstep(0.2, 0.8, noise(vec3<f32>(round * (1.2 + 0.5 * f), f * 9.1 + 5.0)));
    let height = mix(1.0, (0.35 + 0.8 * massif) * PEAK_LIFT, peaks);
    // Octave over octave, so a flank breaks in shoulders and gendarmes, not one straight line.
    // Spires only in a massif's heart, over broad shoulders and saddles between: spires
    // everywhere stood as an even saw blade (the r17 review), and in every massif's heart as
    // a row of needles: only in the highest massifs now, the saddles and shoulders between
    // at 45 to 60 percent of their height (the r18 review).
    let spires = 0.3 + 0.7 * pow(crest * 0.55 + fine * 0.25 + jag * 0.13 + grit * 0.07, 1.3) * mix(0.6, 1.0, shape);
    let shoulder = 0.45 + 0.15 * shape + 0.06 * fine;
    let range = mix(shoulder, max(spires, shoulder), smoothstep(0.5, 0.9, massif)) * height;
    let rolling = smoothstep(0.25, 0.85, shape);
    let top = base + hills * rising[k] * mix(rolling, range, peaks)
        + hills * (0.06 / (1.0 + f)) * crowns * (1.0 - peaks);
    return Ridge(base, top, crest);
}

// Whether the far land hides a direction from an eye: under the top of any of its ridges.
fn hides(direction: vec3<f32>, up: vec3<f32>, eye: vec3<f32>, r: f32, hills: f32, peaks: f32) -> bool {
    let lift = dot(direction, up);
    if hills <= 0.0 || lift > sin(hills * 1.3 * mix(1.0, PEAK_LIFT, peaks)) {
        return false;
    }
    let rise = asin(clamp(lift, -1.0, 1.0));
    let dip = horizon_dip(eye, r);
    let round = around(direction, up);
    for (var k = 0u; k < RIDGES; k++) {
        if rise < ridge(k, round, dip, hills, peaks).top {
            return true;
        }
    }
    return false;
}
