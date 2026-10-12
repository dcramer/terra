//! Construction: walls, floors and roofs on a grid (D152; docs/structures.md).
//!
//! A base is what stands in it, and nothing is placed whole:
//!
//! - **A wall stands on an edge** between two cells, a storey high. What fills it, solid,
//!   a window or a doorway, is a property of the wall cell, so an opening can be any
//!   width and stand anywhere along a wall, a cell at a time.
//! - **A floor lies at the foot of a cell, a roof over it.**
//! - **Rooms are worked out**, storey by storey: the cells that walls close in on every
//!   side. Nobody places a room.
//! - **The room tool is a shortcut** ([`BuildOp::AddRoom`]): walls round a block of cells,
//!   a floor under it and a roof over it. Where it meets a room already there, the two open
//!   into one another.
//!
//! Everything is on one square grid of [`CELL_M`]. That is the smallest thing that can be
//! placed. A cell is a column of that size and one storey, [`STOREY_M`], tall.
//!
//! Everything a player does is a [`BuildOp`], so a base can be sent to other players and
//! saved as the list of operations that made it (D21).
//!
//! Coordinates are local to the base: cells on an integer grid, X and Z along the floor
//! and Y up, counted in storeys.

pub mod logs;
mod mesh;
mod walk;

pub use mesh::{BaseMesh, Surface};
pub use walk::Walker;

use glam::{IVec2, IVec3, Vec3, Vec3Swizzles};
use std::collections::{BTreeMap, BTreeSet};

/// The grid: the width and depth of one cell, in metres. Everything else here is given in
/// metres and turned into cells with [`cells`], so this is the one number to change.
pub const CELL_M: f32 = 0.5;
/// Height of one storey, in metres.
pub const STOREY_M: f32 = 3.0;
/// Width of a door's opening, in metres.
pub const DOOR_WIDTH_M: f32 = 1.5;
/// Height of a door's opening, in metres.
pub const DOOR_HEIGHT_M: f32 = 2.2;

/// How many cells a length in metres is: the nearest whole number, and at least one.
pub fn cells(metres: f32) -> i32 {
    ((metres / CELL_M).round() as i32).max(1)
}

/// One of the six directions a cell faces.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
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

    /// The level direction a wall facing this way runs in.
    pub fn along(self) -> IVec3 {
        self.across()[0]
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

/// One side of one cell.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
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

/// What fills a wall cell.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Panel {
    Solid,
    /// Glass, from `sill` metres above the floor to `top` metres below the ceiling, the
    /// full width of the cell. Neighbouring cells with the same glass are one window.
    Window {
        sill: f32,
        top: f32,
    },
    /// An opening to walk through, the full width of the cell. Neighbouring cells are
    /// one doorway; a walker needs more than one.
    Door,
}

impl Panel {
    /// A window at about eye level.
    pub const WINDOW: Panel = Panel::Window {
        sill: 0.9,
        top: 0.5,
    };
    /// Glass from just above the floor to just under the roof.
    pub const TALL_WINDOW: Panel = Panel::Window {
        sill: 0.2,
        top: 0.2,
    };

    /// The next kind in the order a player cycles through.
    pub fn next(self) -> Panel {
        match self {
            Panel::Solid => Panel::WINDOW,
            p if p == Panel::WINDOW => Panel::TALL_WINDOW,
            Panel::Window { .. } => Panel::Door,
            Panel::Door => Panel::Solid,
        }
    }
}

/// How a pitched roof slopes (docs/structures.md, roofs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RoofStyle {
    /// Sloping up from every side to a ridge or a point.
    Hip,
    /// Sloping up from two sides to a ridge, with upright gable ends.
    Gable,
    /// One slope, up from one side to the other.
    LeanTo,
}

/// A pitched roof over a block of cells on one storey, resting on the top of its walls.
/// Where sections overlap, the roof is the highest of them at each point, so a gable
/// meeting a hip makes its own valley.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RoofSection {
    /// The cells it covers; both on its storey.
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub style: RoofStyle,
    /// For a gable, whether its ridge runs east and west; for a lean-to, whether its high
    /// side runs east and west, rising to the south.
    pub along_x: bool,
    /// How far it rises for each metre it runs.
    pub pitch: f32,
}

impl RoofSection {
    /// How far above the tops of its walls it is at a point, in metres east and south in
    /// the base's own space, if it is over that point.
    pub fn rise(&self, at: glam::Vec2) -> Option<f32> {
        let low = glam::Vec2::new(self.min[0] as f32, self.min[2] as f32) * CELL_M;
        let high = glam::Vec2::new((self.max[0] + 1) as f32, (self.max[2] + 1) as f32) * CELL_M;
        const EDGE: f32 = 1e-4;
        if at.cmplt(low - EDGE).any() || at.cmpgt(high + EDGE).any() {
            return None;
        }
        let (west, east) = ((at.x - low.x).max(0.0), (high.x - at.x).max(0.0));
        let (north, south) = ((at.y - low.y).max(0.0), (high.y - at.y).max(0.0));
        let run = match (self.style, self.along_x) {
            // As far as the nearest side, so every face is flat and every hip a diagonal.
            (RoofStyle::Hip, _) => west.min(east).min(north).min(south),
            (RoofStyle::Gable, true) => north.min(south),
            (RoofStyle::Gable, false) => west.min(east),
            (RoofStyle::LeanTo, true) => north,
            (RoofStyle::LeanTo, false) => west,
        };
        Some(run * self.pitch)
    }

    /// The storey whose walls it rests on.
    pub fn storey(&self) -> i32 {
        self.min[1]
    }

    /// Whether it is over a cell.
    pub fn covers(&self, tile: IVec3) -> bool {
        tile.y == self.min[1]
            && (self.min[0]..=self.max[0]).contains(&tile.x)
            && (self.min[2]..=self.max[2]).contains(&tile.z)
    }
}

/// Something a player did to a base.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BuildOp {
    /// Stand a wall on the edge on this side of a cell, change what fills it, or take it
    /// away with `None`.
    SetWall(Face, Option<Panel>),
    /// Lay a floor at the foot of every cell of a block, or take them away.
    SetFloor {
        min: [i32; 3],
        max: [i32; 3],
        laid: bool,
    },
    /// Lay a roof over every cell of a block, or take them away.
    SetRoof {
        min: [i32; 3],
        max: [i32; 3],
        laid: bool,
    },
    /// Put a pitched roof over a block of cells. It takes the place of any flat roof
    /// there.
    AddRoof(RoofSection),
    /// The room tool's shortcut: walls round a block of cells on each of its storeys, and
    /// a floor and a roof on each. Where it meets rooms already there on the same storey,
    /// the walls between are taken away, so they open into one another and a room drawn
    /// across another extends it.
    AddRoom { min: [i32; 3], max: [i32; 3] },
}

/// A base: its walls, floors and roofs, and the rooms they close in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Base {
    /// Each wall, named from the cell on its west or north, and what fills it.
    walls: BTreeMap<Face, Panel>,
    /// The cells with a floor at their foot.
    floors: BTreeSet<[i32; 3]>,
    /// The cells with a flat roof over them.
    roofs: BTreeSet<[i32; 3]>,
    /// The pitched roofs, in the order they were put up.
    sections: Vec<RoofSection>,
    /// Worked out from the walls: every cell they close in, and which room it is in.
    rooms: BTreeMap<[i32; 3], u32>,
}

/// Where a ray met a base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildHit {
    /// A wall, named from the cell on its west or north; or the floor (`Down`) or roof
    /// (`Up`) of a cell.
    pub face: Face,
    /// Distance along the ray, in metres.
    pub distance: f32,
    /// Whether what was met is a wall, not a floor or roof.
    pub wall: bool,
}

const LEVEL: [Side; 4] = [Side::East, Side::West, Side::South, Side::North];

impl Base {
    pub fn is_empty(&self) -> bool {
        self.walls.is_empty()
            && self.floors.is_empty()
            && self.roofs.is_empty()
            && self.sections.is_empty()
    }

    /// How many cells are in rooms.
    pub fn tile_count(&self) -> usize {
        self.rooms.len()
    }

    /// Whether a cell is in a room: closed in by walls on its storey.
    pub fn contains(&self, tile: IVec3) -> bool {
        self.rooms.contains_key(&tile.to_array())
    }

    /// Every cell in a room and the room it is in, in a fixed order.
    pub fn tiles(&self) -> impl Iterator<Item = (IVec3, u32)> + '_ {
        self.rooms
            .iter()
            .map(|(&tile, &room)| (IVec3::from_array(tile), room))
    }

    /// Which room a cell is in, if it is in one.
    pub fn room(&self, tile: IVec3) -> Option<u32> {
        self.rooms.get(&tile.to_array()).copied()
    }

    /// Whether a cell has a floor at its foot.
    pub fn has_floor(&self, tile: IVec3) -> bool {
        self.floors.contains(&tile.to_array())
    }

    /// Whether a cell has a flat roof over it.
    pub fn has_roof(&self, tile: IVec3) -> bool {
        self.roofs.contains(&tile.to_array())
    }

    /// Whether a cell has any roof over it, flat or pitched.
    pub fn covered(&self, tile: IVec3) -> bool {
        self.has_roof(tile) || self.sections.iter().any(|section| section.covers(tile))
    }

    /// The pitched roofs, in the order they were put up.
    pub fn sections(&self) -> &[RoofSection] {
        &self.sections
    }

    /// How far above the tops of a storey's walls its pitched roof is at a point, in
    /// metres east and south in the base's own space: the highest section over it.
    pub fn roof_rise(&self, storey: i32, at: glam::Vec2) -> Option<f32> {
        self.sections
            .iter()
            .filter(|section| section.storey() == storey)
            .filter_map(|section| section.rise(at))
            .max_by(|a, b| a.total_cmp(b))
    }

    /// Every cell with a floor, in a fixed order.
    pub fn floors(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.floors.iter().map(|&tile| IVec3::from_array(tile))
    }

    /// Every cell with a roof, in a fixed order.
    pub fn roofs(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.roofs.iter().map(|&tile| IVec3::from_array(tile))
    }

    /// Every wall, named from the cell on its west or north, in a fixed order.
    pub fn walls(&self) -> impl Iterator<Item = (Face, Panel)> + '_ {
        self.walls.iter().map(|(&face, &panel)| (face, panel))
    }

    /// Applies an operation. Returns whether it changed anything.
    pub fn apply(&mut self, op: BuildOp) -> bool {
        let changed = match op {
            BuildOp::SetWall(face, panel) => self.set_wall(face, panel),
            BuildOp::SetFloor { min, max, laid } => {
                Self::set_cells(&mut self.floors, min, max, laid)
            }
            BuildOp::SetRoof { min, max, laid } => Self::set_cells(&mut self.roofs, min, max, laid),
            BuildOp::AddRoof(section) => {
                // It takes the place of a flat roof under it.
                Self::set_cells(&mut self.roofs, section.min, section.max, false);
                if self.sections.contains(&section) {
                    false
                } else {
                    self.sections.push(section);
                    true
                }
            }
            BuildOp::AddRoom { min, max } => {
                self.add_room(IVec3::from_array(min), IVec3::from_array(max))
            }
        };
        if changed {
            self.find_rooms();
        }
        changed
    }

    fn set_wall(&mut self, face: Face, panel: Option<Panel>) -> bool {
        if !face.side.is_wall() {
            return false;
        }
        let edge = Self::edge(face);
        match panel {
            Some(panel) => self.walls.insert(edge, panel) != Some(panel),
            None => self.walls.remove(&edge).is_some(),
        }
    }

    fn set_cells(cells: &mut BTreeSet<[i32; 3]>, min: [i32; 3], max: [i32; 3], laid: bool) -> bool {
        let mut changed = false;
        for x in min[0]..=max[0] {
            for y in min[1]..=max[1] {
                for z in min[2]..=max[2] {
                    changed |= if laid {
                        cells.insert([x, y, z])
                    } else {
                        cells.remove(&[x, y, z])
                    };
                }
            }
        }
        changed
    }

    fn add_room(&mut self, min: IVec3, max: IVec3) -> bool {
        let before = (self.walls.clone(), self.floors.clone(), self.roofs.clone());
        // Rooms as they were, so what this block meets is judged before it changes them.
        let was = self.rooms.clone();
        let was_room = |tile: IVec3| was.contains_key(&tile.to_array());
        let inside = |tile: IVec3| tile.cmpge(min).all() && tile.cmple(max).all();
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    let cell = IVec3::new(x, y, z);
                    for side in LEVEL {
                        let next = cell + side.offset();
                        let face = Face::new(cell, side);
                        let between_rooms = was_room(cell) && was_room(next);
                        if inside(next) || was_room(next) {
                            // Within the block, or where it meets a room already there:
                            // open, but for a wall that already stood between two rooms.
                            if !between_rooms {
                                self.set_wall(face, None);
                            }
                        } else if self.filling(face).is_none() {
                            self.set_wall(face, Some(Panel::Solid));
                        }
                    }
                    // A floor and a roof on every storey of it: where a storey stands on
                    // another, the roof below is the ceiling under the floor above.
                    self.floors.insert(cell.to_array());
                    self.roofs.insert(cell.to_array());
                }
            }
        }
        before != (self.walls.clone(), self.floors.clone(), self.roofs.clone())
    }

    /// Works out the rooms: on each storey, the cells that cannot be reached from beyond
    /// the walls without passing through one, grouped by which reach each other.
    fn find_rooms(&mut self) {
        let mut rooms = BTreeMap::new();
        let mut storeys: BTreeMap<i32, (IVec2, IVec2)> = BTreeMap::new();
        for face in self.walls.keys() {
            let tile = face.tile();
            let beyond = tile + face.side.offset();
            let (low, high) = (tile.min(beyond).xz(), tile.max(beyond).xz());
            storeys
                .entry(tile.y)
                .and_modify(|(lo, hi)| {
                    *lo = lo.min(low);
                    *hi = hi.max(high);
                })
                .or_insert((low, high));
        }
        let mut next_room = 0;
        for (y, (low, high)) in storeys {
            // One cell wider all round, so the open air surrounds everything.
            let (low, high) = (low - IVec2::ONE, high + IVec2::ONE);
            let within = |cell: IVec2| cell.cmpge(low).all() && cell.cmple(high).all();
            let open = |cell: IVec2, side: Side| {
                let tile = IVec3::new(cell.x, y, cell.y);
                self.filling(Face::new(tile, side)).is_none()
            };
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
                    if within(next) && open(cell, side) && reached.insert(next.to_array()) {
                        frontier.push(next);
                    }
                }
            }
            // What the open air never reached is closed in: each group of it a room.
            for x in low.x..=high.x {
                for z in low.y..=high.y {
                    let start = IVec2::new(x, z);
                    if !reached.insert(start.to_array()) {
                        continue;
                    }
                    let mut frontier = vec![start];
                    while let Some(cell) = frontier.pop() {
                        rooms.insert([cell.x, y, cell.y], next_room);
                        for side in LEVEL {
                            let next = step(cell, side);
                            if open(cell, side) && reached.insert(next.to_array()) {
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

    /// What fills a wall, seen from either side. `None` where there is no wall, and for
    /// floors and roofs.
    pub fn filling(&self, face: Face) -> Option<Panel> {
        if !face.side.is_wall() {
            return None;
        }
        self.walls.get(&Self::edge(face)).copied()
    }

    /// What fills a wall, or solid where there is none.
    pub fn panel(&self, face: Face) -> Panel {
        self.filling(face).unwrap_or(Panel::Solid)
    }

    /// Whether a wall is an outside wall: a room on one side of it and not the other.
    pub fn exterior(&self, face: Face) -> bool {
        self.filling(face).is_some()
            && self.contains(face.tile()) != self.contains(face.tile() + face.side.offset())
    }

    /// The way out through an outside wall, from its room toward the open. `None` if it is
    /// not an outside wall.
    pub fn outward(&self, face: Face) -> Option<IVec3> {
        if !self.exterior(face) {
            return None;
        }
        let offset = face.side.offset();
        Some(if self.contains(face.tile()) {
            offset
        } else {
            -offset
        })
    }

    /// The window or doorway a wall cell is part of: every cell in a line with it, on the
    /// same storey, that has the same filling, in order along the wall and named from
    /// their west or north. Just the cell itself if it is solid, and nothing if there is
    /// no wall there.
    pub fn run(&self, face: Face) -> Vec<Face> {
        let Some(panel) = self.filling(face) else {
            return Vec::new();
        };
        let face = Self::edge(face);
        if panel == Panel::Solid {
            return vec![face];
        }
        let along = face.side.along();
        let same = |cell: IVec3| self.filling(Face::new(cell, face.side)) == Some(panel);
        let mut first = face.tile();
        while same(first - along) {
            first -= along;
        }
        let mut found = Vec::new();
        let mut cell = first;
        while same(cell) {
            found.push(Face::new(cell, face.side));
            cell += along;
        }
        found
    }

    /// The same edge between two cells, named from whichever has it on its east or south
    /// side. Floors and roofs are left as they are.
    pub fn edge(face: Face) -> Face {
        match face.side {
            Side::West => Face::new(face.tile() + IVec3::NEG_X, Side::East),
            Side::North => Face::new(face.tile() + IVec3::NEG_Z, Side::South),
            _ => face,
        }
    }

    /// The first wall, floor or roof a ray meets, from either side, if any. `origin` and
    /// `direction` are in the base's own space, in metres; `direction` is unit length.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<BuildHit> {
        let size = Vec3::new(CELL_M, STOREY_M, CELL_M);
        let walls = self.walls.keys().map(|&face| (face, true));
        let floors = self.floors.iter().map(|&tile| {
            (
                Face {
                    tile,
                    side: Side::Down,
                },
                false,
            )
        });
        let roofs = self.roofs.iter().map(|&tile| {
            (
                Face {
                    tile,
                    side: Side::Up,
                },
                false,
            )
        });
        walls
            .chain(floors)
            .chain(roofs)
            .filter_map(|(face, wall)| {
                let normal = face.side.offset().as_vec3();
                let low = face.tile().as_vec3() * size;
                // A point on the face's plane: the cell's far corner on positive sides.
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
                    wall,
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
        mesh::build(self, None)
    }

    /// The base with its walls laid as logs: its floors, roofs and gable ends as
    /// triangles, and the walls' pieces, in the order they go in, to draw as they stand.
    pub fn mesh_with_logs(&self, kit: &logs::LogKit) -> (BaseMesh, logs::Layout) {
        let mut mesh = mesh::build(self, Some((kit.colours.board, kit.colours.roof)));
        let layout = logs::lay(self, kit);
        mesh.glass.positions.extend(&layout.glass.positions);
        mesh.glass.normals.extend(&layout.glass.normals);
        mesh.glass.colours.extend(&layout.glass.colours);
        (mesh, layout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A room three cells square, by the shortcut.
    fn room() -> Base {
        let mut base = Base::default();
        assert!(base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [2, 0, 2]
        }));
        base
    }

    #[test]
    fn the_room_tool_lays_walls_round_a_floor_and_a_roof() {
        let base = room();
        assert_eq!(base.walls().count(), 12);
        assert_eq!(base.floors().count(), 9);
        assert_eq!(base.roofs().count(), 9);
        assert_eq!(base.tile_count(), 9);
        // Nothing between two cells of the room.
        assert_eq!(
            base.filling(Face::new(IVec3::new(0, 0, 0), Side::East)),
            None
        );
        assert!(base.exterior(Face::new(IVec3::new(2, 0, 1), Side::East)));
    }

    #[test]
    fn walls_drawn_round_cells_close_them_into_a_room_and_a_gap_opens_it() {
        let mut base = Base::default();
        // Four walls round a block two cells by one, drawn one cell at a time.
        let walls = [
            Face::new(IVec3::new(0, 0, 0), Side::North),
            Face::new(IVec3::new(1, 0, 0), Side::North),
            Face::new(IVec3::new(0, 0, 0), Side::South),
            Face::new(IVec3::new(1, 0, 0), Side::South),
            Face::new(IVec3::new(0, 0, 0), Side::West),
            Face::new(IVec3::new(1, 0, 0), Side::East),
        ];
        for face in walls {
            base.apply(BuildOp::SetWall(face, Some(Panel::Solid)));
        }
        assert_eq!(base.tile_count(), 2);
        assert_eq!(base.room(IVec3::ZERO), base.room(IVec3::X));
        // A doorway still closes it in; a gap does not.
        base.apply(BuildOp::SetWall(walls[0], Some(Panel::Door)));
        assert_eq!(base.tile_count(), 2);
        base.apply(BuildOp::SetWall(walls[0], None));
        assert_eq!(base.tile_count(), 0);
    }

    #[test]
    fn a_wall_across_a_room_makes_two() {
        let mut base = room();
        for z in 0..3 {
            base.apply(BuildOp::SetWall(
                Face::new(IVec3::new(0, 0, z), Side::East),
                Some(Panel::Solid),
            ));
        }
        assert_eq!(base.tile_count(), 9);
        assert_ne!(base.room(IVec3::ZERO), base.room(IVec3::new(2, 0, 0)));
        assert!(!base.exterior(Face::new(IVec3::new(0, 0, 1), Side::East)));
    }

    #[test]
    fn rooms_that_touch_open_into_each_other() {
        let mut base = room();
        base.apply(BuildOp::AddRoom {
            min: [3, 0, 1],
            max: [5, 0, 1],
        });
        // The corridor meets the room, so there is no wall between.
        assert_eq!(
            base.filling(Face::new(IVec3::new(2, 0, 1), Side::East)),
            None
        );
        assert!(
            base.filling(Face::new(IVec3::new(2, 0, 0), Side::East))
                .is_some()
        );
        assert_eq!(base.room(IVec3::new(5, 0, 1)), base.room(IVec3::ZERO));
    }

    #[test]
    fn a_room_drawn_across_another_extends_it() {
        let mut base = room();
        // Half over the first room, half beyond its east wall.
        assert!(base.apply(BuildOp::AddRoom {
            min: [2, 0, 1],
            max: [4, 0, 3],
        }));
        assert_eq!(base.room(IVec3::new(4, 0, 3)), base.room(IVec3::ZERO));
        // The old east wall where the new block crosses it is gone.
        assert_eq!(
            base.filling(Face::new(IVec3::new(2, 0, 1), Side::South)),
            None
        );
        // A block wholly inside changes nothing, and a block apart is its own room.
        assert!(!base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [1, 0, 1],
        }));
        assert!(base.apply(BuildOp::AddRoom {
            min: [8, 0, 0],
            max: [9, 0, 1],
        }));
        assert_ne!(base.room(IVec3::new(8, 0, 0)), base.room(IVec3::ZERO));
    }

    #[test]
    fn a_room_drawn_on_top_of_another_stands_on_it() {
        let mut base = room();
        base.apply(BuildOp::AddRoom {
            min: [0, 1, 0],
            max: [1, 1, 1],
        });
        // The roof below is the ceiling under the floor above.
        assert!(base.has_roof(IVec3::ZERO));
        assert!(base.has_floor(IVec3::Y));
        assert!(base.has_roof(IVec3::Y));
        assert!(base.contains(IVec3::Y));
    }

    #[test]
    fn openings_fill_wall_cells_and_go_with_them() {
        let mut base = room();
        let wall = Face::new(IVec3::new(2, 0, 1), Side::East);
        assert!(base.apply(BuildOp::SetWall(wall, Some(Panel::WINDOW))));
        assert_eq!(base.panel(wall), Panel::WINDOW);
        // The same from the other side, and setting it again changes nothing.
        let other_side = Face::new(IVec3::new(3, 0, 1), Side::West);
        assert_eq!(base.panel(other_side), Panel::WINDOW);
        assert!(!base.apply(BuildOp::SetWall(other_side, Some(Panel::WINDOW))));
        // Floors and roofs take no walls.
        assert!(!base.apply(BuildOp::SetWall(
            Face::new(IVec3::ZERO, Side::Down),
            Some(Panel::WINDOW)
        )));
        assert!(base.apply(BuildOp::SetWall(wall, None)));
        assert_eq!(base.filling(wall), None);
    }

    #[test]
    fn cycling_a_panel_comes_back_round() {
        let mut panel = Panel::Solid;
        let mut seen = vec![panel];
        for _ in 0..4 {
            panel = panel.next();
            seen.push(panel);
        }
        assert_eq!(seen.first(), seen.last());
        assert_eq!(seen[1], Panel::WINDOW);
        assert_eq!(seen[3], Panel::Door);
    }

    #[test]
    fn a_ray_finds_the_wall_it_points_at_from_either_side() {
        let base = room();
        // From outside, east of the room, looking west at the middle cell.
        let t = CELL_M;
        let from_outside = base
            .raycast(Vec3::new(t * 10.0, 1.5, t * 1.5), Vec3::NEG_X, 100.0)
            .unwrap();
        assert_eq!(
            from_outside.face,
            Face::new(IVec3::new(2, 0, 1), Side::East)
        );
        assert!(from_outside.wall);
        assert!((from_outside.distance - t * 7.0).abs() < 1e-4);
        // From inside the room, looking east at the same wall.
        let from_inside = base
            .raycast(Vec3::new(t * 1.5, 1.5, t * 1.5), Vec3::X, 100.0)
            .unwrap();
        assert_eq!(from_inside.face, from_outside.face);
        // Looking down from inside, the floor.
        let down = base
            .raycast(Vec3::new(t * 1.5, 1.5, t * 1.5), Vec3::NEG_Y, 100.0)
            .unwrap();
        assert_eq!(down.face, Face::new(IVec3::new(1, 0, 1), Side::Down));
        assert!(!down.wall);
        // Looking away from the base finds nothing.
        assert!(
            base.raycast(Vec3::new(t * 10.0, 1.5, t * 1.5), Vec3::X, 100.0)
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
            BuildOp::SetWall(
                Face::new(IVec3::new(2, 0, 1), Side::East),
                Some(Panel::TALL_WINDOW),
            ),
            BuildOp::AddRoom {
                min: [0, 1, 0],
                max: [1, 1, 1],
            },
            BuildOp::SetFloor {
                min: [5, 0, 5],
                max: [6, 0, 6],
                laid: true,
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
    fn neighbouring_cells_with_the_same_opening_are_one_run() {
        let mut base = room();
        let south = |x: i32| Face::new(IVec3::new(x, 0, 2), Side::South);
        base.apply(BuildOp::SetWall(south(0), Some(Panel::Door)));
        base.apply(BuildOp::SetWall(south(1), Some(Panel::Door)));
        base.apply(BuildOp::SetWall(south(2), Some(Panel::WINDOW)));
        assert_eq!(base.run(south(1)), vec![south(0), south(1)]);
        assert_eq!(base.run(south(2)), vec![south(2)]);
        // Named from either side, the same run.
        assert_eq!(
            base.run(Face::new(IVec3::new(1, 0, 3), Side::North)),
            vec![south(0), south(1)]
        );
        // A solid cell is a run of one, and where there is no wall there is no run.
        let north = Face::new(IVec3::new(1, 0, -1), Side::South);
        assert_eq!(base.run(north), vec![north]);
        assert!(
            base.run(Face::new(IVec3::new(1, 0, 1), Side::East))
                .is_empty()
        );
        assert_eq!(cells(CELL_M * 3.0), 3);
        assert_eq!(cells(0.0), 1);
    }

    #[test]
    fn a_hip_roof_rises_to_its_middle_and_a_gable_to_its_ridge() {
        let section = |style, along_x| RoofSection {
            min: [0, 0, 0],
            max: [7, 0, 3],
            style,
            along_x,
            pitch: 0.5,
        };
        let at = |x: f32, z: f32| glam::Vec2::new(x, z);
        // Four metres by two: a hip rises half a metre to its ridge, a metre in.
        let hip = section(RoofStyle::Hip, true);
        assert_eq!(hip.rise(at(0.0, 1.0)), Some(0.0));
        assert_eq!(hip.rise(at(2.0, 1.0)), Some(0.5));
        assert_eq!(hip.rise(at(0.5, 1.0)), Some(0.25));
        assert_eq!(hip.rise(at(5.0, 1.0)), None);
        // A gable with its ridge east and west is as high at its end as in its middle.
        let gable = section(RoofStyle::Gable, true);
        assert_eq!(gable.rise(at(0.0, 1.0)), Some(0.5));
        assert_eq!(gable.rise(at(2.0, 0.5)), Some(0.25));
        // A lean-to rises across the whole of it.
        let lean = section(RoofStyle::LeanTo, true);
        assert_eq!(lean.rise(at(1.0, 2.0)), Some(1.0));
    }

    #[test]
    fn a_pitched_roof_takes_the_place_of_a_flat_one_and_covers_the_room() {
        let mut base = room();
        assert!(base.apply(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [2, 0, 2],
            style: RoofStyle::Hip,
            along_x: true,
            pitch: 0.7,
        })));
        assert_eq!(base.roofs().count(), 0);
        assert!(base.covered(IVec3::new(1, 0, 1)));
        assert!(!base.covered(IVec3::new(1, 1, 1)));
        // Where two sections overlap, the higher.
        base.apply(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 1],
            max: [5, 0, 1],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        let middle = glam::Vec2::new(0.25, 0.75);
        assert!((base.roof_rise(0, middle).unwrap() - 0.25 * 0.7).abs() < 1e-5);
    }

    #[test]
    fn the_way_out_of_an_outside_wall_points_away_from_its_room() {
        let base = room();
        let east = Face::new(IVec3::new(2, 0, 1), Side::East);
        assert_eq!(base.outward(east), Some(IVec3::X));
        let north = Face::new(IVec3::new(1, 0, -1), Side::South);
        assert_eq!(base.outward(north), Some(IVec3::NEG_Z));
    }
}
