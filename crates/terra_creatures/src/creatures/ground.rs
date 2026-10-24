//! A step over the ground: round what stands in the way (`steer`), the whole body kept off the
//! bark from rump to nose, kept to its own ground (dry, or water for a swimmer), and, when it
//! makes no way for a while though it is trying to go, a way out found and taken, as a crowd
//! agent re-plans when it stalls (Detour's `dtCrowd` replans a stuck agent's corridor; Unreal's
//! crowd following marks an agent blocked once it has moved too little in a time and asks
//! for a new path). The forest film's boars stood pressed against a trunk, a rump in the bark
//! (10-23: "the other animals kinda clip into something and get stuck").

use super::steer;
use super::{Creature, State};
use crate::species::{Moves, SpeciesDef};
use bevy::prelude::*;
use terra_world::flora::{Obstacles, Trunks};
use terra_world::planet::Planet;

/// A creature trying to go that has made less than a quarter of the way its pace would take it
/// in so long, seconds, is stuck.
pub const STUCK_S: f32 = 1.5;
const STUCK_SHARE: f32 = 0.25;
/// Ways tried out of where it is stuck, degrees off the way it wants, the nearest first: never
/// straight on, which is what stopped it.
const OUT_DEG: [f32; 10] = [
    35.0, -35.0, 70.0, -70.0, 100.0, -100.0, 135.0, -135.0, 165.0, -165.0,
];
/// How far straight on a way out must be clear for its body, metres, looked at so often.
pub const OUT_M: f32 = 4.0;
const OUT_STEP_M: f32 = 0.25;

/// How it has been getting on: where it last made way from, for how long since, and whether
/// it has been put clear of every trunk once since it was put out.
#[derive(Clone, Copy, Debug, Default)]
pub struct Progress {
    from: Option<Vec3>,
    for_s: f32,
}

impl Progress {
    /// Whether one now at a point and wanting to go at a speed has been stuck for `STUCK_S`:
    /// it starts over each time it has gone a quarter of its way, or stops trying.
    pub fn stuck(&mut self, here: Vec3, want_mps: f32, dt: f32) -> bool {
        let Some(from) = self.from.filter(|_| want_mps > 0.0) else {
            (self.from, self.for_s) = (Some(here), 0.0);
            return false;
        };
        let least_m = (want_mps * STUCK_S * STUCK_SHARE).max(0.2);
        if here.distance(from) >= least_m {
            (self.from, self.for_s) = (Some(here), 0.0);
            return false;
        }
        self.for_s += dt;
        if self.for_s < STUCK_S {
            return false;
        }
        (self.from, self.for_s) = (Some(here), 0.0);
        true
    }

    /// Whether it has been looked at since it was put out.
    pub fn placed(&self) -> bool {
        self.from.is_some()
    }
}

/// How far a body of a radius goes straight from a point along a way before it touches any
/// round, metres, up to `OUT_M`.
fn clear_run(rounds: &[(Vec3, f32)], here: Vec3, way: Vec3, radius: f32) -> f32 {
    let up = here.normalize_or(Vec3::Y);
    let steps = (OUT_M / OUT_STEP_M) as usize;
    (1..=steps)
        .map(|i| i as f32 * OUT_STEP_M)
        .find(|&m| {
            let at = here + way * m;
            rounds
                .iter()
                .any(|&(middle, round)| (at - middle).reject_from(up).length() < round + radius)
        })
        .map_or(OUT_M, |m| m - OUT_STEP_M)
}

/// The way out for one stuck at a point wanting to go a way: the nearest turn off it whose
/// way is clear for its body for `OUT_M`, else the clearest.
pub fn way_out(rounds: &[(Vec3, f32)], here: Vec3, want: Vec3, radius: f32) -> Vec3 {
    let up = here.normalize_or(Vec3::Y);
    let want = want
        .reject_from(up)
        .normalize_or(up.any_orthonormal_vector());
    let ways = OUT_DEG.map(|deg| Quat::from_axis_angle(up, deg.to_radians()) * want);
    let runs = ways.map(|way| clear_run(rounds, here, way, radius));
    let best = (0..ways.len())
        .find(|&i| runs[i] >= OUT_M)
        .or_else(|| (0..ways.len()).max_by(|&a, &b| runs[a].total_cmp(&runs[b])))
        .unwrap_or(0);
    ways[best]
}

/// One step on over the ground for one wanting to go a way at a speed: the place it comes to.
pub fn step(
    creature: &mut Creature,
    about: &SpeciesDef,
    (solids, planet): ((&Trunks, &Obstacles), &Planet),
    (here, facing_now): (Vec3, Vec3),
    (want, want_mps): (Vec3, f32),
    dt: f32,
) -> Vec3 {
    let up = here.normalize();
    // Round the trunks, rocks and logs in its way, never through one.
    let (direction, speed) = steer::round(
        solids,
        here,
        about.body.radius,
        (creature.facing, want, want_mps),
    );
    if direction != Vec3::ZERO {
        let turn = 1.0 - libm::expf(-dt * 6.0);
        creature.facing = creature
            .facing
            .lerp(direction, turn)
            .reject_from(up)
            .normalize_or(direction);
    }
    // Its whole body off the bark, rump to nose: on the move, turning where it stands, or
    // just put out, which may have been beside a tree.
    let turned = facing_now.dot(creature.facing) < 0.999;
    let mut next = here + direction * speed * dt;
    if speed > 0.0 || turned || !creature.progress.placed() {
        next = steer::out_of(
            solids,
            next,
            (creature.facing, about.body.radius),
            (about.body.nose_m(), about.body.rear_m()),
        );
    }
    // Each keeps to its own: walkers out of water deeper than they wade, swimmers in it.
    let depth = |point: Vec3| planet.water_radius_m(point) - planet.ground_at(point).length();
    let swims = matches!(about.moves, Moves::Swims(_));
    let kept = if swims {
        depth(next) > 0.3
    } else if about.wades_m > 0.0 {
        depth(next) < about.wades_m
    } else {
        depth(next) < -0.2
    };
    if !kept {
        next = here;
        if let State::Wander(_) = creature.state {
            creature.state = State::Idle(1.0);
        }
    }
    if creature.progress.stuck(next, want_mps, dt) {
        get_out(creature, about, solids, next, want);
    }
    match about.moves {
        // Afloat: on the water's surface, its feet under it.
        Moves::Swims(feet_m) if depth(next) > 0.0 => {
            next.normalize() * (planet.water_radius_m(next) - feet_m)
        }
        _ => planet.ground_at(next),
    }
}

/// One stuck takes a way out: one running off runs that way, one wandering or going home
/// wanders a few metres along it first; one chasing keeps at its quarry.
fn get_out(
    creature: &mut Creature,
    about: &SpeciesDef,
    solids: (&Trunks, &Obstacles),
    here: Vec3,
    want: Vec3,
) {
    let rounds = steer::rounds_near(solids.0, solids.1, here, OUT_M + about.body.radius + 1.0);
    let way = way_out(&rounds, here, want, about.body.radius);
    match creature.state {
        State::Flee(_) => creature.bolt = way,
        State::Wander(_) | State::Return => creature.state = State::Wander(here + way * OUT_M),
        _ => return,
    }
    let off = want.angle_between(way).to_degrees();
    info!(
        "Creatures: a {} stuck for {STUCK_S} s turns {off:.0} degrees off its way.",
        about.name
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_trying_to_go_but_making_no_way_is_stuck_after_a_while_and_one_resting_never() {
        let mut progress = Progress::default();
        let here = Vec3::Y * 1000.0;
        let ticks = (STUCK_S / 0.1) as usize;
        // Pressed against something, a few centimetres either way.
        let stuck = (0..ticks + 2)
            .map(|i| progress.stuck(here + Vec3::X * 0.02 * (i % 2) as f32, 1.5, 0.1))
            .position(|stuck| stuck);
        assert!(stuck.is_some_and(|tick| tick >= ticks - 1), "{stuck:?}");
        // Resting where it is.
        let mut resting = Progress::default();
        assert!((0..100).all(|_| !resting.stuck(here, 0.0, 0.1)));
        // Walking on.
        let mut walking = Progress::default();
        assert!((0..100).all(|i| !walking.stuck(here + Vec3::X * 0.15 * i as f32, 1.5, 0.1)));
    }

    #[test]
    fn the_way_out_of_a_corner_is_the_nearest_turn_clear_for_the_body() {
        let here = Vec3::Y * 1000.0;
        // A wall of rounds straight ahead and to the right: out is to the left.
        let wall: Vec<(Vec3, f32)> = (-6..=6)
            .map(|i| (here + Vec3::X * 1.2 + Vec3::Z * i as f32 * 0.5, 0.4))
            .chain((0..8).map(|i| (here + Vec3::Z * 1.2 + Vec3::X * i as f32 * -0.5, 0.4)))
            .collect();
        let way = way_out(&wall, here, Vec3::X, 0.35);
        assert!(clear_run(&wall, here, way, 0.35) >= OUT_M, "{way}");
        assert!(way.dot(Vec3::NEG_Z) > 0.3, "out to the left: {way}");
    }
}
