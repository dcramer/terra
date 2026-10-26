//! Bodies closing fast (criterion 1's bodies, D53): two bodies meeting at 10-60 m/s, as a fall,
//! a charge, a knockback or a mount may, must never pass through each other, whatever the tick
//! lets each move. A step meets another body by its core (`Kind::core_m`), so passing through
//! is the cores meeting: their axes, each a body's capsule's axis, coming closer than both
//! cores. Known another way: each body's axis is a segment along up from its feet, and between
//! two ticks each moves in a straight line, so how near two axes came in a tick is the least
//! distance of two upright segments over that tick's motion, sampled finely, in the level frame.
//!
//! Three meetings at each closing speed: head on, half each; at a glance, the other's way 45
//! degrees off head on and passing 5 cm from the first's; and one falling onto the head of one
//! walking under it.

use super::field::{Course, GRAVITY, forward_at, local, side_at, world};
use super::runs::{Outcome, Run, walk};
use super::{Body, Intent, Kind, Measure, Mover, TICK};
use glam::{Vec2, Vec3};

/// How fast the two close, m/s.
const SPEEDS: [f32; 4] = [10.0, 25.0, 40.0, 60.0];
/// How long after they start the two meet, s.
const MEET_S: f32 = 0.5;
/// How many points of a tick's straight motion are measured.
const SAMPLES: usize = 256;
/// How far into each other two cores may go and not have met, m: the f32 frames' rounding.
const SLACK_M: f32 = 1.0e-3;

#[derive(Clone, Copy, Debug)]
enum Meeting {
    HeadOn,
    Glancing,
    Falling,
}

fn cases() -> Vec<(Meeting, f32)> {
    [Meeting::HeadOn, Meeting::Glancing, Meeting::Falling]
        .into_iter()
        .flat_map(|m| SPEEDS.map(|s| (m, s)))
        .collect()
}

/// A body's axis in the level frame: the bottom and top of the segment its capsule is
/// round, (`u`, `h`, `w`).
fn axis(body: &Body, kind: &Kind) -> (Vec3, Vec3) {
    let feet = local(body.feet);
    let bottom = feet + Vec3::Y * kind.radius;
    let top = feet + Vec3::Y * (kind.height - kind.radius).max(kind.radius);
    (bottom, top)
}

/// The least distance of two upright segments, each its bottom and top in the level frame.
fn apart(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> f32 {
    let level = Vec2::new(a.0.x - b.0.x, a.0.z - b.0.z).length();
    let gap = (b.0.y - a.1.y).max(a.0.y - b.1.y).max(0.0);
    (level * level + gap * gap).sqrt()
}

/// How near two bodies' axes came over a tick, from where each began to where it ended.
fn nearest(from: [(Vec3, Vec3); 2], to: [(Vec3, Vec3); 2]) -> f32 {
    let at = |s: (Vec3, Vec3), e: (Vec3, Vec3), t: f32| (s.0.lerp(e.0, t), s.1.lerp(e.1, t));
    (0..=SAMPLES)
        .map(|k| {
            let t = k as f32 / SAMPLES as f32;
            apart(at(from[0], to[0], t), at(from[1], to[1], t))
        })
        .fold(f32::INFINITY, f32::min)
}

fn run<M: Mover>(new: &impl Fn() -> M, meeting: Meeting, speed: f32) -> Outcome {
    let man = Kind::man();
    let flat = Course::flat();
    let mut run = Run::new(new(), &flat);
    // Each body's speed and how far the second's way is off head on: at a glance, each at the
    // speed that closes them at `speed` with 135 degrees between their ways.
    let (each, off) = match meeting {
        Meeting::Glancing => (speed / (2.0 * libm::sinf(67.5_f32.to_radians())), 45.0_f32),
        _ => (speed * 0.5, 0.0),
    };
    let (c, s) = (libm::cosf(off.to_radians()), libm::sinf(off.to_radians()));
    let mut ticks = (2.0 * MEET_S / TICK) as usize;
    match meeting {
        Meeting::HeadOn | Meeting::Glancing => {
            run.add(0, man, world(-each * MEET_S, 0.0, 0.0));
            // Coming from ahead (and to the side), its way passing 5 cm aside of the first's
            // at a glance: their axes would pass closer than both cores.
            let from = Vec2::new(c, s) * each * MEET_S;
            let aside = if off > 0.0 { 0.05 } else { 0.0 };
            run.add(1, man, world(from.x - aside * s, 0.0, from.y + aside * c));
        }
        Meeting::Falling => {
            // Dropped from where it meets the other's head at `speed`, the other walking in
            // under it to be there then.
            let fall_s = speed / GRAVITY;
            let drop = speed * speed / (2.0 * GRAVITY) + man.height;
            run.add(0, man, world(0.0, drop, 0.0));
            run.add(1, man, world(-man.jog * fall_s, 0.0, 0.0));
            ticks = ((fall_s + MEET_S) / TICK) as usize;
        }
    }
    let mut met = false;
    for _ in 0..ticks {
        let from = [axis(&run.body(0), &man), axis(&run.body(1), &man)];
        run.tick(|id, b| match (meeting, id) {
            (Meeting::Falling, 0) => Intent::default(),
            (Meeting::Falling, _) => Intent {
                drive: Some(forward_at(b.feet) * man.jog),
                ..walk(b)
            },
            (_, 0) => Intent {
                drive: Some(forward_at(b.feet) * each),
                ..walk(b)
            },
            _ => {
                let way = -(forward_at(b.feet) * c + side_at(b.feet) * s);
                Intent {
                    drive: Some(way * each),
                    ..walk(b)
                }
            }
        });
        let to = [axis(&run.body(0), &man), axis(&run.body(1), &man)];
        met |= nearest(from, to) < 2.0 * man.core_m - SLACK_M;
    }
    run.outcome(vec![f32::from(u8::from(met))])
}

/// `tunnel.closing`: of the meetings, how many passed through, the cores meeting. Also the
/// deepest any body went into the ground.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> (Vec<Measure>, f32) {
    let cases = cases();
    let outcomes = terra_gen::par::map(&cases, |(m, s)| run(new, *m, *s));
    let through = outcomes.iter().map(|o| o.values[0]).sum();
    let deepest = outcomes
        .iter()
        .flat_map(|o| o.depths.iter().copied())
        .fold(0.0, f32::max);
    (
        vec![Measure {
            id: "tunnel.closing",
            value: through,
        }],
        deepest,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_upright_axes_are_as_far_apart_as_their_level_gap_or_their_ends() {
        let a = (Vec3::new(0.0, 0.3, 0.0), Vec3::new(0.0, 1.5, 0.0));
        let beside = (Vec3::new(0.4, 0.3, 0.3), Vec3::new(0.4, 1.5, 0.3));
        assert!((apart(a, beside) - 0.5).abs() < 1e-6);
        let over = (Vec3::new(0.3, 1.8, 0.0), Vec3::new(0.3, 3.0, 0.0));
        assert!((apart(a, over) - 0.3 * 2.0_f32.sqrt()).abs() < 1e-5);
        // Two that crossed between ticks met, though they end farther apart than they began.
        let left = (Vec3::new(-1.0, 0.3, 0.0), Vec3::new(-1.0, 1.5, 0.0));
        let right = (Vec3::new(1.0, 0.3, 0.0), Vec3::new(1.0, 1.5, 0.0));
        assert!(nearest([left, right], [right, left]) < 1e-6);
    }
}
