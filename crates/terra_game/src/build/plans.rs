//! What laying a part means in operations (D139, D152; docs/building.md, planned then
//! constructed): a room, a wall, a floor or a roof is laid as plans of its cubes, each its
//! own part, where nothing is yet; on the ground, with footings under it down to the ground
//! as a part of their own, laid first so they are built first (D194); an opening is planned
//! cubes made open, or built cubes marked to be carved out; a door or window is fitted into
//! an opening. The hammer and the remote both lay through these, so they lay the same. The
//! ground under a plan is filled in by whoever acts, from the planet.

use super::props::{DOOR, WINDOW};
use std::collections::BTreeSet;
use terra_build::ground::{cube_cm, footing as footing_cubes};
use terra_build::ops::{Op, Planned};
use terra_build::{
    Base, Facing, Fill, Kind, Panel, PartId, Pending, Prop, RoofSection, STOREY_CUBES, Shape,
    floor_layer, storey_of, wall_layer,
};

use bevy::math::{IVec2, IVec3};

/// A plan of these cubes in a part, leaving out any already there; none if all are.
fn plan(
    base: &Base,
    part: PartId,
    kind: Kind,
    cubes: impl IntoIterator<Item = IVec3>,
) -> Option<Op> {
    shaped(
        base,
        part,
        kind,
        cubes.into_iter().map(|at| (at, Shape::Whole)),
    )
}

/// A plan of these cubes, each cut to its shape (D193), in a part, leaving out any already
/// there; none if all are.
fn shaped(
    base: &Base,
    part: PartId,
    kind: Kind,
    cubes: impl IntoIterator<Item = (IVec3, Shape)>,
) -> Option<Op> {
    let mut seen = BTreeSet::new();
    let cubes: Vec<Planned> = cubes
        .into_iter()
        .filter(|&(at, _)| base.cube(at).is_none() && base.prop_in(at).is_none())
        .filter(|(at, _)| seen.insert(at.to_array()))
        .map(|(at, shape)| Planned {
            at: at.to_array(),
            shape,
        })
        .collect();
    (!cubes.is_empty()).then(|| Op::LayPlan {
        part: Some(part),
        kind,
        cubes,
        ground: Vec::new(),
    })
}

/// Footings under what is laid on the ground (D194): their kind, and the ground's height
/// under a column, in centimetres in the base's own space.
#[derive(Clone, Copy)]
pub struct Footings<'a> {
    pub kind: Kind,
    pub ground_cm: &'a dyn Fn([i32; 2]) -> i16,
}

impl Footings<'_> {
    /// A footing's plan under columns, each from its top layer down to the ground.
    fn plan(
        &self,
        base: &Base,
        part: PartId,
        columns: impl IntoIterator<Item = ([i32; 2], i32)>,
    ) -> Option<Op> {
        plan(
            base,
            part,
            self.kind,
            footing_cubes(base, columns, self.ground_cm),
        )
    }
}

/// Numbers parts from the base's next, in the order they are laid.
struct Numbers(u32);

impl Numbers {
    fn of(base: &Base) -> Self {
        Numbers(base.next_part().0)
    }

    fn next(&mut self) -> PartId {
        self.0 += 1;
        PartId(self.0 - 1)
    }
}

/// A wall tile's cubes, a storey high, its top included.
fn column(tile: IVec3) -> impl Iterator<Item = IVec3> {
    let foot = wall_layer(tile.y);
    (foot..foot + STOREY_CUBES).map(move |y| IVec3::new(tile.x, y, tile.z))
}

/// The room shortcut (D152) over a block of tiles, its corners both included, on a storey:
/// a floor under all of it, a wall along each side, and a flat roof over the inside, each
/// its own part, where nothing is yet. On the ground storey with footings, a footing under
/// its walls from the floor's layer down to the ground comes first, and the floor is laid
/// inside them.
pub fn room(
    base: &Base,
    min: IVec3,
    max: IVec3,
    walls: Kind,
    floor: Kind,
    footings: Option<Footings>,
) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let y = min.y;
    let tiles = |x0: i32, x1: i32, z0: i32, z1: i32| {
        (x0..=x1).flat_map(move |x| (z0..=z1).map(move |z| IVec3::new(x, y, z)))
    };
    let mut ops = Vec::new();
    let footings = footings.filter(|_| y == 0);
    let edge =
        |tile: IVec3| tile.x == min.x || tile.x == max.x || tile.z == min.z || tile.z == max.z;
    if let Some(footings) = footings {
        let columns = tiles(min.x, max.x, min.z, max.z)
            .filter(|&tile| edge(tile))
            .map(|tile| ([tile.x, tile.z], floor_layer(y)));
        ops.extend(footings.plan(base, parts.next(), columns));
    }
    let under = tiles(min.x, max.x, min.z, max.z)
        .filter(|&tile| footings.is_none() || !edge(tile))
        .map(|tile| tile.with_y(floor_layer(y)));
    ops.extend(plan(base, parts.next(), floor, under));
    // North and south run the whole width; east and west between them.
    let sides = [
        tiles(min.x, max.x, min.z, min.z).collect::<Vec<_>>(),
        tiles(min.x, max.x, max.z, max.z).collect(),
        tiles(min.x, min.x, min.z + 1, max.z - 1).collect(),
        tiles(max.x, max.x, min.z + 1, max.z - 1).collect(),
    ];
    for side in sides {
        let cubes = side.into_iter().flat_map(column);
        ops.extend(plan(base, parts.next(), walls, cubes));
    }
    let over = tiles(min.x + 1, max.x - 1, min.z + 1, max.z - 1)
        .map(|tile| tile.with_y(floor_layer(y + 1)));
    ops.extend(plan(base, parts.next(), walls, over));
    ops
}

/// A wall along these tiles, a storey high, as one part; on the ground storey with
/// footings, a footing under it from the floor's layer down to the ground first.
pub fn wall(base: &Base, tiles: &[IVec3], kind: Kind, footings: Option<Footings>) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let mut ops = Vec::new();
    if let Some(footings) = footings {
        let columns = tiles
            .iter()
            .filter(|tile| tile.y == 0)
            .map(|tile| ([tile.x, tile.z], floor_layer(0)));
        ops.extend(footings.plan(base, parts.next(), columns));
    }
    ops.extend(plan(
        base,
        parts.next(),
        kind,
        tiles.iter().copied().flat_map(column),
    ));
    ops
}

/// A floor under a block of tiles on a storey, as one part; on the ground storey with
/// footings, a footing under its edges down to the ground first, where the ground is below
/// it.
pub fn floor(
    base: &Base,
    min: IVec3,
    max: IVec3,
    kind: Kind,
    footings: Option<Footings>,
) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let layer = floor_layer(min.y);
    let mut ops = Vec::new();
    if let Some(footings) = footings.filter(|_| min.y == 0) {
        let edge = |x: i32, z: i32| x == min.x || x == max.x || z == min.z || z == max.z;
        let columns = (min.x..=max.x)
            .flat_map(|x| (min.z..=max.z).map(move |z| [x, z]))
            .filter(|&[x, z]| edge(x, z))
            .filter(|&column| layer * cube_cm() > i32::from((footings.ground_cm)(column)))
            .map(|column| (column, layer - 1));
        ops.extend(footings.plan(base, parts.next(), columns));
    }
    let cubes = (min.x..=max.x).flat_map(|x| (min.z..=max.z).map(move |z| IVec3::new(x, layer, z)));
    ops.extend(plan(base, parts.next(), kind, cubes));
    ops
}

/// The layer of cubes the floor of the storey over a cube is: a flight climbs to it, a post
/// carries it. A cube in a storey's floor layer, as one set into ground lower than the
/// base's floor is, climbs to the floor over that storey.
fn floor_over(cube: IVec3) -> i32 {
    floor_layer(storey_of(cube.y + 1) + 1)
}

/// A flight of stairs or a ramp (D193), as one part, from its foot at `foot`, the first
/// cube over the floor or ground it starts on, rising toward `facing` a cube a column up to
/// the floor of the storey above, `wide` columns across: each column whole cubes under a
/// stair or a slope. On the ground storey with footings, a footing under it first.
pub fn flight(
    base: &Base,
    foot: IVec3,
    facing: Facing,
    wide: i32,
    top: Shape,
    kind: Kind,
    footings: Option<Footings>,
) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let rise = (floor_over(foot) - foot.y + 1).max(1);
    let (forward, across) = (facing.step(), facing.turned(1).step());
    let columns: Vec<IVec3> = (0..rise)
        .flat_map(|i| (0..wide.max(1)).map(move |w| foot + forward * i + across * w + IVec3::Y * i))
        .collect();
    let mut ops = Vec::new();
    if let Some(footings) = footings.filter(|_| storey_of(foot.y) == 0) {
        let under = columns.iter().map(|top| ([top.x, top.z], foot.y - 1));
        ops.extend(footings.plan(base, parts.next(), under));
    }
    let cubes = columns.iter().flat_map(|&top_cube| {
        (foot.y..=top_cube.y).map(move |y| {
            let at = top_cube.with_y(y);
            (at, if y == top_cube.y { top } else { Shape::Whole })
        })
    });
    ops.extend(shaped(base, parts.next(), kind, cubes));
    ops
}

/// A post (D193), as one part: a column of post cubes from `foot` up to the floor of the
/// storey above, which it carries. On the ground storey with footings, a footing under it.
pub fn post(base: &Base, foot: IVec3, kind: Kind, footings: Option<Footings>) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let mut ops = Vec::new();
    if let Some(footings) = footings.filter(|_| storey_of(foot.y) == 0) {
        ops.extend(footings.plan(base, parts.next(), [([foot.x, foot.z], foot.y - 1)]));
    }
    let cubes = (foot.y..=floor_over(foot)).map(|y| (foot.with_y(y), Shape::Post));
    ops.extend(shaped(base, parts.next(), kind, cubes));
    ops
}

/// One cube of a shape, placed as Enshrouded's blocks are: in the part of a planned cube of
/// its kind beside it, so a run of them is built as one; else a part of its own.
pub fn block(base: &Base, at: IVec3, shape: Shape, kind: Kind) -> Vec<Op> {
    let part = terra_build::support::FACES
        .iter()
        .filter_map(|&step| base.cube(at + step))
        .find(|cube| cube.kind == kind && !cube.is_built() && cube.part.is_some())
        .and_then(|cube| cube.part)
        .unwrap_or_else(|| Numbers::of(base).next());
    shaped(base, part, kind, [(at, shape)])
        .into_iter()
        .collect()
}

/// A flat roof over a block of tiles on a storey, as one part.
pub fn flat_roof(base: &Base, min: IVec3, max: IVec3, kind: Kind) -> Vec<Op> {
    floor(base, min + IVec3::Y, max + IVec3::Y, kind, None)
}

/// A pitched roof planned over a block of tiles, in a kind; a flat roof planned there, with
/// nothing in it, gives way to it.
pub fn roof(base: &Base, section: RoofSection, kind: Kind) -> Vec<Op> {
    let top = floor_layer(section.storey() + 1);
    let flat: Vec<[i32; 3]> = (section.min[0]..=section.max[0])
        .flat_map(|x| (section.min[2]..=section.max[2]).map(move |z| IVec3::new(x, top, z)))
        .filter(|&at| {
            !base.in_wall(at)
                && base
                    .cube(at)
                    .is_some_and(|cube| cube.level() == 0 && cube.fill == Fill::Solid)
        })
        .map(|at| at.to_array())
        .collect();
    let mut ops = Vec::new();
    if !flat.is_empty() {
        ops.push(Op::Unplan { cubes: flat });
    }
    ops.push(Op::AddRoof {
        roof: base.next_roof(),
        section,
        kind: Some(kind),
    });
    ops
}

/// The cubes of an opening of a panel's shape cut along a stretch of wall tiles: a door's
/// from the floor, a window's from its sill to its head.
pub fn opening_cubes(tiles: &[IVec3], panel: Panel) -> Vec<IVec3> {
    tiles
        .iter()
        .flat_map(|&tile| {
            let foot = wall_layer(tile.y);
            (0..STOREY_CUBES)
                .filter(move |&step| panel.fill(step) != Fill::Solid)
                .map(move |step| IVec3::new(tile.x, foot + step, tile.z))
        })
        .collect()
}

/// An opening into a wall: its planned cubes with nothing in them made open, and those with
/// something in marked to be carved out, which building then takes out a course at a time
/// from under the wall over it and makes open (construction-001, experience 4). Cubes
/// already open or marked are left as they are; what cannot be is refused by the
/// operation, with its reason.
pub fn opening(base: &Base, cubes: &[IVec3]) -> Vec<Op> {
    let (mut open, mut carve) = (Vec::new(), Vec::new());
    for &at in cubes {
        match base.cube(at) {
            Some(cube) if cube.fill == Fill::Open || cube.pending == Pending::Carve => {}
            Some(cube) if cube.level() > 0 => carve.push(at.to_array()),
            _ => open.push(at.to_array()),
        }
    }
    let mut ops = Vec::new();
    if !open.is_empty() {
        ops.push(Op::Open { cubes: open });
    }
    if !carve.is_empty() {
        ops.push(Op::Carve { cubes: carve });
    }
    ops
}

/// The opening a cube is part of: every open cube joined to it, side by side and up and
/// down, as the block from its lowest to its highest; `None` if it is not open.
pub fn hole(base: &Base, cube: IVec3) -> Option<(IVec3, IVec3)> {
    let open = |at: IVec3| base.cube(at).is_some_and(|cube| cube.fill == Fill::Open);
    if !open(cube) {
        return None;
    }
    let mut seen = BTreeSet::from([cube.to_array()]);
    let mut frontier = vec![cube];
    let (mut low, mut high) = (cube, cube);
    while let Some(at) = frontier.pop() {
        low = low.min(at);
        high = high.max(at);
        for step in terra_build::support::FACES {
            let next = at + step;
            if open(next) && seen.insert(next.to_array()) {
                frontier.push(next);
            }
        }
    }
    Some((low, high))
}

/// A door, if the hole reaches the floor, else a window, to fit an opening, unless asked
/// for one or the other.
pub fn prop_for(base: &Base, cube: IVec3, door: Option<bool>) -> Option<Prop> {
    let (min, max) = hole(base, cube)?;
    // A hole from the foot of its storey's walls is a doorway.
    let is_door = door.unwrap_or(min.y == wall_layer(terra_build::storey_of(min.y)));
    Some(Prop {
        kind: if is_door { DOOR } else { WINDOW },
        min: min.to_array(),
        max: max.to_array(),
        open: false,
    })
}

/// The tiles along a wall from one to another, in the grid's four directions only (D146):
/// east and west, or north and south, whichever is further. Both ends included.
pub fn wall_tiles(from: IVec2, to: IVec2, level: i32) -> Vec<IVec3> {
    let step = to - from;
    let (along, length) = if step.x.abs() >= step.y.abs() {
        (IVec2::new(step.x.signum(), 0), step.x.abs())
    } else {
        (IVec2::new(0, step.y.signum()), step.y.abs())
    };
    (0..=length)
        .map(|i| {
            let at = from + along * i;
            IVec3::new(at.x, level, at.y)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_build::ops::Context;
    use terra_build::{Rules, State};

    const LOGS: Kind = Kind(1);
    const STONE: Kind = Kind(2);

    #[test]
    fn a_room_is_a_floor_four_walls_and_a_roof_each_its_own_part() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        let ops = room(
            &base,
            IVec3::new(0, 0, 0),
            IVec3::new(9, 0, 7),
            LOGS,
            STONE,
            None,
        );
        assert_eq!(ops.len(), 6);
        base.act_all(&ops, &cx).unwrap();
        assert_eq!(base.parts().count(), 6);
        // A floor of 80 cubes, walls round the edge six high, a roof over the inside.
        assert_eq!(base.progress(PartId(0)).cubes, 80);
        let walls: usize = (1..5).map(|part| base.progress(PartId(part)).cubes).sum();
        assert_eq!(walls, 32 * 6);
        assert_eq!(base.progress(PartId(5)).cubes, 48);
        assert_eq!(base.part(PartId(0)).unwrap().kind, STONE);
        // Nothing of it is built, so it closes no room in.
        assert_eq!(base.tile_count(), 0);
        assert!(
            base.cubes()
                .all(|(_, cube)| cube.state == State::Planned { level: 0 })
        );
        // Laid again over itself, it lays nothing.
        assert!(
            room(
                &base,
                IVec3::new(0, 0, 0),
                IVec3::new(9, 0, 7),
                LOGS,
                STONE,
                None
            )
            .is_empty()
        );
    }

    #[test]
    fn a_room_on_a_slope_stands_on_a_footing_laid_first_down_to_the_ground() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        // The ground falls away to the east, 20 cm a column, from the floor's level.
        let ground = |column: [i32; 2]| (-20 * column[0]) as i16;
        let footings = Footings {
            kind: STONE,
            ground_cm: &ground,
        };
        let (min, max) = (IVec3::new(0, 0, 0), IVec3::new(9, 0, 7));
        let ops = room(&base, min, max, LOGS, LOGS, Some(footings));
        assert_eq!(ops.len(), 7, "a footing, a floor, four walls and a roof");
        base.act_all(&ops, &cx).unwrap();
        // The footing is the first part, in stone, under every column of the walls.
        let footing: Vec<IVec3> = base.cubes_of(PartId(0)).map(|(at, _)| at).collect();
        assert_eq!(base.part(PartId(0)).unwrap().kind, STONE);
        let columns: BTreeSet<[i32; 2]> = footing.iter().map(|at| [at.x, at.z]).collect();
        assert_eq!(columns.len(), 32);
        for column in columns {
            let ys: Vec<i32> = footing
                .iter()
                .filter(|at| [at.x, at.z] == column)
                .map(|at| at.y)
                .collect();
            let lowest = *ys.iter().min().unwrap();
            assert_eq!(*ys.iter().max().unwrap(), -1, "from the floor's layer");
            // Down to the cube the ground passes through, and no further.
            let ground = i32::from(ground(column));
            assert!(lowest * cube_cm() <= ground, "{column:?}");
            assert!(
                lowest == -1 || (lowest + 1) * cube_cm() > ground,
                "{column:?}"
            );
        }
        // The floor is inside the walls only, over nothing on the low side.
        assert_eq!(base.progress(PartId(1)).cubes, 48);
        // Laid again, nothing.
        assert!(room(&base, min, max, LOGS, LOGS, Some(footings)).is_empty());
        let upstairs = room(
            &base,
            min + IVec3::Y,
            max + IVec3::Y,
            LOGS,
            LOGS,
            Some(footings),
        );
        // Higher up, no footing, and its floor the flat roof below: four walls and a roof.
        assert_eq!(upstairs.len(), 5);
    }

    #[test]
    fn a_door_is_planned_into_a_wall_and_fits_the_hole_it_leaves() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        base.act_all(
            &room(
                &base,
                IVec3::new(0, 0, 0),
                IVec3::new(9, 0, 7),
                LOGS,
                LOGS,
                None,
            ),
            &cx,
        )
        .unwrap();
        let tiles = [IVec3::new(4, 0, 7), IVec3::new(5, 0, 7)];
        let cubes = opening_cubes(&tiles, Panel::Door);
        assert_eq!(cubes.len(), 8);
        base.act_all(&opening(&base, &cubes), &cx).unwrap();
        let prop = prop_for(&base, IVec3::new(5, 2, 7), None).unwrap();
        assert_eq!(prop.kind, DOOR);
        assert_eq!((prop.min, prop.max), ([4, 0, 7], [5, 3, 7]));
        // A window is cut from its sill to its head.
        let window = opening_cubes(&[IVec3::new(0, 0, 3)], Panel::WINDOW);
        assert_eq!(window.iter().map(|at| at.y).collect::<Vec<_>>(), vec![2, 3]);
        base.act_all(&opening(&base, &window), &cx).unwrap();
        assert_eq!(
            prop_for(&base, IVec3::new(0, 2, 3), None).unwrap().kind,
            WINDOW
        );
    }

    #[test]
    fn a_flight_climbs_a_storey_to_the_floor_above_and_a_post_carries_it() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        let ground = |_: [i32; 2]| 0i16;
        let footings = Footings {
            kind: STONE,
            ground_cm: &ground,
        };
        let ops = flight(
            &base,
            IVec3::new(0, 0, 8),
            Facing::North,
            2,
            Shape::Stair(Facing::North),
            LOGS,
            Some(footings),
        );
        assert_eq!(ops.len(), 2, "a footing and the flight");
        base.act_all(&ops, &cx).unwrap();
        // Six columns, two across, the last at the floor of the storey above.
        let tops: Vec<(IVec3, Shape)> = base
            .cubes()
            .filter(|(_, cube)| cube.kind == LOGS && cube.shape != Shape::Whole)
            .map(|(at, cube)| (at, cube.shape))
            .collect();
        assert_eq!(tops.len(), 12);
        assert!(
            tops.iter()
                .all(|(_, shape)| *shape == Shape::Stair(Facing::North))
        );
        let highest = tops.iter().map(|(at, _)| at.y).max().unwrap();
        assert_eq!(highest, floor_layer(1));
        assert_eq!(tops.iter().map(|(at, _)| at.z).min(), Some(8 - 5));
        // A post a storey high, to the floor over it, in a part of its own.
        let post_ops = post(&base, IVec3::new(4, 0, 0), LOGS, None);
        base.act_all(&post_ops, &cx).unwrap();
        let column: Vec<i32> = base
            .cubes()
            .filter(|(_, cube)| cube.shape == Shape::Post)
            .map(|(at, _)| at.y)
            .collect();
        assert_eq!(column, (0..=floor_layer(1)).collect::<Vec<_>>());
        // A block beside a planned cube joins its part; one alone is a part of its own.
        let parts = base.parts().count();
        base.act_all(&block(&base, IVec3::new(5, 0, 0), Shape::Half, LOGS), &cx)
            .unwrap();
        assert_eq!(base.parts().count(), parts);
        base.act_all(&block(&base, IVec3::new(9, 0, 0), Shape::Half, LOGS), &cx)
            .unwrap();
        assert_eq!(base.parts().count(), parts + 1);
    }

    #[test]
    fn a_pitched_roof_takes_the_place_of_a_flat_one_planned() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        base.act_all(
            &room(
                &base,
                IVec3::new(0, 0, 0),
                IVec3::new(9, 0, 7),
                LOGS,
                LOGS,
                None,
            ),
            &cx,
        )
        .unwrap();
        let section = RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style: terra_build::RoofStyle::Gable,
            along_x: true,
            pitch: 0.6,
        };
        base.act_all(&roof(&base, section, LOGS), &cx).unwrap();
        assert_eq!(base.part(PartId(5)), None, "the flat roof's plan is gone");
        assert_eq!(base.roofs().count(), 1);
    }
}
