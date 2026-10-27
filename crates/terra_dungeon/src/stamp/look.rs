//! Where the camera looks along the tour: at the way `AHEAD_M` on, turned toward the next
//! doorway on the way once it is in sight and near (`LEAD_M`), drawn to a room's point of
//! interest (the goal, a key's altar, lever or brazier) once that is in sight and near while
//! the way on stays in frame, smoothed, and turning no faster than `TURN_PER_M` but where the
//! way ahead itself turns faster. A cinematic
//! camera's look-at as shipped games aim one along a rail (Unity Cinemachine's dolly track
//! with a look-ahead target and damping, Unreal's Sequencer rail rigs), worked out ahead of
//! time over the cubes, as the tour is.
//!
//! Round 13's reviewers saw the camera stare at a dead end's wall for a second, then yaw
//! fast: its look aimed at the farthest point the way runs straight to, a corridor's end
//! wall where the way turns. Round 15 aimed at the farthest point of the way the eye sees
//! instead, and the plain look at the way 4 m on beat it on every measure (a wall within
//! 2.5 m faced straight on in 0.11% of looks against 3%, and more open down the look).

use super::view::STEP_M;
use super::{Object, ObjectRole, clear, cube_of};
use glam::{IVec3, Quat, Vec3};
use std::collections::HashMap;
use terra_build::Cube;

type Map = HashMap<IVec3, Cube>;

/// How far along the way the point the camera looks at stands, in metres: near enough that
/// a corner ahead turns the look into the way it turns into, not at its end wall.
pub const AHEAD_M: f32 = 4.0;
/// It aims this far under the eye over the way ahead, so the floor shows (a walker's gaze
/// falls a little).
const LOWER_M: f32 = 0.3;
/// The next doorway the way passes draws the look's yaw from this far along the way, in
/// metres, and holds it wholly from the second (`LEAD_OFF_RAD`): round 5's barrow film
/// crossed a 2x2 room toward its south door looking at the blank west wall 5-7 m off for
/// 2.5 s, the way 4 m on still bending round the room's middle. As a walker looks where they
/// go out, and a rail camera's look-ahead target leads the turn. With a doorway 3.5-9 m on in
/// sight, the frame's middle met a wall within 5 m in 5.7/4.1/4.3% of looks (crypt, mine,
/// keep), 3.3/2.2/2.0% led (the plain look at the way 4 m on, unsmoothed: 2.9/1.6/1.8%).
pub const LEAD_M: [f32; 2] = [12.0, 8.0];
/// A doorway this far off the way on, in radians, leads the look wholly, and from the
/// second not at all, so the way on stays in frame (the look test's 0.7 rad).
const LEAD_OFF_RAD: [f32; 2] = [0.6, 0.9];
/// A doorway leads the look only where the look turned to it runs clear this far either side
/// of it, in radians, for the second's metres (`led`).
const GRAZE_RAD: f32 = 0.15;
const GRAZE_M: f32 = 2.0;
/// The doorway lets the look go back to the way on from the first distance to the second, in
/// metres, so it passes through square.
const THROUGH_M: [f32; 2] = [1.5, 3.5];
/// The most a point of interest turns the look off the way `AHEAD_M` on, in radians, so the
/// way on stays in frame (a 20 mm lens's frame is some 84° across, 0.73 rad either side).
pub const FRAME_RAD: f32 = 0.5;
/// A point of interest starts to draw the eye at the first distance and holds it wholly at the
/// second, in metres; only when in sight and within `INTEREST_COS` of the way ahead.
pub const INTEREST_M: [f32; 2] = [9.0, 3.5];
/// The cosine of the widest angle off the way ahead a point of interest draws the eye from.
const INTEREST_COS: f32 = 0.26;
/// How high over an object's place its middle stands, which the eye aims at.
const INTEREST_UP_M: f32 = 0.5;
/// The look's directions are averaged over this much of the way either side.
const SMOOTH_M: f32 = 1.0;
/// The most the look turns, left or right or up or down, a metre of the way, in radians
/// (20°: 50° a second at the film's 2.5 m/s, a slow pan).
pub const TURN_PER_M: f32 = 0.35;
/// Once the look lags more than `HARD_RAD` off the way `AHEAD_M` on (out of a dead end,
/// round a sharp corner), it may turn this much a metre, in radians (60°: a half turn in some
/// 1.2 s at 2.5 m/s, as a walker turns round): the hard limit of a shipped camera's composer
/// (Cinemachine's), which keeps its target in frame whatever its damping. Held to
/// `TURN_PER_M` alone, a half turn out of a dead end took 9 m, the way on out of frame for
/// 8.8 m (round 16); at 50° a metre, 2.8 m.
pub const TURN_MOST_PER_M: f32 = 1.05;
/// How far off the way on, in radians, the look may lag before it turns at `TURN_MOST_PER_M`.
pub const HARD_RAD: f32 = 0.5;
/// How far out along its look each look point stands, in metres.
pub const LOOK_M: f32 = 4.0;
/// The most the look pitches up or down, in radians (30°): round 17's mine film pitched to
/// 59° down at a well, the way 4 m on a short chord under the eye.
pub const PITCH_MOST: f32 = 0.52;
/// The dolly's pace along the view where the look turns slowly, in metres a second.
pub const DOLLY_M_S: f32 = 2.5;
/// The fastest the look turns at the dolly's pace, in radians a second (26°, a slow pan):
/// the dolly slows where it would turn faster (round 17's mine film yawed 168° in 1.9 s
/// out of a dead end at the plain 2.5 m/s, a whip pan; round 7's barrow reviewers saw 60°
/// in 1.75 s at the old 45° as a swing along a bare wall).
pub const TURN_MOST_PER_S: f32 = 0.45;
/// The dolly's slowing is eased over this much of the view either side, in metres.
const EASE_M: f32 = 1.5;
/// Under a theme's vaults (D527) the look tilts up this much, in radians (10°: with
/// `LOWER_M`, 4.4° up on average under the crypt's vaults where the way runs level, against
/// 4.7° down without), so the barrel and groin vaults read: round 2's barrow
/// reviewers saw the vaults black or out of frame at eye height with the look level, and
/// asked 5-10° up in halls.
pub const VAULT_TILT: f32 = 0.17;
/// The tilt starts where the clear height over the eye is the first, in metres, and is whole
/// at the second: the crypt's flat cubes over its vaults stand 2.9 m over a standing eye in a
/// corridor, a doorway's head 1.4 m, so the look levels passing under one.
pub const VAULT_HEADROOM_M: [f32; 2] = [1.6, 2.6];

/// Whether the line between two points passes only open cubes, every 0.1 m.
pub(super) fn seen(map: &Map, a: Vec3, b: Vec3) -> bool {
    let steps = ((b - a).length() / 0.1).ceil().max(1.0) as i32;
    (0..=steps).all(|k| clear(map, cube_of(a.lerp(b, k as f32 / steps as f32))))
}

/// How far the cubes are clear straight over a point, up to 4 m, every 0.1 m.
pub(super) fn headroom(map: &Map, at: Vec3) -> f32 {
    (1..=40)
        .map(|k| k as f32 * 0.1)
        .find(|&up| !clear(map, cube_of(at + Vec3::Y * up)))
        .unwrap_or(4.0)
}

/// The eyes of the view nearest each doorway in plan, within 0.75 m, in order.
pub(super) fn passes(view: &[Vec3], doorways: &[Vec3]) -> Vec<usize> {
    let plan = |a: Vec3, b: Vec3| (a.x - b.x).hypot(a.z - b.z);
    let mut out: Vec<usize> = doorways
        .iter()
        .filter_map(|&d| {
            (0..view.len())
                .filter(|&i| (view[i].y - d.y).abs() < 2.5)
                .min_by(|&a, &b| plan(view[a], d).total_cmp(&plan(view[b], d)))
                .filter(|&i| plan(view[i], d) < 0.75)
        })
        .collect();
    out.sort_unstable();
    out
}

/// A point `LOOK_M` out along the camera's look from each eye of the view, tilted up by
/// `tilt` radians where the eye stands under a vault (`VAULT_TILT` for a theme with a kit,
/// nought for one without), and led toward the next of `doorways` in sight.
pub(super) fn looks(
    map: &Map,
    view: &[Vec3],
    doorways: &[Vec3],
    objects: &[Object],
    tilt: f32,
) -> Vec<Vec3> {
    let n = view.len();
    if n < 2 {
        return view.to_vec();
    }
    let passes = passes(view, doorways);
    let interests: Vec<Vec3> = objects
        .iter()
        .filter(|o| matches!(o.role, ObjectRole::Goal | ObjectRole::Key))
        .map(|o| o.at + Vec3::Y * INTEREST_UP_M)
        .collect();
    let lead = (AHEAD_M / STEP_M).round() as usize;
    // Where the way 4 m on comes back to the eye (a dead end, the tour's end), the way on
    // held as it was: round 17's mine film fell back to the whole tour's last direction
    // there, a jump the turn bound then spread into a whip pan.
    let mut held = level((view[1] - view[0]).normalize_or(Vec3::NEG_Z), PITCH_MOST);
    let ons: Vec<Vec3> = (0..n)
        .map(|i| {
            let to = view[(i + lead).min(n - 1)] - Vec3::Y * LOWER_M - view[i];
            if to.length() > 0.5 {
                let (h, [lo, hi]) = (headroom(map, view[i]), VAULT_HEADROOM_M);
                let (yaw, pitch) = angles(to.normalize());
                let up = tilt * terra_gen::smoothstep(lo, hi, h);
                let yaw = led(map, view, &passes, i, yaw, pitch + up);
                held = level(from_angles(yaw, pitch + up), PITCH_MOST);
            }
            held
        })
        .collect();
    let raw: Vec<Vec3> = (0..n)
        .map(|i| {
            let (eye, on) = (view[i], ons[i]);
            let mut dir = on;
            for &p in &interests {
                let to = p - eye;
                let d = to.length();
                if !(1e-3..=INTEREST_M[0]).contains(&d) || to.normalize().dot(on) < INTEREST_COS {
                    continue;
                }
                if !seen(map, eye, p) {
                    continue;
                }
                let w = terra_gen::smoothstep(INTEREST_M[0], INTEREST_M[1], d);
                dir = dir.lerp(to.normalize(), w).normalize_or(on);
            }
            within(dir, on, FRAME_RAD)
        })
        .collect();
    let reach = (SMOOTH_M / STEP_M).round() as usize;
    let smooth: Vec<Vec3> = (0..n)
        .map(|i| {
            let near = &raw[i.saturating_sub(reach)..(i + reach + 1).min(n)];
            near.iter().sum::<Vec3>().normalize_or(raw[i])
        })
        .collect();
    turned(&smooth, &ons, TURN_PER_M * STEP_M, TURN_MOST_PER_M * STEP_M)
        .into_iter()
        .zip(view)
        .map(|(d, &eye)| eye + d * LOOK_M)
        .collect()
}

/// The yaw of the look from eye `i` of the view, turned from the way on's toward the next
/// doorway it passes once that is in sight and near (`LEAD_M`), where the look so turned
/// runs clear as far as the doorway (`LOOK_M` at most): half turned toward a doorway seen
/// past a jamb, it would face the wall beside it.
fn led(map: &Map, view: &[Vec3], passes: &[usize], i: usize, yaw: f32, pitch: f32) -> f32 {
    let Some(&j) = passes.iter().find(|&&j| j > i) else {
        return yaw;
    };
    let along = (j - i) as f32 * STEP_M;
    let w = terra_gen::smoothstep(LEAD_M[0], LEAD_M[1], along)
        * terra_gen::smoothstep(THROUGH_M[0], THROUGH_M[1], along);
    let door = view[j] - Vec3::Y * LOWER_M;
    if w <= 0.0 || !seen(map, view[i], door) {
        return yaw;
    }
    let (door_yaw, _) = angles((door - view[i]).normalize_or(Vec3::NEG_Z));
    let off = wrapped(door_yaw - yaw);
    let turned = yaw + off * w * terra_gen::smoothstep(LEAD_OFF_RAD[1], LEAD_OFF_RAD[0], off.abs());
    let reach = view[i].distance(door).min(LOOK_M);
    let clear_to = |yaw: f32, m: f32| seen(map, view[i], view[i] + from_angles(yaw, pitch) * m);
    // And a little either side near, as the smoothing and the turn bound leave the look some
    // 0.1 rad off it: round 6's first lead grazed a pier's or jamb's corner within 1.5 m in
    // 0.02% more looks than the plain look did.
    let side = [-GRAZE_RAD, GRAZE_RAD].map(|o| clear_to(turned + o, GRAZE_M));
    if clear_to(turned, reach) && side.iter().all(|&c| c) {
        turned
    } else {
        yaw
    }
}

/// A direction pitched no more than `most` radians up or down.
fn level(d: Vec3, most: f32) -> Vec3 {
    let (yaw, pitch) = angles(d);
    from_angles(yaw, pitch.clamp(-most, most))
}

/// The direction of a yaw and pitch, in radians.
fn from_angles(yaw: f32, pitch: f32) -> Vec3 {
    let flat = libm::cosf(pitch);
    Vec3::new(
        libm::sinf(yaw) * flat,
        libm::sinf(pitch),
        -libm::cosf(yaw) * flat,
    )
}

/// The seconds the dolly takes from each eye of the view to the next (nought at the last):
/// `DOLLY_M_S`, slowed where the look would turn faster than `TURN_MOST_PER_S`, the slowing
/// eased over `EASE_M` either side. Eased as the most needed within `EASE_M` averaged over
/// `EASE_M`, so no step is quicker than its own turn allows.
pub(super) fn pace(view: &[Vec3], look: &[Vec3]) -> Vec<f32> {
    let n = view.len();
    if n < 2 {
        return vec![0.0; n];
    }
    let dir = |i: usize| (look[i] - view[i]).normalize_or(Vec3::NEG_Z);
    let need: Vec<f32> = (0..n - 1)
        .map(|i| {
            let plain = view[i].distance(view[i + 1]) / DOLLY_M_S;
            plain.max(dir(i).angle_between(dir(i + 1)) / TURN_MOST_PER_S)
        })
        .collect();
    let reach = (EASE_M / STEP_M).round() as usize;
    let around = |i: usize| i.saturating_sub(reach)..(i + reach + 1).min(n - 1);
    let most: Vec<f32> = (0..n - 1)
        .map(|i| need[around(i)].iter().fold(0.0f32, |a, &b| a.max(b)))
        .collect();
    let mut out: Vec<f32> = (0..n - 1)
        .map(|i| {
            let near = &most[around(i)];
            near.iter().sum::<f32>() / near.len() as f32
        })
        .collect();
    out.push(0.0);
    out
}

/// A direction turned towards an axis until it is at most `most` radians off it.
fn within(dir: Vec3, axis: Vec3, most: f32) -> Vec3 {
    let off = dir.angle_between(axis);
    if off <= most {
        return dir;
    }
    let (turn, angle) = Quat::from_rotation_arc(axis, dir).to_axis_angle();
    Quat::from_axis_angle(turn, angle * most / off) * axis
}

/// The yaw and pitch of a direction, in radians.
fn angles(d: Vec3) -> (f32, f32) {
    (libm::atan2f(d.x, -d.z), libm::asinf(d.y.clamp(-1.0, 1.0)))
}

/// An angle taken to within half a turn of zero.
fn wrapped(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    a - tau * libm::floorf((a + std::f32::consts::PI) / tau)
}

/// Directions turned towards `dirs` no more than `slow` radians in yaw and in pitch from one
/// to the next, the turn spread after a change; `fast` while one is more than `HARD_RAD` off
/// its `frame` direction.
fn turned(dirs: &[Vec3], frame: &[Vec3], slow: f32, fast: f32) -> Vec<Vec3> {
    let Some(&first) = dirs.first() else {
        return Vec::new();
    };
    let (mut yaw, mut pitch) = angles(first);
    pitch = pitch.clamp(-PITCH_MOST, PITCH_MOST);
    let mut out = vec![from_angles(yaw, pitch)];
    // Clamped forward: the look-ahead already turns before a corner, and a turn spread
    // before the change too swung the look into the corner's wall (round 15: 5.5% of the
    // crypt's looks with a wall within 1.5 m, against 1.1%).
    for (&dir, &on) in dirs.iter().zip(frame).skip(1) {
        let (to_yaw, to_pitch) = angles(dir);
        // The short way round, so a turn through south is not a turn the long way.
        let (dy, dp) = (wrapped(to_yaw - yaw), to_pitch - pitch);
        let step = |most: f32| (dy.clamp(-most, most), dp.clamp(-most, most));
        let (mut sy, mut sp) = step(slow);
        let (on_yaw, on_pitch) = angles(on);
        let off = wrapped(yaw + sy - on_yaw)
            .abs()
            .max((pitch + sp - on_pitch).abs());
        if off > HARD_RAD {
            (sy, sp) = step(fast);
        }
        yaw += sy;
        pitch = (pitch + sp).clamp(-PITCH_MOST, PITCH_MOST);
        out.push(from_angles(yaw, pitch));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_is_spread_and_never_faster_than_the_bound() {
        // A right angle in one step becomes a turn over several, none past the bound.
        let mut dirs = vec![Vec3::NEG_Z; 20];
        dirs.extend(vec![Vec3::X; 20]);
        let out = turned(&dirs, &dirs, 0.2, 0.2);
        for w in out.windows(2) {
            assert!(w[0].angle_between(w[1]) <= 0.2 + 1e-4);
        }
        assert!(out[0].angle_between(Vec3::NEG_Z) < 1e-4);
        assert!(out[39].angle_between(Vec3::X) < 1e-4);
        // Spread after the change.
        assert!(out[20].angle_between(Vec3::X) > 0.3);
        // And through south the short way: from south-west to south-east is a quarter turn.
        let sw = Vec3::new(-1.0, 0.0, 1.0).normalize();
        let se = Vec3::new(1.0, 0.0, 1.0).normalize();
        let out = turned(&[sw, se], &[sw, se], 10.0, 10.0);
        assert!(out[1].angle_between(se) < 1e-4);
        let out = turned(&[sw, se], &[sw, se], 0.5, 0.5);
        assert!(out[0].angle_between(out[1]) <= 0.5 + 1e-4);
    }

    #[test]
    fn the_dolly_slows_where_the_look_turns_fast_and_only_there() {
        // A straight way, then a quarter turn of the look in one step: 2.5 m/s along the
        // straight, and slowed about the turn so that no step turns faster than the bound.
        let view: Vec<Vec3> = (0..60).map(|i| Vec3::Z * (i as f32 * STEP_M)).collect();
        let look: Vec<Vec3> = view
            .iter()
            .enumerate()
            .map(|(i, &p)| p + if i < 30 { Vec3::Z } else { Vec3::X })
            .collect();
        let pace = pace(&view, &look);
        assert_eq!(pace.len(), view.len());
        let plain = STEP_M / DOLLY_M_S;
        assert!((pace[5] - plain).abs() < 1e-5, "plain: {}", pace[5]);
        assert!((pace[55] - plain).abs() < 1e-5, "plain after: {}", pace[55]);
        let turn = std::f32::consts::FRAC_PI_2 / TURN_MOST_PER_S;
        assert!(pace[29] >= turn - 1e-5, "the turn's step {}", pace[29]);
        // Eased: the steps either side slower than plain too, some way off.
        assert!(pace[26] > plain && pace[32] > plain);
        // A look pitched steeply is held to the most pitch.
        assert!(level(Vec3::new(0.0, -1.0, -0.3).normalize(), PITCH_MOST).y >= -0.5);
    }

    #[test]
    fn a_look_out_of_frame_turns_faster_and_only_then() {
        // A half turn: fast while the look is past the hard limit off its frame, slow once
        // back within it.
        let mut dirs = vec![Vec3::NEG_Z; 5];
        dirs.extend(vec![Vec3::Z; 30]);
        let out = turned(&dirs, &dirs, 0.05, 0.3);
        let steps: Vec<f32> = out.windows(2).map(|w| w[0].angle_between(w[1])).collect();
        assert!((steps[4] - 0.3).abs() < 1e-3, "first fast: {}", steps[4]);
        assert!(steps.iter().all(|&s| s <= 0.3 + 1e-4));
        assert!(steps[20] <= 0.05 + 1e-4, "then slow: {}", steps[20]);
        // Done in 30 steps, where the slow bound alone would leave it 1.6 rad off.
        assert!(out[34].angle_between(Vec3::Z) < 1e-3);
    }
}
