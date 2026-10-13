//! What laying a part means in operations (D139, D152; docs/building.md, planned then
//! constructed): a room, a wall, a floor or a roof is laid as plans of its cubes, each its
//! own part, where nothing is yet; an opening is planned cubes made open; a door or window
//! is fitted into an opening. The hammer and the remote both lay through these, so they lay
//! the same. The ground under a plan is filled in by whoever acts, from the planet.

use super::props::{DOOR, WINDOW};
use std::collections::BTreeSet;
use terra_build::ops::{Op, Planned};
use terra_build::{
    Base, Fill, Kind, Panel, PartId, Prop, RoofSection, STOREY_CUBES, Shape, floor_layer,
    wall_layer,
};

use bevy::math::{IVec2, IVec3};

/// A plan of these cubes in a part, leaving out any already there; none if all are.
fn plan(
    base: &Base,
    part: PartId,
    kind: Kind,
    cubes: impl IntoIterator<Item = IVec3>,
) -> Option<Op> {
    let mut seen = BTreeSet::new();
    let cubes: Vec<Planned> = cubes
        .into_iter()
        .filter(|&at| base.cube(at).is_none() && base.prop_in(at).is_none())
        .filter(|at| seen.insert(at.to_array()))
        .map(|at| Planned {
            at: at.to_array(),
            shape: Shape::Whole,
        })
        .collect();
    (!cubes.is_empty()).then(|| Op::LayPlan {
        part: Some(part),
        kind,
        cubes,
        ground: Vec::new(),
    })
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
/// its own part, where nothing is yet.
pub fn room(base: &Base, min: IVec3, max: IVec3, walls: Kind, floor: Kind) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let y = min.y;
    let tiles = |x0: i32, x1: i32, z0: i32, z1: i32| {
        (x0..=x1).flat_map(move |x| (z0..=z1).map(move |z| IVec3::new(x, y, z)))
    };
    let mut ops = Vec::new();
    let under = tiles(min.x, max.x, min.z, max.z).map(|tile| tile.with_y(floor_layer(y)));
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

/// A wall along these tiles, a storey high, as one part.
pub fn wall(base: &Base, tiles: &[IVec3], kind: Kind) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    plan(
        base,
        parts.next(),
        kind,
        tiles.iter().copied().flat_map(column),
    )
    .into_iter()
    .collect()
}

/// A floor under a block of tiles on a storey, as one part.
pub fn floor(base: &Base, min: IVec3, max: IVec3, kind: Kind) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let cubes = (min.x..=max.x)
        .flat_map(|x| (min.z..=max.z).map(move |z| IVec3::new(x, floor_layer(min.y), z)));
    plan(base, parts.next(), kind, cubes).into_iter().collect()
}

/// A flat roof over a block of tiles on a storey, as one part.
pub fn flat_roof(base: &Base, min: IVec3, max: IVec3, kind: Kind) -> Vec<Op> {
    floor(base, min + IVec3::Y, max + IVec3::Y, kind)
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

/// An opening planned into a wall: its planned cubes made open. Cubes already open are
/// left as they are; what is built is refused by the operation, with its reason.
pub fn opening(base: &Base, cubes: &[IVec3]) -> Vec<Op> {
    let cubes: Vec<[i32; 3]> = cubes
        .iter()
        .filter(|&&at| base.cube(at).is_none_or(|cube| cube.fill != Fill::Open))
        .map(|at| at.to_array())
        .collect();
    if cubes.is_empty() {
        Vec::new()
    } else {
        vec![Op::Open { cubes }]
    }
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
        let ops = room(&base, IVec3::new(0, 0, 0), IVec3::new(9, 0, 7), LOGS, STONE);
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
        assert!(room(&base, IVec3::new(0, 0, 0), IVec3::new(9, 0, 7), LOGS, STONE).is_empty());
    }

    #[test]
    fn a_door_is_planned_into_a_wall_and_fits_the_hole_it_leaves() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        base.act_all(
            &room(&base, IVec3::new(0, 0, 0), IVec3::new(9, 0, 7), LOGS, LOGS),
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
    fn a_pitched_roof_takes_the_place_of_a_flat_one_planned() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        base.act_all(
            &room(&base, IVec3::new(0, 0, 0), IVec3::new(9, 0, 7), LOGS, LOGS),
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
