//! The ground under a base (construction-001, gap 3): its height under each column of
//! cubes, sampled when a plan is laid and kept with it, so every machine agrees on what
//! is joined to the ground whatever its terrain does. Heights are whole centimetres in the
//! base's own space, as 2 bytes a column.
//!
//! A column nothing has sampled, as under what the shortcuts built before plans, has its
//! ground at the base's own level, the foot of its first storey's walls.
//!
//! **Footings** (D194, construction-001 rule 9) fill the space between a building and the
//! ground under it: under every column of its ground storey's walls, from the floor's layer,
//! and under its floor's edges, from the layer below the floor, down to the cube the ground
//! passes through, so they are set into the ground and no gap shows; they stop on anything
//! already there. Footings are their own kind's cubes, a part of their own, built first.

use crate::{Base, CELL_M, Cube, Fill, Kind, floor_layer};
use glam::IVec3;
use std::collections::{BTreeMap, BTreeSet};

/// The most cubes a footing goes down, four metres: deeper than that a building wants
/// posts or stilts (construction-001, gap 3), so it stops there and what is over it is not
/// joined to the ground.
pub const FOOTING_MOST: i32 = 8;

/// The ground's height where nothing was sampled, in centimetres.
pub const LEVEL_CM: i16 = 0;

/// A cube's height in centimetres.
pub fn cube_cm() -> i32 {
    (CELL_M * 100.0).round() as i32
}

/// The ground's height under one column of cubes, as a plan carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    /// The column, counted in cubes east and south.
    pub column: [i32; 2],
    pub height_cm: i16,
}

impl Sample {
    /// A sample of a height in metres, rounded to the centimetre.
    pub fn metres(column: [i32; 2], height: f32) -> Sample {
        let cm = (height * 100.0)
            .round()
            .clamp(f32::from(i16::MIN), f32::from(i16::MAX));
        Sample {
            column,
            height_cm: cm as i16,
        }
    }
}

/// The ground's sampled heights under a base.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ground {
    heights: BTreeMap<[i32; 2], i16>,
}

impl Ground {
    /// The sampled height under a column, if any.
    pub fn sample(&self, column: [i32; 2]) -> Option<i16> {
        self.heights.get(&column).copied()
    }

    /// The height under a column, in centimetres: its sample, or the base's level.
    pub fn height_cm(&self, column: [i32; 2]) -> i16 {
        self.sample(column).unwrap_or(LEVEL_CM)
    }

    /// Whether the ground reaches a cube: it passes through it, or the cube is below it.
    pub fn reaches(&self, at: IVec3) -> bool {
        at.y * cube_cm() <= i32::from(self.height_cm([at.x, at.z]))
    }

    pub(crate) fn set(&mut self, column: [i32; 2], height: Option<i16>) {
        match height {
            Some(height) => self.heights.insert(column, height),
            None => self.heights.remove(&column),
        };
    }

    /// Every sample, sorted by column.
    pub fn samples(&self) -> impl Iterator<Item = Sample> + '_ {
        self.heights
            .iter()
            .map(|(&column, &height_cm)| Sample { column, height_cm })
    }
}

/// The cubes footings fill under columns, each from its top layer down to the cube the
/// ground passes through (its foot at or below `height_cm` of its column), stopping on
/// anything already there and after [`FOOTING_MOST`] cubes, in order.
pub fn footing(
    base: &Base,
    columns: impl IntoIterator<Item = ([i32; 2], i32)>,
    height_cm: impl Fn([i32; 2]) -> i16,
) -> Vec<IVec3> {
    let mut cubes = Vec::new();
    let mut seen = BTreeSet::new();
    for (column, top) in columns {
        if !seen.insert(column) {
            continue;
        }
        let ground = i32::from(height_cm(column));
        for y in (top - FOOTING_MOST + 1..=top).rev() {
            let at = IVec3::new(column[0], y, column[1]);
            if base.cube(at).is_some() {
                break;
            }
            cubes.push(at);
            if y * cube_cm() <= ground {
                break;
            }
        }
    }
    cubes
}

impl Base {
    /// Keeps samples of the ground's height under columns, as a plan carries them, for what
    /// the shortcuts build: the ground beside a building's walls, which its roles are read
    /// from (`layout::roles`), and under them.
    pub fn sample_ground(&mut self, samples: impl IntoIterator<Item = Sample>) {
        for sample in samples {
            self.ground.set(sample.column, Some(sample.height_cm));
        }
    }

    /// Every column of the base's cubes and those round them, sorted: where its ground is
    /// sampled.
    pub fn ground_ring(&self) -> Vec<[i32; 2]> {
        let mut columns = BTreeSet::new();
        for column in self.occupied_columns() {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    columns.insert([column.x + dx, column.y + dz]);
                }
            }
        }
        columns.into_iter().collect()
    }

    /// Where footings go under a building's ground storey (D194), as each column and the
    /// layer its footing's top is at: the floor's layer under every column of its walls and
    /// of anything else standing on the ground storey's foot, the layer under the floor
    /// under its floor's edges, in order.
    pub fn footing_columns(&self) -> Vec<([i32; 2], i32)> {
        let floor = floor_layer(0);
        let walls: BTreeSet<[i32; 2]> = self
            .walls()
            .filter(|(tile, _)| tile.y == 0)
            .map(|(tile, _)| [tile.x, tile.z])
            .collect();
        let floored = |[x, z]: [i32; 2]| self.cube(IVec3::new(x, floor, z)).is_some();
        let mut columns: Vec<([i32; 2], i32)> = walls.iter().map(|&c| (c, floor)).collect();
        for (at, _) in self.cubes().filter(|(at, cube)| {
            at.y == floor && cube.fill != Fill::Open && !walls.contains(&[at.x, at.z])
        }) {
            let edge = [[1, 0], [-1, 0], [0, 1], [0, -1]]
                .iter()
                .any(|[dx, dz]| !floored([at.x + dx, at.z + dz]));
            if edge {
                columns.push(([at.x, at.z], floor - 1));
            }
        }
        // Whatever else stands on the ground storey's foot, a post, a flight of steps, a
        // low wall, from the floor's layer, as a wall does.
        let foot = crate::wall_layer(0);
        for (&[x, z], layers) in &self.columns {
            if !walls.contains(&[x, z]) && layers.first() == Some(&foot) && !floored([x, z]) {
                columns.push(([x, z], floor));
            }
        }
        columns.sort();
        columns
    }

    /// Lays footings of a kind under what the shortcuts built, at once, as built cubes:
    /// the floor's layer under its walls of the footing's kind, and the cubes under its
    /// walls and its floor's edges down to the ground, as `height_cm` gives it under each
    /// column. Returns whether it changed anything.
    pub fn lay_footings(&mut self, kind: Kind, height_cm: impl Fn([i32; 2]) -> i16) -> bool {
        let columns = self.footing_columns();
        let stone = Some(Cube::new(kind, Fill::Solid));
        let mut changed = false;
        // Under its walls, the floor's layer is the footing's top, whether a floor is laid
        // there or not; then each column on down to the ground, where it does not reach
        // the floor's layer already.
        let floor = floor_layer(0);
        let mut below = Vec::new();
        for (column, top) in columns {
            let at = IVec3::new(column[0], floor, column[1]);
            if top == floor
                && self
                    .cube(at)
                    .is_none_or(|cube| cube.kind != kind && cube.fill == Fill::Solid)
            {
                changed |= self.set(at.to_array(), stone);
            }
            if floor * cube_cm() > i32::from(height_cm(column)) {
                below.push((column, floor - 1));
            }
        }
        for at in footing(self, below, height_cm) {
            changed |= self.set(at.to_array(), stone);
        }
        if changed {
            self.settle();
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BuildOp;

    #[test]
    fn a_footing_goes_down_to_the_cube_the_ground_passes_through_and_stops_on_what_is_there() {
        let mut base = Base::default();
        base.apply(BuildOp::Fill {
            min: [2, -3, 0],
            max: [2, -3, 0],
            cube: Some(Cube::new(Kind(2), Fill::Solid)),
        });
        // Level ground, 80 cm down, 2 m down, far down, and on something built.
        let heights = |column: [i32; 2]| match column[0] {
            0 => 0,
            1 => -80,
            3 => -200,
            4 => -2000,
            _ => -300,
        };
        let cubes = footing(&base, (0..5).map(|x| ([x, 0], -1)), heights);
        let depth =
            |x: i32| -> Vec<i32> { cubes.iter().filter(|at| at.x == x).map(|at| at.y).collect() };
        assert_eq!(depth(0), vec![-1]);
        assert_eq!(depth(1), vec![-1, -2]);
        assert_eq!(depth(2), vec![-1, -2]);
        assert_eq!(depth(3), vec![-1, -2, -3, -4]);
        assert_eq!(depth(4).len(), FOOTING_MOST as usize);
    }

    #[test]
    fn a_room_on_a_slope_stands_on_footings_under_its_walls_and_its_floor_inside() {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [5, 0, 3],
            kind: Kind(1),
        });
        // The ground falls away to the east, 30 cm a column.
        let slope = |column: [i32; 2]| (-30 * column[0]) as i16;
        assert!(base.lay_footings(Kind(2), slope));
        let stone = |at: IVec3| base.cube(at).is_some_and(|cube| cube.kind == Kind(2));
        for (tile, _) in base.walls().filter(|(tile, _)| tile.y == 0) {
            // Stone from the floor down to where the ground is.
            let ground = i32::from(slope([tile.x, tile.z]));
            let mut y = -1;
            while stone(IVec3::new(tile.x, y, tile.z)) {
                y -= 1;
            }
            assert!(
                (y + 1) * 50 <= ground,
                "{tile}: down to {} over {ground}",
                y + 1
            );
            // No deeper than that, but the floor's own layer, which is always stone.
            assert!(y + 1 == -1 || (y + 2) * 50 > ground, "{tile}: too deep");
        }
        // The floor inside is the walls' kind still, and nothing is under it.
        assert!(!stone(IVec3::new(2, -1, 2)));
        assert!(base.cube(IVec3::new(2, -2, 2)).is_none());
        // Its crawl space is no room.
        assert!(base.tiles().all(|(tile, _)| tile.y == 0));
        // Laid again, nothing changes.
        assert!(!base.lay_footings(Kind(2), slope));
    }

    #[test]
    fn the_ground_reaches_the_cube_it_passes_through_and_those_below() {
        let mut ground = Ground::default();
        // Unsampled: the base's level, so its floor layer and first wall layer.
        assert!(ground.reaches(IVec3::new(0, 0, 0)));
        assert!(ground.reaches(IVec3::new(0, -1, 0)));
        assert!(!ground.reaches(IVec3::new(0, 1, 0)));
        // A slope falling away: 80 cm down, so only cubes from -2 down.
        ground.set([3, 0], Some(Sample::metres([3, 0], -0.8).height_cm));
        assert!(ground.reaches(IVec3::new(3, -2, 0)));
        assert!(!ground.reaches(IVec3::new(3, -1, 0)));
    }
}
