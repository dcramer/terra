//! How a school swims (docs/ambient-life.md, item 3): the school goes round a closed path
//! in the level of the water, a figure of eight, and each fish follows it a little behind
//! the first and a little to one side, so the school strings out, turns as a wave and
//! never leaves the water it was put in. Plain numbers, no Bevy's world.

use bevy::math::{Vec2, Vec3};
use std::f32::consts::TAU;
use terra_gen::hash::{mix3, unit3};

/// A school's path: a figure of eight so wide, swum at so many metres a second, begun so
/// far round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Path {
    pub radius_m: f32,
    pub speed: f32,
    pub phase: f32,
}

impl Path {
    fn round(&self, seconds: f32) -> f32 {
        // The eight is some 6.1 radii round, so this many radians a second goes its speed.
        seconds * self.speed / (self.radius_m * 6.1 / TAU) + self.phase
    }

    /// Where the path is at a time, from its middle: across and along, in metres.
    pub fn at(&self, seconds: f32) -> Vec2 {
        let a = self.round(seconds);
        Vec2::new(libm::cosf(a), 0.5 * libm::sinf(2.0 * a)) * self.radius_m
    }

    /// Which way it goes there.
    pub fn heading(&self, seconds: f32) -> Vec2 {
        let a = self.round(seconds);
        Vec2::new(-libm::sinf(a), libm::cosf(2.0 * a)).normalize_or(Vec2::X)
    }
}

/// One fish of a school: how far behind the first it follows, in seconds, how far to one
/// side and above or below the school's level it keeps, in metres, and where in its
/// stroke its tail is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    pub lag_s: f32,
    pub aside_m: f32,
    pub above_m: f32,
    pub stroke: u32,
}

/// A school's fish, the same for the same seed: strung out behind the first so each keeps
/// about `keeps_m` from the next, and spread as far to the sides and half as far up and
/// down.
pub fn members(seed: u32, count: usize, keeps_m: f32, speed: f32) -> Vec<Member> {
    let rows = libm::sqrtf(count as f32).ceil().max(1.0);
    (0..count as u32)
        .map(|fish| {
            let behind = (fish as f32 / rows).floor() + unit3(seed, fish, 0, 1) * 0.6;
            Member {
                lag_s: behind * keeps_m / speed.max(0.01),
                aside_m: (unit3(seed, fish, 0, 2) - 0.5) * rows * keeps_m,
                above_m: (unit3(seed, fish, 0, 3) - 0.5) * rows * keeps_m * 0.5,
                stroke: mix3(seed, fish, 0, 4) % 6283,
            }
        })
        .collect()
}

/// Where a fish is at a time, from the school's middle (across, up, along), and which way
/// it faces in the level.
pub fn place(path: &Path, member: &Member, seconds: f32) -> (Vec3, Vec2) {
    let then = seconds - member.lag_s;
    let (on, heading) = (path.at(then), path.heading(then));
    let aside = Vec2::new(heading.y, -heading.x) * member.aside_m;
    let at = on + aside;
    (Vec3::new(at.x, member.above_m, at.y), heading)
}

/// How far from its middle a school's fish get at most: what its water must be deep enough
/// under.
pub fn reach_m(path: &Path, count: usize, keeps_m: f32) -> f32 {
    path.radius_m + libm::sqrtf(count as f32).ceil() * keeps_m * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: Path = Path {
        radius_m: 3.0,
        speed: 0.5,
        phase: 1.0,
    };

    /// A school swims at its speed, the way it faces, and comes back to where it began.
    #[test]
    fn a_school_swims_its_path_at_its_speed_facing_where_it_goes() {
        let (mut far, mut least, mut most) = (0.0, f32::MAX, 0.0_f32);
        for step in 0..4000 {
            let t = step as f32 * 0.01;
            let moved = PATH.at(t + 0.01) - PATH.at(t);
            let speed = moved.length() / 0.01;
            least = least.min(speed);
            most = most.max(speed);
            far += moved.length();
            assert!(moved.normalize().dot(PATH.heading(t + 0.005)) > 0.99);
        }
        // Never still, never darting, and its speed on the whole (it slows in the turns).
        assert!(least > 0.2 && most < 0.8, "{least} to {most} m/s");
        assert!((far / 40.0 - 0.5).abs() < 0.1, "{} m/s", far / 40.0);
        let lap = 6.1 * 3.0 / 0.5;
        assert!(PATH.at(0.0).distance(PATH.at(lap)) < 0.2);
    }

    /// Every fish of a school keeps within its reach of the middle, and no two swim as one.
    #[test]
    fn its_fish_keep_within_reach_and_apart() {
        let fish = members(7, 24, 0.15, PATH.speed);
        assert_eq!(fish, members(7, 24, 0.15, PATH.speed));
        assert_ne!(fish, members(8, 24, 0.15, PATH.speed));
        let reach = reach_m(&PATH, 24, 0.15);
        for step in 0..600 {
            let t = step as f32 * 0.1;
            let places: Vec<Vec3> = fish.iter().map(|one| place(&PATH, one, t).0).collect();
            assert!(places.iter().all(|at| at.length() <= reach + 0.01));
            // Strung out, not a ball: the school is longer than a fish's spacing.
            let first = places[0];
            let spread = places
                .iter()
                .map(|at| at.distance(first))
                .fold(0.0, f32::max);
            assert!(spread > 0.15, "{spread} m across at {t} s");
        }
        for (i, one) in fish.iter().enumerate() {
            for other in &fish[i + 1..] {
                assert!(one != other);
            }
        }
    }
}
