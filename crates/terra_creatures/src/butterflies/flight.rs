//! How a group of butterflies flies (docs/ambient-life.md, "Butterflies as things in the
//! world"): each wanders about the group's middle on a path of its own that never runs
//! straight, jinking from side to side as it goes, rising on each bout of wing beats and
//! sinking in the glide after, and the middle itself drifts slowly about where the group
//! was put. Every place is a formula of the time, so nothing is kept from frame to frame
//! but the clock and nothing leaves its leash. Plain numbers, no Bevy's world.
//!
//! What was read (D654): Skyrim's butterflies (`CritterMoth.psc`, read in Enderal's source,
//! <https://git.eddoursul.win/enderal/enderalse/src/tag/2.1.4/source/scripts/crittermoth.psc>;
//! Bethesda's, so only what it does is taken): each is leashed to its spawner, flies arcs
//! between plants at 1.4 to 2.9 m/s (100 to 200 units), turns at most 90 degrees a second,
//! perches 5 to 15 s, flees an actor within 4.3 m and is gone outside the spawner's hours.
//! And the cabbage white measured in free flight (Jantzen and Eisner 2008,
//! <https://pmc.ncbi.nlm.nih.gov/articles/PMC2575472>): 2.37 m/s, turning on a radius of
//! 1.2 m with 5.2 m/s2 across its path, its wings beating 11.8 times a second.
//!
//! Where we depart: ours do not perch or flee (not built: a later job's), so in place of
//! arcs between perches each flies without rest about its group, on sums of sines in place
//! of a spline to a target; the leash is the group's `spread_m`; ours fly slower than the
//! measured 2.37 m/s, which was of butterflies let go in a cage (a guess, the row's
//! `speed`); and the rise on each bout and the jink's width and rate are guesses, looked at
//! in the game, not measured from a butterfly.

use bevy::math::{Vec2, Vec3};
use std::f32::consts::TAU;
use terra_gen::hash::{mix3, unit3};

/// The jink from side to side: so wide, in metres, at so many radians a second.
const JINK_M: f32 = 0.15;
const JINK_RATE: f32 = 4.5;
/// How fast a group's middle drifts, metres a second.
const ROAMS_MPS: f32 = 0.15;
/// How far a bout of beats lifts a butterfly, metres, sunk again in the glide.
pub const LIFT_M: f32 = 0.08;
/// The two turns of a wander: their shares of its width and the second's rate to the first's.
const WANDER: [(f32, f32); 2] = [(0.65, 1.0), (0.35, 2.7)];
/// Each axis swings so much of a radius, so that both together stay within it.
const AXIS: f32 = 0.7;

/// How a kind flies, as its row says, and a group's own draw of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    /// How far from the group's middle any of it gets, metres.
    pub spread_m: f32,
    /// How far the middle drifts from where the group was put, metres.
    pub roams_m: f32,
    /// Metres a second, on the whole.
    pub speed: f32,
    /// How high over the ground it keeps, least and most, metres.
    pub over_m: (f32, f32),
    /// A bout of beats and the glide after it, seconds.
    pub bout_s: (f32, f32),
    /// Where on its round the middle began.
    pub phase: (f32, f32),
}

impl Flight {
    /// Where the group's middle is at a time, from where it was put: across and along.
    pub fn middle(&self, seconds: f32) -> Vec2 {
        let a = seconds * ROAMS_MPS / self.roams_m.max(0.1);
        Vec2::new(
            libm::sinf(a + self.phase.0),
            libm::sinf(a * 1.37 + self.phase.1),
        ) * self.roams_m
            * AXIS
    }

    /// How far from where the group was put any of it gets, metres.
    pub fn reach_m(&self) -> f32 {
        self.roams_m + self.spread_m
    }

    /// How wide the wander is on each axis, and how fast its first turn goes, in radians a
    /// second, for the whole flight to go at its speed: the mean square speed of a sum of
    /// sines is half the sum of each one's width times its rate, squared, on each axis.
    fn wander(&self) -> (f32, f32) {
        let wide = (self.spread_m * AXIS - JINK_M).max(0.0);
        let jink = JINK_M * JINK_RATE;
        let left = libm::sqrtf((self.speed * self.speed - jink * jink).max(0.01));
        let turns = WANDER
            .iter()
            .map(|(share, rate)| share * share * rate * rate)
            .sum::<f32>();
        (wide, left / (wide.max(0.05) * libm::sqrtf(turns)))
    }
}

/// One butterfly of a group: where on each of its turns it began, how much faster or
/// slower than the kind each goes, and its tag, which says where in its bout and its beat
/// its wings are (the shader reads the same, `assets/shaders/butterfly.wesl`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    pub phases: [f32; 8],
    pub rates: [f32; 2],
    pub tag: u32,
}

/// A group's butterflies, the same for the same seed.
pub fn members(seed: u32, count: usize) -> Vec<Member> {
    (0..count as u32)
        .map(|one| {
            let luck = |salt: u32| unit3(seed, one, salt, 7);
            Member {
                phases: std::array::from_fn(|i| luck(i as u32) * TAU),
                rates: [0.85 + 0.3 * luck(8), 0.85 + 0.3 * luck(9)],
                tag: mix3(seed, one, 10, 7),
            }
        })
        .collect()
}

/// Where a butterfly is at a time, from its group's middle (across, up from the ground,
/// along), and which way it flies in the level. `lift` is how far through the rise of its
/// bout it is, 0 to 1 ([`lift`]).
pub fn place(flight: &Flight, member: &Member, seconds: f32, lift: f32) -> (Vec3, Vec2) {
    let (wide, rate) = flight.wander();
    let [across, along] = [0, 1].map(|axis: usize| {
        // Each axis at rates a little apart, so the path is no ellipse and never closes.
        let own = member.rates[axis] * if axis == 0 { 1.0 } else { 1.13 };
        let (mut at, mut going) = (0.0, 0.0);
        for (turn, (share, times)) in WANDER.iter().enumerate() {
            let a = seconds * rate * times * own + member.phases[axis * 3 + turn];
            at += wide * share * libm::sinf(a);
            going += wide * share * rate * times * own * libm::cosf(a);
        }
        let a = seconds * JINK_RATE * own + member.phases[axis * 3 + 2];
        (
            at + JINK_M * libm::sinf(a),
            going + JINK_M * JINK_RATE * own * libm::cosf(a),
        )
    });
    let (low, high) = flight.over_m;
    let a = seconds * 0.6 * member.rates[0];
    let level =
        0.5 + 0.3 * libm::sinf(a + member.phases[6]) + 0.2 * libm::sinf(a * 2.3 + member.phases[7]);
    let up = low + (high - low) * level + LIFT_M * lift;
    (
        Vec3::new(across.0, up, along.0),
        Vec2::new(across.1, along.1).normalize_or(Vec2::Y),
    )
}

/// How far through its bout a butterfly is at a time, 0 to 1: it beats through the first
/// part and glides the rest. The shader works out the same from the same clock and tag, so
/// the body rises as the wings beat.
pub fn bout(bout_s: (f32, f32), tag: u32, seconds: f32) -> f32 {
    let through = seconds / (bout_s.0 + bout_s.1).max(0.01) + (tag & 0xffff) as f32 / 65536.0;
    through - libm::floorf(through)
}

/// The share of a bout that is beating, the rest the glide.
pub fn beats(bout_s: (f32, f32)) -> f32 {
    bout_s.0 / (bout_s.0 + bout_s.1).max(0.01)
}

/// How high a bout has lifted a butterfly, 0 to 1: up through the beats, down through the
/// glide, eased at both ends.
pub fn lift(through: f32, beats: f32) -> f32 {
    let ease = |x: f32| x * x * (3.0 - 2.0 * x);
    if through < beats {
        ease(through / beats.max(1e-3))
    } else {
        1.0 - ease((through - beats) / (1.0 - beats).max(1e-3))
    }
}

/// A heading turned toward another by at most so many radians, the short way round.
pub fn turned(from: f32, toward: f32, most: f32) -> f32 {
    let mut by = (toward - from) % TAU;
    if by > TAU / 2.0 {
        by -= TAU;
    } else if by < -TAU / 2.0 {
        by += TAU;
    }
    from + by.clamp(-most, most)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec3Swizzles;

    const FLIGHT: Flight = Flight {
        spread_m: 1.5,
        roams_m: 3.0,
        speed: 1.6,
        over_m: (0.4, 1.6),
        bout_s: (0.6, 0.2),
        phase: (1.0, 2.0),
    };
    const WIDE: Flight = Flight {
        spread_m: 4.0,
        ..FLIGHT
    };

    /// A group is the same butterflies for the same seed, others for another, no two of it
    /// alike.
    #[test]
    fn a_groups_butterflies_are_its_seeds() {
        let group = members(7, 5);
        assert_eq!(group, members(7, 5));
        assert_ne!(group, members(8, 5));
        for (i, one) in group.iter().enumerate() {
            assert!(group[i + 1..].iter().all(|other| other != one));
        }
    }

    /// Every butterfly keeps within its group's spread of the middle and between its
    /// heights over the ground, the middle within its roam of where the group was put, and
    /// a group is spread out, not a ball.
    #[test]
    fn they_keep_within_the_spread_and_over_the_ground() {
        for flight in [FLIGHT, WIDE] {
            let group = members(3, 5);
            let mut widest = 0.0_f32;
            for step in 0..6000 {
                let t = step as f32 * 0.1;
                assert!(flight.middle(t).length() <= flight.roams_m);
                for one in &group {
                    for rise in [0.0, 1.0] {
                        let (at, _) = place(&flight, one, t, rise);
                        assert!(at.xz().length() <= flight.spread_m, "{at} at {t} s");
                        assert!(
                            at.y >= flight.over_m.0 && at.y <= flight.over_m.1 + LIFT_M,
                            "{} m up at {t} s",
                            at.y
                        );
                    }
                    widest = widest.max(place(&flight, one, t, 0.0).0.xz().length());
                }
            }
            assert!(widest > flight.spread_m * 0.6, "{widest} m at most");
        }
    }

    /// It flies at about its row's speed whatever its group's spread, heading the way it
    /// goes, and never straight: over any two seconds it turns, in all, more than a sixth
    /// of a turn (a straight flight would turn nothing; the straightest two seconds of
    /// these turn a quarter, and the measured cabbage white some four radians).
    #[test]
    fn it_flies_at_its_speed_the_way_it_heads_and_never_straight() {
        for flight in [FLIGHT, WIDE] {
            for one in members(11, 4) {
                let (dt, mut far, mut straightest) = (0.01, 0.0, f32::MAX);
                let mut turns = vec![0.0_f32; 12000];
                for (step, turn) in turns.iter_mut().enumerate() {
                    let t = step as f32 * dt;
                    let (here, _) = place(&flight, &one, t, 0.0);
                    let (there, _) = place(&flight, &one, t + dt, 0.0);
                    let moved = (there - here).xz();
                    far += moved.length();
                    let (_, heading) = place(&flight, &one, t + dt * 0.5, 0.0);
                    if moved.length() > 0.2 * dt {
                        assert!(moved.normalize().dot(heading) > 0.99, "at {t} s");
                    }
                    let (_, next) = place(&flight, &one, t + dt * 1.5, 0.0);
                    *turn = heading.angle_to(next).abs();
                }
                let speed = far / 120.0;
                assert!(
                    (speed / flight.speed - 1.0).abs() < 0.25,
                    "{speed} m/s, {} by its row",
                    flight.speed
                );
                for window in turns.windows(200).step_by(20) {
                    straightest = straightest.min(window.iter().sum());
                }
                assert!(
                    straightest > TAU / 6.0,
                    "{straightest} radians in two seconds"
                );
            }
        }
    }

    /// A bout lifts through its beats and lets down through its glide, with no jump, and
    /// two butterflies are not in step.
    #[test]
    fn a_bout_lifts_as_it_beats_and_sinks_as_it_glides() {
        let share = beats(FLIGHT.bout_s);
        assert!((share - 0.75).abs() < 1e-6);
        assert_eq!(lift(0.0, share), 0.0);
        assert!((lift(share, share) - 1.0).abs() < 1e-5);
        assert!(lift(0.999, share) < 0.01);
        for step in 0..999 {
            let (a, b) = (step as f32 / 1000.0, (step + 1) as f32 / 1000.0);
            let (low, high) = (lift(a, share), lift(b, share));
            assert!((high - low).abs() < 0.01, "a jump at {a}");
            assert_eq!(high > low, b <= share, "at {b}");
        }
        // One bout every 0.8 s, begun where the tag says.
        assert!((bout(FLIGHT.bout_s, 0, 0.4) - 0.5).abs() < 1e-5);
        assert!((bout(FLIGHT.bout_s, 0, 8.4) - 0.5).abs() < 1e-4);
        assert!((bout(FLIGHT.bout_s, 0x8000, 0.4) - 0.0).abs() < 1e-5);
        assert!((bout(FLIGHT.bout_s, 0xabcd_4000, 0.0) - 0.25).abs() < 1e-5);
    }

    /// A heading turns the short way round, no faster than it may.
    #[test]
    fn a_heading_turns_the_short_way_at_most_so_fast() {
        assert!((turned(0.0, 1.0, 0.25) - 0.25).abs() < 1e-6);
        assert!((turned(0.0, 0.1, 0.25) - 0.1).abs() < 1e-6);
        assert!((turned(3.0, -3.0, 0.5) - 3.2831855).abs() < 1e-5);
        assert!((turned(-3.0, 3.0, 0.1) + 3.1).abs() < 1e-5);
        assert!((turned(7.0, 7.0 + TAU * 3.0, 1.0) - 7.0).abs() < 1e-4);
    }
}
