//! Local obstacle avoidance: a body on its way steers round what stands in it, a trunk, a
//! rock, a post, choosing each tick the velocity nearest the one it wants that keeps it clear
//! for a while ahead (docs/scene-rules.md, Steering). Creatures and people share it, as the
//! mover's (docs/movement.md).
//!
//! Ported from Recast Navigation's `dtObstacleAvoidanceQuery::sampleVelocityAdaptive` and
//! `processSample` (DetourCrowd/Source/DetourObstacleAvoidance.cpp, zlib licence, Copyright
//! (c) 2009-2010 Mikko Mononen memon@inside.org; altered here, not the original source): the
//! same adaptive pattern of samples round the wanted velocity, refined five times, and the
//! same penalty of straying from the wanted and the current velocity, of passing on the
//! obstacle's near side and of the time to the nearest impact. Departure: what stands is
//! still, so its velocity obstacle is the plain one (`vcand`), not Detour's reciprocal
//! `2 vcand - vel`, which assumes the other agent turns half the way too.

use glam::{Vec2, Vec3};

/// How a body weighs its choices, Detour's crowd defaults (`dtCrowd::init`).
#[derive(Clone, Copy, Debug)]
pub struct Avoidance {
    /// How far ahead an impact counts, seconds.
    pub horizon_s: f32,
    pub vel_bias: f32,
    pub weight_desired: f32,
    pub weight_current: f32,
    pub weight_side: f32,
    pub weight_impact: f32,
    /// The sampling pattern: directions to a ring, rings, and refinements.
    pub divs: usize,
    pub rings: usize,
    pub depth: usize,
}

impl Avoidance {
    pub const DETOUR: Avoidance = Avoidance {
        horizon_s: 2.5,
        vel_bias: 0.4,
        weight_desired: 2.0,
        weight_current: 0.75,
        weight_side: 0.75,
        weight_impact: 2.5,
        divs: 7,
        rings: 2,
        depth: 5,
    };
}

impl Default for Avoidance {
    fn default() -> Self {
        Self::DETOUR
    }
}

/// Something round standing still, across the ground from the body: its middle relative to
/// the body's, and how far round it reaches.
#[derive(Clone, Copy, Debug)]
pub struct Round {
    pub at: Vec2,
    pub radius: f32,
}

/// When a body of a radius at the origin, going at `v`, first touches a round, and when it
/// leaves it: Detour's `sweepCircleCircle`. None if it never does or is not moving.
fn sweep(radius: f32, v: Vec2, round: &Round) -> Option<(f32, f32)> {
    let s = round.at;
    let r = radius + round.radius;
    let c = s.dot(s) - r * r;
    let a = v.dot(v);
    if a < 1e-4 {
        return None;
    }
    let b = v.dot(s);
    let d = b * b - a * c;
    if d < 0.0 {
        return None;
    }
    let rd = d.sqrt();
    Some(((b - rd) / a, (b + rd) / a))
}

impl Avoidance {
    /// How bad a candidate velocity is: Detour's `processSample` for still obstacles.
    fn penalty(
        &self,
        candidate: Vec2,
        radius: f32,
        now: Vec2,
        want: Vec2,
        vmax: f32,
        rounds: &[Round],
    ) -> f32 {
        let inv_vmax = 1.0 / vmax.max(1e-3);
        let desired = self.weight_desired * candidate.distance(want) * inv_vmax;
        let current = self.weight_current * candidate.distance(now) * inv_vmax;
        let mut nearest = self.horizon_s;
        let mut side = 0.0;
        for round in rounds {
            // Passing on its near side, of the way it is from the body, costs a little.
            let toward = round.at.normalize_or_zero();
            let across = Vec2::new(-toward.y, toward.x);
            side += (toward.dot(candidate) * 0.5 + 0.5)
                .min(across.dot(candidate) * 2.0)
                .clamp(0.0, 1.0);
            let Some((mut first, last)) = sweep(radius, candidate, round) else {
                continue;
            };
            // Already in it: the more so the deeper.
            if first < 0.0 && last > 0.0 {
                first = -first * 0.5;
            }
            if first >= 0.0 && first < nearest {
                nearest = first;
            }
        }
        if !rounds.is_empty() {
            side /= rounds.len() as f32;
        }
        desired
            + current
            + self.weight_side * side
            + self.weight_impact / (0.1 + nearest / self.horizon_s)
    }

    /// The velocity a body of a radius takes, going at `now` and wanting `want`, at most
    /// `vmax`, among rounds placed relative to it: Detour's `sampleVelocityAdaptive`.
    pub fn steer(&self, radius: f32, now: Vec2, want: Vec2, vmax: f32, rounds: &[Round]) -> Vec2 {
        if rounds.is_empty() || vmax <= 0.0 {
            return want;
        }
        // The pattern, aligned to the way wanted: the middle, then rings of directions, each
        // ring turned half a step from the one outside it.
        let step = std::f32::consts::TAU / self.divs as f32;
        let ahead = want.normalize_or(Vec2::X);
        let mut pattern = vec![Vec2::ZERO];
        for ring in 0..self.rings {
            let r = (self.rings - ring) as f32 / self.rings as f32;
            let turn = if ring % 2 == 1 { step * 0.5 } else { 0.0 };
            for div in 0..self.divs {
                pattern.push(Vec2::from_angle(turn + step * div as f32).rotate(ahead) * r);
            }
        }
        let mut reach = vmax * (1.0 - self.vel_bias);
        let mut best = want * self.vel_bias;
        for _ in 0..self.depth {
            let (mut least, mut chosen) = (f32::MAX, best);
            for &offset in &pattern {
                let candidate = best + offset * reach;
                if candidate.length_squared() > (vmax + 0.001) * (vmax + 0.001) {
                    continue;
                }
                let penalty = self.penalty(candidate, radius, now, want, vmax, rounds);
                if penalty < least {
                    (least, chosen) = (penalty, candidate);
                }
            }
            best = chosen;
            reach *= 0.5;
        }
        best
    }

    /// The same on a planet's ground: a body at `here` (its up the way out from the middle)
    /// going at `now` and wanting `want`, among still rounds (each a point and a radius),
    /// those further than it could reach within the horizon left out. Velocities across the
    /// ground, metres a second.
    pub fn steer_on_ground(
        &self,
        here: Vec3,
        radius: f32,
        (now, want): (Vec3, Vec3),
        rounds: impl IntoIterator<Item = (Vec3, f32)>,
    ) -> Vec3 {
        let up = here.normalize_or(Vec3::Y);
        let east = up.any_orthonormal_vector();
        let north = up.cross(east);
        let flat = |v: Vec3| Vec2::new(v.dot(east), v.dot(north));
        let vmax = want.length();
        let reach = vmax * self.horizon_s + radius;
        let near: Vec<Round> = rounds
            .into_iter()
            .map(|(at, r)| Round {
                at: flat(at - here),
                radius: r,
            })
            .filter(|round| round.at.length() - round.radius < reach)
            .collect();
        let v = self.steer(radius, flat(now), flat(want), vmax, &near);
        east * v.x + north * v.y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Walks a body from a point toward a goal among rounds, steering each step, and gives
    /// its path.
    fn walk(from: Vec2, to: Vec2, radius: f32, speed: f32, rounds: &[Round]) -> Vec<Vec2> {
        let avoid = Avoidance::default();
        let dt = 1.0 / 30.0;
        let (mut at, mut v) = (from, Vec2::ZERO);
        let mut path = vec![at];
        for _ in 0..3000 {
            if at.distance(to) < 0.3 {
                break;
            }
            let want = (to - at).normalize_or_zero() * speed;
            let placed: Vec<Round> = rounds
                .iter()
                .map(|r| Round {
                    at: r.at - at,
                    radius: r.radius,
                })
                .collect();
            v = avoid.steer(radius, v, want, speed, &placed);
            at += v * dt;
            path.push(at);
        }
        path
    }

    #[test]
    fn a_body_going_straight_at_a_trunk_goes_round_it_and_gets_there() {
        let trunk = Round {
            at: Vec2::new(5.0, 0.0),
            radius: 0.4,
        };
        let radius = 0.3;
        for speed in [1.4, 4.0, 9.0] {
            // Dead on, and a hair off either side.
            for aim in [0.0, 0.05, -0.05] {
                let to = Vec2::new(10.0, aim);
                let path = walk(Vec2::ZERO, to, radius, speed, &[trunk]);
                let closest = path
                    .iter()
                    .map(|p| p.distance(trunk.at))
                    .fold(f32::MAX, f32::min);
                assert!(
                    closest >= trunk.radius + radius * 0.5,
                    "at {speed} m/s came within {closest:.2} m of the trunk's middle"
                );
                assert!(
                    path.last().unwrap().distance(to) < 0.3,
                    "never got there at {speed} m/s"
                );
            }
        }
    }

    #[test]
    fn a_body_through_a_stand_of_trunks_never_enters_one() {
        // A grid of trunks 3 m apart, crossed on a diagonal and straight down a row.
        let mut rounds = Vec::new();
        for i in 0..5 {
            for j in -2..3 {
                rounds.push(Round {
                    at: Vec2::new(3.0 + i as f32 * 3.0, j as f32 * 3.0),
                    radius: 0.35,
                });
            }
        }
        let radius = 0.3;
        for to in [
            Vec2::new(18.0, 0.0),
            Vec2::new(18.0, 5.0),
            Vec2::new(18.0, -1.5),
        ] {
            let path = walk(Vec2::new(0.0, 0.0), to, radius, 3.0, &rounds);
            for p in &path {
                for r in &rounds {
                    assert!(
                        p.distance(r.at) >= r.radius + radius * 0.5,
                        "into a trunk at {p:?} going to {to:?}"
                    );
                }
            }
            assert!(
                path.last().unwrap().distance(to) < 0.3,
                "never got to {to:?}"
            );
        }
    }

    #[test]
    fn nothing_in_the_way_leaves_the_wanted_velocity_alone() {
        let want = Vec2::new(1.0, 2.0);
        let far = Round {
            at: Vec2::new(-20.0, 0.0),
            radius: 0.5,
        };
        let got = Avoidance::default().steer(0.3, want, want, want.length(), &[far]);
        assert!(got.distance(want) < 0.1 * want.length(), "{got:?}");
        assert_eq!(
            Avoidance::default().steer(0.3, want, want, want.length(), &[]),
            want
        );
    }
}
