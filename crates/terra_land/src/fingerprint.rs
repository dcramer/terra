//! A world's fingerprint (D596): a hash of each part of what its seed makes round its start,
//! so the same seed is seen to make the same world bit for bit, on every run and machine, and
//! two seeds to make two worlds in every part, none of it left on a seed of its own.
//!
//! The parts: the ground (its heights, and what lies under it), the woods' stands and
//! biomes (how close and tall trees stand at each place, which takes in the climate, the
//! season, lying snow, the lakes and what the ground is), the trees stood, and the weather
//! over days. Each is hashed from the numbers' `Debug` text, which tells every bit of a
//! float apart, so nothing is rounded away.

use crate::round::Round;
use crate::woods::{Woods, ground_normal};
use crate::{Land, PLANET_START, ground};
use glam::{Vec2, Vec3};
use terra_climate::{Climate, WorldTime};
use terra_core::{PlanetSpec, Seed};
use terra_gen::hash::splitmix64;

/// How far round the start the fingerprint looks, in metres.
pub const ACROSS_M: f32 = 600.0;
/// How many points a side of the grid it samples the ground and stands on.
const SIDE: usize = 24;

/// What a world's seed makes, each part as a hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    pub ground: u64,
    pub stands: u64,
    pub trees: u64,
    pub weather: u64,
}

impl Fingerprint {
    /// Each part's name and hash, for a message saying which differ.
    pub fn parts(&self) -> [(&'static str, u64); 4] {
        [
            ("ground", self.ground),
            ("stands", self.stands),
            ("trees", self.trees),
            ("weather", self.weather),
        ]
    }

    /// The whole, as one number.
    pub fn whole(&self) -> u64 {
        let mut sum = Sum::default();
        for (_, part) in self.parts() {
            sum.add(part);
        }
        sum.0
    }
}

/// A running hash: each number folded in by SplitMix64 (`terra_gen::hash`).
#[derive(Default)]
struct Sum(u64);

impl Sum {
    fn add(&mut self, x: u64) {
        self.0 = splitmix64(self.0.rotate_left(17) ^ x);
    }

    fn text(&mut self, text: &str) {
        for chunk in text.as_bytes().chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
        self.add(text.len() as u64);
    }
}

/// The fingerprint of the world a seed makes on a planet of that radius, in its climate.
pub fn of(seed: Seed, radius_m: f32, climate: Climate) -> Fingerprint {
    // On the snowbed date, so the round works out the snow lying once, not twice.
    let day = terra_climate::Date::parse(crate::round::SNOWBED_DATE)
        .expect("SNOWBED_DATE is a date")
        .day(&climate.year);
    let land = Land::new(ground::world(PlanetSpec { seed, radius_m }), climate);
    let points: Vec<Vec3> = (0..SIDE * SIDE)
        .map(|i| {
            let cell = Vec2::new((i % SIDE) as f32, (i / SIDE) as f32) / (SIDE - 1) as f32;
            land.surface(land.along(PLANET_START, (cell - 0.5) * ACROSS_M))
        })
        .collect();
    let mut ground = Sum::default();
    for &point in &points {
        ground.text(&format!("{point:?} {:?}", land.footing(point, None)));
    }
    let weather = weather(&land, &points);
    let round = Round::new(land, PLANET_START, ACROSS_M, day);
    let woods = Woods::of(&round);
    let mut stands = Sum::default();
    for &point in &points {
        let normal = ground_normal(&round.land, point);
        stands.text(&format!("{:?}", woods.stand_at(point, normal)));
    }
    let mut trees = Sum::default();
    let stood = woods.trees(PLANET_START, 0.0, ACROSS_M / 2.0, 8.0, seed.derive("woods"));
    for tree in &stood {
        trees.text(&format!("{tree:?}"));
    }
    trees.add(stood.len() as u64);
    Fingerprint {
        ground: ground.0,
        stands: stands.0,
        trees: trees.0,
        weather: weather.0,
    }
}

/// The weather at a few of the points over a few days, at dawn, noon and dusk.
fn weather(land: &Land, points: &[Vec3]) -> Sum {
    let mut sum = Sum::default();
    for &point in points.iter().step_by(37) {
        let place = land.place(point, point.normalize());
        for day in [0, 9, 40, 200] {
            for hour in [6.0, 12.0, 19.0] {
                let at = land.fields.at(place, WorldTime::at(day, hour));
                sum.text(&format!("{at:?}"));
            }
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The starting world's radius (D13), whose land has mountains, lakes and latitudes.
    const RADIUS_M: f32 = 30_000.0;

    fn fingerprint(seed: u64) -> Fingerprint {
        of(Seed(seed), RADIUS_M, crate::start_climate().unwrap())
    }

    #[test]
    fn the_same_seed_makes_the_same_world_bit_for_bit_and_another_seed_another_in_every_part() {
        // Made at once on other threads, as a server's and a player's machines would.
        let seeds = [1, 1, 0x5EED_CAFE];
        let made = terra_gen::par::map(&seeds, |&seed| fingerprint(seed));
        assert_eq!(made[0], made[1], "seed 1 made two worlds");
        for (i, j) in [(0, 2), (1, 2)] {
            for ((part, a), (_, b)) in made[i].parts().into_iter().zip(made[j].parts()) {
                assert_ne!(
                    a, b,
                    "seeds {} and {} made the same {part}: something in it is not seeded from the world (D596)",
                    seeds[i], seeds[j]
                );
            }
        }
    }
}
