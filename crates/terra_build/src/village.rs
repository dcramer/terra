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
    /// Walls of round timber, of masonry and of squared beams, by role, each a material
    /// the settlement's data names (`assets/data/settlement.ron`).
    pub timber: Kind,
    pub masonry: Kind,
    pub beams: Kind,
    /// Walls packed of earth, and an infill in a timber frame (D227, D236).
    pub cob: Kind,
    pub framed: Kind,
    /// The roof each of the first three takes: its material's `roof_kind`.
    pub timber_roof: Kind,
    pub masonry_roof: Kind,
    pub beams_roof: Kind,
}

/// Where what a settlement is built of is said, under `assets/`.
pub const SETTLEMENT: &str = "data/settlement.ron";

/// What a settlement is built of, by role, as its data names the materials (D290): no
/// code names one. Each role is one material or several, each building picking one by its
/// seed, so a settlement of many materials is one of many looks (D182).
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settlement {
    pub timber: Vec<String>,
    pub masonry: Vec<String>,
    pub beams: Vec<String>,
    pub cob: Vec<String>,
    pub framed: Vec<String>,
}

/// A settlement's materials by role, as kinds, and the roof each wall kind takes: what each
/// building picks its [`Kinds`] from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Palette {
    /// Timber, masonry, beams, cob and framed, in that order.
    pub roles: [Vec<Kind>; 5],
    pub roofs: std::collections::BTreeMap<Kind, Kind>,
}

/// What a building's kinds are picked from: one set for every building, or a palette each
/// picks from by its seed.
pub trait Picks {
    fn pick(&self, seed: u32) -> Kinds;
}

impl Picks for Kinds {
    fn pick(&self, _seed: u32) -> Kinds {
        *self
    }
}

impl Picks for &Palette {
    fn pick(&self, seed: u32) -> Kinds {
        self.choose(|r, of| (unit(hash(&[seed, 0x9a1e, r as u32])) * of as f32) as usize)
    }
}

impl Palette {
    /// A settlement's palette from its file, `read` giving a file under `assets/`, `named` a
    /// material's kind by its file's name, and `kits` the roof each takes.
    pub fn read(
        read: &dyn Fn(&str) -> Result<String, String>,
        named: &dyn Fn(&str) -> Option<Kind>,
        kits: &crate::kit::Kits,
    ) -> Result<Palette, String> {
        let text = read(SETTLEMENT)?;
        let settlement: Settlement =
            ron::from_str(&text).map_err(|why| format!("{SETTLEMENT}: {why}"))?;
        let kinds = |names: &[String]| -> Result<Vec<Kind>, String> {
            if names.is_empty() {
                return Err(format!("{SETTLEMENT}: a role with no material"));
            }
            names
                .iter()
                .map(|name| named(name).ok_or_else(|| format!("{SETTLEMENT}: no material {name}")))
                .collect()
        };
        let roles = [
            kinds(&settlement.timber)?,
            kinds(&settlement.masonry)?,
            kinds(&settlement.beams)?,
            kinds(&settlement.cob)?,
            kinds(&settlement.framed)?,
        ];
        let mut roofs = std::collections::BTreeMap::new();
        for &kind in roles.iter().flatten() {
            let roof = kits
                .roof_for(kind)
                .ok_or_else(|| format!("{SETTLEMENT}: no roof over {kind:?}"))?;
            roofs.insert(kind, roof);
        }
        Ok(Palette { roles, roofs })
    }

    /// The first of each role's, as a building with no seed of its own takes.
    pub fn first(&self) -> Kinds {
        self.choose(|_, _| 0)
    }

    /// Each role's material, `which` saying which of a role's (by its number, of how many),
    /// and the roof each of the first three takes.
    fn choose(&self, which: impl Fn(usize, usize) -> usize) -> Kinds {
        let role = |r: usize| {
            let all = &self.roles[r];
            all[which(r, all.len()).min(all.len() - 1)]
        };
        let roof = |kind: Kind| self.roofs.get(&kind).copied().unwrap_or(Kind::FIRST_ROOF);
        let (timber, masonry, beams) = (role(0), role(1), role(2));
        Kinds {
            timber,
            masonry,
            beams,
            cob: role(3),
            framed: role(4),
            timber_roof: roof(timber),
            masonry_roof: roof(masonry),
            beams_roof: roof(beams),
        }
    }
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
    Timber,
    Masonry,
    Beams,
    /// Timber walls on a masonry plinth a metre high.
    TimberOnPlinth,
    /// A masonry ground floor with beams over it.
    MasonryUnderBeams,
    /// Timber walls, their gable ends of masonry.
    MasonryEnds,
    /// Walls packed of earth on their plinth (D236).
    Cob,
    /// An infill in a timber frame (D236).
    Framed,
    /// A masonry ground floor with framed walls over it.
    MasonryUnderFramed,
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

    /// The doorway in the front wall at the origin, a metre wide, or two for a hall's pair
    /// of doors (D225).
    fn door(&mut self, storey: i32, kind: Kind, wide: bool) {
        let tiles = if wide { -2..=1 } else { -1..=0 };
        for x in tiles {
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
        Scheme::Timber | Scheme::TimberOnPlinth | Scheme::MasonryEnds => {
            (kinds.timber, kinds.timber)
        }
        Scheme::Masonry => (kinds.masonry, kinds.masonry),
        Scheme::Beams => (kinds.beams, kinds.beams),
        Scheme::MasonryUnderBeams => (kinds.masonry, kinds.beams),
        Scheme::Cob => (kinds.cob, kinds.cob),
        Scheme::Framed => (kinds.framed, kinds.framed),
        Scheme::MasonryUnderFramed => (kinds.masonry, kinds.framed),
    }
}

/// The kind a scheme's floors, stairs and landings are of: its upper walls', but timber
/// where those are framed, whose infill makes no floor.
fn decks_of(scheme: Scheme, kinds: Kinds) -> Kind {
    match scheme {
        Scheme::Framed | Scheme::MasonryUnderFramed => kinds.beams,
        _ => walls_of(scheme, kinds).1,
    }
}

/// A house of one or more blocks on the ground storey, each its own room, joined where
/// they meet, its door in the first block's front wall at the origin, windows where its
/// seed puts them, and its scheme's stone where it has any; a hall's doorway two metres
/// wide, for a pair of doors.
fn house(draft: &mut Draft, blocks: &[Block], scheme: Scheme, kinds: Kinds, hall: bool) {
    let (kind, _) = walls_of(scheme, kinds);
    for block in blocks {
        draft.room(block.min(0), block.max(0), kind);
    }
    draft.door(0, kind, hall);
    let door_half = if hall { 2 } else { 1 };
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
        // A pier of wall at least a cube wide between each window and the doorway.
        let off = door_half + 2 + (draft.roll(n) * 2.0) as i32;
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
    let stone = Cube::new(kinds.masonry, Fill::Solid);
    match scheme {
        // A metre of stone at the foot of every wall, its doorway's left open, laid after
        // the windows, which set their tile's whole column.
        Scheme::TimberOnPlinth => {
            for block in blocks {
                for side in sides(block.x0, block.x1, block.z0, block.z1) {
                    for [x, z] in side {
                        if z == -1 && (-door_half..door_half).contains(&x) {
                            continue;
                        }
                        draft.fill([x, 0, z], [x, 1, z], stone);
                    }
                }
            }
        }
        // Both ends of the first block, under its gables, of stone to the top.
        Scheme::MasonryEnds => {
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
        Scheme::Timber | Scheme::TimberOnPlinth | Scheme::MasonryEnds => kinds.timber_roof,
        Scheme::Masonry => kinds.masonry_roof,
        Scheme::Beams
        | Scheme::MasonryUnderBeams
        | Scheme::Cob
        | Scheme::Framed
        | Scheme::MasonryUnderFramed => kinds.beams_roof,
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
    house(&mut draft, &blocks, scheme, kinds, plan == Plan::T);
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
    house(&mut draft, &[main], scheme, kinds, false);
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

/// A cube at a time, as a player builds by hand (D226): each cube its own, of a kind and
/// a shape.
struct ByHand(Vec<BuildOp>);

impl ByHand {
    fn put(&mut self, at: [i32; 3], kind: Kind, shape: Shape) {
        self.0.push(BuildOp::Fill {
            min: at,
            max: at,
            cube: Some(Cube {
                shape,
                ..Cube::new(kind, Fill::Solid)
            }),
        });
    }

    fn open(&mut self, at: [i32; 3], kind: Kind) {
        self.0.push(BuildOp::Fill {
            min: at,
            max: at,
            cube: Some(Cube::new(kind, Fill::Open)),
        });
    }

    fn take(&mut self, at: [i32; 3]) {
        self.0.push(BuildOp::Fill {
            min: at,
            max: at,
            cube: None,
        });
    }

    /// Walls round a box of columns from one layer to another, its doorway a metre wide in
    /// the front (+Z) wall at the origin, `door` cubes high, the rest whole.
    fn walls(&mut self, x: [i32; 2], z: [i32; 2], y: [i32; 2], kind: Kind, door: i32) {
        for layer in y[0]..=y[1] {
            for cx in x[0]..=x[1] {
                for cz in z[0]..=z[1] {
                    if cx != x[0] && cx != x[1] && cz != z[0] && cz != z[1] {
                        continue;
                    }
                    if cz == z[1] && (cx == -1 || cx == 0) && layer < y[0] + door {
                        self.open([cx, layer, cz], kind);
                    } else {
                        self.put([cx, layer, cz], kind, Shape::Whole);
                    }
                }
            }
        }
    }

    /// A layer of whole cubes over a box of columns.
    fn layer(&mut self, x: [i32; 2], z: [i32; 2], y: i32, kind: Kind) {
        for cx in x[0]..=x[1] {
            for cz in z[0]..=z[1] {
                self.put([cx, y, cz], kind, Shape::Whole);
            }
        }
    }

    /// A gable over a box of columns, its ridge east and west, a row of slopes a layer up
    /// from each long side in turn from `y`, the gable ends of `ends` carried up under it.
    fn gable(&mut self, x: [i32; 2], z: [i32; 2], y: i32, roof: Kind, ends: Kind) {
        let deep = z[1] - z[0] + 1;
        for k in 0..(deep + 1) / 2 {
            let layer = y + k;
            for cx in x[0]..=x[1] {
                self.put([cx, layer, z[0] + k], roof, Shape::Slope(Facing::South));
                self.put([cx, layer, z[1] - k], roof, Shape::Slope(Facing::North));
            }
            for cz in z[0] + k + 1..z[1] - k {
                for cx in [x[0], x[1]] {
                    self.put([cx, layer, cz], ends, Shape::Whole);
                }
            }
        }
    }
}

/// A log cabin built by hand (D226): walls, floor and a gable of shakes laid a cube at a
/// time, and a dormer let into the front of its roof, its window looking out over the
/// street: what the shortcuts cannot make, and the demo shows (the goals reviews' finding).
fn dormer_house(kinds: Kinds) -> Vec<BuildOp> {
    let mut hand = ByHand(Vec::new());
    let (x, z) = ([-6, 5], [-10, -1]);
    hand.layer(x, z, -1, kinds.masonry);
    hand.walls(x, z, [0, 5], kinds.timber, 4);
    hand.gable(x, z, 6, kinds.timber_roof, kinds.timber);
    // The dormer, four cubes across, a wall dormer: its face of logs carried up from the
    // front wall through the eaves, a window two cubes wide in it, and its own little gable
    // of shakes running back into the main roof's slope, a roof on a roof (D343): nothing
    // but a roof stands on a roof's cubes.
    let dormer = [-2, 1];
    for cx in dormer[0]..=dormer[1] {
        hand.take([cx, 6, -1]);
        for layer in 6..=8 {
            if layer == 7 && cx != dormer[0] && cx != dormer[1] {
                hand.open([cx, layer, -1], kinds.timber);
            } else {
                hand.put([cx, layer, -1], kinds.timber, Shape::Whole);
            }
        }
    }
    // Its gable, its front of logs over the window, ridge running back into the main roof.
    for cz in -3..=-1 {
        hand.put(
            [dormer[0], 9, cz],
            kinds.timber_roof,
            Shape::Slope(Facing::East),
        );
        hand.put(
            [dormer[1], 9, cz],
            kinds.timber_roof,
            Shape::Slope(Facing::West),
        );
        for cx in dormer[0] + 1..dormer[1] {
            let kind = if cz == -1 {
                kinds.timber
            } else {
                kinds.timber_roof
            };
            hand.put([cx, 9, cz], kind, Shape::Whole);
        }
        hand.put(
            [dormer[0] + 1, 10, cz],
            kinds.timber_roof,
            Shape::Slope(Facing::East),
        );
        hand.put(
            [dormer[1] - 1, 10, cz],
            kinds.timber_roof,
            Shape::Slope(Facing::West),
        );
    }
    hand.0
}

/// A stone house built by hand with a storey of timber over it (D226): a stone ground floor
/// floored over in stone, and on that floor a storey of hewn timber, set back from the
/// stone's edge, under a gable of its roof, laid a cube at a time. A storey goes on a floor,
/// never on a roof (D343): the roof is put on over the top storey.
fn storey_over_roof(kinds: Kinds) -> Vec<BuildOp> {
    let mut hand = ByHand(Vec::new());
    let (x, z) = ([-6, 5], [-8, -1]);
    hand.layer(x, z, -1, kinds.masonry);
    hand.walls(x, z, [0, 5], kinds.masonry, 4);
    hand.layer(x, z, 6, kinds.masonry);
    // The storey over it, on its floor, a window in its front.
    let (ux, uz) = ([-5, 4], [-7, -2]);
    hand.walls(ux, uz, [7, 11], kinds.beams, 0);
    for cx in [-1, 0] {
        for layer in [8, 9] {
            hand.open([cx, layer, uz[1]], kinds.beams);
        }
    }
    hand.gable(ux, uz, 12, kinds.beams_roof, kinds.beams);
    hand.0
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
    draft.room(block.min(0), block.max(0), kinds.timber);
    for x in -2..=1 {
        draft.wall([x, 0, -1], Panel::Door, kinds.timber);
    }
    roof(&mut draft, &[block], Roof::LeanTo, 0, kinds.timber_roof);
    draft.ops
}

/// A lookout deck of hewn timber, three metres by two, a storey up on four posts, its
/// stair coming down toward the front: the door's place is the stair's foot.
fn deck(kinds: Kinds) -> Vec<BuildOp> {
    let mut draft = Draft::new(0);
    let kind = kinds.beams;
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
        Cube::new(kinds.masonry, Fill::Solid),
    );
    draft.ops
}

/// What a person would call a building of a plan, a scheme and a roof.
fn name(plan: Plan, scheme: Scheme, roofed: Roof) -> String {
    let what = match scheme {
        Scheme::Timber => "timber",
        Scheme::Masonry => "masonry",
        Scheme::Beams => "squared timber",
        Scheme::TimberOnPlinth => "timber on a masonry plinth",
        Scheme::MasonryUnderBeams => "masonry and timber",
        Scheme::MasonryEnds => "timber with masonry ends",
        Scheme::Cob => "packed",
        Scheme::Framed => "framed",
        Scheme::MasonryUnderFramed => "masonry and framed",
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

/// One of each sort of the village's buildings (`village`, seed 7), as the checks look at
/// them (`cargo xtask artifacts`, `cargo xtask rules`): each put up whole in its own cubes, on
/// ground falling in steps of 20 cm, its footings laid in the palette's first masonry.
pub fn each_sort(palette: &Palette) -> Vec<(Building, Base)> {
    sorts(palette)
        .into_iter()
        .map(|building| {
            let base = standing(&building, palette);
            (building, base)
        })
        .collect()
}

/// One of each sort of the village's buildings (seed 7), not yet put up.
pub fn sorts(palette: &Palette) -> Vec<Building> {
    let mut seen = BTreeSet::new();
    village(7, palette)
        .into_iter()
        .filter(|building| seen.insert(format!("{:?} {:?}", building.plan, building.scheme)))
        .collect()
}

/// A building put up whole in its own cubes, as [`each_sort`] puts each up.
pub fn standing(building: &Building, palette: &Palette) -> Base {
    let mut base = Base::default();
    for op in &building.ops {
        base.apply(*op);
    }
    base.lay_footings(palette.first().masonry, |column| {
        -((column[0] + column[1]).rem_euclid(3) as i16) * 20
    });
    base
}

/// The village's buildings from its seed: the same seed, the same village.
pub fn village(seed: u32, picks: impl Picks + Copy) -> Vec<Building> {
    let kinds = picks.pick(seed);
    // Each house: its plan, scheme and roof, how wide its front, where its door is east of
    // the green's middle and how far from the street's middle, and which side it is on.
    const NORTH: f32 = std::f32::consts::PI;
    const SOUTH: f32 = 0.0;
    let houses: [(Plan, Scheme, Roof, i32, [f32; 2], f32); 12] = [
        (
            Plan::Rectangle,
            Scheme::Timber,
            Roof::Gable,
            12,
            [-26.0, -5.5],
            SOUTH,
        ),
        (
            Plan::Rectangle,
            Scheme::Masonry,
            Roof::Hip,
            14,
            [-13.0, -5.5],
            SOUTH,
        ),
        (
            Plan::TwoStoreys,
            Scheme::MasonryUnderBeams,
            Roof::Gable,
            12,
            [-1.0, -9.5],
            SOUTH,
        ),
        (
            Plan::Rectangle,
            Scheme::TimberOnPlinth,
            Roof::Gable,
            14,
            [13.0, -5.5],
            SOUTH,
        ),
        (Plan::L, Scheme::Beams, Roof::Gable, 14, [27.0, -9.0], SOUTH),
        (
            Plan::Rectangle,
            Scheme::MasonryEnds,
            Roof::Gable,
            14,
            [-25.0, 5.5],
            NORTH,
        ),
        (Plan::T, Scheme::Timber, Roof::Hip, 14, [-9.0, 9.5], NORTH),
        (
            Plan::U,
            Scheme::Masonry,
            Roof::Gable,
            18,
            [8.0, 10.0],
            NORTH,
        ),
        (
            Plan::Rectangle,
            Scheme::Beams,
            Roof::Hip,
            12,
            [24.0, 5.5],
            NORTH,
        ),
        // At the street's ends, the cob and the wattle and daub (D236).
        (
            Plan::Rectangle,
            Scheme::Cob,
            Roof::Gable,
            12,
            [-40.0, 5.5],
            NORTH,
        ),
        (
            Plan::Rectangle,
            Scheme::Framed,
            Roof::Gable,
            12,
            [-42.0, -5.5],
            SOUTH,
        ),
        (
            Plan::TwoStoreys,
            Scheme::MasonryUnderFramed,
            Roof::Gable,
            12,
            [40.0, 9.5],
            NORTH,
        ),
    ];
    let mut buildings = Vec::new();
    for (index, &(plan, scheme, roofed, wide, at, facing)) in houses.iter().enumerate() {
        let seed = hash(&[seed, index as u32]);
        let kinds = picks.pick(seed);
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
        name: name(Plan::Shed, Scheme::Timber, Roof::LeanTo),
        plan: Plan::Shed,
        scheme: Scheme::Timber,
        at: [-34.5, -6.5],
        facing: SOUTH + 0.15,
        ops: shed(hash(&[seed, 20]), kinds),
    });
    // The lookout at the east end, its stair down toward the street.
    buildings.push(Building {
        name: name(Plan::Deck, Scheme::Beams, Roof::Gable),
        plan: Plan::Deck,
        scheme: Scheme::Beams,
        at: [38.0, 2.0],
        facing: -std::f32::consts::FRAC_PI_2,
        ops: deck(kinds),
    });
    // At the street's ends, two built by hand a cube at a time (D226): a log cabin with a
    // dormer, and a stone house with a storey of timber over it.
    buildings.push(Building {
        name: "log cabin with a dormer, built by hand".to_string(),
        plan: Plan::Rectangle,
        scheme: Scheme::Timber,
        at: [-56.0, -5.5],
        facing: SOUTH,
        ops: dormer_house(kinds),
    });
    buildings.push(Building {
        name: "masonry house with a timber storey, built by hand".to_string(),
        plan: Plan::Rectangle,
        scheme: Scheme::MasonryUnderBeams,
        at: [54.0, -5.5],
        facing: SOUTH,
        ops: storey_over_roof(kinds),
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
                name: format!("masonry garden wall {n}"),
                plan: Plan::Wall,
                scheme: Scheme::Masonry,
                at: [at_east, at_south],
                facing,
                ops: low_wall(long, kinds),
            });
        }
    }
    buildings
}

/// Every building the rules make, as its plan, scheme, roof and front's width (D182): a
/// rectangle under each roof, an L, a T or a U under a gable or a hip (a U only wide enough
/// for a yard between its wings), two storeys in each scheme that has them, a shed and a
/// deck. What a town to measure picks from ([`town`]).
pub fn every_building() -> Vec<(Plan, Scheme, Roof, i32)> {
    use Scheme::*;
    const SCHEMES: [Scheme; 9] = [
        Timber,
        Masonry,
        Beams,
        TimberOnPlinth,
        MasonryUnderBeams,
        MasonryEnds,
        Cob,
        Framed,
        MasonryUnderFramed,
    ];
    let mut all = Vec::new();
    for scheme in SCHEMES {
        for wide in WIDTHS {
            for roofed in [Roof::Gable, Roof::Hip, Roof::LeanTo] {
                all.push((Plan::Rectangle, scheme, roofed, wide));
            }
            for plan in [Plan::L, Plan::T, Plan::U] {
                if plan == Plan::U && wide < 14 {
                    continue;
                }
                for roofed in [Roof::Gable, Roof::Hip] {
                    all.push((plan, scheme, roofed, wide));
                }
            }
        }
        if !matches!(scheme, TimberOnPlinth | MasonryEnds) {
            for roofed in [Roof::Gable, Roof::Hip] {
                all.push((Plan::TwoStoreys, scheme, roofed, 12));
            }
        }
    }
    all.push((Plan::Shed, Timber, Roof::LeanTo, 6));
    all.push((Plan::Deck, Beams, Roof::Gable, 6));
    all
}

/// A building of [`every_building`]'s, from its seed, its door at a place looking a way.
pub(crate) fn put_up(
    seed: u32,
    (plan, scheme, roofed, wide): (Plan, Scheme, Roof, i32),
    at: [f32; 2],
    facing: f32,
    kinds: Kinds,
) -> Building {
    let ops = match plan {
        Plan::TwoStoreys => two_storeys(seed, scheme, roofed, kinds),
        Plan::Shed => shed(seed, kinds),
        Plan::Deck => deck(kinds),
        Plan::Wall => low_wall(wide, kinds),
        _ => one_storey(seed, plan, scheme, roofed, wide, kinds),
    };
    Building {
        name: name(plan, scheme, roofed),
        plan,
        scheme,
        at,
        facing,
        ops,
    }
}

/// How far a building's door stands back from its street's middle: further where its
/// wings come forward, or its stair beside it.
fn set_back(plan: Plan) -> f32 {
    match plan {
        Plan::L | Plan::U | Plan::TwoStoreys => 9.5,
        _ => 5.5,
    }
}

/// [`every_building`] in an order its seed shuffles, so a town of any size mixes them.
fn shuffled(seed: u32) -> Vec<(Plan, Scheme, Roof, i32)> {
    let mut all = every_building();
    for i in (1..all.len()).rev() {
        let j = hash(&[seed, 100, i as u32]) as usize % (i + 1);
        all.swap(i, j);
    }
    all
}

/// A town's plots: this far apart along a street, so many a side of each, and its streets
/// this far apart, so two rows of houses stand back to back between them with a gap.
const PLOT_M: f32 = 14.0;
const PLOTS_A_SIDE: usize = 20;
const STREETS_APART_M: f32 = 36.0;

/// A town of `count` buildings to measure building's drawing at scale (D185's `town300`):
/// streets running east and west, houses along both sides of each with their doors to it,
/// every plan, scheme, roof and width the rules make in an order the seed shuffles, so
/// three hundred are nearly every one of them once. Places in metres east and south of
/// the town's middle, as [`village`] gives them.
pub fn town(seed: u32, count: usize, picks: impl Picks + Copy) -> Vec<Building> {
    let choices = shuffled(seed);
    let per_street = PLOTS_A_SIDE * 2;
    let streets = count.div_ceil(per_street).max(1);
    (0..count)
        .map(|index| {
            let choice = choices[index % choices.len()];
            let (street, along) = (index / per_street, index % per_street);
            let (plot, north) = (along / 2, along % 2 == 0);
            let seed = hash(&[seed, index as u32]);
            // A little out of line, as a village's are.
            let lean = (unit(hash(&[seed, 1])) - 0.5) * 0.1;
            let shift = (unit(hash(&[seed, 2])) - 0.5) * 1.2;
            let east = (plot as f32 - (PLOTS_A_SIDE as f32 - 1.0) * 0.5) * PLOT_M + shift;
            let middle = (street as f32 - (streets as f32 - 1.0) * 0.5) * STREETS_APART_M;
            let back = set_back(choice.0);
            let (south, facing) = if north {
                (middle - back, 0.0)
            } else {
                (middle + back, std::f32::consts::PI)
            };
            put_up(seed, choice, [east, south], facing + lean, picks.pick(seed))
        })
        .collect()
}

/// Hamlets of houses spread over the land round a middle, out to `radius_m` and no nearer
/// than a tenth of it, to measure bases seen far off (D185's `far`): `count` buildings, five
/// to a hamlet along a lane of its own, each hamlet turned its own way.
pub fn hamlets(seed: u32, count: usize, radius_m: f32, picks: impl Picks + Copy) -> Vec<Building> {
    const PER_HAMLET: usize = 5;
    let choices = shuffled(seed ^ 0x5eed);
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let hamlet = (index / PER_HAMLET) as u32;
        let (plot, north) = ((index % PER_HAMLET) / 2, index % 2 == 0);
        // Spread evenly over the area between a tenth of the radius and all of it.
        let near = radius_m * 0.1;
        let share = unit(hash(&[seed, hamlet, 1]));
        let distance = libm::sqrtf(near * near + share * (radius_m * radius_m - near * near));
        let angle = unit(hash(&[seed, hamlet, 2])) * std::f32::consts::TAU;
        let turn = unit(hash(&[seed, hamlet, 3])) * std::f32::consts::TAU;
        let middle = [distance * libm::cosf(angle), distance * libm::sinf(angle)];
        let choice = choices[index % choices.len()];
        let back = set_back(choice.0);
        // Along its lane, then turned with the hamlet about its middle.
        let east = (plot as f32 - 1.0) * PLOT_M;
        let (south, facing) = if north {
            (-back, 0.0)
        } else {
            (back, std::f32::consts::PI)
        };
        let (s, c) = (libm::sinf(turn), libm::cosf(turn));
        let at = [
            middle[0] + east * c + south * s,
            middle[1] - east * s + south * c,
        ];
        let seed_of = hash(&[seed, index as u32]);
        out.push(put_up(
            seed_of,
            choice,
            at,
            facing + turn,
            picks.pick(seed_of),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Base;
    use std::collections::BTreeSet;

    const KINDS: Kinds = Kinds {
        timber: Kind(1),
        masonry: Kind(2),
        beams: Kind(3),
        cob: Kind(4),
        framed: Kind(5),
        timber_roof: Kind(0x800),
        masonry_roof: Kind(0x801),
        beams_roof: Kind(0x802),
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

    /// Whether any two buildings stand within a metre of one another, by the half-metre
    /// cells their columns are in.
    fn crowded(all: &[Building]) -> Option<(String, String)> {
        let mut taken: std::collections::HashMap<[i32; 2], usize> = Default::default();
        for (index, building) in all.iter().enumerate() {
            for p in columns(building) {
                let cell = [(p[0] / 0.5).floor() as i32, (p[1] / 0.5).floor() as i32];
                for dx in -2..=2 {
                    for dz in -2..=2 {
                        if let Some(&other) = taken.get(&[cell[0] + dx, cell[1] + dz])
                            && other != index
                        {
                            return Some((building.name.clone(), all[other].name.clone()));
                        }
                    }
                }
            }
            for p in columns(building) {
                taken.insert(
                    [(p[0] / 0.5).floor() as i32, (p[1] / 0.5).floor() as i32],
                    index,
                );
            }
        }
        None
    }

    #[test]
    fn a_town_mixes_every_scheme_and_plan_and_no_two_buildings_stand_in_one_another() {
        let all = town(7, 90, KINDS);
        assert_eq!(all.len(), 90);
        let schemes: BTreeSet<Scheme> = all.iter().map(|b| b.scheme).collect();
        let plans: BTreeSet<Plan> = all.iter().map(|b| b.plan).collect();
        assert_eq!(schemes.len(), 9, "{schemes:?}");
        assert!(plans.len() >= 5, "{plans:?}");
        assert_eq!(crowded(&all), None);
        assert!(every_building().len() >= 200, "{}", every_building().len());
    }

    #[test]
    fn a_town_of_many_materials_builds_each_house_of_one_and_uses_every_one() {
        // Six stones, each with its own roof; one of every other role.
        let stones: Vec<Kind> = (10..16).map(Kind).collect();
        let roofs = stones
            .iter()
            .map(|&k| (k, Kind(0x800 + k.0)))
            .chain([
                (KINDS.timber, KINDS.timber_roof),
                (KINDS.beams, KINDS.beams_roof),
            ])
            .collect();
        let palette = Palette {
            roles: [
                vec![KINDS.timber],
                stones.clone(),
                vec![KINDS.beams],
                vec![KINDS.cob],
                vec![KINDS.framed],
            ],
            roofs,
        };
        let all = town(7, 90, &palette);
        let mut used = BTreeSet::new();
        for building in &all {
            let mut base = Base::default();
            for &op in &building.ops {
                base.apply(op);
            }
            let walls: BTreeSet<Kind> = base
                .cubes()
                .map(|(_, cube)| cube.kind)
                .filter(|k| stones.contains(k))
                .collect();
            // A house is one stone throughout, and its roof the one that stone takes.
            assert!(walls.len() <= 1, "{}: {walls:?}", building.name);
            if let Some(&stone) = walls.first()
                && building.scheme == Scheme::Masonry
            {
                assert!(
                    base.cubes()
                        .any(|(_, cube)| cube.kind == Kind(0x800 + stone.0)),
                    "{}",
                    building.name
                );
            }
            used.extend(walls);
        }
        assert_eq!(used.len(), stones.len(), "{used:?}");
        assert_eq!(palette.first().masonry, stones[0]);
    }

    #[test]
    fn shortcuts_applied_together_lay_what_they_do_one_by_one() {
        for building in village(7, KINDS) {
            let mut one_by_one = Base::default();
            for &op in &building.ops {
                one_by_one.apply(op);
            }
            assert!(
                Base::from_shortcuts(&building.ops) == one_by_one,
                "{}",
                building.name
            );
        }
    }

    #[test]
    fn hamlets_spread_out_to_their_radius() {
        let all = hamlets(7, 60, 2000.0, KINDS);
        assert_eq!(all.len(), 60);
        let furthest = all
            .iter()
            .map(|b| libm::hypotf(b.at[0], b.at[1]))
            .fold(0.0, f32::max);
        assert!((1200.0..2050.0).contains(&furthest), "{furthest}");
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

    // Slow, a minute or two: lays and draws one of each sort of the village's buildings, which
    // the quick check before each commit skips and the full check runs (`SLOW_TESTS` in xtask).
    #[test]
    fn no_two_faces_of_the_villages_buildings_fight_for_the_same_pixels() {
        // One of each sort of the village's buildings, in the settlement's own materials as
        // the game reads them, modelled pieces and all: none of their near meshes' faces lie
        // in one plane over one another (`inspect::fights`, the checker's measure). The
        // farmstead's lintels at a wall's end, the outside stair's stringers and the deck
        // beside it, a doorway's posts on a footing, the lookout's rail, ridge caps cut to
        // length and a verge under a higher roof's eave all did.
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let read = |path: &str| {
            std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
        };
        let mut paths: Vec<_> = std::fs::read_dir(assets.join("data/kits"))
            .expect("the kits")
            .flatten()
            .map(|entry| entry.path())
            .filter(|p| p.extension().is_some_and(|e| e == "ron"))
            .collect();
        paths.sort();
        let mut names = std::collections::BTreeMap::new();
        let mut kits = Vec::new();
        for path in paths {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let kit =
                crate::kit::Kit::read(&format!("{}/{stem}.ron", crate::kit::MATERIALS), &read)
                    .unwrap();
            names.insert(stem, kit.kind());
            kits.push(kit);
        }
        let kits = crate::kit::Kits(kits);
        let palette = Palette::read(&read, &|name| names.get(name).copied(), &kits).unwrap();
        let maps = crate::relief::Maps::default();
        let progress = crate::sections::Progress::Whole;
        for (building, base) in each_sort(&palette) {
            let whole = crate::sections::Whole::lay(&base, &kits);
            let mut meshes = Vec::new();
            for at in crate::sections::drawable(&base) {
                let section = crate::sections::Section {
                    base: &base,
                    kits: &kits,
                    maps: &maps,
                    at: glam::IVec3::from_array(at),
                    progress: &progress,
                };
                let laid = crate::sections::lay(&section, &whole);
                let near = crate::sections::draw(&section, &laid, crate::sections::Reach::Near);
                for (&(kind, material), surface) in &near.near {
                    meshes.push((
                        format!("{kind:?} {material:?}"),
                        Some((kind, material)),
                        surface.clone(),
                    ));
                }
                meshes.push(("plain".to_string(), None, near.plain));
            }
            let fights = crate::inspect::fights(meshes.iter().map(|(n, k, s)| (n.clone(), *k, s)));
            assert!(fights.is_empty(), "{}: {fights:?}", building.name);
        }
    }
}
