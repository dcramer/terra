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

/// Which kind a tree is (D313, docs/demos.md open question 4): the lush forest's conifers by
/// their niches. Norway spruce holds the shaded slopes and the cooler, damper ground, as it
/// makes the Tatras' forest; Scots pine the sunny, drier slopes; and past the closed forest,
/// both stunted to krummholz, the dwarf mountain pine's place, which is not yet a kind of
/// its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Conifer {
    Spruce,
    Pine,
}

/// A tree the land's rules stand somewhere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tree {
    /// Where its foot is, on the ground as generated.
    pub at: glam::Vec3,
    pub kind: Conifer,
    /// Its stature, as a share of its kind's full height, and a number of its own, 0 to 1.
    pub stature: f32,
    pub luck: f32,
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
        use terra_core::Water;
        let up = point.normalize_or(glam::Vec3::Y);
        let water = self.round.waters.level(up);
        let footing = self.round.land.footing(point, water);
        self.round.stand(point, normal, footing)
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
                if stand.density <= 0.0 || rng.unit() > stand.density {
                    continue;
                }
                let place = land.place(point, normal);
                // Spruce in the shade and the cool, pine where the sun bakes the slope, the
                // two mixing over a band of facing.
                let sunny = smoothstep(0.05, 0.3, place.sunward) + (rng.unit() - 0.5) * 0.5;
                let kind = if sunny > 0.5 {
                    Conifer::Pine
                } else {
                    Conifer::Spruce
                };
                out.push(Tree {
                    at: point,
                    kind,
                    stature: (stand.stature * (0.85 + 0.3 * rng.unit())).min(1.0),
                    luck: rng.unit(),
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

    #[test]
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
}
