//! Places found by what the land's rules make of them, as a demo or a settlement asks for
//! one (docs/demos.md, D321): "a basin holding a lake, with ground 1 km above it within 3 km".
//! A query runs over the land round the planet's start and keeps the place that fits best,
//! the same on every machine. Nothing is painted: if the rules put no such place on a world,
//! the query says so, and why.

use crate::Land;
use crate::lakes;
use crate::region::Region;
use glam::{Vec2, Vec3};

/// A mountain lake found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Site {
    /// The direction from the centre to its deepest water.
    pub toward: Vec3,
    /// Its water's level over the sea, and the tree line's height there, in metres.
    pub level_m: f32,
    pub tree_line_m: f32,
    /// How big it is, in square metres, and how deep, in metres, as the search's grid found.
    pub area_m2: f32,
    pub deepest_m: f32,
    /// The highest ground within the query's reach of it, over the sea.
    pub highest_m: f32,
    /// Kilometres east and north of the planet's start.
    pub at_km: Vec2,
    /// The direction from the centre to the dry ground nearest its deepest water: where one
    /// stands at its edge looking over it.
    pub shore: Vec3,
}

/// What a mountain lake must be (docs/demos.md, §1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MountainLake {
    /// How far under the tree line its water lies, at least and at most, in metres: so the
    /// forest climbs that far above it before it gives out.
    pub under_tree_line_m: (f32, f32),
    /// How much higher the highest ground within `reach_m` of it stands, at least.
    pub peaks_above_m: f32,
    pub reach_m: f32,
    /// How big it is, at least and at most, in square metres, and how deep at least, in metres.
    pub area_m2: (f32, f32),
    pub deepest_m: f32,
    /// How far from the planet's start to look, in metres, and the grid looked over.
    pub search_m: f32,
    pub step_m: f32,
}

impl MountainLake {
    /// The demo's (docs/demos.md, open question 1): forest 350 to 650 m above the water and
    /// peaks a kilometre over it within 3 km, a lake a tenth of a square kilometre to one, as
    /// Morskie Oko's 0.35 is, under a tree line 150 m over it.
    pub const DEMO: MountainLake = MountainLake {
        under_tree_line_m: (250.0, 700.0),
        peaks_above_m: 900.0,
        reach_m: 4000.0,
        area_m2: (100_000.0, 1_000_000.0),
        deepest_m: 8.0,
        search_m: 40_000.0,
        step_m: 100.0,
    };
}

/// How many lakes a query looked at, and what ruled them out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Searched {
    pub lakes: usize,
    pub wrong_size: usize,
    pub wrong_height: usize,
    pub no_peaks: usize,
    pub fit: usize,
}

impl Land {
    /// The direction from the centre so many metres east and north of a direction, along the
    /// ground.
    pub fn along(&self, from: Vec3, east_north_m: Vec2) -> Vec3 {
        let from = from.normalize();
        let (east, north) = (self.frame.east(from), self.frame.north(from));
        let along = east * east_north_m.x + north * east_north_m.y;
        let distance = along.length();
        if distance < 1e-3 {
            return from;
        }
        let angle = distance / self.radius_m();
        (from * libm::cosf(angle) + along / distance * libm::sinf(angle)).normalize()
    }

    /// The best mountain lake near the planet's start, if the land has one.
    pub fn mountain_lake(&self, start: Vec3, want: &MountainLake) -> Option<Site> {
        self.mountain_lake_searched(start, want).0
    }

    /// [`Land::mountain_lake`], and how many lakes each of its tests ruled out, so a query that
    /// finds nothing says why.
    pub fn mountain_lake_searched(
        &self,
        start: Vec3,
        want: &MountainLake,
    ) -> (Option<Site>, Searched) {
        let mut searched = Searched::default();
        if self.ground.mountains().is_none() {
            return (None, searched);
        }
        let start = start.normalize();
        let region = Region::sample(self, start, want.search_m * 2.0, want.step_m);
        let sea = self.sea_radius_m() - self.radius_m();
        let found = lakes::fill(&region, self.ground.sea_radius_m().map(|_| sea));
        let n = region.cells;
        let reach = (want.reach_m / want.step_m).ceil() as i64;
        let mut best: Option<(f32, Site)> = None;
        for (index, lake) in found.lakes.iter().enumerate() {
            searched.lakes += 1;
            if lake.area_m2 < want.area_m2.0
                || lake.area_m2 > want.area_m2.1
                || lake.deepest_m < want.deepest_m
            {
                searched.wrong_size += 1;
                continue;
            }
            let (c, r) = lake.deepest;
            let toward = region.direction(c, r);
            let level_m = lake.level_m - sea;
            let tree_line_m = self
                .fields
                .tree_line_m(self.frame.latitude(toward), 0.0, 6000.0)
                .unwrap_or(0.0);
            let under = tree_line_m - level_m;
            if under < want.under_tree_line_m.0 || under > want.under_tree_line_m.1 {
                searched.wrong_height += 1;
                continue;
            }
            let mut highest = f32::MIN;
            for dr in -reach..=reach {
                for dc in -reach..=reach {
                    let (cc, rr) = (c as i64 + dc, r as i64 + dr);
                    if cc < 0 || rr < 0 || cc >= n as i64 || rr >= n as i64 {
                        continue;
                    }
                    if ((dc * dc + dr * dr) as f32).sqrt() > reach as f32 {
                        continue;
                    }
                    highest = highest.max(region.height(cc as usize, rr as usize));
                }
            }
            let highest_m = highest - sea;
            if highest_m < level_m + want.peaks_above_m {
                searched.no_peaks += 1;
                continue;
            }
            searched.fit += 1;
            // Best: the forest's climb nearest the middle of what is wanted, its size nearest
            // Morskie Oko's, then nearest the start.
            let offset = region.offset(c, r);
            let middle = (want.under_tree_line_m.0 + want.under_tree_line_m.1) * 0.5;
            let score = (under - middle).abs() / 100.0
                + libm::logf(lake.area_m2 / 350_000.0).abs()
                + offset.length() / want.search_m;
            let site = Site {
                toward,
                level_m,
                tree_line_m,
                area_m2: lake.area_m2,
                deepest_m: lake.deepest_m,
                highest_m,
                at_km: self.east_north_km(start, toward),
                shore: shore(&region, &found, index as u32, (c, r)),
            };
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, site));
            }
        }
        (best.map(|(_, site)| site), searched)
    }

    /// How far east and north of `from` a direction is, in kilometres, along the ground: the
    /// inverse of [`Land::along`].
    pub fn east_north_km(&self, from: Vec3, toward: Vec3) -> Vec2 {
        let from = from.normalize();
        let toward = toward.normalize();
        let angle = libm::acosf(from.dot(toward).clamp(-1.0, 1.0));
        let level = (toward - from * from.dot(toward)).normalize_or_zero();
        let (east, north) = (self.frame.east(from), self.frame.north(from));
        Vec2::new(level.dot(east), level.dot(north)) * angle * self.radius_m() / 1000.0
    }
}

/// A walk round a lake's shore from where one stands by it (`Start` by a lake): spots a little
/// inland of its water every `step_m` along the shore, half way round, as metres right and
/// back of one facing the water from `shore` (the sandbox's east and south of the start), for
/// a film's or a bench's route to follow (`places.ts`).
pub fn shore_walk(
    waters: &crate::water::Waters,
    shore: Vec3,
    toward: Vec3,
    step_m: f32,
) -> Vec<Vec2> {
    let region = waters.region();
    let Some((c, r)) = region.cell_of(toward) else {
        return Vec::new();
    };
    let Some(lake) = waters.lake_at(c, r) else {
        return Vec::new();
    };
    let n = region.cells;
    // The dry cells along the lake's edge, in the region's own east and north.
    let mut edge: Vec<Vec2> = Vec::new();
    let mut wet = Vec2::ZERO;
    let mut count = 0.0;
    for (c, r) in waters.cells_of(lake) {
        wet += region.offset(c, r);
        count += 1.0;
        for (dc, dr) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
            let (cc, rr) = (c as i64 + dc, r as i64 + dr);
            if cc < 0 || rr < 0 || cc >= n as i64 || rr >= n as i64 {
                continue;
            }
            if waters.lake_at(cc as usize, rr as usize).is_none() {
                edge.push(region.offset(cc as usize, rr as usize));
            }
        }
    }
    if edge.is_empty() {
        return Vec::new();
    }
    let middle = wet / count;
    let angle = |p: Vec2| libm::atan2f(p.y - middle.y, p.x - middle.x);
    let on_plane = |d: Vec3| {
        let p = d * (region.middle.length() / d.dot(region.middle.normalize())) - region.middle;
        Vec2::new(p.dot(region.east), p.dot(region.north))
    };
    let from = angle(on_plane(shore.normalize()));
    // Round the shore by angle from where one stands, half way.
    edge.sort_by(|a, b| {
        let turn = |p: Vec2| (angle(p) - from).rem_euclid(core::f32::consts::TAU);
        turn(*a).total_cmp(&turn(*b))
    });
    edge.dedup();
    let start = on_plane(shore.normalize());
    let up = shore.normalize();
    let water = toward.normalize();
    let facing3 = (water - up * water.dot(up)).normalize_or_zero();
    let facing = Vec2::new(facing3.dot(region.east), facing3.dot(region.north)).normalize_or_zero();
    let right = Vec2::new(facing.y, -facing.x);
    let mut walk = Vec::new();
    let mut last = start;
    for p in edge {
        if (angle(p) - from).rem_euclid(core::f32::consts::TAU) > core::f32::consts::PI {
            break;
        }
        if p.distance(last) < step_m {
            continue;
        }
        // A few metres inland of the water, away from the lake's middle.
        let inland = p + (p - middle).normalize_or_zero() * 6.0;
        let at = inland - start;
        walk.push(Vec2::new(at.dot(right), -at.dot(facing)));
        last = p;
    }
    walk
}

/// The dry cell of a region nearest a lake's cell, as the crow flies, outside the lake; the
/// cell itself if the lake fills the region.
fn shore(region: &Region, lakes: &lakes::Lakes, index: u32, from: (usize, usize)) -> Vec3 {
    let n = region.cells as i64;
    let (fc, fr) = (from.0 as i64, from.1 as i64);
    let mut best: Option<(i64, (usize, usize))> = None;
    for ring in 1..n {
        if best.is_some_and(|(d, _)| d <= ring * ring) {
            break;
        }
        for dr in -ring..=ring {
            for dc in -ring..=ring {
                if dr.abs() != ring && dc.abs() != ring {
                    continue;
                }
                let (c, r) = (fc + dc, fr + dr);
                if c < 0 || r < 0 || c >= n || r >= n {
                    continue;
                }
                let k = (r * n + c) as usize;
                if lakes.lake[k] == Some(index) {
                    continue;
                }
                let d = dc * dc + dr * dr;
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, (c as usize, r as usize)));
                }
            }
        }
    }
    let (c, r) = best.map_or(from, |(_, cell)| cell);
    region.direction(c, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PLANET_START, ground, start_climate};
    use terra_core::{PlanetSpec, Seed};

    fn land(radius_m: f32) -> Land {
        Land::new(
            ground::world(PlanetSpec {
                seed: Seed(1),
                radius_m,
            }),
            start_climate().unwrap(),
        )
    }

    #[test]
    fn the_lake_found_meets_what_was_asked_of_it() {
        let land = land(30_000.0);
        let want = MountainLake::DEMO;
        let (site, searched) = land.mountain_lake_searched(PLANET_START, &want);
        let site = site.unwrap_or_else(|| panic!("no mountain lake: {searched:?}"));
        // Counted again from the ground and the climate, not from the query's own sums.
        let bottom = land.surface(site.toward).length() - land.sea_radius_m();
        assert!(bottom < site.level_m, "{bottom} not under {}", site.level_m);
        let line = land
            .fields
            .tree_line_m(land.frame.latitude(site.toward), 0.0, 6000.0)
            .unwrap();
        let under = line - site.level_m;
        assert!(
            (want.under_tree_line_m.0..=want.under_tree_line_m.1).contains(&under),
            "{under} under the tree line"
        );
        assert!(site.highest_m >= site.level_m + want.peaks_above_m);
        assert!(
            land.along(PLANET_START, site.at_km * 1000.0)
                .distance(site.toward)
                < 1e-3
        );
    }

    #[test]
    fn a_world_without_mountains_has_no_mountain_lake() {
        let land = land(2000.0);
        assert_eq!(land.mountain_lake(PLANET_START, &MountainLake::DEMO), None);
    }
}
