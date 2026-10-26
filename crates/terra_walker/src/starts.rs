//! A figure's starts and stops (docs/controller.md criterion 21d, D455), as Lyra plays them
//! (its Animation Locomotion Library's distance matching, UE5's documentation): a clip that
//! keeps its root motion, `Jog_Start` from standing and `Jog_Stop` from a jog or sprint, its
//! travel taken off the pelvis (Unreal's root lock, `lock_roots`) and its time picked by
//! distance (`terra_motion::distance`): a stop where the clip's distance left to its own stop
//! is the body's predicted stopping distance (the kind's `decel`, movers.ron), a start advanced
//! by as far as the body went. So the clip steps the feet, not D455's rule. A stop played to
//! the standing body plays out at its own pace and gives way to the idle; a start gives way to
//! the jog once it runs, the loop entered at the start's step (sync markers).

use bevy::{animation::AnimationTargetId, prelude::*};
use terra_motion::feet::Markers;

use crate::character;
use crate::feet::Baked;

/// The clips.
pub(crate) const START: &str = "Jog_Start";
pub(crate) const STOP: &str = "Jog_Stop";
/// The body stands once it goes slower than this over the ground, m/s.
pub(crate) const STANDING_MPS: f32 = 0.05;

/// What the figure shows next, `was` what it showed and `now` what its speed alone says (a
/// stride, standing, or anything else, which goes as said): a stop when a stride's wish is let
/// go, played to its end; a start when a body standing wishes to go, played until it runs
/// (`at` the start's or stop's time, `clip` its markers).
pub(crate) fn shown(
    was: crate::figure::Doing,
    now: crate::figure::Doing,
    wish: bool,
    at: f32,
    clip: Option<&Markers>,
    blend: f32,
) -> crate::figure::Doing {
    use crate::figure::Doing::*;
    if !matches!(now, Idle | Jog | Sprint) {
        return now;
    }
    match (was, clip) {
        (Jog | Sprint | Start, _) if !wish => Stop,
        (Start, Some(clip)) if at >= clip.cruise() => now,
        (Start, _) => Start,
        (Stop, _) if wish => match now {
            Idle => Start,
            _ => now,
        },
        (Stop, Some(clip)) if at >= clip.length_s - blend => Idle,
        (Stop, None) => Idle,
        (Stop, _) => Stop,
        (Idle, _) if wish => Start,
        _ => now,
    }
}

/// Where a start or stop's clip enters, `speed` the body's, scaled to the clip's units by
/// `scale`: a stop where its distance left is the body's stopping distance, a start where its
/// clip first goes.
pub(crate) fn entered(
    doing: crate::figure::Doing,
    clip: &Markers,
    speed: f32,
    decel: f32,
    scale: f32,
) -> f32 {
    match doing {
        crate::figure::Doing::Stop => clip.stop_time(stopping(speed, decel) / scale, 0.0),
        _ => clip.goes(),
    }
}

/// How far a body going `speed` stops in, braking at `decel` (the mover's: `v^2 / 2a`).
fn stopping(speed: f32, decel: f32) -> f32 {
    if decel > 0.0 {
        speed * speed / (2.0 * decel)
    } else {
        0.0
    }
}

/// A start or stop's time this frame, `dt` on from `at`, the body going `speed`: a stop's by
/// its stopping distance until the body stands, then on at its own pace; a start's by the
/// distance it went. Playing it, its time is set (rate 0) or it plays at 1.
pub(crate) fn matched(
    doing: crate::figure::Doing,
    playing: &mut bevy::animation::ActiveAnimation,
    at: &mut f32,
    clip: &Markers,
    (speed, decel, scale, dt): (f32, f32, f32, f32),
) {
    match doing {
        crate::figure::Doing::Stop if speed < STANDING_MPS => {
            playing.set_speed(1.0);
            *at = playing.seek_time().max(*at);
            return;
        }
        crate::figure::Doing::Stop => {
            *at = clip.stop_time(stopping(speed, decel) / scale, *at);
        }
        _ => *at = clip.start_time(*at, speed * dt / scale, dt),
    }
    playing.set_speed(0.0);
    playing.seek_to(*at);
}

/// The loop's time a start gives way to it at, the start `at`: as far between the same feet's
/// plants (Unreal's sync markers); its start if either has none.
pub(crate) fn into_loop(start: &Markers, at: f32, run: &Markers) -> f32 {
    start
        .phase(at)
        .map_or(0.0, |(side, share)| run.time_in_loop(side, share))
}

/// Takes each travelling clip's root motion off the pelvis of every skeleton playing one, as
/// the animation left it (Unreal's root lock; the body moves instead): the clips' travel along
/// their way at the time each is at, by their share of the pelvis's weight.
pub(crate) fn lock_roots(
    baked: Res<Baked>,
    graphs: Res<Assets<AnimationGraph>>,
    animators: Query<(Entity, &AnimationPlayer, &AnimationGraphHandle)>,
    children: Query<&Children>,
    names: Query<&Name>,
    targets: Query<&AnimationTargetId>,
    mut transforms: Query<&mut Transform>,
) {
    for (entity, animator, graph) in &animators {
        let Some(graph) = graphs.get(graph) else {
            continue;
        };
        let travels = animator.playing_animations().any(|(node, _)| {
            matches!(graph.get(*node).map(|n| &n.node_type),
                Some(AnimationNodeType::Clip(clip)) if baked.get(clip.id()).is_some_and(Markers::travels))
        });
        if !travels {
            continue;
        }
        let Some(pelvis) = character::find(entity, "pelvis", &children, &names) else {
            continue;
        };
        let target = targets.get(pelvis).ok();
        let (mut off, mut whole) = (Vec3::ZERO, 0.0);
        for (node, active) in animator.playing_animations() {
            let Some(each) = graph.get(*node) else {
                continue;
            };
            let AnimationNodeType::Clip(clip) = &each.node_type else {
                continue;
            };
            let masked = target
                .and_then(|target| graph.mask_groups.get(target))
                .is_some_and(|groups| groups & each.mask != 0);
            let weight = active.weight() * each.weight;
            if masked || weight <= 0.0 {
                continue;
            }
            whole += weight;
            if let Some(markers) = baked.get(clip.id()).filter(|m| m.travels()) {
                off += Vec3::from(markers.distance_way)
                    * markers.travelled(active.seek_time())
                    * weight;
            }
        }
        if whole > 0.0
            && let Ok(mut transform) = transforms.get_mut(pelvis)
        {
            transform.translation -= off / whole;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::Doing::*;

    fn clip(length_s: f32, rate: f32, distance: Vec<f32>) -> Markers {
        Markers {
            length_s,
            rate,
            distance,
            ..Default::default()
        }
    }

    #[test]
    fn a_stride_let_go_stops_and_a_standing_wish_starts() {
        // A start standing its first half second, then going 2 m/s.
        let start = clip(
            2.0,
            10.0,
            (0..=20)
                .map(|i| (i as f32 / 10.0 - 0.5).max(0.0) * 2.0)
                .collect(),
        );
        let stop = clip(
            2.0,
            10.0,
            (0..=20).map(|i| (i as f32 / 10.0).min(1.0)).collect(),
        );
        // Jogging, the wish let go: a stop, whatever the speed says.
        assert_eq!(shown(Jog, Jog, false, 0.0, None, 0.2), Stop);
        assert_eq!(shown(Sprint, Idle, false, 0.0, None, 0.2), Stop);
        // Stopping, it plays to its end then stands; wished again while still going, it jogs.
        assert_eq!(shown(Stop, Idle, false, 1.0, Some(&stop), 0.2), Stop);
        assert_eq!(shown(Stop, Idle, false, 1.9, Some(&stop), 0.2), Idle);
        assert_eq!(shown(Stop, Jog, true, 0.5, Some(&stop), 0.2), Jog);
        assert_eq!(shown(Stop, Idle, true, 1.5, Some(&stop), 0.2), Start);
        // Standing and wishing: a start, until it runs, then the jog.
        assert_eq!(shown(Idle, Idle, true, 0.0, None, 0.2), Start);
        assert_eq!(shown(Start, Jog, true, 0.3, Some(&start), 0.2), Start);
        assert_eq!(shown(Start, Jog, true, 0.8, Some(&start), 0.2), Jog);
        // Anything but walking goes as said.
        assert_eq!(shown(Jog, Fall, false, 0.0, None, 0.2), Fall);
        assert_eq!(shown(Idle, Idle, false, 0.0, None, 0.2), Idle);
    }

    #[test]
    fn a_stop_enters_where_its_distance_left_is_the_bodys_stopping_distance() {
        // Braking 2 m/s to standing over a second: d = 2t - t^2.
        let stop = clip(
            2.0,
            60.0,
            (0..=120)
                .map(|i| {
                    let t = (i as f32 / 60.0).min(1.0);
                    2.0 * t - t * t
                })
                .collect(),
        );
        // 4 m/s at 40 m/s^2 stops in 0.2 m; a body drawn twice its clip's size, 0.1 m of clip:
        // 1 - (1 - t)^2 = 0.9 at t = 0.684.
        let t = entered(Stop, &stop, 4.0, 40.0, 2.0);
        assert!((t - 0.684).abs() < 0.01, "{t}");
    }
}
