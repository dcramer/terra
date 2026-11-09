//! Where a small world's trees stand, as a rule of the world alone: its planet, its start,
//! its seed, the plants' data, each kind's variants' sizes and the season. `plant` stands the
//! trees it gives; a world that draws nothing (a realm instance, D640) asks the same rule for
//! what its trees stop ([`stops`]), with the variants measured, not grown
//! (`grown::measure`), so both come to the same trees. On a world with mountains the woods
//! grow the trees by the land's rules (`crate::woods`) and this gives none.

use super::{Patch, Placing, scatter, wooded};
use crate::flora_kinds::{Flora, Grows, Season};
use crate::grown;
use crate::planet::Planet;
use bevy::prelude::*;
use terra_gen::{Noise, Rng, Seed};

/// How far from the start trees are scattered, and how far apart at their closest, metres.
const REACH_M: f32 = 340.0;
const SPACING_M: f32 = 6.5;

/// Each kind's variants' sizes, by the kind's place in the data: how tall each stands and how
/// far round its foot, metres.
pub type Sizes = Vec<Vec<(f32, f32)>>;

/// The sizes of the trees' variants as measured, with nothing grown: none for a kind that is
/// not a grown tree, which the trees' layer never chooses.
pub fn measured(flora: &Flora, seed: Seed) -> Sizes {
    (flora.kinds().iter())
        .map(|kind| {
            let Some(how) = &kind.grown else {
                return Vec::new();
            };
            let seed = seed.derive("grown").derive(&kind.name);
            let size = kind.size.unwrap_or((1.0, 1.0));
            (0..kind.variants)
                .filter_map(|one| grown::measure::measure(how, seed, (one, kind.variants), size))
                .collect()
        })
        .collect()
}

/// Standing on the ground at a point, turned at random, leaning at most a little.
pub(super) fn standing(rng: &mut Rng, point: Vec3, lean: f32, scale: f32, sunk: f32) -> Transform {
    let up = point.normalize();
    let lean_axis = up.any_orthonormal_vector();
    let rotation = Quat::from_axis_angle(up, rng.range(0.0, std::f32::consts::TAU))
        * Quat::from_axis_angle(lean_axis, rng.range(0.0, lean))
        * Quat::from_rotation_arc(Vec3::Y, up);
    Transform {
        translation: point - up * sunk,
        rotation,
        scale: Vec3::splat(scale),
    }
}

/// The trees of a world whose flora's seed is `seed`: woods with clearings, thinning to
/// nothing round the start and at the shore, each a kind's variant at its place and size.
pub(super) fn trees(
    (planet, start): (&Planet, &Transform),
    seed: Seed,
    flora: &Flora,
    sizes: &Sizes,
    season: &Season,
) -> Vec<Placing> {
    if planet.has_mountains() {
        return Vec::new();
    }
    let middle = start.translation;
    let up = middle.normalize();
    let east = up.cross(Vec3::X).normalize();
    let ground = Patch {
        middle,
        east,
        north: east.cross(up),
    };
    let to_base = start.compute_affine().inverse();
    let woods = Noise::new(seed.derive("woods"));
    let species = Noise::new(seed.derive("species"));
    scatter(
        seed.derive("trees"),
        REACH_M,
        SPACING_M,
        |rng, x, z, out| {
            let point = ground.ground(planet, x, z);
            let normal = planet.normal(point);
            let from_base = Vec2::new(x, z).length();
            if point.length() - planet.water_near_m(point) < 1.4
                || normal.dot(point.normalize()) < 0.86
            {
                return;
            }
            // Keep the view from the door to the water open.
            let from_door = to_base.transform_point3(point) - Vec3::new(7.5, 0.0, 0.0);
            if (0.0..120.0).contains(&from_door.z) && from_door.x.abs() < 10.0 + 0.3 * from_door.z {
                return;
            }
            if rng.unit() > wooded(&woods, point, from_base) {
                return;
            }
            let which = species.fbm(point / 150.0 + 9.0, 2, 2.0, 0.5) + rng.range(-0.12, 0.12);
            let Some(kind) = flora.choose(Grows::Trees, which, 0.0, 0.0, season, rng) else {
                return;
            };
            let (least, most) = flora.kinds()[kind].size.unwrap_or((1.0, 1.0));
            let tall = rng.range(least, most);
            if sizes[kind].is_empty() {
                return;
            }
            let model = rng.below(sizes[kind].len() as u32) as usize;
            let (height, spread) = sizes[kind][model];
            let scale = tall / height;
            let trunk = (point, (spread * scale).clamp(0.25, 0.6));
            out.push(Placing {
                kind,
                model,
                transform: standing(rng, point, 0.06, scale, 0.1),
                room: 5.0,
                // The tallest are great trees, which only an iron axe will fell (D96).
                cut: flora.kinds()[kind].cuttable(tall),
                stops: Some(trunk),
            });
        },
    )
}

/// What a world's trees stop: each one's foot and how far round it, for a world that draws
/// nothing. The same feet and widths as `plant` files for the trees it stands.
pub fn stops(world: (&Planet, &Transform), flora: &Flora, season: &Season) -> Vec<(Vec3, f32)> {
    let seed = world.0.seed().derive("flora");
    let sizes = measured(flora, seed);
    (trees(world, seed, flora, &sizes, season).iter())
        .filter_map(|tree| tree.stops)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::PlanetSpec;

    fn world(seed: u64) -> (Planet, Transform) {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(seed),
            radius_m: 2000.0,
        });
        let start = Transform::from_translation(planet.ground_at(Vec3::Y * 2100.0));
        (planet, start)
    }

    /// A world that draws nothing comes to its trees by the rule alone: a wood of them round
    /// the start, each on the ground with a trunk's width, none where a player begins, the
    /// same every time it is asked and another wood for another seed.
    #[test]
    fn a_world_that_draws_nothing_has_its_trees_stops() {
        let flora = Flora::read();
        let (planet, start) = world(3);
        let stood = stops((&planet, &start), &flora, &Season::Unknown);
        assert!(stood.len() > 200, "{} trees", stood.len());
        for (foot, radius) in &stood {
            assert!((0.25..=0.6).contains(radius), "a trunk {radius} m round");
            assert!(
                foot.distance(planet.ground_at(*foot)) < 0.01,
                "a tree off the ground"
            );
            assert!(
                foot.distance(start.translation) > 16.0,
                "a tree where a player begins"
            );
            assert!(foot.distance(start.translation) < 500.0);
        }
        assert_eq!(stood, stops((&planet, &start), &flora, &Season::Unknown));
        let (other, begun) = world(4);
        assert_ne!(stood, stops((&other, &begun), &flora, &Season::Unknown));
    }

    /// The trees' layer chooses only kinds whose variants are measured: a kind it could
    /// choose with none would stand nowhere in a world that draws nothing and somewhere in
    /// one that does.
    #[test]
    fn every_kind_the_trees_layer_grows_is_measured() {
        let flora = Flora::read();
        let sizes = measured(&flora, Seed(3));
        for (kind, about) in flora.kinds().iter().enumerate() {
            if about.grows == Grows::Trees {
                assert_eq!(sizes[kind].len(), about.variants as usize, "{}", about.name);
            }
        }
    }

    /// No tree dies back, so where the trees stand does not turn on the season: a world with
    /// no climate (a realm instance asks with `Season::Unknown`) has the trees of one with it.
    /// A tree kind given a `dies_back` would make the two differ: then the realm needs the
    /// season.
    #[test]
    fn no_tree_dies_back_so_the_season_does_not_move_the_trees() {
        for kind in Flora::read().kinds() {
            if kind.grows == Grows::Trees {
                assert!(!kind.dies_back(), "{} dies back", kind.name);
            }
        }
    }
}
