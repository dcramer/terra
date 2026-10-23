//! Never put out before a camera's eyes (docs/scene-rules.md, Wildlife; the lake film's "animals
//! pop into existence"): every animal wildlife puts out goes through `put`, which refuses a
//! spot any camera sees now or will see while the animal fades in as it comes on, as a shipped
//! game's ambient population spawns outside the view frustum and beyond its cull distance (Red
//! Dead's ambient spawns, Unreal's MassEntity spawn-out-of-view). It is tested where the animal
//! shows, not where its feet are: a duck afloat on a lake, tested on the bed, was hidden by the
//! shore's slope and put out in plain view (lake film r21, 6.4 s).

use super::Surroundings;
use crate::creatures::{self, Kinds};
use crate::species::{Bestiary, Species};
use bevy::prelude::*;

/// How high over the ground or water an animal's middle is tested from, metres.
pub const BODY_UP_M: f32 = 0.4;
/// How far on a camera may come while one put out fades in, metres: about a second of a run,
/// the fade (`appear::FADE_S`) and its meshes coming in.
const COMING_ON_M: f32 = 6.0;
/// A group's member whose own spot is seen is moved out of sight within so far of it,
/// metres, tried in rings so far apart at so many bearings, nearest first (`unseen_near`).
const MOVED_UNSEEN_M: f32 = 6.0;
const MOVED_RING_M: f32 = 0.75;
const MOVED_BEARINGS: usize = 12;

/// Where something at a point on the ground shows: its middle, over the water's surface where
/// the ground is under water (a swimmer floats there; anything else is seen through it).
pub fn afloat(ground: Vec3, water_radius_m: f32) -> Vec3 {
    let up = ground.normalize_or(Vec3::Y);
    let base = if water_radius_m > ground.length() {
        up * water_radius_m
    } else {
        ground
    };
    base + up * BODY_UP_M
}

impl Surroundings<'_, '_> {
    /// Where an animal put out at a spot shows (`afloat`).
    pub fn shows_at(&self, spot: Vec3) -> Vec3 {
        let ground = self.planet.ground_at(spot);
        afloat(ground, self.planet.water_radius_m(ground))
    }

    /// Whether a camera sees a point now, or will once it has come a little on the way it
    /// looks (`COMING_ON_M`), within so far of it.
    pub fn seen_soon(&self, at: Vec3, within_m: f32) -> bool {
        self.eyes.iter().any(|(eye, projection)| {
            let up = eye.translation().normalize_or(Vec3::Y);
            let on = eye.forward().reject_from(up).normalize_or_zero() * COMING_ON_M;
            [Vec3::ZERO, on].into_iter().any(|by| {
                let moved = GlobalTransform::from(
                    eye.compute_transform()
                        .with_translation(eye.translation() + by),
                );
                self.seen_from(&moved, projection, at, within_m)
            })
        })
    }

    /// Whether a spot is out of every camera's sight for one of a kind drawn so far off, now
    /// and as the camera comes on: where an animal may be put out.
    pub fn unseen_for(&self, spot: Vec3, seen_m: f32) -> bool {
        !self.seen_soon(self.shows_at(spot), seen_m + 4.0)
    }

    /// The nearest spot to one on the ground that is out of sight for one of a kind drawn so
    /// far off (`unseen_for`), within `MOVED_UNSEEN_M`: itself if it is. None if all are seen.
    pub fn unseen_near(&self, spot: Vec3, seen_m: f32) -> Option<Vec3> {
        if self.unseen_for(spot, seen_m) {
            return Some(spot);
        }
        let up = spot.normalize_or(Vec3::Y);
        let start = up.any_orthonormal_vector();
        let rings = (MOVED_UNSEEN_M / MOVED_RING_M) as usize;
        (1..=rings).find_map(|ring| {
            (0..MOVED_BEARINGS)
                .map(|bearing| {
                    let turn = bearing as f32 / MOVED_BEARINGS as f32 * std::f32::consts::TAU;
                    let off =
                        Quat::from_axis_angle(up, turn) * start * (ring as f32 * MOVED_RING_M);
                    self.ground(spot + off)
                })
                .find(|&moved| self.unseen_for(moved, seen_m))
        })
    }
}

/// Puts out one of a kind at a spot, unless a camera would see it appear there (`unseen_for`).
pub fn put(
    commands: &mut Commands,
    (kinds, bestiary): (&Kinds, &Bestiary),
    around: &Surroundings,
    species: Species,
    at: Vec3,
) -> Option<Entity> {
    if !around.unseen_for(at, bestiary.get(species).seen_m()) {
        debug!(
            "Wildlife: a {} not put out in view.",
            bestiary.name(species)
        );
        return None;
    }
    creatures::spawn(commands, (kinds, bestiary), &around.planet, species, at)
}

/// Puts out one of a group at its spot, or, where a camera would see it appear there, at the
/// nearest spot near it that none would (`unseen_near`): a member whose spread fell in view
/// was left out, and the fox met alone, or the boar's sounder, went missing (forest walk,
/// 10-22).
pub fn put_near(
    commands: &mut Commands,
    (kinds, bestiary): (&Kinds, &Bestiary),
    around: &Surroundings,
    species: Species,
    at: Vec3,
) -> Option<Entity> {
    let Some(spot) = around.unseen_near(at, bestiary.get(species).seen_m()) else {
        debug!(
            "Wildlife: a {} not put out, in view all round.",
            bestiary.name(species)
        );
        return None;
    };
    creatures::spawn(commands, (kinds, bestiary), &around.planet, species, spot)
}

#[cfg(test)]
mod tests {
    use super::super::tests::surroundings_world;
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn a_spot_under_water_shows_on_the_surface_and_a_dry_one_on_the_ground() {
        let bed = Vec3::Y * 995.0;
        assert!((afloat(bed, 1000.0) - Vec3::Y * (1000.0 + BODY_UP_M)).length() < 1e-4);
        let dry = Vec3::Y * 1005.0;
        assert!((afloat(dry, 1000.0) - Vec3::Y * (1005.0 + BODY_UP_M)).length() < 1e-4);
    }

    #[test]
    fn no_animal_is_put_out_in_a_cameras_view_now_or_as_it_comes_on() {
        let (mut world, eye, up) = surroundings_world();
        let fov = PerspectiveProjection::default();
        let (half_tall, half_wide) = (
            libm::tanf(fov.fov * 0.5),
            libm::tanf(fov.fov * 0.5) * fov.aspect_ratio,
        );
        let ahead = Vec3::X;
        let side = ahead.cross(up);
        // Every spot round the camera out to 60 m, by a plain frustum test of its middle from
        // the eye now and 6 m on: one in either picture, inside the seen distance, is refused.
        let spots: Vec<Vec3> = (-30..=30)
            .flat_map(|a| (-30..=30).map(move |b| (a as f32 * 2.0, b as f32 * 2.0)))
            .map(|(along, aside)| eye + ahead * along + side * aside)
            .collect();
        let seen_m = 50.0;
        let accepted = {
            let spots = spots.clone();
            world
                .run_system_once(move |around: Surroundings| {
                    spots
                        .iter()
                        .map(|&spot| around.unseen_for(spot, seen_m))
                        .collect::<Vec<_>>()
                })
                .unwrap()
        };
        let in_picture = |from: Vec3, at: Vec3| {
            let off = at - from;
            let deep = off.dot(ahead);
            deep > 0.0
                && off.dot(side).abs() <= deep * half_wide
                && off.dot(up).abs() <= deep * half_tall
                && off.length() <= seen_m
        };
        let (mut refused, mut put) = (0, 0);
        for (spot, accepted) in spots.iter().zip(accepted) {
            let ground = spot.normalize() * (eye.length() - 2.0);
            let middle = ground + ground.normalize() * BODY_UP_M;
            let seen = in_picture(eye, middle) || in_picture(eye + ahead * COMING_ON_M, middle);
            if seen {
                assert!(!accepted, "put out in view at {:?}", spot - eye);
                refused += 1;
            } else if accepted {
                put += 1;
            }
        }
        // And it does not refuse everywhere: behind and well to the side it puts them out.
        assert!(refused > 100 && put > 1000, "{refused} refused, {put} put");
    }

    #[test]
    fn a_member_seen_where_it_would_stand_is_moved_near_out_of_sight_not_left_out() {
        let (mut world, eye, up) = surroundings_world();
        let side = Vec3::X.cross(up);
        let seen_m = 50.0;
        // Spots of a group's spread: two in the picture 9 to 11 m ahead, one already out of it.
        let spots = [
            eye + Vec3::X * 9.0 + side * 2.0,
            eye + Vec3::X * 11.0 - side * 1.0,
            eye - Vec3::X * 8.0,
        ];
        let check = world
            .run_system_once(move |around: Surroundings| {
                spots.map(|spot| {
                    let spot = around.ground(spot);
                    let moved = around
                        .unseen_near(spot, seen_m)
                        .expect("an unseen spot near the group");
                    let unseen = around.unseen_for(spot, seen_m);
                    (spot, unseen, moved, around.unseen_for(moved, seen_m))
                })
            })
            .unwrap();
        for (k, (spot, unseen, moved, moved_unseen)) in check.into_iter().enumerate() {
            assert!(moved_unseen, "{k}: moved to {:?}, still seen", moved - eye);
            if unseen {
                assert_eq!(moved, spot, "{k}: one out of sight stays where it is");
            } else {
                let by = moved.distance(spot);
                assert!(
                    by > 0.1 && by <= MOVED_UNSEEN_M + 0.5,
                    "{k}: moved {by:.2} m"
                );
            }
        }
        assert!(
            !check[0].1 && !check[1].1 && check[2].1,
            "two of the spread in view, one behind"
        );
    }
}
