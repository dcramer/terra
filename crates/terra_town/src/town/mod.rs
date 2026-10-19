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

use crate::village::{self, Building, Kinds, Picks, Plan, Roof, Scheme};
use glam::Vec2;
use std::collections::BTreeSet;
use terra_build::{Base, CELL_M, STOREY_CUBES};
use terra_gen::hash;

mod enclosure;
mod furnish;
mod geometry;
mod market;
mod paths;
mod plots;
mod streets;
mod svg;
mod walls;
pub use enclosure::{Enclosure, enclosure};
pub use furnish::{FURNISHING, Fence, FenceKind, Furnishing};
use geometry::{along_line, from_street, line_length, turned_box};
pub use geometry::{
    contour, facing_of, inside, looking, place, point_segment, polygons, segment_polygon, segments,
    slope,
};
pub use market::{Counter, Good, Market, Trade};
use streets::{extent, square, streets};
pub use svg::{Mark, svg};

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
    /// Where its works begin among its buildings: its wall's lengths, towers and gates, and
    /// the castle last of all; as many as its buildings if it is open.
    pub works: usize,
    /// The line of its wall's outer face, in order round it: round every house, yard and the
    /// square and round the castle. Empty if it is open.
    pub wall: Vec<Vec2>,
}

/// How big a town is laid (D381): how many buildings, at least and at most, sheds and walls
/// not counted; how much longer than a market town's its streets run; and whether a wall with
/// towers and gates closes it round, a castle on the highest ground beside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub buildings: (usize, usize),
    pub streets: f32,
    pub walled: bool,
}

impl Size {
    /// A market town of thirty to forty houses along streets of 80 to 105 metres, open.
    pub const MARKET: Size = Size {
        buildings: BUILDINGS,
        streets: 1.0,
        walled: false,
    };
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
    town_sized(seed, picks, heights, &Size::MARKET)
}

/// A town of a size (D381): as [`town`], its buildings as many and its streets as long as
/// `size` says, and walled with a castle over it if it asks.
pub fn town_sized(seed: u32, picks: impl Picks + Copy, heights: Heights, size: &Size) -> Town {
    let (along, uphill) = contour(heights, Vec2::ZERO, 8.0);
    let shape = match hash::of(&[seed, 1]) % 3 {
        0 => SquareShape::Long,
        1 => SquareShape::Triangle,
        _ => SquareShape::Widened,
    };
    let square = square(seed, shape, along, uphill);
    let streets = streets(seed, &square, heights, size.streets);
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
        works: 0,
        wall: Vec::new(),
    };
    town.on_the_square(seed);
    town.plots(seed, picks, size.buildings);
    town.yards(seed, picks);
    town.furnish(seed);
    town.ends();
    town.works = town.buildings.len();
    if size.walled {
        town.walls(seed, picks, heights);
    }
    town.paths();
    town.unfurnish_cut_off();
    town
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
}

#[cfg(test)]
mod tests;
