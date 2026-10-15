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

use crate::{Base, CELL_M, Cube, Facing, Fill, Kind, Shape, floor_layer};
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

/// Where a post laid from the cube `aimed` in a column stands on the ground, and the footing
/// under it (D194, D306): its foot, and the footing's cubes, top first. A post stands on a
/// footing whose top is at the ground or a little over it, as a post is set on a pier, never
/// buried: where the ground passes through the cube aimed at, the footing fills it and the
/// post starts on the cube over it; over a dip the footing goes down to the ground, as far
/// as a footing goes. A post standing on a cube has none and starts where it is aimed.
pub fn post_on_ground(
    base: &Base,
    column: [i32; 2],
    aimed: i32,
    height_cm: impl Fn([i32; 2]) -> i16,
) -> (i32, Vec<IVec3>) {
    let at = IVec3::new(column[0], aimed, column[1]);
    if base.cube(at - IVec3::Y).is_some() {
        return (aimed, Vec::new());
    }
    let ground = i32::from(height_cm(column));
    let foot = if aimed * cube_cm() < ground {
        aimed + 1
    } else {
        aimed
    };
    (foot, footing(base, [(column, foot - 1)], height_cm))
}

/// How many columns a flight from `foot`, rising toward `facing` a cube a column and `wide`
/// columns across, climbs to step onto what is ahead of it (D306): the fewest, up to two
/// storeys, for which the cube after its top step, on the top step's layer, is solid all
/// across it, none of its own cubes being taken. None where it reaches nothing.
pub fn flight_onto(base: &Base, foot: IVec3, facing: Facing, wide: i32) -> Option<i32> {
    let (forward, across) = (facing.step(), facing.turned(1).step());
    let across_all =
        |at: IVec3, test: &dyn Fn(IVec3) -> bool| (0..wide.max(1)).all(|w| test(at + across * w));
    for rise in 1..=2 * crate::STOREY_CUBES {
        let column = foot + forward * (rise - 1);
        let free = |at: IVec3| (foot.y..foot.y + rise).all(|y| base.cube(at.with_y(y)).is_none());
        if !across_all(column, &free) {
            return None;
        }
        let ahead = foot + forward * rise + IVec3::Y * (rise - 1);
        let solid = |at: IVec3| {
            base.cube(at)
                .is_some_and(|cube| cube.fill == Fill::Solid && !cube.kind.is_roof())
        };
        if across_all(ahead, &solid) {
            return Some(rise);
        }
    }
    None
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

/// How far the ground may be below a doorway's threshold, or a step's tread, with no step
/// down to it: less than a step's rise, in centimetres.
const NO_STEP_CM: i32 = 20;

/// The most step cubes down from a doorway, however far the ground falls away; a doorway
/// further over the ground than they reach gets none.
const MOST_STEPS: i32 = 8;

/// A doorway's cube (D261): an opening at a floor, whatever its height or how it was made,
/// with something not open under it as its threshold, a floor on one side of it at the
/// threshold's layer and none on the other, where the way out is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Doorway {
    /// The opening's lowest cube.
    pub at: IVec3,
    /// Level, out of the doorway, toward where there is no floor.
    pub outward: IVec3,
}

impl Base {
    /// Every doorway's lowest cube, sorted, taking the cubes `opening` names as open too, as
    /// a plan about to open them would leave them.
    pub fn doorways(&self, opening: &BTreeSet<[i32; 3]>) -> Vec<Doorway> {
        let open = |at: IVec3| {
            opening.contains(&at.to_array())
                || self.cube(at).is_some_and(|cube| cube.fill == Fill::Open)
        };
        let solid = |at: IVec3| !open(at) && self.cube(at).is_some();
        let mut cubes: BTreeSet<[i32; 3]> = opening.clone();
        cubes.extend(
            self.cubes()
                .filter(|(_, cube)| cube.fill == Fill::Open)
                .map(|(at, _)| at.to_array()),
        );
        let mut found = Vec::new();
        for at in cubes.into_iter().map(IVec3::from_array) {
            if !solid(at - IVec3::Y) || !open(at + IVec3::Y) {
                continue;
            }
            // A floor on one side at the threshold's layer, and nothing on the other.
            let floored = |out: IVec3| solid(at - IVec3::Y + out);
            let clear = |out: IVec3| self.cube(at + out).is_none() && !floored(out);
            // Inward, through the rest of the opening, a room to walk into: a hole in a wall
            // thicker than it, a window in a castle's curtain or a recess carved in a thick
            // wall, has a floor beside its sill but leads into the wall, not a room.
            let leads_in = |inward: IVec3| {
                let mut step = inward;
                while open(at + step) {
                    step += inward;
                }
                self.cube(at + step).is_none() && floored(step)
            };
            let outward = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
                .into_iter()
                .find(|&out| clear(out) && floored(-out) && leads_in(-out));
            if let Some(outward) = outward {
                found.push(Doorway { at, outward });
            }
        }
        found
    }

    /// Door steps (D194, D261): from every doorway down to the ground outside, as cubes of
    /// their own (the caller lays them in the footings' kind), a stair a column rising to
    /// the threshold, a riser of a quarter of a metre at a time, its first a half cube where
    /// one riser is all the drop needs, each on a footing down to the ground, so they are
    /// built, walked and saved as everything else is. The ground is `height_cm` under each
    /// column, as footings take it. Stops at any column something is in. In order, a
    /// doorway's from the door out, each column from the top down.
    pub fn door_steps(
        &self,
        opening: &BTreeSet<[i32; 3]>,
        height_cm: impl Fn([i32; 2]) -> i16,
    ) -> Vec<(IVec3, Shape)> {
        let quarter = cube_cm() / 2;
        let mut out: Vec<(IVec3, Shape)> = Vec::new();
        let mut taken: BTreeSet<[i32; 3]> = BTreeSet::new();
        for Doorway { at, outward } in self.doorways(opening) {
            let toward = Facing::ALL
                .into_iter()
                .find(|facing| facing.step() == -outward)
                .unwrap_or_default();
            // The tread a walker is on, coming out: the threshold first. A door further over
            // the ground than steps go wants a stair, not steps.
            let mut tread = at.y * cube_cm();
            let first = at + outward;
            if tread - i32::from(height_cm([first.x, first.z])) > MOST_STEPS * cube_cm() {
                continue;
            }
            for k in 1..=MOST_STEPS {
                let column = at + outward * k;
                let ground = i32::from(height_cm([column.x, column.z]));
                if tread - ground <= NO_STEP_CM {
                    break;
                }
                let layer = at.y - k;
                let top = column.with_y(layer);
                // The last step, where the ground rises over a stair's lower tread, as on a
                // slope falling away from the door: the stair would show only a thin wedge
                // of its upper tread over it, so a whole cube is the step, its top the tread
                // where a stair would be the last (D308's films).
                let beyond = i32::from(height_cm([column.x + outward.x, column.z + outward.z]));
                let last = k == MOST_STEPS || layer * cube_cm() + quarter - beyond <= NO_STEP_CM;
                let shape = if k == 1 && tread - quarter - ground <= NO_STEP_CM {
                    Shape::Half
                } else if last && ground >= layer * cube_cm() + quarter {
                    Shape::Whole
                } else {
                    Shape::Stair(toward)
                };
                let mut cubes = vec![(top, shape)];
                if layer * cube_cm() > ground {
                    cubes.extend(
                        footing(self, [([column.x, column.z], layer - 1)], &height_cm)
                            .into_iter()
                            .map(|at| (at, Shape::Whole)),
                    );
                }
                if cubes
                    .iter()
                    .any(|(at, _)| self.cube(*at).is_some() || taken.contains(&at.to_array()))
                {
                    break;
                }
                taken.extend(cubes.iter().map(|(at, _)| at.to_array()));
                out.extend(cubes);
                if matches!(shape, Shape::Half | Shape::Whole) {
                    break;
                }
                tread = layer * cube_cm() + quarter;
            }
        }
        out
    }

    /// Lays door steps of a kind at once, as built cubes, for what the shortcuts built
    /// ([`Base::door_steps`]). Returns whether it changed anything.
    pub fn lay_door_steps(&mut self, kind: Kind, height_cm: impl Fn([i32; 2]) -> i16) -> bool {
        let steps = self.door_steps(&BTreeSet::new(), height_cm);
        let mut changed = false;
        for (at, shape) in steps {
            let cube = Cube {
                shape,
                ..Cube::new(kind, Fill::Solid)
            };
            changed |= self.set(at.to_array(), Some(cube));
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
    use crate::{BuildOp, Panel};

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
        assert!(base.room_cells().all(|(at, _)| at.y >= 0));
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

    /// A room, its floor the layer under the ground's level, with a doorway two cubes wide
    /// in its south wall.
    fn with_door() -> Base {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [5, 0, 5],
            kind: Kind(1),
        });
        for x in [2, 3] {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 5],
                panel: Some(Panel::Door),
                kind: Kind(1),
            });
        }
        base
    }

    #[test]
    fn a_doorway_is_an_opening_at_a_floor_and_opens_toward_no_floor() {
        let base = with_door();
        let doorways = base.doorways(&BTreeSet::new());
        assert_eq!(
            doorways,
            [2, 3].map(|x| Doorway {
                at: IVec3::new(x, 0, 5),
                outward: IVec3::Z
            })
        );
        // A window's opening is no doorway: no floor beside its sill.
        let mut windowed = base.clone();
        windowed.apply(BuildOp::SetWall {
            tile: [0, 0, 2],
            panel: Some(Panel::WINDOW),
            kind: Kind(1),
        });
        assert_eq!(windowed.doorways(&BTreeSet::new()).len(), 2);
        // An opening about to be cut at a floor by hand is one, wherever it is.
        let cut: BTreeSet<[i32; 3]> = [[5, 0, 2], [5, 1, 2], [5, 2, 2], [5, 3, 2]].into();
        assert!(base.doorways(&cut).contains(&Doorway {
            at: IVec3::new(5, 0, 2),
            outward: IVec3::X
        }));
    }

    #[test]
    fn a_hole_in_a_thick_wall_above_the_floor_is_no_doorway() {
        // A wall four cubes thick and six high, east to west, on a floor that runs on south
        // of it under a room.
        let mut base = Base::default();
        let solid = Some(Cube::new(Kind(2), Fill::Solid));
        base.apply(BuildOp::Fill {
            min: [0, -1, 0],
            max: [7, -1, 7],
            cube: solid,
        });
        base.apply(BuildOp::Fill {
            min: [0, 0, 0],
            max: [7, 5, 3],
            cube: solid,
        });
        // A window two cubes deep and two high cut into its north face a metre up: its sill
        // has wall beside it inside, but leads into the wall, not a room.
        let window: BTreeSet<[i32; 3]> = [[3, 2, 0], [3, 3, 0], [3, 2, 1], [3, 3, 1]].into();
        assert!(base.doorways(&window).is_empty());
        // Cut right through at the floor, it leads into the room: a doorway out to the north.
        let through: BTreeSet<[i32; 3]> = (0..4)
            .flat_map(|z| (0..4).map(move |y| [3, y, z]))
            .collect();
        assert_eq!(
            base.doorways(&through),
            [Doorway {
                at: IVec3::new(3, 0, 0),
                outward: IVec3::NEG_Z
            }]
        );
    }

    #[test]
    fn door_steps_are_stairs_a_riser_apart_down_to_the_ground_on_footings() {
        let base = with_door();
        // The ground falls away south, 30 cm a cube from 30 cm down.
        let ground = |[_, z]: [i32; 2]| (-30 - 30 * (z - 5).max(0)) as i16;
        let steps = base.door_steps(&BTreeSet::new(), ground);
        let in_column = |x: i32, z: i32| -> Vec<(i32, Shape)> {
            steps
                .iter()
                .filter(|(at, _)| at.x == x && at.z == z)
                .map(|(at, shape)| (at.y, *shape))
                .collect()
        };
        // Out from the door: a stair rising to the threshold, each one a cube lower than
        // the last, on a footing down to the cube the ground passes through.
        let rise = Shape::Stair(Facing::North);
        for x in [2, 3] {
            for z in 6..10 {
                let column = in_column(x, z);
                // A stair, or where the ground rises over the last's lower tread a whole cube.
                let buried = i32::from(ground([x, z])) >= (5 - z) * cube_cm() + cube_cm() / 2
                    && (5 - z) * cube_cm() + cube_cm() / 2 - i32::from(ground([x, z + 1]))
                        <= NO_STEP_CM;
                let step = if buried { Shape::Whole } else { rise };
                assert_eq!(column[0], (5 - z, step), "{x} {z}");
                let ys: Vec<i32> = column.iter().map(|&(y, _)| y).collect();
                assert!(ys.windows(2).all(|w| w[1] == w[0] - 1), "{ys:?}");
                assert!(column[1..].iter().all(|&(_, shape)| shape == Shape::Whole));
                assert!(ys[ys.len() - 1] * cube_cm() <= i32::from(ground([x, z])));
            }
        }
        // The treads come down a quarter metre at a time, from the threshold to a step
        // above the ground where they end.
        let mut tread = 0;
        for z in 6.. {
            let Some(&(y, shape)) = in_column(2, z).first() else {
                assert!(tread + 25 - i32::from(ground([2, z])) <= NO_STEP_CM, "{z}");
                break;
            };
            // Near the door, then away from it.
            let tops = [shape.top(0.5, 0.1), shape.top(0.5, 0.9)]
                .map(|t| y * cube_cm() + (t * cube_cm() as f32) as i32);
            if shape == Shape::Whole {
                assert_eq!(tops, [tread, tread], "{z}");
                // The last step: nothing further out.
                assert!(in_column(2, z + 1).is_empty());
                break;
            } else {
                assert_eq!(tops, [tread, tread - 25], "{z}");
            }
            tread -= 50;
        }
        // On level ground a step down is all it needs: a half cube.
        let low = base.door_steps(&BTreeSet::new(), |_| -40);
        assert_eq!(low.len(), 2);
        assert!(
            low.iter()
                .all(|(at, shape)| at.y == -1 && *shape == Shape::Half)
        );
        assert!(base.door_steps(&BTreeSet::new(), |_| -10).is_empty());
        // Laid, they are cubes, and laid again, nothing changes.
        let mut laid = base.clone();
        assert!(laid.lay_door_steps(Kind(2), ground));
        assert!(!laid.lay_door_steps(Kind(2), ground));
        assert_eq!(laid.cube(IVec3::new(2, -1, 6)).map(|c| c.shape), Some(rise));
    }

    #[test]
    fn a_flight_on_a_slope_ends_in_a_step_not_a_wedge_of_stair_over_the_ground() {
        let base = with_door();
        let (mut wholes, mut flights) = (0, 0);
        // Ground falling away from the door at every slope from gentle to steep, from every
        // depth below the threshold a step can start at.
        for fall in (10..=60).step_by(5) {
            for start in (25..=95).step_by(5) {
                let ground = |[_, z]: [i32; 2]| (-start - fall * (z - 6).max(0)) as i16;
                let steps = base.door_steps(&BTreeSet::new(), ground);
                // Each column's step is its highest cube, the rest footings; the last step
                // is the furthest from the door.
                for x in [2, 3] {
                    let Some(&(at, shape)) = steps
                        .iter()
                        .filter(|(at, _)| at.x == x)
                        .max_by_key(|(at, _)| (at.z, at.y))
                    else {
                        continue;
                    };
                    flights += 1;
                    let soil = i32::from(ground([at.x, at.z]));
                    if let Shape::Stair(_) = shape {
                        // Its lower tread, half way up the cube, clear of the ground.
                        assert!(
                            soil < at.y * cube_cm() + cube_cm() / 2,
                            "{fall} {start} {at}"
                        );
                    }
                    wholes += usize::from(shape == Shape::Whole);
                }
            }
        }
        assert!(flights > 200, "{flights}");
        // Some slopes end in a whole cube set into the ground for the last step.
        assert!(wholes > 0);
    }
}
