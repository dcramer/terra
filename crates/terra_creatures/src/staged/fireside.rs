//! A staged fire is laid clear of the legs of whoever rests by it, as a person sat at a
//! campfire keeps their feet out of it: wherever a figure at rest has its legs (its own
//! skeleton's, so a troll's reach forward as far as its shins and toes do), a fire set down
//! by the script that has any part of them over it is moved straight away from them, the
//! least that clears them all. The fire gives way, not the sitter, who is on a seat of its
//! own. A troll's toes in its campfire was the lake film's round 23, its shins over the
//! stones round 24.
//!
//! How near is the larger of two: the fire's whole footprint (its model's bounds as drawn,
//! the stone ring at whatever scale it was set down) and its heat's, no nearer than the
//! sun's kilowatt a square metre radiated on the legs, a flux a body bears for as long as it
//! sits (NIST TN 1797, the 1 kW/m² of tenability tables; pain comes at 2.5), the fire a point
//! source at its middle, measured from its flames' edge. Either is kept from the leg's skin,
//! not its bones: the shin from knee to ankle, the foot to its ball and the toes to their
//! tip, each as thick as a share of the shin is long.

use super::Staged;
use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use terra_survival::props::{CAMPFIRE, FIRE_HEAT_W};
use terra_walker::character;
use terra_world::planet::Planet;
use terra_world::{fire::Kindled, footprint::Footprint};

/// The flux the legs bear as long as they sit, watts a square metre.
const BORNE_W_M2: f32 = 1_000.0;
/// Each leg's joints from the knee down to the toes' tip (Unreal's names, `character::HIPS`;
/// `ball_leaf` is the end of the toes): the shin, the foot and the toes between them.
const LEGS: [[&str; 4]; 2] = [
    ["calf_l", "foot_l", "ball_l", "ball_leaf_l"],
    ["calf_r", "foot_r", "ball_r", "ball_leaf_r"],
];
/// A leg's half thickness as a share of its shin's length: a man's calf about 6 cm across
/// a 43 cm shin, a little more for the ankle bones and the boot.
const LIMB_OF_SHIN: f32 = 0.18;
/// Points measured along each bone between two joints.
const ALONG: usize = 8;
/// Moved, a fire goes this much past the least that clears the legs, so a foot shifting in
/// an idle loop does not move it again.
const SPARE_M: f32 = 0.1;
/// Moving slower than this, metres a second, a figure is at rest.
const RESTING_MPS: f32 = 0.05;

/// How far from a fire's middle its heat keeps a leg, metres: its flames' half width (a
/// flame with no width of its own is half as wide as it is tall, `fire::light`) and the
/// distance its radiated heat falls to what a body bears, a campfire's heat radiating
/// `FIRE_HEAT_W`.
pub(super) fn clearance_m(fire: &Kindled) -> f32 {
    let wide = if fire.wide > 0.0 {
        fire.wide
    } else {
        fire.tall * 0.5
    };
    let watts = FIRE_HEAT_W * fire.heat.max(0.0) / CAMPFIRE.heat;
    0.5 * wide + libm::sqrtf(watts / (4.0 * std::f32::consts::PI * BORNE_W_M2))
}

/// How far a fire's model reaches from its middle across the level, metres: the furthest of
/// its parts' bounds along the fire's own two level axes, so a round ring of stones inside
/// its square bounds counts as its radius, not the bounds' corner. `parts` are each mesh's
/// bounds and where it is drawn; `at` is the fire's own transform, its scale with it.
pub(super) fn footprint_m(at: &GlobalTransform, parts: &[(Aabb, GlobalTransform)]) -> f32 {
    let middle = at.translation();
    let (right, ahead) = (at.right().as_vec3(), at.forward().as_vec3());
    let mut reach = 0.0f32;
    for (bounds, drawn) in parts {
        let (c, h) = (Vec3::from(bounds.center), Vec3::from(bounds.half_extents));
        for corner in 0..8 {
            let sign = Vec3::new(
                if corner & 1 == 0 { -1.0 } else { 1.0 },
                if corner & 2 == 0 { -1.0 } else { 1.0 },
                if corner & 4 == 0 { -1.0 } else { 1.0 },
            );
            let d = drawn.transform_point(c + h * sign) - middle;
            reach = reach.max(d.dot(right).abs()).max(d.dot(ahead).abs());
        }
    }
    reach
}

/// The points a leg is measured from, and how thick it is about them: along each bone of
/// `joints` (knee, ankle, ball, toes' tip, as many as the skeleton has), [`ALONG`] points
/// a bone, with the half thickness a share of the first bone's (the shin's) length.
pub(super) fn leg_points(joints: &[Vec3]) -> (Vec<Vec3>, f32) {
    let Some(shin) = joints.windows(2).next().map(|w| w[0].distance(w[1])) else {
        return (joints.to_vec(), 0.0);
    };
    let mut points = vec![joints[0]];
    for bone in joints.windows(2) {
        for i in 1..=ALONG {
            points.push(bone[0].lerp(bone[1], i as f32 / ALONG as f32));
        }
    }
    (points, shin * LIMB_OF_SHIN)
}
/// Where a fire at `fire` goes so that every point of the legs, `feet`, is at least `clear`
/// from it across `up`'s level: straight along `away` (from the sitter toward the fire, so
/// never back under its seat, even when the fire is under its knees; from the legs' middle
/// if `away` is level with nothing), the least that clears them all and [`SPARE_M`] more;
/// `None` when they are clear already.
pub(super) fn laid_clear(
    fire: Vec3,
    feet: &[Vec3],
    clear: f32,
    up: Vec3,
    away: Vec3,
) -> Option<Vec3> {
    let level = |v: Vec3| v.reject_from(up);
    if feet
        .iter()
        .all(|foot| level(fire - *foot).length() >= clear)
    {
        return None;
    }
    let middle = feet.iter().copied().sum::<Vec3>() / feet.len() as f32;
    let way = level(away)
        .try_normalize()
        .or_else(|| level(fire - middle).try_normalize())?;
    // The least `t` with |fire + way t - foot| >= clear + spare for each foot, across the level.
    let reach = clear + SPARE_M;
    let t = feet
        .iter()
        .map(|foot| {
            let d = level(fire - *foot);
            let b = d.dot(way);
            let c = d.length_squared() - reach * reach;
            (-b + libm::sqrtf((b * b - c).max(0.0))).max(0.0)
        })
        .fold(0.0, f32::max);
    Some(fire + way * t)
}

/// Each staged fire near a staged figure at rest moved clear of its legs, kept on the ground,
/// the ground it clears cleared again where it now is. A fire whose model has not been drawn
/// yet waits for it, so it is moved once, by its whole footprint.
#[allow(clippy::too_many_arguments)]
pub(super) fn keep_clear(
    mut commands: Commands,
    planet: Res<Planet>,
    figures: Query<(Entity, &Staged, &GlobalTransform), Without<Kindled>>,
    mut fires: Query<(Entity, &Kindled, &mut Transform), With<Staged>>,
    joints: Query<&GlobalTransform>,
    drawn: Query<(&Aabb, &GlobalTransform)>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    if fires.is_empty() {
        return;
    }
    for (figure, staged, body) in &figures {
        if staged.going.is_some() || staged.speed_now > RESTING_MPS {
            continue;
        }
        let mut legs = Vec::new();
        let mut thick = 0.0f32;
        for leg in LEGS {
            let at: Vec<Vec3> = leg
                .iter()
                .filter_map(|bone| character::find(figure, bone, &children, &names))
                .filter_map(|bone| joints.get(bone).ok().map(GlobalTransform::translation))
                .collect();
            let (points, half) = leg_points(&at);
            legs.extend(points);
            thick = thick.max(half);
        }
        if legs.is_empty() {
            continue;
        }
        for (fire, kindled, mut at) in &mut fires {
            let parts: Vec<(Aabb, GlobalTransform)> = children
                .iter_descendants(fire)
                .filter_map(|part| drawn.get(part).ok())
                .map(|(bounds, place)| (*bounds, *place))
                .collect();
            if parts.is_empty() {
                continue;
            }
            let ring = footprint_m(&GlobalTransform::from(*at), &parts);
            let clear = clearance_m(kindled).max(ring) + thick;
            if at.translation.distance(body.translation()) > clear + 10.0 {
                continue;
            }
            let up = at.translation.normalize_or(Vec3::Y);
            let away = at.translation - body.translation();
            let Some(to) = laid_clear(at.translation, &legs, clear, up, away) else {
                continue;
            };
            let moved = to.distance(at.translation);
            at.translation = planet.ground_at(to);
            // Kindled again, so the ground it clears is cleared where it is (`fire::scorch`).
            commands
                .entity(fire)
                .remove::<(Kindled, Footprint)>()
                .insert(*kindled);
            info!(
                "Staged: a fire moved {moved:.2} m clear of {}'s legs (its footprint {ring:.2} m, \
                 kept {clear:.2} m from its middle).",
                staged.name
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRES: [Kindled; 4] = [
        CAMPFIRE,
        Kindled {
            tall: 1.95,
            wide: 0.9,
            heat: 0.9,
            logs: terra_world::fire::Logs::CAMPFIRE,
        },
        Kindled {
            tall: 0.6,
            wide: 0.0,
            heat: 0.3,
            logs: terra_world::fire::Logs::CAMPFIRE,
        },
        Kindled {
            tall: 3.0,
            wide: 2.5,
            heat: 2.0,
            logs: terra_world::fire::Logs::CAMPFIRE,
        },
    ];

    #[test]
    fn a_hotter_or_wider_fire_is_kept_further_from_the_feet_and_none_less_than_its_flames() {
        let man = clearance_m(&CAMPFIRE);
        // A man's campfire: his feet a little under a metre from its middle, half a metre
        // clear of its flames, as people sit at one.
        assert!((0.8..1.2).contains(&man), "{man}");
        for fire in FIRES {
            let clear = clearance_m(&fire);
            let flames = 0.5
                * if fire.wide > 0.0 {
                    fire.wide
                } else {
                    fire.tall * 0.5
                };
            assert!(clear > flames + 0.2, "{fire:?}: {clear} by flames {flames}");
            let hotter = Kindled {
                heat: fire.heat * 2.0,
                ..fire
            };
            let wider = Kindled {
                wide: fire.wide + 1.0,
                ..fire
            };
            assert!(clearance_m(&hotter) > clear && clearance_m(&wider) > clear);
        }
    }

    #[test]
    fn a_fire_at_a_sitters_feet_goes_clear_of_every_foot_by_the_least_and_a_clear_one_stays() {
        let up = Vec3::Y;
        // Feet of a small sitter and of a troll sat with its legs out, each before its hips.
        let small = [Vec3::new(0.12, 0.0, 0.4), Vec3::new(-0.12, 0.0, 0.45)];
        let troll = [
            Vec3::new(0.35, 0.1, 1.1),
            Vec3::new(-0.3, 0.1, 0.9),
            Vec3::new(0.3, 0.2, 0.8),
            Vec3::new(-0.25, 0.2, 0.7),
        ];
        for feet in [&small[..], &troll[..]] {
            for fire in FIRES {
                let clear = clearance_m(&fire);
                // Among its feet, a little before them, and to one side.
                for at in [
                    Vec3::new(0.0, 0.0, 0.8),
                    Vec3::new(0.3, 0.0, 1.4),
                    Vec3::new(-0.9, 0.0, 0.6),
                ] {
                    let to = laid_clear(at, feet, clear, up, Vec3::Z).unwrap_or(at);
                    let nearest = feet
                        .iter()
                        .map(|f| (to - *f).reject_from(up).length())
                        .fold(f32::MAX, f32::min);
                    assert!(
                        nearest >= clear - 1e-3,
                        "{fire:?} from {at} to {to}: a foot {nearest} off, {clear} kept"
                    );
                    // No further than clear of the nearest foot and the spare, past it.
                    assert!(
                        nearest <= clear + SPARE_M + 1e-3 || to == at,
                        "{at} to {to}: {nearest}"
                    );
                    // Moved away from the feet, never toward the sitter behind them.
                    assert!(
                        to.z >= at.z - 1e-3 || to.x.abs() > at.x.abs(),
                        "{at} to {to}"
                    );
                }
                // One well clear stays where it was laid.
                let far = Vec3::new(0.0, 0.0, 1.2 + clear + 0.5);
                assert_eq!(laid_clear(far, feet, clear, up, Vec3::Z), None);
            }
        }
    }

    /// A campfire's stone ring as its model's bounds hold it: 0.6 m across each way from the
    /// middle and 12 cm tall (`Campfire.glb`'s widest part).
    fn ring() -> Aabb {
        Aabb::from_min_max(Vec3::new(-0.6, 0.0, -0.6), Vec3::new(0.6, 0.12, 0.6))
    }

    #[test]
    fn a_fires_footprint_is_its_rings_radius_at_its_scale_however_it_is_turned() {
        for (scale, turn) in [(1.0, 0.0), (1.3, 0.7), (2.0, -2.0)] {
            let at = GlobalTransform::from(
                Transform::from_xyz(5.0, 0.0, -3.0)
                    .with_rotation(Quat::from_rotation_y(turn))
                    .with_scale(Vec3::splat(scale)),
            );
            // The mesh drawn where the model's root is, as a glTF's one node is.
            let reach = footprint_m(&at, &[(ring(), at)]);
            assert!(
                (reach - 0.6 * scale).abs() < 1e-3,
                "{scale} {turn}: {reach}"
            );
        }
    }

    /// Every point of a leg, its bones walked finely from knee to toes' tip, and how thick.
    fn skin(joints: &[Vec3]) -> (Vec<Vec3>, f32) {
        let mut points = Vec::new();
        for bone in joints.windows(2) {
            for i in 0..=100 {
                points.push(bone[0].lerp(bone[1], i as f32 / 100.0));
            }
        }
        (points, joints[0].distance(joints[1]) * LIMB_OF_SHIN)
    }

    #[test]
    fn a_fire_under_a_seated_trolls_shins_goes_clear_of_its_ring_and_heat_from_knee_to_toes() {
        let up = Vec3::Y;
        let troll_fire = FIRES[1];
        // A troll sat on the ground facing +z, knee, ankle, ball and toes' tip of each leg,
        // in two poses: its legs out before it (the feet past the knees) and its shins
        // leaning forward over its feet (the knees past the toes), as the lake film's sat.
        let out = |x: f32| {
            [
                Vec3::new(x, 0.75, 0.9),
                Vec3::new(x * 1.1, 0.15, 1.5),
                Vec3::new(x * 1.1, 0.04, 1.72),
                Vec3::new(x * 1.1, 0.02, 1.86),
            ]
        };
        let leaning = |x: f32| {
            [
                Vec3::new(x, 0.95, 1.55),
                Vec3::new(x * 0.9, 0.15, 1.15),
                Vec3::new(x * 0.9, 0.04, 1.37),
                Vec3::new(x * 0.9, 0.02, 1.5),
            ]
        };
        for pose in [[out(0.3), out(-0.3)], [leaning(0.3), leaning(-0.35)]] {
            let mut legs = Vec::new();
            let mut thick = 0.0f32;
            for leg in &pose {
                let (points, half) = leg_points(leg);
                legs.extend(points);
                thick = thick.max(half);
            }
            for scale in [1.0, 1.3] {
                let ring_m = 0.6 * scale;
                let clear = clearance_m(&troll_fire).max(ring_m) + thick;
                // Under the knees, under the toes, between the feet and off to one side.
                for at in [
                    Vec3::new(0.2, 0.0, 1.5),
                    Vec3::new(0.0, 0.0, 1.9),
                    Vec3::new(0.0, 0.0, 1.2),
                    Vec3::new(-0.7, 0.0, 1.4),
                ] {
                    let to = laid_clear(at, &legs, clear, up, at).unwrap_or(at);
                    for leg in &pose {
                        let (skin, half) = skin(leg);
                        let nearest = skin
                            .iter()
                            .map(|p| (to - *p).reject_from(up).length() - half)
                            .fold(f32::MAX, f32::min);
                        // No stone of the ring and no flame under any of the leg's skin, and
                        // its heat no more than a body bears, within a centimetre of the
                        // points measured.
                        assert!(
                            nearest >= ring_m - 0.01 && nearest >= clearance_m(&troll_fire) - 0.01,
                            "{at} to {to} at scale {scale}: a leg {nearest} off, the ring {ring_m}"
                        );
                    }
                    // Away from the troll, never back under its seat.
                    assert!(to.z >= at.z - 1e-3, "{at} to {to}");
                }
            }
        }
    }
}
