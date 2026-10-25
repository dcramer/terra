//! Falls (D476): a fall hurts by the speed it meets the floor at, harmless below the kind's
//! onset, fatal at its fatal speed, broken by water deep enough to swim in. Each answer is
//! known another way: a drop from a height lands at `sqrt(2gh)` (the law, not the mover's
//! report), so the heights for the kind's two speeds are known, and a slope takes the share of
//! that speed along its normal, `cos` of its grade. Run for two kinds, so the numbers are the
//! kind's (D136).

use super::data::second;
use super::field::{Course, Field, Water, world};
use super::runs::{Run, ground, walk};
use super::{Intent, Kind, Measure, Mover, TICK};
use glam::{Vec2, Vec3};

/// Short of and past a speed: a drop at 3% under the onset must not hurt, 3% over it must;
/// 3% under the fatal speed must leave the body alive, 3% over it must not.
const NEAR: [f32; 2] = [0.97, 1.03];
/// The steps jogged off, m: each of the course's and a drop of 1.5 m (the filmed yard's).
const STEPS: [f32; 9] = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 1.5];
/// Short drops, as shares of the height the onset is reached from: none hurts.
const SHORT: [f32; 4] = [0.1, 0.3, 0.6, 0.9];
/// The heights dropped from for the harm's rise, m: 1 to 30 by 0.5.
const RISE: std::ops::RangeInclusive<i32> = 2..=60;
/// The slope dropped onto, deg: walkable, so the body stands where it lands.
const SLOPE_DEG: f32 = 40.0;
/// The heights dropped into water from, m.
const WATER: [f32; 4] = [5.0, 10.0, 20.0, 30.0];

#[derive(Clone, Copy, Debug)]
enum Case {
    /// A jump in place on level ground, for the kind at an index of the two.
    Jump(usize),
    /// Jogged off a step down so high.
    StepDown(usize, f32),
    /// Jogged down a flight of the ground's stairs.
    Stairs(usize),
    /// Dropped from a height onto level ground.
    Drop(usize, f32),
    /// Dropped from a height onto a slope of [`SLOPE_DEG`].
    Slope(usize, f32),
    /// Dropped from a height into water, deep or only just deep enough to swim in.
    Water(usize, f32, bool),
}

fn kinds() -> [Kind; 2] {
    let man = Kind::man();
    [man, second(&man)]
}

/// The height a drop lands from at a speed: `v² / 2g`.
fn height_for(speed: f32, gravity: f32) -> f32 {
    speed * speed / (2.0 * gravity)
}

fn cases(gravity: f32) -> Vec<Case> {
    let mut cases = Vec::new();
    for (k, kind) in kinds().iter().enumerate() {
        cases.push(Case::Jump(k));
        cases.extend(STEPS.map(|h| Case::StepDown(k, h)));
        cases.push(Case::Stairs(k));
        let onset = height_for(kind.fall_onset, gravity);
        cases.extend(SHORT.map(|share| Case::Drop(k, onset * share)));
        for speed in [kind.fall_onset, kind.fall_fatal] {
            cases.extend(NEAR.map(|near| Case::Drop(k, height_for(speed * near, gravity))));
        }
        cases.extend(RISE.map(|i| Case::Drop(k, i as f32 * 0.5)));
        // Onto the slope: a little past the onset, harmless there (its normal takes less of
        // the speed), and a little past the fatal speed, hurt but alive.
        let cos = libm::cosf(SLOPE_DEG.to_radians());
        for speed in [kind.fall_onset * 1.05, kind.fall_fatal * NEAR[1]] {
            cases.push(Case::Slope(k, height_for(speed, gravity)));
        }
        debug_assert!(kind.fall_onset * 1.05 * cos < kind.fall_onset);
        for h in WATER {
            cases.extend([Case::Water(k, h, true), Case::Water(k, h, false)]);
        }
    }
    cases
}

/// The harm a case took, summed over its ticks.
fn run<M: Mover>(new: &impl Fn() -> M, case: Case) -> (f32, Vec<f32>) {
    let ticks = |s: f32| (s / TICK) as usize;
    let k = match case {
        Case::Jump(k)
        | Case::StepDown(k, _)
        | Case::Stairs(k)
        | Case::Drop(k, _)
        | Case::Slope(k, _)
        | Case::Water(k, ..) => k,
    };
    let kind = kinds()[k];
    let (course, start, seconds, walking) = match case {
        Case::Jump(_) => (Course::flat(), world(0.0, 0.0, 0.0), 2.0, false),
        Case::StepDown(_, h) => {
            let course = ground(&[(0.0, 0.0), (1.5, 0.0), (1.5, -h), (8.0, -h)]);
            (course, world(0.0, 0.0, 0.0), 2.0, true)
        }
        Case::Stairs(_) => {
            // Eight steps of 0.3 m down, on treads of 0.3 m.
            let mut corners = vec![(-5.0, 0.0)];
            for i in 0..8 {
                let (u, h) = (1.0 + i as f32 * 0.3, -0.3 * i as f32);
                corners.extend([(u, h), (u, h - 0.3)]);
            }
            corners.push((10.0, -2.4));
            (ground(&corners), world(0.0, 0.0, 0.0), 3.0, true)
        }
        Case::Drop(_, h) => (Course::flat(), world(0.0, h, 0.0), 3.0, false),
        Case::Slope(_, h) => {
            let (s, c) = (
                libm::sinf(SLOPE_DEG.to_radians()),
                libm::cosf(SLOPE_DEG.to_radians()),
            );
            let course = ground(&[(-20.0 * c, -20.0 * s), (20.0 * c, 20.0 * s)]);
            (course, world(0.0, h, 0.0), 3.0, false)
        }
        Case::Water(_, h, deep) => {
            let depth = if deep {
                6.0
            } else {
                kind.swim_depth * kind.height + 0.25
            };
            // Dug out to well over the drop: a dug box's top in the air would leave a floor
            // the field makes up under its top half (a difference is a bound, not exact).
            let mut pool = Course::flat();
            pool.dug
                .push((Vec3::new(-4.0, -depth, -4.0), Vec3::new(4.0, 40.0, 4.0)));
            pool.water = Some(Water {
                min: Vec2::new(-4.0, -4.0),
                max: Vec2::new(4.0, 4.0),
                surface: 0.0,
            });
            (pool, world(0.0, h, 0.0), 3.0, false)
        }
    };
    let mut run = Run::new(new(), &course);
    run.add(0, kind, start);
    let mut harm = 0.0;
    for tick in 0..ticks(seconds) {
        run.tick(|_, body| {
            let mut intent = if walking {
                walk(body)
            } else {
                Intent::default()
            };
            intent.jump = matches!(case, Case::Jump(_)) && tick == 0;
            intent
        });
        harm += run.mover().fall(0);
    }
    (harm, run.outcome(vec![harm]).depths)
}

/// The fall rows and the deepest any case's capsule went into the ground.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> (Vec<Measure>, f32) {
    let gravity = Course::flat().gravity();
    let cases = cases(gravity);
    let outcomes = terra_gen::par::map(&cases, |case| run(new, *case));
    let deepest = outcomes
        .iter()
        .flat_map(|(_, depths)| depths.iter().copied())
        .fold(0.0, f32::max);
    let all = || cases.iter().zip(outcomes.iter().map(|(harm, _)| *harm));
    let kinds = kinds();
    let speed = |h: f32| libm::sqrtf(2.0 * gravity * h);

    let harmless: f32 = all()
        .filter(|(case, _)| match case {
            Case::Jump(_) | Case::StepDown(..) | Case::Stairs(_) => true,
            Case::Drop(k, h) => speed(*h) < kinds[*k].fall_onset * NEAR[0] + 1e-3,
            _ => false,
        })
        .map(|(_, harm)| harm)
        .sum();
    // Each drop just short of or past a speed judged: below the onset unhurt, past it hurt;
    // below the fatal speed alive, past it dead.
    let judged = |k: usize, h: f32, harm: f32| -> Option<bool> {
        let (v, kind) = (speed(h), &kinds[k]);
        let near = |of: f32| NEAR.iter().any(|n| (v - of * n).abs() < 1e-3);
        if near(kind.fall_onset) {
            Some((harm > 0.0) != (v > kind.fall_onset))
        } else if near(kind.fall_fatal) {
            Some((harm >= 1.0) != (v > kind.fall_fatal))
        } else {
            None
        }
    };
    let speeds = all()
        .filter_map(|(case, harm)| match case {
            Case::Drop(k, h) => judged(*k, *h, harm),
            _ => None,
        })
        .filter(|wrong| *wrong)
        .count();
    // The harm by height never falls, and rises between the onset and the fatal speed.
    let mut rise = 0;
    for (k, kind) in kinds.iter().enumerate() {
        let drops: Vec<(f32, f32)> = all()
            .filter_map(|(case, harm)| match case {
                Case::Drop(of, h) if *of == k && RISE.contains(&((h * 2.0).round() as i32)) => {
                    Some((*h, harm))
                }
                _ => None,
            })
            .filter(|(h, _)| (h * 2.0 - (h * 2.0).round()).abs() < 1e-4)
            .collect();
        for pair in drops.windows(2) {
            let ((_, below), (h, above)) = (pair[0], pair[1]);
            let v = speed(h);
            let rising = v > kind.fall_onset && speed(pair[0].0) < kind.fall_fatal;
            if above < below || (rising && above <= below) {
                rise += 1;
            }
        }
    }
    // Onto the slope: past the onset but harmless, as its normal takes less; past the fatal
    // speed but alive, and hurt.
    let normal = all()
        .filter(|(case, harm)| match case {
            Case::Slope(k, h) if speed(*h) < kinds[*k].fall_fatal => *harm > 0.0,
            Case::Slope(..) => *harm <= 0.0 || *harm >= 1.0,
            _ => false,
        })
        .count();
    let water: f32 = all()
        .filter(|(case, _)| matches!(case, Case::Water(..)))
        .map(|(_, harm)| harm)
        .sum();
    let m = |id, value: f32| Measure { id, value };
    (
        vec![
            m("fall.harmless", harmless),
            m("fall.speeds", speeds as f32),
            m("fall.rise", rise as f32),
            m("fall.normal", normal as f32),
            m("fall.water", water),
        ],
        deepest,
    )
}
