//! Bushes, herbs and flowers grown from cards: a bush as clumps of leaf sprays in a rough
//! dome, a herb or fern as a ring of leaves or fronds arching out from its middle, and a
//! flower as a tuft of upright cards crossing at its foot, as games draw small plants.

use super::build::{Build, Placing, card};
use super::tree::{Canopy, clump};
use super::{BushDef, Card, RosetteDef};
use bevy::prelude::*;
use std::f32::consts::TAU;
use terra_gen::Rng;

/// A bush a metre or so high: clumps of leaf cards in a rough dome.
pub fn bush(dice: &mut Rng, def: &BushDef, leaf: &Card, tint: Vec3) -> Build {
    let height = dice.range(0.9, 1.4);
    let width = height * def.spread * dice.range(0.85, 1.15);
    let canopy = Canopy {
        centre: Vec3::Y * height * 0.45,
        reach: width * 0.6,
        height,
        bend: 0.25,
        tint,
        from_axis: false,
    };
    let mut build = Build::default();
    for _ in 0..def.clumps.0 + dice.below(def.clumps.1 - def.clumps.0 + 1) {
        let around = dice.range(0.0, TAU);
        let out = dice.range(0.0, 0.45) * width;
        let at = Vec3::new(
            libm::cosf(around) * out,
            dice.range(0.35, 0.8) * height,
            libm::sinf(around) * out,
        );
        let (cards, size) = (6 + dice.below(4), dice.range(0.35, 0.5) * height);
        clump(&mut build, dice, leaf, &canopy, at, size, cards);
    }
    build
}

/// A herb or fern about a metre across: leaves from its middle, each tilted up from the
/// ground and arching out, its stalk at the middle. The picture's stalk and tip say which
/// way a leaf runs in it.
pub fn rosette(dice: &mut Rng, def: &RosetteDef, leaf: &Card, tint: Vec3) -> Build {
    let mut build = Build::default();
    let count = def.leaves.0 + dice.below(def.leaves.1 - def.leaves.0 + 1);
    let stalk = Vec2::new(def.stalk.0, def.stalk.1);
    let tip = Vec2::new(def.tip.0, def.tip.1);
    // The leaf's own axis in its picture, and its length there.
    let axis = tip - stalk;
    let reach = axis.length().max(0.1);
    let turn = dice.range(0.0, TAU);
    for i in 0..count {
        let around = turn + i as f32 / count as f32 * TAU + dice.range(-0.3, 0.3);
        let out = Vec3::new(libm::cosf(around), 0.0, libm::sinf(around));
        let tilt = dice.range(def.tilt.0, def.tilt.1).to_radians();
        let length = dice.range(0.75, 1.1);
        // Which way the leaf runs from its stalk, and across it.
        let run = (out * libm::cosf(tilt) + Vec3::Y * libm::sinf(tilt)).normalize();
        let across = out.cross(Vec3::Y).normalize();
        let facing = across.cross(run).normalize();
        // The picture's axis is turned onto the leaf's run: its right and down edges are
        // laid so that stalk-to-tip in the picture is stalk-to-tip in the world.
        let scale = length / reach;
        let (ax, ay) = (axis.x / reach, axis.y / reach);
        let right = (run * ax + across * ay) * scale;
        let down = (run * ay - across * ax) * scale;
        let shade = 0.82 + 0.18 * (i as f32 / count as f32) + dice.range(-0.05, 0.05);
        card(
            &mut build,
            leaf,
            &Placing {
                pin: stalk,
                at: Vec3::Y * 0.02,
                right,
                down,
                normal: (facing * 0.5 + Vec3::Y * 0.5 + out * 0.2).normalize(),
                sway: Vec2::new(0.6 * libm::sinf(tilt), 0.5),
                colour: tint * shade,
            },
        );
    }
    build
}

/// A flower: two or three upright cards crossing at its foot, each the picture of a small
/// clump, turned so they never line up.
pub fn tuft(dice: &mut Rng, planes: u32, flower: &Card, tint: Vec3) -> Build {
    let mut build = Build::default();
    let turn = dice.range(0.0, TAU);
    for i in 0..planes {
        let around = turn + i as f32 / planes as f32 * std::f32::consts::PI + dice.range(-0.2, 0.2);
        let across = Vec3::new(libm::cosf(around), 0.0, libm::sinf(around));
        let lean = Vec3::new(dice.range(-0.1, 0.1), 0.0, dice.range(-0.1, 0.1));
        let up = (Vec3::Y + lean).normalize();
        let size = dice.range(0.85, 1.1);
        card(
            &mut build,
            flower,
            &Placing {
                pin: Vec2::new(0.5, 0.97),
                at: Vec3::ZERO,
                right: across * size,
                down: -up * size,
                normal: (Vec3::Y * 0.6 + across.cross(Vec3::Y) * 0.4).normalize(),
                sway: Vec2::new(0.0, 0.0),
                colour: tint * dice.range(0.9, 1.05),
            },
        );
    }
    // Sway grows with height above the foot, so the stems' feet stay put.
    for (sway, position) in build.sway.iter_mut().zip(&build.positions) {
        let up = position[1].max(0.0);
        *sway = [0.9 * up, 0.4 * up];
    }
    build
}
