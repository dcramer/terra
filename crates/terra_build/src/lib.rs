//! Construction: bases you shape instead of assemble.
//!
//! An experiment inspired by Subnautica 2's builder (research/subnautica-2-building.md).
//! There are no room modules. A base is the set of tiles that are inside it, each
//! remembering which room it was added as. The player grows and shrinks a room by pushing
//! and pulling one of its walls, and everything else follows from the shape:
//!
//! - **The hull is generated.** Wherever inside meets outside there is a wall, floor or
//!   roof. Nobody places one.
//! - **Rooms that touch open into each other**, with no connector piece.
//! - **Windows and doors are a property of a wall tile**, not separate parts. A window is
//!   described by its edges, so it can be any size, and windows on neighbouring tiles
//!   with no gap between them read as one long window.
//!
//! Everything a player does is a [`BuildOp`], so a base can be sent to other players and
//! saved as the list of operations that made it (D21).
//!
//! Coordinates are local to the base: tiles on an integer grid, X and Z along the floor
//! and Y up. [`TILE_M`] and [`STOREY_M`] turn tiles into metres.

mod mesh;
mod walk;

pub use mesh::{BaseMesh, Surface};
pub use walk::Walker;

use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// Width and depth of one tile, in metres.
pub const TILE_M: f32 = 3.0;
/// Height of one storey, in metres.
pub const STOREY_M: f32 = 3.0;
/// Width of a door's opening, in metres.
pub const DOOR_WIDTH_M: f32 = 1.3;
/// Height of a door's opening, in metres.
pub const DOOR_HEIGHT_M: f32 = 2.2;

/// One of the six directions a tile faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    East,
    West,
    South,
    North,
    Up,
    Down,
}

impl Side {
    pub const ALL: [Side; 6] = [
        Side::East,
        Side::West,
        Side::South,
        Side::North,
        Side::Up,
        Side::Down,
    ];

    /// The neighbouring tile in this direction.
    pub fn offset(self) -> IVec3 {
        match self {
            Side::East => IVec3::X,
            Side::West => IVec3::NEG_X,
            Side::South => IVec3::Z,
            Side::North => IVec3::NEG_Z,
            Side::Up => IVec3::Y,
            Side::Down => IVec3::NEG_Y,
        }
    }

    /// Whether this is a wall, as opposed to a floor or roof.
    pub fn is_wall(self) -> bool {
        !matches!(self, Side::Up | Side::Down)
    }

    /// The two directions that run along a surface facing this way.
    fn across(self) -> [IVec3; 2] {
        match self {
            Side::East | Side::West => [IVec3::Z, IVec3::Y],
            Side::South | Side::North => [IVec3::X, IVec3::Y],
            Side::Up | Side::Down => [IVec3::X, IVec3::Z],
        }
    }
}

/// One side of one tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Face {
    pub tile: [i32; 3],
    pub side: Side,
}

impl Face {
    pub fn new(tile: IVec3, side: Side) -> Self {
        Face {
            tile: tile.to_array(),
            side,
        }
    }

    pub fn tile(self) -> IVec3 {
        IVec3::from_array(self.tile)
    }
}

/// What fills a wall tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Panel {
    Solid,
    /// An opening with glass. Each edge is a distance in metres from the matching edge of
    /// the tile, so the window can be any size. A side inset of zero runs the glass to
    /// the tile's edge, where it meets a neighbouring window.
    Window {
        left: f32,
        right: f32,
        sill: f32,
        top: f32,
    },
    /// An opening to walk through.
    Door,
}

impl Panel {
    /// A window with a frame all round.
    pub const WINDOW: Panel = Panel::Window {
        left: 0.45,
        right: 0.45,
        sill: 0.9,
        top: 0.5,
    };
    /// A window that runs the full width of its tile, to join its neighbours.
    pub const WIDE_WINDOW: Panel = Panel::Window {
        left: 0.0,
        right: 0.0,
        sill: 0.9,
        top: 0.5,
    };
    /// Glass from just above the floor to just under the roof, full width.
    pub const TALL_WINDOW: Panel = Panel::Window {
        left: 0.0,
        right: 0.0,
        sill: 0.2,
        top: 0.2,
    };

    /// The next kind in the order a player cycles through.
    pub fn next(self) -> Panel {
        match self {
            Panel::Solid => Panel::WINDOW,
            p if p == Panel::WINDOW => Panel::WIDE_WINDOW,
            p if p == Panel::WIDE_WINDOW => Panel::TALL_WINDOW,
            Panel::Window { .. } => Panel::Door,
            Panel::Door => Panel::Solid,
        }
    }
}

/// Something a player did to a base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuildOp {
    /// Add a block of tiles: a new room, or more of an existing one.
    AddRoom { min: [i32; 3], max: [i32; 3] },
    /// Push the whole wall containing this face outward by one tile.
    Grow(Face),
    /// Pull the whole wall containing this face inward by one tile.
    Shrink(Face),
    /// Change what fills a wall tile.
    SetPanel(Face, Panel),
    /// Stand an interior wall on the edge between two tiles that are both inside, or
    /// take it away again.
    SetPartition(Face, Option<Panel>),
}

/// A base: the tiles inside it, which room each belongs to, and what fills each wall
/// tile that is not solid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Base {
    /// Each tile inside the base, and the room it was added as.
    tiles: BTreeMap<[i32; 3], u32>,
    panels: BTreeMap<Face, Panel>,
    /// Interior walls, each on the east or south side of a tile.
    partitions: BTreeMap<Face, Panel>,
    rooms_added: u32,
}

/// Where a ray met a base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildHit {
    pub face: Face,
    /// Distance along the ray, in metres.
    pub distance: f32,
    /// Whether what was met is an interior wall, not the hull.
    pub partition: bool,
}

impl Base {
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn contains(&self, tile: IVec3) -> bool {
        self.tiles.contains_key(&tile.to_array())
    }

    /// Every tile inside the base and the room it belongs to, in a fixed order.
    pub fn tiles(&self) -> impl Iterator<Item = (IVec3, u32)> + '_ {
        self.tiles
            .iter()
            .map(|(&tile, &room)| (IVec3::from_array(tile), room))
    }

    /// Which room a tile belongs to, if it is inside the base.
    pub fn room(&self, tile: IVec3) -> Option<u32> {
        self.tiles.get(&tile.to_array()).copied()
    }

    /// Applies an operation. Returns whether it changed anything.
    pub fn apply(&mut self, op: BuildOp) -> bool {
        let changed = match op {
            BuildOp::AddRoom { min, max } => {
                self.add_room(IVec3::from_array(min), IVec3::from_array(max))
            }
            BuildOp::Grow(face) => self.grow(face),
            BuildOp::Shrink(face) => self.shrink(face),
            BuildOp::SetPanel(face, panel) => self.set_panel(face, panel),
            BuildOp::SetPartition(face, panel) => self.set_partition(face, panel),
        };
        // A window on a wall that no longer exists means nothing.
        let stale: Vec<Face> = self
            .panels
            .keys()
            .copied()
            .filter(|&face| !self.is_hull(face))
            .collect();
        for face in stale {
            self.panels.remove(&face);
        }
        // An interior wall with outdoors on one side is the hull's job.
        let stale: Vec<Face> = self
            .partitions
            .keys()
            .copied()
            .filter(|&face| !self.can_partition(face))
            .collect();
        for face in stale {
            self.partitions.remove(&face);
        }
        changed
    }

    fn add_room(&mut self, min: IVec3, max: IVec3) -> bool {
        let before = self.tiles.len();
        // A block that overlaps a room is more of that room: the two become one shape,
        // with walls that move together. A block that overlaps nothing is a new room.
        let overlapped = self
            .tiles
            .iter()
            .filter(|(tile, _)| (0..3).all(|axis| (min[axis]..=max[axis]).contains(&tile[axis])))
            .map(|(_, &room)| room)
            .min();
        let room = overlapped.unwrap_or(self.rooms_added);
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    // A tile already inside the base stays with the room that has it.
                    self.tiles.entry([x, y, z]).or_insert(room);
                }
            }
        }
        let changed = self.tiles.len() != before;
        if changed && overlapped.is_none() {
            self.rooms_added += 1;
        }
        changed
    }

    /// The same edge between two tiles, named from whichever tile has it on its east or
    /// south side.
    fn edge(face: Face) -> Face {
        match face.side {
            Side::West => Face::new(face.tile() + IVec3::NEG_X, Side::East),
            Side::North => Face::new(face.tile() + IVec3::NEG_Z, Side::South),
            _ => face,
        }
    }

    /// Whether an interior wall could stand on a face: an upright one with a tile of the
    /// base on both sides.
    pub fn can_partition(&self, face: Face) -> bool {
        face.side.is_wall()
            && self.contains(face.tile())
            && self.contains(face.tile() + face.side.offset())
    }

    /// The interior wall on a face, if there is one, seen from either side.
    pub fn partition(&self, face: Face) -> Option<Panel> {
        self.partitions.get(&Self::edge(face)).copied()
    }

    /// Every interior wall, each named from the tile on its west or north, in a fixed
    /// order.
    pub fn partitions(&self) -> impl Iterator<Item = (Face, Panel)> + '_ {
        self.partitions.iter().map(|(&face, &panel)| (face, panel))
    }

    fn set_partition(&mut self, face: Face, panel: Option<Panel>) -> bool {
        if !self.can_partition(face) || self.partition(face) == panel {
            return false;
        }
        match panel {
            Some(panel) => self.partitions.insert(Self::edge(face), panel),
            None => self.partitions.remove(&Self::edge(face)),
        };
        true
    }

    /// Whether a face is part of the hull: inside on one side, outside on the other.
    pub fn is_hull(&self, face: Face) -> bool {
        self.contains(face.tile()) && !self.contains(face.tile() + face.side.offset())
    }

    /// Every face of the hull, in a fixed order.
    pub fn hull(&self) -> impl Iterator<Item = Face> + '_ {
        self.tiles
            .keys()
            .flat_map(|&tile| Side::ALL.map(|side| Face { tile, side }))
            .filter(|&face| self.is_hull(face))
    }

    /// What fills a hull face. Floors and roofs are always solid.
    pub fn panel(&self, face: Face) -> Panel {
        self.panels.get(&face).copied().unwrap_or(Panel::Solid)
    }

    fn set_panel(&mut self, face: Face, panel: Panel) -> bool {
        if !face.side.is_wall() || !self.is_hull(face) || self.panel(face) == panel {
            return false;
        }
        if panel == Panel::Solid {
            self.panels.remove(&face);
        } else {
            self.panels.insert(face, panel);
        }
        true
    }

    /// The wall a face belongs to: every hull face of the same room, in the same plane
    /// and facing the same way, reachable from it without leaving the plane. Pushing or
    /// pulling a wall therefore resizes one room and leaves its neighbours alone.
    pub fn wall(&self, face: Face) -> Vec<Face> {
        if !self.is_hull(face) {
            return Vec::new();
        }
        let room = self.room(face.tile());
        let mut found = BTreeSet::from([face]);
        let mut frontier = vec![face];
        while let Some(current) = frontier.pop() {
            for step in face.side.across() {
                for direction in [step, -step] {
                    let next = Face::new(current.tile() + direction, face.side);
                    if self.is_hull(next) && self.room(next.tile()) == room && found.insert(next) {
                        frontier.push(next);
                    }
                }
            }
        }
        found.into_iter().collect()
    }

    fn grow(&mut self, face: Face) -> bool {
        let wall = self.wall(face);
        let Some(room) = self.room(face.tile()) else {
            return false;
        };
        for part in &wall {
            self.tiles
                .insert((part.tile() + face.side.offset()).to_array(), room);
        }
        !wall.is_empty()
    }

    fn shrink(&mut self, face: Face) -> bool {
        let wall = self.wall(face);
        // Never shrink a base away to nothing.
        if wall.is_empty() || wall.len() >= self.tiles.len() {
            return false;
        }
        for part in &wall {
            self.tiles.remove(&part.tile);
        }
        true
    }

    /// The first hull face or interior wall a ray meets, from either side, if any. `origin` and
    /// `direction` are in the base's own space, in metres; `direction` is unit length.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<BuildHit> {
        let size = Vec3::new(TILE_M, STOREY_M, TILE_M);
        let hull = self.hull().map(|face| (face, false));
        let interior = self.partitions.keys().map(|&face| (face, true));
        hull.chain(interior)
            .filter_map(|(face, partition)| {
                let normal = face.side.offset().as_vec3();
                let low = face.tile().as_vec3() * size;
                // A point on the face's plane: the tile's far corner on positive sides.
                let on_plane = low + normal.max(Vec3::ZERO) * size;
                let approach = direction.dot(normal);
                if approach.abs() < 1e-6 {
                    return None;
                }
                let distance = (on_plane - origin).dot(normal) / approach;
                if distance <= 0.0 || distance > max_distance {
                    return None;
                }
                let point = origin + direction * distance;
                let inside = (0..3).all(|axis| {
                    normal[axis] != 0.0
                        || (point[axis] >= low[axis] && point[axis] <= low[axis] + size[axis])
                });
                inside.then_some(BuildHit {
                    face,
                    distance,
                    partition,
                })
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    /// The four corners of a face, in the base's own space, in metres.
    pub fn corners(face: Face) -> [Vec3; 4] {
        mesh::corners(face)
    }

    /// The base as triangles, ready to draw.
    pub fn mesh(&self) -> BaseMesh {
        mesh::build(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room() -> Base {
        let mut base = Base::default();
        assert!(base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [2, 0, 2]
        }));
        base
    }

    fn east_wall(base: &Base) -> Face {
        base.hull().find(|face| face.side == Side::East).unwrap()
    }

    #[test]
    fn the_hull_is_wherever_inside_meets_outside() {
        let base = room();
        assert_eq!(base.tile_count(), 9);
        // A 3 by 3 room: 9 floor, 9 roof, 4 walls of 3.
        assert_eq!(base.hull().count(), 9 + 9 + 12);
        assert_eq!(base.hull().filter(|face| face.side.is_wall()).count(), 12);
        // Nothing between two tiles of the same room.
        assert!(!base.is_hull(Face::new(IVec3::new(0, 0, 0), Side::East)));
    }

    #[test]
    fn growing_pushes_a_whole_wall_out_and_shrinking_pulls_it_back() {
        let mut base = room();
        let original = base.clone();
        let wall = east_wall(&base);
        assert_eq!(base.wall(wall).len(), 3);

        assert!(base.apply(BuildOp::Grow(wall)));
        assert_eq!(base.tile_count(), 12);
        let moved = east_wall(&base);
        assert_eq!(moved.tile[0], 3);

        assert!(base.apply(BuildOp::Shrink(moved)));
        assert_eq!(base, original);
    }

    #[test]
    fn rooms_that_touch_become_one_space() {
        let mut base = room();
        base.apply(BuildOp::AddRoom {
            min: [3, 0, 1],
            max: [5, 0, 1],
        });
        // The corridor's tile shares a side with the room, so there is no wall between.
        assert!(!base.is_hull(Face::new(IVec3::new(2, 0, 1), Side::East)));
        // The room's east wall is now two separate stretches, either side of the corridor.
        assert_eq!(
            base.wall(Face::new(IVec3::new(2, 0, 0), Side::East)).len(),
            1
        );
        assert_eq!(
            base.wall(Face::new(IVec3::new(2, 0, 2), Side::East)).len(),
            1
        );
    }

    #[test]
    fn pushing_a_wall_resizes_one_room_and_leaves_its_neighbour_alone() {
        // Two rooms side by side whose south walls line up.
        let mut base = room();
        base.apply(BuildOp::AddRoom {
            min: [3, 0, 0],
            max: [5, 0, 2],
        });
        assert_eq!(base.room(IVec3::new(0, 0, 0)), Some(0));
        assert_eq!(base.room(IVec3::new(3, 0, 0)), Some(1));

        let south = Face::new(IVec3::new(1, 0, 2), Side::South);
        assert_eq!(
            base.wall(south).len(),
            3,
            "only the first room's part of the line"
        );
        base.apply(BuildOp::Grow(south));
        assert!(base.contains(IVec3::new(1, 0, 3)));
        assert!(
            !base.contains(IVec3::new(4, 0, 3)),
            "the neighbour did not grow"
        );
        assert_eq!(
            base.room(IVec3::new(1, 0, 3)),
            Some(0),
            "new tiles join the room that grew"
        );
    }

    #[test]
    fn a_base_cannot_be_shrunk_to_nothing() {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [0, 0, 0],
        });
        let wall = east_wall(&base);
        assert!(!base.apply(BuildOp::Shrink(wall)));
        assert_eq!(base.tile_count(), 1);
    }

    #[test]
    fn windows_belong_to_wall_tiles_and_vanish_with_them() {
        let mut base = room();
        let wall = east_wall(&base);
        assert!(base.apply(BuildOp::SetPanel(wall, Panel::WINDOW)));
        assert_eq!(base.panel(wall), Panel::WINDOW);
        // Setting it again changes nothing; floors cannot have windows.
        assert!(!base.apply(BuildOp::SetPanel(wall, Panel::WINDOW)));
        assert!(!base.apply(BuildOp::SetPanel(
            Face::new(IVec3::ZERO, Side::Down),
            Panel::WINDOW
        )));

        // Growing the wall moves the hull, so the old window's tile is no longer a wall.
        base.apply(BuildOp::Grow(wall));
        assert_eq!(base.panel(wall), Panel::Solid);
        assert!(base.hull().all(|face| base.panel(face) == Panel::Solid));
    }

    #[test]
    fn cycling_a_panel_comes_back_round() {
        let mut panel = Panel::Solid;
        let mut seen = vec![panel];
        for _ in 0..5 {
            panel = panel.next();
            seen.push(panel);
        }
        assert_eq!(seen.first(), seen.last());
        assert_eq!(seen[1], Panel::WINDOW);
        assert_eq!(seen[4], Panel::Door);
    }

    #[test]
    fn a_ray_finds_the_wall_it_points_at_from_either_side() {
        let base = room();
        // From outside, east of the room, looking west at the middle tile.
        let from_outside = base
            .raycast(Vec3::new(20.0, 1.5, 4.5), Vec3::NEG_X, 100.0)
            .unwrap();
        assert_eq!(
            from_outside.face,
            Face::new(IVec3::new(2, 0, 1), Side::East)
        );
        assert!((from_outside.distance - 11.0).abs() < 1e-4);
        // From inside the room, looking east at the same wall.
        let from_inside = base
            .raycast(Vec3::new(4.5, 1.5, 4.5), Vec3::X, 100.0)
            .unwrap();
        assert_eq!(from_inside.face, from_outside.face);
        // Looking away from the base finds nothing.
        assert!(
            base.raycast(Vec3::new(20.0, 1.5, 4.5), Vec3::X, 100.0)
                .is_none()
        );
    }

    #[test]
    fn the_same_operations_build_the_same_base() {
        let ops = [
            BuildOp::AddRoom {
                min: [0, 0, 0],
                max: [2, 0, 2],
            },
            BuildOp::Grow(Face::new(IVec3::new(2, 0, 1), Side::East)),
            BuildOp::SetPanel(
                Face::new(IVec3::new(3, 0, 1), Side::East),
                Panel::WIDE_WINDOW,
            ),
            BuildOp::AddRoom {
                min: [0, 1, 0],
                max: [1, 1, 1],
            },
        ];
        let build = || {
            let mut base = Base::default();
            for op in ops {
                base.apply(op);
            }
            base
        };
        assert_eq!(build(), build());
        assert_eq!(build().mesh(), build().mesh());
    }

    #[test]
    fn a_room_built_across_another_becomes_part_of_it() {
        let mut base = room();
        let before = base.room(IVec3::new(0, 0, 0));
        // Half over the first room, half beyond its east wall.
        assert!(base.apply(BuildOp::AddRoom {
            min: [2, 0, 1],
            max: [4, 0, 3],
        }));
        assert_eq!(base.room(IVec3::new(4, 0, 3)), before);
        // One room, so its whole north wall moves together where it is in one line.
        let north = base.wall(Face::new(IVec3::new(0, 0, 0), Side::North));
        assert_eq!(north.len(), 3);
        // A block wholly inside changes nothing, and a block apart is its own room.
        assert!(!base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [1, 0, 1],
        }));
        assert!(base.apply(BuildOp::AddRoom {
            min: [8, 0, 0],
            max: [9, 0, 1],
        }));
        assert_ne!(base.room(IVec3::new(8, 0, 0)), before);
    }

    #[test]
    fn interior_walls_stand_only_between_two_tiles_of_the_base() {
        let mut base = room();
        let inside = Face::new(IVec3::new(0, 0, 1), Side::East);
        let same_from_the_other_side = Face::new(IVec3::new(1, 0, 1), Side::West);
        let on_the_hull = Face::new(IVec3::new(2, 0, 1), Side::East);
        assert!(!base.apply(BuildOp::SetPartition(on_the_hull, Some(Panel::Solid))));
        assert!(base.apply(BuildOp::SetPartition(inside, Some(Panel::Solid))));
        assert_eq!(base.partition(same_from_the_other_side), Some(Panel::Solid));
        assert_eq!(base.partitions().count(), 1);
        // The same wall again changes nothing; a doorway in it does.
        assert!(!base.apply(BuildOp::SetPartition(
            same_from_the_other_side,
            Some(Panel::Solid)
        )));
        assert!(base.apply(BuildOp::SetPartition(
            same_from_the_other_side,
            Some(Panel::Door)
        )));
        assert_eq!(base.partitions().count(), 1);

        // A ray across the room meets it, and says it is not the hull.
        let hit = base
            .raycast(Vec3::new(0.5, 1.0, 4.5), Vec3::X, 50.0)
            .unwrap();
        assert!(hit.partition && (hit.distance - 2.5).abs() < 1e-4);

        // Pull the east wall in twice and the tiles beside it go, and the wall with them.
        let east = Face::new(IVec3::new(2, 0, 1), Side::East);
        assert!(base.apply(BuildOp::Shrink(east)));
        assert_eq!(base.partitions().count(), 1);
        assert!(base.apply(BuildOp::Shrink(Face::new(IVec3::new(1, 0, 1), Side::East))));
        assert_eq!(base.partitions().count(), 0);
        assert!(!base.apply(BuildOp::SetPartition(inside, None)));
    }
}
