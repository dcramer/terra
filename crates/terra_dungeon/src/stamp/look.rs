//! Where the camera looks along the tour: at the way `AHEAD_M` on, drawn to a room's point of
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

/// Whether the line between two points passes only open cubes, every 0.1 m.
pub(super) fn seen(map: &Map, a: Vec3, b: Vec3) -> bool {
    let steps = ((b - a).length() / 0.1).ceil().max(1.0) as i32;
    (0..=steps).all(|k| clear(map, cube_of(a.lerp(b, k as f32 / steps as f32))))
}

/// A point `LOOK_M` out along the camera's look from each eye of the view.
pub(super) fn looks(map: &Map, view: &[Vec3], objects: &[Object]) -> Vec<Vec3> {
    let n = view.len();
    if n < 2 {
        return view.to_vec();
    }
    let interests: Vec<Vec3> = objects
        .iter()
        .filter(|o| matches!(o.role, ObjectRole::Goal | ObjectRole::Key))
        .map(|o| o.at + Vec3::Y * INTEREST_UP_M)
        .collect();
    let lead = (AHEAD_M / STEP_M).round() as usize;
    let last = (view[n - 1] - view[n - 2]).normalize_or(Vec3::NEG_Z);
    let ons: Vec<Vec3> = (0..n)
        .map(|i| {
            let to = view[(i + lead).min(n - 1)] - Vec3::Y * LOWER_M - view[i];
            if to.length() > 0.5 {
                to.normalize()
            } else {
                last
            }
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
    let mut out = vec![first];
    let (mut yaw, mut pitch) = angles(first);
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
        pitch += sp;
        let flat = libm::cosf(pitch);
        out.push(Vec3::new(
            libm::sinf(yaw) * flat,
            libm::sinf(pitch),
            -libm::cosf(yaw) * flat,
        ));
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
