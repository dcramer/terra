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

    /// Works out the rooms: on each storey, the tiles that cannot be reached from beyond
    /// the walls without passing through one, grouped by which reach each other.
    pub(crate) fn find_rooms(&mut self) {
        let mut rooms = BTreeMap::new();
        let mut storeys: BTreeMap<i32, (IVec2, IVec2)> = BTreeMap::new();
        for &[x, y, z] in self.walls.keys() {
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
            let open = |cell: IVec2| !self.walls.contains_key(&[cell.x, y, cell.y]);
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
            // of it a room.
            for x in low.x..=high.x {
                for z in low.y..=high.y {
                    let start = IVec2::new(x, z);
                    if !open(start) || !reached.insert(start.to_array()) {
                        continue;
                    }
                    let mut frontier = vec![start];
                    while let Some(cell) = frontier.pop() {
                        rooms.insert([cell.x, y, cell.y], next_room);
                        for side in LEVEL {
                            let next = step(cell, side);
                            if open(next) && reached.insert(next.to_array()) {
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
