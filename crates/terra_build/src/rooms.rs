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
//!
//! After an edit only the rooms its cubes touch are flooded again (D308): every room the
//! changed cubes or their columns border is let go, and the space round them flooded anew;
//! the rest are kept as they were, so an edit to a castle costs what its rooms near it
//! cost, not the castle.

use crate::mesh::QuickMap;
use crate::{Base, CELL_M, Cube};
use glam::IVec3;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

/// The six neighbours of a cube.
const FACES: [IVec3; 6] = crate::support::FACES;

/// How many cubes may change closing in between two looks at the rooms before they are
/// flooded again from nothing: past this, letting go every room they touch is no cheaper.
const CHANGES_KEPT: usize = 1 << 16;

/// The rooms of a base, worked out when first asked after a change; and, until then, the
/// rooms last worked out and every cube whose closing in has changed since, to work them
/// out again from rather than from nothing.
#[derive(Clone, Default)]
pub(crate) struct Rooms {
    found: OnceLock<Arc<Found>>,
    last: Option<Arc<Found>>,
    changed: Vec<IVec3>,
}

/// Rooms found: each cube closed in and the room it is in, each room's cubes and boxes,
/// and the next room's number.
#[derive(Debug, Default)]
struct Found {
    rooms: BTreeMap<[i32; 3], u32>,
    cubes: BTreeMap<u32, Vec<[i32; 3]>>,
    boxes: BTreeMap<u32, Vec<(IVec3, IVec3)>>,
    next: u32,
}

impl Found {
    /// A room of these cubes, under the next number.
    fn add(&mut self, cubes: Vec<[i32; 3]>) {
        let room = self.next;
        self.next += 1;
        for &at in &cubes {
            self.rooms.insert(at, room);
        }
        self.boxes.insert(room, boxes_of(cubes.clone()));
        self.cubes.insert(room, cubes);
    }
}

impl Rooms {
    /// A cube's closing in has changed: the rooms found are let go, kept to work the next
    /// ones out from.
    pub(crate) fn changed(&mut self, at: IVec3) {
        if let Some(found) = self.found.take() {
            self.last = Some(found);
            self.changed.clear();
        }
        if self.last.is_some() {
            self.changed.push(at);
            if self.changed.len() > CHANGES_KEPT {
                self.last = None;
                self.changed.clear();
            }
        }
    }
}

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
pub(crate) fn encloses(cube: &Cube) -> bool {
    cube.is_built()
}

/// A room's cubes as boxes from one cube to another: rows east and west joined south of
/// one another where they reach as far, then layers of the same box joined up.
fn boxes_of(mut cubes: Vec<[i32; 3]>) -> Vec<(IVec3, IVec3)> {
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
    stacked
        .into_iter()
        .map(|([x0, x1, z0, z1], y0, y1)| (IVec3::new(x0, y0, z0), IVec3::new(x1, y1, z1)))
        .collect()
}

impl Base {
    /// The rooms found: from the last ones and what changed since where there are both,
    /// else from nothing.
    fn found(&self) -> &Found {
        self.rooms.found.get_or_init(|| {
            let again = self
                .rooms
                .last
                .as_ref()
                .and_then(|last| self.find_again(last, &self.rooms.changed));
            Arc::new(again.unwrap_or_else(|| self.find_rooms()))
        })
    }

    /// Every cube closed in and the room it is in.
    fn rooms(&self) -> &BTreeMap<[i32; 3], u32> {
        &self.found().rooms
    }

    /// How many cubes are in rooms.
    pub fn room_cubes(&self) -> usize {
        self.rooms().len()
    }

    /// How many rooms there are.
    pub fn room_count(&self) -> usize {
        self.found().cubes.len()
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
        let mut boxes: Vec<(IVec3, IVec3)> =
            self.found().boxes.values().flatten().copied().collect();
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

    /// Whether a cube closes a room in, and the lowest and highest layers of a column that
    /// do, each asked of the base once.
    fn closing(&self) -> Closing<'_> {
        Closing {
            base: self,
            columns: Default::default(),
        }
    }

    /// Works out the rooms: the cubes not built with something built over and under them
    /// in their column, flooded face to face; a flood that reaches a cube with nothing built
    /// over or under it is the outside, and the rest each a room.
    fn find_rooms(&self) -> Found {
        let closing = self.closing();
        let mut seen: Seen = Default::default();
        let mut found = Found::default();
        for &[x, z] in self.columns.keys() {
            let Some((low, high)) = closing.bounds([x, z]) else {
                continue;
            };
            for y in low + 1..high {
                let start = IVec3::new(x, y, z);
                if let Some(space) = closing.flood(start, &mut seen, &|_| false) {
                    found.add(space);
                }
            }
        }
        found
    }

    /// Works out the rooms again after the cubes `changed` have changed closing in since
    /// `last` was found: every room they or the cubes beside them are in is let go and its
    /// space flooded again, the rest kept. `None` if a flood meets a room
    /// kept, which only flooding from nothing can then settle.
    fn find_again(&self, last: &Found, changed: &[IVec3]) -> Option<Found> {
        let closing = self.closing();
        // What may have changed: the space beside each changed cube. A change far up or
        // down a column moves what is between its lowest and highest built only for the
        // space running up or down to it, so beside it too.
        let seeds: Vec<IVec3> = changed
            .iter()
            .flat_map(|&at| std::iter::once(at).chain(FACES.iter().map(move |&step| at + step)))
            .collect();
        let gone: BTreeSet<u32> = seeds
            .iter()
            .filter_map(|at| last.rooms.get(&at.to_array()).copied())
            .collect();
        let mut found = Found {
            rooms: last.rooms.clone(),
            cubes: last.cubes.clone(),
            boxes: last.boxes.clone(),
            next: last.next,
        };
        for room in &gone {
            for at in found.cubes.remove(room).unwrap_or_default() {
                found.rooms.remove(&at);
            }
            found.boxes.remove(room);
        }
        let kept = |at: IVec3| found.rooms.contains_key(&at.to_array());
        let mut seen: Seen = Default::default();
        let mut spaces = Vec::new();
        let mut met_kept = false;
        for &start in &seeds {
            if kept(start) {
                continue;
            }
            match closing.flood_checked(start, &mut seen, &kept) {
                Flooded::Room(space) => spaces.push(space),
                Flooded::Kept => {
                    met_kept = true;
                    break;
                }
                Flooded::Nothing => {}
            }
        }
        if met_kept {
            return None;
        }
        for space in spaces {
            found.add(space);
        }
        Some(found)
    }
}

/// The cubes flooded so far, and whether each is the outside: a flood stops when it meets
/// the outside, so the cubes it got to are the outside too, which a later flood reaching
/// them learns at once.
type Seen = QuickMap<IVec3, bool>;

/// What flooding from a cube came to: a room of these cubes, a room kept already, or
/// nothing (the outside, or no space).
enum Flooded {
    Room(Vec<[i32; 3]>),
    Kept,
    Nothing,
}

/// A column's lowest and highest layers that close a room in, if any do.
type Bounds = Option<(i32, i32)>;

/// A base's cubes asked whether they close a room in, and its columns' lowest and highest
/// cubes that do, each column asked once.
struct Closing<'a> {
    base: &'a Base,
    columns: std::cell::RefCell<QuickMap<[i32; 2], Bounds>>,
}

impl Closing<'_> {
    fn closes(&self, at: IVec3) -> bool {
        self.base.cube(at).is_some_and(|cube| encloses(&cube))
    }

    /// The lowest and highest layers of a column that close a room in, if any do.
    fn bounds(&self, column: [i32; 2]) -> Option<(i32, i32)> {
        if let Some(&known) = self.columns.borrow().get(&column) {
            return known;
        }
        let layers = self.base.columns.get(&column);
        let closing = |y: &&i32| self.closes(IVec3::new(column[0], **y, column[1]));
        let low = layers.and_then(|layers| layers.iter().find(closing).copied());
        let high = layers.and_then(|layers| layers.iter().rev().find(closing).copied());
        let bounds = low.zip(high);
        self.columns.borrow_mut().insert(column, bounds);
        bounds
    }

    /// Between the lowest and highest of its column that close a room in, and not closing
    /// one in itself.
    fn between(&self, at: IVec3) -> bool {
        self.bounds([at.x, at.z])
            .is_some_and(|(low, high)| low < at.y && at.y < high && !self.closes(at))
    }

    /// The space flooded from a cube face to face, if it is a room: `None` for a cube that
    /// closes one in, one seen already, or a space that reaches the outside.
    fn flood(
        &self,
        start: IVec3,
        seen: &mut Seen,
        kept: &dyn Fn(IVec3) -> bool,
    ) -> Option<Vec<[i32; 3]>> {
        match self.flood_checked(start, seen, kept) {
            Flooded::Room(space) => Some(space),
            _ => None,
        }
    }

    /// As [`Closing::flood`], saying too whether it met a room kept; a flood stops at the
    /// outside, as the rest of its space is the outside too.
    fn flood_checked(
        &self,
        start: IVec3,
        seen: &mut Seen,
        kept: &dyn Fn(IVec3) -> bool,
    ) -> Flooded {
        if !self.between(start) || seen.contains_key(&start) {
            return Flooded::Nothing;
        }
        seen.insert(start, false);
        let mut space = vec![start.to_array()];
        let mut frontier = vec![start];
        let outside = |space: &[[i32; 3]], seen: &mut Seen| {
            for &at in space {
                seen.insert(IVec3::from_array(at), true);
            }
            Flooded::Nothing
        };
        while let Some(at) = frontier.pop() {
            for step in FACES {
                let next = at + step;
                if self.closes(next) {
                    continue;
                }
                if !self.between(next) {
                    return outside(&space, seen);
                }
                if kept(next) {
                    return Flooded::Kept;
                }
                match seen.get(&next) {
                    Some(true) => return outside(&space, seen),
                    Some(false) => {}
                    None => {
                        seen.insert(next, false);
                        space.push(next.to_array());
                        frontier.push(next);
                    }
                }
            }
        }
        Flooded::Room(space)
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

    /// Each cube in a room, named by the least cube of its room: the rooms as a partition,
    /// whatever they are numbered.
    fn partition(found: &super::Found) -> Vec<([i32; 3], [i32; 3])> {
        let least: std::collections::BTreeMap<u32, [i32; 3]> = found
            .cubes
            .iter()
            .map(|(&room, cubes)| (room, *cubes.iter().min().unwrap()))
            .collect();
        found
            .rooms
            .iter()
            .map(|(&at, room)| (at, least[room]))
            .collect()
    }

    #[test]
    fn rooms_found_again_after_edits_are_the_rooms_found_from_nothing() {
        // Two rooms side by side under one flat roof, a third over them.
        let mut base = Base::default();
        shell(&mut base, [0, -1, 0], [6, 6, 5]);
        shell(&mut base, [6, -1, 0], [12, 6, 5]);
        shell(&mut base, [0, 6, 0], [12, 11, 5]);
        let _ = base.room_cubes();
        // Cubes knocked out and put back at random through the walls, floors and roofs, and
        // inside: holes open rooms to the outside and to each other, and close them again.
        let mut random = crate::layout::Random(7);
        let (mut again, mut asked, mut changes) = (0, 0, 0);
        for step in 0..400 {
            let at = [
                (random.unit() * 14.0) as i32 - 1,
                (random.unit() * 14.0) as i32 - 2,
                (random.unit() * 8.0) as i32 - 1,
            ];
            let cube = if random.unit() < 0.5 { None } else { solid() };
            fill(&mut base, at, at, cube);
            if step % 3 != 0 {
                continue;
            }
            changes += base.rooms.changed.len();
            if let Some(last) = base.rooms.last.clone() {
                let from_nothing = base.find_rooms();
                if let Some(found) = base.find_again(&last, &base.rooms.changed) {
                    assert_eq!(partition(&found), partition(&from_nothing), "step {step}");
                    again += 1;
                }
                asked += 1;
            }
            let _ = base.room_cubes();
        }
        assert!(changes > 50, "{changes} changes");
        assert!(asked > 50 && again * 10 > asked * 9, "{again} of {asked}");
    }
}
