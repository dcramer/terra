//! A village's buildings from its seed (D108, D182; docs/building.md, one system for
//! everything built): every house, shed, deck and garden wall put up from the same parts
//! and shortcuts a player's base is, by rules, so a settlement is a list of operations and
//! a place for each, and nothing about it is a prefab.
//!
//! **What varies.** A building is a plan (a rectangle, an L, a T, a U, two storeys with an
//! outside stair, a shed, a deck on posts, a length of low wall), built in a scheme of the
//! kits' kinds (all logs; all stone; hewn timber; logs on a stone plinth a metre high; a
//! stone ground floor with timber over it; log walls with stone gable ends; cob; wattle and daub;
//! a stone ground floor under wattle and daub), under a roof of cubes (a gable, a hip or a
//! lean-to, D241) of the roof kind its walls take, at a size, with its windows where its
//! seed puts them, and a lean of its own. Eight plans, nine schemes, three roofs and three
//! widths, less what does not go together, is about three hundred buildings a player can tell apart
//! ([`variants`]); a village picks a dozen or so, each scheme and plan at least once.
//!
//! **How it is laid out.** As a street village: a street running east and west through
//! the middle, a green on it where the well stands, and the houses along both sides with
//! their doors to the street, a few metres apart; a shed beside a house, garden walls of
//! stone behind the southern row, and a lookout deck on posts at the east end, its stair
//! toward the street. Each building's place and the way its door looks are given in metres
//! east and south of the green's middle, as the game sets it on the ground; a building is
//! laid out in its own cubes with its door's middle at its origin and its door looking
//! south (+Z), so the game turns and stands it where it goes.

use crate::layout::{hash, unit};
use crate::roofer::{self, Rectangle, Style};
use crate::{Base, BuildOp, Cube, Facing, Fill, Kind, Panel, STOREY_CUBES, Shape};
use glam::Vec2;
use std::collections::BTreeSet;

/// The kinds a village is built of: its kits', and the roofs over each kind of wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kinds {
    pub logs: Kind,
    pub stone: Kind,
    pub hewn: Kind,
    /// Cob earth, and wattle and daub in a timber frame (D227, D236).
    pub earth: Kind,
    pub wattle: Kind,
    /// Shakes over logs, slate over stone, thatch over hewn timber.
    pub shakes: Kind,
    pub slate: Kind,
    pub thatch: Kind,
}

/// One of a village's buildings: what it is, where it stands and which way it looks, and
/// the shortcuts that put it up in its own cubes.
#[derive(Clone, Debug, PartialEq)]
pub struct Building {
    /// What it is, as a person would say: "a log cabin", "a stone house under a hip roof".
    pub name: String,
    /// Its plan and scheme, which tell it apart from the rest.
    pub plan: Plan,
    pub scheme: Scheme,
    /// Where its door's middle stands, in metres east and south of the green's middle.
    pub at: [f32; 2],
    /// The way its door looks, in radians from south (+Z) toward east (+X).
    pub facing: f32,
    /// The shortcuts that put it up, in its own cubes: its door's middle at the origin, on
    /// the outer face of its front wall, looking south (+Z).
    pub ops: Vec<BuildOp>,
}

/// A building's plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Plan {
    Rectangle,
    L,
    T,
    U,
    /// Two storeys, with a stair up the outside of a gable end to a door on the upper one.
    TwoStoreys,
    /// A woodshed, open at the front under a lean-to.
    Shed,
    /// A lookout deck on posts, reached by a stair.
    Deck,
    /// A length of low wall, a metre high.
    Wall,
}

/// What a building's walls are, of the kits' kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scheme {
    Logs,
    Stone,
    Hewn,
    /// Log walls on a stone plinth a metre high.
    LogsOnPlinth,
    /// A stone ground floor with hewn timber over it.
    StoneUnderTimber,
    /// Log walls, their gable ends of stone.
    StoneEnds,
    /// Cob earth on its plinth (D236).
    Earth,
    /// Wattle and daub in an oak frame (D236).
    Wattle,
    /// A stone ground floor with wattle and daub over it.
    StoneUnderWattle,
}

/// A roof over a building.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Roof {
    Gable,
    Hip,
    LeanTo,
}

/// The widths a house's front may be, in cubes.
pub const WIDTHS: [i32; 3] = [12, 14, 18];

/// How many distinct buildings the rules make (D182): every plan in every scheme it goes
/// with, under every roof it takes, at every width; windows placed by the seed not counted.
pub fn variants() -> usize {
    let sizes = WIDTHS.len();
    // A rectangle in any scheme under a gable, a hip or a lean-to.
    let rectangles = 9 * 3 * sizes;
    // An L, a T or a U in any scheme under gables or hips.
    let wings = 3 * 9 * 2 * sizes;
    // Two storeys: stone under timber or under wattle, or all logs, stone, hewn, earth or
    // wattle, under a gable or a hip.
    let two = 7 * 2 * sizes;
    // Sheds and decks in either timber, low walls in stone, each at a few lengths.
    let small = 2 * 2 + 2 * 2 + 3;
    rectangles + wings + two + small
}

/// A building's own cubes, laid out with the door's middle at the origin, looking south.
struct Draft {
    ops: Vec<BuildOp>,
    seed: u32,
}

impl Draft {
    fn new(seed: u32) -> Self {
        Draft {
            ops: Vec::new(),
            seed,
        }
    }

    /// A number from 0 up to 1 for the `n`th choice.
    fn roll(&self, n: u32) -> f32 {
        unit(hash(&[self.seed, n]))
    }

    fn room(&mut self, min: [i32; 3], max: [i32; 3], kind: Kind) {
        self.ops.push(BuildOp::AddRoom { min, max, kind });
    }

    fn wall(&mut self, tile: [i32; 3], panel: Panel, kind: Kind) {
        self.ops.push(BuildOp::SetWall {
            tile,
            panel: Some(panel),
            kind,
        });
    }

    fn fill(&mut self, min: [i32; 3], max: [i32; 3], cube: Cube) {
        self.ops.push(BuildOp::Fill {
            min,
            max,
            cube: Some(cube),
        });
    }

    /// A roof of cubes over a footprint on a storey, in a kind, its height over each point
    /// `height` gives (`roofer`), in the place of the flat roof the rooms laid there, the
    /// walls under its gable ends carried up to it in their own kind.
    fn roof(
        &mut self,
        footprint: &BTreeSet<[i32; 2]>,
        storey: i32,
        kind: Kind,
        height: &dyn Fn(Vec2) -> f32,
    ) {
        let walls = Base::from_shortcuts(&self.ops);
        let ops = roofer::put_up(&walls, footprint, storey, kind, height);
        self.ops.extend(ops);
    }

    /// The doorway in the front wall, a metre wide, at the origin.
    fn door(&mut self, storey: i32, kind: Kind) {
        for x in [-1, 0] {
            self.wall([x, storey, -1], Panel::Door, kind);
        }
    }

    /// A window a metre wide in a wall, its tiles from `a` along a side.
    fn window(&mut self, tiles: [[i32; 3]; 2], kind: Kind) {
        for tile in tiles {
            self.wall(tile, Panel::WINDOW, kind);
        }
    }
}

/// A rectangle's walls, from x0 to x1 and z0 to z1 inclusive: the tiles along each side,
/// north, south, west and east.
fn sides(x0: i32, x1: i32, z0: i32, z1: i32) -> [Vec<[i32; 2]>; 4] {
    [
        (x0..=x1).map(|x| [x, z0]).collect(),
        (x0..=x1).map(|x| [x, z1]).collect(),
        (z0..=z1).map(|z| [x0, z]).collect(),
        (z0..=z1).map(|z| [x1, z]).collect(),
    ]
}

/// A rectangular block of a building: from x0 to x1 and z0 to z1, inclusive, in tiles.
#[derive(Clone, Copy, Debug)]
struct Block {
    x0: i32,
    x1: i32,
    z0: i32,
    z1: i32,
}

impl Block {
    fn min(&self, storey: i32) -> [i32; 3] {
        [self.x0, storey, self.z0]
    }

    fn max(&self, storey: i32) -> [i32; 3] {
        [self.x1, storey, self.z1]
    }

    fn wide(&self) -> i32 {
        self.x1 - self.x0 + 1
    }

    fn columns(&self) -> impl Iterator<Item = [i32; 2]> + '_ {
        (self.x0..=self.x1).flat_map(move |x| (self.z0..=self.z1).map(move |z| [x, z]))
    }

    fn deep(&self) -> i32 {
        self.z1 - self.z0 + 1
    }
}

/// The kinds a scheme's walls are: on the ground storey and over it.
fn walls_of(scheme: Scheme, kinds: Kinds) -> (Kind, Kind) {
    match scheme {
        Scheme::Logs | Scheme::LogsOnPlinth | Scheme::StoneEnds => (kinds.logs, kinds.logs),
        Scheme::Stone => (kinds.stone, kinds.stone),
        Scheme::Hewn => (kinds.hewn, kinds.hewn),
        Scheme::StoneUnderTimber => (kinds.stone, kinds.hewn),
        Scheme::Earth => (kinds.earth, kinds.earth),
        Scheme::Wattle => (kinds.wattle, kinds.wattle),
        Scheme::StoneUnderWattle => (kinds.stone, kinds.wattle),
    }
}

/// The kind a scheme's floors, stairs and landings are of: its upper walls', but timber
/// where those are framed, whose infill makes no floor.
fn decks_of(scheme: Scheme, kinds: Kinds) -> Kind {
    match scheme {
        Scheme::Wattle | Scheme::StoneUnderWattle => kinds.hewn,
        _ => walls_of(scheme, kinds).1,
    }
}

/// A house of one or more blocks on the ground storey, each its own room, joined where
/// they meet, its door in the first block's front wall at the origin, windows where its
/// seed puts them, and its scheme's stone where it has any. `doors` are the tiles of the
/// front wall a window may not take.
fn house(draft: &mut Draft, blocks: &[Block], scheme: Scheme, kinds: Kinds) {
    let (kind, _) = walls_of(scheme, kinds);
    for block in blocks {
        draft.room(block.min(0), block.max(0), kind);
    }
    draft.door(0, kind);
    // Windows: one either side of the door where the front is wide enough, and one in each
    // side wall of each block, at a place along it its seed chooses; only in a wall open to
    // the outside, never between two blocks.
    let outside = |x: i32, z: i32| {
        !blocks
            .iter()
            .any(|b| b.x0 < x && x < b.x1 && b.z0 < z && z < b.z1)
    };
    let front = blocks[0];
    let mut n = 10;
    if front.wide() >= 12 {
        let off = 3 + (draft.roll(n) * 2.0) as i32;
        for tiles in [
            [[-1 - off, 0, -1], [-off, 0, -1]],
            [[off - 1, 0, -1], [off, 0, -1]],
        ] {
            if tiles.iter().all(|t| outside(t[0], t[2])) {
                draft.window(tiles, kind);
            }
        }
    }
    for block in blocks {
        n += 1;
        if block.deep() >= 6 {
            let along = block.z0 + 1 + (draft.roll(n) * (block.deep() - 4) as f32) as i32;
            for x in [block.x0, block.x1] {
                if outside(x, along) && outside(x, along + 1) {
                    draft.window([[x, 0, along], [x, 0, along + 1]], kind);
                }
            }
        }
    }
    let stone = Cube::new(kinds.stone, Fill::Solid);
    match scheme {
        // A metre of stone at the foot of every wall, its doorway's left open, laid after
        // the windows, which set their tile's whole column.
        Scheme::LogsOnPlinth => {
            for block in blocks {
                for side in sides(block.x0, block.x1, block.z0, block.z1) {
                    for [x, z] in side {
                        if z == -1 && (x == -1 || x == 0) {
                            continue;
                        }
                        draft.fill([x, 0, z], [x, 1, z], stone);
                    }
                }
            }
        }
        // Both ends of the first block, under its gables, of stone to the top.
        Scheme::StoneEnds => {
            for x in [front.x0, front.x1] {
                draft.fill([x, 0, front.z0], [x, STOREY_CUBES - 1, front.z1], stone);
            }
        }
        _ => {}
    }
}

/// The roof kind over a scheme's walls: shakes over logs, slate over stone, thatch over
/// hewn timber, cob and wattle and daub; over stone under timber, the timber's.
fn roof_kind(scheme: Scheme, kinds: Kinds) -> Kind {
    match scheme {
        Scheme::Logs | Scheme::LogsOnPlinth | Scheme::StoneEnds => kinds.shakes,
        Scheme::Stone => kinds.slate,
        Scheme::Hewn
        | Scheme::StoneUnderTimber
        | Scheme::Earth
        | Scheme::Wattle
        | Scheme::StoneUnderWattle => kinds.thatch,
    }
}

/// A roof of cubes over a house's blocks (D241), as a builder would choose for its plan: a
/// rectangle under a gable, a hip or a lean-to; an L, a T or a U under gables over each
/// block, a wing's ridge dying into the main block's so no wing stands up as a tall
/// gable, or hipped all round.
fn roof(draft: &mut Draft, blocks: &[Block], roof: Roof, storey: i32, kind: Kind) {
    let main = blocks[0];
    let footprint: BTreeSet<[i32; 2]> = blocks.iter().flat_map(Block::columns).collect();
    let rectangle = |x0: i32, x1: i32, z0: i32, z1: i32| Rectangle {
        min: [x0, z0],
        max: [x1, z1],
    };
    match roof {
        Roof::LeanTo => {
            let height = roofer::field(&footprint, Style::LeanTo, Some(true));
            draft.roof(&footprint, storey, kind, &height);
        }
        Roof::Hip => {
            let height = roofer::field(&footprint, Style::Hip, None);
            draft.roof(&footprint, storey, kind, &height);
        }
        Roof::Gable => {
            // The main block's ridge along its front; each wing's across it, reaching in to
            // the main ridge.
            let middle = (main.z0 + main.z1 + 1) / 2;
            let mut parts = vec![(
                rectangle(main.x0, main.x1, main.z0, main.z1),
                Style::Gable,
                true,
            )];
            for wing in &blocks[1..] {
                let (z0, z1) = if wing.z0 >= main.z0 {
                    (middle, wing.z1)
                } else {
                    (wing.z0, middle - 1)
                };
                parts.push((rectangle(wing.x0, wing.x1, z0, z1), Style::Gable, false));
            }
            let (_, height) = roofer::sections(&parts);
            draft.roof(&footprint, storey, kind, &height);
        }
    }
}

/// A one-storey house of a plan, a scheme and a roof, `wide` tiles across its front.
fn one_storey(
    seed: u32,
    plan: Plan,
    scheme: Scheme,
    roofed: Roof,
    wide: i32,
    kinds: Kinds,
) -> Vec<BuildOp> {
    let mut draft = Draft::new(seed);
    let half = wide / 2;
    let deep = 10;
    let main = Block {
        x0: -half,
        x1: half - 1,
        z0: -deep,
        z1: -1,
    };
    let wing = 8;
    let blocks: Vec<Block> = match plan {
        // A wing forward to the street on the east, its gable end to it.
        Plan::L => vec![
            main,
            Block {
                x0: half - 6,
                x1: half - 1,
                z0: -deep,
                z1: 7,
            },
        ],
        // A wing behind, in the middle.
        Plan::T => vec![
            main,
            Block {
                x0: -wing / 2,
                x1: wing / 2 - 1,
                z0: -deep - 8,
                z1: -1,
            },
        ],
        // Wings forward either side of a yard before the door.
        Plan::U => vec![
            main,
            Block {
                x0: -half,
                x1: -half + 5,
                z0: -deep,
                z1: 7,
            },
            Block {
                x0: half - 6,
                x1: half - 1,
                z0: -deep,
                z1: 7,
            },
        ],
        _ => vec![main],
    };
    house(&mut draft, &blocks, scheme, kinds);
    roof(&mut draft, &blocks, roofed, 0, roof_kind(scheme, kinds));
    draft.ops
}

/// Two storeys: the ground one in the scheme's lower kind, the upper in its upper, a floor
/// of timber between; a stair up the outside of the east gable end, two cubes wide, to a
/// landing on posts before a door in the upper storey.
fn two_storeys(seed: u32, scheme: Scheme, roofed: Roof, kinds: Kinds) -> Vec<BuildOp> {
    let mut draft = Draft::new(seed);
    let (low, high) = walls_of(scheme, kinds);
    let deck = decks_of(scheme, kinds);
    let main = Block {
        x0: -6,
        x1: 5,
        z0: -10,
        z1: -1,
    };
    house(&mut draft, &[main], scheme, kinds);
    draft.room(main.min(1), main.max(1), high);
    // The upper floor of timber, inside the walls.
    draft.ops.push(BuildOp::SetFloor {
        min: [main.x0 + 1, 1, main.z0 + 1],
        max: [main.x1 - 1, 1, main.z1 - 1],
        laid: true,
        kind: deck,
    });
    // Windows upstairs, front and back.
    for z in [main.z0, main.z1] {
        draft.window([[-4, 1, z], [-3, 1, z]], high);
        draft.window([[2, 1, z], [3, 1, z]], high);
    }
    // The stair, from the front corner up toward the back, beside the east wall.
    let stair = Cube {
        shape: Shape::Stair(Facing::North),
        ..Cube::new(deck, Fill::Solid)
    };
    let whole = Cube::new(deck, Fill::Solid);
    let (x0, x1) = (main.x1 + 1, main.x1 + 2);
    for step in 0..STOREY_CUBES {
        let z = -2 - step;
        if step > 0 {
            draft.fill([x0, 0, z], [x1, step - 1, z], whole);
        }
        draft.fill([x0, step, z], [x1, step, z], stair);
    }
    // The landing at the top, on two posts at its outer corners, and the door off it.
    let top = STOREY_CUBES - 1;
    let landing = [-2 - STOREY_CUBES, -3 - STOREY_CUBES];
    draft.fill([x0, top, landing[1]], [x1, top, landing[0]], whole);
    let post = Cube {
        shape: Shape::Post,
        ..whole
    };
    draft.fill([x1, 0, landing[1]], [x1, top - 1, landing[1]], post);
    for z in landing {
        draft.wall([main.x1, 1, z], Panel::Door, high);
    }
    let _ = low;
    roof(&mut draft, &[main], roofed, 1, roof_kind(scheme, kinds));
    draft.ops
}

/// A woodshed of logs, three metres by two, open at the front under a lean-to rising over
/// it.
fn shed(seed: u32, kinds: Kinds) -> Vec<BuildOp> {
    let mut draft = Draft::new(seed);
    let block = Block {
        x0: -3,
        x1: 2,
        z0: -4,
        z1: -1,
    };
    draft.room(block.min(0), block.max(0), kinds.logs);
    for x in -2..=1 {
        draft.wall([x, 0, -1], Panel::Door, kinds.logs);
    }
    roof(&mut draft, &[block], Roof::LeanTo, 0, kinds.shakes);
    draft.ops
}

/// A lookout deck of hewn timber, three metres by two, a storey up on four posts, its
/// stair coming down toward the front: the door's place is the stair's foot.
fn deck(kinds: Kinds) -> Vec<BuildOp> {
    let mut draft = Draft::new(0);
    let kind = kinds.hewn;
    let whole = Cube::new(kind, Fill::Solid);
    let post = Cube {
        shape: Shape::Post,
        ..whole
    };
    let top = STOREY_CUBES - 1;
    // The deck from x -3 to 2 and z -10 to -7; the stair, two wide, from z -1 up north.
    let (x0, x1, z0, z1) = (-3, 2, -10, -7);
    for [x, z] in [[x0, z0], [x1, z0], [x0, z1], [x1, z1]] {
        draft.fill([x, 0, z], [x, top - 1, z], post);
    }
    draft.fill([x0, top, z0], [x1, top, z1], whole);
    let stair = Cube {
        shape: Shape::Stair(Facing::North),
        ..whole
    };
    for step in 0..STOREY_CUBES {
        let z = -1 - step;
        if step > 0 {
            draft.fill([-1, 0, z], [0, step - 1, z], whole);
        }
        draft.fill([-1, step, z], [0, step, z], stair);
    }
    // A parapet a cube high round its open sides, but over the stair.
    let rail = Cube::new(kind, Fill::Solid);
    draft.fill([x0, top + 1, z0], [x1, top + 1, z0], rail);
    draft.fill([x0, top + 1, z0], [x0, top + 1, z1], rail);
    draft.fill([x1, top + 1, z0], [x1, top + 1, z1], rail);
    draft.fill([x0, top + 1, z1], [-2, top + 1, z1], rail);
    draft.fill([1, top + 1, z1], [x1, top + 1, z1], rail);
    draft.ops
}

/// A low wall of stone a metre high, `long` cubes from the origin west to east.
fn low_wall(long: i32, kinds: Kinds) -> Vec<BuildOp> {
    let mut draft = Draft::new(0);
    let half = long / 2;
    draft.fill(
        [-half, 0, -1],
        [long - half - 1, 1, -1],
        Cube::new(kinds.stone, Fill::Solid),
    );
    draft.ops
}

/// What a person would call a building of a plan, a scheme and a roof.
fn name(plan: Plan, scheme: Scheme, roofed: Roof) -> String {
    let what = match scheme {
        Scheme::Logs => "log",
        Scheme::Stone => "stone",
        Scheme::Hewn => "hewn timber",
        Scheme::LogsOnPlinth => "log on a stone plinth",
        Scheme::StoneUnderTimber => "stone and timber",
        Scheme::StoneEnds => "log with stone ends",
        Scheme::Earth => "cob",
        Scheme::Wattle => "wattle and daub",
        Scheme::StoneUnderWattle => "stone and wattle and daub",
    };
    let roofed = match roofed {
        Roof::Gable => "a gable",
        Roof::Hip => "a hip roof",
        Roof::LeanTo => "a lean-to",
    };
    match plan {
        Plan::Rectangle => format!("{what} house under {roofed}"),
        Plan::L => format!("{what} L-shaped house under {roofed}"),
        Plan::T => format!("{what} T-shaped hall under {roofed}"),
        Plan::U => format!("{what} U-shaped farmstead under {roofed}"),
        Plan::TwoStoreys => format!("{what} house of two storeys under {roofed}"),
        Plan::Shed => format!("{what} woodshed under {roofed}"),
        Plan::Deck => format!("{what} lookout deck on posts"),
        Plan::Wall => format!("{what} garden wall"),
    }
}

/// The village's buildings from its seed: the same seed, the same village.
pub fn village(seed: u32, kinds: Kinds) -> Vec<Building> {
    // Each house: its plan, scheme and roof, how wide its front, where its door is east of
    // the green's middle and how far from the street's middle, and which side it is on.
    const NORTH: f32 = std::f32::consts::PI;
    const SOUTH: f32 = 0.0;
    let houses: [(Plan, Scheme, Roof, i32, [f32; 2], f32); 12] = [
        (
            Plan::Rectangle,
            Scheme::Logs,
            Roof::Gable,
            12,
            [-26.0, -5.5],
            SOUTH,
        ),
        (
            Plan::Rectangle,
            Scheme::Stone,
            Roof::Hip,
            14,
            [-13.0, -5.5],
            SOUTH,
        ),
        (
            Plan::TwoStoreys,
            Scheme::StoneUnderTimber,
            Roof::Gable,
            12,
            [-1.0, -9.5],
            SOUTH,
        ),
        (
            Plan::Rectangle,
            Scheme::LogsOnPlinth,
            Roof::Gable,
            14,
            [13.0, -5.5],
            SOUTH,
        ),
        (Plan::L, Scheme::Hewn, Roof::Gable, 14, [27.0, -9.0], SOUTH),
        (
            Plan::Rectangle,
            Scheme::StoneEnds,
            Roof::Gable,
            14,
            [-25.0, 5.5],
            NORTH,
        ),
        (Plan::T, Scheme::Logs, Roof::Hip, 14, [-9.0, 9.5], NORTH),
        (Plan::U, Scheme::Stone, Roof::Gable, 18, [8.0, 10.0], NORTH),
        (
            Plan::Rectangle,
            Scheme::Hewn,
            Roof::Hip,
            12,
            [24.0, 5.5],
            NORTH,
        ),
        // At the street's ends, the cob and the wattle and daub (D236).
        (
            Plan::Rectangle,
            Scheme::Earth,
            Roof::Gable,
            12,
            [-40.0, 5.5],
            NORTH,
        ),
        (
            Plan::Rectangle,
            Scheme::Wattle,
            Roof::Gable,
            12,
            [-42.0, -5.5],
            SOUTH,
        ),
        (
            Plan::TwoStoreys,
            Scheme::StoneUnderWattle,
            Roof::Gable,
            12,
            [40.0, 9.5],
            NORTH,
        ),
    ];
    let mut buildings = Vec::new();
    for (index, &(plan, scheme, roofed, wide, at, facing)) in houses.iter().enumerate() {
        let seed = hash(&[seed, index as u32]);
        // A little out of line, as a village is: a lean of a few degrees and a step either
        // way along the street.
        let lean = (unit(hash(&[seed, 1])) - 0.5) * 0.1;
        let shift = (unit(hash(&[seed, 2])) - 0.5) * 1.2;
        let ops = match plan {
            Plan::TwoStoreys => two_storeys(seed, scheme, roofed, kinds),
            _ => one_storey(seed, plan, scheme, roofed, wide, kinds),
        };
        buildings.push(Building {
            name: name(plan, scheme, roofed),
            plan,
            scheme,
            at: [at[0] + shift, at[1]],
            facing: facing + lean,
            ops,
        });
    }
    // A woodshed beside the log cabin, its open front to the street.
    buildings.push(Building {
        name: name(Plan::Shed, Scheme::Logs, Roof::LeanTo),
        plan: Plan::Shed,
        scheme: Scheme::Logs,
        at: [-34.5, -6.5],
        facing: SOUTH + 0.15,
        ops: shed(hash(&[seed, 20]), kinds),
    });
    // The lookout at the east end, its stair down toward the street.
    buildings.push(Building {
        name: name(Plan::Deck, Scheme::Hewn, Roof::Gable),
        plan: Plan::Deck,
        scheme: Scheme::Hewn,
        at: [38.0, 2.0],
        facing: -std::f32::consts::FRAC_PI_2,
        ops: deck(kinds),
    });
    // Gardens walled in stone behind two houses of the southern row, a gap in each back
    // wall for a gate; each length its own, standing on the ground where it is.
    const EAST: f32 = std::f32::consts::FRAC_PI_2;
    let long = |metres: f32| (metres / crate::CELL_M).round() as i32;
    let mut n = 0;
    for (west, east, front, back) in [(-30.5, -19.5, 11.5, 17.5), (19.5, 29.5, 11.5, 16.5)] {
        let gate = (west + east) * 0.5;
        let lengths = [
            // The back wall, either side of the gate.
            (
                (west + gate - 0.5) * 0.5,
                back,
                NORTH,
                long(gate - 0.5 - west),
            ),
            (
                (gate + 0.5 + east) * 0.5,
                back,
                NORTH,
                long(east - gate - 0.5),
            ),
            // The sides, from the house's back to a cube short of the back wall.
            (
                west,
                (front + back - 0.5) * 0.5,
                -EAST,
                long(back - 0.5 - front),
            ),
            (
                east,
                (front + back - 0.5) * 0.5,
                EAST,
                long(back - 0.5 - front),
            ),
        ];
        for (at_east, at_south, facing, long) in lengths {
            n += 1;
            buildings.push(Building {
                name: format!("stone garden wall {n}"),
                plan: Plan::Wall,
                scheme: Scheme::Stone,
                at: [at_east, at_south],
                facing,
                ops: low_wall(long, kinds),
            });
        }
    }
    buildings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Base;
    use std::collections::BTreeSet;

    const KINDS: Kinds = Kinds {
        logs: Kind(1),
        stone: Kind(2),
        hewn: Kind(3),
        earth: Kind(4),
        wattle: Kind(5),
        shakes: Kind(0x800),
        slate: Kind(0x801),
        thatch: Kind(0x802),
    };

    #[test]
    fn every_building_is_roofed_in_cubes_that_hold() {
        for building in village(7, KINDS) {
            let base = Base::from_shortcuts(&building.ops);
            let roofed = base.cubes().filter(|(_, c)| c.kind.is_roof()).count();
            let wants = !matches!(building.plan, Plan::Wall | Plan::Deck);
            assert_eq!(roofed > 0, wants, "{}", building.name);
            assert!(
                base.floating().is_empty(),
                "{}: {:?}",
                building.name,
                base.floating()
            );
        }
    }

    /// A building's cubes' columns where they stand in the village, in metres east and
    /// south, a cube's middle each.
    fn columns(building: &Building) -> Vec<[f32; 2]> {
        let mut base = Base::default();
        for &op in &building.ops {
            base.apply(op);
        }
        let (s, c) = (libm::sinf(building.facing), libm::cosf(building.facing));
        base.occupied_columns()
            .map(|column| {
                let (x, z) = (
                    (column.x as f32 + 0.5) * crate::CELL_M,
                    (column.y as f32 + 0.5) * crate::CELL_M,
                );
                // Turned so its +Z looks along `facing`.
                [
                    building.at[0] + x * c + z * s,
                    building.at[1] - x * s + z * c,
                ]
            })
            .collect()
    }

    #[test]
    fn a_village_is_the_same_from_the_same_seed_and_varies_with_it() {
        assert_eq!(village(7, KINDS), village(7, KINDS));
        assert_ne!(village(7, KINDS), village(8, KINDS));
    }

    #[test]
    fn every_scheme_and_plan_is_there_and_no_two_buildings_stand_in_one_another() {
        let all = village(7, KINDS);
        let schemes: BTreeSet<Scheme> = all.iter().map(|b| b.scheme).collect();
        let plans: BTreeSet<Plan> = all.iter().map(|b| b.plan).collect();
        assert_eq!(schemes.len(), 9, "{schemes:?}");
        assert_eq!(plans.len(), 8, "{plans:?}");
        assert!(variants() >= 150, "{}", variants());
        let placed: Vec<Vec<[f32; 2]>> = all.iter().map(columns).collect();
        for (i, a) in placed.iter().enumerate() {
            for (j, b) in placed.iter().enumerate().skip(i + 1) {
                // A metre apart at least; lengths of one garden wall may meet at a corner.
                let apart = if all[i].plan == Plan::Wall && all[j].plan == Plan::Wall {
                    0.4
                } else {
                    1.0
                };
                let near = a.iter().any(|p| {
                    b.iter()
                        .any(|q| (p[0] - q[0]).abs() < apart && (p[1] - q[1]).abs() < apart)
                });
                assert!(!near, "{} stands in {}", all[i].name, all[j].name);
            }
        }
    }

    #[test]
    fn every_house_looks_onto_the_street_and_stands_off_it() {
        for building in village(7, KINDS)
            .iter()
            .filter(|b| !matches!(b.plan, Plan::Wall | Plan::Deck))
        {
            // Its door looks toward the street's middle line, z = 0.
            let toward = libm::cosf(building.facing) * -building.at[1].signum();
            assert!(toward > 0.9, "{}: {toward}", building.name);
            // Nothing of it in the street, three metres either side of its middle.
            assert!(
                columns(building).iter().all(|p| p[1].abs() > 3.0),
                "{} stands in the street",
                building.name
            );
        }
    }

    #[test]
    fn a_two_storey_house_has_its_stair_up_to_a_landing_at_its_upper_door() {
        let all = village(7, KINDS);
        let two = all.iter().find(|b| b.plan == Plan::TwoStoreys).unwrap();
        let mut base = Base::default();
        for &op in &two.ops {
            base.apply(op);
        }
        // A stair's top step a storey up, the landing beside it, and the door off that.
        let stairs: Vec<_> = base
            .cubes()
            .filter(|(_, cube)| matches!(cube.shape, Shape::Stair(_)))
            .map(|(at, _)| at.y)
            .collect();
        assert_eq!(stairs.iter().max(), Some(&(STOREY_CUBES - 1)));
        assert!(
            base.walls()
                .any(|(tile, panel)| tile.y == 1 && panel == Panel::Door)
        );
    }
}
