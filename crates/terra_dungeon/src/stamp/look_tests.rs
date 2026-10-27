//! The tour's look from every eye: down the way on, off walls, turning no faster than a slow
//! pan and not swinging back the way it came.

use super::tests::{SEEDS, kind};
use super::*;
use crate::theme::assets;
use crate::{ModuleSet, generate};
use terra_gen::Seed;

#[derive(Default)]
struct Looked {
    samples: usize,
    /// Looks with the way 4 m on more than `FRAME_RAD` off them, out of frame, but where the
    /// way turns back on itself (its 4 m behind and 4 m on over 120° apart), where a camera
    /// turning round has it out of frame for a moment whatever it does.
    lost: usize,
    /// The longest stretch of the way, in metres, so.
    lost_m: f32,
    /// Looks facing a wall within `NEAR_M`, within 45° of straight on.
    walled: usize,
    /// The same for a look at the way 4 m on.
    naive_walled: usize,
    /// How far down the look is open, summed, and the same for a look at the way 4 m on.
    open_m: f32,
    naive_open_m: f32,
    turn: f32,
    goal_off: f32,
    /// The fastest the look turns over `PAN_S`, in radians a second, at the stamp's pace and
    /// at the plain dolly's 2.5 m/s.
    pan: f32,
    plain_pan: f32,
    /// The most the look turns over any `LONG_PAN_S` at the stamp's pace, in radians a
    /// second: round 7's barrow reviewers saw 60° in 1.75 s as a swing.
    long_pan: f32,
    /// The most the look yaws back the way it came within `SWING_S`, in radians: half its
    /// yaw travelled there less its net yaw, nought for a turn one way.
    swing: f32,
    /// Where the look swings back most: the seconds into the tour and the eye.
    swing_at: (f32, Vec3),
    seed: u64,
    /// Frames with a wall within `WALL_M` of the eye anywhere in them (`frame_rays`), and
    /// within a metre.
    walled_frame: usize,
    metre_frame: usize,
    /// The nearest wall in any frame, in metres (up to 1.5).
    nearest: f32,
    /// Eyes back within `RETRACE_M` of where the eye stood more than `RETRACE_ALONG_M` of the
    /// view before: the tour gone into a dead end and out the way it came, which a film cuts
    /// out, turning on the spot where it cut (round 20's mine and keep, 2-7 s at a wall).
    retraced: usize,
    /// Looks where the way turns back on itself, its 4 m behind and 4 m on over 120° apart.
    turned_back: usize,
    /// The steepest the look pitches, up or down, in radians.
    pitch: f32,
    /// Looks under a vault (`look::VAULT_HEADROOM_M`'s whole tilt) and their pitches summed,
    /// in radians.
    vaulted: usize,
    vault_pitch: f32,
    /// Frames whose middle three rays of the middle row are all on a wall within `FILL_M`: a wall,
    /// jamb or pier filling the frame near (round 2's barrow reviewers, a pier and a wall
    /// over two thirds of the frame, the look panning across a blank wall 2-3 m off).
    filled: usize,
    /// How long the dolly takes from the first look measured to the last, at the stamp's
    /// pace and at 2.5 m/s, in seconds.
    seconds: f32,
    plain_seconds: f32,
}

/// The look's turn is measured over this long, in seconds (round 17's mine whip: 168° in
/// 1.9 s, some 90° a second).
const PAN_S: f32 = 0.5;
/// The look's sustained turn and its swing back are measured over these, in seconds.
const LONG_PAN_S: f32 = 1.75;
const SWING_S: f32 = 4.0;
/// A look that yaws back more than this within `SWING_S` swings (round 7's crypt film: 25°
/// north into a room, then 115° south out of it).
const SWING_MOST: f32 = 0.26;
/// A wall this near the eye in frame fills it (the smear round 18 saw at 0.3-0.6 m).
const WALL_M: f32 = 0.5;

/// Rays across a 20 mm lens's frame, 84° by 53°: five across and three up, through its
/// corners, edges' middles and middle.
fn frame_rays(d: Vec3) -> impl Iterator<Item = Vec3> {
    let right = d.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = right.cross(d);
    let (across, high) = (
        libm::tanf(42f32.to_radians()),
        libm::tanf(26.5f32.to_radians()),
    );
    (0..15).map(move |k| {
        let (u, w) = ((k % 5) as f32 / 2.0 - 1.0, (k / 5) as f32 - 1.0);
        (d + right * (u * across) + up * (w * high)).normalize()
    })
}

/// Where a look is at most this far off the way 4 m on, the way on is in frame (a 20 mm
/// lens's frame is some 84° across).
const FRAME_RAD: f32 = 0.7;
/// A wall at arm's length or so.
const NEAR_M: f32 = 1.5;
/// A wall this near across the frame's middle fills it.
const FILL_M: f32 = 2.5;
/// An eye this near where the eye stood over `RETRACE_ALONG_M` of the view before has come
/// back the way it went: round 20's tour tool cut such a loop out, as a film would.
const RETRACE_M: f32 = 0.5;
const RETRACE_ALONG_M: f32 = 2.0;

/// The look from each eye of one stamp's view, past the surface leg and short of where a
/// film stops (3.5 m from the goal in plan): how often the way on is out of frame and the
/// longest stretch so, how often it faces a wall near, how far down it is open (a cube met
/// marching the ray every 0.05 m, up to 8 m) against a look at the way 4 m on, the fastest
/// turn a metre, and how far off the goal it looks where the film stops.
fn check_look(theme: &Theme, dungeon: &Dungeon) -> Looked {
    use super::view::STEP_M;
    let stamp = stamp(dungeon, theme, &kind);
    let map = cubes(&stamp.ops);
    let (view, look) = (&stamp.view, &stamp.look);
    assert_eq!(view.len(), look.len(), "a look an eye");
    assert_eq!(view.len(), stamp.pace.len(), "a pace an eye");
    let dir = |i: usize| (look[i] - view[i]).normalize();
    let goal = stamp
        .objects
        .iter()
        .find(|o| o.role == ObjectRole::Goal)
        .expect("a goal");
    let stop = (0..view.len())
        .rev()
        .find(|&i| {
            let off = view[i] - goal.at;
            off.x.hypot(off.z) >= 3.5
        })
        .unwrap_or(view.len() - 1);
    let surface = stamp.route[0].distance(stamp.route[1]);
    let lead = (4.0 / STEP_M) as usize;
    // How far a ray from an eye runs through open cubes, up to `most` metres, and the axis it
    // stepped across into the cube it meets (nought if none).
    let hit = |from: Vec3, d: Vec3, most: f32| {
        let mut was = cube_of(from);
        for k in 1..=(most / 0.05).round() as i32 {
            let c = cube_of(from + d * (k as f32 * 0.05));
            if !clear(&map, c) {
                return (k as f32 * 0.05, c - was);
            }
            was = c;
        }
        (most, IVec3::ZERO)
    };
    // How far it runs, up to 8 m, and whether the cube it meets faces it within 45° of
    // straight on.
    let open = |from: Vec3, d: Vec3| {
        let (free, across) = hit(from, d, 8.0);
        let face = if across.x != 0 {
            d.x
        } else if across.y != 0 {
            d.y
        } else {
            d.z
        };
        (
            free,
            across != IVec3::ZERO && face.abs() > std::f32::consts::FRAC_1_SQRT_2,
        )
    };
    // When the dolly reaches each eye, at the stamp's pace and at the plain 2.5 m/s.
    let mut times = vec![0.0f32];
    let mut plain = vec![0.0f32];
    for i in 1..view.len() {
        times.push(times[i - 1] + stamp.pace[i - 1]);
        plain.push(plain[i - 1] + view[i].distance(view[i - 1]) / 2.5);
    }
    let pan = |t: &[f32], i: usize| {
        (i + 1..view.len())
            .find(|&j| t[j] - t[i] >= PAN_S)
            .map_or(0.0, |j| dir(i).angle_between(dir(j)) / (t[j] - t[i]))
    };
    // The look's yaw unwrapped along the view, and how far it yaws over a window of seconds
    // from eye `i`: net, and travelled either way.
    let mut yaw = vec![libm::atan2f(dir(0).x, -dir(0).z)];
    for i in 1..view.len() {
        let (d, was) = (dir(i), yaw[i - 1]);
        let step = libm::atan2f(d.x, -d.z) - was;
        let tau = std::f32::consts::TAU;
        yaw.push(was + step - tau * libm::floorf((step + std::f32::consts::PI) / tau));
    }
    let yawed = |i: usize, s: f32| {
        let end = (i + 1..view.len())
            .find(|&j| j >= stop || times[j] - times[i] >= s)
            .unwrap_or(stop);
        let travel: f32 = (i..end).map(|j| (yaw[j + 1] - yaw[j]).abs()).sum();
        ((yaw[end] - yaw[i]).abs(), travel, end)
    };
    let mut seen = Looked {
        nearest: 1.5,
        ..Looked::default()
    };
    let mut run = 0.0f32;
    for i in 0..stop {
        if (i as f32) * STEP_M < surface + 1.0 {
            continue;
        }
        let on = view[(i + lead).min(view.len() - 1)] - view[i];
        if on.length() < 0.5 {
            continue;
        }
        seen.samples += 1;
        let d = dir(i);
        let lost = d.angle_between(on) > FRAME_RAD;
        let behind = view[i] - view[i.saturating_sub(lead)];
        let back = behind.length() > 0.5 && behind.angle_between(on) > 2.1;
        seen.turned_back += back as usize;
        let gone_by = i.saturating_sub((RETRACE_ALONG_M / STEP_M) as usize);
        seen.retraced += (0..gone_by).any(|j| view[j].distance(view[i]) < RETRACE_M) as usize;
        seen.lost += (lost && !back) as usize;
        run = if lost { run + STEP_M } else { 0.0 };
        seen.lost_m = seen.lost_m.max(run);
        let (free, flat) = open(view[i], d);
        seen.walled += (free < NEAR_M && flat) as usize;
        seen.open_m += free;
        let (naive_free, naive_flat) = open(view[i], on.normalize());
        seen.naive_open_m += naive_free;
        seen.naive_walled += (naive_free < NEAR_M && naive_flat) as usize;
        if i + 1 < view.len() {
            seen.turn = seen.turn.max(d.angle_between(dir(i + 1)) / STEP_M);
        }
        seen.pan = seen.pan.max(pan(&times, i));
        seen.plain_pan = seen.plain_pan.max(pan(&plain, i));
        let (_, _, end) = yawed(i, LONG_PAN_S);
        let secs = times[end] - times[i];
        if secs >= LONG_PAN_S {
            seen.long_pan = seen.long_pan.max(d.angle_between(dir(end)) / secs);
        }
        let (net, travel, _) = yawed(i, SWING_S);
        let back = 0.5 * (travel - net);
        if back > seen.swing {
            (seen.swing, seen.swing_at) = (back, (times[i], view[i]));
        }
        let wall = |r: Vec3| match hit(view[i], r, 1.5) {
            (free, across) if across.y == 0 && across != IVec3::ZERO => free,
            _ => 1.5,
        };
        let nearest = frame_rays(d).map(wall).fold(1.5, f32::min);
        seen.walled_frame += (nearest < WALL_M) as usize;
        seen.metre_frame += (nearest < 1.0) as usize;
        seen.nearest = seen.nearest.min(nearest);
        seen.pitch = seen.pitch.max(libm::asinf(d.y.abs().min(1.0)));
        // Under a vault where the way on runs within 5° of level: a hall or corridor, not a
        // stair's run.
        let level = libm::asinf((on.y / on.length()).abs().min(1.0)) < 0.09;
        if level && super::look::headroom(&map, view[i]) >= super::look::VAULT_HEADROOM_M[1] {
            seen.vaulted += 1;
            seen.vault_pitch += libm::asinf(d.y.clamp(-1.0, 1.0));
        }
        let near = |r: Vec3| {
            let (_, across) = hit(view[i], r, FILL_M);
            across.y == 0 && across != IVec3::ZERO
        };
        // The middle three of the middle row's five, its middle 48°.
        seen.filled += frame_rays(d).skip(6).take(3).all(near) as usize;
        if i + 1 < view.len() {
            seen.seconds += times[i + 1] - times[i];
            seen.plain_seconds += plain[i + 1] - plain[i];
        }
    }
    seen.goal_off = dir(stop).angle_between(goal.at + Vec3::Y * 0.5 - view[stop]);
    seen
}

#[test]
fn the_camera_looks_down_the_way_ahead_not_at_walls() {
    use super::look::{PITCH_MOST, TURN_MOST_PER_M, TURN_MOST_PER_S};
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let seeds: Vec<u64> = (0..SEEDS).collect();
        let seen: Vec<Looked> = terra_gen::par::map(&seeds, |&s| {
            let dungeon = generate(&theme, &set, Seed(s)).ok()?;
            Some(Looked {
                seed: s,
                ..check_look(&theme, &dungeon)
            })
        })
        .into_iter()
        .flatten()
        .collect();
        let sum = |f: &dyn Fn(&Looked) -> f32| seen.iter().map(f).sum::<f32>();
        let samples = sum(&|s| s.samples as f32);
        let lost = sum(&|s| s.lost as f32) / samples;
        let walled = sum(&|s| s.walled as f32) / samples;
        let naive_walled = sum(&|s| s.naive_walled as f32) / samples;
        let open = sum(&|s| s.open_m) / samples;
        let naive_open = sum(&|s| s.naive_open_m) / samples;
        let most = |f: &dyn Fn(&Looked) -> f32| seen.iter().map(f).fold(0.0, f32::max);
        let (lost_m, turn) = (most(&|s| s.lost_m), most(&|s| s.turn));
        let goal_off = most(&|s| s.goal_off);
        let (pan, plain_pan) = (most(&|s| s.pan), most(&|s| s.plain_pan));
        let (long_pan, swing) = (most(&|s| s.long_pan), most(&|s| s.swing));
        let swings = seen.iter().filter(|s| s.swing > SWING_MOST).count();
        let mut worst: Vec<&Looked> = seen.iter().collect();
        worst.sort_by(|a, b| b.swing.total_cmp(&a.swing));
        let demo = seen.iter().filter(|s| s.seed == 7);
        for w in worst.iter().take(3).copied().chain(demo) {
            println!(
                "{}: seed {} swings back {:.0}° at {:.1} s, {:.1}",
                theme.name,
                w.seed,
                w.swing.to_degrees(),
                w.swing_at.0,
                w.swing_at.1
            );
        }
        println!(
            "{}: yawing {:.0}°/s over {LONG_PAN_S} s at most; swinging back {:.0}° within {SWING_S} s at most, over {:.0}° in {swings} of {} tours",
            theme.name,
            long_pan.to_degrees(),
            swing.to_degrees(),
            SWING_MOST.to_degrees(),
            seen.len()
        );
        // Round 7's barrow film yawed 60° in 1.75 s crossing a room and its reviewers saw a
        // swing: no sustained yaw past the slow pan's bound.
        assert!(
            long_pan <= TURN_MOST_PER_S * 1.01,
            "{}: yaws {}°/s over {LONG_PAN_S} s",
            theme.name,
            long_pan.to_degrees()
        );
        let walled_frame = sum(&|s| s.walled_frame as f32) / samples;
        let metre_frame = sum(&|s| s.metre_frame as f32) / samples;
        let nearest = seen.iter().map(|s| s.nearest).fold(1.5, f32::min);
        let pitch = most(&|s| s.pitch);
        let vault_pitch = sum(&|s| s.vault_pitch) / sum(&|s| s.vaulted as f32).max(1.0);
        let filled = sum(&|s| s.filled as f32) / samples;
        let vaulted = sum(&|s| s.vaulted as f32) / samples;
        println!(
            "{}: under a vault {:.0}% of looks, pitched {:.1}° on average there; a wall filling the frame's middle within {FILL_M} m {:.2}% of frames",
            theme.name,
            100.0 * vaulted,
            vault_pitch.to_degrees(),
            100.0 * filled
        );
        // Under a theme's vaults the look tilts up so they read (round 2's barrow reviewers:
        // the vaults black or out of frame at eye height, the look level; a theme without
        // vaults looks some 4.7° down there, at the floor 4 m on).
        if theme.kit.is_some() {
            assert!(
                vault_pitch > 3f32.to_radians() && vaulted > 0.3,
                "{}: {vault_pitch} up under a vault, {vaulted} of looks",
                theme.name
            );
        }
        // No wall fills the frame's middle near (round 2's barrow film panned across a blank
        // wall some 3 m off for 1.8 s, which no tour of 200 did within 2.5 m, and 0.1% within
        // 4 m).
        assert!(
            filled <= 0.001,
            "{}: a wall filling the frame's middle in {filled} of frames",
            theme.name
        );
        let retraced = sum(&|s| s.retraced as f32) / samples;
        let turned_back = sum(&|s| s.turned_back as f32) / samples;
        let slower = sum(&|s| s.seconds) / sum(&|s| s.plain_seconds);
        println!(
            "{}: turning {:.0}°/s over {PAN_S} s at most at the tour's pace ({:.0} at 2.5 m/s), the tour {slower:.2} times as long; pitched {:.0}° at most; a wall in frame within {WALL_M} m {:.2}% of frames, within 1 m {:.2}%, {nearest:.2} m at the nearest; back where it was {:.2}% of eyes, the way turning back on itself {:.2}%",
            theme.name,
            pan.to_degrees(),
            plain_pan.to_degrees(),
            pitch.to_degrees(),
            100.0 * walled_frame,
            100.0 * metre_frame,
            100.0 * retraced,
            100.0 * turned_back
        );
        // A showcase walk never goes into a dead end it must turn round in (round 20's films
        // turned on the spot at a key room's back wall for 2-7 s; 14/14/28% of eyes came
        // back where they had been, crypt/mine/keep), and turns sharply round only where a
        // room's way out is beside its way in, short of its middle (4.7/4.3/6.1% of looks
        // with the way turning back on itself then, 0.27/0.39/0.27 round 21).
        assert!(
            retraced == 0.0 && turned_back < 0.01,
            "{}: back where it was in {retraced} of eyes, the way turning back {turned_back}",
            theme.name
        );
        println!(
            "{}: {} tours, {samples} looks: the way on out of frame {:.2}% but turning round ({lost_m:.1} m at most), facing a wall within {NEAR_M} m {:.2}% ({:.2}), open {open:.2} m down the look on average ({naive_open:.2} looking at the way 4 m on), turning {:.0}° a metre at most, the goal {:.0}° off at the stop at most",
            theme.name,
            seen.len(),
            100.0 * lost,
            100.0 * walled,
            100.0 * naive_walled,
            turn.to_degrees(),
            goal_off.to_degrees()
        );
        // Round 13's films looked down a corridor's straight to its end wall, the way on 90°
        // off and out of frame for a second (the crypt's 16 s), then whipped round. Turning
        // round out of a dead end, at most 2.5 m (some 2.3 at `TURN_MOST_PER_M`).
        assert!(
            lost < 0.02 && lost_m <= 2.5,
            "{}: the way on out of frame {lost}, {lost_m} m",
            theme.name
        );
        // Facing a wall near no more often than the plain look at the way 4 m on, give or
        // take a thousandth (round 15's look at the farthest point it saw: 3% against 0.11%).
        // The plain look itself faces one in 1.09% of the mine's looks, along straight ways:
        // the view's, not the look's (round 16).
        assert!(
            walled <= naive_walled + 0.001,
            "{}: {walled} of looks at a wall near, against {naive_walled}",
            theme.name
        );
        // As open in front of it as the plain look at the way 4 m on, within a twentieth: the
        // smoothing, the turn bound and the pull towards the goal cost it little.
        assert!(
            open > 0.95 * naive_open,
            "{}: open {open} m against {naive_open}",
            theme.name
        );
        // At most the bound in yaw and in pitch at once.
        let bound = TURN_MOST_PER_M * std::f32::consts::SQRT_2 * 1.01;
        assert!(turn <= bound, "{}: turns {turn} a metre", theme.name);
        assert!(goal_off < 0.35, "{}: the goal {goal_off} off", theme.name);
        // Round 17's mine film whipped round out of a dead end, some 90° a second; at the
        // tour's pace no look turns faster than a slow pan.
        assert!(
            pan <= TURN_MOST_PER_S * 1.01,
            "{}: turns {}°/s",
            theme.name,
            pan.to_degrees()
        );
        // The view keeps the eye off walls (`view::ROOM_M`), and the look keeps a wall from
        // filling the frame: round 18 saw walls smeared at 0.3-0.6 m.
        // Round 17's mine film pitched 59° down at a well.
        assert!(
            pitch <= PITCH_MOST + 1e-3,
            "{}: pitched {}°",
            theme.name,
            pitch.to_degrees()
        );
        assert!(
            walled_frame <= 0.001 && metre_frame < 0.03 && nearest >= 0.6,
            "{}: a wall within {WALL_M} m in {walled_frame} of frames, {nearest} m at the nearest",
            theme.name
        );
    }
}
