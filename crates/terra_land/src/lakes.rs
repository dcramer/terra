//! Lakes: every depression of the ground as generated, filled to its spill point (D322), the
//! "basin filled to its own level" the engine's water asks for (docs/engine.md). Barnes,
//! Lehman and Mulla's Priority-Flood (2014, https://arxiv.org/abs/1511.04463), as GIS fills a
//! height grid: water enters from the grid's edge, and each cell is reached in order of the
//! lowest level water could stand at there, which is its own height or, in a hollow, the
//! level of the lowest way out. Where that stands above the ground, there is water.

use crate::region::Region;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// A region's lakes.
#[derive(Clone, Debug)]
pub struct Lakes {
    /// The level water stands at in each cell, over the nominal radius, in metres: the
    /// ground's height where there is no lake.
    pub level: Vec<f32>,
    /// Which lake each cell is in, as an index into `lakes`, or `None` on dry ground.
    pub lake: Vec<Option<u32>>,
    pub lakes: Vec<Lake>,
}

/// One lake.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lake {
    /// The level of its water over the nominal radius, in metres.
    pub level_m: f32,
    /// How many cells it covers, and its area, in square metres.
    pub cells: usize,
    pub area_m2: f32,
    /// Its deepest water, in metres, and the cell it is in.
    pub deepest_m: f32,
    pub deepest: (usize, usize),
}

/// The deepest a lake can be, in metres: glacial lakes run to a hundred metres and more (Morskie
/// Oko 51 m, Wielki Staw 79 m).
const DEEPEST_M: f32 = 80.0;
/// Water shallower than this is wet ground, not a lake.
const SHALLOWEST_M: f32 = 0.3;

/// A cell waiting to be reached, by the level water would stand at there: the lowest first.
#[derive(Clone, Copy, PartialEq)]
struct Waiting {
    level: f32,
    cell: usize,
}

impl Eq for Waiting {}

impl Ord for Waiting {
    fn cmp(&self, other: &Self) -> Ordering {
        // Lowest first, and the cell's index to break ties, so the fill is the same every time.
        other
            .level
            .total_cmp(&self.level)
            .then_with(|| other.cell.cmp(&self.cell))
    }
}

impl PartialOrd for Waiting {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Fills a region's depressions, water spilling off its edge, and finds the lakes they make;
/// `sea` is the sea's level over the nominal radius, below which nothing is a lake.
pub fn fill(region: &Region, sea: Option<f32>) -> Lakes {
    let n = region.cells;
    let height = &region.heights;
    let mut level = height.clone();
    let mut reached = vec![false; n * n];
    let mut waiting = BinaryHeap::new();
    for k in 0..n * n {
        let (c, r) = (k % n, k / n);
        if c == 0 || r == 0 || c == n - 1 || r == n - 1 {
            reached[k] = true;
            waiting.push(Waiting {
                level: height[k],
                cell: k,
            });
        }
    }
    while let Some(Waiting { level: at, cell }) = waiting.pop() {
        let (c, r) = ((cell % n) as i64, (cell / n) as i64);
        for (dc, dr) in [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let (nc, nr) = (c + dc, r + dr);
            if nc < 0 || nr < 0 || nc >= n as i64 || nr >= n as i64 {
                continue;
            }
            let next = nr as usize * n + nc as usize;
            if reached[next] {
                continue;
            }
            reached[next] = true;
            level[next] = height[next].max(at);
            waiting.push(Waiting {
                level: level[next],
                cell: next,
            });
        }
    }
    // A basin that would fill deeper than a lake can be stands with its lake at its bottom,
    // as deep as one can be: its water seeps or evaporates away before it reaches its rim. Our
    // ground has no rivers to cut a closed valley's outlet, so a valley ringed by ridges
    // would otherwise fill hundreds of metres up to its lowest pass.
    let mut bottoms: Vec<(usize, f32)> = Vec::new();
    {
        let mut seen = vec![false; n * n];
        for start in 0..n * n {
            if seen[start] || level[start] - height[start] < SHALLOWEST_M {
                continue;
            }
            // One basin's cells: under water, all at its spill level.
            let spill = level[start];
            let mut basin = vec![start];
            let mut stack = vec![start];
            seen[start] = true;
            let mut lowest = height[start];
            while let Some(k) = stack.pop() {
                lowest = lowest.min(height[k]);
                let (c, r) = ((k % n) as i64, (k / n) as i64);
                for (dc, dr) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let (nc, nr) = (c + dc, r + dr);
                    if nc < 0 || nr < 0 || nc >= n as i64 || nr >= n as i64 {
                        continue;
                    }
                    let next = nr as usize * n + nc as usize;
                    if !seen[next]
                        && (level[next] - spill).abs() < 1e-3
                        && level[next] > height[next]
                    {
                        seen[next] = true;
                        basin.push(next);
                        stack.push(next);
                    }
                }
            }
            if spill - lowest > DEEPEST_M {
                let stands = lowest + DEEPEST_M;
                bottoms.extend(basin.into_iter().map(|k| (k, stands)));
            }
        }
    }
    for (k, stands) in bottoms {
        level[k] = height[k].max(stands);
    }
    // The lakes: cells under water, joined to their neighbours under the same level, however
    // shallow those are. A lake's shallows are its water too: left dry, a flat basin's margin
    // a hand under the water was bare ground below the lake's level, tens of metres wide past
    // where its water was drawn (D381's lake). Only a basin shallow everywhere is wet ground.
    let under = |k: usize| level[k] > height[k] && sea.is_none_or(|sea| level[k] > sea);
    let wet = |k: usize| under(k) && level[k] - height[k] >= SHALLOWEST_M;
    let mut lake = vec![None; n * n];
    let mut lakes = Vec::new();
    for start in 0..n * n {
        if lake[start].is_some() || !wet(start) {
            continue;
        }
        let index = lakes.len() as u32;
        let mut found = Lake {
            level_m: level[start],
            cells: 0,
            area_m2: 0.0,
            deepest_m: 0.0,
            deepest: (start % n, start / n),
        };
        let mut stack = vec![start];
        lake[start] = Some(index);
        while let Some(k) = stack.pop() {
            found.cells += 1;
            let depth = level[k] - height[k];
            if depth > found.deepest_m {
                found.deepest_m = depth;
                found.deepest = (k % n, k / n);
            }
            let (c, r) = ((k % n) as i64, (k / n) as i64);
            for (dc, dr) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (nc, nr) = (c + dc, r + dr);
                if nc < 0 || nr < 0 || nc >= n as i64 || nr >= n as i64 {
                    continue;
                }
                let next = nr as usize * n + nc as usize;
                if lake[next].is_none() && under(next) && (level[next] - level[k]).abs() < 1e-3 {
                    lake[next] = Some(index);
                    stack.push(next);
                }
            }
        }
        found.area_m2 = found.cells as f32 * region.cell_m * region.cell_m;
        lakes.push(found);
    }
    Lakes { level, lake, lakes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn region(cells: usize, height: impl Fn(f32, f32) -> f32) -> Region {
        let mut region = Region {
            middle: Vec3::Y * 30_000.0,
            east: Vec3::X,
            north: Vec3::Z,
            cells,
            cell_m: 10.0,
            heights: vec![0.0; cells * cells],
        };
        for r in 0..cells {
            for c in 0..cells {
                let at = region.offset(c, r);
                region.heights[r * cells + c] = height(at.x, at.y);
            }
        }
        region
    }

    #[test]
    fn a_bowl_fills_to_its_rim_and_no_higher() {
        // A bowl 20 m deep inside a ring 6 m high, with a gap in the ring only 2 m high, and
        // the land outside lower still.
        let land = region(61, |x, y| {
            let d = (x * x + y * y).sqrt();
            if d < 200.0 {
                -20.0 + 20.0 * d / 200.0
            } else if d < 260.0 {
                if x > 0.0 && y.abs() < 15.0 { 2.0 } else { 6.0 }
            } else {
                -10.0
            }
        });
        let lakes = fill(&land, None);
        assert_eq!(lakes.lakes.len(), 1, "{:?}", lakes.lakes);
        let lake = lakes.lakes[0];
        // Its level is the gap's: the lowest way out.
        assert_eq!(lake.level_m, 2.0);
        assert!((lake.deepest_m - 22.0).abs() < 1.0, "{}", lake.deepest_m);
        for (k, under) in lakes.lake.iter().enumerate() {
            if under.is_some() {
                assert!(lakes.level[k] > land.heights[k]);
                assert_eq!(lakes.level[k], lake.level_m);
            }
        }
        // It never spills: every dry neighbour of the lake stands at its level or higher.
        for k in 0..61 * 61 {
            if lakes.lake[k].is_none() {
                continue;
            }
            for next in [k - 1, k + 1, k - 61, k + 61] {
                if lakes.lake[next].is_none() {
                    assert!(
                        land.heights[next] >= lake.level_m - 1e-3,
                        "spills at {next}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_lakes_shallows_are_its_water_and_a_shallow_hollow_is_wet_ground() {
        // A pond 3 m deep in the middle of a flat floor 0.1 m under the rim's gap, so its
        // shallows reach far out over the floor; and apart from it a hollow 0.2 m deep.
        let land = region(41, |x, y| {
            let d = (x * x + y * y).sqrt();
            let hollow = ((x - 120.0).powi(2) + (y - 120.0).powi(2)).sqrt();
            if d < 40.0 {
                -3.0
            } else if d < 110.0 {
                -0.1
            } else if hollow < 25.0 {
                0.8
            } else if x > 0.0 && y.abs() < 15.0 {
                0.0
            } else {
                1.0
            }
        });
        let lakes = fill(&land, None);
        assert_eq!(lakes.lakes.len(), 1, "{:?}", lakes.lakes);
        // Its water reaches over the whole floor, to the ground at its level.
        for (k, under) in lakes.lake.iter().enumerate() {
            assert_eq!(under.is_some(), land.heights[k] < -0.05, "cell {k}");
        }
    }

    #[test]
    fn ground_falling_off_the_edge_holds_no_water() {
        let tilted = region(40, |x, y| 0.1 * x + 0.05 * y);
        let lakes = fill(&tilted, None);
        assert!(lakes.lakes.is_empty());
        assert_eq!(lakes.level, tilted.heights);
    }

    #[test]
    fn below_the_sea_is_not_a_lake() {
        let bowl = region(40, |x, y| (x * x + y * y).sqrt() * 0.1 - 30.0);
        assert!(!fill(&bowl, None).lakes.is_empty());
        assert!(fill(&bowl, Some(0.0)).lakes.is_empty());
    }
}
