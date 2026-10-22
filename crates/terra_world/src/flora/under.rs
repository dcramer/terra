//! The low growth under and beside the trees: bushes and leafy plants in the grass, and the
//! layers grown by the place (D136), rushes and reeds on a lake's wet margin and ferns and
//! shrubs on the shaded forest floor. Each kind of the place's layers says in
//! `assets/data/flora.ron` where it grows (`Site`), by the ground's wetness, slope, shade and
//! biome; this measures the ground (`Ground`) and asks.
use super::model::Model;
use super::{MOUNTAIN_SHORE_M, Patch, scatter, wooded};
use crate::flora_kinds::{Cuttable, Flora, Ground, Grows, Season};
use crate::planet::Planet;
use bevy::prelude::{Transform, Vec2, Vec3};
use terra_gen::{Noise, Rng, Seed, smoothstep};

/// A lake's margin is looked for this far from the start, every so many metres.
const SHORE_M: f32 = 300.0;
const SHORE_SPACING_M: f32 = 1.1;
/// The forest floor is grown this far from the start, every so many metres: as far as the
/// lake's shore is, since a mountain world's woods stand everywhere (the lake run walks 135 to
/// 265 m out).
const UNDERSTORY_M: f32 = 300.0;
const UNDERSTORY_SPACING_M: f32 = 2.2;

/// What a layer puts: a model, where, how much room it keeps, what cutting it gives, and
/// what it stops (`flora::plant`'s `put`).
pub(super) type Put<'a> =
    dyn FnMut(&Model, Transform, f32, Option<Cuttable>, Option<(Vec3, f32)>) + 'a;
/// Standing on the ground at a point, turned at random, leaning at most so much, so big and
/// sunk so far (`flora::plant`'s `standing`).
pub(super) type Standing<'a> = dyn Fn(&mut Rng, Vec3, f32, f32, f32) -> Transform + 'a;

/// What the low layers grow with.
pub(super) struct Sow<'a, 'p> {
    pub planet: &'a Planet,
    pub flora: &'a Flora,
    pub models: &'a [Vec<Model>],
    pub season: &'a Season,
    /// The woods' noise, which says where trees stand on a world without mountains.
    pub woods: &'a Noise,
    pub put: &'a mut Put<'p>,
    pub standing: &'a Standing<'a>,
}

impl Sow<'_, '_> {
    /// One of a kind, of its own size, leaning at most `lean`, sunk `sunk`, keeping `room`.
    fn grow(&mut self, kind: usize, rng: &mut Rng, point: Vec3, lean: f32, sunk: f32, room: f32) {
        let models = &self.models[kind];
        let model = &models[rng.below(models.len() as u32) as usize];
        let (least, most) = self.flora.kinds()[kind].size.unwrap_or((1.0, 1.0));
        let tall = rng.range(least, most);
        let scale = tall / model.height;
        let cut = self.flora.kinds()[kind].cuttable(tall);
        let transform = (self.standing)(rng, point, lean, scale, sunk);
        (self.put)(model, transform, room, cut, None);
    }

    /// Bushes and leafy plants, in the grass, round a patch's middle so far.
    pub fn in_the_grass(
        &mut self,
        patch: &Patch,
        seed: Seed,
        reach: f32,
        on_heath: &dyn Fn(Vec3) -> bool,
    ) {
        let (planet, flora, season) = (self.planet, self.flora, self.season);
        scatter(seed.derive("bushes"), 230.0 * reach, 5.5, |rng, x, z| {
            let point = patch.ground(planet, x, z);
            let cover = planet.cover(point, planet.normal(point));
            if rng.unit() > 0.3 * cover.grass {
                return;
            }
            // Some bear berries, the first food to be found.
            if let Some(kind) = flora.choose(Grows::Bushes, 0.0, cover.grass, 0.0, season, rng) {
                self.grow(kind, rng, point, 0.1, 0.05, 1.2);
            }
        });
        scatter(seed.derive("plants"), 150.0 * reach, 3.2, |rng, x, z| {
            let point = patch.ground(planet, x, z);
            let cover = planet.cover(point, planet.normal(point));
            if rng.unit() > 0.28 * cover.grass || on_heath(point) {
                return;
            }
            if let Some(kind) = flora.choose(Grows::Plants, 0.0, cover.grass, 0.0, season, rng) {
                self.grow(kind, rng, point, 0.15, 0.02, 0.6);
            }
        });
    }

    /// Rushes and reeds along a lake's or pool's wet margin, in beds with gaps between, a
    /// few tufts to a clump, as its kinds' sites say.
    pub fn shore(&mut self, patch: &Patch, seed: Seed) {
        let (planet, flora, season, woods) = (self.planet, self.flora, self.season, self.woods);
        let Some(round) = planet.round() else {
            return;
        };
        let beds = Noise::new(seed.derive("reed beds"));
        scatter(
            seed.derive("shore"),
            SHORE_M,
            SHORE_SPACING_M,
            |rng, x, z| {
                // Asked of the lakes' grid first, which is cheap, so only the shore is looked at.
                let across = patch.middle + patch.east * x + patch.north * z;
                if round.water_near(across.normalize(), 1).is_none() {
                    return;
                }
                let point = patch.ground(planet, x, z);
                if !(-0.4..0.7).contains(&(point.length() - planet.water_near_m(point))) {
                    return;
                }
                let bed = smoothstep(-0.15, 0.2, beds.fbm(point / 18.0, 2, 2.0, 0.5));
                if rng.unit() > 0.1 + 0.9 * bed {
                    return;
                }
                let ground = measure(planet, woods, point, Vec2::new(x, z).length());
                let Some(kind) = flora.choose_on(Grows::Shore, &ground, bed, season, rng) else {
                    return;
                };
                for _ in 0..1 + rng.below(3) {
                    let off =
                        patch.east * rng.range(-0.35, 0.35) + patch.north * rng.range(-0.35, 0.35);
                    let at = planet.ground_at(point + off);
                    self.grow(kind, rng, at, 0.12, 0.04, 0.4);
                }
            },
        );
    }

    /// Ferns and shrubs on the forest floor, thicker the closer the trees stand, in drifts,
    /// as its kinds' sites say; none on the open heath.
    pub fn understory(&mut self, patch: &Patch, seed: Seed, on_heath: &dyn Fn(Vec3) -> bool) {
        let (planet, flora, season, woods) = (self.planet, self.flora, self.season, self.woods);
        let drifts = Noise::new(seed.derive("fern drifts"));
        scatter(
            seed.derive("understory"),
            UNDERSTORY_M,
            UNDERSTORY_SPACING_M,
            |rng, x, z| {
                let point = patch.ground(planet, x, z);
                let drift = smoothstep(-0.1, 0.25, drifts.fbm(point / 16.0, 2, 2.0, 0.5));
                if rng.unit() > 0.2 + 0.8 * drift || on_heath(point) {
                    return;
                }
                let ground = measure(planet, woods, point, Vec2::new(x, z).length());
                if rng.unit() > ground.shade {
                    return;
                }
                if let Some(kind) = flora.choose_on(Grows::Understory, &ground, drift, season, rng)
                {
                    self.grow(kind, rng, point, 0.12, 0.03, 0.7);
                }
            },
        );
    }
}

/// Whether a point is inside the footprint of something standing that stops a walker (a
/// boulder's, as `flora::plant`'s `put` files them), so no rush or fern grows out of its top.
pub(super) fn in_a_stone(stops: &[(Vec3, f32)], at: Vec3) -> bool {
    stops
        .iter()
        .any(|&(middle, radius)| middle.distance(at) < radius)
}

/// What the place's layers know of a point on the ground so far from the start: how far over
/// the water, how steep, how shaded by the trees (the woods' stand on a world with mountains,
/// else the trees' own chance there, `wooded`), how far the open heath and how far its water a
/// mountain lake's.
fn measure(planet: &Planet, woods: &Noise, point: Vec3, from_start_m: f32) -> Ground {
    let normal = planet.normal(point);
    let up = point.normalize();
    let round = planet.round();
    let shade = match round {
        Some(round) if planet.has_mountains() => {
            round
                .stand(point, normal, terra_land::woods::Footing::Soil)
                .density
        }
        _ => wooded(woods, point, from_start_m),
    };
    Ground {
        over_water_m: point.length() - planet.water_near_m(point),
        slope_deg: libm::acosf(normal.dot(up).clamp(-1.0, 1.0)).to_degrees(),
        shade,
        heath: round.map_or(0.0, |round| round.heath.at(point).open),
        mountain: smoothstep(
            MOUNTAIN_SHORE_M.0,
            MOUNTAIN_SHORE_M.1,
            point.length() - planet.sea_radius_m(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the game's file grows on some ground, over many draws, by name.
    fn grown(layer: Grows, ground: Ground) -> Vec<String> {
        let flora = Flora::read();
        let mut rng = Seed(7).derive("under").rng();
        let mut names: Vec<String> = (0..400)
            .filter_map(|_| flora.choose_on(layer, &ground, 0.5, &Season::Unknown, &mut rng))
            .map(|kind| flora.kinds()[kind].name.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    const MARGIN: Ground = Ground {
        over_water_m: 0.1,
        slope_deg: 4.0,
        shade: 0.0,
        heath: 0.0,
        mountain: 0.0,
    };

    const FLOOR: Ground = Ground {
        over_water_m: 12.0,
        slope_deg: 8.0,
        shade: 0.8,
        heath: 0.0,
        mountain: 0.0,
    };

    #[test]
    fn a_wet_gentle_shore_grows_reeds_and_rushes_a_mountain_one_only_rushes() {
        let lowland = grown(Grows::Shore, MARGIN);
        assert!(
            lowland.len() >= 2,
            "a lowland margin grows only {lowland:?}"
        );
        let mountain = grown(
            Grows::Shore,
            Ground {
                mountain: 1.0,
                ..MARGIN
            },
        );
        assert!(
            !mountain.is_empty()
                && mountain.len() < lowland.len()
                && mountain.iter().all(|name| lowland.contains(name)),
            "a mountain shore grows {mountain:?}, a lowland one {lowland:?}"
        );
    }

    #[test]
    fn nothing_of_the_shore_grows_on_a_steep_bank_dry_ground_deep_water_or_the_heath() {
        for (what, ground) in [
            (
                "a steep bank",
                Ground {
                    slope_deg: 35.0,
                    ..MARGIN
                },
            ),
            (
                "dry ground",
                Ground {
                    over_water_m: 2.0,
                    ..MARGIN
                },
            ),
            (
                "deep water",
                Ground {
                    over_water_m: -1.0,
                    ..MARGIN
                },
            ),
            (
                "the heath",
                Ground {
                    heath: 0.9,
                    ..MARGIN
                },
            ),
        ] {
            let grows = grown(Grows::Shore, ground);
            assert!(grows.is_empty(), "{what} grows {grows:?}");
        }
    }

    #[test]
    fn a_shaded_forest_floor_grows_ferns_and_shrubs_and_the_open_ground_none() {
        let floor = grown(Grows::Understory, FLOOR);
        assert!(floor.len() >= 2, "a shaded floor grows only {floor:?}");
        for (what, ground) in [
            (
                "open ground",
                Ground {
                    shade: 0.1,
                    ..FLOOR
                },
            ),
            (
                "the shore",
                Ground {
                    over_water_m: 0.5,
                    ..FLOOR
                },
            ),
            (
                "a cliff",
                Ground {
                    slope_deg: 50.0,
                    ..FLOOR
                },
            ),
            (
                "the heath",
                Ground {
                    heath: 0.9,
                    ..FLOOR
                },
            ),
        ] {
            let grows = grown(Grows::Understory, ground);
            assert!(grows.is_empty(), "{what} grows {grows:?}");
        }
    }

    #[test]
    fn nothing_grows_out_of_a_boulder_but_beside_it_does() {
        // A shore boulder 2 m across standing at the waterline (lake round 31's review: a rush
        // out of a boulder's top), and a smaller stone off to one side.
        let boulder = (Vec3::new(10.0, 2000.0, 0.0), 1.0);
        let stones = [boulder, (Vec3::new(-6.0, 2000.0, 4.0), 0.5)];
        let up = Vec3::Y * 0.9;
        for on in [boulder.0, boulder.0 + up, boulder.0 + Vec3::X * 0.7] {
            assert!(
                in_a_stone(&stones, on),
                "a rush grows at {on} in the boulder"
            );
        }
        for beside in [
            boulder.0 + Vec3::X * 1.3,
            boulder.0 - Vec3::Z * 1.1,
            Vec3::Y * 2000.0,
        ] {
            assert!(!in_a_stone(&stones, beside), "nothing may grow at {beside}");
        }
    }
}
