//! A tree's crown from its cards, one set for every level (D221): near, far, and the stand-in
//! its shadow is cast from.

use super::build::Build;
use super::card::Card;
use bevy::prelude::*;

/// A crown near the camera: every card, each knowing its rank, which the foliage shader
/// shrinks to its middle one by one with distance while the rest grow to cover as much, as
/// SpeedTree's leaves drop out: the crown thins without ever being swapped (D223), down to
/// `keep` a clump.
/// With `pivots`, one a tip, it carries them for wind in levels (D597); empty, it does not.
pub fn crown_near(cards: &[Vec<Card>], keep: usize, pivots: &[[[f32; 4]; 2]]) -> Build {
    let mut build = Build {
        levels: !pivots.is_empty(),
        ..default()
    };
    for (i, tip) in cards.iter().enumerate() {
        build.pivot = pivots.get(i).map(|p| p[0]);
        build.twig = pivots.get(i).map(|p| p[1]);
        for (k, card) in tip.iter().enumerate() {
            card.draw_ranked(&mut build, 1.0, Some((k, tip.len(), keep)));
        }
    }
    build
}

/// A crown far off: `keep` cards a clump, the first of its near crown's, grown as they are
/// there when the rest have dropped out (D223): two (D318), each the rectangle round its
/// leaves rather than their octagon (`Card::draw_far`).
pub fn crown_far(cards: &[Vec<Card>], keep: usize, pivots: &[[[f32; 4]; 2]]) -> Build {
    let mut build = Build {
        levels: !pivots.is_empty(),
        ..default()
    };
    for (i, tip) in cards.iter().enumerate() {
        build.pivot = pivots.get(i).map(|p| p[0]);
        build.twig = pivots.get(i).map(|p| p[1]);
        let keep = keep.min(tip.len());
        let grow = libm::powf(tip.len() as f32 / keep as f32, 0.55);
        for card in &tip[..keep] {
            card.draw_far(&mut build, grow);
        }
    }
    build
}

/// What a crown's shadow is cast from near the camera: the first two cards of every tip,
/// the same as the crown's own, enlarged as the simpler levels are, so the light still
/// comes through in flecks where it does through the crown.
pub fn crown_stand_in(cards: &[Vec<Card>], pivots: &[[[f32; 4]; 2]]) -> Build {
    let mut build = Build {
        offset_in_colour: true,
        levels: !pivots.is_empty(),
        ..default()
    };
    for (i, tip) in cards.iter().enumerate() {
        build.pivot = pivots.get(i).map(|p| p[0]);
        build.twig = pivots.get(i).map(|p| p[1]);
        let keep = 2.min(tip.len());
        // Two cards for all of a clump's, a little smaller than they cover together, as the
        // clumps overlap: grown as far as the levels grow theirs, a crown of more clumps closed
        // its shadow over the floor's flecks (D309).
        let grow = libm::sqrtf(tip.len() as f32 / keep as f32).min(2.6) * 0.7;
        for card in &tip[..keep] {
            card.draw(&mut build, grow);
        }
    }
    build
}
