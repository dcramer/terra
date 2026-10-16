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
            // How steeply the ground stands over the water seen from its deepest point, the
            // most of any way round: peaks looming over it, not far off over forested hills.
            let mut looming = 0.0f32;
            for dr in -reach..=reach {
                for dc in -reach..=reach {
                    let (cc, rr) = (c as i64 + dc, r as i64 + dr);
                    if cc < 0 || rr < 0 || cc >= n as i64 || rr >= n as i64 {
                        continue;
                    }
                    if ((dc * dc + dr * dr) as f32).sqrt() > reach as f32 {
                        continue;
                    }
                    let height = region.height(cc as usize, rr as usize);
                    highest = highest.max(height);
                    let off = ((dc * dc + dr * dr) as f32).sqrt() * want.step_m;
                    if off > 0.0 {
                        looming = looming.max(libm::atan2f(height - lake.level_m, off));
                    }
                }
            }
            let highest_m = highest - sea;
            if highest_m < level_m + want.peaks_above_m {
                searched.no_peaks += 1;
                continue;
            }
            searched.fit += 1;
            // Best: the peaks looming steepest over it, a point for every 5 degrees, as Rysy's
            // walls stand over Morskie Oko; the forest's climb nearest the middle of what is
            // wanted; its size nearest Morskie Oko's; then nearest the start.
            let offset = region.offset(c, r);
            let middle = (want.under_tree_line_m.0 + want.under_tree_line_m.1) * 0.5;
            let score = (under - middle).abs() / 100.0
                + libm::logf(lake.area_m2 / 350_000.0).abs()
                + offset.length() / want.search_m
                - looming.to_degrees() / 5.0;
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

/// How high over a lake's water [`shore_walk`] keeps to, in metres: dry under foot and under
/// the camera, and below the height its shore's trees start (`woods::SHORE_BARE_M`), so the
/// walk runs on the bare band between the water and the forest's edge.
pub const SHORE_WALK_OVER_M: f32 = 0.5;
/// The walk keeps between these heights over the water all the way from spot to spot: never
/// wet, and no further into the shore's first, sparse trees (`woods::SHORE_WOODED_M`) than a
/// walker steps round, so it turns with the shore's bays, not with every crook of its edge.
const SHORE_WALK_BAND_M: (f32, f32) = (0.1, 1.6);

/// A walk round a lake's shore from where one stands by it (`Start` by a lake): spots on the
/// shore every `step_m` or nearer, half way round, as metres right and back of one facing the
/// water from `shore` (the sandbox's east and south of the start), for a film's or a bench's
/// route to follow (`places.ts`). Each spot is on the ground's own contour
/// [`SHORE_WALK_OVER_M`] over the water, found from the lake's edge cells by stepping along the
/// ground's slope; a spot is kept nearer than `step_m` where the straight way to the next
/// would leave the band between the water and the trees, so the walker neither wades across
/// a bay nor cuts into the forest over a point.
pub fn shore_walk(
    land: &Land,
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
    let level = waters.lake_radius_m(lake);
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
    // The ground's height over the water at a place east and north of the region's middle.
    let over = |p: Vec2| {
        let direction = region.middle + region.east * p.x + region.north * p.y;
        land.surface(direction).length() - level
    };
    // The contour's point nearest a place, by Newton's steps along the ground's slope.
    let on_contour = |from: Vec2| {
        let mut p = from;
        for _ in 0..40 {
            let miss = SHORE_WALK_OVER_M - over(p);
            if miss.abs() < 0.03 {
                return (p.distance(from) < 30.0).then_some(p);
            }
            let rise = Vec2::new(
                over(p + Vec2::X * 0.5) - over(p - Vec2::X * 0.5),
                over(p + Vec2::Y * 0.5) - over(p - Vec2::Y * 0.5),
            );
            if rise.length_squared() < 1e-6 {
                return None;
            }
            let step = rise * (miss / rise.length_squared());
            p += step.clamp_length_max(2.0);
        }
        None
    };
    let in_band = |a: Vec2, b: Vec2| {
        let samples = (a.distance(b).ceil() as usize).max(1);
        (0..=samples).all(|k| {
            let h = over(a.lerp(b, k as f32 / samples as f32));
            (SHORE_WALK_BAND_M.0..=SHORE_WALK_BAND_M.1).contains(&h)
        })
    };
    let from = angle(on_plane(shore.normalize()));
    // Round the shore by angle from where one stands, half way.
    edge.sort_by(|a, b| {
        let turn = |p: Vec2| (angle(p) - from).rem_euclid(core::f32::consts::TAU);
        turn(*a).total_cmp(&turn(*b))
    });
    edge.dedup();
    // The start's own frame: its level through the shore, facing the water, as `Start::at`
    // takes metres east and south. On a small world the region's level and the start's part
    // by metres a few hundred metres off, so each spot is put on the start's level along its
    // own direction from the centre.
    let up = shore.normalize();
    let standing = land.surface(up);
    let water = toward.normalize();
    let facing = (water - up * water.dot(up)).normalize_or_zero();
    let right = facing.cross(up);
    // The contour's points in turn round the shore, half way.
    let mut round: Vec<Vec2> = Vec::new();
    for p in edge {
        if (angle(p) - from).rem_euclid(core::f32::consts::TAU) > core::f32::consts::PI {
            break;
        }
        if let Some(p) = on_contour(p)
            && round.last().is_none_or(|last| last.distance(p) >= 2.0)
        {
            round.push(p);
        }
    }
    // Where even the way to the next point leaves the band, as across a bay the edge's
    // order jumped, it goes by the contour's points between, halving the way each time.
    fn bridge(
        a: Vec2,
        b: Vec2,
        depth: u32,
        in_band: &impl Fn(Vec2, Vec2) -> bool,
        on_contour: &impl Fn(Vec2) -> Option<Vec2>,
    ) -> Vec<Vec2> {
        if depth == 0 || in_band(a, b) {
            return Vec::new();
        }
        let Some(m) = on_contour((a + b) * 0.5) else {
            return Vec::new();
        };
        let mut out = bridge(a, m, depth - 1, in_band, on_contour);
        out.push(m);
        out.extend(bridge(m, b, depth - 1, in_band, on_contour));
        out
    }
    // From each spot, the furthest point within `step_m` the way to which keeps to the band.
    let mut spots: Vec<Vec2> = round.first().copied().into_iter().collect();
    let mut i = 0;
    while i + 1 < round.len() {
        let a = spots[spots.len() - 1];
        let mut furthest = None;
        for (j, p) in round.iter().enumerate().skip(i + 1) {
            if a.distance(*p) > step_m {
                break;
            }
            if in_band(a, *p) {
                furthest = Some(j);
            }
        }
        let j = furthest.unwrap_or(i + 1);
        if furthest.is_none() {
            spots.extend(bridge(a, round[j], 5, &in_band, &on_contour));
        }
        spots.push(round[j]);
        i = j;
    }
    spots
        .into_iter()
        .map(|p| {
            let direction = (region.middle + region.east * p.x + region.north * p.y).normalize();
            let at = direction * (standing.length() / direction.dot(up)) - standing;
            Vec2::new(at.dot(right), -at.dot(facing))
        })
        .collect()
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
    fn the_shore_walk_keeps_to_dry_shore_below_the_trees() {
        let land = land(30_000.0);
        let site = land
            .mountain_lake(PLANET_START, &MountainLake::DEMO)
            .unwrap();
        let waters = crate::water::Waters::round(&land, site.toward, 10_000.0, 10.0);
        let shore = waters.shore_of(site.toward).unwrap();
        let walk = shore_walk(&land, &waters, shore, site.toward, 25.0);
        assert!(walk.len() > 20, "{} spots", walk.len());
        // Each spot and the way between them put back on the ground as the sandbox's start
        // takes metres east and south (`Start::at`), its own level through the shore facing
        // the water, and measured against the lake's surface.
        let level = waters.lake_level(site.toward).unwrap();
        let up = shore.normalize();
        let standing = land.surface(up);
        let water = site.toward.normalize();
        let facing = (water - up * water.dot(up)).normalize();
        let right = facing.cross(up);
        let over = |at: Vec2| {
            let point = standing + right * at.x - facing * at.y;
            land.surface(point).length() - level
        };
        let mut going = 0.0;
        for pair in walk.windows(2) {
            going += pair[0].distance(pair[1]);
            for k in 0..=10 {
                let h = over(pair[0].lerp(pair[1], k as f32 / 10.0));
                assert!(h > 0.0, "{h:.2} m under the water by {:?}", pair[0]);
                assert!(
                    h < crate::woods::SHORE_WOODED_M,
                    "{h:.2} m over the water by {:?}, among the trees",
                    pair[0]
                );
            }
        }
        // Half way round a lake half a square kilometre big.
        assert!(going > 800.0, "{going:.0} m");
    }

    #[test]
    fn a_world_without_mountains_has_no_mountain_lake() {
        let land = land(2000.0);
        assert_eq!(land.mountain_lake(PLANET_START, &MountainLake::DEMO), None);
    }
}
