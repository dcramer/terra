// The wind the trees sway in, shared by every shader that draws one: the near trees'
// (foliage.wesl's wind in levels, D597) and the far forest's cards (far_forest.wesl), so a
// gust crosses the near and the far forest together.

// Gusts (D255): one wind for the grass, the bushes and the trees, as Ghost of Tsushima has it.
// Its strength rises and falls as fronts roll downwind at about 6 m/s, waves 40 and 90 m long
// (a gust every 7 to 15 s), so they never line up the same way twice, sharpened so calm spells
// are still and gusts come through as fronts; the whole breathes slowly. From under a half to
// one and a half times the old steady wind, about the same on average: at two and a half, as
// it was, a gust swung an oak's crown a metre (D282). grass.wesl and wind.wesl keep the same
// formula.
fn gusts(downwind: f32, crosswind: f32, time: f32) -> f32 {
    let front = downwind - time * 6.0;
    let a = sin(front * 0.157 + crosswind * 0.05);
    let b = sin(front * 0.07 - crosswind * 0.083 + 1.7);
    let g = 0.5 + 0.5 * (0.55 * a + 0.45 * b);
    let breath = 0.8 + 0.2 * sin(time * 0.13 + 0.4);
    return (0.45 + 1.1 * g * g * g) * breath;
}

// The forest's shared sway (D597): one slow wave every tree's trunk and great limbs follow,
// at WAVE_PACE radians a second (a swing every 2.5 s), its crests rolling downwind at WAVE_SPEED, so a stand
// sways together and a gust's push is seen to travel across it, as Enshrouded's Springlands'
// crowns share one period of about 2.5 s, alike within a region (0.72 to 0.86) and two gusts
// crossing a still view in 12 s (the wind round's judge, research/2026-11-02-why-our-trees-
// read-fake.md). -1 to 1.
const WAVE_PACE: f32 = 2.5;
const WAVE_SPEED: f32 = 12.0;
fn wave(downwind: f32, crosswind: f32, time: f32) -> f32 {
    let along = downwind * (WAVE_PACE / WAVE_SPEED);
    return 0.8 * sin(time * WAVE_PACE - along + crosswind * 0.031) + 0.2 * sin(time * WAVE_PACE * 0.53 - along * 0.53 + 1.3);
}

// A trunk's lean: how far its top goes at a gust's full strength, a share of its height
// (0.004: 10 cm on a 25 m pine), so a tall crown 10 to 25 m off moves by its metres, the
// same for every viewer; and how much of it is steady downwind, the shared wave and its own
// rocking. The wave is most of it: it is what makes a region's crowns sway together over a
// second (the lake's pines' regional r 0.34 at 0.5, 0.51 at 0.8; Enshrouded 0.52 to 0.55).
const TRUNK_SWAY: f32 = 0.004;
const LEAN_STEADY: f32 = 0.3;
const LEAN_WAVE: f32 = 0.8;
const LEAN_OWN: f32 = 0.2;

fn tree_hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

// The tree standing at `foot`'s lean, downwind and across, as metres its top goes for each
// metre of its height: the near tree's (foliage.wesl's `levels`) and its far card's alike.
fn trunk_lean(foot: vec3<f32>, direction: vec3<f32>, across: vec3<f32>, time: f32) -> vec2<f32> {
    let downwind = dot(foot, direction);
    let crosswind = dot(foot, across);
    let phase = tree_hash(foot) * 6.2832;
    let own = sin(time * (0.6 + 0.2 * tree_hash(foot + 7.0)) + phase);
    let lean = (LEAN_STEADY + LEAN_WAVE * wave(downwind, crosswind, time) + LEAN_OWN * own) * gusts(downwind, crosswind, time);
    let rock = 0.3 * LEAN_OWN * sin(time * 0.53 + phase * 1.3);
    return vec2<f32>(lean, rock) * TRUNK_SWAY;
}
