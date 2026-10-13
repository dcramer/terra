//! Rooms and indoors (D155): on each storey, the tiles that wall tiles close in on every
//! side, grouped by which reach each other. A wall tile is a column with cubes in it
//! below its top layer.

use crate::{Base, LEVEL, Side};
use glam::{IVec2, IVec3};
use std::collections::{BTreeMap, BTreeSet};

impl Base {
    /// How many tiles are in rooms.
    pub fn tile_count(&self) -> usize {
        self.rooms.len()
    }

    /// Whether a tile is in a room: closed in by walls on its storey.
    pub fn contains(&self, tile: IVec3) -> bool {
        self.rooms.contains_key(&tile.to_array())
    }

    /// Every tile in a room and the room it is in, in a fixed order.
    pub fn tiles(&self) -> impl Iterator<Item = (IVec3, u32)> + '_ {
        self.rooms
            .iter()
            .map(|(&tile, &room)| (IVec3::from_array(tile), room))
    }

    /// Which room a tile is in, if it is in one.
    pub fn room(&self, tile: IVec3) -> Option<u32> {
        self.rooms.get(&tile.to_array()).copied()
    }

    /// Whether a wall tile is an outside wall: a room on one side of it and open air on
    /// another.
    pub fn exterior(&self, tile: IVec3) -> bool {
        self.outward(tile).is_some()
    }

    /// The way out through an outside wall tile, from its room toward the open. `None` if
    /// it is not an outside wall.
    pub fn outward(&self, tile: IVec3) -> Option<IVec3> {
        self.filling(tile)?;
        let open = |at: IVec3| !self.contains(at) && self.filling(at).is_none();
        LEVEL
            .iter()
            .map(|side| side.offset())
            .find(|&out| self.contains(tile - out) && open(tile + out))
    }

    /// Indoors (D155): each room closed all round and covered all over, by a floor over
    /// it or a roof up, as boxes of its tiles, from one tile to another, each storey's
    /// tiles in rows east and west, rows of the same reach south of one another joined.
    pub fn indoors(&self) -> Vec<(IVec3, IVec3)> {
        let mut rooms: BTreeMap<u32, BTreeSet<[i32; 3]>> = BTreeMap::new();
        for (&tile, &room) in &self.rooms {
            rooms.entry(room).or_default().insert(tile);
        }
        let roofed = |tile: IVec3| {
            self.has_roof(tile)
                || self
                    .roofs()
                    .any(|(_, roof)| roof.standing() && roof.section.covers(tile))
        };
        let mut boxes = Vec::new();
        for tiles in rooms.values() {
            if !tiles.iter().all(|&tile| roofed(IVec3::from_array(tile))) {
                continue;
            }
            // Rows east and west: on a storey and a line south, from one tile to another.
            let mut rows: Vec<(i32, i32, i32, i32)> = Vec::new();
            let mut ordered: Vec<[i32; 3]> = tiles.iter().copied().collect();
            ordered.sort_by_key(|&[x, y, z]| (y, z, x));
            for [x, y, z] in ordered {
                match rows.last_mut() {
                    Some((ry, rz, _, last)) if *ry == y && *rz == z && *last + 1 == x => *last = x,
                    _ => rows.push((y, z, x, x)),
                }
            }
            // Rows reaching as far, one south of another, as one box.
            let mut open: BTreeMap<(i32, i32, i32), (i32, i32)> = BTreeMap::new();
            for (y, z, first, last) in rows {
                match open.get_mut(&(y, first, last)) {
                    Some((_, south)) if *south + 1 == z => *south = z,
                    _ => {
                        if let Some((north, south)) = open.insert((y, first, last), (z, z)) {
                            boxes.push((IVec3::new(first, y, north), IVec3::new(last, y, south)));
                        }
                    }
                }
            }
            boxes.extend(open.into_iter().map(|((y, first, last), (north, south))| {
                (IVec3::new(first, y, north), IVec3::new(last, y, south))
            }));
        }
        boxes.sort_by_key(|(low, high)| (low.to_array(), high.to_array()));
        boxes
    }

    /// Works out the rooms: on each storey, the tiles that cannot be reached from beyond
    /// the walls without passing through one, grouped by which reach each other. Only
    /// walls built whole close a room in; a plan, or a wall going up, does not.
    pub(crate) fn find_rooms(&mut self) {
        let mut rooms = BTreeMap::new();
        let mut storeys: BTreeMap<i32, (IVec2, IVec2)> = BTreeMap::new();
        for &[x, y, z] in &self.enclosing {
            let at = IVec2::new(x, z);
            storeys
                .entry(y)
                .and_modify(|(lo, hi)| {
                    *lo = lo.min(at);
                    *hi = hi.max(at);
                })
                .or_insert((at, at));
        }
        let mut next_room = 0;
        for (y, (low, high)) in storeys {
            // One tile wider all round, so the open air surrounds everything.
            let (low, high) = (low - IVec2::ONE, high + IVec2::ONE);
            let within = |cell: IVec2| cell.cmpge(low).all() && cell.cmple(high).all();
            let open = |cell: IVec2| !self.enclosing.contains(&[cell.x, y, cell.y]);
            let step = |cell: IVec2, side: Side| {
                let offset = side.offset();
                cell + IVec2::new(offset.x, offset.z)
            };
            let mut reached = BTreeSet::new();
            let mut frontier: Vec<IVec2> = Vec::new();
            for x in low.x..=high.x {
                for z in low.y..=high.y {
                    let cell = IVec2::new(x, z);
                    if x == low.x || x == high.x || z == low.y || z == high.y {
                        reached.insert(cell.to_array());
                        frontier.push(cell);
                    }
                }
            }
            while let Some(cell) = frontier.pop() {
                for side in LEVEL {
                    let next = step(cell, side);
                    if within(next) && open(next) && reached.insert(next.to_array()) {
                        frontier.push(next);
                    }
                }
            }
            // What the open air never reached, and is not wall, is closed in: each group
            // of it a room. Below the ground storey, only where it has a floor: the space
            // a building's footings close in under its floor is no room (D194).
            let floored = |cell: IVec2| y >= 0 || self.has_floor(IVec3::new(cell.x, y, cell.y));
            for x in low.x..=high.x {
                for z in low.y..=high.y {
                    let start = IVec2::new(x, z);
                    if !open(start) || !floored(start) || !reached.insert(start.to_array()) {
                        continue;
                    }
                    let mut frontier = vec![start];
                    while let Some(cell) = frontier.pop() {
                        rooms.insert([cell.x, y, cell.y], next_room);
                        for side in LEVEL {
                            let next = step(cell, side);
                            if open(next) && floored(next) && reached.insert(next.to_array()) {
                                frontier.push(next);
                            }
                        }
                    }
                    next_room += 1;
                }
            }
        }
        self.rooms = rooms;
    }
}

#[cfg(test)]
mod tests {
    use crate::{Base, BuildOp, Kind, RoofSection, RoofStyle};
    use glam::IVec3;

    fn room(base: &mut Base, min: [i32; 3], max: [i32; 3]) {
        base.apply(BuildOp::AddRoom {
            min,
            max,
            kind: Kind::PLAIN,
        });
    }

    fn roof(base: &mut Base, min: [i32; 3], max: [i32; 3]) {
        base.apply(BuildOp::AddRoof(RoofSection {
            min,
            max,
            style: RoofStyle::Hip,
            along_x: true,
            pitch: 0.7,
        }));
    }

    /// How many tiles a set of boxes covers, each once.
    fn tiles(boxes: &[(IVec3, IVec3)]) -> usize {
        boxes
            .iter()
            .map(|(low, high)| ((high.x - low.x + 1) * (high.z - low.z + 1)) as usize)
            .sum()
    }

    #[test]
    fn a_room_is_indoors_once_it_is_roofed_and_an_l_is_two_boxes() {
        let mut base = Base::default();
        room(&mut base, [0, 0, 0], [7, 0, 5]);
        // Its flat roof taken off.
        base.apply(BuildOp::Fill {
            min: [1, crate::floor_layer(1), 1],
            max: [6, crate::floor_layer(1), 4],
            cube: None,
        });
        assert!(base.indoors().is_empty(), "no roof");
        roof(&mut base, [0, 0, 0], [7, 0, 5]);
        // Inside its walls, one tile in all round.
        assert_eq!(
            base.indoors(),
            vec![(IVec3::new(1, 0, 1), IVec3::new(6, 0, 4))]
        );
        let mut ell = Base::default();
        room(&mut ell, [0, 0, 0], [9, 0, 5]);
        room(&mut ell, [0, 0, 5], [5, 0, 11]);
        roof(&mut ell, [0, 0, 0], [9, 0, 11]);
        let boxes = ell.indoors();
        assert_eq!(boxes.len(), 2, "{boxes:?}");
        assert_eq!(tiles(&boxes), ell.tile_count());
    }
}
