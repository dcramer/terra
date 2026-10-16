//! A market town from its seed and the ground it stands on (docs/demos.md, a living town;
//! D324): a square where the roads meet, streets out of it along the slope's contours,
//! plots along each with the houses' doors to the street, yards behind with sheds and pens,
//! the things a town's day is done at (a well, stalls, a forge, chopping blocks, benches, a
//! trough, guard posts, the gates its people go out to the fields by), and the path graph
//! its people walk.
//!
//! **How it is laid out**, as Emilien et al. lay villages (2012) and Ghost Recon Wildlands
//! adapted them: the square on the flattest ground, which the game finds, its long side
//! along the contour; streets traced out of it a few metres at a time, each step turned the
//! way the ground is most level, so they follow the slope rather than climb it; plots cut
//! along each side of a street and round the square, each building's door to it, its front
//! set back by how far its own wings come forward; a yard behind each house where there is
//! room. Every building is one of [`village`]'s, put up by the same shortcuts as a player's
//! base (D108): nothing is a prefab.
//!
//! **What varies** (D182): the square is long, a triangle where three roads meet, or a
//! widened street; two or three streets leave it, each bending with its own ground; its
//! thirty to forty buildings are drawn from the three hundred the rules make, in an order
//! the seed shuffles; the square's stalls, well and benches stand where its seed puts
//! them. Three shapes by two or three streets by the ground under them make a few dozen
//! towns a player can tell apart, and each one's houses are its own.
//!
//! **Where things are**: in metres east and south of the square's middle (`x` east, `y`
//! south), as [`village`] gives a village's. A building's door is at its `at`, looking the
//! way `facing` turns from south toward east.
//!
//! **The path graph** is what the town's people walk (D324): street centre lines, a grid
//! over the square, a node before every door, every thing's place to stand and every yard,
//! joined where nothing built is in the way. It needs no navmesh: the layout knows every
//! street.

use crate::layout::{hash, unit};
use crate::village::{self, Building, Kinds, Picks, Plan, Roof, Scheme};
use crate::{Base, CELL_M, STOREY_CUBES};
use glam::Vec2;
use std::collections::BTreeSet;

mod enclosure;
mod furnish;
mod market;
pub use enclosure::{Enclosure, enclosure};
pub use furnish::{FURNISHING, Fence, FenceKind, Furnishing};
pub use market::{Counter, Good, Market, Trade};

/// The shape of a town's square.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SquareShape {
    /// A rectangle, its long side along the contour; the main street out of its ends.
    Long,
    /// A triangle where three roads meet, its base along the contour.
    Triangle,
    /// The main street widened into a long, narrow market.
    Widened,
}

/// A town's square: its outline, a convex polygon in order round it.
#[derive(Clone, Debug, PartialEq)]
pub struct Square {
    pub shape: SquareShape,
    pub outline: Vec<Vec2>,
    /// Along its length, the contour at its middle; and across it, up the slope.
    pub along: Vec2,
    pub across: Vec2,
}

/// A street: its centre line, a point every few metres out from the square, and how wide it
/// is.
#[derive(Clone, Debug, PartialEq)]
pub struct Street {
    pub points: Vec<Vec2>,
    pub wide: f32,
}

/// What a thing in the town is, which says what can be done at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThingKind {
    /// A house's door: where its household goes in and out.
    Door,
    Well,
    /// A market stall, its counter to the front.
    Stall,
    Forge,
    /// A chopping block in a yard.
    Block,
    /// A bench on the square.
    Bench,
    /// A water trough by the well.
    Trough,
    /// A guard's post near a street's end.
    Post,
    /// Where a few people stand and talk.
    Gathering,
    /// A street's far end, out to the fields: people go out and come back by it.
    Gate,
    /// A pen of low walls behind a house, its middle; its node is before its gate.
    Pen,
    /// A yard behind a house, where hens scratch.
    Yard,
    /// A barrel standing by a stall, the forge or in a yard (D346).
    Barrel,
    /// A hand cart, by the market or in a yard, its shafts to its front (D346).
    Cart,
    /// Room on the ground beside a stall for the bigger goods of its trade (D352).
    Goods,
    /// Where builders stand to work on a building going up, facing its wall (D381): laid
    /// only for a settlement being built, open while its building rises.
    Site,
}

impl ThingKind {
    /// Every kind, in order.
    pub const ALL: [ThingKind; 16] = [
        ThingKind::Door,
        ThingKind::Well,
        ThingKind::Stall,
        ThingKind::Forge,
        ThingKind::Block,
        ThingKind::Bench,
        ThingKind::Trough,
        ThingKind::Post,
        ThingKind::Gathering,
        ThingKind::Gate,
        ThingKind::Pen,
        ThingKind::Yard,
        ThingKind::Barrel,
        ThingKind::Cart,
        ThingKind::Goods,
        ThingKind::Site,
    ];

    /// Its name, as data names it (`assets/data/life.ron`).
    pub fn name(self) -> &'static str {
        match self {
            ThingKind::Door => "Door",
            ThingKind::Well => "Well",
            ThingKind::Stall => "Stall",
            ThingKind::Forge => "Forge",
            ThingKind::Block => "Block",
            ThingKind::Bench => "Bench",
            ThingKind::Trough => "Trough",
            ThingKind::Post => "Post",
            ThingKind::Gathering => "Gathering",
            ThingKind::Gate => "Gate",
            ThingKind::Pen => "Pen",
            ThingKind::Yard => "Yard",
            ThingKind::Barrel => "Barrel",
            ThingKind::Cart => "Cart",
            ThingKind::Goods => "Goods",
            ThingKind::Site => "Site",
        }
    }

    /// The ground it covers that a walker must go round, in its own metres (x across its
    /// front, y out of it) from low to high, as its model stands (`assets/models/props`);
    /// nothing for what is walked over. A stall's is its counter, at its front: the seller
    /// stands behind it, under the awning.
    pub fn ground(self) -> Option<(Vec2, Vec2)> {
        let across =
            |x: f32, back: f32, front: f32| Some((Vec2::new(-x, back), Vec2::new(x, front)));
        match self {
            ThingKind::Well => across(0.85, -0.87, 0.87),
            ThingKind::Stall => across(1.3, 0.0, 1.0),
            ThingKind::Forge => across(1.3, -0.77, 0.77),
            ThingKind::Block => across(0.3, -0.3, 0.3),
            ThingKind::Bench => across(0.9, -0.2, 0.2),
            ThingKind::Trough => across(1.0, -0.35, 0.35),
            ThingKind::Barrel => across(0.29, -0.29, 0.29),
            ThingKind::Cart => across(0.7, -1.35, 1.35),
            ThingKind::Goods => across(0.26, -0.43, 0.43),
            _ => None,
        }
    }

    /// Where someone coming to it stands, in its own metres: before it, and for a stall
    /// behind it too, where its seller comes in.
    pub fn stands(self) -> Vec<Vec2> {
        let front = self.ground().map_or(0.0, |(_, high)| high.y) + 0.75;
        match self {
            ThingKind::Stall => vec![Vec2::new(0.0, front), Vec2::new(0.0, -1.6)],
            _ => vec![Vec2::new(0.0, front)],
        }
    }
}

/// A thing in the town: what it is, where its middle stands, which way its front looks,
/// the building it belongs to if any (a house's door, a yard's chopping block), and its
/// node on the path graph, where someone coming to it arrives.
#[derive(Clone, Debug, PartialEq)]
pub struct Thing {
    pub kind: ThingKind,
    pub at: Vec2,
    pub facing: f32,
    pub building: Option<usize>,
    pub node: u32,
}

impl Thing {
    fn new(kind: ThingKind, at: Vec2, facing: f32, building: Option<usize>) -> Self {
        Thing {
            kind,
            at,
            facing,
            building,
            node: u32::MAX,
        }
    }

    /// The way its front looks, as a direction east and south.
    pub fn front(&self) -> Vec2 {
        looking(self.facing)
    }

    /// The ground it covers, a rectangle in order round it, if it stands in a walker's way.
    pub fn outline(&self) -> Option<[Vec2; 4]> {
        let (low, high) = self.kind.ground()?;
        Some(turned_box(self.at, self.facing, low, high))
    }

    /// A place in its own metres (x across its front, y out of its front) in the town.
    pub fn place(&self, local: Vec2) -> Vec2 {
        place(self.at, self.facing, local)
    }
}

/// What a node of the path graph is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum On {
    /// On a street's centre line.
    Street(usize),
    /// On the square.
    Square,
    /// Before a thing, where someone coming to it arrives.
    Thing(usize),
    /// Along the side of a building, on the way to its yard.
    Lane(usize),
}

/// The path graph the town's people walk: places on the ground and the straight walks
/// between them that nothing built stands across.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Paths {
    pub nodes: Vec<(Vec2, On)>,
    pub edges: Vec<[u32; 2]>,
}

/// A market town.
#[derive(Clone, Debug, PartialEq)]
pub struct Town {
    pub square: Square,
    pub streets: Vec<Street>,
    /// Its buildings, in [`village`]'s terms.
    pub buildings: Vec<Building>,
    /// Each building's ground, as rectangles in order round each: what walkers go round.
    pub footprints: Vec<Vec<[Vec2; 4]>>,
    /// Which buildings are homes: houses a household lives in.
    pub homes: Vec<usize>,
    /// Each home's yard behind it, if it has room for one, as a rectangle; and the lane to
    /// it, from before its door along the house's side to the yard's gate.
    pub yards: Vec<(usize, [Vec2; 4])>,
    pub lanes: Vec<(usize, Vec<Vec2>)>,
    pub things: Vec<Thing>,
    /// The fences round its yards, a module at a time (D346).
    pub fences: Vec<Fence>,
    pub paths: Paths,
}

/// The ground under a town: its height in metres at a place east and south of the middle.
pub type Heights<'a> = &'a dyn Fn(Vec2) -> f32;

/// How many buildings a town has, at least and at most, sheds and walls not counted.
pub const BUILDINGS: (usize, usize) = (30, 40);

/// How wide a town's main streets are, and its others, in metres.
const MAIN_WIDE_M: f32 = 3.8;
const SIDE_WIDE_M: f32 = 3.3;
/// How far a street's centre line steps, and how far it may turn at each step.
const STEP_M: f32 = 6.0;
const TURN: f32 = 0.11;
/// How far out along a street from the square its houses are of two storeys where they can be.
const TALL_WITHIN_M: f32 = 45.0;
/// The share of those that are of two storeys, the rest the queue's next.
const TALL_SHARE: f32 = 0.7;
/// How many of the queue's next buildings a plot on the square tries before it moves on.
const SQUARE_TRIES: usize = 12;
/// How far apart two buildings stand at the least, and a building from a street's edge.
/// A town's houses stand joined along its ways, wall to wall as Kingdom Come's Rattay's and
/// Kuttenberg's do, so a street is a few metres between closed fronts; every few a passage
/// is left between two for the lane to the yards behind.
const APART_M: f32 = 0.1;
const OFF_STREET_M: f32 = 0.3;
const PASSAGE_M: f32 = 1.8;
/// How far from the square's corners its fronts begin and end.
const SQUARE_CORNER_M: f32 = 0.8;
/// How far along a street from the square its first house stands: close, so the square's
/// corners are closed by the streets' first houses.
const STREET_FIRST_M: f32 = 0.5;
/// How far apart the stalls in a market's row stand, middle to middle.
const STALL_STEP_M: f32 = 3.2;
/// How far out before a door the way to it is kept clear, and how far either side of it.
const DOOR_WAY_M: f32 = 2.0;
const DOOR_ABREAST_M: f32 = 1.0;
/// How far a house on the square stands back from its edge.
const SQUARE_OFF_M: f32 = 0.6;
const PASSAGE_EVERY: u32 = 3;
/// The square's grid of path nodes, how far apart.
const GRID_M: f32 = 4.0;
/// How far behind a house its yard begins: a way along its back to the yard's gate.
const BEHIND_M: f32 = 1.6;
/// How far the paths keep from where people gather: round the few standing and talking, not
/// through them.
const GATHERED_M: f32 = 1.8;
/// How far a walk between two nodes keeps from anything built.
pub const CLEAR_M: f32 = 0.45;

/// The town's buildings, square, streets, things and paths from its seed, on the ground
/// `heights` gives: the same seed and ground, the same town.
pub fn town(seed: u32, picks: impl Picks + Copy, heights: Heights) -> Town {
    let (along, uphill) = contour(heights, Vec2::ZERO, 8.0);
    let shape = match hash(&[seed, 1]) % 3 {
        0 => SquareShape::Long,
        1 => SquareShape::Triangle,
        _ => SquareShape::Widened,
    };
    let square = square(seed, shape, along, uphill);
    let streets = streets(seed, &square, heights);
    let mut town = Town {
        square,
        streets,
        buildings: Vec::new(),
        footprints: Vec::new(),
        homes: Vec::new(),
        yards: Vec::new(),
        lanes: Vec::new(),
        things: Vec::new(),
        fences: Vec::new(),
        paths: Paths::default(),
    };
    town.on_the_square(seed);
    town.plots(seed, picks);
    town.yards(seed, picks);
    town.furnish(seed);
    town.ends();
    town.paths();
    town.unfurnish_cut_off();
    town
}

/// Which way the ground runs level at a place, and which way is up it: the contour and the
/// slope, each a unit direction east and south. On level ground, east and south.
pub fn contour(heights: Heights, at: Vec2, across_m: f32) -> (Vec2, Vec2) {
    let slope = slope(heights, at, across_m);
    if slope.length() < 1e-3 {
        return (Vec2::X, Vec2::Y);
    }
    let uphill = slope.normalize();
    (uphill.perp(), uphill)
}

/// How the ground rises at a place, in metres a metre east and south.
pub fn slope(heights: Heights, at: Vec2, across_m: f32) -> Vec2 {
    let d = across_m;
    Vec2::new(
        heights(at + Vec2::X * d) - heights(at - Vec2::X * d),
        heights(at + Vec2::Y * d) - heights(at - Vec2::Y * d),
    ) / (2.0 * d)
}

/// The way a facing looks: turned from south (+y) toward east (+x).
pub fn looking(facing: f32) -> Vec2 {
    Vec2::new(libm::sinf(facing), libm::cosf(facing))
}

/// The facing that looks a way.
pub fn facing_of(way: Vec2) -> f32 {
    libm::atan2f(way.x, way.y)
}

/// A point in a building's or thing's own metres (x across its front, y out of its door)
/// where it stands in the town, its door or middle at `at` looking the way `facing` turns.
pub fn place(at: Vec2, facing: f32, local: Vec2) -> Vec2 {
    let (s, c) = (libm::sinf(facing), libm::cosf(facing));
    at + Vec2::new(c, -s) * local.x + Vec2::new(s, c) * local.y
}

/// A box from `low` to `high` in a thing's own metres, where it stands, in order round it.
fn turned_box(at: Vec2, facing: f32, low: Vec2, high: Vec2) -> [Vec2; 4] {
    [
        place(at, facing, low),
        place(at, facing, Vec2::new(high.x, low.y)),
        place(at, facing, high),
        place(at, facing, Vec2::new(low.x, high.y)),
    ]
}

/// The ground a building covers in its own metres (x across its front, y out of its door),
/// as rectangles from low to high: its cubes on its ground storey, roofs left out, and
/// whatever its walls close in all round filled.
pub fn ground_rects(building: &Building) -> Vec<(Vec2, Vec2)> {
    let base = Base::from_shortcuts(&building.ops);
    let cells: BTreeSet<(i32, i32)> = base
        .cubes()
        .filter(|(at, cube)| !cube.kind.is_roof() && (0..STOREY_CUBES).contains(&at.y))
        .map(|(at, _)| (at.x, at.z))
        .collect();
    if cells.is_empty() {
        return Vec::new();
    }
    let (x0, x1) = (
        cells.iter().map(|c| c.0).min().unwrap_or(0),
        cells.iter().map(|c| c.0).max().unwrap_or(0),
    );
    let (z0, z1) = (
        cells.iter().map(|c| c.1).min().unwrap_or(0),
        cells.iter().map(|c| c.1).max().unwrap_or(0),
    );
    // Closed in: a cell with something built either side of it along its row and its
    // column, so a house's floor counts and a U's yard before its door does not.
    let built = |x: i32, z: i32| cells.contains(&(x, z));
    let closed = |x: i32, z: i32| {
        built(x, z)
            || ((x0..x).any(|a| built(a, z))
                && (x + 1..=x1).any(|a| built(a, z))
                && (z0..z).any(|a| built(x, a))
                && (z + 1..=z1).any(|a| built(x, a)))
    };
    // Runs along each row, merged with the same run on the rows after.
    let mut open: Vec<(i32, i32, i32, i32)> = Vec::new();
    let mut rects = Vec::new();
    for z in z0..=z1 + 1 {
        let mut runs = Vec::new();
        if z <= z1 {
            let mut x = x0;
            while x <= x1 {
                if closed(x, z) {
                    let start = x;
                    while x <= x1 && closed(x, z) {
                        x += 1;
                    }
                    runs.push((start, x - 1));
                } else {
                    x += 1;
                }
            }
        }
        let mut still = Vec::new();
        for (a, b, from, _) in open.drain(..) {
            if let Some(index) = runs.iter().position(|&run| run == (a, b)) {
                runs.remove(index);
                still.push((a, b, from, z));
            } else {
                rects.push((a, b, from, z - 1));
            }
        }
        still.extend(runs.into_iter().map(|(a, b)| (a, b, z, z)));
        open = still;
    }
    rects
        .into_iter()
        .map(|(a, b, from, to)| {
            (
                Vec2::new(a as f32, from as f32) * CELL_M,
                Vec2::new((b + 1) as f32, (to + 1) as f32) * CELL_M,
            )
        })
        .collect()
}

/// A building's ground in the town, where it stands.
fn footprint(building: &Building, rects: &[(Vec2, Vec2)]) -> Vec<[Vec2; 4]> {
    rects
        .iter()
        .map(|&(low, high)| turned_box(Vec2::from(building.at), building.facing, low, high))
        .collect()
}

/// The square: a shape of the seed's size, its middle at the origin, along the contour.
fn square(seed: u32, shape: SquareShape, along: Vec2, across: Vec2) -> Square {
    let size = |salt: u32, low: f32, high: f32| low + unit(hash(&[seed, 2, salt])) * (high - low);
    let corner = |a: f32, u: f32| along * a + across * u;
    let outline = match shape {
        SquareShape::Long => {
            let (long, wide) = (size(1, 28.0, 34.0) * 0.5, size(2, 18.0, 21.0) * 0.5);
            vec![
                corner(-long, -wide),
                corner(long, -wide),
                corner(long, wide),
                corner(-long, wide),
            ]
        }
        SquareShape::Triangle => {
            // Its base downhill, its point up the slope, its middle at the origin.
            let (base, deep) = (size(1, 42.0, 46.0) * 0.5, size(2, 27.0, 30.0));
            vec![
                corner(-base, -deep / 3.0),
                corner(base, -deep / 3.0),
                corner(0.0, deep * 2.0 / 3.0),
            ]
        }
        SquareShape::Widened => {
            let (long, wide) = (size(1, 44.0, 50.0) * 0.5, size(2, 12.0, 14.0) * 0.5);
            vec![
                corner(-long, -wide),
                corner(long, -wide),
                corner(long, wide),
                corner(-long, wide),
            ]
        }
    };
    Square {
        shape,
        outline,
        along,
        across,
    }
}

/// The lowest and highest a polygon reaches along a way.
fn extent(outline: &[Vec2], way: Vec2) -> (f32, f32) {
    outline
        .iter()
        .map(|p| p.dot(way))
        .fold((f32::MAX, f32::MIN), |(l, h), x| (l.min(x), h.max(x)))
}

/// The streets out of the square: the main street out of either end along the contour, and
/// for some squares a third, from a corner or a side, up or down the slope until it can
/// turn along it.
fn streets(seed: u32, square: &Square, heights: Heights) -> Vec<Street> {
    let (along, across) = (square.along, square.across);
    let long = |salt: u32| 80.0 + unit(hash(&[seed, 3, salt])) * 25.0;
    let mut starts: Vec<(Vec2, Vec2, f32, f32)> = Vec::new();
    match square.shape {
        SquareShape::Triangle => {
            // From each corner of its base along the contour, and from its point up the slope.
            let [a, b, c] = [square.outline[0], square.outline[1], square.outline[2]];
            starts.push((a, -along, long(1), MAIN_WIDE_M));
            starts.push((b, along, long(2), MAIN_WIDE_M));
            starts.push((c, across, long(3) * 0.9, SIDE_WIDE_M));
        }
        _ => {
            let (low, high) = extent(&square.outline, along);
            starts.push((along * low, -along, long(1), MAIN_WIDE_M));
            starts.push((along * high, along, long(2), MAIN_WIDE_M));
            if !hash(&[seed, 4]).is_multiple_of(3) {
                // A side street from one long side, up or down the slope.
                let way = if hash(&[seed, 5]).is_multiple_of(2) {
                    across
                } else {
                    -across
                };
                let shift = (unit(hash(&[seed, 6])) - 0.5) * (high - low) * 0.4;
                let (_, edge) = extent(&square.outline, way);
                starts.push((along * shift + way * edge, way, long(3) * 0.9, SIDE_WIDE_M));
            }
        }
    }
    let mut streets: Vec<Street> = Vec::new();
    for (index, (from, way, length, wide)) in starts.into_iter().enumerate() {
        let points = trace(
            hash(&[seed, 7, index as u32]),
            from,
            way,
            length,
            heights,
            &streets,
            &square.outline,
            // A main street leaves the square straight; a side street climbs or falls away
            // from it far enough for a row of plots between, then turns along the slope.
            if wide == MAIN_WIDE_M { 2 } else { 5 },
        );
        streets.push(Street { points, wide });
    }
    streets
}

/// A street's centre line traced from a point a way out, a step at a time, each step turned
/// a little at most the way the ground is most level, so it follows the contour: its first
/// `out_steps` straight on, so it leaves the square cleanly, and away from the other
/// streets and the square.
#[allow(clippy::too_many_arguments)]
fn trace(
    seed: u32,
    from: Vec2,
    way: Vec2,
    length: f32,
    heights: Heights,
    others: &[Street],
    square: &[Vec2],
    out_steps: usize,
) -> Vec<Vec2> {
    let mut points = vec![from];
    let (mut at, mut heading) = (from, way.normalize());
    let steps = (length / STEP_M).ceil() as usize;
    for step in 0..steps {
        let mut best: Option<(f32, Vec2, Vec2)> = None;
        for turn in -2i32..=2 {
            let angle = turn as f32 * TURN;
            let ahead = Vec2::from_angle(angle).rotate(heading);
            let next = at + ahead * STEP_M;
            let rise = (heights(next) - heights(at)).abs() / STEP_M;
            // Straight on out of the square, then by the ground, with a little of its own.
            let straight = if step < out_steps { 4.0 } else { 0.003 };
            let mut cost = rise * 8.0
                + turn.unsigned_abs() as f32 * straight
                + unit(hash(&[seed, step as u32, turn as u32])) * 0.004;
            for street in others {
                for &point in street.points.iter().skip(2) {
                    let near = 14.0 - next.distance(point);
                    if near > 0.0 {
                        cost += near * 0.5;
                    }
                }
            }
            if step > 0 && inside(next, square) {
                cost += 100.0;
            }
            if best.is_none_or(|(least, _, _)| cost < least) {
                best = Some((cost, next, ahead));
            }
        }
        let Some((_, next, ahead)) = best else {
            break;
        };
        at = next;
        heading = ahead;
        points.push(at);
    }
    points
}

/// Whether a point is inside a convex polygon, given in order round it either way.
pub fn inside(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut sign = 0.0f32;
    for (index, &a) in polygon.iter().enumerate() {
        let b = polygon[(index + 1) % polygon.len()];
        let side = (b - a).perp_dot(point - a);
        if side.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = side.signum();
        } else if side.signum() != sign {
            return false;
        }
    }
    true
}

/// How far a point is from a segment.
pub fn point_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let along = if ab.length_squared() > 0.0 {
        ((point - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    point.distance(a + ab * along)
}

/// How far two segments are apart: nothing where they cross.
pub fn segments(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> f32 {
    let (ab, cd) = (b - a, d - c);
    let denominator = ab.perp_dot(cd);
    if denominator.abs() > 1e-9 {
        let s = (c - a).perp_dot(cd) / denominator;
        let t = (c - a).perp_dot(ab) / denominator;
        if (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&t) {
            return 0.0;
        }
    }
    point_segment(a, c, d)
        .min(point_segment(b, c, d))
        .min(point_segment(c, a, b))
        .min(point_segment(d, a, b))
}

/// How far a segment is from a convex polygon: nothing where it touches or is inside.
pub fn segment_polygon(a: Vec2, b: Vec2, polygon: &[Vec2]) -> f32 {
    if inside(a, polygon) || inside(b, polygon) {
        return 0.0;
    }
    (0..polygon.len())
        .map(|i| segments(a, b, polygon[i], polygon[(i + 1) % polygon.len()]))
        .fold(f32::MAX, f32::min)
}

/// How far two convex polygons are apart: nothing where they overlap.
pub fn polygons(p: &[Vec2], q: &[Vec2]) -> f32 {
    if p.iter().any(|&v| inside(v, q)) || q.iter().any(|&v| inside(v, p)) {
        return 0.0;
    }
    (0..p.len())
        .map(|i| segment_polygon(p[i], p[(i + 1) % p.len()], q))
        .fold(f32::MAX, f32::min)
}

/// How far a polygon is from a street's centre line.
fn from_street(polygon: &[Vec2], street: &Street) -> f32 {
    street
        .points
        .windows(2)
        .map(|pair| segment_polygon(pair[0], pair[1], polygon))
        .fold(f32::MAX, f32::min)
}

/// A point a way along a line of points, and the way the line runs there.
fn along_line(points: &[Vec2], distance: f32) -> Option<(Vec2, Vec2)> {
    let mut left = distance;
    for pair in points.windows(2) {
        let length = pair[0].distance(pair[1]);
        if left <= length && length > 0.0 {
            let way = (pair[1] - pair[0]) / length;
            return Some((pair[0] + way * left, way));
        }
        left -= length;
    }
    None
}

fn line_length(points: &[Vec2]) -> f32 {
    points
        .windows(2)
        .map(|pair| pair[0].distance(pair[1]))
        .sum()
}

/// Where plots are cut along: a line, which side of it the buildings stand on, and how far
/// from it their fronts must stand.
struct Front {
    points: Vec<Vec2>,
    side: f32,
    off: f32,
    at: f32,
    done: bool,
    /// The square's edges take its larger buildings.
    grand: bool,
}

impl Town {
    /// Where builders stand to put each building up (D381): a [`ThingKind::Site`] a little
    /// out from the middle of every side of its ground that is not against another's, and
    /// from each corner along the longer sides, facing the wall, arrived at by its door's
    /// node. Laid only for a settlement being built (a time-lapse of one), after the rest of
    /// the town, so every other thing keeps its place in the list. How many were laid.
    pub fn add_sites(&mut self) -> usize {
        const OUT_M: f32 = 1.1;
        let before = self.things.len();
        for (index, ground) in self.footprints.iter().enumerate() {
            let Some(node) = self
                .things
                .iter()
                .find(|t| t.kind == ThingKind::Door && t.building == Some(index))
                .map(|t| t.node)
            else {
                continue;
            };
            for quad in ground {
                let middle = quad.iter().copied().sum::<Vec2>() / 4.0;
                for side in 0..4 {
                    let (a, b) = (quad[side], quad[(side + 1) % 4]);
                    let along = b - a;
                    let length = along.length();
                    if length < 1.5 {
                        continue;
                    }
                    let centre = (a + b) * 0.5;
                    let out = (centre - middle).normalize_or_zero();
                    let mut places = vec![0.5];
                    if length > 6.0 {
                        places.extend([0.2, 0.8]);
                    }
                    for share in places {
                        let at = a + along * share + out * OUT_M;
                        let blocked = self
                            .footprints
                            .iter()
                            .flatten()
                            .any(|other| inside(at, other))
                            || self
                                .things
                                .iter()
                                .filter_map(Thing::outline)
                                .any(|other| inside(at, &other));
                        if blocked {
                            continue;
                        }
                        let mut site =
                            Thing::new(ThingKind::Site, at, facing_of(-out), Some(index));
                        site.node = node;
                        self.things.push(site);
                    }
                }
            }
        }
        self.things.len() - before
    }

    /// Whether a building's ground, where it would stand, keeps clear of everything there:
    /// other buildings, yards, the streets, the square and its things.
    fn clear_for(&self, ground: &[[Vec2; 4]], apart: f32) -> bool {
        ground.iter().all(|quad| {
            self.footprints
                .iter()
                .flatten()
                .all(|other| polygons(quad, other) >= apart)
                && self
                    .yards
                    .iter()
                    .all(|(_, yard)| polygons(quad, yard) >= apart * 0.5)
                && self.streets.iter().all(|street| {
                    from_street(quad, street) >= street.wide * 0.5 + OFF_STREET_M * 0.9
                })
                && polygons(quad, &self.square.outline) >= SQUARE_OFF_M
                && self
                    .things
                    .iter()
                    .filter_map(Thing::outline)
                    .all(|thing| polygons(quad, &thing) >= 1.0)
        })
    }

    /// The well, the trough, the stalls, benches and places to gather on the square.
    fn on_the_square(&mut self, seed: u32) {
        let (along, across) = (self.square.along, self.square.across);
        let outline = self.square.outline.clone();
        let middle = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
        let (low_a, high_a) = extent(&outline, along);
        let (low_c, high_c) = extent(&outline, across);
        let roll = |salt: u32| unit(hash(&[seed, 10, salt])) - 0.5;
        // Inside the square by a margin, to be stood on.
        let well_in = |point: Vec2, margin: f32| {
            inside(point, &outline)
                && (0..outline.len()).all(|i| {
                    point_segment(point, outline[i], outline[(i + 1) % outline.len()]) >= margin
                })
        };
        let well = middle
            + along * roll(1) * (high_a - low_a) * 0.25
            + across * roll(2) * (high_c - low_c) * 0.2;
        let side = facing_of(along);
        self.things
            .push(Thing::new(ThingKind::Well, well, side, None));
        // The trough beside it is laid with the town's furnishing (`furnish`).
        // The market: two rows of stalls facing each other across an aisle, along the
        // square on the side away from the well.
        let line = middle - across * (well - middle).dot(across).signum() * (high_c - low_c) * 0.1;
        let stalls = 4 + hash(&[seed, 11]) as usize % 4;
        let mut placed = 0;
        let mut slot = 0usize;
        while placed < stalls && slot < 24 {
            let row = if slot.is_multiple_of(2) { -1.0 } else { 1.0 };
            let m = slot / 2;
            let offset = if m == 0 {
                0.0
            } else {
                let sign = if m % 2 == 1 { 1.0 } else { -1.0 };
                sign * m.div_ceil(2) as f32 * STALL_STEP_M
            } + roll(20) * 1.0;
            slot += 1;
            let at = line + along * offset + across * row * 2.7;
            let facing = facing_of(across * -row);
            let stall = Thing::new(ThingKind::Stall, at, facing, None);
            let Some(ground) = stall.outline() else {
                continue;
            };
            let fits = ground.iter().all(|&corner| well_in(corner, 1.5))
                && stall
                    .kind
                    .stands()
                    .iter()
                    .all(|&stand| well_in(stall.place(stand), 0.5))
                && self.things.iter().all(|other| match other.outline() {
                    // A market's stalls stand in rows close beside each other, as Kingdom
                    // Come's do; anything else keeps room to walk round.
                    Some(theirs) if other.kind == ThingKind::Stall => {
                        polygons(&ground, &theirs) >= 0.5
                    }
                    Some(theirs) => polygons(&ground, &theirs) >= 2.2,
                    None => true,
                });
            if fits {
                self.things.push(stall);
                placed += 1;
            }
        }
        // Benches near the square's edges, facing in.
        for index in 0..outline.len() {
            let (a, b) = (outline[index], outline[(index + 1) % outline.len()]);
            let edge = b - a;
            let inward = (middle - a).dot(edge.perp()).signum() * edge.perp().normalize();
            let at = a + edge * (0.3 + roll(30 + index as u32) * 0.2) + inward * 2.2;
            let bench = Thing::new(ThingKind::Bench, at, facing_of(inward), None);
            let clear = bench.outline().is_some_and(|ground| {
                self.things.iter().all(|other| match other.outline() {
                    Some(theirs) => polygons(&ground, &theirs) >= 1.8,
                    None => true,
                })
            });
            if clear && well_in(at, 1.5) {
                self.things.push(bench);
            }
        }
        // Two places to gather: the open ground farthest from anything on the square.
        let mut open: Vec<(f32, Vec2)> = Vec::new();
        for i in 0..24 {
            for j in 0..24 {
                let point = along * (low_a + (high_a - low_a) * (i as f32 + 0.5) / 24.0)
                    + across * (low_c + (high_c - low_c) * (j as f32 + 0.5) / 24.0);
                if !well_in(point, 3.5) {
                    continue;
                }
                // Out of the way of anyone coming onto the square from a street.
                let mouths = self
                    .streets
                    .iter()
                    .map(|street| street.points[0].distance(point) - 5.0)
                    .fold(f32::MAX, f32::min);
                let room = self
                    .things
                    .iter()
                    .map(|thing| match thing.outline() {
                        Some(ground) => segment_polygon(point, point, &ground),
                        None => thing.at.distance(point),
                    })
                    .fold(mouths, f32::min);
                open.push((room, point));
            }
        }
        open.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut gathered: Vec<Vec2> = Vec::new();
        for (room, point) in open {
            if room < 3.0 || gathered.len() == 2 {
                break;
            }
            if gathered.iter().all(|other| other.distance(point) > 9.0) {
                gathered.push(point);
                self.things.push(Thing::new(
                    ThingKind::Gathering,
                    point,
                    facing_of(along),
                    None,
                ));
            }
        }
    }

    /// The houses: plots cut along each street's sides and round the square, each building
    /// the next of the rules' in the seed's order that fits, its door to the way, until the
    /// town has its count. The first on the main street is the smithy: a woodshed open to
    /// the street with a forge before it.
    fn plots(&mut self, seed: u32, picks: impl Picks + Copy) {
        let want = BUILDINGS.0 + hash(&[seed, 12]) as usize % (BUILDINGS.1 - BUILDINGS.0 + 1);
        let mut queue: Vec<(Plan, Scheme, Roof, i32)> = village::every_building()
            .into_iter()
            // Houses, not sheds: a lean-to's one tall gable reads as a house half built
            // among a town's fronts (the town review, round 4).
            .filter(|(plan, _, roof, _)| {
                matches!(
                    plan,
                    Plan::Rectangle | Plan::L | Plan::T | Plan::U | Plan::TwoStoreys
                ) && *roof != Roof::LeanTo
            })
            .collect();
        for i in (1..queue.len()).rev() {
            let j = hash(&[seed, 13, i as u32]) as usize % (i + 1);
            queue.swap(i, j);
        }
        let mut fronts: Vec<Front> = Vec::new();
        let outline = self.square.outline.clone();
        let middle = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
        for index in 0..outline.len() {
            let (a, b) = (outline[index], outline[(index + 1) % outline.len()]);
            // Outward from the square: the side of the edge away from its middle.
            let side = -(middle - a).dot((b - a).perp()).signum();
            fronts.push(Front {
                points: vec![a, b],
                side,
                off: 0.0,
                at: SQUARE_CORNER_M,
                done: false,
                grand: true,
            });
        }
        for street in &self.streets {
            for side in [1.0, -1.0] {
                fronts.push(Front {
                    points: street.points.clone(),
                    side,
                    off: street.wide * 0.5,
                    at: STREET_FIRST_M,
                    done: false,
                    grand: false,
                });
            }
        }
        let mut next = 0;
        let mut smithy = false;
        let mut houses = 0;
        while houses < want && fronts.iter().any(|front| !front.done) {
            let square_done = fronts.iter().all(|front| !front.grand || front.done);
            for f in 0..fronts.len() {
                if houses >= want {
                    break;
                }
                // The square's edges are built up first, the streets' houses after.
                if fronts[f].done || (!fronts[f].grand && !square_done) {
                    continue;
                }
                let first_street = f == outline.len();
                let building_seed = hash(&[seed, 14, self.buildings.len() as u32]);
                let kinds = picks.pick(building_seed);
                if first_street && !smithy {
                    let choice = (Plan::Shed, Scheme::Timber, Roof::LeanTo, 6);
                    match self.try_plot(f, &mut fronts, building_seed, choice, kinds, true) {
                        Some(true) => smithy = true,
                        Some(false) => fronts[f].at += 2.0,
                        None => fronts[f].done = true,
                    }
                    continue;
                }
                let (first, length) = (next % queue.len(), queue.len());
                // The streets' first stretch out of the square, its busiest, is of houses of
                // two storeys where the queue has them, so it is closed in as a town's is.
                if !fronts[f].grand
                    && fronts[f].at < TALL_WITHIN_M
                    && unit(hash(&[building_seed, 16])) < TALL_SHARE
                    && let Some(look) = (0..length)
                        .find(|look| matches!(queue[(first + look) % length].0, Plan::TwoStoreys))
                {
                    queue.swap(first, (first + look) % length);
                }
                if !fronts[f].grand {
                    match self.try_plot(f, &mut fronts, building_seed, queue[first], kinds, false) {
                        Some(true) => {
                            next += 1;
                            houses += 1;
                        }
                        Some(false) => fronts[f].at += 2.0,
                        None => fronts[f].done = true,
                    }
                    continue;
                }
                // The square is closed in by house fronts, as a market square is: its edges
                // take the larger houses where the queue has them, and where the next will
                // not fit (by a street's mouth, at a corner, at an edge's end) a smaller one
                // that does, before the plot moves on a metre.
                let tall = unit(hash(&[building_seed, 16])) < TALL_SHARE;
                if let Some(look) = (0..length)
                    .find(|look| {
                        tall && matches!(queue[(first + look) % length].0, Plan::TwoStoreys)
                    })
                    .or_else(|| {
                        (0..6).find(|look| {
                            let (plan, _, _, wide) = queue[(first + look) % length];
                            matches!(plan, Plan::T) || wide >= 14
                        })
                    })
                {
                    queue.swap(first, (first + look) % length);
                }
                let mut room = false;
                let mut built = false;
                for look in 0..SQUARE_TRIES.min(length) {
                    let at = (first + look) % length;
                    // Fronts flat to the square: a wing brought forward either side of a door
                    // (an L, a U) makes a notch its crowd jams in.
                    if matches!(queue[at].0, Plan::L | Plan::U) {
                        continue;
                    }
                    match self.try_plot(f, &mut fronts, building_seed, queue[at], kinds, false) {
                        Some(true) => {
                            queue.swap(first, at);
                            built = true;
                            break;
                        }
                        Some(false) => room = true,
                        None => {}
                    }
                }
                if built {
                    next += 1;
                    houses += 1;
                } else if room {
                    fronts[f].at += 1.0;
                } else {
                    fronts[f].done = true;
                }
            }
        }
    }

    /// Tries the next plot along a front: the building put up, set where its own wings let
    /// its front stand back from the way, and kept if it is clear of everything. Nothing
    /// when the front has no room left for it, false when this place did not fit.
    fn try_plot(
        &mut self,
        f: usize,
        fronts: &mut [Front],
        seed: u32,
        choice: (Plan, Scheme, Roof, i32),
        kinds: Kinds,
        smithy: bool,
    ) -> Option<bool> {
        let front = &fronts[f];
        let length = line_length(&front.points);
        let probe = village::put_up(seed, choice, [0.0, 0.0], 0.0, kinds);
        let rects = ground_rects(&probe);
        if rects.is_empty() {
            return Some(false);
        }
        let low = rects.iter().map(|r| r.0).reduce(Vec2::min)?;
        let high = rects.iter().map(|r| r.1).reduce(Vec2::max)?;
        // Along the front, the building's local x runs against the line on its left side.
        let (from, to) = if front.side > 0.0 {
            (-high.x, -low.x)
        } else {
            (low.x, high.x)
        };
        let door_along = front.at - from;
        // The square's corners built close, its fronts running on to them; a street keeps
        // a little room at its end.
        let end = if front.grand { SQUARE_CORNER_M } else { 2.0 };
        if door_along + to > length - end {
            return None;
        }
        let (point, way) = along_line(&front.points, door_along)?;
        let outward = way.perp() * front.side;
        let back = if smithy { 2.6 } else { 0.0 };
        let off_way = if front.grand {
            SQUARE_OFF_M
        } else {
            OFF_STREET_M
        };
        let door = point + outward * (front.off + off_way + 0.2 + high.y.max(0.0) + back);
        let facing = facing_of(-outward);
        let building = Building {
            at: door.to_array(),
            facing,
            ..probe
        };
        let ground = footprint(&building, &rects);
        let gap = if hash(&[seed, 15]).is_multiple_of(PASSAGE_EVERY) {
            PASSAGE_M
        } else {
            APART_M
        };
        if !self.clear_for(&ground, APART_M) {
            return Some(false);
        }
        // Every door reached straight from the way, this one and those already built: with
        // the houses joined, a neighbour's corner on a bend can otherwise close one off.
        // Each door's way in, out from it and across before it, room for two abreast: a
        // door in a notch beside a neighbour's wing jammed those coming home.
        let way_in = |at: Vec2, facing: f32| {
            [
                (
                    place(at, facing, Vec2::new(0.0, 0.75)),
                    place(at, facing, Vec2::new(0.0, DOOR_WAY_M)),
                ),
                (
                    place(at, facing, Vec2::new(-DOOR_ABREAST_M, 0.9)),
                    place(at, facing, Vec2::new(DOOR_ABREAST_M, 0.9)),
                ),
            ]
        };
        let own = way_in(door, facing);
        let doors: Vec<(Vec2, Vec2)> = self
            .things
            .iter()
            .filter(|thing| thing.kind == ThingKind::Door)
            .flat_map(|thing| way_in(thing.at, thing.facing))
            .collect();
        if !self.footprints.iter().flatten().all(|quad| {
            own.iter()
                .all(|&(a, b)| segment_polygon(a, b, quad) >= CLEAR_M)
        }) || !ground.iter().all(|quad| {
            doors
                .iter()
                .all(|&(a, b)| segment_polygon(a, b, quad) >= CLEAR_M)
        }) {
            return Some(false);
        }
        fronts[f].at = door_along + to + gap;
        let index = self.buildings.len();
        self.buildings.push(building);
        self.footprints.push(ground);
        if smithy {
            self.things.push(Thing::new(
                ThingKind::Forge,
                place(door, facing, Vec2::new(0.0, 1.3)),
                facing,
                Some(index),
            ));
        } else {
            self.homes.push(index);
            self.things
                .push(Thing::new(ThingKind::Door, door, facing, Some(index)));
        }
        Some(true)
    }

    /// A yard behind each home that has room, and in it, by the seed, a goat pen of low
    /// walls, hens, a woodshed with a chopping block, or a chopping block alone.
    fn yards(&mut self, seed: u32, picks: impl Picks + Copy) {
        let homes = self.homes.clone();
        let mut made = 0u32;
        for home in homes {
            let rects = ground_rects(&self.buildings[home]);
            let (Some(low), Some(high)) = (
                rects.iter().map(|r| r.0).reduce(Vec2::min),
                rects.iter().map(|r| r.1).reduce(Vec2::max),
            ) else {
                continue;
            };
            let (at, facing) = (
                Vec2::from(self.buildings[home].at),
                self.buildings[home].facing,
            );
            let mut yard = None;
            for deep in [8.0, 6.0, 4.5] {
                let quad = turned_box(
                    at,
                    facing,
                    Vec2::new(low.x, low.y - BEHIND_M - deep),
                    Vec2::new(high.x, low.y - BEHIND_M),
                );
                let clear = self
                    .footprints
                    .iter()
                    .flatten()
                    .all(|other| polygons(&quad, other) >= 0.6)
                    && self
                        .yards
                        .iter()
                        .all(|(_, other)| polygons(&quad, other) >= 0.6)
                    && self.streets.iter().all(|street| {
                        from_street(&quad, street) >= street.wide * 0.5 + OFF_STREET_M
                    })
                    && polygons(&quad, &self.square.outline) >= 1.0
                    && self
                        .things
                        .iter()
                        .filter_map(Thing::outline)
                        .all(|thing| polygons(&quad, &thing) >= 1.0)
                    && self.lanes.iter().all(|(_, lane)| {
                        lane.windows(2)
                            .all(|pair| segment_polygon(pair[0], pair[1], &quad) >= 0.6)
                    });
                if clear {
                    yard = Some((quad, deep));
                    break;
                }
            }
            let Some((quad, deep)) = yard else {
                continue;
            };
            // A yard is only a yard if it can be walked to, along one side of the house.
            let Some(lane) = self.lane(home, low, high, &quad) else {
                continue;
            };
            self.lanes.push((home, lane));
            self.yards.push((home, quad));
            let back = low.y - BEHIND_M - deep;
            let middle = place(
                at,
                facing,
                Vec2::new((low.x + high.x) * 0.5, back + deep * 0.5),
            );
            let kinds = picks.pick(hash(&[seed, 16, home as u32]));
            // Turn by turn, so a town has some of each, then by the seed.
            let what = (made + hash(&[seed, 17]) % 4) % 4;
            made += 1;
            match what {
                0 if deep >= 6.0 && high.x - low.x >= 5.0 => {
                    self.pen(home, at, facing, low.x, high.x, back, deep, kinds);
                }
                1 | 0 => {
                    self.things
                        .push(Thing::new(ThingKind::Yard, middle, facing, Some(home)));
                }
                2 if deep >= 6.0 => {
                    // A woodshed in the yard's far corner, open toward the house, a chopping
                    // block before it.
                    let corner = place(at, facing, Vec2::new(high.x - 1.8, back + 2.2));
                    let shed = village::put_up(
                        hash(&[seed, 18, home as u32]),
                        (Plan::Shed, Scheme::Timber, Roof::LeanTo, 6),
                        corner.to_array(),
                        facing,
                        kinds,
                    );
                    let rects = ground_rects(&shed);
                    self.footprints.push(footprint(&shed, &rects));
                    self.buildings.push(shed);
                    self.things.push(Thing::new(
                        ThingKind::Block,
                        place(at, facing, Vec2::new(high.x - 1.8, back + 4.0)),
                        facing,
                        Some(home),
                    ));
                }
                _ => {
                    self.things.push(Thing::new(
                        ThingKind::Block,
                        place(
                            at,
                            facing,
                            Vec2::new((low.x + high.x) * 0.5, back + deep * 0.6),
                        ),
                        facing,
                        Some(home),
                    ));
                }
            }
        }
    }

    /// A goat pen of low walls filling a yard, a gate in its side toward the house.
    #[allow(clippy::too_many_arguments)]
    fn pen(
        &mut self,
        home: usize,
        at: Vec2,
        facing: f32,
        left: f32,
        right: f32,
        back: f32,
        deep: f32,
        kinds: Kinds,
    ) {
        let corner = |x: f32, y: f32| place(at, facing, Vec2::new(x, y));
        let (l, r) = (left + 0.3, right - 0.3);
        let (b, f) = (back + 0.3, back + deep - 0.3);
        let gate = (l + r) * 0.5;
        let lengths = [
            (corner(l, b), corner(r, b)),
            (corner(r, b), corner(r, f)),
            (corner(r, f), corner(gate + 0.8, f)),
            (corner(gate - 0.8, f), corner(l, f)),
            (corner(l, f), corner(l, b)),
        ];
        for (index, (from, to)) in lengths.into_iter().enumerate() {
            // Each length starts a cube on from the corner the last ended at.
            let way = to - from;
            let from = from + way.normalize_or_zero() * CELL_M;
            let way = to - from;
            let long = (way.length() / CELL_M).round() as i32;
            if long < 2 {
                continue;
            }
            // A low wall runs along its own x, its middle at its place; its face behind it.
            let wall = village::put_up(
                hash(&[home as u32, index as u32]),
                (Plan::Wall, Scheme::Masonry, Roof::Gable, long),
                ((from + to) * 0.5).to_array(),
                libm::atan2f(-way.y, way.x),
                kinds,
            );
            let rects = ground_rects(&wall);
            self.footprints.push(footprint(&wall, &rects));
            self.buildings.push(wall);
        }
        let middle = corner((l + r) * 0.5, (b + f) * 0.5);
        self.things
            .push(Thing::new(ThingKind::Pen, middle, facing, Some(home)));
    }

    /// At each street's far end, the gate out to the fields and a guard's post beside it.
    fn ends(&mut self) {
        let mut posts = Vec::new();
        for street in &self.streets {
            let count = street.points.len();
            if count < 3 {
                continue;
            }
            let (end, before) = (street.points[count - 1], street.points[count - 2]);
            let out = (end - before).normalize_or(Vec2::X);
            posts.push(Thing::new(ThingKind::Gate, end, facing_of(out), None));
            // The post on whichever side of the street is clear to stand on.
            for side in [1.0, -1.0] {
                let across = out.perp() * side;
                let post = before + across * (street.wide * 0.5 + 0.6);
                if self.standable(post, 1.0) {
                    posts.push(Thing::new(ThingKind::Post, post, facing_of(-across), None));
                    break;
                }
            }
        }
        self.things.extend(posts);
    }

    /// Whether a walk between two points keeps clear of everything built and every thing
    /// in the way.
    pub fn walkable(&self, a: Vec2, b: Vec2) -> bool {
        self.footprints
            .iter()
            .flatten()
            .all(|quad| segment_polygon(a, b, quad) >= CLEAR_M)
            && self
                .things
                .iter()
                .filter_map(Thing::outline)
                .all(|quad| segment_polygon(a, b, &quad) >= CLEAR_M)
            && self
                .fences
                .iter()
                .all(|fence| segments(a, b, fence.from, fence.to) >= CLEAR_M)
            && self
                .things
                .iter()
                .filter(|thing| thing.kind == ThingKind::Gathering)
                .all(|thing| point_segment(thing.at, a, b) >= GATHERED_M)
    }

    /// Joins a node that reaches none of the streets' or the square's nodes (in a narrow
    /// street, the walk slantwise to one passes too near the fronts; on the square, the
    /// market stands in the way) to the nearest street straight across from it, a node there
    /// on the centre line between that stretch's own two; or else a step in onto the square.
    fn straight_across(
        &self,
        nodes: &mut Vec<(Vec2, On)>,
        edges: &mut BTreeSet<[u32; 2]>,
        node: u32,
    ) {
        let at = nodes[node as usize].0;
        let mut best: Option<(f32, Vec2, u32, u32, usize)> = None;
        for (index, (a, b)) in nodes
            .iter()
            .enumerate()
            .zip(nodes.iter().enumerate().skip(1))
            .filter_map(|((i, (p, on)), (j, (q, next)))| match (on, next) {
                (On::Street(s), On::Street(t)) if s == t => Some((s, ((i, *p), (j, *q)))),
                _ => None,
            })
            .map(|(&s, pair)| (s, pair))
        {
            let ((i, p), (j, q)) = (a, b);
            let way = q - p;
            let t = ((at - p).dot(way) / way.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let foot = p + way * t;
            let d = foot.distance(at);
            if d < 8.0 && best.is_none_or(|b| d < b.0) && self.walkable(at, foot) {
                best = Some((d, foot, i as u32, j as u32, index));
            }
        }
        if let Some((_, foot, i, j, street)) = best {
            let new = nodes.len() as u32;
            nodes.push((foot, On::Street(street)));
            for (a, b) in [(new, i), (new, j), (new, node)] {
                if a != b {
                    edges.insert([a.min(b), a.max(b)]);
                }
            }
            return;
        }
        // On the square, a step in toward its middle, and on from there to the square's
        // nearest nodes it can walk to.
        let outline = &self.square.outline;
        let middle = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
        let step = at + (middle - at).normalize_or_zero() * 1.5;
        if !self.walkable(at, step) {
            return;
        }
        let mut near: Vec<(f32, u32)> = nodes
            .iter()
            .enumerate()
            .filter(|(_, (_, on))| *on == On::Square)
            .map(|(k, (p, _))| (p.distance(step), k as u32))
            .filter(|&(d, _)| d < 10.0)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let reached: Vec<u32> = near
            .into_iter()
            .filter(|&(_, k)| self.walkable(step, nodes[k as usize].0))
            .take(3)
            .map(|(_, k)| k)
            .collect();
        if reached.is_empty() {
            return;
        }
        let new = nodes.len() as u32;
        nodes.push((step, On::Square));
        edges.insert([node.min(new), node.max(new)]);
        for k in reached {
            edges.insert([k.min(new), k.max(new)]);
        }
    }

    /// Whether a point is clear to stand on.
    fn standable(&self, point: Vec2, margin: f32) -> bool {
        self.walkable(point, point)
            && self
                .things
                .iter()
                .filter(|thing| thing.kind == ThingKind::Gathering)
                .all(|thing| thing.at.distance(point) >= GATHERED_M + margin)
            && self
                .footprints
                .iter()
                .flatten()
                .all(|quad| segment_polygon(point, point, quad) >= margin)
    }

    /// The path graph: the streets' centre lines, a grid over the square, a node before
    /// every thing and lanes beside houses to their yards, joined where the walk between is
    /// clear, and every part joined to the rest.
    fn paths(&mut self) {
        let mut nodes: Vec<(Vec2, On)> = Vec::new();
        let mut edges: BTreeSet<[u32; 2]> = BTreeSet::new();
        let edge = |edges: &mut BTreeSet<[u32; 2]>, a: u32, b: u32| {
            if a != b {
                edges.insert([a.min(b), a.max(b)]);
            }
        };
        // The square's grid.
        let (along, across) = (self.square.along, self.square.across);
        let outline = self.square.outline.clone();
        let (low_a, high_a) = extent(&outline, along);
        let (low_c, high_c) = extent(&outline, across);
        let mut grid: Vec<((i32, i32), u32)> = Vec::new();
        let (columns, rows) = (
            ((high_a - low_a) / GRID_M) as i32,
            ((high_c - low_c) / GRID_M) as i32,
        );
        for i in 0..=columns {
            for j in 0..=rows {
                let point = along * (low_a + (high_a - low_a) * i as f32 / columns.max(1) as f32)
                    + across * (low_c + (high_c - low_c) * j as f32 / rows.max(1) as f32);
                let edge_room = (0..outline.len())
                    .map(|k| point_segment(point, outline[k], outline[(k + 1) % outline.len()]))
                    .fold(f32::MAX, f32::min);
                if !inside(point, &outline) || edge_room < 1.0 || !self.standable(point, 0.8) {
                    continue;
                }
                grid.push(((i, j), nodes.len() as u32));
                nodes.push((point, On::Square));
            }
        }
        for &((i, j), a) in &grid {
            for (di, dj) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                if let Some(&(_, b)) = grid.iter().find(|((x, y), _)| *x == i + di && *y == j + dj)
                    && self.walkable(nodes[a as usize].0, nodes[b as usize].0)
                {
                    edge(&mut edges, a, b);
                }
            }
        }
        // The streets.
        for (index, street) in self.streets.iter().enumerate() {
            let mut last: Option<u32> = None;
            for &point in &street.points {
                let node = nodes.len() as u32;
                nodes.push((point, On::Street(index)));
                if let Some(last) = last {
                    edge(&mut edges, last, node);
                }
                last = Some(node);
            }
        }
        let ways = nodes.len();
        // Joins a new node to the nearest of the streets' and the square's that it can walk
        // to, up to a few of them.
        let join = |town: &Town,
                    nodes: &[(Vec2, On)],
                    edges: &mut BTreeSet<[u32; 2]>,
                    node: u32,
                    most: usize| {
            let at = nodes[node as usize].0;
            let mut near: Vec<(f32, u32)> = (0..ways as u32)
                .map(|other| (nodes[other as usize].0.distance(at), other))
                .filter(|&(d, _)| d < 16.0)
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut joined = 0;
            for (_, other) in near {
                if joined == most {
                    break;
                }
                if town.walkable(at, nodes[other as usize].0) {
                    edge(edges, node, other);
                    joined += 1;
                }
            }
            joined
        };
        // Each street's first point to the square.
        for street in 0..self.streets.len() {
            if let Some(first) = nodes.iter().position(|(_, on)| *on == On::Street(street)) {
                join(self, &nodes, &mut edges, first as u32, 3);
            }
        }
        // Before each thing, where someone coming to it arrives; a gate at its street's end.
        let stands: Vec<Vec2> = self.things.iter().map(|thing| self.stand(thing)).collect();
        for (index, stand) in stands.into_iter().enumerate() {
            // A gate is its street's last node; a place to gather is come to by the square's
            // nearest, so no one walks through its middle to get round it.
            let at_end = match self.things[index].kind {
                ThingKind::Gate => nodes.iter().position(|(point, on)| {
                    matches!(on, On::Street(_)) && point.distance(stand) < 0.01
                }),
                ThingKind::Gathering => nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, on))| *on == On::Square)
                    .min_by(|a, b| a.1.0.distance(stand).total_cmp(&b.1.0.distance(stand)))
                    .map(|(node, _)| node),
                _ => None,
            };
            let node = match at_end {
                Some(node) => node as u32,
                None => {
                    let node = nodes.len() as u32;
                    nodes.push((stand, On::Thing(index)));
                    // A door between a building's wings first steps out past them.
                    let out = self.out_of(&self.things[index]);
                    match out {
                        Some(out) if self.walkable(stand, out) => {
                            let step = nodes.len() as u32;
                            nodes.push((out, On::Thing(index)));
                            edge(&mut edges, node, step);
                            if join(self, &nodes, &mut edges, step, 2) == 0 {
                                self.straight_across(&mut nodes, &mut edges, step);
                            }
                        }
                        _ => {
                            if join(self, &nodes, &mut edges, node, 2) == 0 {
                                self.straight_across(&mut nodes, &mut edges, node);
                            }
                        }
                    }
                    node
                }
            };
            self.things[index].node = node;
            // Its other places to stand, a stall's back.
            let thing = &self.things[index];
            for &extra in thing.kind.stands().iter().skip(1) {
                let point = thing.place(extra);
                let more = nodes.len() as u32;
                nodes.push((point, On::Thing(index)));
                join(self, &nodes, &mut edges, more, 2);
            }
        }
        // The lanes to the yards: on from the door's last step, to the yard's gate, and from
        // there to the yard's own things.
        for (home, lane) in self.lanes.clone() {
            let Some(door) = self
                .things
                .iter()
                .find(|t| t.kind == ThingKind::Door && t.building == Some(home))
                .map(|t| t.node)
            else {
                continue;
            };
            // The door's node, then its step out past its wings if it has one.
            let stepped = self
                .things
                .iter()
                .find(|t| t.kind == ThingKind::Door && t.building == Some(home))
                .and_then(|t| self.out_of(t));
            let skip = if stepped.is_some() && lane.get(1) == stepped.as_ref() {
                2
            } else {
                1
            };
            let mut last = if skip == 2 { door + 1 } else { door };
            let mut gate = last;
            for &point in &lane[skip..] {
                let node = nodes.len() as u32;
                nodes.push((point, On::Lane(home)));
                edge(&mut edges, last, node);
                last = node;
                gate = node;
            }
            for thing in &self.things {
                if thing.building == Some(home)
                    && thing.kind != ThingKind::Door
                    && self.walkable(nodes[gate as usize].0, nodes[thing.node as usize].0)
                {
                    edge(&mut edges, gate, thing.node);
                }
            }
        }
        self.paths = Paths {
            nodes,
            edges: edges.into_iter().collect(),
        };
        self.join_parts();
    }

    /// Joins any part of the path graph cut off from the largest to it, by the shortest
    /// clear walk between them.
    fn join_parts(&mut self) {
        loop {
            let parts = self.paths.parts();
            let mut sizes = std::collections::BTreeMap::<usize, usize>::new();
            for &part in &parts {
                *sizes.entry(part).or_default() += 1;
            }
            let Some((&largest, _)) = sizes
                .iter()
                .max_by_key(|(part, size)| (**size, usize::MAX - **part))
            else {
                return;
            };
            if sizes.len() == 1 {
                return;
            }
            let mut best: Option<(f32, u32, u32)> = None;
            for (a, &part_a) in parts.iter().enumerate() {
                if part_a == largest {
                    continue;
                }
                for (b, &part_b) in parts.iter().enumerate() {
                    if part_b != largest {
                        continue;
                    }
                    let (pa, pb) = (self.paths.nodes[a].0, self.paths.nodes[b].0);
                    let d = pa.distance(pb);
                    if d < 30.0
                        && best.is_none_or(|(least, _, _)| d < least)
                        && self.walkable(pa, pb)
                    {
                        best = Some((d, a as u32, b as u32));
                    }
                }
            }
            match best {
                Some((_, a, b)) => self.paths.edges.push([a.min(b), a.max(b)]),
                None => return,
            }
        }
    }

    /// The lane from before a home's door, out past its wings if it has any, along one of
    /// its sides to its yard's gate, if either side is clear of everything built and every
    /// other yard.
    fn lane(&self, home: usize, low: Vec2, high: Vec2, quad: &[Vec2; 4]) -> Option<Vec<Vec2>> {
        let building = &self.buildings[home];
        let (at, facing) = (Vec2::from(building.at), building.facing);
        let door = self
            .things
            .iter()
            .find(|t| t.kind == ThingKind::Door && t.building == Some(home))?;
        let stand = self.stand(door);
        let mut start = vec![stand];
        start.extend(self.out_of(door).filter(|&out| self.walkable(stand, out)));
        let gate = (quad[2] + quad[3]) * 0.5 + door.front() * 0.8;
        for x in [high.x + 0.8, low.x - 0.8, high.x + 1.2, low.x - 1.2] {
            let front = place(at, facing, Vec2::new(x, high.y.max(0.0) + 0.9));
            let side = place(at, facing, Vec2::new(x, low.y - 0.8));
            let mut chain = start.clone();
            chain.extend([front, side, gate]);
            let clear = chain.windows(2).all(|pair| {
                self.walkable(pair[0], pair[1])
                    && self
                        .yards
                        .iter()
                        .all(|(_, yard)| segment_polygon(pair[0], pair[1], yard) >= 0.3)
            });
            if clear {
                return Some(chain);
            }
        }
        None
    }

    /// Where someone leaving a door between its building's wings steps out to, past them;
    /// nothing for a door in a plain front.
    fn out_of(&self, thing: &Thing) -> Option<Vec2> {
        let building = &self.buildings[thing.building?];
        if thing.kind != ThingKind::Door {
            return None;
        }
        let forward = ground_rects(building)
            .iter()
            .map(|r| r.1.y)
            .fold(0.0f32, f32::max);
        (forward > 0.5).then(|| thing.place(Vec2::new(0.0, forward + 0.9)))
    }

    /// Where someone coming to a thing stands: before a door, a stall's counter, the well;
    /// at a pen's gate, between it and the house; on a yard, a place to gather, a post.
    fn stand(&self, thing: &Thing) -> Vec2 {
        match thing.kind {
            ThingKind::Door => [0.0, -0.5, 0.5, -1.0, 1.0, -1.5, 1.5]
                .into_iter()
                .map(|aside| thing.place(Vec2::new(aside, 1.0)))
                .find(|&point| self.standable(point, CLEAR_M + 0.05))
                .unwrap_or_else(|| thing.place(Vec2::new(0.0, 1.0))),
            ThingKind::Pen => match self.yards.iter().find(|y| Some(y.0) == thing.building) {
                Some((_, quad)) => (quad[2] + quad[3]) * 0.5 + thing.front() * 0.8,
                None => thing.at,
            },
            ThingKind::Yard | ThingKind::Gathering | ThingKind::Gate | ThingKind::Post => thing.at,
            _ => thing.place(thing.kind.stands()[0]),
        }
    }
}

impl Paths {
    /// Which connected part each node is in, as the lowest node in it.
    pub fn parts(&self) -> Vec<usize> {
        let mut part: Vec<usize> = (0..self.nodes.len()).collect();
        fn root(part: &mut [usize], mut node: usize) -> usize {
            while part[node] != node {
                part[node] = part[part[node]];
                node = part[node];
            }
            node
        }
        for &[a, b] in &self.edges {
            let (ra, rb) = (root(&mut part, a as usize), root(&mut part, b as usize));
            if ra != rb {
                part[ra.max(rb)] = ra.min(rb);
            }
        }
        (0..self.nodes.len())
            .map(|node| root(&mut part, node))
            .collect()
    }
}

/// Something to mark on a plan: a place, a colour, how big, and a word by it.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub at: Vec2,
    pub colour: &'static str,
    pub size: f32,
    pub label: Option<String>,
}

/// A town's plan as an SVG to look at (`cargo xtask town`, and the life check's map): the
/// ground's height in metre bands, the square, the streets, every building's ground, the
/// yards, the things a day is done at, which way each looks, the path graph with anything cut
/// off from it in red, a legend, and marks laid over it all.
pub fn svg(town: &Town, heights: &dyn Fn(Vec2) -> f32, marks: &[Mark]) -> String {
    let mut points: Vec<Vec2> = town.paths.nodes.iter().map(|n| n.0).collect();
    points.extend(town.footprints.iter().flatten().flatten());
    let low = points
        .iter()
        .copied()
        .reduce(Vec2::min)
        .unwrap_or(Vec2::ZERO)
        - 12.0;
    let high = points
        .iter()
        .copied()
        .reduce(Vec2::max)
        .unwrap_or(Vec2::ZERO)
        + 12.0;
    let size = high - low;
    let scale = 4.0;
    let p = |v: Vec2| {
        let s = (v - low) * scale;
        format!("{:.1},{:.1}", s.x, s.y)
    };
    let poly = |quad: &[Vec2], style: &str| {
        let pts: Vec<String> = quad.iter().map(|&v| p(v)).collect();
        format!("<polygon points=\"{}\" {style}/>\n", pts.join(" "))
    };
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.0}\" height=\"{:.0}\" font-family=\"sans-serif\">\n<rect width=\"100%\" height=\"100%\" fill=\"#e9e4d4\"/>\n",
        size.x * scale,
        size.y * scale + 70.0
    );
    // Contours every metre, as dots, so the slope reads.
    let mut y = low.y;
    while y < high.y {
        let mut x = low.x;
        while x < high.x {
            let h = heights(Vec2::new(x, y));
            let shade = (h.rem_euclid(2.0) < 1.0) as u8;
            if shade == 1 {
                svg.push_str(&format!(
                    "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" fill=\"#dcd5bf\"/>\n",
                    (x - low.x) * scale,
                    (y - low.y) * scale,
                    2.0 * scale,
                    2.0 * scale
                ));
            }
            x += 2.0;
        }
        y += 2.0;
    }
    svg.push_str(&poly(
        &town.square.outline,
        "fill=\"#cdbf9c\" stroke=\"#8a7a55\"",
    ));
    for street in &town.streets {
        let pts: Vec<String> = street.points.iter().map(|&v| p(v)).collect();
        svg.push_str(&format!(
            "<polyline points=\"{}\" fill=\"none\" stroke=\"#cdbf9c\" stroke-width=\"{:.0}\" stroke-linejoin=\"round\"/>\n",
            pts.join(" "),
            street.wide * scale
        ));
    }
    for (_, yard) in &town.yards {
        svg.push_str(&poly(
            yard,
            "fill=\"#b9c99a\" stroke=\"#7d8f5c\" stroke-dasharray=\"3 2\"",
        ));
    }
    for (index, quads) in town.footprints.iter().enumerate() {
        let home = town.homes.contains(&index);
        let fill = if home { "#8c6b4f" } else { "#6f6a63" };
        for quad in quads {
            svg.push_str(&poly(quad, &format!("fill=\"{fill}\" stroke=\"#3b2f25\"")));
        }
    }
    for &[a, b] in &town.paths.edges {
        let (pa, pb) = (
            town.paths.nodes[a as usize].0,
            town.paths.nodes[b as usize].0,
        );
        svg.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#2f6db5\" stroke-width=\"0.8\"/>\n",
            ((pa - low) * scale).x,
            ((pa - low) * scale).y,
            ((pb - low) * scale).x,
            ((pb - low) * scale).y
        ));
    }
    let parts = town.paths.parts();
    for (index, (at, on)) in town.paths.nodes.iter().enumerate() {
        let colour = if parts[index] != parts[0] {
            "#e0201b"
        } else {
            match on {
                On::Street(_) => "#2f6db5",
                On::Square => "#2f6db5",
                On::Thing(_) => "#111",
                On::Lane(_) => "#5a8f2c",
            }
        };
        let s = (*at - low) * scale;
        svg.push_str(&format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"1.6\" fill=\"{colour}\"/>\n",
            s.x, s.y
        ));
    }
    for thing in &town.things {
        let colour = match thing.kind {
            ThingKind::Door => "#f2c14e",
            ThingKind::Well => "#3a8fd9",
            ThingKind::Stall => "#d9573a",
            ThingKind::Forge => "#1c1c1c",
            ThingKind::Block => "#a0522d",
            ThingKind::Bench => "#7a5c3a",
            ThingKind::Trough => "#3a8fd9",
            ThingKind::Post => "#7b2fbe",
            ThingKind::Gathering => "#e08bd1",
            ThingKind::Gate => "#7b2fbe",
            ThingKind::Pen => "#d6c7a1",
            ThingKind::Yard => "#e3d26a",
            ThingKind::Barrel => "#6b4a2b",
            ThingKind::Cart => "#9c7a4a",
            ThingKind::Goods => "#c9a227",
            ThingKind::Site => "#ffffff",
        };
        match thing.outline() {
            Some(quad) => svg.push_str(&poly(&quad, &format!("fill=\"{colour}\" stroke=\"#222\""))),
            None => {
                let s = (thing.at - low) * scale;
                svg.push_str(&format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" fill=\"{colour}\" stroke=\"#222\"/>\n",
                    s.x, s.y
                ));
            }
        }
        // Which way it looks.
        let (from, to) = (
            (thing.at - low) * scale,
            (thing.at + thing.front() * 1.5 - low) * scale,
        );
        svg.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#222\" stroke-width=\"1\"/>\n",
            from.x, from.y, to.x, to.y
        ));
    }
    for mark in marks {
        let s = (mark.at - low) * scale;
        svg.push_str(&format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"{}\" stroke=\"#111\" stroke-width=\"0.6\"/>\n",
            s.x, s.y, mark.size, mark.colour
        ));
        if let Some(label) = &mark.label {
            svg.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"9\">{label}</text>\n",
                s.x + mark.size + 1.0,
                s.y + 3.0
            ));
        }
    }
    let legend = [
        ("#8c6b4f", "home"),
        ("#6f6a63", "shed, wall"),
        ("#b9c99a", "yard"),
        ("#f2c14e", "door"),
        ("#d9573a", "stall"),
        ("#3a8fd9", "well, trough"),
        ("#1c1c1c", "forge"),
        ("#a0522d", "block"),
        ("#7a5c3a", "bench"),
        ("#e08bd1", "gathering"),
        ("#7b2fbe", "gate, post"),
        ("#e3d26a", "hens"),
        ("#d6c7a1", "pen"),
        ("#2f6db5", "paths"),
        ("#e0201b", "cut off"),
    ];
    let base = size.y * scale + 10.0;
    for (index, (colour, name)) in legend.iter().enumerate() {
        let x = 10.0 + (index % 8) as f32 * 110.0;
        let y = base + (index / 8) as f32 * 24.0;
        svg.push_str(&format!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"14\" height=\"14\" fill=\"{colour}\" stroke=\"#222\"/><text x=\"{}\" y=\"{}\" font-size=\"13\">{name}</text>\n",
            x + 20.0,
            y + 12.0
        ));
    }
    svg.push_str(&format!(
        "<text x=\"10\" y=\"{}\" font-size=\"12\">North is up; each ground band is a metre of height. 10 m = {:.0} px.</text>\n",
        base + 56.0,
        10.0 * scale
    ));
    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests;
