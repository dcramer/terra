//! The distance-matched stop (Lyra's: UE5's Animation Locomotion Library,
//! `DistanceMatchToTarget`, its documentation; D631's methods, `terra_motion::distance`): a jog
//! or sprint braking with nothing asked plays `Jog_Stop` (or its mirror, `Jog_Stop_R`, by the
//! foot planted last) at the time whose distance left to the clip's own stop is the body's
//! predicted stopping distance (`Motion::stopping`), its play rate set a frame so it lands
//! there, its travel taken off the pelvis (Unreal's root lock) as the mover moves the body. So
//! its planted feet stand on the ground however hard the mover brakes, and it ends standing,
//! where the idle takes over.

use super::{Animated, Doing, Moves};
use bevy::{animation::AnimatedBy, platform::collections::HashMap, prelude::*};
use terra_motion::feet::Markers;

/// The stop's clip with the left foot planted last before it (and swinging to its plant, the
/// right planted through the brake) and its mirror.
const STOPS: [&str; 2] = ["Jog_Stop_R", "Jog_Stop"];

/// The stop's clip, by the foot planted last in the stride it takes over from.
pub(super) fn clip(right: bool) -> &'static str {
    STOPS[right as usize]
}

/// What a figure shows given what its speed says (`now`), what it showed (`was`), the clip
/// playing and how far in, and the body's stopping distance: a jog or sprint braking with
/// nothing asked goes into its stop on the foot its stride planted last, and stays in it while
/// it travels, unless a wish comes back.
pub(super) fn doing(
    markers: &HashMap<String, Markers>,
    (now, was): (Doing, Doing),
    playing: Option<(&str, f32)>,
    stopping: Option<f32>,
) -> Doing {
    if stopping.is_none() {
        return now;
    }
    let travels = |clip: &str| markers.get(clip).is_some_and(Markers::travels);
    match was {
        Doing::Jog | Doing::Sprint if matches!(now, Doing::Jog | Doing::Sprint) => {
            let right = playing
                .and_then(|(name, t)| markers.get(name)?.phase_in_loop(t))
                .is_none_or(|(foot, _)| foot == 1);
            if travels(clip(right)) {
                Doing::Stop(right)
            } else {
                now
            }
        }
        Doing::Stop(right) if matches!(now, Doing::Jog | Doing::Sprint | Doing::Idle) => {
            let stood = playing.is_none_or(|(name, t)| {
                markers
                    .get(name)
                    .is_none_or(|m| t >= m.stop().0 - 1.0 / m.rate)
            });
            if stood { now } else { Doing::Stop(right) }
        }
        _ => now,
    }
}

/// Where a stop starts with the body `left` metres from standing, its figure `size` times its
/// model: as far from the clip's own stop.
pub(super) fn entry(markers: &HashMap<String, Markers>, clip: &str, left: f32, size: f32) -> f32 {
    markers
        .get(clip)
        .map_or(0.0, |m| m.stop_time(left / size.max(1e-3), 0.0))
}

/// Where the clip of `now` starts as the figure changes to it from `was`: a stop just begun at
/// its `entry` for where the body was a frame back (`left` then, m), so this frame plays as far
/// as it went; any other change from its start.
pub(super) fn from_s(
    markers: &HashMap<String, Markers>,
    (now, was): (Doing, Doing),
    left: Option<f32>,
    size: f32,
) -> f32 {
    match now {
        Doing::Stop(_) if was != now => entry(markers, now.clip(), left.unwrap_or(0.0), size),
        _ => 0.0,
    }
}

/// The play rate that takes a stop from `seek` to where its distance left is the body's
/// (`left`, m, the figure `size` times its model) this frame, `dt` seconds on: never back.
pub(super) fn rate(m: &Markers, seek: f32, left: f32, size: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    ((m.stop_time(left / size.max(1e-3), seek) - seek).max(0.0) / dt).min(FASTEST)
}

/// A stop plays at most this times its own rate, its planted feet held by the foot lock for
/// the distance it falls behind the body: a clip's jerk grows with its rate squared (pop.stop
/// about 0.55 x this squared from a sprint), and at a 40 m/s² brake a matched stop asks about
/// 3.5x. Swept 1.5-3.5 in anim-blend j9: 2.5 is the least that keeps stop.settle and
/// glide.stop, and 2.75 and 3 drag a foot 25 cm in stop.jog.half (the leg out of reach of its
/// lock). At D633's 17 m/s² a matched stop asks under 2x, so the cap is a guard; lower caps
/// (1.5-2) lengthen stop.settle and leave pop.stop at its floor, the change into the stop.
const FASTEST: f32 = 2.5;

/// The pace a clip's planted feet stand on the ground at, m/s at its own rate in its units: a
/// stride loop's (`paces`) or a travelling clip's root speed `seek` seconds in.
pub(super) fn pace(moves: &Moves, clip: &str, seek: f32) -> f32 {
    moves
        .paces
        .get(clip)
        .copied()
        .unwrap_or_else(|| moves.markers.get(clip).map_or(0.0, |m| m.root_speed(seek)))
}

/// Each figure's travelling clips' travel taken off its pelvis, by their weights (Unreal's root
/// lock): the mover moves the body, so a clip's own travel would carry it twice as far. After
/// the clips are evaluated, before the inertialization records the pose.
pub(super) fn root_lock(
    figures: Query<(Entity, &AnimationPlayer, &Animated)>,
    mut bones: Query<(Entity, &AnimatedBy, &Name, &mut Transform)>,
    mut pelvises: Local<HashMap<Entity, Entity>>,
) {
    for (figure, animator, animated) in &figures {
        let moves = &animated.moves;
        let mut off = Vec3::ZERO;
        for (name, m) in moves.markers.iter().filter(|(_, m)| m.travels()) {
            let Some(nodes) = moves.clips.get(name) else {
                continue;
            };
            let all = nodes
                .whole
                .iter()
                .chain(&nodes.under)
                .chain(nodes.under_one.iter().flatten());
            for node in all {
                if let Some(a) = animator.animation(*node) {
                    off += Vec3::from(m.distance_way) * m.travelled(a.seek_time()) * a.weight();
                }
            }
        }
        if off == Vec3::ZERO {
            continue;
        }
        if !pelvises.contains_key(&figure) {
            let found = bones
                .iter()
                .find(|(_, by, name, _)| by.0 == figure && name.as_str() == "pelvis");
            match found {
                Some((pelvis, ..)) => pelvises.insert(figure, pelvis),
                None => continue,
            };
        }
        if let Ok((.., mut place)) = bones.get_mut(pelvises[&figure]) {
            place.translation -= off;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stop clip braking from 2 m/s over its first second (d = 2t - t^2), standing a second
    /// after, its right foot planted from 0.5 s on; and a jog loop of a second, its left foot
    /// down 0-0.3 s and its right 0.5-0.8 s.
    fn clips() -> HashMap<String, Markers> {
        let rate = 60.0;
        let stop = Markers {
            length_s: 2.0,
            rate,
            distance: (0..=120)
                .map(|i| {
                    let t = (i as f32 / rate).min(1.0);
                    2.0 * t - t * t
                })
                .collect(),
            ..Default::default()
        };
        let jog = Markers {
            length_s: 1.0,
            rate,
            lock_l: (0..=60).map(|i| f32::from(i <= 18)).collect(),
            lock_r: (0..=60)
                .map(|i| f32::from((30..=48).contains(&i)))
                .collect(),
            foot_down_l: vec![[0.0, 0.3]],
            foot_down_r: vec![[0.5, 0.8]],
            ..Default::default()
        };
        HashMap::from([
            ("Jog_Stop".to_string(), stop.clone()),
            ("Jog_Stop_R".to_string(), stop),
            ("Jog_Fwd_Loop".to_string(), jog),
        ])
    }

    /// A jog braking goes into the stop on the foot it planted last; the stop holds while it
    /// travels and gives way to what the speed says once it stands, or at once to a wish.
    #[test]
    fn a_braking_jog_stops_on_the_foot_it_planted_last_and_stands_after() {
        let m = clips();
        let jog = (Doing::Jog, Doing::Jog);
        assert_eq!(
            doing(&m, jog, Some(("Jog_Fwd_Loop", 0.6)), Some(0.3)),
            Doing::Stop(true)
        );
        assert_eq!(
            doing(&m, jog, Some(("Jog_Fwd_Loop", 0.1)), Some(0.3)),
            Doing::Stop(false)
        );
        assert_eq!(
            doing(&m, jog, Some(("Jog_Fwd_Loop", 0.6)), None),
            Doing::Jog
        );
        let stopping = (Doing::Idle, Doing::Stop(true));
        assert_eq!(
            doing(&m, stopping, Some(("Jog_Stop", 0.5)), Some(0.0)),
            Doing::Stop(true)
        );
        assert_eq!(
            doing(&m, stopping, Some(("Jog_Stop", 1.2)), Some(0.0)),
            Doing::Idle
        );
        // A wish back: the jog again.
        assert_eq!(
            doing(
                &m,
                (Doing::Jog, Doing::Stop(true)),
                Some(("Jog_Stop", 0.5)),
                None
            ),
            Doing::Jog
        );
        // Nothing to stop from a roll or a guard.
        assert_eq!(
            doing(&m, (Doing::Roll, Doing::Jog), None, Some(0.3)),
            Doing::Roll
        );
    }

    /// The stop enters where its distance left is the body's, and each frame its rate takes it
    /// to where the body's distance left is then: with the body braking at a constant rate, the
    /// clip ends its travel as the body stands, and the ground under its feet goes as fast as
    /// the body (its root speed by its rate).
    #[test]
    fn a_stop_played_by_distance_travels_as_the_body_does_and_stands_with_it() {
        let m = clips();
        let stop = &m["Jog_Stop"];
        let (decel, size, dt) = (8.0, 1.0, 1.0 / 60.0);
        let mut speed: f32 = 1.5;
        let left = terra_motion::distance::stopping_distance(speed, decel);
        let mut seek = entry(&m, "Jog_Stop", left, size);
        // The clip's distance left to its farthest is (1 - t)^2.
        let expected = 1.0 - libm::sqrtf(left);
        assert!((seek - expected).abs() < 0.02, "{seek} {expected}");
        while speed > 0.0 {
            speed = (speed - decel * dt).max(0.0);
            let left = terra_motion::distance::stopping_distance(speed, decel);
            let r = rate(stop, seek, left, size, dt);
            // Until the last centimetres, where the clip is held at its own stop (STOPPED_M).
            if speed > 0.6 {
                let ground = stop.root_speed(seek) * r;
                assert!(
                    (ground - speed).abs() < 0.25 * speed,
                    "at {seek:.3}: the feet's ground {ground:.2} m/s, the body {speed:.2}"
                );
            }
            seek += r * dt;
        }
        assert!((seek - stop.stop().0).abs() < 1e-3, "{seek}");
    }
}
