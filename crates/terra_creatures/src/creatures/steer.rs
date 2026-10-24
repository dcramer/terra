//! What a creature on the ground steers round: the trunks the woods and the flora know
//! (`flora::Trunks`, the land's woods among them, which no walker collides with) and the
//! rocks, stumps and logs too tall to step onto (`flora::Obstacles`), D267.

use bevy::prelude::*;
use terra_core::Avoidance;
use terra_world::flora::{Obstacles, Trunks};

/// How a creature weighs its way round them: Detour's crowd defaults, looking a second and a
/// half ahead (docs/scene-rules.md: "for a second ahead"; a walk of 1.4 m/s sees 2 m).
pub const AVOIDS: Avoidance = Avoidance {
    horizon_s: 1.5,
    ..Avoidance::DETOUR
};

/// The rounds standing within `reach` of a point: each trunk's foot and radius, and each
/// tall piece of a solid.
pub fn rounds_near(
    trunks: &Trunks,
    obstacles: &Obstacles,
    at: Vec3,
    reach: f32,
) -> Vec<(Vec3, f32)> {
    trunks
        .near(at, reach, obstacles)
        .map(|trunk| (trunk.foot, trunk.radius_m))
        .chain(obstacles.near(at, reach))
        .collect()
}

/// A body of a radius at a point moved out of every round it is in, across the ground.
pub fn clear_of(rounds: Vec<(Vec3, f32)>, at: Vec3, radius: f32) -> Vec3 {
    let up = at.normalize_or(Vec3::Y);
    let mut at = at;
    for (middle, round) in rounds {
        let off = (at - middle).reject_from(up);
        let least = round + radius;
        let apart = off.length();
        if apart < least {
            at += off.normalize_or(up.any_orthonormal_vector()) * (least - apart);
        }
    }
    at
}

/// The way and speed a creature at a point takes round what stands near, facing one way and
/// wanting another at a speed: the velocity nearest it that keeps it clear (the lake film's
/// fox ran through a tree).
pub fn round(
    (trunks, obstacles): (&Trunks, &Obstacles),
    here: Vec3,
    radius: f32,
    (facing, want, speed): (Vec3, Vec3, f32),
) -> (Vec3, f32) {
    if speed <= 0.0 || want == Vec3::ZERO {
        return (want, speed);
    }
    let reach = speed * AVOIDS.horizon_s + radius + 1.0;
    let rounds = rounds_near(trunks, obstacles, here, reach);
    let v = AVOIDS.steer_on_ground(here, radius, (facing * speed, want * speed), rounds);
    (v.normalize_or_zero(), v.length())
}

/// Where a creature standing at a point is, pushed out of any round steering left it in: its
/// middle, its rump and its head with it, every point from its rump `rear_m` behind its
/// middle to its nose `nose_m` before it along `facing` kept its body's radius off any bark,
/// the whole body moved by what each needs, the head last. Steering plans for the middle
/// alone, so a long body coming at a trunk head on (a fox's snout, 0.4 m before its middle,
/// touched one in lake film r24) stops short by its head, as a capsule agent does where a
/// round one would not; and one turning away from a trunk keeps its hindquarters out of it
/// (the forest film's boar, its rump in a redwood, 10-23).
pub fn out_of(
    (trunks, obstacles): (&Trunks, &Obstacles),
    at: Vec3,
    (facing, radius): (Vec3, f32),
    (nose_m, rear_m): (f32, f32),
) -> Vec3 {
    let (ahead, behind) = ((nose_m - radius).max(0.0), (rear_m - radius).max(0.0));
    let rounds = rounds_near(trunks, obstacles, at, radius + ahead.max(behind) + 1.0);
    let mut at = clear_of(rounds.clone(), at, radius);
    let up = at.normalize_or(Vec3::Y);
    let facing = facing.reject_from(up).normalize_or_zero();
    if facing == Vec3::ZERO {
        return at;
    }
    // The haunch and the rump's tip, then the neck, the head and the nose's tip: no more
    // than a radius apart, so no trunk fits between two of them.
    for (reach, way) in [(behind, -facing), (ahead, facing)] {
        if reach <= 0.0 {
            continue;
        }
        let steps = (reach / radius).ceil().max(1.0) as usize;
        for step in 1..=steps {
            let point = at + way * (reach + radius) * step as f32 / steps as f32;
            at += clear_of(rounds.clone(), point, radius) - point;
        }
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_world::flora::Trunk;

    #[test]
    fn a_long_body_stops_short_of_a_trunk_by_its_head_and_a_round_one_by_its_middle() {
        let ground = Vec3::Y * 1000.0;
        let foot = ground + Vec3::X * 1.0;
        let bark = 0.3;
        let mut trunks = Trunks::default();
        trunks.add(Trunk {
            foot,
            radius_m: bark,
            height_m: 18.0,
            solid: false,
        });
        let obstacles = Obstacles::default();
        // A fox's body: 0.15 m round, its snout 0.4 m before its middle, coming at the trunk
        // head on, a little to one side and slant, from where its middle is clear and its
        // snout would be in the bark.
        let (radius, nose) = (0.15, 0.4);
        for (aside, slant) in [(0.0, 0.0), (0.1, 0.0), (-0.2, 0.3), (0.25, -0.4)] {
            let facing = (Vec3::X + Vec3::Z * slant).normalize();
            let at = ground + Vec3::X * (1.0 - bark - radius - 0.05) + Vec3::Z * aside;
            let to = out_of((&trunks, &obstacles), at, (facing, radius), (nose, radius));
            for step in 0..=20 {
                let point = to + facing * nose * step as f32 / 20.0;
                let apart = (point - foot).reject_from(Vec3::Y).length();
                assert!(
                    apart >= bark + radius - 1e-3,
                    "{aside} {slant}: {:.2} m along it, {apart:.2} m from the trunk's middle",
                    nose * step as f32 / 20.0
                );
            }
            // Round, it stands where it was.
            let round = out_of(
                (&trunks, &obstacles),
                at,
                (facing, radius),
                (radius, radius),
            );
            assert!(round.distance(at) < 1e-4, "{aside} {slant}");
        }
    }

    #[test]
    fn a_long_body_turned_away_from_a_trunk_keeps_its_rump_out_of_the_bark() {
        // The forest film's boar stood with its rump in a redwood (10-23): its middle clear,
        // facing away, its hindquarters 0.65 m behind it.
        let ground = Vec3::Y * 1000.0;
        let foot = ground + Vec3::X * -1.0;
        let bark = 0.9;
        let mut trunks = Trunks::default();
        trunks.add(Trunk {
            foot,
            radius_m: bark,
            height_m: 30.0,
            solid: false,
        });
        let obstacles = Obstacles::default();
        let (radius, nose, rear) = (0.35, 0.95, 0.65);
        for (aside, slant) in [(0.0, 0.0), (0.3, 0.2), (-0.4, -0.5), (0.2, 0.9)] {
            let facing = (Vec3::X + Vec3::Z * slant).normalize();
            let at = ground + Vec3::X * (bark + radius + 0.05 - 1.0) + Vec3::Z * aside;
            let to = out_of((&trunks, &obstacles), at, (facing, radius), (nose, rear));
            for step in 0..=20 {
                let along = -rear + (nose + rear) * step as f32 / 20.0;
                let apart = (to + facing * along - foot).reject_from(Vec3::Y).length();
                assert!(
                    apart >= bark + radius - 1e-3,
                    "{aside} {slant}: {along:.2} m along it, {apart:.2} m from the trunk's middle"
                );
            }
        }
    }
}
