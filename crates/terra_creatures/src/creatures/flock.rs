//! Birds rise together (D647, docs/ambient-life.md): when one takes wing, those of its kind
//! about it go with it, to come down beside where it does, so a party feeding on the ground
//! is seen as a flock crossing and not as one small bird now and then. A flight begun is
//! kept a moment, as each bird thinks in its own turn, and whoever of its kind is near where
//! it began, and not on the wing, follows.

use crate::species::Species;
use bevy::prelude::*;
use terra_gen::Rng;

/// A bird within this of where one of its kind took wing goes with it, metres.
const WITH_IT_M: f32 = 8.0;
/// So many of ten of those near follow.
const FOLLOWS: f32 = 0.85;
/// A flight begun is followed for so many of the system's runs: a quarter of a second.
const KEPT_RUNS: u8 = 16;
/// A follower comes down within this of where the first does, metres.
const BESIDE_M: f32 = 3.0;

/// The flights lately begun: of which kind, from where, to where, and how long each is kept.
#[derive(Default)]
pub struct Flock(Vec<(Species, Vec3, Vec3, u8)>);

impl Flock {
    /// One run of the system on: flights begun too long ago are let go.
    pub fn age(&mut self) {
        self.0.retain_mut(|(.., left)| {
            *left = left.saturating_sub(1);
            *left > 0
        });
    }

    /// A bird has taken wing.
    pub fn began(&mut self, species: Species, from: Vec3, to: Vec3) {
        self.0.push((species, from, to, KEPT_RUNS));
    }

    /// Where a bird at a place flies to with one of its kind that took wing near it, if it
    /// goes with it: beside where that one comes down.
    pub fn joins(&self, species: Species, here: Vec3, dice: &mut Rng) -> Option<Vec3> {
        let (_, _, to, _) = (self.0.iter())
            .find(|(kind, from, ..)| *kind == species && from.distance(here) < WITH_IT_M)?;
        if dice.unit() >= FOLLOWS {
            return None;
        }
        let up = to.normalize_or(Vec3::Y);
        let turn = Quat::from_axis_angle(up, dice.range(0.0, std::f32::consts::TAU));
        Some(*to + turn * up.any_orthonormal_vector() * dice.range(0.5, BESIDE_M))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_gen::Seed;

    /// Those near where one took wing follow, most of them, to beside where it comes down;
    /// one far off, or of another kind, does not; and a flight is forgotten after its moment.
    #[test]
    fn birds_near_one_that_took_wing_go_with_it_and_others_do_not() {
        let bestiary = crate::species::Bestiary::read(&terra_survival::inventory::Items::default());
        let kind = |name: &str| bestiary.named(name).expect("the data has it");
        let (finch, pigeon) = (kind("Chaffinch"), kind("Wood pigeon"));
        let ground = Vec3::Y * 2000.0;
        let to = ground + Vec3::X * 30.0;
        let mut flock = Flock::default();
        flock.began(finch, ground, to);
        let mut dice = Seed(7).rng();
        let near: Vec<Vec3> = (0..200)
            .filter_map(|_| flock.joins(finch, ground + Vec3::Z * 3.0, &mut dice))
            .collect();
        assert!(
            (150..190).contains(&near.len()),
            "{} of 200 followed",
            near.len()
        );
        assert!(near.iter().all(|at| at.distance(to) <= BESIDE_M + 0.01));
        assert_eq!(flock.joins(finch, ground + Vec3::Z * 20.0, &mut dice), None);
        assert_eq!(flock.joins(pigeon, ground + Vec3::Z * 3.0, &mut dice), None);
        for _ in 0..KEPT_RUNS {
            flock.age();
        }
        assert_eq!(flock.joins(finch, ground + Vec3::Z * 3.0, &mut dice), None);
    }
}
