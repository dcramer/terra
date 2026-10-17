//! What trees a place holds, by the land's rules (D313, D321): how close they stand and how
//! tall they grow, from the warmth of its growing season, the steepness of its ground and what
//! lies on it. So the forest gives out with height where the season grows too cold, as the
//! tree line the world over does (Paulsen and Körner, `terra_climate::Season`), higher on warm
//! slopes and lower on shaded ones and in cold hollows; never a painted contour.
//!
//! Real tree lines are a band, not a line (Körner, *Alpine Treelines*, 2012): the closed forest
//! (the timberline) stops about a degree warmer than the last trees three metres tall (the tree
//! line), and above those, stunted and wind-shaped krummholz creeps to about half a degree
//! colder (the tree species line). The numbers below say so.

use terra_climate::{Season, TREE_LINE_DAYS, TREE_LINE_SEASON_C};

/// How much warmer than the tree line's the growing season is where the forest closes, and how
/// much colder where the last krummholz creeps, in degrees Celsius.
pub const CLOSED_FOREST_C: f32 = 1.0;
pub const KRUMMHOLZ_C: f32 = 0.6;
/// Ground steeper than this, in degrees, holds no trees, and the steeper half of the way up to
/// it fewer: roots hold on ledges, not walls.
pub const TOO_STEEP_DEG: f32 = 45.0;
pub const THINS_FROM_DEG: f32 = 32.0;
/// How far over a lake's surface a tree's foot stands at the least, in metres.
pub const DRY_FOOT_M: f32 = 0.4;
/// A lake's shore is bare of trees up to this height over its water, and wooded fully from
/// that, in metres: the band its spring high water, waves and ice push keep scoured, where
/// shipped and real shores alike show stones, gravel and sedge between the water and the
/// forest's edge (docs/demos.md §1, shores). On a gentle shore it is metres wide, on a steep
/// one a step.
pub const SHORE_BARE_M: f32 = 1.0;
pub const SHORE_WOODED_M: f32 = 2.5;

/// What a place's ground is, for what grows on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Footing {
    /// Soil, where roots hold.
    Soil,
    /// Gravel in a gully's bed, or scree: a few trees on it, mostly at its edges.
    Loose,
    /// Bare rock: none.
    Rock,
    /// Under water: none.
    Water,
}

/// What trees a place holds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stand {
    /// How close they stand, as a share of a closed forest's: 0 none, 1 closed.
    pub density: f32,
    /// How tall they grow, as a share of their kind's full height: 1 in the forest, falling
    /// toward the tree line, a fifth as krummholz.
    pub stature: f32,
}

impl Stand {
    /// Whether the trees are krummholz: stunted below the height a tree line's last trees reach.
    pub fn krummholz(&self) -> bool {
        self.density > 0.0 && self.stature < 0.3
    }
}

/// The trees a place holds, from its growing season, how steep its ground is and what it is.
pub fn stand(season: &Season, slope_deg: f32, footing: Footing) -> Stand {
    let over = season.growing_mean_c - TREE_LINE_SEASON_C;
    let long_enough = smoothstep(
        TREE_LINE_DAYS * 0.8,
        TREE_LINE_DAYS * 1.1,
        season.growing_days,
    );
    let warm = smoothstep(-KRUMMHOLZ_C, CLOSED_FOREST_C, over) * long_enough;
    let held = 1.0 - smoothstep(THINS_FROM_DEG, TOO_STEEP_DEG, slope_deg);
    let ground = match footing {
        Footing::Soil => 1.0,
        Footing::Loose => 0.15,
        Footing::Rock | Footing::Water => 0.0,
    };
    let density = warm * held * ground;
    if density <= 0.0 {
        return Stand::default();
    }
    Stand {
        density,
        stature: 0.2 + 0.8 * smoothstep(-KRUMMHOLZ_C * 0.5, CLOSED_FOREST_C * 1.5, over),
    }
}

/// How much warmer than its season a place is for a tree, in degrees Celsius: its microsite.
/// At the forest's limit trees hold on where the ground shelters them, in the lee where winter's
/// snow covers their young and the wind does not dry and scour them, and in islands where the
/// soil and the stand's own shelter favour them, so the forest-tundra is a band of tree islands
/// thinning onto the open heath (*Waldtundra*), never a line along a contour (Holtmeier,
/// *Mountain Timberlines*, 2009). Inside the forest it changes nothing that shows.
#[derive(Clone, Debug)]
pub struct Microsites(terra_gen::Noise);

/// How much the lee warms a tree's place, and an exposed rise chills it, at most; and how much
/// warmer or colder an island's ground is, at most, its islands some hundreds of metres across.
const SHELTER_C: f32 = 0.5;
const ISLANDS_C: f32 = 0.9;
const ISLANDS_M: f32 = 220.0;

impl Microsites {
    pub fn new(seed: terra_gen::Seed) -> Self {
        Self(terra_gen::Noise::new(seed.derive("tree islands")))
    }

    /// A place's microsite, at a point in the world, so sheltered (Winstral's Sx, degrees).
    pub fn at(&self, point: glam::Vec3, shelter_deg: f32) -> f32 {
        // The noise's spread is about 0.27 a sample.
        let islands = (self.0.fbm(point / ISLANDS_M, 3, 2.2, 0.55) / 0.27).clamp(-2.0, 2.0) * 0.5;
        SHELTER_C * libm::tanhf(shelter_deg / 8.0) + ISLANDS_C * islands
    }
}

/// The trees a place holds, its season warmed or chilled for them by its microsite.
pub fn stand_in(season: &Season, slope_deg: f32, footing: Footing, microsite_c: f32) -> Stand {
    let season = Season {
        growing_mean_c: season.growing_mean_c + microsite_c,
        ..*season
    };
    stand(&season, slope_deg, footing)
}

/// The growing season worked out ahead over the latitudes, heights and leanings a region
/// spans, for asking about many places quickly: a season takes a year of days to work out,
/// and a forest's trees are tens of thousands of places. Between its points it blends; the
/// season changes smoothly with all three.
#[derive(Clone, Debug)]
pub struct Seasons {
    latitude: (f32, f32),
    height_m: (f32, f32),
    sunward: (f32, f32),
    steps: (usize, usize, usize),
    /// The growing season's mean and length and the year's mean, by latitude, then height,
    /// then leaning.
    table: Vec<[f32; 3]>,
}

impl Seasons {
    /// The seasons over latitudes, heights over the sea and leanings toward the equator,
    /// so finely: a tenth of a degree of latitude and 25 m of height do not change a season
    /// by much.
    pub fn over(
        fields: &terra_climate::Fields,
        latitude: (f32, f32),
        height_m: (f32, f32),
    ) -> Self {
        Self::over_by(fields, latitude, height_m, 0.1, 25.0, 12)
    }

    /// The seasons over latitudes and heights in steps of so many degrees and metres, and so
    /// many leanings: coarser for a search over a whole world, finer for the woods round a
    /// place.
    pub fn over_by(
        fields: &terra_climate::Fields,
        latitude: (f32, f32),
        height_m: (f32, f32),
        latitude_step_deg: f32,
        height_step_m: f32,
        leanings: usize,
    ) -> Self {
        let sunward = (-0.9, 0.9);
        let steps = (
            (((latitude.1 - latitude.0).to_degrees() / latitude_step_deg).ceil() as usize).max(1),
            (((height_m.1 - height_m.0) / height_step_m).ceil() as usize).max(1),
            leanings.max(1),
        );
        let points: Vec<(usize, usize, usize)> = (0..=steps.0)
            .flat_map(|i| (0..=steps.1).flat_map(move |j| (0..=steps.2).map(move |k| (i, j, k))))
            .collect();
        let at = |range: (f32, f32), step: usize, of: usize| {
            range.0 + (range.1 - range.0) * step as f32 / of as f32
        };
        let table = crate::maps::par_map(&points, |&(i, j, k)| {
            let season = fields.season(terra_climate::Place {
                height_m: at(height_m, j, steps.1),
                latitude: at(latitude, i, steps.0),
                sunward: at(sunward, k, steps.2),
                ..terra_climate::Place::level(glam::Vec3::ZERO, 0.0, 0.0)
            });
            [
                season.growing_mean_c,
                season.growing_days,
                season.year_mean_c,
            ]
        });
        Self {
            latitude,
            height_m,
            sunward,
            steps,
            table,
        }
    }

    /// The growing season at a place, blended from the table's points round it: its mean and
    /// its length, which is all a stand asks of it, and the year's mean, which permafrost
    /// follows; the coldest and warmest days are given as the year's mean.
    pub fn at(&self, latitude: f32, height_m: f32, sunward: f32) -> Season {
        let index = |v: f32, range: (f32, f32), of: usize| {
            let t = ((v - range.0) / (range.1 - range.0)).clamp(0.0, 1.0) * of as f32;
            let i = (t as usize).min(of.saturating_sub(1));
            (i, t - i as f32)
        };
        let (i, fi) = index(latitude, self.latitude, self.steps.0);
        let (j, fj) = index(height_m, self.height_m, self.steps.1);
        let (k, fk) = index(sunward, self.sunward, self.steps.2);
        let get = |a: usize, b: usize, c: usize| {
            let a = a.min(self.steps.0);
            let b = b.min(self.steps.1);
            let c = c.min(self.steps.2);
            self.table[(a * (self.steps.1 + 1) + b) * (self.steps.2 + 1) + c]
        };
        let lerp = |x: [f32; 3], y: [f32; 3], t: f32| [0, 1, 2].map(|n| x[n] + (y[n] - x[n]) * t);
        let along_k = |a, b| lerp(get(a, b, k), get(a, b, k + 1), fk);
        let along_j = |a| lerp(along_k(a, j), along_k(a, j + 1), fj);
        let [mean, days, year] = lerp(along_j(i), along_j(i + 1), fi);
        Season {
            growing_days: days,
            growing_mean_c: mean,
            year_mean_c: year,
            coldest_c: year,
            warmest_c: year,
        }
    }
}

/// Which kind a tree is (D313, docs/demos.md open question 4): the lush forest's trees by
/// their niches. Norway spruce holds the shaded slopes and the cooler, damper ground, as it
/// makes the Tatras' forest; Scots pine the sunny, drier slopes; and past the closed forest,
/// both stunted to krummholz, the dwarf mountain pine's place, which is not yet a kind of
/// its own. Down in the lowlands, oak among them, on the warmer, sunnier ground, as the
/// Carpathians' foothill forests mix it with pine below the spruce belt (D381).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TreeKind {
    Spruce,
    Pine,
    Oak,
}

/// A tree the land's rules stand somewhere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tree {
    /// Where its foot is, on the ground as generated.
    pub at: glam::Vec3,
    pub kind: TreeKind,
    /// Its stature, as a share of its kind's full height where it stands, and a number of
    /// its own, 0 to 1.
    pub stature: f32,
    pub luck: f32,
    /// How grown it is for its stand, as a share of a mature tree's height: a sapling's
    /// fifth to a veteran's over the rest. Unlike its stature it does not stunt it.
    pub age: f32,
}

/// Oak's share of the lowland woods, and the heights over the sea it thins out over, in
/// metres: a mixed foothill forest below, conifers alone toward the mountains.
const OAK_SHARE: f32 = 0.3;
const OAK_UP_TO_M: (f32, f32) = (650.0, 1_000.0);

/// How wide a stand's cohorts are, each an age of trees, in metres; how many trees are
/// saplings and poles come up under or between the rest; and how far the ages spread.
const COHORT_M: f32 = 45.0;
const SAPLINGS: f32 = 0.14;
const AGES: (f32, f32) = (0.55, 1.12);
/// How wide the glades a stand opens are, in metres, and how thin it stands in them.
const GLADES_M: f32 = 70.0;
const GLADE_DENSITY: f32 = 0.35;

/// How likely a tree is an oak, at a height over the sea, its ground so sunward.
pub fn oak_chance(height_m: f32, sunward: f32) -> f32 {
    OAK_SHARE
        * (1.0 - smoothstep(OAK_UP_TO_M.0, OAK_UP_TO_M.1, height_m))
        * (0.5 + smoothstep(-0.1, 0.3, sunward))
}

/// A world's woods: the land round a place, its rules worked out ahead.
pub struct Woods<'a> {
    pub round: &'a crate::round::Round,
}

impl<'a> Woods<'a> {
    pub fn of(round: &'a crate::round::Round) -> Self {
        Self { round }
    }

    /// What trees stand at a point on the ground, facing `normal`.
    pub fn stand_at(&self, point: glam::Vec3, normal: glam::Vec3) -> Stand {
        let up = point.normalize_or(glam::Vec3::Y);
        // A lake is found cell by cell, on a grid metres wide, so ground in a dry cell at its
        // shore can still lie under its surface: the trees there stood in a metre of water. So
        // the lake is asked of the cells round the point too, and a tree's foot stands clear of
        // its water by a little, as a shore's trees stand back from the edge.
        let near = self.round.water_near(up, 2);
        let water = match near {
            Some(lake) => Some(lake + DRY_FOOT_M),
            None => self.round.water_level(up),
        };
        let footing = self.round.land.footing(point, water);
        let mut stand = self.round.stand(point, normal, footing);
        if let Some(lake) = near {
            stand.density *= smoothstep(SHORE_BARE_M, SHORE_WOODED_M, point.length() - lake);
        }
        stand
    }

    /// The trees standing within `radius_m` of a direction, between `inner_m` and it, on a
    /// grid of cells `spacing_m` wide, each holding a tree by its stand's density, jittered
    /// within it; the same every time for a seed.
    pub fn trees(
        &self,
        middle: glam::Vec3,
        inner_m: f32,
        radius_m: f32,
        spacing_m: f32,
        seed: terra_gen::Seed,
    ) -> Vec<Tree> {
        let land = &self.round.land;
        let middle = middle.normalize();
        // A stand is not one age (D381): it is a mosaic of cohorts tens of metres across, each
        // grown up together after a storm or a felling took the trees before, with glades
        // where it stands thin, saplings and poles coming up among the rest; so its canopy
        // is ragged and its edge uneven, not a wall of trees of one height. Laid on the
        // world, so the same tree is the same wherever it is asked from.
        let cohorts = terra_gen::Noise::new(seed.derive("cohorts"));
        let glades = terra_gen::Noise::new(seed.derive("glades"));
        let (east, north) = (land.frame.east(middle), land.frame.north(middle));
        let cells = (radius_m / spacing_m).ceil() as i32;
        let rows: Vec<i32> = (-cells..cells).collect();
        let found = crate::maps::par_map(&rows, |&row| {
            let mut out = Vec::new();
            for column in -cells..cells {
                let mut rng = seed
                    .index(((row as i64) << 32 | (column as u32 as i64)) as u64)
                    .rng();
                let at = glam::Vec2::new(
                    (column as f32 + rng.unit()) * spacing_m,
                    (row as f32 + rng.unit()) * spacing_m,
                );
                let distance = at.length();
                if distance > radius_m || distance < inner_m {
                    continue;
                }
                let toward = (middle * land.radius_m() + east * at.x + north * at.y).normalize();
                let point = land.surface(toward);
                let normal = ground_normal(land, point);
                let stand = self.stand_at(point, normal);
                // The noise's spread is about 0.27 a sample.
                let spread = |noise: &terra_gen::Noise, wide: f32| {
                    (noise.fbm(point / wide, 2, 2.1, 0.5) / 0.27).clamp(-1.5, 1.5) / 3.0 + 0.5
                };
                let glade = GLADE_DENSITY
                    + (1.0 - GLADE_DENSITY) * smoothstep(0.2, 0.45, spread(&glades, GLADES_M));
                if stand.density <= 0.0 || rng.unit() > stand.density * glade {
                    continue;
                }
                let place = land.place(point, normal);
                // Spruce in the shade and the cool, pine where the sun bakes the slope, the
                // two mixing over a band of facing.
                let sunny = smoothstep(0.05, 0.3, place.sunward) + (rng.unit() - 0.5) * 0.5;
                let kind = if sunny > 0.5 {
                    TreeKind::Pine
                } else {
                    TreeKind::Spruce
                };
                let stature = (stand.stature * (0.85 + 0.3 * rng.unit())).min(1.0);
                let luck = rng.unit();
                // Oak only where the stand grows tall, never toward the tree line.
                let kind = if stand.stature >= 0.8
                    && rng.unit() < oak_chance(place.height_m, place.sunward)
                {
                    TreeKind::Oak
                } else {
                    kind
                };
                let cohort = spread(&cohorts, COHORT_M);
                let age = if rng.unit() < SAPLINGS {
                    0.18 + 0.3 * rng.unit()
                } else {
                    (AGES.0 + (AGES.1 - AGES.0) * cohort + (rng.unit() - 0.5) * 0.18).min(AGES.1)
                };
                out.push(Tree {
                    at: point,
                    kind,
                    stature,
                    luck,
                    age,
                });
            }
            out
        });
        found.into_iter().flatten().collect()
    }
}

/// Which way the ground as generated faces at a point on it, from its heights a metre round.
pub fn ground_normal(land: &crate::Land, point: glam::Vec3) -> glam::Vec3 {
    let up = point.normalize();
    let (a, b) = (
        up.any_orthonormal_vector(),
        up.any_orthonormal_vector().cross(up),
    );
    let h = |v: glam::Vec3| land.surface(point + v * 1.5);
    let (pa, pb) = (h(a) - h(-a), h(b) - h(-b));
    pa.cross(pb).normalize_or(up)
        * if pa.cross(pb).dot(up) < 0.0 {
            -1.0
        } else {
            1.0
        }
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn season(growing_mean_c: f32) -> Season {
        Season {
            growing_days: 150.0,
            growing_mean_c,
            year_mean_c: growing_mean_c - 6.0,
            coldest_c: -10.0,
            warmest_c: growing_mean_c + 5.0,
        }
    }

    #[test]
    fn a_colder_place_never_grows_taller_or_closer_trees() {
        let mut last = stand(&season(14.0), 10.0, Footing::Soil);
        assert_eq!(last.density, 1.0);
        for k in 1..200 {
            let here = stand(&season(14.0 - k as f32 * 0.05), 10.0, Footing::Soil);
            assert!(
                here.density <= last.density
                    && (here.stature <= last.stature || here.density == 0.0)
            );
            last = here;
        }
        // Past the krummholz, nothing.
        assert_eq!(
            stand(
                &season(TREE_LINE_SEASON_C - KRUMMHOLZ_C - 0.01),
                10.0,
                Footing::Soil
            ),
            Stand::default()
        );
    }

    #[test]
    fn the_forest_closes_a_degree_warmer_than_the_tree_line_and_krummholz_creeps_above_it() {
        let closed = stand(
            &season(TREE_LINE_SEASON_C + CLOSED_FOREST_C + 0.1),
            10.0,
            Footing::Soil,
        );
        assert!(closed.density > 0.99 && !closed.krummholz());
        let above = stand(&season(TREE_LINE_SEASON_C - 0.4), 10.0, Footing::Soil);
        assert!(above.density > 0.0 && above.krummholz(), "{above:?}");
    }

    #[test] // Slow: run before a push, not a commit (SLOW_TESTS in xtask).
    fn the_seasons_worked_out_ahead_are_the_seasons() {
        let climate = crate::start_climate().unwrap();
        let fields = terra_climate::Fields::new(climate, terra_gen::Seed(1));
        let (low, high) = (35f32.to_radians(), 45f32.to_radians());
        let seasons = Seasons::over(&fields, (low, high), (0.0, 3000.0));
        let mut rng = terra_gen::Seed(3).rng();
        for _ in 0..40 {
            let (latitude, height, sunward) = (
                rng.range(low, high),
                rng.range(0.0, 3000.0),
                rng.range(-0.6, 0.6),
            );
            let exact = fields.season(terra_climate::Place {
                at: glam::Vec3::ZERO,
                height_m: height,
                latitude,
                sunward,
                catch: 1.0,
            });
            let blended = seasons.at(latitude, height, sunward);
            // Where it barely grows the season's mean jumps to the year's; trees need months.
            if exact.growing_days < 60.0 {
                continue;
            }
            assert!(
                (exact.growing_mean_c - blended.growing_mean_c).abs() < 0.15,
                "{exact:?} {blended:?}"
            );
            assert!(
                (exact.growing_days - blended.growing_days).abs() < 6.0,
                "{exact:?} {blended:?}"
            );
        }
    }

    #[test]
    fn cliffs_rock_and_water_hold_no_trees_and_scree_few() {
        let warm = season(12.0);
        assert_eq!(
            stand(&warm, TOO_STEEP_DEG + 1.0, Footing::Soil).density,
            0.0
        );
        assert_eq!(stand(&warm, 10.0, Footing::Rock).density, 0.0);
        assert_eq!(stand(&warm, 10.0, Footing::Water).density, 0.0);
        let scree = stand(&warm, 30.0, Footing::Loose).density;
        assert!(scree > 0.0 && scree < 0.3);
    }

    #[test] // Slow: run before a push, not a commit (SLOW_TESTS in xtask).
    fn round_a_mountain_lake_spruce_holds_the_shade_and_pine_the_sun() {
        use crate::site::MountainLake;
        let climate = crate::start_climate().unwrap();
        let day = terra_climate::Date::parse("06-10")
            .unwrap()
            .day(&climate.year);
        let land = crate::Land::new(
            crate::ground::world(terra_core::PlanetSpec {
                seed: terra_core::Seed(1),
                radius_m: 30_000.0,
            }),
            climate,
        );
        let site = land
            .mountain_lake(crate::PLANET_START, &MountainLake::demo_pinned(30_000.0))
            .unwrap();
        let round = crate::round::Round::new(land, site.toward, 1_400.0, day);
        let trees = Woods::of(&round).trees(site.toward, 0.0, 600.0, 6.5, terra_gen::Seed(5));
        // How sunward the ground each kind stands on faces, on average.
        let facing = |kind: TreeKind| {
            let sunward: Vec<f32> = trees
                .iter()
                .filter(|tree| tree.kind == kind)
                .map(|tree| {
                    let normal = ground_normal(&round.land, tree.at);
                    round.land.place(tree.at, normal).sunward
                })
                .collect();
            (
                sunward.len(),
                sunward.iter().sum::<f32>() / sunward.len().max(1) as f32,
            )
        };
        let ((pines, pine_sun), (spruces, spruce_sun)) =
            (facing(TreeKind::Pine), facing(TreeKind::Spruce));
        // Both kinds make the lake's woods, neither a scatter in the other's.
        let all = (pines + spruces) as f32;
        assert!(all > 3_000.0, "{all} trees");
        assert!(
            pines as f32 > all * 0.15 && spruces as f32 > all * 0.15,
            "{pines} pines, {spruces} spruces"
        );
        assert!(
            pine_sun > spruce_sun + 0.15,
            "pines on ground {pine_sun:.2} sunward, spruces {spruce_sun:.2}"
        );
        // At 1,317 m, no oak.
        assert!(trees.iter().all(|tree| tree.kind != TreeKind::Oak));
        // The stand is of many ages, saplings to veterans, each age in patches: neighbours
        // are nearer an age than trees far apart.
        let share =
            |keep: &dyn Fn(f32) -> bool| trees.iter().filter(|t| keep(t.age)).count() as f32 / all;
        let (young, old) = (share(&|a| a < 0.5), share(&|a| a > 1.0));
        assert!(young > 0.08 && old > 0.05, "{young} young, {old} old");
        let (mut near, mut far) = ((0.0, 0), (0.0, 0));
        for (i, a) in trees.iter().enumerate().step_by(7) {
            for b in trees.iter().skip(i + 1).step_by(5) {
                let (apart, differ) = (a.at.distance(b.at), (a.age - b.age).abs());
                if a.age > 0.5 && b.age > 0.5 {
                    if apart < 15.0 {
                        near = (near.0 + differ, near.1 + 1);
                    } else if apart > 300.0 {
                        far = (far.0 + differ, far.1 + 1);
                    }
                }
            }
        }
        let (near, far) = (near.0 / near.1 as f32, far.0 / far.1 as f32);
        assert!(
            near < far * 0.7,
            "neighbours {near:.3} apart in age, far trees {far:.3}"
        );
    }

    #[test]
    fn oak_grows_in_the_lowlands_on_the_sunny_side_and_not_toward_the_mountains() {
        assert!(oak_chance(400.0, 0.3) > 0.3);
        assert!(oak_chance(400.0, 0.3) > oak_chance(400.0, -0.2) * 2.0);
        assert_eq!(oak_chance(1_100.0, 0.3), 0.0);
    }

    #[test]
    fn at_the_forests_limit_trees_stand_in_islands_and_in_the_lee() {
        // Ground a little past the tree line: along a line across it, some places hold trees
        // and some none, so the edge is islands, not a line; and the lee holds more than a rise.
        let sites = Microsites::new(terra_gen::Seed(5));
        let limit = season(TREE_LINE_SEASON_C - 0.4);
        let along: Vec<f32> = (0..400)
            .map(|k| {
                let at = glam::Vec3::new(k as f32 * 10.0, 30_000.0, 0.0);
                stand_in(&limit, 5.0, Footing::Soil, sites.at(at, 0.0)).density
            })
            .collect();
        let wooded = along.iter().filter(|&&d| d > 0.3).count();
        let open = along.iter().filter(|&&d| d == 0.0).count();
        assert!(
            wooded > 20 && open > 20,
            "{wooded} wooded, {open} open of 400"
        );
        let at = glam::Vec3::new(0.0, 30_000.0, 0.0);
        assert!(sites.at(at, 12.0) > sites.at(at, -12.0) + 0.6);
    }
}
