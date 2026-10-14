//! The cube store (docs/building-plan.md, D195): a base's cubes in sections of 16 a side,
//! each a dense array of cubes packed in 32 bits, kept in a sorted map, with a dirty mark
//! per section for whatever lays and draws them.
//!
//! A cube is packed as:
//!
//! | Bits | Holds |
//! |---|---|
//! | 0 to 11 | Its kind, plus one, so 0 is air |
//! | 12 to 16 | Its shape ([`Shape::code`]), and 31 for an opening (30, glass before D225 took it out, reads as an opening) |
//! | 17 to 20 | Its fill level while planned, in sixteenths of the cube |
//! | 21 | Built: all of it in |
//! | 22 to 23 | The change pending on it |
//! | 24 to 31 | The part it was laid in, as an index into its section's palette of parts |
//!
//! The part is a section's own index, as Minecraft's block palettes are, so a cube stays
//! 32 bits whatever the number of parts in a base; a section holds up to 255 parts. The
//! shape had four bits until D231 took one of the three the pending change had, which used
//! two; [`from_v1`] reads a word packed before.
//!
//! Iteration is always in one sorted order, east then up then south (the order of
//! `[x, y, z]`), so every machine lays and floods the same way.

use glam::IVec3;
use std::collections::{BTreeMap, BTreeSet};

/// How many cubes a section is along each side.
pub const SECTION: i32 = 16;
const SECTION_CUBES: usize = (SECTION * SECTION * SECTION) as usize;

/// A full cube's fill level: a level counts sixteenths of a cube.
pub const FULL: u8 = 16;

/// A kind of construction, as the game's kits number them (`assets/data/kits/`): the one
/// numbered 0 is drawn plain.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Kind(pub u16);

impl Kind {
    pub const PLAIN: Kind = Kind(0);
    /// The highest kind a cube can hold: twelve bits, less the one for air.
    pub const LAST: Kind = Kind(0xFFE);
    /// The first roof kind (D241): kinds from here up are a roof's covering, laid on cubes
    /// as any kind is but drawn as a roof over them, with no wall tile, floor or room of
    /// their own. Known by number, as the cubes alone say what holds and where rooms are.
    pub const FIRST_ROOF: Kind = Kind(0x800);

    /// Whether it is a roof's covering (D241): shakes, slate, thatch.
    pub fn is_roof(self) -> bool {
        self >= Kind::FIRST_ROOF && self <= Kind::LAST
    }
}

/// What fills a cube that is not air.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Fill {
    #[default]
    Solid,
    /// An opening cut in a wall, a doorway or a window: part of the wall still, so the
    /// room stays closed in, but a walker goes through it. A window's glass, written
    /// before D225 took glass out, reads as one.
    #[serde(alias = "Glass")]
    Open,
}

/// Which way a stair or slope rises: toward this side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Facing {
    #[default]
    North,
    East,
    South,
    West,
}

impl Facing {
    pub const ALL: [Facing; 4] = [Facing::North, Facing::East, Facing::South, Facing::West];

    /// A step toward this side, along the floor.
    pub fn step(self) -> glam::IVec3 {
        match self {
            Facing::North => glam::IVec3::NEG_Z,
            Facing::East => glam::IVec3::X,
            Facing::South => glam::IVec3::Z,
            Facing::West => glam::IVec3::NEG_X,
        }
    }

    /// Turned a quarter clockwise, seen from above, so many times.
    pub fn turned(self, quarters: i32) -> Facing {
        Facing::ALL[(self as i32 + quarters).rem_euclid(4) as usize]
    }

    pub fn opposite(self) -> Facing {
        self.turned(2)
    }

    /// How far a point of a cube's footprint is toward this side, from 0 at the far side to
    /// 1 at this one, given as fractions of the cube east (`u`) and south (`v`).
    pub fn toward(self, u: f32, v: f32) -> f32 {
        match self {
            Facing::North => 1.0 - v,
            Facing::East => u,
            Facing::South => v,
            Facing::West => 1.0 - u,
        }
    }
}

/// A cube's shape (D193): whole, a stair, a slope or a half, each with its collision from
/// its top ([`Shape::top`], `walk.rs`); and a post, a column's cube drawn as an upright as
/// narrow as its kit says, which collides and holds as a whole cube. Then, as Enshrouded's
/// blocks have them (D231): the upper half; a stair or a slope upside down, under a ceiling
/// or a flight; and the corners where two slopes meet, an outer corner as a hip's and an
/// inner one as a valley's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Shape {
    #[default]
    Whole,
    /// The lower half.
    Half,
    /// Two steps, each half the cube's height and half its depth, rising toward the side.
    Stair(Facing),
    /// A ramp, rising across the cube to its top at the side.
    Slope(Facing),
    /// A post: a column's cube drawn as an upright, as narrow as its kit says.
    Post,
    /// The upper half.
    UpperHalf,
    /// A stair upside down: whole on the side's half, its upper half on the other.
    InvertedStair(Facing),
    /// A slope upside down: whole at the side, its underside a ramp up to nothing at the
    /// other.
    InvertedSlope(Facing),
    /// Where two slopes meet round an outside corner, as at a hip: rising toward the side
    /// and toward the next side clockwise, its top the lower of the two ramps, so it peaks
    /// at the corner between those sides.
    OuterCorner(Facing),
    /// Where two slopes meet round an inside corner, as at a valley: rising toward the side
    /// and toward the next side clockwise, its top the higher of the two ramps, so it is low
    /// only at the corner across from them.
    InnerCorner(Facing),
}

impl Shape {
    /// Every shape, each facing of those that face a way, in the order of their codes.
    pub const ALL: [Shape; 28] = {
        let [n, e, s, w] = Facing::ALL;
        [
            Shape::Whole,
            Shape::Half,
            Shape::Stair(n),
            Shape::Stair(e),
            Shape::Stair(s),
            Shape::Stair(w),
            Shape::Slope(n),
            Shape::Slope(e),
            Shape::Slope(s),
            Shape::Slope(w),
            Shape::Post,
            Shape::UpperHalf,
            Shape::InvertedStair(n),
            Shape::InvertedStair(e),
            Shape::InvertedStair(s),
            Shape::InvertedStair(w),
            Shape::InvertedSlope(n),
            Shape::InvertedSlope(e),
            Shape::InvertedSlope(s),
            Shape::InvertedSlope(w),
            Shape::OuterCorner(n),
            Shape::OuterCorner(e),
            Shape::OuterCorner(s),
            Shape::OuterCorner(w),
            Shape::InnerCorner(n),
            Shape::InnerCorner(e),
            Shape::InnerCorner(s),
            Shape::InnerCorner(w),
        ]
    };

    /// Its number in a cube's five shape bits (D231), its place in [`Shape::ALL`]; an
    /// opening is 31, and 30, once glass, reads as one.
    pub fn code(self) -> u8 {
        match self {
            Shape::Whole => 0,
            Shape::Half => 1,
            Shape::Stair(f) => 2 + f as u8,
            Shape::Slope(f) => 6 + f as u8,
            Shape::Post => 10,
            Shape::UpperHalf => 11,
            Shape::InvertedStair(f) => 12 + f as u8,
            Shape::InvertedSlope(f) => 16 + f as u8,
            Shape::OuterCorner(f) => 20 + f as u8,
            Shape::InnerCorner(f) => 24 + f as u8,
        }
    }

    /// The shape a code is, if it is one.
    pub fn of_code(code: u8) -> Option<Shape> {
        Shape::ALL.get(usize::from(code)).copied()
    }

    /// The way it faces, if it faces one.
    pub fn facing(self) -> Option<Facing> {
        match self {
            Shape::Stair(f)
            | Shape::Slope(f)
            | Shape::InvertedStair(f)
            | Shape::InvertedSlope(f)
            | Shape::OuterCorner(f)
            | Shape::InnerCorner(f) => Some(f),
            _ => None,
        }
    }

    /// How high it reaches at its highest, as a fraction of the cube.
    pub fn height(self) -> f32 {
        match self {
            Shape::Half => 0.5,
            _ => 1.0,
        }
    }

    /// How high its top is over a point of its footprint, as a fraction of the cube, the
    /// point given as fractions of the cube east (`u`) and south (`v`): what holds a walker
    /// up there (`walk.rs`).
    pub fn top(self, u: f32, v: f32) -> f32 {
        match self {
            Shape::Whole
            | Shape::Post
            | Shape::UpperHalf
            | Shape::InvertedStair(_)
            | Shape::InvertedSlope(_) => 1.0,
            Shape::Half => 0.5,
            Shape::Stair(facing) => {
                if facing.toward(u, v) < 0.5 {
                    0.5
                } else {
                    1.0
                }
            }
            Shape::Slope(facing) => facing.toward(u, v).clamp(0.0, 1.0),
            Shape::OuterCorner(facing) => facing
                .toward(u, v)
                .min(facing.turned(1).toward(u, v))
                .clamp(0.0, 1.0),
            Shape::InnerCorner(facing) => facing
                .toward(u, v)
                .max(facing.turned(1).toward(u, v))
                .clamp(0.0, 1.0),
        }
    }

    /// How high its underside is over a point of its footprint, as [`Shape::top`] gives its
    /// top: nothing of it is lower, so a walker passes under an upper half.
    pub fn bottom(self, u: f32, v: f32) -> f32 {
        match self {
            Shape::UpperHalf => 0.5,
            Shape::InvertedStair(facing) => {
                if facing.toward(u, v) < 0.5 {
                    0.5
                } else {
                    0.0
                }
            }
            Shape::InvertedSlope(facing) => (1.0 - facing.toward(u, v)).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// Whether its top is a plane cut across the cube, a half's, a slope's or a corner's,
    /// rather than steps or the cube's own.
    pub fn planed_top(self) -> bool {
        matches!(
            self,
            Shape::Half | Shape::Slope(_) | Shape::OuterCorner(_) | Shape::InnerCorner(_)
        )
    }

    /// Whether its underside is a plane cut across the cube: an upper half's or an inverted
    /// slope's.
    pub fn planed_under(self) -> bool {
        matches!(self, Shape::UpperHalf | Shape::InvertedSlope(_))
    }

    /// Whether it is drawn as boxes, a step each: a stair, either way up.
    pub fn stepped(self) -> bool {
        matches!(self, Shape::Stair(_) | Shape::InvertedStair(_))
    }

    /// Whether it is a cube cut to a shape, drawn apart from its kind's whole cubes.
    pub fn is_cut(self) -> bool {
        !matches!(self, Shape::Whole)
    }

    /// Turned a quarter clockwise, seen from above, so many times.
    pub fn turned(self, quarters: i32) -> Shape {
        match self {
            Shape::Stair(facing) => Shape::Stair(facing.turned(quarters)),
            Shape::Slope(facing) => Shape::Slope(facing.turned(quarters)),
            Shape::InvertedStair(facing) => Shape::InvertedStair(facing.turned(quarters)),
            Shape::InvertedSlope(facing) => Shape::InvertedSlope(facing.turned(quarters)),
            Shape::OuterCorner(facing) => Shape::OuterCorner(facing.turned(quarters)),
            Shape::InnerCorner(facing) => Shape::InnerCorner(facing.turned(quarters)),
            other => other,
        }
    }

    /// The boxes a stepped shape is, in fractions of the cube from its low corner: a
    /// stair's lower step across it and its upper step at the side; an inverted stair's
    /// whole half at the side and its upper half across the rest. None for other shapes.
    pub fn steps(self) -> Vec<([f32; 3], [f32; 3])> {
        let (facing, inverted) = match self {
            Shape::Stair(f) => (f, false),
            Shape::InvertedStair(f) => (f, true),
            _ => return Vec::new(),
        };
        // The half of the cube toward the side along its axis, and the half away.
        let step = facing.step();
        let axis = if step.x != 0 { 0 } else { 2 };
        let (mut back, mut front) = (([0.0; 3], [1.0; 3]), ([0.0; 3], [1.0; 3]));
        if step[axis] > 0 {
            back.0[axis] = 0.5;
            front.1[axis] = 0.5;
        } else {
            back.1[axis] = 0.5;
            front.0[axis] = 0.5;
        }
        if inverted {
            // The whole back half, and the front half's upper half.
            front.0[1] = 0.5;
            vec![back, front]
        } else {
            // The lower half across it, and the back half's upper half.
            back.0[1] = 0.5;
            vec![([0.0; 3], [1.0, 0.5, 1.0]), back]
        }
    }
}

/// How far a cube is in: planned, with so many sixteenths of it in, or built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum State {
    /// Below [`FULL`].
    Planned { level: u8 },
    #[default]
    Built,
}

/// A change pending on a cube (construction-001, gap 1): at most one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Pending {
    #[default]
    None,
    /// Its kind is to change: what it changes to is kept beside the cubes.
    Replace,
    /// It is to be cut out, leaving an opening.
    Carve,
    /// Its kind has changed and the new is going in: what it was is kept beside the cubes,
    /// so a swap again turns it back (D232).
    Swapped,
}

impl Pending {
    /// Whether its kind is changing, before or after the swap.
    pub fn changing(self) -> bool {
        matches!(self, Pending::Replace | Pending::Swapped)
    }

    /// Whether it is being worked on in place, under what stays over it: carved out, or
    /// changed to another kind (D199, D232).
    pub fn in_place(self) -> bool {
        self != Pending::None
    }
}

/// A part a player laid (D139): the cubes it groups, its kind, and what each step of a
/// cube costs. Numbered by whoever lays it, so laying it again after an undo gives the
/// same number.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct PartId(pub u32);

/// What a cube marked to change changes to: a kind, in a part.
pub type Replacement = (Kind, Option<PartId>);

/// A cube of a kind of construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Cube {
    pub kind: Kind,
    pub fill: Fill,
    #[serde(default, skip_serializing_if = "is_default")]
    pub shape: Shape,
    #[serde(default, skip_serializing_if = "is_default")]
    pub state: State,
    #[serde(default, skip_serializing_if = "is_default")]
    pub pending: Pending,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<PartId>,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

impl Cube {
    /// A whole cube, built, in no part: what the shortcuts lay.
    pub const fn new(kind: Kind, fill: Fill) -> Cube {
        Cube {
            kind,
            fill,
            shape: Shape::Whole,
            state: State::Built,
            pending: Pending::None,
            part: None,
        }
    }

    /// A whole cube of a part, planned, with nothing in it.
    pub const fn planned(kind: Kind, part: Option<PartId>) -> Cube {
        Cube {
            kind,
            fill: Fill::Solid,
            shape: Shape::Whole,
            state: State::Planned { level: 0 },
            pending: Pending::None,
            part,
        }
    }

    /// How much of it is in, in sixteenths: [`FULL`] once built.
    pub fn level(&self) -> u8 {
        match self.state {
            State::Built => FULL,
            State::Planned { level } => level,
        }
    }

    /// The cube at a fill level: built at [`FULL`].
    pub fn at_level(mut self, level: u8) -> Cube {
        self.state = if level >= FULL {
            State::Built
        } else {
            State::Planned { level }
        };
        self
    }

    pub fn is_built(&self) -> bool {
        self.state == State::Built
    }

    /// Whether anything of it is in, so it holds weight and joins its neighbours: some of
    /// it built, and not an opening.
    pub fn is_in(&self) -> bool {
        self.level() > 0 && self.fill != Fill::Open
    }

    /// The cube as 32 bits, with its part as an index into its section's palette.
    fn pack(&self, part: u8) -> u32 {
        debug_assert!(
            self.kind <= Kind::LAST,
            "kind {:?} is past the last",
            self.kind
        );
        let shape = match self.fill {
            Fill::Open => OPEN,
            Fill::Solid => u32::from(self.shape.code()),
        };
        let (level, built) = match self.state {
            State::Built => (0, 1),
            State::Planned { level } => (u32::from(level.min(FULL - 1)), 0),
        };
        let pending = self.pending as u32;
        (u32::from(self.kind.0) + 1)
            | shape << 12
            | level << 17
            | built << 21
            | pending << 22
            | u32::from(part) << 24
    }

    /// A cube from its 32 bits, and its palette index; `None` for air.
    fn unpack(word: u32) -> Option<(Cube, u8)> {
        let kind = word & 0xFFF;
        if kind == 0 {
            return None;
        }
        let (fill, shape) = match (word >> 12) & 0x1F {
            OLD_GLASS | OPEN => (Fill::Open, Shape::Whole),
            code => (
                Fill::Solid,
                Shape::of_code(code as u8).unwrap_or(Shape::Whole),
            ),
        };
        let state = if (word >> 21) & 1 == 1 {
            State::Built
        } else {
            State::Planned {
                level: ((word >> 17) & 0xF) as u8,
            }
        };
        let pending = match (word >> 22) & 0x3 {
            1 => Pending::Replace,
            2 => Pending::Carve,
            3 => Pending::Swapped,
            _ => Pending::None,
        };
        let cube = Cube {
            kind: Kind((kind - 1) as u16),
            fill,
            shape,
            state,
            pending,
            part: None,
        };
        Some((cube, (word >> 24) as u8))
    }
}

/// The shape code of an opening, and of a window's glass before D225 took glass out,
/// which reads as an opening.
const OPEN: u32 = 31;
const OLD_GLASS: u32 = 30;

/// A cube's word as it was packed before D231, with four bits of shape (glass 14, an
/// opening 15) and three of the pending change, as it is packed now.
pub fn from_v1(word: u32) -> u32 {
    if word & 0xFFF == 0 {
        return 0;
    }
    let shape = match (word >> 12) & 0xF {
        14 => OLD_GLASS,
        15 => OPEN,
        code => code,
    };
    let (level, built, pending) = ((word >> 16) & 0xF, (word >> 20) & 1, (word >> 21) & 0x3);
    (word & 0xFFF) | shape << 12 | level << 17 | built << 21 | pending << 22 | (word & 0xFF00_0000)
}

/// Which section a cube is in, counted in sections.
pub fn section_of(at: IVec3) -> IVec3 {
    IVec3::new(
        at.x.div_euclid(SECTION),
        at.y.div_euclid(SECTION),
        at.z.div_euclid(SECTION),
    )
}

/// Where a cube is in its section: east, then up, then south, as the sorted order.
fn index_of(at: IVec3) -> usize {
    let local = at - section_of(at) * SECTION;
    (local.x * SECTION * SECTION + local.y * SECTION + local.z) as usize
}

/// A section: its cubes, its palette of parts, and how many cubes are not air.
#[derive(Clone)]
struct Section {
    cubes: Box<[u32]>,
    /// The parts its cubes were laid in, each with how many of its cubes it holds; the
    /// index of each, plus one, is what a cube stores. A part none holds frees its place.
    palette: Vec<(PartId, u32)>,
    count: u32,
}

impl Default for Section {
    fn default() -> Self {
        Section {
            cubes: vec![0; SECTION_CUBES].into_boxed_slice(),
            palette: Vec::new(),
            count: 0,
        }
    }
}

impl Section {
    fn get(&self, index: usize) -> Option<Cube> {
        let (mut cube, part) = Cube::unpack(self.cubes[index])?;
        cube.part = (part > 0).then(|| self.palette[part as usize - 1].0);
        Some(cube)
    }

    /// The palette index of a part, taking a free place for it if it is new.
    fn place_of(&mut self, part: Option<PartId>) -> Result<u8, TooManyParts> {
        let Some(part) = part else { return Ok(0) };
        if let Some(index) = self.palette.iter().position(|&(p, n)| p == part && n > 0) {
            return Ok(index as u8 + 1);
        }
        if let Some(index) = self.palette.iter().position(|&(_, n)| n == 0) {
            self.palette[index] = (part, 0);
            return Ok(index as u8 + 1);
        }
        if self.palette.len() >= usize::from(u8::MAX) {
            return Err(TooManyParts);
        }
        self.palette.push((part, 0));
        Ok(self.palette.len() as u8)
    }

    fn holds_part(&self, part: PartId) -> bool {
        self.palette.iter().any(|&(p, n)| p == part && n > 0)
    }

    fn free_places(&self) -> usize {
        usize::from(u8::MAX) - self.palette.iter().filter(|&&(_, n)| n > 0).count()
    }
}

/// A section has room for no more parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooManyParts;

/// A base's cubes.
#[derive(Clone, Default)]
pub struct Cubes {
    sections: BTreeMap<[i32; 3], Section>,
    /// Sections changed since the dirty marks were last taken: their own cubes, or a
    /// neighbour's on their edge.
    dirty: BTreeSet<[i32; 3]>,
    count: usize,
    planned: usize,
}

impl Cubes {
    /// The cube at a place, counted in cubes, if it is not air.
    pub fn get(&self, at: IVec3) -> Option<Cube> {
        self.sections
            .get(&section_of(at).to_array())?
            .get(index_of(at))
    }

    /// Sets a cube, or air with `None`, and marks its section dirty, and its neighbours
    /// when it is on an edge. Returns what was there. Fails, changing nothing, when its
    /// section has no room for another part.
    pub fn set(&mut self, at: IVec3, cube: Option<Cube>) -> Result<Option<Cube>, TooManyParts> {
        let key = section_of(at).to_array();
        let index = index_of(at);
        let old = self.get(at);
        if old == cube {
            return Ok(old);
        }
        let section = self.sections.entry(key).or_default();
        let word = match cube {
            Some(cube) => match section.place_of(cube.part) {
                Ok(place) => cube.pack(place),
                Err(full) => {
                    if section.count == 0 {
                        self.sections.remove(&key);
                    }
                    return Err(full);
                }
            },
            None => 0,
        };
        let old_place = (section.cubes[index] >> 24) as usize;
        if old_place > 0 {
            section.palette[old_place - 1].1 -= 1;
        }
        let new_place = (word >> 24) as usize;
        if new_place > 0 {
            section.palette[new_place - 1].1 += 1;
        }
        section.cubes[index] = word;
        let planned = |cube: Option<Cube>| usize::from(cube.is_some_and(|c| !c.is_built()));
        self.planned = self.planned + planned(cube) - planned(old);
        match (old.is_some(), cube.is_some()) {
            (false, true) => {
                section.count += 1;
                self.count += 1;
            }
            (true, false) => {
                section.count -= 1;
                self.count -= 1;
                if section.count == 0 {
                    self.sections.remove(&key);
                }
            }
            _ => {}
        }
        self.mark(at);
        Ok(old)
    }

    /// Marks a cube's section dirty, and the neighbours that share the face it is on.
    pub fn mark(&mut self, at: IVec3) {
        let section = section_of(at);
        let local = at - section * SECTION;
        self.dirty.insert(section.to_array());
        for axis in 0..3 {
            let mut step = IVec3::ZERO;
            if local[axis] == 0 {
                step[axis] = -1;
            } else if local[axis] == SECTION - 1 {
                step[axis] = 1;
            } else {
                continue;
            }
            self.dirty.insert((section + step).to_array());
        }
    }

    /// The sections changed since this was last asked, sorted; the marks are cleared.
    pub fn take_dirty(&mut self) -> BTreeSet<[i32; 3]> {
        std::mem::take(&mut self.dirty)
    }

    /// The sections changed and not yet taken.
    pub fn dirty(&self) -> &BTreeSet<[i32; 3]> {
        &self.dirty
    }

    /// How many cubes are not air.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// How many cubes are planned and not yet built.
    pub fn planned(&self) -> usize {
        self.planned
    }

    /// Every section with cubes in it, sorted, counted in sections.
    pub fn sections(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.sections.keys().map(|&key| IVec3::from_array(key))
    }

    /// Whether a section could take cubes of these parts.
    pub fn has_room(&self, section: IVec3, parts: &BTreeSet<PartId>) -> bool {
        let Some(held) = self.sections.get(&section.to_array()) else {
            return parts.len() <= usize::from(u8::MAX);
        };
        let new = parts.iter().filter(|&&part| !held.holds_part(part)).count();
        new <= held.free_places()
    }

    /// Every cube that is not air within a box, from `low` to `high` inclusive, sorted by
    /// `[x, y, z]`: only the sections the box reaches are read, so a layout of one section
    /// does not walk a whole castle (docs/building-plan.md, drawing).
    pub fn within(&self, low: IVec3, high: IVec3) -> Vec<(IVec3, Cube)> {
        let (from, to) = (section_of(low), section_of(high));
        let mut found = Vec::new();
        for (&key, section) in self.sections.range(from.to_array()..=to.to_array()) {
            let at = IVec3::from_array(key);
            if at.cmplt(from).any() || at.cmpgt(to).any() {
                continue;
            }
            let origin = at * SECTION;
            let (a, b) = (
                (low - origin).max(IVec3::ZERO),
                (high - origin).min(IVec3::splat(SECTION - 1)),
            );
            for x in a.x..=b.x {
                for y in a.y..=b.y {
                    for z in a.z..=b.z {
                        let local = IVec3::new(x, y, z);
                        let index = (x * SECTION * SECTION + y * SECTION + z) as usize;
                        if section.cubes[index] != 0
                            && let Some(cube) = section.get(index)
                        {
                            found.push((origin + local, cube));
                        }
                    }
                }
            }
        }
        found.sort_by_key(|(at, _)| at.to_array());
        found
    }

    /// The smallest box holding every section with cubes in it, in cubes, if any has.
    pub fn bounds(&self) -> Option<(IVec3, IVec3)> {
        let mut keys = self.sections.keys().map(|&key| IVec3::from_array(key));
        let first = keys.next()?;
        let (low, high) = keys.fold((first, first), |(lo, hi), at| (lo.min(at), hi.max(at)));
        Some((low * SECTION, (high + IVec3::ONE) * SECTION - IVec3::ONE))
    }

    /// Every cube that is not air, sorted by `[x, y, z]`.
    pub fn iter(&self) -> Iter<'_> {
        // Sections grouped by their x, then y, so that the cubes come out in the order of
        // their places across the sections, not section by section.
        let mut columns: Columns<'_> = Vec::new();
        for (&[x, y, z], section) in &self.sections {
            if columns.last().is_none_or(|(at, _)| *at != x) {
                columns.push((x, Vec::new()));
            }
            let rows = &mut columns.last_mut().expect("just pushed").1;
            if rows.last().is_none_or(|(at, _)| *at != y) {
                rows.push((y, Vec::new()));
            }
            rows.last_mut().expect("just pushed").1.push((z, section));
        }
        Iter {
            columns,
            cursor: [0; 6],
        }
    }
}

/// Sections by their x, then their y, each row of them sorted by z.
type Columns<'a> = Vec<(i32, Vec<(i32, Vec<(i32, &'a Section)>)>)>;

/// The cubes of a store in sorted order.
pub struct Iter<'a> {
    columns: Columns<'a>,
    /// Which x group, x within it, y group, y within it, z section, z within it.
    cursor: [usize; 6],
}

impl Iterator for Iter<'_> {
    type Item = (IVec3, Cube);

    fn next(&mut self) -> Option<Self::Item> {
        let side = SECTION as usize;
        loop {
            let [gx, lx, gy, ly, gz, lz] = self.cursor;
            let (sx, rows) = self.columns.get(gx)?;
            if lx >= side {
                self.cursor = [gx + 1, 0, 0, 0, 0, 0];
                continue;
            }
            let Some((sy, row)) = rows.get(gy) else {
                self.cursor = [gx, lx + 1, 0, 0, 0, 0];
                continue;
            };
            if ly >= side {
                self.cursor = [gx, lx, gy + 1, 0, 0, 0];
                continue;
            }
            let Some(&(sz, section)) = row.get(gz) else {
                self.cursor = [gx, lx, gy, ly + 1, 0, 0];
                continue;
            };
            if lz >= side {
                self.cursor = [gx, lx, gy, ly, gz + 1, 0];
                continue;
            }
            self.cursor[5] += 1;
            let index = (lx * side + ly) * side + lz;
            if section.cubes[index] == 0 {
                continue;
            }
            let at = IVec3::new(sx * SECTION, sy * SECTION, sz * SECTION)
                + IVec3::new(lx as i32, ly as i32, lz as i32);
            return section.get(index).map(|cube| (at, cube));
        }
    }
}

impl PartialEq for Cubes {
    /// The same cubes in the same places; dirty marks and how palettes are ordered aside.
    fn eq(&self, other: &Self) -> bool {
        self.count == other.count && self.iter().eq(other.iter())
    }
}

impl std::fmt::Debug for Cubes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cubes")
            .field("count", &self.count)
            .field("planned", &self.planned)
            .field("sections", &self.sections.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// A section as saved: its place, its palette, and its cubes as runs of the same word.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedSection {
    pub at: [i32; 3],
    pub parts: Vec<PartId>,
    /// How many cubes in a row, in the sorted order, hold the same packed word.
    pub runs: Vec<(u16, u32)>,
}

impl SavedSection {
    /// The section saved before D231, its words packed as they are now ([`from_v1`]).
    pub fn from_v1(&self) -> SavedSection {
        SavedSection {
            runs: self
                .runs
                .iter()
                .map(|&(count, word)| (count, from_v1(word)))
                .collect(),
            ..self.clone()
        }
    }
}

/// Why saved sections could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BadSection {
    /// Its runs do not cover a section exactly.
    Length([i32; 3]),
    /// A cube names a part its palette does not have.
    Part([i32; 3]),
    /// Two sections at one place.
    Twice([i32; 3]),
}

impl Cubes {
    /// The sections as saved, sorted, each palette in order of first use.
    pub fn save(&self) -> Vec<SavedSection> {
        self.sections
            .iter()
            .map(|(&at, section)| {
                let mut parts: Vec<PartId> = Vec::new();
                let mut runs: Vec<(u16, u32)> = Vec::new();
                for index in 0..SECTION_CUBES {
                    let word = section.cubes[index];
                    let place = (word >> 24) as usize;
                    let word = if place > 0 {
                        let part = section.palette[place - 1].0;
                        let new = match parts.iter().position(|&p| p == part) {
                            Some(found) => found + 1,
                            None => {
                                parts.push(part);
                                parts.len()
                            }
                        };
                        (word & 0x00FF_FFFF) | (new as u32) << 24
                    } else {
                        word
                    };
                    match runs.last_mut() {
                        Some((count, last)) if *last == word && *count < u16::MAX => *count += 1,
                        _ => runs.push((1, word)),
                    }
                }
                SavedSection { at, parts, runs }
            })
            .collect()
    }

    /// The cubes from their saved sections.
    pub fn load(saved: &[SavedSection]) -> Result<Cubes, BadSection> {
        let mut cubes = Cubes::default();
        for section in saved {
            let at = section.at;
            if cubes.sections.contains_key(&at) {
                return Err(BadSection::Twice(at));
            }
            // Checked as the runs are read, so a snapshot sent with runs past a section's
            // end is refused before it takes any more memory than a section.
            let mut words = Vec::with_capacity(SECTION_CUBES);
            for &(count, word) in &section.runs {
                if words.len() + usize::from(count) > SECTION_CUBES {
                    return Err(BadSection::Length(at));
                }
                words.extend(std::iter::repeat_n(word, usize::from(count)));
            }
            if words.len() != SECTION_CUBES {
                return Err(BadSection::Length(at));
            }
            let origin = IVec3::from_array(at) * SECTION;
            for (index, word) in words.into_iter().enumerate() {
                let Some((mut cube, place)) = Cube::unpack(word) else {
                    continue;
                };
                if place > 0 {
                    let part = section.parts.get(usize::from(place) - 1);
                    cube.part = Some(*part.ok_or(BadSection::Part(at))?);
                }
                let side = SECTION as usize;
                let local = IVec3::new(
                    (index / (side * side)) as i32,
                    (index / side % side) as i32,
                    (index % side) as i32,
                );
                cubes
                    .set(origin + local, Some(cube))
                    .map_err(|_| BadSection::Part(at))?;
            }
        }
        Ok(cubes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_cube() -> Vec<Cube> {
        let mut all = Vec::new();
        for shape in Shape::ALL {
            for level in [0, 1, 8, 15, FULL] {
                for pending in [
                    Pending::None,
                    Pending::Replace,
                    Pending::Carve,
                    Pending::Swapped,
                ] {
                    for part in [None, Some(PartId(7))] {
                        all.push(
                            Cube {
                                kind: Kind(3),
                                fill: Fill::Solid,
                                shape,
                                state: State::Built,
                                pending,
                                part,
                            }
                            .at_level(level),
                        );
                    }
                }
            }
        }
        all.push(Cube::new(Kind::LAST, Fill::Open));
        all.push(Cube::new(Kind::PLAIN, Fill::Open).at_level(4));
        all
    }

    #[test]
    fn every_cube_packs_into_32_bits_and_back() {
        for cube in every_cube() {
            let word = cube.pack(u8::from(cube.part.is_some()));
            let (mut back, place) = Cube::unpack(word).expect("not air");
            back.part = (place > 0).then_some(PartId(7));
            assert_eq!(back, cube);
        }
        assert_eq!(Cube::unpack(0), None);
    }

    #[test]
    fn every_shape_has_its_own_code_and_turns_round_to_itself() {
        for (code, shape) in Shape::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(shape.code()), code, "{shape:?}");
            assert_eq!(Shape::of_code(shape.code()), Some(shape));
            assert_eq!(shape.turned(4), shape);
            assert_eq!(
                shape.turned(1).code() == shape.code(),
                shape.facing().is_none()
            );
        }
        assert!(Shape::ALL.len() < OLD_GLASS as usize);
    }

    #[test]
    fn a_shapes_top_and_bottom_bound_what_it_fills() {
        use Facing::*;
        // An outer corner rising north and east peaks at the north east corner; an inner one
        // is low only at the south west.
        let (outer, inner) = (Shape::OuterCorner(North), Shape::InnerCorner(North));
        assert_eq!(outer.top(1.0, 0.0), 1.0);
        assert_eq!(outer.top(0.0, 0.0), 0.0);
        assert_eq!(outer.top(1.0, 1.0), 0.0);
        assert_eq!(inner.top(0.0, 1.0), 0.0);
        assert_eq!(inner.top(0.0, 0.0), 1.0);
        assert_eq!(inner.top(1.0, 1.0), 1.0);
        // Upside down, a slope rising east is whole at the east and nothing at the west.
        let under = Shape::InvertedSlope(East);
        assert_eq!((under.bottom(1.0, 0.5), under.top(1.0, 0.5)), (0.0, 1.0));
        assert_eq!(under.bottom(0.0, 0.5), 1.0);
        assert_eq!(Shape::UpperHalf.bottom(0.3, 0.3), 0.5);
        // A stair's steps, either way up, are as much of the cube as its top and bottom say.
        for shape in [Shape::Stair(South), Shape::InvertedStair(West)] {
            let boxes = shape.steps();
            let volume: f32 = boxes
                .iter()
                .map(|(lo, hi)| (hi[0] - lo[0]) * (hi[1] - lo[1]) * (hi[2] - lo[2]))
                .sum();
            assert!((volume - 0.75).abs() < 1e-6, "{shape:?}");
            for (u, v) in [(0.1, 0.1), (0.9, 0.9), (0.1, 0.9), (0.9, 0.1)] {
                let filled = |y: f32| {
                    boxes.iter().any(|(lo, hi)| {
                        (lo[0]..=hi[0]).contains(&u)
                            && (lo[1]..=hi[1]).contains(&y)
                            && (lo[2]..=hi[2]).contains(&v)
                    })
                };
                assert!(filled(shape.top(u, v) - 0.01) && !filled(shape.top(u, v) + 0.01));
                assert!(!filled(shape.bottom(u, v) - 0.01) && filled(shape.bottom(u, v) + 0.01));
            }
        }
    }

    #[test]
    fn a_word_packed_before_five_shape_bits_reads_as_the_same_cube() {
        // Packed as cubes were before D231, by hand.
        let v1 = |cube: &Cube, part: u32| {
            let shape = match (cube.fill, cube.shape) {
                (Fill::Open, _) => 15,
                (_, shape) => u32::from(shape.code()),
            };
            let (level, built) = match cube.state {
                State::Built => (0, 1),
                State::Planned { level } => (u32::from(level), 0),
            };
            (u32::from(cube.kind.0) + 1)
                | shape << 12
                | level << 16
                | built << 20
                | (cube.pending as u32) << 21
                | part << 24
        };
        let old = every_cube()
            .into_iter()
            .filter(|cube| cube.shape.code() <= 10 && !matches!(cube.pending, Pending::Swapped));
        for cube in old {
            let part = u32::from(cube.part.is_some());
            assert_eq!(from_v1(v1(&cube, part)), cube.pack(part as u8), "{cube:?}");
        }
        assert_eq!(from_v1(0), 0);
    }

    #[test]
    fn glass_from_before_d225_reads_as_an_opening() {
        // A built cube of kind 2 packed as glass now (30) and before D231 (14), and an op
        // written with a glass cube: each is an opening.
        let open = Cube::new(Kind(2), Fill::Open);
        let (glass, _) = Cube::unpack(3 | OLD_GLASS << 12 | 1 << 21).expect("not air");
        assert_eq!(glass, open);
        let (glass_v1, _) = Cube::unpack(from_v1(3 | 14 << 12 | 1 << 20)).expect("not air");
        assert_eq!(glass_v1, open);
        let written: Cube = ron::from_str("(kind: (2), fill: Glass)").unwrap();
        assert_eq!(written, open);
    }

    #[test]
    fn cubes_come_out_sorted_across_sections() {
        let mut cubes = Cubes::default();
        let mut sorted = BTreeMap::new();
        let mut seed = 1u64;
        for _ in 0..600 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let at = IVec3::new(
                (seed >> 20) as i32 % 50 - 25,
                (seed >> 30) as i32 % 40 - 20,
                (seed >> 40) as i32 % 50 - 25,
            );
            let cube = Cube::planned(Kind((seed >> 50) as u16 % 4), Some(PartId(seed as u32 % 5)));
            cubes.set(at, Some(cube)).unwrap();
            sorted.insert(at.to_array(), cube);
        }
        let got: Vec<_> = cubes
            .iter()
            .map(|(at, cube)| (at.to_array(), cube))
            .collect();
        let want: Vec<_> = sorted.into_iter().collect();
        assert_eq!(got, want);
        assert_eq!(cubes.len(), got.len());
        assert_eq!(cubes.planned(), got.len());
    }

    #[test]
    fn a_change_marks_its_section_and_the_neighbours_on_its_edge() {
        let mut cubes = Cubes::default();
        cubes
            .set(IVec3::new(5, 5, 5), Some(Cube::new(Kind(1), Fill::Solid)))
            .unwrap();
        assert_eq!(cubes.take_dirty(), BTreeSet::from([[0, 0, 0]]));
        cubes
            .set(IVec3::new(0, 15, 5), Some(Cube::new(Kind(1), Fill::Solid)))
            .unwrap();
        assert_eq!(
            cubes.take_dirty(),
            BTreeSet::from([[-1, 0, 0], [0, 0, 0], [0, 1, 0]])
        );
        // Setting what is there already changes nothing.
        cubes
            .set(IVec3::new(5, 5, 5), Some(Cube::new(Kind(1), Fill::Solid)))
            .unwrap();
        assert!(cubes.dirty().is_empty());
        // An emptied section goes.
        cubes.set(IVec3::new(5, 5, 5), None).unwrap();
        cubes.set(IVec3::new(0, 15, 5), None).unwrap();
        assert_eq!(cubes.sections().count(), 0);
        assert!(cubes.is_empty());
    }

    #[test]
    fn a_section_holds_255_parts_and_frees_the_place_of_one_gone() {
        let mut cubes = Cubes::default();
        let at = |i: i32| IVec3::new(i % 16, i / 16 % 16, 0);
        for i in 0..255 {
            let cube = Cube::planned(Kind(1), Some(PartId(i as u32)));
            cubes.set(at(i), Some(cube)).unwrap();
        }
        let one_more = Some(Cube::planned(Kind(1), Some(PartId(999))));
        assert_eq!(cubes.set(at(255), one_more), Err(TooManyParts));
        assert!(!cubes.has_room(IVec3::ZERO, &BTreeSet::from([PartId(999)])));
        assert!(cubes.has_room(IVec3::ZERO, &BTreeSet::from([PartId(3)])));
        cubes.set(at(3), None).unwrap();
        assert!(cubes.set(at(255), one_more).is_ok());
        assert_eq!(cubes.get(at(255)).unwrap().part, Some(PartId(999)));
        assert_eq!(cubes.get(at(4)).unwrap().part, Some(PartId(4)));
    }

    #[test]
    fn sections_save_as_runs_and_load_the_same() {
        let mut cubes = Cubes::default();
        for (i, cube) in every_cube().into_iter().enumerate() {
            let i = i as i32;
            cubes
                .set(IVec3::new(i % 20 - 3, i / 20 - 2, 7), Some(cube))
                .unwrap();
        }
        let saved = cubes.save();
        let loaded = Cubes::load(&saved).unwrap();
        assert_eq!(loaded, cubes);
        assert_eq!(loaded.save(), saved);
        // A mostly empty section is a few runs, not four thousand words.
        let mut one = Cubes::default();
        one.set(IVec3::new(3, 4, 5), Some(Cube::new(Kind(1), Fill::Solid)))
            .unwrap();
        assert_eq!(one.save()[0].runs.len(), 3);
    }

    #[test]
    fn a_malformed_section_is_refused_for_what_is_wrong_with_it() {
        let air = |runs: Vec<(u16, u32)>| SavedSection {
            at: [1, 2, 3],
            parts: Vec::new(),
            runs,
        };
        let whole = SECTION_CUBES as u16;
        assert!(Cubes::load(&[air(vec![(whole, 0)])]).is_ok());
        // Runs short of a section, or past its end.
        for runs in [vec![(whole - 1, 0)], vec![(whole, 0), (1, 0)]] {
            assert_eq!(
                Cubes::load(&[air(runs)]).err(),
                Some(BadSection::Length([1, 2, 3]))
            );
        }
        // Runs far past its end, as a snapshot sent to bring a server down would have:
        // refused before they are spread out, which would take 275 GB.
        let hostile = air(vec![(u16::MAX, 0); 1 << 20]);
        assert_eq!(
            Cubes::load(&[hostile]).err(),
            Some(BadSection::Length([1, 2, 3]))
        );
        // A cube in the second part of a palette of one.
        let cube = Cube::planned(Kind(1), Some(PartId(4))).pack(2);
        let mut bad = air(vec![(1, cube), (whole - 1, 0)]);
        bad.parts = vec![PartId(4)];
        assert_eq!(
            Cubes::load(&[bad.clone()]).err(),
            Some(BadSection::Part([1, 2, 3]))
        );
        bad.parts.push(PartId(9));
        let loaded = Cubes::load(&[bad.clone()]).unwrap();
        assert_eq!(
            loaded.get(IVec3::new(16, 32, 48)).unwrap().part,
            Some(PartId(9))
        );
        // The same section twice.
        assert_eq!(
            Cubes::load(&[bad.clone(), bad]).err(),
            Some(BadSection::Twice([1, 2, 3]))
        );
    }
}
