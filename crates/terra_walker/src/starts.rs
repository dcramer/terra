//! A figure's stops (docs/controller.md criterion 21d, D455), as Paragon's and ALS's are
//! played: a clip that keeps its root motion, one for each foot it brakes on (`Jog_Stop` on the
//! left, `Jog_Stop_R` its mirror, `tools/characters/mirror.ts`), picked by the foot the jog has
//! planted when the wish is let go and entered where that foot stands with its distance left
//! nearest the body's stopping distance (the kind's `decel`, movers.ron), so the jog's locked
//! foot stays locked and the clip brings the other alongside. Its travel is taken off the
//! pelvis (Unreal's root lock, `lock_roots`) and its time matched to the body's distance while
//! it brakes (Lyra's distance matching, `terra_motion::distance`), played no faster than
//! RATE_CAP, then on at its own pace once the body stands; it gives way to the idle once its
//! travel is done and both feet stand, and to a jog wished again at the jog's step (Unreal's
//! sync markers). So the clip steps the feet, not D455's rule.

use bevy::{animation::AnimationTargetId, prelude::*};
use terra_motion::feet::{FULL, Markers};

use crate::character;
use crate::feet::Baked;

/// The stops, each braking on its own foot: the one the jog has planted is picked.
pub(crate) const STOPS: [&str; 2] = ["Jog_Stop", "Jog_Stop_R"];
/// The body stands once it goes slower than this over the ground, m/s.
pub(crate) const STANDING_MPS: f32 = 0.05;
/// A stop's clip is played no faster than this while its distance is matched (Lyra's play
/// rate clamp): a body braking harder than the clip leaves the rest to play at 1 once it
/// stands, its feet held by their locks, not a clip's last steps rushed through.
pub(crate) const RATE_CAP: f32 = 2.0;

/// What the figure shows next, `was` what it showed and `now` what its speed alone says (a
/// stride, standing, or anything else, which goes as said): a stop when a stride's wish is let
/// go, played until its travel is done and both its feet stand (`clip` its markers, `at` its
/// time); a stride wished again while stopping once the body goes.
pub(crate) fn shown(
    was: crate::figure::Doing,
    now: crate::figure::Doing,
    wish: bool,
    at: f32,
    clip: Option<&Markers>,
) -> crate::figure::Doing {
    use crate::figure::Doing::*;
    if !matches!(now, Idle | Jog | Sprint) {
        return now;
    }
    match (was, clip) {
        (Jog | Sprint, _) if !wish => Stop,
        (Stop, _) if wish && now != Idle => now,
        (Stop, Some(clip)) if !stood(clip, at, now == Idle) => Stop,
        (Stop, _) => Idle,
        _ => now,
    }
}

/// Whether a stop's clip `at` stands on both feet and is done: its travel done, or the body
/// `standing` (a body braking harder than the clip stands before the clip's travel is done;
/// the rest of it, its pelvis settling over feet already locked, would only hold the idle back).
fn stood(clip: &Markers, at: f32, standing: bool) -> bool {
    (standing || at >= clip.stop().0) && (0..2).all(|side| clip.lock(side, at) >= FULL)
}

/// The foot a jog stands on as it is let go, from how each foot is kept (`states`) and its
/// clips' lock curves (`curves`): the one kept planted while the other is not; else the one its
/// clips have down (curve FULL); else, one foot leaving the ground (its curve falling, between
/// 0 and FULL), the other, which lands next; none in the air. A foot whose lock weight is
/// still high while D455's rule lifts it (stepping, blending back to its clip) is not planted:
/// a stop entered on it plants a foot that is in the air, which then glides to the clip's.
pub(crate) fn planted(states: [&str; 2], curves: [f32; 2]) -> Option<usize> {
    let kept = states.map(|state| state == "planted");
    if kept[0] != kept[1] {
        return Some(usize::from(kept[1]));
    }
    let down = curves.map(|curve| curve >= FULL);
    if down[0] != down[1] {
        return Some(usize::from(down[1]));
    }
    let leaving = curves.map(|curve| curve > 0.0 && curve < FULL);
    match leaving {
        [true, false] if !down[1] => Some(1),
        [false, true] if !down[0] => Some(0),
        _ => None,
    }
}

/// Which of `stops` to play and from when, the body's stopping distance `left` in the clips'
/// units and `planted` the foot its jog stands on (none in the air): of the times its clip
/// has that foot locked and the other not, the one whose distance left to the clip's stop is
/// nearest `left`; any foot if none is planted.
pub(crate) fn stop_entry(
    stops: &[Option<&Markers>],
    planted: Option<usize>,
    left: f32,
) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32, f32)> = None;
    for (which, clip) in stops.iter().enumerate() {
        let Some(clip) = clip.filter(|clip| clip.travels()) else {
            continue;
        };
        let (stop, end) = clip.stop();
        for i in 0..clip.distance.len() {
            let t = i as f32 / clip.rate;
            if t > stop {
                break;
            }
            let stands = |side: usize| clip.lock(side, t) >= FULL;
            let fits = match planted {
                Some(side) => stands(side) && !stands(1 - side),
                None => stands(0) != stands(1),
            };
            let off = (end - clip.distance[i] - left).abs();
            if fits && best.is_none_or(|(_, _, was)| off < was) {
                best = Some((which, t, off));
            }
        }
    }
    best.map(|(which, t, _)| (which, t))
}

/// How far a body going `speed` stops in, braking at `decel` (the mover's: `v^2 / 2a`).
pub(crate) fn stopping(speed: f32, decel: f32) -> f32 {
    if decel > 0.0 {
        speed * speed / (2.0 * decel)
    } else {
        0.0
    }
}

/// A stop's time this frame, `dt` on from `at`, the body going `speed`: while it brakes, where
/// the clip's distance left is the body's stopping distance, never back and no faster than
/// RATE_CAP (its time set, rate 0); once it stands, on at the clip's own pace (rate 1).
pub(crate) fn matched(
    playing: &mut bevy::animation::ActiveAnimation,
    at: &mut f32,
    clip: &Markers,
    (speed, decel, scale, dt): (f32, f32, f32, f32),
) {
    if speed < STANDING_MPS {
        playing.set_speed(1.0);
        *at = playing.seek_time().max(*at);
        return;
    }
    *at = braking(clip, *at, (speed, decel, scale, dt));
    playing.set_speed(0.0);
    playing.seek_to(*at);
}

/// A braking stop's time `dt` on from `at`: where its distance left is the body's stopping
/// distance, never back and no faster than RATE_CAP.
fn braking(clip: &Markers, at: f32, (speed, decel, scale, dt): (f32, f32, f32, f32)) -> f32 {
    clip.stop_time(stopping(speed, decel) / scale, at)
        .min(at + RATE_CAP * dt)
}

/// The loop's time a stop or start gives way to it at, the clip `at`: as far between the same
/// feet's plants (Unreal's sync markers); its start if either has none.
pub(crate) fn into_loop(from: &Markers, at: f32, run: &Markers) -> f32 {
    from.phase(at)
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

    /// A stop at 10 samples a second over 2 s braking 2 m/s to standing over its first second
    /// (d = 2t - t^2), planted on `first` from 0.4 s and on both from 1 s.
    fn stop(first: usize) -> Markers {
        let lock = |side: usize| -> Vec<f32> {
            (0..=20)
                .map(|i| {
                    let from = if side == first { 4 } else { 10 };
                    if i >= from { 1.0 } else { 0.0 }
                })
                .collect()
        };
        let (lock_l, lock_r) = (lock(0), lock(1));
        Markers {
            length_s: 2.0,
            rate: 10.0,
            lock_l,
            lock_r,
            distance: (0..=20)
                .map(|i| {
                    let t = (i as f32 / 10.0).min(1.0);
                    2.0 * t - t * t
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_stride_let_go_stops_until_both_feet_stand_and_its_travel_is_done_or_the_body_stands() {
        let clip = stop(0);
        // Jogging, the wish let go: a stop, whatever the speed says.
        assert_eq!(shown(Jog, Jog, false, 0.0, None), Stop);
        assert_eq!(shown(Sprint, Idle, false, 0.0, None), Stop);
        // Braking, then standing on one foot: still the stop; travel done on both: the idle.
        assert_eq!(shown(Stop, Idle, false, 0.5, Some(&clip)), Stop);
        assert_eq!(shown(Stop, Idle, false, 0.85, Some(&clip)), Stop);
        assert_eq!(shown(Stop, Idle, false, 0.95, Some(&clip)), Stop);
        assert_eq!(shown(Stop, Idle, false, 1.0, Some(&clip)), Idle);
        // Both feet standing before its travel is done: the idle once the body stands, the
        // stop while it still goes.
        let mut long = stop(0);
        long.distance = (0..=20).map(|i| i as f32 / 20.0).collect();
        assert_eq!(shown(Stop, Idle, false, 1.2, Some(&long)), Idle);
        assert_eq!(shown(Stop, Jog, false, 1.2, Some(&long)), Stop);
        assert_eq!(shown(Stop, Idle, false, 0.95, Some(&long)), Stop);
        // Wished again: the stop holds until the body goes, then jogs.
        assert_eq!(shown(Stop, Idle, true, 0.5, Some(&clip)), Stop);
        assert_eq!(shown(Stop, Jog, true, 0.5, Some(&clip)), Jog);
        // Anything but walking goes as said; standing without a wish stands.
        assert_eq!(shown(Jog, Fall, false, 0.0, None), Fall);
        assert_eq!(shown(Idle, Idle, false, 0.0, None), Idle);
        assert_eq!(shown(Idle, Jog, true, 0.0, None), Jog);
    }

    #[test]
    fn the_planted_foot_is_the_one_kept_down_else_its_clips_else_the_one_landing_next() {
        // Kept planted on the right, the left swinging: the right.
        assert_eq!(planted(["free", "planted"], [0.0, 1.0]), Some(1));
        // The rule lifting the left though its clip has it down: the right, kept planted.
        assert_eq!(planted(["stepping", "planted"], [1.0, 0.0]), Some(1));
        // Neither kept planted: the one its clips have down.
        assert_eq!(planted(["stepping", "releasing"], [1.0, 0.0]), Some(0));
        // The right leaving the ground (lifted by the rule, its curve falling), the left in
        // the air: the left lands next (the yard's sprint stop at frame 634).
        assert_eq!(planted(["free", "blending"], [0.0, 0.803]), Some(0));
        // Both in the air, or both down: none.
        assert_eq!(planted(["free", "free"], [0.0, 0.0]), None);
        assert_eq!(planted(["planted", "planted"], [1.0, 1.0]), None);
    }

    #[test]
    fn a_stop_is_the_one_braking_on_the_planted_foot_entered_nearest_the_distance_left() {
        let (left, right) = (stop(0), stop(1));
        let stops = [Some(&left), Some(&right)];
        // Planted on the right with 0.25 m to go: the right's stop, 0.5 s in (0.25 m left),
        // where its right stands and its left does not yet.
        let (which, t) = stop_entry(&stops, Some(1), 0.25).unwrap();
        assert_eq!(which, 1);
        assert!((t - 0.5).abs() < 1e-4, "{t}");
        // On the left with farther to go than the clip has from its plant: from its plant.
        let (which, t) = stop_entry(&stops, Some(0), 0.9).unwrap();
        assert_eq!(which, 0);
        assert!((t - 0.4).abs() < 1e-4, "{t}");
        // A stop with no clips to play has none.
        assert_eq!(stop_entry(&[None, None], Some(0), 0.2), None);
    }

    #[test]
    fn a_stop_braking_harder_than_its_clip_plays_no_faster_than_its_cap() {
        let clip = stop(0);
        // 4 m/s at 40 m/s^2 stops in 0.2 m: its clip at 0.2 m left is 0.553 s in (1 - (1 -
        // t)^2 = 0.8), but from 0.4 s a 60th of a second on goes no farther than RATE_CAP's.
        let clip_at = clip.stop_time(stopping(4.0, 40.0), 0.0);
        assert!((clip_at - 0.553).abs() < 0.01, "{clip_at}");
        let (was, dt) = (0.4, 1.0 / 60.0);
        let to = braking(&clip, was, (4.0, 40.0, 1.0, dt));
        assert!((to - (was + RATE_CAP * dt)).abs() < 1e-5, "{to}");
        // Near its match it goes to it, and never back.
        assert!((braking(&clip, 0.54, (4.0, 40.0, 1.0, dt)) - clip_at).abs() < 1e-4);
        assert_eq!(braking(&clip, 0.6, (4.0, 40.0, 1.0, dt)), 0.6);
    }
}
