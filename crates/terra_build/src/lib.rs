//! Construction: everything built is half-metre cubes on a base's grid (D48, D181;
//! docs/building-tech.md, docs/building-plan.md).
//!
//! - **A cube is air or a kind of construction** ([`Kind`]), with a shape ([`Shape`]),
//!   planned or built to a fill level ([`State`], D194), at most one change pending on it,
//!   and the part it was laid in, packed in 32 bits in sections of 16 a side
//!   ([`cubes`]). What fills it ([`Fill`]) is solid or open: an opening cut in a
//!   wall, a doorway or a window, part of the wall still, which lets a walker through.
//! - **A wall is a row of cubes**, a storey high unless built otherwise; a post is a
//!   column of them, a floor a layer. So walls meet, cross, are carved and mix kinds by
//!   sharing cubes.
//! - **A storey is six cubes, three metres**, but only as the quick tools' default height
//!   (D176, D277): what a cube is, wall or floor, and the rooms, are read from the cubes
//!   alone. The quick tools lay a storey's floor as the layer just under it and its walls
//!   six cubes high, and read a column of six cubes as a wall *tile*.
//! - **Rooms are worked out** from the cubes, the space they close in all round ([`rooms`]).
//! - **A roof is cubes too** (D241): cubes of a roof kind ([`Kind::is_roof`]) cut to
//!   slopes and corners, laid by hand or by the roofer ([`roofer`]) and drawn as a roof
//!   from their neighbours (`layout::roofs`): no wall tile, floor or room of their own.
//! - **Doors and shutters are props** ([`Prop`]) fitted into holes in built walls, their
//!   leaves hung on the hole's sides ([`fitting`]).
//! - **Cubes take damage and are repaired** ([`loss`], D193): one destroyed, what then
//!   hangs from nothing comes down, bursting into fragments as the game draws it.
//! - **Everything a player does is an operation** ([`ops::Op`]), checked by one pure
//!   function against the state when it arrives, and undone by its inverse: nothing
//!   floats, and nothing is taken while something rests on it ([`support`], D193).
//! - **Shortcuts** ([`BuildOp`]) lay built cubes as a player means them, instantly, as
//!   the yard and the game do today: a wall tile as a panel says, solid or with a window
//!   or doorway of a default size ([`BuildOp::SetWall`]); a floor or flat roof over a
//!   block; and the room tool, walls round a footprint with a floor inside and a flat roof
//!   over it.
//!
//! A base is saved and sent as a snapshot ([`save`]), and before that as the list of
//! shortcuts that made it (D21), which is still read.
//!
//! Coordinates are local to the base: X and Z along the floor and Y up. Cubes are counted
//! in cubes all three ways; tiles count X and Z in cubes and Y in storeys.

pub mod castle;
pub mod cells;
pub mod checker;
pub mod cost;
pub mod cubes;
pub mod draw;
pub mod dress;
mod example;
pub mod fitting;
pub mod fragments;
pub mod ground;
pub mod hull;
pub mod inspect;
pub mod kit;
pub mod layers;
pub mod layout;
pub mod loss;
mod mesh;
pub mod modules;
pub mod ops;
pub mod order;
pub mod packs;
pub mod parts;
pub mod plans;
pub mod put_up;
pub mod qa_cabin;
pub mod relief;
pub mod roofer;
pub mod roofing;
pub mod rooms;
pub mod save;
pub mod sections;
pub mod support;
pub mod surface;
pub mod timber;
pub mod town;
pub mod village;
mod walk;

pub use cost::{Amount, Bill, PropKind, Rules, Terms, Toughness};
pub use cubes::{Cube, Cubes, Facing, Fill, Kind, PartId, Pending, Replacement, Shape, State};
pub use example::{
    RoofParts, VILLAGE_HOUSES, bench_village, example, plan as roof_plan, roof_over, roof_rows,
    with_roof,
};
pub use ground::{Ground, Sample};
pub use mesh::{BaseMesh, Indexed, Surface};
pub use order::Progress;
pub use walk::Walker;

use glam::{IVec2, IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

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
    /// Open from `sill` metres above the floor to `top` metres below the ceiling, a hole
    /// framed by its jambs, sill and lintel (D215, D225). Neighbouring tiles with the same
    /// sill and head are one window.
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
    /// Open from half a metre over the floor to half a metre under the ceiling.
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
            Panel::Window { sill, top: below } if within(sill, STOREY_M - below) => Fill::Open,
            _ => Fill::Solid,
        }
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

/// A door or shutters fitted into a hole in a wall (construction-001, gap 15), filling
/// the cubes from `min` to `max`, and how it hangs there ([`fitting`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Prop {
    pub kind: PropKind,
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub open: bool,
    /// The way its leaves swing open, across the wall, as it was hung: into the room for
    /// a door, out for shutters. None given, they open south or east.
    #[serde(default)]
    pub opens: [i32; 3],
    /// Whether it hangs as a door or as shutters.
    #[serde(default)]
    pub hang: fitting::Hang,
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

/// A base: its cubes, parts, props and the ground under them, and what is worked out from
/// them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Base {
    cubes: Cubes,
    /// The parts its cubes were laid in.
    parts: BTreeMap<PartId, Part>,
    /// For each cube whose kind is to change, the kind and part it changes to.
    replacing: BTreeMap<[i32; 3], Replacement>,
    /// Fitted props, by their lowest cube.
    props: BTreeMap<[i32; 3], Prop>,
    /// What was paid for each fitted prop, which taking it out gives back.
    prop_paid: BTreeMap<[i32; 3], Bill>,
    /// How much damage each cube with anything in it has taken, where it has taken any
    /// (D193, `loss`).
    damage: BTreeMap<[i32; 3], u16>,
    ground: Ground,
    /// Each column of cubes, by where it stands, and its layers. Each column's, part's
    /// and section's own are shared (`Arc`) and copied only when changed, so a base is
    /// copied for the workers in a few thousand steps, not hundreds of thousands: a
    /// castle's copy took 11 ms of the main thread an edit (D308).
    columns: BTreeMap<[i32; 2], Arc<BTreeSet<i32>>>,
    /// The cubes of each part, and every cube with work pending on it, kept as cubes are
    /// put, so what goes in next is found among them rather than among every cube: a
    /// castle's hundred and forty thousand were walked several times an edit (D308).
    /// A part's cubes are kept a slab of 16 along x at a time, so a change copies one slab
    /// of a part shared with a copy, not the whole of a castle's wall; slabs in order of x
    /// keep the part's cubes in order of `[x, y, z]`.
    in_parts: BTreeMap<(PartId, i32), Arc<BTreeSet<[i32; 3]>>>,
    /// Each part's cubes not yet built, which what goes in next is among, in slabs too.
    unbuilt: BTreeMap<(PartId, i32), Arc<BTreeSet<[i32; 3]>>>,
    pending: BTreeSet<[i32; 3]>,
    /// How many cubes of each kind it has, so which kits lay anything is known at once.
    kinds: BTreeMap<Kind, usize>,
    /// Every cube closed in, and which room it is in, worked out when asked (D261).
    rooms: rooms::Rooms,
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
    /// The way out of the cube's face that was met: the cube beyond it is where something
    /// set against it goes.
    pub normal: IVec3,
}

const LEVEL: [Side; 4] = [Side::East, Side::West, Side::South, Side::North];

impl Base {
    /// About how many bytes it takes in memory: its cubes, and what is worked out from them
    /// a node of a map each (its columns and their layers, parts, props). Rooms are
    /// worked out when asked and not counted.
    pub fn bytes(&self) -> usize {
        const NODE: usize = 48;
        let layers: usize = self.columns.values().map(|layers| layers.len()).sum();
        self.cubes.bytes()
            + (self.columns.len()
                + layers
                + self.parts.len()
                + self.replacing.len()
                + self.props.len()
                + self.damage.len())
                * NODE
    }

    pub fn is_empty(&self) -> bool {
        self.cubes.is_empty() && self.props.is_empty()
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

    /// Whether a cube is part of a wall, rather than a floor or roof, from the cubes alone
    /// (D277): it stands in a column of cubes, with one whole, a post or an opening over it
    /// or under it, not of a roof. A floor is a layer a cube thick, which runs on under the
    /// foot of a wall standing on it: a cube with nothing under it beside a cube of floor is
    /// floor. A cube alone in its column is a wall's first course where it rests on the
    /// ground and is no slab, and a walkway in the air. A stair or a slope set on a floor
    /// leaves it a floor.
    pub fn in_wall(&self, at: IVec3) -> bool {
        let stands = |at: IVec3| self.cube(at).is_some_and(|cube| !cube.kind.is_roof());
        let standing = |at: IVec3| {
            self.cube(at).is_some_and(|cube| {
                !cube.kind.is_roof()
                    && (cube.fill == Fill::Open || matches!(cube.shape, Shape::Whole | Shape::Post))
            })
        };
        if !stands(at) {
            return false;
        }
        let (over, under) = (standing(at + IVec3::Y), standing(at - IVec3::Y));
        if under {
            return true;
        }
        // Nothing under it: a wall's foot, unless a floor beside it, or corner to corner
        // with it, runs on under it.
        let floor = |beside: IVec3| {
            stands(beside) && !standing(beside + IVec3::Y) && !standing(beside - IVec3::Y)
        };
        let around = (-1..=1).flat_map(|x| (-1..=1).map(move |z| IVec3::new(x, 0, z)));
        if over {
            return !around
                .filter(|&step| step != IVec3::ZERO)
                .any(|step| floor(at + step));
        }
        // Alone in its column: a wall's first course where it rests on the ground and is
        // no slab, two by two of cubes on its layer, as a floor on the ground is; up in
        // the air, a walkway or a deck.
        let slab = [(1, 1), (1, -1), (-1, 1), (-1, -1)].iter().any(|&(x, z)| {
            stands(at + IVec3::new(x, 0, 0))
                && stands(at + IVec3::new(0, 0, z))
                && stands(at + IVec3::new(x, 0, z))
        });
        self.grounded(at) && !slab
    }

    /// The way the wall a cube is in runs along the floor, from the cubes beside it: east
    /// and west where a cube of a wall is beside it that way, else north and south where
    /// one is, else east and west.
    pub fn runs_along(&self, at: IVec3) -> IVec3 {
        let wall = |step: IVec3| self.in_wall(at + step);
        if wall(IVec3::X) || wall(IVec3::NEG_X) {
            IVec3::X
        } else if wall(IVec3::Z) || wall(IVec3::NEG_Z) {
            IVec3::Z
        } else {
            IVec3::X
        }
    }

    /// Whether a tile has a floor under it: a cube at its floor's layer, not of a roof.
    pub fn has_floor(&self, tile: IVec3) -> bool {
        let at = IVec3::new(tile.x, floor_layer(tile.y), tile.z);
        self.solid(at) && self.cube(at).is_some_and(|cube| !cube.kind.is_roof())
    }

    /// Whether a tile that is not a wall has a flat roof over it, or a floor of the
    /// storey above.
    pub fn has_roof(&self, tile: IVec3) -> bool {
        self.filling(tile).is_none() && self.has_floor(tile + IVec3::Y)
    }

    /// Whether a tile has any roof over it: a flat one, or a roof's cubes over its column,
    /// from the top of its walls up to the floor of the storey over it, or two storeys up
    /// where there is none (D241). `standing` asks only for roof cubes with something in.
    pub fn covered(&self, tile: IVec3, standing: bool) -> bool {
        if self.has_roof(tile) {
            return true;
        }
        let from = wall_layer(tile.y) + STOREY_CUBES - 1;
        self.column(tile.x, tile.z)
            .filter(|&y| y >= from && y < from + 2 * STOREY_CUBES)
            .filter_map(|y| self.cube(IVec3::new(tile.x, y, tile.z)))
            .any(|cube| cube.kind.is_roof() && (!standing || cube.is_in()))
    }

    /// Every tile with a floor under it that is not the top of a wall below, in a fixed
    /// order: a flat roof is the floor of the storey over it.
    pub fn floors(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.cubes
            .iter()
            .filter(|(at, cube)| (at.y + 1).rem_euclid(STOREY_CUBES) == 0 && !cube.kind.is_roof())
            .map(|(at, _)| IVec3::new(at.x, storey_of(at.y + 1), at.z))
            .filter(|tile| self.filling(*tile - IVec3::Y).is_none())
    }

    /// Every wall tile and what fills it, in a fixed order: the quick tools' view of the
    /// cubes, a storey's column at its default height ([`Base::filling`]).
    pub fn walls(&self) -> impl Iterator<Item = (IVec3, Panel)> + '_ {
        let tiles: BTreeSet<[i32; 3]> = self
            .cubes
            .iter()
            .filter(|(at, cube)| {
                !cube.kind.is_roof() && at.y - wall_layer(storey_of(at.y)) < STOREY_CUBES - 1
            })
            .map(|(at, _)| [at.x, storey_of(at.y), at.z])
            .collect();
        tiles.into_iter().filter_map(|tile| {
            let tile = IVec3::from_array(tile);
            self.filling(tile).map(|panel| (tile, panel))
        })
    }

    /// The layers of the cubes standing at a place, counted in cubes along the floor.
    pub fn column(&self, x: i32, z: i32) -> impl Iterator<Item = i32> + '_ {
        self.columns
            .get(&[x, z])
            .into_iter()
            .flat_map(|layers| layers.iter())
            .copied()
    }

    fn solid(&self, at: IVec3) -> bool {
        self.cube(at).is_some_and(|cube| cube.fill != Fill::Open)
    }

    /// A base of these cubes, with its walls and rooms worked out.
    fn from_cubes(cubes: Cubes) -> Base {
        let mut base = Base {
            cubes,
            ..Base::default()
        };
        base.count_columns();
        base.settle();
        base
    }

    /// Whether anything stands at a place, counted in cubes along the floor.
    pub fn occupied(&self, x: i32, z: i32) -> bool {
        self.columns.contains_key(&[x, z])
    }

    /// Every place along the floor that is in a room at the ground storey's foot, or has a
    /// floor there. In no fixed order.
    pub fn ground_columns(&self) -> impl Iterator<Item = IVec2> + '_ {
        let rooms = self
            .room_cells()
            .filter(|(at, _)| at.y == wall_layer(0))
            .map(|(at, _)| IVec2::new(at.x, at.z));
        // Each column asked for its cube at the ground floor's layer, not every cube.
        let floors = self.columns.keys().filter_map(|&[x, z]| {
            self.cube(IVec3::new(x, floor_layer(0), z))
                .is_some_and(|cube| cube.fill != Fill::Open)
                .then_some(IVec2::new(x, z))
        });
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
            BuildOp::AddRoom { min, max, kind } => {
                changed |= self.add_room(IVec3::from_array(min), IVec3::from_array(max), kind)
            }
        }
        if changed {
            self.settle();
        }
        changed
    }

    /// Applies shortcuts in order, as [`Base::apply`] does each, but works out the walls and
    /// rooms once after a run of fills rather than after every one: what a fill lays does
    /// not depend on them, and the roofer lays a roof a cube a fill, which made a castle's
    /// roofs cost the square of its cubes. Says which of them changed anything.
    pub fn apply_shortcuts(&mut self, ops: &[BuildOp]) -> Vec<bool> {
        let mut unsettled = false;
        let mut out = Vec::with_capacity(ops.len());
        for &op in ops {
            let changed = match op {
                BuildOp::Fill { min, max, cube } => {
                    let mut changed = false;
                    for_block(min, max, |at| changed |= self.set(at, cube));
                    unsettled |= changed;
                    changed
                }
                op => {
                    if std::mem::take(&mut unsettled) {
                        self.settle();
                    }
                    self.apply(op)
                }
            };
            out.push(changed);
        }
        if unsettled {
            self.settle();
        }
        out
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
        let closes = |cube: Option<Cube>| cube.is_some_and(|cube| rooms::encloses(&cube));
        if closes(old) != closes(cube) {
            self.rooms.changed(at);
        }
        self.unmark(at, old);
        self.mark(at, cube);
        // The column's layers kept as cubes come and go, so an edit never walks them all.
        let column = [at.x, at.z];
        match (old.is_some(), cube.is_some()) {
            (false, true) => {
                Arc::make_mut(self.columns.entry(column).or_default()).insert(at.y);
            }
            (true, false) => {
                if let Some(layers) = self.columns.get_mut(&column) {
                    Arc::make_mut(layers).remove(&at.y);
                    if layers.is_empty() {
                        self.columns.remove(&column);
                    }
                }
            }
            _ => {}
        }
        if let Some(part) = old.and_then(|old| old.part) {
            self.parts.entry(part).and_modify(|part| part.uses -= 1);
        }
        if let Some(part) = cube.and_then(|cube| cube.part) {
            self.parts.entry(part).and_modify(|part| part.uses += 1);
        }
        if cube.is_none_or(|cube| !cube.pending.changing()) {
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
    /// columns with no cubes is forgotten, and the walls and rooms are worked out.
    fn settle(&mut self) {
        self.parts.retain(|_, part| part.uses > 0);
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
                && self.filling(tile).is_none()
            {
                continue;
            }
            changed |= self.set(at, cube);
        }
        changed
    }

    fn add_room(&mut self, min: IVec3, max: IVec3, kind: Kind) -> bool {
        // The tiles whose foot is in a room.
        let was: BTreeSet<[i32; 3]> = self
            .room_cells()
            .filter(|(at, _)| at.y == wall_layer(storey_of(at.y)))
            .map(|(at, _)| [at.x, storey_of(at.y), at.z])
            .collect();
        let was_room = |tile: IVec3| was.contains(&tile.to_array());
        let edge =
            |tile: IVec3| tile.x == min.x || tile.x == max.x || tile.z == min.z || tile.z == max.z;
        let inside = |tile: IVec3| tile.cmpge(min).all() && tile.cmple(max).all() && !edge(tile);
        let solid = Cube::new(kind, Fill::Solid);
        let mut changed = false;
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                for z in min.z..=max.z {
                    let tile = IVec3::new(x, y, z);
                    let wall = self.filling(tile).is_some();
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
                            // Roofed over, as the rooms either side are.
                            changed |= self.set([x, floor_layer(y + 1), z], Some(solid));
                        }
                    } else {
                        changed |= self.set_wall(tile, Some(Panel::Solid), kind);
                    }
                }
            }
        }
        changed
    }

    /// Works out the columns from every cube, for cubes come by other than being put, as
    /// from a save.
    pub(crate) fn count_columns(&mut self) {
        self.columns.clear();
        self.in_parts.clear();
        self.unbuilt.clear();
        self.pending.clear();
        self.kinds.clear();
        let cubes: Vec<(IVec3, Cube)> = self.cubes.iter().collect();
        for (at, cube) in cubes {
            Arc::make_mut(self.columns.entry([at.x, at.z]).or_default()).insert(at.y);
            self.mark(at, Some(cube));
        }
    }

    /// A cube kept among its part's and those with work pending.
    fn mark(&mut self, at: IVec3, cube: Option<Cube>) {
        let Some(cube) = cube else {
            return;
        };
        *self.kinds.entry(cube.kind).or_default() += 1;
        if let Some(part) = cube.part {
            let slab = (part, slab_of(at));
            Arc::make_mut(self.in_parts.entry(slab).or_default()).insert(at.to_array());
            if !cube.is_built() {
                Arc::make_mut(self.unbuilt.entry(slab).or_default()).insert(at.to_array());
            }
        }
        if cube.pending != Pending::None {
            self.pending.insert(at.to_array());
        }
    }

    /// A cube no longer kept among its part's or those with work pending.
    fn unmark(&mut self, at: IVec3, cube: Option<Cube>) {
        let Some(cube) = cube else {
            return;
        };
        if let Some(count) = self.kinds.get_mut(&cube.kind) {
            *count -= 1;
            if *count == 0 {
                self.kinds.remove(&cube.kind);
            }
        }
        if let Some(part) = cube.part
            && let Some(cubes) = self.in_parts.get_mut(&(part, slab_of(at)))
        {
            Arc::make_mut(cubes).remove(&at.to_array());
            if cubes.is_empty() {
                self.in_parts.remove(&(part, slab_of(at)));
            }
        }
        if let Some(part) = cube.part
            && let Some(cubes) = self.unbuilt.get_mut(&(part, slab_of(at)))
        {
            Arc::make_mut(cubes).remove(&at.to_array());
            if cubes.is_empty() {
                self.unbuilt.remove(&(part, slab_of(at)));
            }
        }
        if cube.pending != Pending::None {
            self.pending.remove(&at.to_array());
        }
    }

    /// Every kind it has a cube of, in order.
    pub fn kinds(&self) -> impl Iterator<Item = Kind> + '_ {
        self.kinds.keys().copied()
    }

    /// A part's cubes not yet built, in a fixed order: by `[x, y, z]`.
    pub fn unbuilt_of(&self, part: PartId) -> impl Iterator<Item = (IVec3, Cube)> + '_ {
        self.unbuilt
            .range((part, i32::MIN)..=(part, i32::MAX))
            .flat_map(|(_, cubes)| cubes.iter())
            .filter_map(move |&at| {
                let at = IVec3::from_array(at);
                self.cube(at)
                    .filter(|cube| cube.part == Some(part))
                    .map(|cube| (at, cube))
            })
    }

    /// Every cube with work pending on it (to be carved, or changing kind), in a fixed
    /// order: by `[x, y, z]`.
    pub fn pending(&self) -> impl Iterator<Item = (IVec3, Cube)> + '_ {
        self.pending.iter().filter_map(|&at| {
            let at = IVec3::from_array(at);
            self.cube(at).map(|cube| (at, cube))
        })
    }

    /// What fills a wall tile, read from its cubes: the quick tools' view of a storey's
    /// column at its default height, six cubes (D176, D277). `None` where no cube but a
    /// roof's stands below its top layer.
    pub fn filling(&self, tile: IVec3) -> Option<Panel> {
        let foot = wall_layer(tile.y);
        let fills: Vec<Option<Fill>> = (0..STOREY_CUBES)
            .map(|step| {
                self.cube(IVec3::new(tile.x, foot + step, tile.z))
                    .filter(|cube| step < STOREY_CUBES - 1 && !cube.kind.is_roof())
                    .map(|cube| cube.fill)
            })
            .collect();
        fills.iter().any(Option::is_some).then(|| panel_of(&fills))
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
        let wall = |step: IVec3| self.filling(tile + step).is_some();
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
        self.raycast_where(origin, direction, max_distance, |cube| {
            cube.fill != Fill::Open
        })
    }

    /// As [`Base::raycast`], meeting only the cubes `meets` says, such as openings too.
    pub fn raycast_where(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        meets: impl Fn(&Cube) -> bool,
    ) -> Option<BuildHit> {
        self.first_cube_along(origin, direction, max_distance, &meets)
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
                    normal,
                }
            })
    }

    /// The first cube a ray enters that `meets` takes, how far along, and the face it
    /// enters by: the cells walked one by one along the ray, nearest first (Amanatides
    /// and Woo's grid traversal, as voxel engines cast rays), from where it enters the
    /// base's bounds, so it looks at the cubes on its way and no others.
    fn first_cube_along(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        meets: &dyn Fn(&Cube) -> bool,
    ) -> Option<(IVec3, f32, IVec3)> {
        let (low, high) = self.cubes.bounds()?;
        let (box_low, box_high) = (low.as_vec3() * CELL_M, (high + 1).as_vec3() * CELL_M);
        // Where the ray is inside the bounds, along it.
        let (mut enter, mut leave) = (0.0_f32, max_distance);
        for axis in 0..3 {
            if direction[axis].abs() < 1e-9 {
                if origin[axis] < box_low[axis] || origin[axis] > box_high[axis] {
                    return None;
                }
                continue;
            }
            let (a, b) = (
                (box_low[axis] - origin[axis]) / direction[axis],
                (box_high[axis] - origin[axis]) / direction[axis],
            );
            enter = enter.max(a.min(b));
            leave = leave.min(a.max(b));
        }
        if enter > leave {
            return None;
        }
        let start = origin + direction * enter;
        let mut cell = (start / CELL_M).floor().as_ivec3().clamp(low, high);
        let step = IVec3::new(
            direction.x.signum() as i32,
            direction.y.signum() as i32,
            direction.z.signum() as i32,
        );
        // How far along the ray each next boundary is, and how far apart they are.
        let mut next = Vec3::splat(f32::MAX);
        let mut apart = Vec3::splat(f32::MAX);
        for axis in 0..3 {
            if direction[axis].abs() < 1e-9 {
                continue;
            }
            let edge = (cell[axis] + i32::from(step[axis] > 0)) as f32 * CELL_M;
            next[axis] = (edge - origin[axis]) / direction[axis];
            apart[axis] = CELL_M / direction[axis].abs();
        }
        loop {
            if let Some(cube) = self.cubes.get(cell)
                && meets(&cube)
            {
                let corner = cell.as_vec3() * CELL_M;
                if let Some((distance, normal)) =
                    ray_box(origin, direction, corner, corner + Vec3::splat(CELL_M))
                    && distance <= max_distance
                {
                    return Some((cell, distance, normal));
                }
            }
            let axis = if next.x <= next.y && next.x <= next.z {
                0
            } else if next.y <= next.z {
                1
            } else {
                2
            };
            if next[axis] > leave {
                return None;
            }
            cell[axis] += step[axis];
            next[axis] += apart[axis];
            if cell[axis] < low[axis] || cell[axis] > high[axis] {
                return None;
            }
        }
    }

    /// The rectangle up the middle of a wall tile, along the way its wall runs, in the
    /// base's own space: low-left, low-right, high-right, high-left as seen from the side
    /// it faces, south or east.
    pub fn corners(&self, tile: IVec3) -> [Vec3; 4] {
        corners(tile, self.along(tile))
    }

    /// The base as triangles, ready to draw: cubes of a kind with a kit, which `kitted`
    /// says, are left to the kit.
    pub fn mesh(&self, kitted: impl Fn(Kind) -> bool) -> BaseMesh {
        mesh::build(self, kitted)
    }

    /// The base as triangles, its cubes of a kind with a kit left to the kit.
    pub fn mesh_for_kits(&self, kits: &kit::Kits) -> BaseMesh {
        mesh::build(self, |kind| kits.of(kind).is_some())
    }
}

pub use mesh::{Material, Surfaces};

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
    // Open from the floor is a doorway; open higher up, a window.
    let open = layers(Fill::Open);
    if open.first() == Some(&0) {
        return Panel::Door;
    }
    match (open.first(), open.last()) {
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

/// The slab of 16 along x a cube's part keeps it in (`Base::in_parts`).
fn slab_of(at: IVec3) -> i32 {
    at.x.div_euclid(16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ground_floor_is_rooms_and_floors_and_anything_standing_is_occupied() {
        let mut base = Base::default();
        let solid = Some(Cube::new(LOGS, Fill::Solid));
        // A room three tiles square, with a floor under all of it.
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [2, 0, 2],
            kind: LOGS,
        });
        // A ring of walls on bare ground, with no floor or roof: open to the sky, no room.
        for x in 20..23 {
            for z in 0..3 {
                if (x, z) != (21, 1) {
                    base.apply(BuildOp::SetWall {
                        tile: [x, 0, z],
                        panel: Some(Panel::Solid),
                        kind: LOGS,
                    });
                }
            }
        }
        // A floor out in the open, a floor cube cut open, a post on the ground and a
        // floor on the storey above.
        base.apply(BuildOp::SetFloor {
            min: [5, 0, 0],
            max: [6, 0, 0],
            laid: true,
            kind: LOGS,
        });
        let fill =
            |min: [i32; 3], max: [i32; 3], cube: Option<Cube>| BuildOp::Fill { min, max, cube };
        base.apply(fill(
            [8, -1, 0],
            [8, -1, 0],
            Some(Cube::new(LOGS, Fill::Open)),
        ));
        base.apply(fill([10, 0, 0], [10, 2, 0], solid));
        base.apply(fill([12, 5, 0], [12, 5, 0], solid));
        let ground: BTreeSet<[i32; 2]> = base.ground_columns().map(|c| c.to_array()).collect();
        let standing: BTreeSet<[i32; 2]> = base.occupied_columns().map(|c| c.to_array()).collect();
        let room: Vec<[i32; 2]> = (0..3).flat_map(|x| (0..3).map(move |z| [x, z])).collect();
        let ring: Vec<[i32; 2]> = (20..23)
            .flat_map(|x| (0..3).map(move |z| [x, z]))
            .filter(|&c| c != [21, 1])
            .collect();
        let want_ground: BTreeSet<[i32; 2]> =
            room.iter().copied().chain([[5, 0], [6, 0]]).collect();
        let want_standing: BTreeSet<[i32; 2]> = room
            .iter()
            .chain(&ring)
            .copied()
            .chain([[5, 0], [6, 0], [8, 0], [10, 0], [12, 0]])
            .collect();
        assert_eq!(ground, want_ground);
        assert_eq!(standing, want_standing);
        assert!(base.occupied(10, 0) && !base.occupied(21, 1));
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
        // Two tiles inside, each five cubes from the floor to the roof, one room.
        assert_eq!(base.room_cubes(), 10);
        assert_eq!(base.room_count(), 1);
        assert!(base.contains(IVec3::new(1, 0, 1)) && base.contains(IVec3::new(2, 4, 1)));
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
        assert_eq!(base.room_cubes(), 10);
        // Its cubes up to two metres are open, and the rest still wall.
        assert_eq!(base.cube(IVec3::new(1, 3, 2)).unwrap().fill, Fill::Open);
        assert_eq!(base.cube(IVec3::new(1, 4, 2)).unwrap().fill, Fill::Solid);
        base.apply(BuildOp::SetWall {
            tile: door.to_array(),
            panel: None,
            kind: LOGS,
        });
        assert_eq!(base.room_cubes(), 0);
    }

    #[test]
    fn a_window_is_open_between_its_sill_and_head_and_reads_back() {
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
                Fill::Open,
                Fill::Open,
                Fill::Solid,
                Fill::Solid
            ]
        );
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
        let rooms = |base: &Base| base.room_count();
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
    fn what_is_wall_and_what_is_floor_is_read_from_the_cubes_not_storeys() {
        let mut base = Base::default();
        let solid = Some(Cube::new(LOGS, Fill::Solid));
        let fill = |base: &mut Base, min: [i32; 3], max: [i32; 3], cube: Option<Cube>| {
            base.apply(BuildOp::Fill { min, max, cube });
        };
        // A floor a cube thick, a wall on it from layer 3 to 8 on a plinth across where
        // two storeys meet, a knee-high wall, and a stair set on the floor.
        fill(&mut base, [0, -1, 0], [9, -1, 5], solid);
        fill(&mut base, [0, 0, 0], [0, 2, 0], solid);
        fill(&mut base, [0, 3, 0], [0, 8, 5], solid);
        fill(&mut base, [5, 0, 0], [9, 0, 0], solid);
        let stair = Cube {
            shape: Shape::Stair(Facing::North),
            ..Cube::new(LOGS, Fill::Solid)
        };
        fill(&mut base, [5, 0, 3], [5, 0, 3], Some(stair));
        assert!(
            (3..=8).all(|y| base.in_wall(IVec3::new(0, y, 4))),
            "the wall off a storey's layer"
        );
        assert!(base.in_wall(IVec3::new(7, 0, 0)), "the knee-high wall");
        // The floor is floor: under the knee-high wall and the plinth too, where it runs
        // on under them, and under the stair.
        for at in [[4, -1, 2], [7, -1, 0], [0, -1, 0], [5, -1, 3]] {
            assert!(!base.in_wall(IVec3::from_array(at)), "{at:?}");
        }
        // A deck alone up a storey on a post is no wall; the post is.
        let mut deck = Base::default();
        fill(&mut deck, [20, 0, 0], [20, 4, 0], solid);
        fill(&mut deck, [18, 5, -2], [22, 5, 2], solid);
        assert!(deck.in_wall(IVec3::new(20, 2, 0)));
        assert!(!deck.in_wall(IVec3::new(19, 5, 1)));
        // A roof's cubes are never wall.
        fill(
            &mut deck,
            [20, 6, 0],
            [20, 7, 0],
            Some(Cube::new(Kind::FIRST_ROOF, Fill::Solid)),
        );
        assert!(!deck.in_wall(IVec3::new(20, 7, 0)));
    }

    #[test]
    fn a_first_course_on_bare_ground_is_a_wall_and_a_walkway_in_the_air_is_not() {
        // A ring of logs a cube high laid by hand on bare ground, as a player's first
        // course is, and a garden wall of stone a cube high: walls, before any course goes
        // on them (the third goals review, finding 7).
        let mut base = Base::default();
        let solid = Some(Cube::new(LOGS, Fill::Solid));
        let fill = |base: &mut Base, min: [i32; 3], max: [i32; 3], cube: Option<Cube>| {
            base.apply(BuildOp::Fill { min, max, cube });
        };
        fill(&mut base, [0, 0, 0], [7, 0, 5], solid);
        fill(&mut base, [1, 0, 1], [6, 0, 4], None);
        fill(&mut base, [20, 0, 0], [29, 0, 0], solid);
        for at in [[0, 0, 0], [3, 0, 0], [0, 0, 3], [7, 0, 5], [24, 0, 0]] {
            assert!(base.in_wall(IVec3::from_array(at)), "{at:?}");
        }
        // A slab on the ground is a floor, to its edges.
        fill(&mut base, [40, 0, 0], [43, 0, 3], solid);
        for at in [[40, 0, 0], [41, 0, 2], [43, 0, 3]] {
            assert!(!base.in_wall(IVec3::from_array(at)), "{at:?}");
        }
        // The same ring and wall up in the air, as walkways on posts, are no walls.
        let mut high = Base::default();
        fill(&mut high, [0, 4, 0], [7, 4, 5], solid);
        fill(&mut high, [1, 4, 1], [6, 4, 4], None);
        fill(&mut high, [20, 4, 0], [29, 4, 0], solid);
        for at in [[0, 4, 0], [3, 4, 0], [24, 4, 0]] {
            assert!(!high.in_wall(IVec3::from_array(at)), "{at:?}");
        }
    }

    #[test]
    fn a_roofs_cubes_are_no_wall_or_floor_and_cover_the_room_under_them() {
        let mut base = room();
        // The flat roof off, and a hip of roof cubes over the walls.
        base.apply(BuildOp::Fill {
            min: [1, 5, 1],
            max: [2, 5, 1],
            cube: None,
        });
        let footprint = (0..4).flat_map(|x| (0..3).map(move |z| [x, z])).collect();
        let plan = roofer::over(&footprint, 6, roofer::Style::Hip, None);
        for (at, shape) in plan.roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
                }),
            });
        }
        assert!(!base.has_roof(IVec3::new(1, 0, 1)));
        assert!(base.covered(IVec3::new(1, 0, 1), true));
        assert!(!base.covered(IVec3::new(1, 1, 1), true));
        // No wall tiles, floors or rooms on the storey over it of its cubes.
        assert_eq!(base.walls().count(), 10);
        assert!(base.walls().all(|(tile, _)| tile.y == 0));
        assert!(!base.has_floor(IVec3::new(1, 1, 1)));
        assert_eq!(base.room_count(), 1);
        assert!(base.contains(IVec3::new(1, 0, 1)) && base.contains(IVec3::new(1, 5, 1)));
    }

    #[test]
    fn a_ray_walked_cell_by_cell_meets_the_cube_testing_every_cube_does() {
        // Every cube tested against the ray, the nearest it enters taken: the plain way.
        fn plain(
            base: &Base,
            origin: Vec3,
            direction: Vec3,
            max: f32,
        ) -> Option<(IVec3, f32, IVec3)> {
            base.cubes()
                .filter(|(_, cube)| cube.fill != Fill::Open)
                .filter_map(|(at, _)| {
                    let low = at.as_vec3() * CELL_M;
                    let (distance, normal) =
                        ray_box(origin, direction, low, low + Vec3::splat(CELL_M))?;
                    (distance <= max).then_some((at, distance, normal))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
        }
        let kinds = |_| Kind::FIRST_ROOF;
        let mut random = crate::layout::Random(11);
        let (mut hits, mut misses) = (0, 0);
        for (_, ops) in crate::bench_village(LOGS, Kind(2), Kind(3), &kinds)
            .into_iter()
            .take(12)
        {
            let mut base = Base::default();
            base.apply_shortcuts(&ops);
            let (low, high) = base.store().bounds().unwrap();
            let (low, high) = (low.as_vec3() * CELL_M, (high + 1).as_vec3() * CELL_M);
            for _ in 0..300 {
                // From anywhere round it, inside it too, any way, some straight along an axis.
                let origin = Vec3::new(
                    low.x - 4.0 + random.unit() * (high.x - low.x + 8.0),
                    low.y - 2.0 + random.unit() * (high.y - low.y + 4.0),
                    low.z - 4.0 + random.unit() * (high.z - low.z + 8.0),
                );
                let mut direction = Vec3::new(random.signed(), random.signed(), random.signed());
                if random.unit() < 0.1 {
                    direction = Vec3::new(direction.x, 0.0, 0.0);
                }
                let direction = direction.normalize_or(Vec3::X);
                let max = 2.0 + random.unit() * 30.0;
                let walked = base
                    .raycast(origin, direction, max)
                    .map(|hit| (hit.cube, hit.distance, hit.normal));
                let tested = plain(&base, origin, direction, max);
                match (walked, tested) {
                    (Some(a), Some(b)) => {
                        // The same cube, or one entered at the same distance, as at an edge.
                        assert!(
                            a == b || (a.1 - b.1).abs() < 1e-5,
                            "{origin} {direction}: {a:?} {b:?}"
                        );
                        hits += 1;
                    }
                    (None, None) => misses += 1,
                    (a, b) => panic!("{origin} {direction} {max}: walked {a:?}, tested {b:?}"),
                }
            }
        }
        assert!(hits > 300 && misses > 300, "{hits} hits, {misses} misses");
    }
}
