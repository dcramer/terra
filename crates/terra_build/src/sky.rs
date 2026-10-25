//! How much of the open sky's light reaches each room (D486): what comes in through its
//! openings, doorways and windows (`Fill::Open`), as a room-and-portal engine carries light
//! from the outside in. An opening onto the open air brings in as much as the air beyond it
//! sees of the sky, which the caller says (the ground round it, a well's sides); one onto
//! another room brings that room's share, dimmed by the doorway; a room with no way out has
//! none, lit only by its own fires. Shipped games bake this per room (Unreal's sky light
//! occlusion in its volumetric lightmaps, id Tech's and Source's area portals carrying an
//! area's ambient); worked out from the cubes, it holds for a house, a cellar and a dungeon
//! alike.

use crate::Base;
use glam::IVec3;
use std::collections::BTreeMap;

/// How much of a room's light a doorway between two rooms passes to the room beyond: half, a
/// room seeing another through a door seeing about half of what that room sees round it.
pub const THROUGH_A_DOORWAY: f32 = 0.5;

/// How thick a wall's opening may be, in cubes, before what is beyond it is no longer looked
/// for.
const OPENING_DEEP: i32 = 4;

/// The six neighbours of a cube.
const FACES: [IVec3; 6] = crate::support::FACES;

impl Base {
    /// Each room's share of the open sky's light, from 0 (none comes in) to 1 (as a room
    /// with a door or window onto the open air): the sum of what its openings onto the
    /// outside see of the sky, `seen` of the cube of air just beyond each, up to all of it;
    /// or what its doorways into other rooms pass on, whichever is more.
    pub fn room_sky(&self, seen: impl Fn(IVec3) -> f32) -> BTreeMap<u32, f32> {
        let mut direct: BTreeMap<u32, f32> = BTreeMap::new();
        let mut doorways: Vec<(u32, u32)> = Vec::new();
        for (at, room) in self.room_cells() {
            direct.entry(room).or_insert(0.0);
            for step in FACES {
                match self.beyond_opening(at, step) {
                    Beyond::Nothing => {}
                    Beyond::Room(other) if other != room => doorways.push((room, other)),
                    Beyond::Room(_) => {}
                    Beyond::Outside(air) => *direct.entry(room).or_insert(0.0) += seen(air),
                }
            }
        }
        let mut share: BTreeMap<u32, f32> = direct
            .into_iter()
            .map(|(room, sky)| (room, sky.clamp(0.0, 1.0)))
            .collect();
        // Light passed on room to room, each pass at least a doorway's step, until it settles.
        for _ in 0..share.len() {
            let mut changed = false;
            for &(room, other) in &doorways {
                let passed = share.get(&other).copied().unwrap_or(0.0) * THROUGH_A_DOORWAY;
                let mine = share.entry(room).or_insert(0.0);
                if passed > *mine + 1e-6 {
                    *mine = passed;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        share
    }

    /// What a room's cube looks out on through an opening beside it, going one way: nothing
    /// if no opening is there or a solid cube closes it, else the room or the outside air
    /// past the opening.
    fn beyond_opening(&self, at: IVec3, step: IVec3) -> Beyond {
        let is_opening = |cube: Option<crate::Cube>| {
            cube.is_some_and(|c| c.is_built() && c.fill == crate::Fill::Open)
        };
        if !is_opening(self.cube(at + step)) {
            return Beyond::Nothing;
        }
        let mut past = at + step * 2;
        for _ in 1..OPENING_DEEP {
            if !is_opening(self.cube(past)) {
                break;
            }
            past += step;
        }
        if self.cube(past).is_some_and(|c| c.is_built()) {
            return Beyond::Nothing;
        }
        match self.room(past) {
            Some(room) => Beyond::Room(room),
            None => Beyond::Outside(past),
        }
    }
}

enum Beyond {
    Nothing,
    Room(u32),
    Outside(IVec3),
}

#[cfg(test)]
mod tests {
    use super::THROUGH_A_DOORWAY;
    use crate::{Base, BuildOp, Cube, Fill, Kind};
    use glam::IVec3;

    const STONE: Kind = Kind(2);

    fn fill(base: &mut Base, min: [i32; 3], max: [i32; 3], cube: Option<Cube>) {
        base.apply(BuildOp::Fill { min, max, cube });
    }

    fn opening() -> Option<Cube> {
        Some(Cube::new(STONE, Fill::Open))
    }

    /// Two rooms side by side, west and east, shut from each other by a solid wall, each
    /// a box of walls a cube thick, 3 cubes inside each way.
    fn two_rooms() -> Base {
        let mut base = Base::default();
        fill(
            &mut base,
            [0, 0, 0],
            [8, 4, 4],
            Some(Cube::new(STONE, Fill::Solid)),
        );
        fill(&mut base, [1, 1, 1], [3, 3, 3], None);
        fill(&mut base, [5, 1, 1], [7, 3, 3], None);
        base
    }

    fn share(base: &Base, at: [i32; 3], seen: f32) -> f32 {
        let room = base.room(IVec3::from_array(at)).expect("a room");
        base.room_sky(|_| seen)[&room]
    }

    #[test]
    fn a_room_shut_all_round_has_no_sky_and_a_window_onto_the_open_brings_it_all() {
        let mut base = two_rooms();
        assert_eq!(base.room_count(), 2);
        assert_eq!(share(&base, [2, 2, 2], 1.0), 0.0);
        // A window in the west room's west wall, onto the open air.
        fill(&mut base, [0, 2, 2], [0, 2, 2], opening());
        assert_eq!(share(&base, [2, 2, 2], 1.0), 1.0);
        // The east room, still shut off, has none.
        assert_eq!(share(&base, [6, 2, 2], 1.0), 0.0);
        // Onto air that sees a fifth of the sky (a well's foot), a fifth.
        assert!((share(&base, [2, 2, 2], 0.2) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn a_doorway_passes_on_part_of_a_rooms_light_and_more_rooms_in_less() {
        let mut base = two_rooms();
        fill(&mut base, [0, 1, 2], [0, 2, 2], opening());
        // A doorway in the wall between them.
        fill(&mut base, [4, 1, 2], [4, 2, 2], opening());
        assert_eq!(base.room_count(), 2);
        assert_eq!(share(&base, [2, 2, 2], 1.0), 1.0);
        assert!((share(&base, [6, 2, 2], 1.0) - THROUGH_A_DOORWAY).abs() < 1e-6);
        // Air beyond that sees no sky (dug ground under the earth) brings none either way.
        assert_eq!(share(&base, [6, 2, 2], 0.0), 0.0);
    }
}
