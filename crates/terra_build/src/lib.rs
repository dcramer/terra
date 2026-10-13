//! Construction: everything built is half-metre cubes on a base's grid (D48, D181;
//! docs/building-tech.md, docs/building-plan.md).
//!
//! - **A cube is air or a kind of construction** ([`Kind`]), with a shape ([`Shape`]),
//!   planned or built to a fill level ([`State`], D194), at most one change pending on it,
//!   and the part it was laid in, packed in 32 bits in sections of 16 a side
//!   ([`cubes`]). What fills it ([`Fill`]) is solid, glass, or open: an opening cut in a
//!   wall, part of the wall still, which lets a walker through.
//! - **A wall is a row of cubes**, a storey high unless built otherwise; a post is a
//!   column of them, a floor a layer. So walls meet, cross, are carved and mix kinds by
//!   sharing cubes.
//! - **A storey is six cubes, three metres.** Its floor is the layer of cubes just under
//!   it, so a floor's top is the storey's height, and the layer at the top of its walls is
//!   the next storey's floor, or a flat roof. A cell of a storey, a *tile*, is a column of
//!   six cubes.
//! - **Rooms are worked out**, storey by storey ([`rooms`]).
//! - **Pitched roofs are the one thing not cubes**: surfaces over the tiles they cover
//!   ([`RoofSection`]), each cell going up in stages ([`Roof`]).
//! - **Doors and windows are props** ([`Prop`]) fitted into holes in built walls.
//! - **Everything a player does is an operation** ([`ops::Op`]), checked by one pure
//!   function against the state when it arrives, and undone by its inverse: nothing
//!   floats, and nothing is taken while something rests on it ([`support`], D193).
//! - **Shortcuts** ([`BuildOp`]) lay built cubes as a player means them, instantly, as
//!   the yard and the game do today: a wall tile as a panel says, solid or with a window
//!   or doorway of a default size ([`BuildOp::SetWall`]); a floor or flat roof over a
//!   block; and the room tool, walls round a footprint with a floor inside and a roof
//!   over it.
//!
//! A base is saved and sent as a snapshot ([`save`]), and before that as the list of
//! shortcuts that made it (D21), which is still read.
//!
//! Coordinates are local to the base: X and Z along the floor and Y up. Cubes are counted
//! in cubes all three ways; tiles count X and Z in cubes and Y in storeys.

pub mod cost;
pub mod cubes;
mod example;
pub mod ground;
pub mod logs;
pub mod masonry;
mod mesh;
pub mod ops;
pub mod rooms;
pub mod save;
pub mod support;
pub mod surface;
mod walk;

pub use cost::{Amount, Bill, PropKind, Rules, Terms};
pub use cubes::{Cube, Cubes, Facing, Fill, Kind, PartId, Pending, Replacement, Shape, State};
pub use example::example;
pub use ground::{Ground, Sample};
pub use mesh::{BaseMesh, Surface};
pub use walk::Walker;

use glam::{IVec2, IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// The grid: the size of one cube, in metres. Everything else here is given in metres and
/// turned into cubes with [`cells`], so this is the one number to change.
pub const CELL_M: f32 = 0.5;
/// Height of one storey, in metres.
pub const STOREY_M: f32 = 3.0;
/// Height of one storey, in cubes.
pub const STOREY_CUBES: i32 = 6;
/// Width of a door's opening by default, in metres.
pub const DOOR_WIDTH_M: f32 = 1.0;
/// Height of a door's opening, in metres.
pub const DOOR_HEIGHT_M: f32 = 2.0;

/// How many cells a length in metres is: the nearest whole number, and at least one.
pub fn cells(metres: f32) -> i32 {
    ((metres / CELL_M).round() as i32).max(1)
}

/// The layer of cubes a storey's walls start on.
pub fn wall_layer(storey: i32) -> i32 {
    storey * STOREY_CUBES
}

/// The layer of cubes a storey's floor is, just under its walls' foot.
pub fn floor_layer(storey: i32) -> i32 {
    wall_layer(storey) - 1
}

/// The storey whose walls a layer of cubes is part of.
pub fn storey_of(layer: i32) -> i32 {
    layer.div_euclid(STOREY_CUBES)
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

    fn of(direction: IVec3) -> Side {
        match direction.to_array() {
            [1, _, _] => Side::East,
            [-1, _, _] => Side::West,
            [_, _, 1] => Side::South,
            [_, _, -1] => Side::North,
            [_, 1, _] => Side::Up,
            _ => Side::Down,
        }
    }

    /// Whether this is a wall, as opposed to a floor or roof.
    pub fn is_wall(self) -> bool {
        !matches!(self, Side::Up | Side::Down)
    }
}

/// One side of one tile.
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

/// What fills a wall tile, as the shortcut lays it and as it is read back.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Panel {
    Solid,
    /// Glass from `sill` metres above the floor to `top` metres below the ceiling.
    /// Neighbouring tiles with the same glass are one window.
    Window {
        sill: f32,
        top: f32,
    },
    /// An opening to walk through, a door's height. Neighbouring tiles are one doorway;
    /// a walker needs more than one.
    Door,
}

impl Panel {
    /// A window by default (D175, D181): a sill at a metre, a head at two, level with the
    /// doors.
    pub const WINDOW: Panel = Panel::Window {
        sill: 1.0,
        top: 1.0,
    };
    /// Glass from half a metre over the floor to half a metre under the ceiling.
    pub const TALL_WINDOW: Panel = Panel::Window {
        sill: 0.5,
        top: 0.5,
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

    /// What it puts in the cube so many layers up a wall tile.
    pub fn fill(self, layer: i32) -> Fill {
        let (bottom, top) = (layer as f32 * CELL_M, (layer + 1) as f32 * CELL_M);
        let within = |from: f32, to: f32| bottom >= from - 1e-3 && top <= to + 1e-3;
        match self {
            Panel::Door if within(0.0, DOOR_HEIGHT_M) => Fill::Open,
            Panel::Window { sill, top: below } if within(sill, STOREY_M - below) => Fill::Glass,
            _ => Fill::Solid,
        }
    }
}

/// How a pitched roof slopes (docs/building-tech.md, roofs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RoofStyle {
    /// Sloping up from every side to a ridge or a point.
    Hip,
    /// Sloping up from two sides to a ridge, with upright gable ends.
    Gable,
    /// One slope, up from one side to the other.
    LeanTo,
}

/// A pitched roof over a block of tiles on one storey, resting on the top of its walls.
/// Where sections overlap, the roof is the highest of them at each point, so a gable
/// meeting a hip makes its own valley.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RoofSection {
    /// The tiles it covers; both on its storey.
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
        self.rise_beyond(at, 0.0).map(|rise| rise.max(0.0))
    }

    /// Its corners in metres along the floor, east and south: the lowest and the highest.
    pub fn bounds(&self) -> (glam::Vec2, glam::Vec2) {
        (
            glam::Vec2::new(self.min[0] as f32, self.min[2] as f32) * CELL_M,
            glam::Vec2::new((self.max[0] + 1) as f32, (self.max[2] + 1) as f32) * CELL_M,
        )
    }

    /// As [`RoofSection::rise`], but carried on down past its edges as far as `beyond`
    /// metres, for eaves that overhang the walls: below the walls' tops there.
    pub fn rise_beyond(&self, at: glam::Vec2, beyond: f32) -> Option<f32> {
        let (low, high) = self.bounds();
        let edge = beyond + 1e-4;
        if at.cmplt(low - edge).any() || at.cmpgt(high + edge).any() {
            return None;
        }
        let (west, east) = (at.x - low.x, high.x - at.x);
        let (north, south) = (at.y - low.y, high.y - at.y);
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

    /// Whether it is over a tile.
    pub fn covers(&self, tile: IVec3) -> bool {
        tile.y == self.min[1]
            && (self.min[0]..=self.max[0]).contains(&tile.x)
            && (self.min[2]..=self.max[2]).contains(&tile.z)
    }
}

/// How many stages a roof cell goes up in: its frame, its battens, its covering
/// (docs/building-tech.md, constructing).
pub const ROOF_STAGES: u8 = 3;

/// A roof section's number in its base, given by whoever puts it up, so putting it up
/// again after an undo gives the same number.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct RoofId(pub u32);

/// A roof section and how far each of its cells is up.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Roof {
    pub section: RoofSection,
    /// The kind it is built of; for a roof the shortcuts put up, the kind of the walls
    /// under it.
    pub kind: Option<Kind>,
    /// What a stage of one of its cells takes, as it was when it was put up.
    pub per_stage: Bill,
    /// Each cell's stage, from 0, planned, to [`ROOF_STAGES`], covered; east, then south.
    pub stages: Vec<u8>,
}

impl Roof {
    /// A roof planned, with nothing of it up.
    pub fn planned(section: RoofSection, kind: Option<Kind>, per_stage: Bill) -> Roof {
        let (low, high) = (section.min, section.max);
        let cells = ((high[0] - low[0] + 1) * (high[2] - low[2] + 1)).max(0) as usize;
        Roof {
            section,
            kind,
            per_stage,
            stages: vec![0; cells],
        }
    }

    /// Its cells, counted in cubes east and south, in order.
    pub fn cells(&self) -> impl Iterator<Item = [i32; 2]> + '_ {
        let (low, high) = (self.section.min, self.section.max);
        (low[0]..=high[0]).flat_map(move |x| (low[2]..=high[2]).map(move |z| [x, z]))
    }

    fn index(&self, cell: [i32; 2]) -> Option<usize> {
        let (low, high) = (self.section.min, self.section.max);
        let inside = (low[0]..=high[0]).contains(&cell[0]) && (low[2]..=high[2]).contains(&cell[1]);
        inside.then(|| ((cell[0] - low[0]) * (high[2] - low[2] + 1) + cell[1] - low[2]) as usize)
    }

    /// A cell's stage, if it is under the roof.
    pub fn stage(&self, cell: [i32; 2]) -> Option<u8> {
        self.index(cell).map(|index| self.stages[index])
    }

    /// Whether any of it is up, so it rests on the walls under its eaves.
    pub fn standing(&self) -> bool {
        self.stages.iter().any(|&stage| stage > 0)
    }
}

/// A part a player laid: its kind and the terms its cubes are built on, kept as they were
/// when it was laid, so taking it apart gives back what went in even if a kit changes.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Part {
    pub kind: Kind,
    pub terms: Terms,
    /// How many cubes, and cubes waiting to change to it, it is in.
    #[serde(skip)]
    uses: u32,
}

impl Part {
    pub fn new(kind: Kind, terms: Terms) -> Part {
        Part {
            kind,
            terms,
            uses: 0,
        }
    }
}

/// A door, window or shutters fitted into a hole in a wall (construction-001, gap 15),
/// filling the cubes from `min` to `max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Prop {
    pub kind: PropKind,
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub open: bool,
}

impl Prop {
    /// The cubes it fills.
    pub fn cubes(&self) -> impl Iterator<Item = IVec3> + '_ {
        let (low, high) = (self.min, self.max);
        (low[0]..=high[0]).flat_map(move |x| {
            (low[1]..=high[1])
                .flat_map(move |y| (low[2]..=high[2]).map(move |z| IVec3::new(x, y, z)))
        })
    }

    /// Whether it fills a cube.
    pub fn fills(&self, at: IVec3) -> bool {
        at.cmpge(IVec3::from_array(self.min)).all() && at.cmple(IVec3::from_array(self.max)).all()
    }
}

/// Something a player did to a base with a shortcut, laying built cubes at once.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BuildOp {
    /// Fill every cube of a block, counted in cubes, or empty them with `None`.
    Fill {
        min: [i32; 3],
        max: [i32; 3],
        cube: Option<Cube>,
    },
    /// Stand a wall on a tile in a kind, as a panel says, or take it away with `None`. Its
    /// cubes are the tile's column; the top one too, unless a floor or roof is there.
    SetWall {
        tile: [i32; 3],
        panel: Option<Panel>,
        kind: Kind,
    },
    /// Lay a floor under every tile of a block, or take them away.
    SetFloor {
        min: [i32; 3],
        max: [i32; 3],
        laid: bool,
        kind: Kind,
    },
    /// Lay a flat roof over every tile of a block, or take them away.
    SetRoof {
        min: [i32; 3],
        max: [i32; 3],
        laid: bool,
        kind: Kind,
    },
    /// Put a pitched roof over a block of tiles. It takes the place of any flat roof
    /// there.
    AddRoof(RoofSection),
    /// The room tool's shortcut, over a footprint of tiles on each of its storeys: walls
    /// round its edge, a floor inside and a flat roof over it. Where it meets rooms
    /// already there on the same storey, the walls between are taken away, so they open
    /// into one another and a room drawn across another extends it.
    AddRoom {
        min: [i32; 3],
        max: [i32; 3],
        kind: Kind,
    },
}

/// A base: its cubes, roofs, parts, props and the ground under them, and what is worked
/// out from them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Base {
    cubes: Cubes,
    /// The pitched roofs, by number, which is the order they were put up.
    roofs: BTreeMap<RoofId, Roof>,
    /// The parts its cubes were laid in.
    parts: BTreeMap<PartId, Part>,
    /// For each cube whose kind is to change, the kind and part it changes to.
    replacing: BTreeMap<[i32; 3], Replacement>,
    /// Fitted props, by their lowest cube.
    props: BTreeMap<[i32; 3], Prop>,
    ground: Ground,
    /// Worked out from the roofs: their sections, in order.
    sections: Vec<RoofSection>,
    /// Worked out from the cubes: each wall tile and what fills it.
    walls: BTreeMap<[i32; 3], Panel>,
    /// Each column of cubes, by where it stands, and its layers.
    columns: BTreeMap<[i32; 2], BTreeSet<i32>>,
    /// Every tile the walls close in, and which room it is in.
    rooms: BTreeMap<[i32; 3], u32>,
}

/// Where a ray met a base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildHit {
    /// The cube met, counted in cubes.
    pub cube: IVec3,
    /// A wall tile met on a side; or the tile whose floor (`Down`, met from above) or roof
    /// (`Up`, met from below) was met.
    pub face: Face,
    /// Distance along the ray, in metres.
    pub distance: f32,
    /// Whether what was met is a wall, not a floor or roof.
    pub wall: bool,
}

const LEVEL: [Side; 4] = [Side::East, Side::West, Side::South, Side::North];

impl Base {
    pub fn is_empty(&self) -> bool {
        self.cubes.is_empty() && self.roofs.is_empty() && self.props.is_empty()
    }

    /// The cube at a place, counted in cubes, if it is not air.
    pub fn cube(&self, at: IVec3) -> Option<Cube> {
        self.cubes.get(at)
    }

    /// Every cube that is not air, in a fixed order: by `[x, y, z]`.
    pub fn cubes(&self) -> impl Iterator<Item = (IVec3, Cube)> + '_ {
        self.cubes.iter()
    }

    /// The cube store, for its sections and their dirty marks.
    pub fn store(&self) -> &Cubes {
        &self.cubes
    }

    /// The sections changed since this was last asked; the marks are cleared.
    pub fn take_dirty(&mut self) -> BTreeSet<[i32; 3]> {
        self.cubes.take_dirty()
    }

    /// A part, if any cube is in it.
    pub fn part(&self, id: PartId) -> Option<&Part> {
        self.parts.get(&id)
    }

    /// Every part, by number.
    pub fn parts(&self) -> impl Iterator<Item = (PartId, &Part)> + '_ {
        self.parts.iter().map(|(&id, part)| (id, part))
    }

    /// The number after every part's, for a new part.
    pub fn next_part(&self) -> PartId {
        PartId(self.parts.keys().next_back().map_or(0, |id| id.0 + 1))
    }

    /// The terms a cube is built on: its part's, or free in one step.
    pub fn terms(&self, cube: &Cube) -> Terms {
        cube.part
            .and_then(|part| self.parts.get(&part))
            .map(|part| part.terms.clone())
            .unwrap_or_default()
    }

    /// What a cube whose kind is to change changes to.
    pub fn replacing(&self, at: IVec3) -> Option<Replacement> {
        self.replacing.get(&at.to_array()).copied()
    }

    /// A roof section by its number.
    pub fn roof(&self, id: RoofId) -> Option<&Roof> {
        self.roofs.get(&id)
    }

    /// Every roof section, in the order they were put up.
    pub fn roofs(&self) -> impl Iterator<Item = (RoofId, &Roof)> + '_ {
        self.roofs.iter().map(|(&id, roof)| (id, roof))
    }

    /// The number after every roof's, for a new one.
    pub fn next_roof(&self) -> RoofId {
        RoofId(self.roofs.keys().next_back().map_or(0, |id| id.0 + 1))
    }

    /// The prop fitted with its lowest cube at a place.
    pub fn prop(&self, at: IVec3) -> Option<&Prop> {
        self.props.get(&at.to_array())
    }

    /// Every fitted prop, by its lowest cube.
    pub fn props(&self) -> impl Iterator<Item = &Prop> + '_ {
        self.props.values()
    }

    /// The prop filling a cube, if any.
    pub fn prop_in(&self, at: IVec3) -> Option<&Prop> {
        self.props.values().find(|prop| prop.fills(at))
    }

    /// The ground under the base, as sampled.
    pub fn ground(&self) -> &Ground {
        &self.ground
    }

    /// Whether a cube is part of a wall, rather than a floor or roof: its tile is a wall
    /// tile.
    pub fn in_wall(&self, at: IVec3) -> bool {
        self.walls.contains_key(&[at.x, storey_of(at.y), at.z])
    }

    /// Whether a tile has a floor under it.
    pub fn has_floor(&self, tile: IVec3) -> bool {
        self.solid(IVec3::new(tile.x, floor_layer(tile.y), tile.z))
    }

    /// Whether a tile that is not a wall has a flat roof over it, or a floor of the
    /// storey above.
    pub fn has_roof(&self, tile: IVec3) -> bool {
        !self.walls.contains_key(&tile.to_array()) && self.has_floor(tile + IVec3::Y)
    }

    /// Whether a tile has any roof over it, flat or pitched.
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

    /// Every tile with a floor under it that is not the top of a wall below, in a fixed
    /// order: a flat roof is the floor of the storey over it.
    pub fn floors(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.cubes
            .iter()
            .map(|(at, _)| at)
            .filter(|at| (at.y + 1).rem_euclid(STOREY_CUBES) == 0)
            .map(|at| IVec3::new(at.x, storey_of(at.y + 1), at.z))
            .filter(|tile| !self.walls.contains_key(&(*tile - IVec3::Y).to_array()))
    }

    /// Every wall tile and what fills it, in a fixed order.
    pub fn walls(&self) -> impl Iterator<Item = (IVec3, Panel)> + '_ {
        self.walls
            .iter()
            .map(|(&tile, &panel)| (IVec3::from_array(tile), panel))
    }

    /// The layers of the cubes standing at a place, counted in cubes along the floor.
    pub fn column(&self, x: i32, z: i32) -> impl Iterator<Item = i32> + '_ {
        self.columns.get(&[x, z]).into_iter().flatten().copied()
    }

    fn solid(&self, at: IVec3) -> bool {
        self.cube(at).is_some_and(|cube| cube.fill != Fill::Open)
    }

    /// A base of these cubes and roofs, with its walls and rooms worked out.
    fn from_cubes(cubes: Cubes, roofs: BTreeMap<RoofId, Roof>) -> Base {
        let mut base = Base {
            cubes,
            roofs,
            ..Base::default()
        };
        base.settle();
        base
    }

    /// Whether anything stands at a place, counted in cubes along the floor.
    pub fn occupied(&self, x: i32, z: i32) -> bool {
        self.columns.contains_key(&[x, z])
    }

    /// Every place along the floor that is in a room, or has a floor, on the ground storey:
    /// where `contains` or `has_floor` holds at storey 0. In no fixed order.
    pub fn ground_columns(&self) -> impl Iterator<Item = IVec2> + '_ {
        let rooms = self
            .tiles()
            .filter(|(tile, _)| tile.y == 0)
            .map(|(tile, _)| IVec2::new(tile.x, tile.z));
        let floors = self
            .cubes
            .iter()
            .filter(|(at, cube)| at.y == floor_layer(0) && cube.fill != Fill::Open)
            .map(|(at, _)| IVec2::new(at.x, at.z));
        rooms.chain(floors)
    }

    /// Every place along the floor where anything stands: where `occupied` holds.
    pub fn occupied_columns(&self) -> impl Iterator<Item = IVec2> + '_ {
        self.columns.keys().map(|&[x, z]| IVec2::new(x, z))
    }

    /// Applies a shortcut, laying built cubes at once, unchecked, as the yard builds.
    /// Returns whether it changed anything.
    pub fn apply(&mut self, op: BuildOp) -> bool {
        let mut changed = false;
        match op {
            BuildOp::Fill { min, max, cube } => {
                for_block(min, max, |at| changed |= self.set(at, cube));
            }
            BuildOp::SetWall { tile, panel, kind } => {
                changed |= self.set_wall(IVec3::from_array(tile), panel, kind);
            }
            BuildOp::SetFloor {
                min,
                max,
                laid,
                kind,
            } => {
                let cube = laid.then_some(Cube::new(kind, Fill::Solid));
                for_block(min, max, |[x, storey, z]| {
                    changed |= self.set([x, floor_layer(storey), z], cube)
                });
            }
            BuildOp::SetRoof {
                min,
                max,
                laid,
                kind,
            } => {
                let cube = laid.then_some(Cube::new(kind, Fill::Solid));
                for_block(min, max, |[x, storey, z]| {
                    changed |= self.set([x, floor_layer(storey + 1), z], cube)
                });
            }
            BuildOp::AddRoof(section) => {
                // It takes the place of a flat roof under it, but not of the walls' tops.
                for_block(section.min, section.max, |[x, storey, z]| {
                    if !self.walls.contains_key(&[x, storey, z]) {
                        changed |= self.set([x, floor_layer(storey + 1), z], None);
                    }
                });
                if !self.roofs.values().any(|roof| roof.section == section) {
                    let mut roof = Roof::planned(section, None, Bill::default());
                    roof.stages.fill(ROOF_STAGES);
                    self.roofs.insert(self.next_roof(), roof);
                    changed = true;
                }
            }
            BuildOp::AddRoom { min, max, kind } => {
                changed |= self.add_room(IVec3::from_array(min), IVec3::from_array(max), kind)
            }
        }
        if changed {
            self.settle();
        }
        changed
    }

    /// A shortcut's cube: built, whole and in no part, whatever it was given.
    fn set(&mut self, at: [i32; 3], cube: Option<Cube>) -> bool {
        let cube = cube.map(|cube| Cube {
            part: None,
            pending: Pending::None,
            ..cube
        });
        self.put(IVec3::from_array(at), cube)
    }

    /// Sets a cube, keeping count of what its part is in. Returns whether it changed.
    fn put(&mut self, at: IVec3, cube: Option<Cube>) -> bool {
        let old = self
            .cubes
            .set(at, cube)
            .expect("room for its part was checked");
        if old == cube {
            return false;
        }
        if let Some(part) = old.and_then(|old| old.part) {
            self.parts.entry(part).and_modify(|part| part.uses -= 1);
        }
        if let Some(part) = cube.and_then(|cube| cube.part) {
            self.parts.entry(part).and_modify(|part| part.uses += 1);
        }
        if cube.is_none_or(|cube| cube.pending != Pending::Replace) {
            self.set_replacing(at, None);
        }
        true
    }

    /// Sets what a cube changes to, keeping count of what its part is in.
    fn set_replacing(&mut self, at: IVec3, to: Option<Replacement>) {
        let old = match to {
            Some(to) => self.replacing.insert(at.to_array(), to),
            None => self.replacing.remove(&at.to_array()),
        };
        if let Some(part) = old.and_then(|(_, part)| part) {
            self.parts.entry(part).and_modify(|part| part.uses -= 1);
        }
        if let Some(part) = to.and_then(|(_, part)| part) {
            self.parts.entry(part).and_modify(|part| part.uses += 1);
        }
    }

    /// Works out what follows from a change: parts no cube is in go, the ground under
    /// columns with no cubes is forgotten, and the walls, rooms and roofs are worked out.
    fn settle(&mut self) {
        self.parts.retain(|_, part| part.uses > 0);
        self.work_out();
        let columns = &self.columns;
        let gone: Vec<[i32; 2]> = self
            .ground
            .samples()
            .map(|sample| sample.column)
            .filter(|column| !columns.contains_key(column))
            .collect();
        for column in gone {
            self.ground.set(column, None);
        }
        self.sections = self.roofs.values().map(|roof| roof.section).collect();
    }

    /// A wall tile's cubes as a panel says, or none. The top layer is left to a floor or
    /// roof there.
    fn set_wall(&mut self, tile: IVec3, panel: Option<Panel>, kind: Kind) -> bool {
        let base = wall_layer(tile.y);
        let mut changed = false;
        for step in 0..STOREY_CUBES {
            let at = [tile.x, base + step, tile.z];
            let top = step == STOREY_CUBES - 1;
            let cube = panel.map(|panel| Cube::new(kind, panel.fill(step)));
            // The top is the wall's, unless a floor or roof is laid there and the wall is
            // only being changed.
            if top
                && self.cubes.get(IVec3::from_array(at)).is_some()
                && !self.walls.contains_key(&tile.to_array())
            {
                continue;
            }
            changed |= self.set(at, cube);
        }
        self.work_out();
        changed
    }

    fn add_room(&mut self, min: IVec3, max: IVec3, kind: Kind) -> bool {
        let was = self.rooms.clone();
        let was_room = |tile: IVec3| was.contains_key(&tile.to_array());
        let edge =
            |tile: IVec3| tile.x == min.x || tile.x == max.x || tile.z == min.z || tile.z == max.z;
        let inside = |tile: IVec3| tile.cmpge(min).all() && tile.cmple(max).all() && !edge(tile);
        let solid = Cube::new(kind, Fill::Solid);
        let mut changed = false;
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                for z in min.z..=max.z {
                    let tile = IVec3::new(x, y, z);
                    let wall = self.walls.contains_key(&tile.to_array());
                    // A floor under all of it, the walls too, unless the top of a wall
                    // below is there.
                    if self.cubes.get(IVec3::new(x, floor_layer(y), z)).is_none() {
                        changed |= self.set([x, floor_layer(y), z], Some(solid));
                    }
                    if inside(tile) {
                        // Inside: open, but for a wall that already stood between two rooms.
                        let between_rooms = wall
                            && LEVEL.iter().any(|side| {
                                was_room(tile + side.offset()) && was_room(tile - side.offset())
                            });
                        if wall && !between_rooms {
                            changed |= self.set_wall(tile, None, kind);
                        }
                        changed |= self.set([x, floor_layer(y + 1), z], Some(solid));
                    } else if was_room(tile) {
                        // Across a room already there: it is extended, not walled.
                    } else if wall {
                        // A wall already there between this room and another opens.
                        let opens = LEVEL.iter().any(|side| {
                            inside(tile + side.offset()) && was_room(tile - side.offset())
                        });
                        if opens {
                            changed |= self.set_wall(tile, None, kind);
                        }
                    } else {
                        changed |= self.set_wall(tile, Some(Panel::Solid), kind);
                    }
                }
            }
        }
        changed
    }

    /// Works out the wall tiles, the columns and the rooms from the cubes.
    fn work_out(&mut self) {
        self.columns.clear();
        // A wall tile has cubes below its top layer.
        let mut tiles: BTreeMap<[i32; 3], Vec<Option<Fill>>> = BTreeMap::new();
        for (at, cube) in self.cubes.iter() {
            self.columns.entry([at.x, at.z]).or_default().insert(at.y);
            let storey = storey_of(at.y);
            let step = at.y - wall_layer(storey);
            if step < STOREY_CUBES - 1 {
                tiles
                    .entry([at.x, storey, at.z])
                    .or_insert_with(|| vec![None; STOREY_CUBES as usize])[step as usize] =
                    Some(cube.fill);
            }
        }
        self.walls = tiles
            .into_iter()
            .map(|(tile, fills)| (tile, panel_of(&fills)))
            .collect();
        self.find_rooms();
    }

    /// What fills a wall tile. `None` where there is no wall.
    pub fn filling(&self, tile: IVec3) -> Option<Panel> {
        self.walls.get(&tile.to_array()).copied()
    }

    /// What fills a wall tile, or solid where there is none.
    pub fn panel(&self, tile: IVec3) -> Panel {
        self.filling(tile).unwrap_or(Panel::Solid)
    }

    /// The kind of a wall tile: of its lowest cube.
    pub fn kind(&self, tile: IVec3) -> Option<Kind> {
        let base = wall_layer(tile.y);
        (base..base + STOREY_CUBES)
            .find_map(|layer| self.cube(IVec3::new(tile.x, layer, tile.z)))
            .map(|cube| cube.kind)
    }

    /// The way a wall tile's wall runs along the floor: east and west where it has wall
    /// either side that way, else north and south where it has, else east and west.
    pub fn along(&self, tile: IVec3) -> IVec3 {
        let wall = |step: IVec3| self.walls.contains_key(&(tile + step).to_array());
        if wall(IVec3::X) || wall(IVec3::NEG_X) {
            IVec3::X
        } else if wall(IVec3::Z) || wall(IVec3::NEG_Z) {
            IVec3::Z
        } else {
            IVec3::X
        }
    }

    /// The window or doorway a wall tile is part of: every tile in a line with it, along
    /// its wall, that has the same filling, in order. Just the tile itself if it is solid,
    /// and nothing if there is no wall there.
    pub fn run(&self, tile: IVec3) -> Vec<IVec3> {
        let Some(panel) = self.filling(tile) else {
            return Vec::new();
        };
        if panel == Panel::Solid {
            return vec![tile];
        }
        let along = self.along(tile);
        let same = |at: IVec3| self.filling(at) == Some(panel);
        let mut first = tile;
        while same(first - along) {
            first -= along;
        }
        let mut found = Vec::new();
        let mut at = first;
        while same(at) {
            found.push(at);
            at += along;
        }
        found
    }

    /// The first cube a ray meets, from any side, if any; openings are passed through.
    /// `origin` and `direction` are in the base's own space, in metres; `direction` is
    /// unit length.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<BuildHit> {
        self.cubes
            .iter()
            .filter(|(_, cube)| cube.fill != Fill::Open)
            .filter_map(|(at, _)| {
                let low = at.as_vec3() * CELL_M;
                let (distance, normal) =
                    ray_box(origin, direction, low, low + Vec3::splat(CELL_M))?;
                (distance <= max_distance).then_some((at, distance, normal))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(cube, distance, normal)| {
                let wall = self.in_wall(cube);
                let face = if wall {
                    Face::new(
                        IVec3::new(cube.x, storey_of(cube.y), cube.z),
                        Side::of(normal),
                    )
                } else if normal.y >= 0 {
                    Face::new(
                        IVec3::new(cube.x, storey_of(cube.y + 1), cube.z),
                        if normal.y > 0 {
                            Side::Down
                        } else {
                            Side::of(normal)
                        },
                    )
                } else {
                    Face::new(
                        IVec3::new(cube.x, storey_of(cube.y + 1) - 1, cube.z),
                        Side::Up,
                    )
                };
                BuildHit {
                    cube,
                    face,
                    distance,
                    wall,
                }
            })
    }

    /// The rectangle up the middle of a wall tile, along the way its wall runs, in the
    /// base's own space: low-left, low-right, high-right, high-left as seen from the side
    /// it faces, south or east.
    pub fn corners(&self, tile: IVec3) -> [Vec3; 4] {
        corners(tile, self.along(tile))
    }

    /// The base as triangles, ready to draw: walls of a kind with a kit are left to the
    /// kit, which `kit` says.
    pub fn mesh(&self, kit: impl Fn(Kind) -> Option<mesh::KitLook>) -> BaseMesh {
        mesh::build(self, kit)
    }

    /// The base with its log walls laid as logs: everything else as triangles, and the
    /// logs, in the order they go in, to draw as they stand.
    pub fn mesh_with_kits(&self, kits: &logs::Kits) -> (BaseMesh, Vec<logs::Layout>) {
        let mesh = mesh::build(self, |kind| {
            kits.of(kind).map(|kit| mesh::KitLook {
                gable: kit.colours.board,
                floor: kit.colours.light,
                roof: kit.colours.roof,
                side: kit.colours.dark,
                eaves: kit.eaves,
                textures: kit
                    .textures
                    .iter()
                    .map(|(material, texture)| (*material, texture.metres))
                    .collect(),
            })
        });
        let layouts = kits.0.iter().map(|kit| logs::lay(self, kit)).collect();
        (mesh, layouts)
    }
}

pub use mesh::{KitLook, Material, Surfaces};

/// The rectangle up the middle of a wall tile running along `along`, in the base's own
/// space: low-left, low-right, high-right, high-left as seen from the south or east.
pub fn corners(tile: IVec3, along: IVec3) -> [Vec3; 4] {
    let low = Vec3::new(
        tile.x as f32,
        tile.y as f32 * STOREY_CUBES as f32,
        tile.z as f32,
    ) * CELL_M;
    let middle = low + Vec3::new(CELL_M, 0.0, CELL_M) * 0.5;
    let (from, right) = if along == IVec3::X {
        (Vec3::new(low.x, low.y, middle.z), Vec3::X)
    } else {
        (Vec3::new(middle.x, low.y, low.z + CELL_M), Vec3::NEG_Z)
    };
    let up = Vec3::Y * STOREY_M;
    [
        from,
        from + right * CELL_M,
        from + right * CELL_M + up,
        from + up,
    ]
}

/// What a wall tile's cubes, from its foot up, read as.
fn panel_of(fills: &[Option<Fill>]) -> Panel {
    let layers = |fill: Fill| -> Vec<i32> {
        (0..fills.len() as i32)
            .filter(|&i| fills[i as usize] == Some(fill))
            .collect()
    };
    let (open, glass) = (layers(Fill::Open), layers(Fill::Glass));
    if !open.is_empty() && open[0] == 0 {
        return Panel::Door;
    }
    match (glass.first(), glass.last()) {
        (Some(&first), Some(&last)) => Panel::Window {
            sill: first as f32 * CELL_M,
            top: STOREY_M - (last + 1) as f32 * CELL_M,
        },
        _ => Panel::Solid,
    }
}

fn for_block(min: [i32; 3], max: [i32; 3], mut each: impl FnMut([i32; 3])) {
    for x in min[0]..=max[0] {
        for y in min[1]..=max[1] {
            for z in min[2]..=max[2] {
                each([x, y, z]);
            }
        }
    }
}

/// Where a ray enters a box, and the side of the box it enters by, if it does.
fn ray_box(origin: Vec3, direction: Vec3, low: Vec3, high: Vec3) -> Option<(f32, IVec3)> {
    let mut near = f32::MIN;
    let mut far = f32::MAX;
    let mut normal = IVec3::ZERO;
    for axis in 0..3 {
        if direction[axis].abs() < 1e-9 {
            if origin[axis] < low[axis] || origin[axis] > high[axis] {
                return None;
            }
            continue;
        }
        let (a, b) = (
            (low[axis] - origin[axis]) / direction[axis],
            (high[axis] - origin[axis]) / direction[axis],
        );
        let (enter, leave) = (a.min(b), a.max(b));
        if enter > near {
            near = enter;
            normal = IVec3::ZERO;
            normal[axis] = if direction[axis] > 0.0 { -1 } else { 1 };
        }
        far = far.min(leave);
    }
    (near <= far && near > 0.0).then_some((near, normal))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_columns_a_base_stands_on_are_where_its_tiles_say() {
        let mut base = Base::default();
        for op in crate::example(Kind::PLAIN) {
            base.apply(op);
        }
        let ground: std::collections::HashSet<IVec2> = base.ground_columns().collect();
        let standing: std::collections::HashSet<IVec2> = base.occupied_columns().collect();
        assert!(!ground.is_empty() && !standing.is_empty());
        for x in -40..60 {
            for z in -40..40 {
                let tile = IVec3::new(x, 0, z);
                let at = IVec2::new(x, z);
                assert_eq!(
                    ground.contains(&at),
                    base.contains(tile) || base.has_floor(tile)
                );
                assert_eq!(standing.contains(&at), base.occupied(x, z));
            }
        }
    }

    const LOGS: Kind = Kind(1);

    /// A room four tiles by three outside, two by one inside, by the shortcut.
    fn room() -> Base {
        let mut base = Base::default();
        assert!(base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [3, 0, 2],
            kind: LOGS,
        }));
        base
    }

    #[test]
    fn the_room_tool_lays_walls_round_a_floor_and_a_roof() {
        let base = room();
        assert_eq!(base.walls().count(), 10);
        assert_eq!(base.tile_count(), 2);
        assert!(base.contains(IVec3::new(1, 0, 1)) && base.contains(IVec3::new(2, 0, 1)));
        // A floor under all twelve tiles, and the roof over the two inside, which would be
        // the floor of a storey over them.
        assert_eq!(base.floors().count(), 14);
        assert!(base.has_roof(IVec3::new(1, 0, 1)) && !base.has_roof(IVec3::new(0, 0, 0)));
        // A wall is six cubes high, on the floor, the layer under it.
        assert_eq!(
            base.column(0, 0).collect::<Vec<_>>(),
            vec![-1, 0, 1, 2, 3, 4, 5]
        );
        assert_eq!(base.column(1, 1).collect::<Vec<_>>(), vec![-1, 5]);
        assert!(base.exterior(IVec3::new(1, 0, 2)));
        assert_eq!(base.outward(IVec3::new(1, 0, 2)), Some(IVec3::Z));
        assert_eq!(base.kind(IVec3::new(0, 0, 0)), Some(LOGS));
    }

    #[test]
    fn a_doorway_still_closes_a_room_in_and_a_gap_does_not() {
        let mut base = room();
        let door = IVec3::new(1, 0, 2);
        base.apply(BuildOp::SetWall {
            tile: door.to_array(),
            panel: Some(Panel::Door),
            kind: LOGS,
        });
        assert_eq!(base.filling(door), Some(Panel::Door));
        assert_eq!(base.tile_count(), 2);
        // Its cubes up to two metres are open, and the rest still wall.
        assert_eq!(base.cube(IVec3::new(1, 3, 2)).unwrap().fill, Fill::Open);
        assert_eq!(base.cube(IVec3::new(1, 4, 2)).unwrap().fill, Fill::Solid);
        base.apply(BuildOp::SetWall {
            tile: door.to_array(),
            panel: None,
            kind: LOGS,
        });
        assert_eq!(base.tile_count(), 0);
    }

    #[test]
    fn a_window_is_glass_between_its_sill_and_head_and_reads_back() {
        let mut base = room();
        let tile = IVec3::new(0, 0, 1);
        base.apply(BuildOp::SetWall {
            tile: tile.to_array(),
            panel: Some(Panel::WINDOW),
            kind: LOGS,
        });
        assert_eq!(base.filling(tile), Some(Panel::WINDOW));
        let fills: Vec<Fill> = (0..6)
            .map(|layer| base.cube(IVec3::new(0, layer, 1)).unwrap().fill)
            .collect();
        assert_eq!(
            fills,
            [
                Fill::Solid,
                Fill::Solid,
                Fill::Glass,
                Fill::Glass,
                Fill::Solid,
                Fill::Solid
            ]
        );
    }

    #[test]
    fn cubes_carved_out_of_a_wall_are_an_opening_of_any_shape() {
        let mut base = room();
        // One cube, a metre up, opened.
        base.apply(BuildOp::Fill {
            min: [0, 2, 1],
            max: [0, 2, 1],
            cube: Some(Cube::new(LOGS, Fill::Open)),
        });
        assert!(base.filling(IVec3::new(0, 0, 1)).is_some());
        assert_eq!(base.tile_count(), 2);
    }

    #[test]
    fn rooms_that_touch_open_into_each_other() {
        let mut base = room();
        // Another sharing the east wall: the wall between is taken away.
        base.apply(BuildOp::AddRoom {
            min: [3, 0, 0],
            max: [6, 0, 2],
            kind: LOGS,
        });
        assert_eq!(base.filling(IVec3::new(3, 0, 1)), None);
        assert_eq!(
            base.room(IVec3::new(1, 0, 1)),
            base.room(IVec3::new(5, 0, 1))
        );
    }

    #[test]
    fn a_wall_across_a_room_makes_two() {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [6, 0, 3],
            kind: LOGS,
        });
        let rooms = |base: &Base| base.tiles().map(|(_, r)| r).collect::<BTreeSet<_>>().len();
        assert_eq!(rooms(&base), 1);
        for z in 1..3 {
            base.apply(BuildOp::SetWall {
                tile: [3, 0, z],
                panel: Some(Panel::Solid),
                kind: LOGS,
            });
        }
        assert_eq!(rooms(&base), 2);
    }

    #[test]
    fn a_room_drawn_on_top_of_another_stands_on_it() {
        let mut base = room();
        base.apply(BuildOp::AddRoom {
            min: [0, 1, 0],
            max: [3, 1, 2],
            kind: LOGS,
        });
        assert!(base.contains(IVec3::new(1, 1, 1)));
        // The roof below is the floor above: one layer of cubes.
        assert!(base.has_roof(IVec3::new(1, 0, 1)) && base.has_floor(IVec3::new(1, 1, 1)));
        assert_eq!(base.column(0, 0).count(), 13);
    }

    #[test]
    fn a_ray_finds_the_wall_it_points_at_and_passes_through_a_doorway() {
        let mut base = room();
        let hit = base
            .raycast(Vec3::new(1.25, 1.0, 5.0), Vec3::NEG_Z, 20.0)
            .unwrap();
        assert!(hit.wall);
        assert_eq!(hit.face, Face::new(IVec3::new(2, 0, 2), Side::South));
        assert!((hit.distance - 3.5).abs() < 1e-4);
        base.apply(BuildOp::SetWall {
            tile: [2, 0, 2],
            panel: Some(Panel::Door),
            kind: LOGS,
        });
        let hit = base
            .raycast(Vec3::new(1.25, 1.0, 5.0), Vec3::NEG_Z, 20.0)
            .unwrap();
        assert_eq!(hit.face.tile(), IVec3::new(2, 0, 0));
        // Looking down at a floor.
        let hit = base
            .raycast(Vec3::new(0.75, 2.0, 0.75), Vec3::NEG_Y, 20.0)
            .unwrap();
        assert_eq!(hit.face, Face::new(IVec3::new(1, 0, 1), Side::Down));
    }

    #[test]
    fn the_same_operations_build_the_same_base() {
        let ops = [
            BuildOp::AddRoom {
                min: [0, 0, 0],
                max: [5, 0, 4],
                kind: LOGS,
            },
            BuildOp::SetWall {
                tile: [2, 0, 4],
                panel: Some(Panel::Door),
                kind: LOGS,
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
    }

    #[test]
    fn neighbouring_tiles_with_the_same_opening_are_one_run() {
        let mut base = room();
        for x in 1..3 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 2],
                panel: Some(Panel::WINDOW),
                kind: LOGS,
            });
        }
        assert_eq!(
            base.run(IVec3::new(2, 0, 2)),
            vec![IVec3::new(1, 0, 2), IVec3::new(2, 0, 2)]
        );
        assert_eq!(base.along(IVec3::new(2, 0, 2)), IVec3::X);
        assert_eq!(base.along(IVec3::new(0, 0, 1)), IVec3::Z);
        assert_eq!(base.run(IVec3::new(0, 0, 1)), vec![IVec3::new(0, 0, 1)]);
        assert!(base.run(IVec3::new(1, 0, 1)).is_empty());
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
        let hip = section(RoofStyle::Hip, true);
        assert_eq!(hip.rise(at(0.0, 1.0)), Some(0.0));
        assert_eq!(hip.rise(at(2.0, 1.0)), Some(0.5));
        assert_eq!(hip.rise(at(5.0, 1.0)), None);
        let gable = section(RoofStyle::Gable, true);
        assert_eq!(gable.rise(at(0.0, 1.0)), Some(0.5));
        let lean = section(RoofStyle::LeanTo, true);
        assert_eq!(lean.rise(at(1.0, 2.0)), Some(1.0));
    }

    #[test]
    fn a_pitched_roof_takes_the_place_of_a_flat_one_and_covers_the_room() {
        let mut base = room();
        assert!(base.apply(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [3, 0, 2],
            style: RoofStyle::Hip,
            along_x: true,
            pitch: 0.7,
        })));
        assert!(!base.has_roof(IVec3::new(1, 0, 1)));
        assert!(base.covered(IVec3::new(1, 0, 1)));
        assert!(!base.covered(IVec3::new(1, 1, 1)));
        // The walls keep their tops.
        assert_eq!(base.column(0, 0).count(), 7);
    }
}
