//! A campfire's pool on the ground and its stones at dusk, modelled as `light` draws them
//! (`tests`' helpers).

use super::tests::{keyed, on_face, on_ground_drawn};
use super::*;

/// A kept fire's light as `light` draws it at an hour, the sun a degree or more under: keyed
/// up (`keyed`) and its reach drawn in at dusk (`range_by_hour`).
pub(super) fn at_hour(lit: &Lit, night: f32, sky: f32) -> Lit {
    Lit {
        lumens: lit.lumens * keyed(lit, night, sky),
        range: range_by_hour(lit.range, lit.dusk_reach, night, 1.0),
        ..lit.clone()
    }
}

/// The fire sheet's campfire at the yard's dusk (night 0.13, the sky's light on the shade 830):
/// how many times as lit as by the sky alone a surface is, sky and fire together, averaged over
/// ten seconds of flicker, each light giving it what `on` says.
fn yard_dusk(on: impl Fn(&Lit, Vec3) -> f32) -> f32 {
    let foot = Vec3::new(0.0, 2000.0, 0.0);
    let blazes = Blazes {
        kept: vec![Blaze {
            at: foot,
            tall: 1.5,
            heat: 0.9,
            seed: 0.3,
            wide: 0.7,
        }],
        ..default()
    };
    let (night, sky) = (0.13, 830.0);
    let sky_on = 4.56 * core::f32::consts::PI * sky;
    let fire: f32 = (0..100)
        .map(|i| {
            lit(&blazes, i as f32 * 0.1, crate::fire::BreathTier::Cards)
                .iter()
                .map(|l| on(&at_hour(l, night, sky), foot))
                .sum::<f32>()
        })
        .sum::<f32>()
        / 100.0;
    1.0 + fire / sky_on
}

#[test]
fn a_campfires_dusk_pool_is_brightest_by_its_stones_and_falls_off_within_a_few_metres() {
    // Lit mostly by the top light, hung high and wide, the dusk pool stood flat: the ground a
    // metre out took some four times the sky's light, 3 m out half as much, and 6 m out still
    // more than the far field, on the strip a tint to the frame's edges (g79's judges, g80's
    // measure). RDR2's pool is bright by its stones and falls off within a few metres: so a
    // metre out the ground is several times as lit as by the sky alone, two and a half times as
    // lit as 3 m out (main's rig, 2.1), and the field 6 m out lit by the sky alone.
    let ground = |out: f32| yard_dusk(|l, foot| on_ground_drawn(l, foot, out));
    let (one, three, six) = (ground(1.0), ground(3.0), ground(6.0));
    println!("dusk pool: {one:.2}x the sky's at 1 m, {three:.2}x at 3 m, {six:.2}x at 6 m");
    assert!(one >= 4.0, "1 m out: {one:.2} times the sky's");
    assert!(one >= 2.5 * three, "1 m {one:.2}, 3 m {three:.2}");
    assert!(six < 1.05, "6 m out: {six:.2} times the sky's");
}

#[test]
fn a_campfires_stones_are_not_driven_past_the_white_at_dusk() {
    // A ring stone's fire-facing side, 0.7 m out and a hand high, faces the pit key square on.
    // With the dusk's boost on the key (30 times, g80) it took some fifteen times the sky's
    // light: past the tonemapper's shoulder, the ring and ash bed read flat saturated
    // clay-orange to every judge (g82's faults). Lit, as main lit it at three times, but no
    // more than some six.
    let stone = yard_dusk(|l, foot| on_face(l, foot, 0.7, 0.12));
    println!("dusk stone: {stone:.2}x the sky's");
    assert!(stone <= 6.0, "a ring stone: {stone:.2} times the sky's");
    assert!(stone >= 2.0, "a ring stone: {stone:.2} times the sky's");
}
