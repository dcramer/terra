//! How a group met is put out: about a spot, along a roof's ridge, or crossing the way ahead,
//! each with its young at its side if its kind has them (`young`).

use super::Surroundings;
use crate::play::creatures::{self, Kinds};
use crate::play::species::{Bestiary, Haunt, Moves, Species};
use bevy::prelude::*;

/// So many meetings in ten are a crossing (`cross`) rather than a group waiting by the way.
pub(super) const CROSSINGS: f32 = 0.5;
/// A big animal's meeting is a crossing so often, the most it can be seen by.
pub(super) const BIG_CROSSINGS: f32 = 0.75;
/// A crossing starts so far ahead and so far aside, just inside the trees about 15° off the
/// camera's way, and crosses the way so far ahead of the player as they reach it; a runner
/// runs on so long past.
const CROSS_FROM_M: (f32, f32) = (28.0, 36.0);
const CROSS_ASIDE_M: (f32, f32) = (8.0, 12.0);
/// Starts tried for a crossing, one side of the way and then the other.
const CROSS_TRIES: usize = 6;
pub(super) const CROSS_LEAD_M: f32 = 10.0;
const CROSS_ON_S: f32 = 2.5;

/// A group put up by something before the player came, crossing their way ahead of them, as
/// shipped open worlds stage their ambient wildlife (Red Dead Redemption 2's deer and foxes
/// crossing the trail ahead): from far ahead and out to one side across to the other, timed
/// from how fast it goes and how fast the player comes on to cross a little way ahead of them,
/// so it is seen from small to near and moving all the while. A runner runs it; a flier flies
/// it, landing past the way. None for a kind that swims or wades, or ground under water.
pub(super) fn cross(
    commands: &mut Commands,
    (kinds, bestiary): (&Kinds, &Bestiary),
    around: &Surroundings,
    species: Species,
    (here, ahead, aside, speed): (Vec3, Vec3, Vec3, f32),
    count: u32,
    dice: &mut terra_gen::Rng,
) -> Option<Vec<Entity>> {
    let about = bestiary.get(species);
    let (mps, flies) = match about.moves {
        Moves::Flies(mps) => (creatures::Pace::Calm.flies_mps(mps), true),
        Moves::Walks => (creatures::Pace::Calm.runs_mps(about), false),
        Moves::Swims(_) => return None,
    };
    if about.wades_m > 0.0 || mps < 1.0 {
        return None;
    }
    let up = here.normalize_or(Vec3::Y);
    let dry = |p: Vec3| around.depth_m(p) < -0.1;
    // A few starts are tried, each side of the way: the first seen where it crosses, from
    // where the player is by then, and halfway or three quarters of the way there, from where
    // they are by then, so it is seen breaking from the trees and across.
    let lift = up * 0.6;
    let (start, point, way, timed) = (0..CROSS_TRIES).find_map(|tried| {
        let side = if tried % 2 == 0 { aside } else { -aside };
        let start = around.ground(
            here + ahead * dice.range(CROSS_FROM_M.0, CROSS_FROM_M.1)
                + side * dice.range(CROSS_ASIDE_M.0, CROSS_ASIDE_M.1),
        );
        let (at, timed) = crossing_point(here, ahead, start, mps, speed)?;
        let point = around.ground(at);
        let way = (point - start).reject_from(up).normalize_or_zero();
        if way == Vec3::ZERO || (!flies && !(dry(start) && dry(point))) {
            return None;
        }
        let eye = |s: f32| super::eye_at(around, here + ahead * speed * timed * s, ahead);
        let shows = |s: f32| super::shows_from(around, eye(s), start.lerp(point, s) + lift);
        let seen = shows(1.0) && (shows(0.5) || shows(0.75));
        seen.then_some((start, point, way, timed))
    })?;
    let past = around.ground(point + way * dice.range(10.0, 22.0));
    let mut put = Vec::new();
    for _ in 0..count {
        let from = around
            .ground(start + (way.cross(up) * dice.range(-1.5, 1.5)) + way * dice.range(-2.0, 0.5));
        let Some(animal) =
            creatures::spawn(commands, (kinds, bestiary), &around.planet, species, from)
        else {
            continue;
        };
        let to = around.ground(past + way.cross(up) * dice.range(-3.0, 3.0));
        let for_s = timed + CROSS_ON_S + dice.range(-0.4, 0.4);
        // Its young, if it has one at its side, running with it a step behind.
        let young = young_of(bestiary, species, dice).and_then(|young| {
            let beside = from + way.cross(up) * dice.range(-1.6, 1.6) - way * dice.range(0.6, 1.6);
            creatures::spawn(
                commands,
                (kinds, bestiary),
                &around.planet,
                young,
                around.ground(beside),
            )
        });
        for animal in std::iter::once(animal).chain(young) {
            commands
                .entity(animal)
                .entry::<creatures::Creature>()
                .and_modify(move |mut animal| {
                    if flies {
                        animal.fly(from, to);
                    } else {
                        animal.dash(way, for_s);
                    }
                });
            put.push(animal);
        }
    }
    Some(put)
}

/// Where on a player's way an animal starting from a point and going so fast reaches it
/// `CROSS_LEAD_M` ahead of them as they come on so fast, and how long it takes to.
/// None if it could not make it, the player coming on too fast.
pub(super) fn crossing_point(
    here: Vec3,
    ahead: Vec3,
    start: Vec3,
    mps: f32,
    speed: f32,
) -> Option<(Vec3, f32)> {
    let takes = |along: f32| (here + ahead * along).distance(start) / mps.max(0.1);
    // How far it falls short of meeting the way that far ahead of the player.
    let short = |along: f32| along - CROSS_LEAD_M - speed * takes(along);
    let (mut near, mut far) = (CROSS_LEAD_M, (start - here).dot(ahead));
    if far <= near || short(far) < 0.0 {
        return None;
    }
    for _ in 0..24 {
        let middle = (near + far) * 0.5;
        if short(middle) < 0.0 {
            near = middle;
        } else {
            far = middle;
        }
    }
    Some((here + ahead * far, takes(far)))
}

/// A town's birds are put out on a roof's ridge within so far, so often, rather than on the
/// ground; so far apart along it, and sitting there so long, seconds.
const ON_A_RIDGE: (f32, f32) = (25.0, 0.6);
const ON_A_RIDGE_APART_M: (f32, f32) = (0.3, 0.7);
const ON_A_RIDGE_S: (f32, f32) = (12.0, 45.0);

/// Puts out a group of a kind about a spot, within the reach given of it: on the ground, or
/// a flier of a settlement now and then in a row along the nearest roof's ridge, sitting.
pub(super) fn put_group(
    commands: &mut Commands,
    (kinds, bestiary): (&Kinds, &Bestiary),
    around: &Surroundings,
    species: Species,
    (spot, reach): (Vec3, Vec3),
    count: u32,
    dice: &mut terra_gen::Rng,
) -> Vec<Entity> {
    let about = bestiary.get(species);
    let townsbird = matches!(about.moves, Moves::Flies(_))
        && about
            .niche
            .as_ref()
            .is_some_and(|niche| niche.haunts.contains(&Haunt::Trodden));
    let ridge = if townsbird && dice.unit() < ON_A_RIDGE.1 {
        around.ridge_near(spot, ON_A_RIDGE.0)
    } else {
        None
    };
    let up = spot.normalize_or(Vec3::Y);
    let mut put = Vec::new();
    if let Some((a, b)) = ridge {
        let length = a.distance(b);
        let mut along = dice.range(0.0, (length * 0.5).max(0.0));
        for _ in 0..count {
            if along > length {
                break;
            }
            let sits = a.lerp(b, along / length.max(1e-3));
            let left = dice.range(ON_A_RIDGE_S.0, ON_A_RIDGE_S.1);
            if let Some(bird) =
                creatures::spawn(commands, (kinds, bestiary), &around.planet, species, sits)
            {
                commands
                    .entity(bird)
                    .entry::<creatures::Creature>()
                    .and_modify(move |mut bird| bird.roost(sits, left));
                put.push(bird);
            }
            along += dice.range(ON_A_RIDGE_APART_M.0, ON_A_RIDGE_APART_M.1);
        }
        return put;
    }
    for _ in 0..count {
        let turn = Quat::from_axis_angle(up, dice.range(0.0, std::f32::consts::TAU));
        let at = around.ground(spot + turn * reach * dice.range(0.0, 1.0));
        if let Some(animal) =
            creatures::spawn(commands, (kinds, bestiary), &around.planet, species, at)
        {
            put.push(animal);
        }
        if let Some(young) = young_of(bestiary, species, dice) {
            let beside =
                around.ground(at + turn * reach.normalize_or_zero() * dice.range(1.0, 2.5));
            put.extend(creatures::spawn(
                commands,
                (kinds, bestiary),
                &around.planet,
                young,
                beside,
            ));
        }
    }
    put
}

/// The kind of young one of a kind has at its side this time, if any (`young` in
/// creatures.ron): a hind with her calf at foot, so often as her kind says.
fn young_of(bestiary: &Bestiary, species: Species, dice: &mut terra_gen::Rng) -> Option<Species> {
    let (name, share) = bestiary.get(species).young.as_ref()?;
    if dice.unit() >= *share {
        return None;
    }
    bestiary.named(name)
}
