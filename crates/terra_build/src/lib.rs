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
//! - **Shortcuts** ([`BuildOp`], `shortcuts`) lay built cubes as a player means them,
//!   instantly, as the yard and the examples do: a wall tile as a panel says, solid or
//!   with a window or doorway of a default size ([`BuildOp::SetWall`]); a floor or flat
//!   roof over a block; and the room tool, walls round a footprint with a floor inside and
//!   a flat roof over it.
//!
//! A base is saved and sent as a snapshot ([`save`]); older formats are refused (D353).
//!
//! Coordinates are local to the base: X and Z along the floor and Y up. Cubes are counted
//! in cubes all three ways; tiles count X and Z in cubes and Y in storeys.

pub mod cells;
pub mod checker;
pub mod construct;
pub mod cost;
pub mod cubes;
pub mod draw;
pub mod dress;
pub mod fire;
pub use fire::Scorch;
pub mod fitting;
pub mod fragments;
pub mod ground;
pub mod hull;
pub mod inspect;
pub mod junctions;
pub mod kit;
pub mod layers;
pub mod layout;
pub mod look;
pub mod loss;
mod mesh;
pub mod modules;
pub mod ops;
pub mod order;
pub mod packs;
pub mod parts;
pub mod plans;
pub mod put_up;
mod ray;
pub mod relief;
pub mod roof_walls;
pub mod roofer;
pub mod roofing;
pub mod rooms;
pub mod save;
pub mod scenes;
pub mod sections;
mod shortcuts;
pub mod support;
pub mod surface;
pub mod timber;
pub mod town;
pub mod village;
mod walk;

pub use cost::{Amount, Bill, PropKind, Rules, Terms, Toughness};
pub use cubes::{Cube, Cubes, Facing, Fill, Kind, PartId, Pending, Replacement, Shape, State};
pub use ground::{Ground, Sample};
pub use mesh::{Indexed, Surface};
pub use order::Progress;
pub use ray::BuildHit;
pub use scenes::example::{
    RoofParts, VILLAGE_HOUSES, bench_village, example, plan as roof_plan, roof_over, roof_rows,
    with_roof,
};
pub use shortcuts::{BuildOp, Panel, corners};
pub use walk::Walker;

use glam::{IVec2, IVec3};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// The grid: the size of one cube, in metres. Everything else here is given in metres and
/// turned into cubes with [`cells`], so this is the one number to change.
pub const CELL_M: f32 = 0.5;
/// What a share of the light, an occlusion's or a shade's, multiplies a colour by: colours
/// here are as on screen (sRGB) and made linear to be drawn, which takes them about to their
/// 2.2th power, so a shade of a third laid on as a third took nine tenths of the light, and
/// stacked shades crushed a log wall in the shade to black (D373).
pub fn light_share(share: f32) -> f32 {
    libm::powf(share.max(0.0), 1.0 / 2.2)
}

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
    /// How each cube a fire has reached is blackened (D377, `fire`): [`Scorch`]'s levels,
    /// set by the fire as it changes and drawn as damage is. Not saved: a fire is not.
    scorch: BTreeMap<[i32; 3], Scorch>,
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
    /// Whether it is what stands of a base ([`Base::what_stands`]): only cubes with
    /// something in, a cube going in counted as far as it is filled.
    standing: bool,
}

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

    /// How a cube is blackened by fire (D377).
    pub fn scorch(&self, at: IVec3) -> Scorch {
        self.scorch.get(&at.to_array()).copied().unwrap_or_default()
    }

    /// Every cube a fire has blackened, sorted.
    pub fn scorched(&self) -> impl Iterator<Item = (IVec3, Scorch)> + '_ {
        self.scorch
            .iter()
            .map(|(&at, &scorch)| (IVec3::from_array(at), scorch))
    }

    /// The cubes a fire has blackened from one cube to another, that are still there.
    pub fn scorched_within(
        &self,
        low: IVec3,
        high: IVec3,
    ) -> impl Iterator<Item = ([i32; 3], Scorch)> + '_ {
        self.scorch
            .range([low.x, i32::MIN, i32::MIN]..=[high.x, i32::MAX, i32::MAX])
            .filter(move |(at, _)| {
                let at = IVec3::from_array(**at);
                at.cmpge(low).all() && at.cmple(high).all() && self.cube(at).is_some()
            })
            .map(|(&at, &scorch)| (at, scorch))
    }

    /// Sets how a cube is blackened, as its fire says; its section is drawn again only when
    /// that changes. Nothing for a cube that is not there.
    pub fn set_scorch(&mut self, at: IVec3, scorch: Scorch) {
        if self.cube(at).is_none() {
            self.scorch.remove(&at.to_array());
            return;
        }
        let was = self.scorch(at);
        if was == scorch {
            return;
        }
        if scorch == Scorch::default() {
            self.scorch.remove(&at.to_array());
        } else {
            self.scorch.insert(at.to_array(), scorch);
        }
        self.cubes.mark(at);
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
                // A column first built on keeps its own ground from then on, as it stands
                // now, so what holds it never moves with a sample beside it (D367).
                if !self.columns.contains_key(&column) && self.ground.sample(column).is_none() {
                    let height = self.ground.height_cm(column);
                    self.ground.set(column, Some(height));
                }
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

    /// What stands of the base, to draw what is in from (D215 as amended by D358): its
    /// cubes with something in, and its openings; a plan's cubes with nothing in are left
    /// out, so a wall built halfway shows a top and an end where it stops. A cube going in
    /// counts as built as far as it is filled (`standing`).
    pub fn what_stands(&self) -> Base {
        let mut view = self.clone();
        let planned: Vec<IVec3> = self
            .cubes
            .iter()
            .filter(|(_, cube)| !stands(cube))
            .map(|(at, _)| at)
            .collect();
        for at in planned {
            view.put(at, None);
        }
        view.cubes.take_dirty();
        view.standing = true;
        view
    }

    /// What stands of the base, from what stood of it before these sections changed
    /// (`was`, [`Base::what_stands`] of the base before): only their cubes are looked at
    /// again, so an edit costs what it changed, not what is planned. Also whether any cube
    /// of what stands changed.
    pub fn what_stands_again(&self, was: &Base, marked: &BTreeSet<[i32; 3]>) -> (Base, bool) {
        let mut view = Base {
            cubes: was.cubes.clone(),
            columns: was.columns.clone(),
            in_parts: was.in_parts.clone(),
            unbuilt: was.unbuilt.clone(),
            pending: was.pending.clone(),
            kinds: was.kinds.clone(),
            rooms: was.rooms.clone(),
            standing: true,
            ..self.clone()
        };
        let mut changed = false;
        let side = cubes::SECTION;
        for &section in marked {
            let low = IVec3::from_array(section) * side;
            let high = low + IVec3::splat(side - 1);
            let now: BTreeMap<[i32; 3], Cube> = self
                .cubes
                .within(low, high)
                .into_iter()
                .filter(|(_, cube)| stands(cube))
                .map(|(at, cube)| (at.to_array(), cube))
                .collect();
            let gone: Vec<[i32; 3]> = view
                .cubes
                .within(low, high)
                .into_iter()
                .map(|(at, _)| at.to_array())
                .filter(|at| !now.contains_key(at))
                .collect();
            for at in gone {
                changed |= view.put(IVec3::from_array(at), None);
            }
            for (at, cube) in now {
                changed |= view.put(IVec3::from_array(at), Some(cube));
            }
        }
        // What `put` keeps of parts and changes is the base's own.
        view.parts = self.parts.clone();
        view.replacing = self.replacing.clone();
        view.cubes.take_dirty();
        (view, changed)
    }

    /// Whether its fitted props are not another's.
    pub fn props_differ(&self, other: &Base) -> bool {
        self.props != other.props
    }

    /// Whether it is what stands of a base ([`Base::what_stands`]).
    pub fn is_standing(&self) -> bool {
        self.standing
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

    /// The base as triangles, ready to draw: cubes of a kind with a kit, which `kitted`
    /// says, are left to the kit.
    pub fn mesh(&self, kitted: impl Fn(Kind) -> bool) -> Surface {
        mesh::build(self, kitted)
    }

    /// The base as triangles, its cubes of a kind with a kit left to the kit.
    pub fn mesh_for_kits(&self, kits: &kit::Kits) -> Surface {
        mesh::build(self, |kind| kits.of(kind).is_some())
    }
}

pub use mesh::{Material, Surfaces};

/// Whether a cube is part of what stands: something of it in, or an opening, or being
/// carved, which is drawn open whatever is in.
fn stands(cube: &Cube) -> bool {
    cube.level() > 0 || layout::drawn_fill(cube) == Fill::Open
}

/// The slab of 16 along x a cube's part keeps it in (`Base::in_parts`).
fn slab_of(at: IVec3) -> i32 {
    at.x.div_euclid(16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

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
}
