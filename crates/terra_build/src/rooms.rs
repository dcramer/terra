//! Rooms and indoors (D155, D261): the space a building's cubes close in all round, roof
//! and floor too, found by flooding the cubes that are not built, in three dimensions, as
//! Minecraft's and Enshrouded's room checks do. Only what is built closes a room in: a plan,
//! or a wall going up, encloses nothing; an opening cut in a wall is part of it still, so a
//! doorway closes a room as its wall does. A room may be any height and shape, two storeys
//! and a mezzanine in one, and a storey is no part of it.
//!
//! A space is closed in when every cube of it has something built over it and under it in
//! its column, and none of it opens onto a cube that has not: so the open sky and the open
//! ground under a building are the outside, and the space under a floor that only footings
//! ring is no room, as nothing built is under it (D194).

use crate::{Base, CELL_M, Cube};
use glam::IVec3;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/// The six neighbours of a cube.
const FACES: [IVec3; 6] = crate::support::FACES;

/// The rooms of a base, worked out when first asked after a change: each cube closed in,
/// and the room it is in.
#[derive(Clone, Default)]
pub(crate) struct Rooms(OnceLock<BTreeMap<[i32; 3], u32>>);

impl PartialEq for Rooms {
    /// Worked out from the cubes, so always the same where the cubes are.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for Rooms {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Rooms")
    }
}

/// Whether a cube closes a room in: anything built, an opening in a wall too, which is
/// built open.
fn encloses(cube: &Cube) -> bool {
    cube.is_built()
}

impl Base {
    /// Every cube closed in and the room it is in.
    fn rooms(&self) -> &BTreeMap<[i32; 3], u32> {
        self.rooms.0.get_or_init(|| self.find_rooms())
    }

    /// How many cubes are in rooms.
    pub fn room_cubes(&self) -> usize {
        self.rooms().len()
    }

    /// How many rooms there are.
    pub fn room_count(&self) -> usize {
        self.rooms()
            .values()
            .copied()
            .collect::<BTreeSet<u32>>()
            .len()
    }

    /// Whether a cube is in a room: closed in all round.
    pub fn contains(&self, at: IVec3) -> bool {
        self.rooms().contains_key(&at.to_array())
    }

    /// Every cube in a room and the room it is in, in a fixed order.
    pub fn room_cells(&self) -> impl Iterator<Item = (IVec3, u32)> + '_ {
        self.rooms()
            .iter()
            .map(|(&at, &room)| (IVec3::from_array(at), room))
    }

    /// Which room a cube is in, if it is in one.
    pub fn room(&self, at: IVec3) -> Option<u32> {
        self.rooms().get(&at.to_array()).copied()
    }

    /// Indoors (D155): every room, as boxes of its cubes from one cube to another, rows east
    /// and west joined south of one another where they reach as far, then layers of the same
    /// box joined up, sorted.
    pub fn indoors(&self) -> Vec<(IVec3, IVec3)> {
        let mut rooms: BTreeMap<u32, Vec<[i32; 3]>> = BTreeMap::new();
        for (&at, &room) in self.rooms() {
            rooms.entry(room).or_default().push(at);
        }
        let mut boxes = Vec::new();
        for mut cubes in rooms.into_values() {
            // Rows east and west, on a layer and a line south.
            cubes.sort_by_key(|&[x, y, z]| (y, z, x));
            let mut rows: Vec<(i32, i32, i32, i32)> = Vec::new();
            for [x, y, z] in cubes {
                match rows.last_mut() {
                    Some((ry, rz, _, last)) if *ry == y && *rz == z && *last + 1 == x => *last = x,
                    _ => rows.push((y, z, x, x)),
                }
            }
            // Rows reaching as far, one south of another, as one rectangle.
            let mut open: BTreeMap<(i32, i32, i32), (i32, i32)> = BTreeMap::new();
            let mut flat: Vec<(i32, [i32; 4])> = Vec::new();
            for (y, z, first, last) in rows {
                match open.get_mut(&(y, first, last)) {
                    Some((_, south)) if *south + 1 == z => *south = z,
                    _ => {
                        if let Some((north, south)) = open.insert((y, first, last), (z, z)) {
                            flat.push((y, [first, last, north, south]));
                        }
                    }
                }
            }
            flat.extend(
                open.into_iter()
                    .map(|((y, first, last), (north, south))| (y, [first, last, north, south])),
            );
            // The same rectangle on layers one over another, as one box.
            flat.sort_by_key(|&(y, rect)| (rect, y));
            let mut stacked: Vec<([i32; 4], i32, i32)> = Vec::new();
            for (y, rect) in flat {
                match stacked.last_mut() {
                    Some((r, _, top)) if *r == rect && *top + 1 == y => *top = y,
                    _ => stacked.push((rect, y, y)),
                }
            }
            boxes.extend(stacked.into_iter().map(|([x0, x1, z0, z1], y0, y1)| {
                (IVec3::new(x0, y0, z0), IVec3::new(x1, y1, z1))
            }));
        }
        boxes.sort_by_key(|(low, high)| (low.to_array(), high.to_array()));
        boxes
    }

    /// The rooms' boxes in metres in the base's own space, low and high corners.
    pub fn indoors_m(&self) -> Vec<(glam::Vec3, glam::Vec3)> {
        self.indoors()
            .into_iter()
            .map(|(low, high)| {
                (
                    low.as_vec3() * CELL_M,
                    (high + IVec3::ONE).as_vec3() * CELL_M,
                )
            })
            .collect()
    }

    /// Works out the rooms: the cubes not built with something built over and under them
    /// in their column, flooded face to face; a flood that reaches a cube with nothing built
    /// over or under it is the outside, and the rest each a room.
    fn find_rooms(&self) -> BTreeMap<[i32; 3], u32> {
        // Each column's layers that close a room in.
        let mut walls: BTreeMap<[i32; 2], BTreeSet<i32>> = BTreeMap::new();
        for (at, cube) in self.cubes() {
            if encloses(&cube) {
                walls.entry([at.x, at.z]).or_default().insert(at.y);
            }
        }
        let closes = |at: IVec3| {
            walls
                .get(&[at.x, at.z])
                .is_some_and(|layers| layers.contains(&at.y))
        };
        // Between the lowest and highest of a column, and not closing in itself.
        let between = |at: IVec3| {
            walls.get(&[at.x, at.z]).is_some_and(|layers| {
                !layers.contains(&at.y)
                    && layers.range(..at.y).next().is_some()
                    && layers.range(at.y + 1..).next().is_some()
            })
        };
        let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut rooms = BTreeMap::new();
        let mut next_room = 0;
        for (&[x, z], layers) in &walls {
            let (Some(&low), Some(&high)) = (layers.first(), layers.last()) else {
                continue;
            };
            for y in low + 1..high {
                let start = IVec3::new(x, y, z);
                if closes(start) || !seen.insert(start.to_array()) {
                    continue;
                }
                let mut space = vec![start.to_array()];
                let mut frontier = vec![start];
                let mut outside = false;
                while let Some(at) = frontier.pop() {
                    for step in FACES {
                        let next = at + step;
                        if closes(next) {
                            continue;
                        }
                        if !between(next) {
                            outside = true;
                            continue;
                        }
                        if seen.insert(next.to_array()) {
                            space.push(next.to_array());
                            frontier.push(next);
                        }
                    }
                }
                if !outside {
                    for at in space {
                        rooms.insert(at, next_room);
                    }
                    next_room += 1;
                }
            }
        }
        rooms
    }
}

#[cfg(test)]
mod tests {
    use crate::roofer::{self, Style};
    use crate::{Base, BuildOp, Cube, Fill, Kind};
    use glam::IVec3;
    use std::collections::BTreeSet;

    const LOGS: Kind = Kind(1);

    fn fill(base: &mut Base, min: [i32; 3], max: [i32; 3], cube: Option<Cube>) {
        base.apply(BuildOp::Fill { min, max, cube });
    }

    fn solid() -> Option<Cube> {
        Some(Cube::new(LOGS, Fill::Solid))
    }

    /// A box of walls a cube thick from one corner to another, its floor and ceiling too:
    /// so many cubes inside it.
    fn shell(base: &mut Base, min: [i32; 3], max: [i32; 3]) -> usize {
        fill(base, min, max, solid());
        let inner = (
            [min[0] + 1, min[1] + 1, min[2] + 1],
            [max[0] - 1, max[1] - 1, max[2] - 1],
        );
        fill(base, inner.0, inner.1, None);
        (0..3)
            .map(|i| (inner.1[i] - inner.0[i] + 1) as usize)
            .product()
    }

    /// A hip roof's cubes over the columns from one to another on the ground storey,
    /// built.
    fn roof(base: &mut Base, min: [i32; 3], max: [i32; 3]) {
        let footprint: BTreeSet<[i32; 2]> = (min[0]..=max[0])
            .flat_map(|x| (min[2]..=max[2]).map(move |z| [x, z]))
            .filter(|&[x, z]| base.occupied(x, z))
            .collect();
        for (at, shape) in roofer::over(&footprint, 6, Style::Hip, None).roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
                }),
            });
        }
    }

    #[test]
    fn a_closed_box_is_one_room_and_a_hole_in_it_none() {
        let mut base = Base::default();
        let inside = shell(&mut base, [0, -1, 0], [5, 6, 4]);
        assert_eq!(base.room_cubes(), inside);
        assert_eq!(base.room_count(), 1);
        assert!(base.contains(IVec3::new(2, 3, 2)));
        // An opening cut in a wall still closes it; a hole knocked through does not.
        fill(
            &mut base,
            [2, 0, 0],
            [2, 3, 0],
            Some(Cube::new(LOGS, Fill::Open)),
        );
        assert_eq!(base.room_cubes(), inside);
        fill(&mut base, [2, 0, 0], [2, 3, 0], None);
        assert_eq!(base.room_cubes(), 0);
        // Nor does a box with its roof off, or its floor: the sky and the ground are out.
        let mut open = Base::default();
        shell(&mut open, [0, -1, 0], [5, 6, 4]);
        fill(&mut open, [1, 6, 1], [4, 6, 3], None);
        assert_eq!(open.room_cubes(), 0, "no roof");
        let mut floorless = Base::default();
        shell(&mut floorless, [0, -1, 0], [5, 6, 4]);
        fill(&mut floorless, [1, -1, 1], [4, -1, 3], None);
        assert_eq!(floorless.room_cubes(), 0, "no floor");
    }

    #[test]
    fn a_knee_high_ring_is_no_room_and_a_wall_off_a_storey_closes_one() {
        let mut base = Base::default();
        // A ring of walls a cube high beside a tall building: nothing over it.
        shell(&mut base, [0, -1, 0], [5, 8, 4]);
        fill(&mut base, [8, 0, 0], [12, 0, 4], solid());
        fill(&mut base, [9, 0, 1], [11, 0, 3], None);
        assert_eq!(base.room_count(), 1);
        assert!(!base.contains(IVec3::new(10, 0, 2)));
        // A box from layer 3 to 8, across what were two storeys, is one room.
        let mut high = Base::default();
        let inside = shell(&mut high, [0, 3, 0], [4, 8, 4]);
        assert_eq!(high.room_cubes(), inside);
        assert_eq!(high.room_count(), 1);
    }

    #[test]
    fn two_storeys_and_a_mezzanine_are_one_room_and_a_shut_floor_makes_two() {
        // An L of two wings, two storeys high, a floor across it with a well cut in it.
        let mut base = Base::default();
        fill(&mut base, [0, -1, 0], [11, 12, 5], solid());
        fill(&mut base, [0, -1, 6], [5, 12, 11], solid());
        fill(&mut base, [1, 0, 1], [10, 11, 4], None);
        fill(&mut base, [1, 0, 1], [4, 11, 10], None);
        fill(&mut base, [1, 5, 1], [10, 5, 4], solid());
        fill(&mut base, [1, 5, 1], [4, 5, 10], solid());
        fill(&mut base, [2, 5, 7], [3, 5, 9], None);
        assert_eq!(base.room_count(), 1, "the well joins the storeys");
        assert!(base.contains(IVec3::new(9, 2, 2)) && base.contains(IVec3::new(2, 8, 9)));
        fill(&mut base, [2, 5, 7], [3, 5, 9], solid());
        assert_eq!(base.room_count(), 2, "the floor shut");
        assert_ne!(
            base.room(IVec3::new(9, 2, 2)),
            base.room(IVec3::new(9, 8, 2))
        );
        // As boxes: every cube of each room once.
        let boxes = base.indoors();
        let covered: usize = boxes
            .iter()
            .map(|(low, high)| ((*high - *low + IVec3::ONE).element_product()) as usize)
            .sum();
        assert_eq!(covered, base.room_cubes());
    }

    #[test]
    fn a_plan_encloses_nothing_until_it_is_built() {
        let mut base = Base::default();
        let inside = shell(&mut base, [0, -1, 0], [5, 6, 4]);
        let planned = Cube::planned(LOGS, None);
        fill(&mut base, [0, 2, 2], [0, 2, 2], Some(planned));
        assert_eq!(base.room_cubes(), 0);
        fill(&mut base, [0, 2, 2], [0, 2, 2], solid());
        assert_eq!(base.room_cubes(), inside);
    }

    #[test]
    fn a_room_under_a_roof_of_cubes_is_indoors() {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [7, 0, 5],
            kind: Kind::PLAIN,
        });
        // Its flat roof taken off: open to the sky.
        base.apply(BuildOp::Fill {
            min: [1, crate::floor_layer(1), 1],
            max: [6, crate::floor_layer(1), 4],
            cube: None,
        });
        assert!(base.indoors().is_empty(), "no roof");
        roof(&mut base, [0, 0, 0], [7, 0, 5]);
        assert!(!base.indoors().is_empty());
        assert!(base.contains(IVec3::new(3, 2, 2)));
    }
}
