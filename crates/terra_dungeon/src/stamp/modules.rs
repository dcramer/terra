//! What each module of the dressing becomes: cubes in the cell and props on its floor, in the
//! module's frame (its north side as written, turned as the variant turns). A module is known
//! by words in its name, so a theme's new module that says `pillar`, `alcove` or `shore` is
//! drawn as one; a name not known is a plain floor. The features placement holds (the goal,
//! the keys, the rest's fire) are objects and lights here too.

use super::{
    CELL, EDGE, Frame, INNER, Kinds, Light, LightKind, MID, Object, ObjectRole, Prop, Stamp, hash,
    shaped, solid,
};
use crate::Dungeon;
use crate::dress::{ModuleSet, RUBBLE, WATER};
use crate::place::{Cell, STEPS, Side, Space, opposite};
use crate::theme::Theme;
use terra_build::{BuildOp, Facing, Shape};

/// What a cell is drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Plain,
    /// A square column of trim, floor to ceiling, at the middle.
    Pillar,
    /// Two tiers of niches in a thickened north wall, loculi.
    Alcove,
    /// A heap of stone against its walled corner; blocks the cell.
    Collapse,
    /// Half cubes and small rocks along its rubble side.
    Rubble,
    /// The floor a cube lower, for water.
    Basin,
    /// Steps down from the dry side to the basin on its wet side.
    Shore,
    /// A strip of trim down the floor's middle, north to south.
    Runner,
    /// A torch on its north wall first.
    Decor,
    /// A stone tomb against the north wall.
    Sarcophagus,
    Urns,
    /// Barrels and crates in the corner.
    Stores,
    Rack,
    Table,
    /// Posts of trim in the side walls and a beam across.
    Timber,
    /// A rock against the north wall.
    Vein,
    Goal,
    Key,
    Rest,
    Stair,
}

/// A cell's module as drawn: its class, its turn and the sockets of its sides, turned.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Look {
    pub class: Class,
    pub turn: usize,
    pub sockets: [u32; 4],
}

pub(crate) fn looks(dungeon: &Dungeon, theme: &Theme, set: &ModuleSet) -> Vec<Look> {
    dungeon
        .dressing
        .picks
        .iter()
        .map(|&v| {
            let variant = &set.variants[v];
            Look {
                class: class_of(theme, set, v),
                turn: variant.turn as usize,
                sockets: variant.sides,
            }
        })
        .collect()
}

fn class_of(theme: &Theme, set: &ModuleSet, variant: usize) -> Class {
    let m = &theme.modules[set.variants[variant].module];
    let f = &theme.features;
    if m.name == f.goal {
        return Class::Goal;
    } else if m.name == f.key {
        return Class::Key;
    } else if m.name == f.rest {
        return Class::Rest;
    } else if m.name == f.stair {
        return Class::Stair;
    } else if m.blocks {
        return Class::Collapse;
    }
    let name = m.name.to_lowercase();
    // A word names a class where one of the name's words starts with it ("sarcophag" in
    // "sarcophagus", "urn" in "urns"), never inside another: by "rack" in "cracked_flags" each
    // cracked floor of the crypt laid a weapon rack, 21 sets of axes lying about it (dress6).
    let has = |words: &[&str]| {
        (name.split(['_', ' ', '-'])).any(|token| words.iter().any(|w| token.starts_with(w)))
    };
    let classes: [(&[&str], Class); 14] = [
        (&["shore", "bank"], Class::Shore),
        (&["flood", "pool", "puddle", "water", "basin"], Class::Basin),
        (&["pillar", "post", "column"], Class::Pillar),
        (&["loculi", "alcove", "niche"], Class::Alcove),
        (
            &["rubble", "spill", "spoil", "drift", "debris", "scree"],
            Class::Rubble,
        ),
        (&["rail", "runner", "carpet"], Class::Runner),
        (&["sconce", "torch", "banner", "decor"], Class::Decor),
        (&["sarcophag", "tomb", "coffin"], Class::Sarcophagus),
        (&["urn", "vase"], Class::Urns),
        (&["tool", "crate", "barrel", "store"], Class::Stores),
        (&["rack", "weapon"], Class::Rack),
        (&["trestle", "table", "bench"], Class::Table),
        (&["timber", "shoring"], Class::Timber),
        (&["vein", "ore"], Class::Vein),
    ];
    if set.wet(variant) && !has(&["shore", "bank"]) {
        return Class::Basin;
    }
    classes
        .iter()
        .find(|(words, _)| has(words))
        .map_or(Class::Plain, |&(_, class)| class)
}

pub(super) fn lay(
    dungeon: &Dungeon,
    theme: &Theme,
    looks: &[Look],
    kinds: &Kinds,
    out: &mut Stamp,
) {
    for (c, cell) in dungeon.layout.cells.iter().enumerate() {
        let look = looks[c];
        let f = Frame::new(cell.at, look.turn);
        let walled = |d: usize| cell.sides[f.side(d)] != Side::Open;
        // The cube under the ceiling, and the middle two rows.
        let (top, m) = (kinds.head(cell.space) - 1, CELL / 2);
        let ops = &mut out.ops;
        match look.class {
            // The kit's piers stand where a hall's bays meet instead.
            Class::Pillar if theme.kit.is_some() => {}
            Class::Pillar => pillar(cell, look, top, kinds, ops),
            Class::Alcove if walled(0) => match &theme.kit {
                Some(kit) => super::kit::loculi(f, cell, kit, kinds, out),
                None => {
                    f.fill(ops, [1, 0, 1], [INNER, top, 1], solid(kinds.wall));
                    for y in [1, 5].into_iter().filter(|&y| y + 2 < top) {
                        f.fill(ops, [m - 1, y, 1], [m, y + 2, 1], None);
                    }
                }
            },
            Class::Collapse => collapse(cell, kinds, out),
            // Fallen stone lies only under what it fell from (D539's plan): a spill beside no
            // collapse is, in a kit's halls, a drift of the dead swept to the wall.
            Class::Rubble => match (fallen_from(dungeon, looks, c), &theme.kit) {
                (Some(side), _) => rubble(cell, side, out),
                (None, Some(kit)) => drift(cell, &kit.remains, out),
                (None, None) => rubble(cell, rubble_side(cell, look), out),
            },
            Class::Basin => sink(Frame::new(cell.at, 0), cell, 0, kinds, ops),
            Class::Shore => shore(cell, look, kinds, ops),
            Class::Runner => {
                let z0 = if walled(0) { 1 } else { 0 };
                let z1 = if walled(2) { INNER } else { EDGE };
                f.fill(ops, [m - 1, -1, z0], [m, -1, z1], solid(kinds.trim));
            }
            // Two metres long, a metre deep and high.
            Class::Sarcophagus => {
                let z = tomb_row(cell, f);
                f.fill(ops, [m - 2, 0, z], [m + 1, 1, z + 1], solid(kinds.wall));
                if let Some(tomb) = theme.kit.as_ref().and_then(|k| k.sarcophagus.as_ref()) {
                    out.pieces.push(super::Piece {
                        model: tomb.clone(),
                        at: f.point(MID, 0.0, z as f32 + 1.0),
                        facing: f.side(2),
                        scale: glam::Vec3::ONE,
                    });
                }
            }
            Class::Timber => {
                // Each set where its cell puts it along the bay, and one in four with a post
                // and its beam gone: every set whole in the bay's middle, a straight
                // gallery read as five identical frames nested down the vista, a tiling bug
                // (round 13's mine film, 28.5 s).
                let z = 2 + (hash(cell.at, 420) % (CELL - 3) as u32) as i32;
                let broken = hash(cell.at, 421).is_multiple_of(4);
                let gone = hash(cell.at, 422) % 2;
                let wall = |d: usize| cell.sides[f.side(d)] == Side::Wall;
                for (k, (d, x)) in [(1, EDGE), (3, 0)].into_iter().enumerate() {
                    if wall(d) && !(broken && gone == k as u32) {
                        f.fill(ops, [x, 0, z], [x, top, z], solid(kinds.trim));
                    }
                }
                if !broken && wall(1) && wall(3) {
                    f.fill(ops, [0, top, z], [EDGE, top, z], solid(kinds.trim));
                }
            }
            Class::Goal => {
                f.fill(ops, [m - 1, 0, m - 1], [m, 0, m], solid(kinds.trim));
                let name = &theme.features.goal;
                // The trapped miner is the textured base body, not the pale untextured
                // mannequin, which burnt pink-white between the bowls (round 16).
                let model = if name.contains("miner") {
                    "models/character/base.glb"
                } else {
                    "models/props/Chest.glb"
                };
                out.objects
                    .push(object(name, ObjectRole::Goal, f, 1.0, model));
                goal_fires(f, cell, kinds, out);
            }
            Class::Key => key(f, theme, kinds, out),
            Class::Rest => out.lights.push(Light {
                at: f.point(MID, 0.0, MID),
                kind: LightKind::Campfire,
                facing: 0,
            }),
            _ => props(f, cell, look.class, &mut out.props),
        }
        if matches!(cell.space, Space::Corridor(_)) && look.class != Class::Timber {
            corridor(cell, kinds, theme.kit.is_none(), out);
        }
    }
}

/// A corridor's run: a rib of trim across its ceiling every cell, a half metre down, where
/// both its sides are walled, so a run reads in bays (a vault's transverse ribs, a mine's
/// sets); and its edges worn, a fallen stone or two at a wall's foot and now and then a
/// smaller one by the other, out of the middle where feet go. The fallen stones are rocks,
/// each turned and sized its own way: half cubes of the wall's stone stood there before and
/// read as blockout boxes, a step of them down every corridor (round 13's films). `bare`, a
/// theme with no kit: a kit's barrel vault carries its own ribs, and in its halls stone lies
/// only under a collapse (D539's plan; its corridors' stones read as confetti, dress r19).
fn corridor(cell: &Cell, kinds: &Kinds, bare: bool, out: &mut Stamp) {
    let walled = |d: usize| cell.sides[d] == Side::Wall;
    let along = if walled(1) && walled(3) {
        Some(0)
    } else if walled(0) && walled(2) {
        Some(1)
    } else {
        None
    };
    let Some(turn) = along else {
        return;
    };
    // Seen along the run: its walls east and west of the frame.
    let f = Frame::new(cell.at, turn);
    let top = kinds.head(cell.space) - 1;
    if !bare {
        return;
    }
    f.fill(
        &mut out.ops,
        [1, top, CELL / 2],
        [INNER, top, CELL / 2],
        solid(kinds.trim),
    );
    let west = hash(cell.at, 400).is_multiple_of(2);
    let (foot, other) = if west {
        (1.4, CELL as f32 - 1.5)
    } else {
        (CELL as f32 - 1.4, 1.5)
    };
    for k in 0..1 + hash(cell.at, 401) % 2 {
        let z = 1.0 + (hash(cell.at, 402 + k) % (INNER as u32 * 10)) as f32 / 10.0;
        out.props.push(Prop {
            model: rock(cell.at, 404 + k),
            at: f.point(foot, 0.0, z),
            facing: (hash(cell.at, 406 + k) % 4) as usize,
            scale: 0.35 + (hash(cell.at, 408 + k) % 4) as f32 * 0.05,
        });
    }
    if hash(cell.at, 410).is_multiple_of(3) {
        let z = 0.5 + (hash(cell.at, 411) % EDGE as u32) as f32;
        out.props.push(Prop {
            model: rock(cell.at, 412),
            at: f.point(other, 0.0, z),
            facing: (hash(cell.at, 413) % 4) as usize,
            scale: 0.25,
        });
    }
}

/// A fire bowl either side of the goal's plinth, in the back corners, each on a stone pillar
/// a metre high: the goal is the room's brightest thing, lit from both sides (a dark goal
/// room was the worst still of every theme, round 6), but from behind and a metre off, not
/// beside it: at 0.75 m the mine's pale figure burnt pink-white (5.6% of a still near white,
/// round 16). None stands in front of a doorway: with one in the back wall, the bowls stand
/// beside the plinth instead.
fn goal_fires(f: Frame, cell: &Cell, kinds: &Kinds, out: &mut Stamp) {
    let opening = |d: usize| matches!(cell.sides[f.side(d)], Side::Door(_) | Side::Way);
    let z = if opening(0) { 2 } else { 1 };
    for (x, side) in [(1, 3), (INNER, 1)] {
        if opening(side) {
            continue;
        }
        f.fill(&mut out.ops, [x, 0, z], [x, 1, z], solid(kinds.trim));
        out.lights.push(Light {
            at: f.point(x as f32 + 0.5, 2.0, z as f32 + 0.5),
            kind: LightKind::Bowl,
            facing: f.side(2),
        });
    }
}

fn object(name: &str, role: ObjectRole, f: Frame, y: f32, model: &str) -> Object {
    Object {
        name: name.to_string(),
        role,
        at: f.point(MID, y, MID),
        facing: f.side(2),
        model: model.to_string(),
        lock: crate::door::Lock::None,
        opens: [1.0; 2],
    }
}

/// The row a tomb stands from: against its cell's first side where that is walled.
fn tomb_row(cell: &Cell, f: Frame) -> i32 {
    if cell.sides[f.side(0)] != Side::Open {
        1
    } else {
        CELL / 2 - 1
    }
}

/// Where a cell's tomb or key feature stands, on its floor (the tomb's middle, the key's
/// altar, brazier or lever), for the lamp that lights it (furnish.rs).
pub(super) fn feature_at(cell: &Cell, look: Look) -> Option<glam::Vec3> {
    let f = Frame::new(cell.at, look.turn);
    match look.class {
        Class::Sarcophagus => Some(f.point(MID, 0.0, tomb_row(cell, f) as f32 + 1.0)),
        Class::Key => Some(f.point(MID, 0.0, MID)),
        _ => None,
    }
}

/// A key feature: an altar on a pedestal two cubes high, a brazier on one a cube high and lit,
/// a lever on the floor.
fn key(f: Frame, theme: &Theme, kinds: &Kinds, out: &mut Stamp) {
    let name = &theme.features.key;
    let (high, model) = if name.contains("altar") {
        (2, "models/props/Goblets.glb")
    } else if name.contains("brazier") {
        (1, "models/props/Campfire.glb")
    } else if name.contains("lever") || name.contains("pump") {
        (0, "models/props/Well.glb")
    } else {
        (1, "models/props/Chest.glb")
    };
    if high > 0 {
        let m = CELL / 2;
        f.fill(
            &mut out.ops,
            [m - 1, 0, m - 1],
            [m, high - 1, m],
            solid(kinds.trim),
        );
    }
    out.objects
        .push(object(name, ObjectRole::Key, f, high as f32, model));
    if name.contains("brazier") {
        out.lights.push(Light {
            at: f.point(MID, high as f32 + 1.0, MID),
            kind: LightKind::Brazier,
            facing: f.side(2),
        });
    }
}

fn prop(model: &str, f: Frame, x: f32, z: f32, facing: usize, scale: f32) -> Prop {
    Prop {
        model: model.to_string(),
        at: f.point(x, 0.0, z),
        facing: f.side(facing),
        scale,
    }
}

fn rock(at: glam::IVec3, salt: u32) -> String {
    format!("models/nature/Rock_{}.glb", 1 + hash(at, salt) % 10)
}

/// The modules that are props on the floor.
fn props(f: Frame, cell: &Cell, class: Class, props: &mut Vec<Prop>) {
    match class {
        Class::Urns => {
            props.push(prop("models/props/VaseRow.glb", f, MID, 1.5, 2, 1.0));
            props.push(prop("models/props/VasePair.glb", f, MID + 1.5, MID, 3, 1.0));
        }
        Class::Stores => {
            props.push(prop("models/props/Barrel.glb", f, MID + 1.3, 1.7, 2, 1.0));
            props.push(prop(
                "models/props/BoltCrate.glb",
                f,
                MID - 0.5,
                1.5,
                2,
                1.0,
            ));
        }
        Class::Rack => props.push(prop("models/props/AxesLaid.glb", f, MID, 1.5, 2, 1.0)),
        Class::Table => props.push(prop("models/props/Bench.glb", f, MID, MID, 1, 1.0)),
        Class::Vein => props.push(prop(&rock(cell.at, 1), f, MID, 1.3, 2, 0.6)),
        _ => {}
    }
}

/// A pillar of a row running north to south (its module's aisle sockets): at the middle
/// between two aisles; at a row's end, stood against the end wall as a respond is; none in
/// front of a doorway. On 4 m cells a middle pillar leaves a metre to an end wall, a gap a
/// tour squeezed through 0.5 m from both, and stood a metre in front of a doorway at the end.
fn pillar(cell: &Cell, look: Look, top: i32, kinds: &Kinds, ops: &mut Vec<BuildOp>) {
    let side = |d: usize| cell.sides[(d + look.turn) % 4];
    let (m, trim) = (CELL / 2, solid(kinds.trim));
    if [0, 2]
        .iter()
        .any(|&d| !matches!(side(d), Side::Open | Side::Wall))
    {
        return;
    }
    match [0, 2].into_iter().find(|&d| side(d) == Side::Wall) {
        Some(d) => Frame::new(cell.at, look.turn + d).fill(ops, [m - 1, 0, 1], [m, top, 2], trim),
        None => Frame::new(cell.at, look.turn).fill(ops, [m - 1, 0, m - 1], [m, top, m], trim),
    }
}

/// A heap against the cell's most walled corner: a cube lower every two cubes out, from 3.5 m,
/// a slope on each column's top, and a few rocks on it.
fn collapse(cell: &Cell, kinds: &Kinds, out: &mut Stamp) {
    let walls = |t: usize| {
        (cell.sides[t] != Side::Open) as u32 + (cell.sides[(t + 1) % 4] != Side::Open) as u32
    };
    let turn = (0..4).max_by_key(|&t| (walls(t), 4 - t)).unwrap_or(0);
    let f = Frame::new(cell.at, turn);
    let walled = |d: usize| cell.sides[f.side(d)] != Side::Open;
    let height = |x: i32, z: i32| EDGE - ((EDGE - x) + z) / 2;
    for x in 0..CELL {
        for z in 0..CELL {
            let edge = (z == 0 && walled(0))
                || (x == EDGE && walled(1))
                || (z == EDGE && walled(2))
                || (x == 0 && walled(3));
            let h = height(x, z);
            if edge || h <= 0 {
                continue;
            }
            if h > 1 {
                f.fill(&mut out.ops, [x, 0, z], [x, h - 2, z], solid(kinds.wall));
            }
            let rise = if hash(cell.at, (x * CELL + z) as u32).is_multiple_of(2) {
                Facing::North
            } else {
                Facing::East
            };
            f.fill(
                &mut out.ops,
                [x, h - 1, z],
                [x, h - 1, z],
                shaped(kinds.wall, Shape::Slope(rise)),
            );
        }
    }
    let rocks = 2 + hash(cell.at, 7) % 2;
    for k in 0..rocks {
        let (x, z) = (
            1 + (hash(cell.at, 10 + k) % INNER as u32) as i32,
            1 + k as i32,
        );
        let at = f.point(x as f32 + 0.5, height(x, z) as f32, z as f32 + 0.5);
        out.props.push(Prop {
            model: rock(cell.at, 20 + k),
            at,
            facing: (hash(cell.at, 30 + k) % 4) as usize,
            scale: 0.5 + (hash(cell.at, 40 + k) % 4) as f32 * 0.1,
        });
    }
}

/// How far out from its cause fallen stone rolls at most, in metres: a graded pile, big
/// stones at the cause's foot and smaller ones out to this, never strewn over the floor
/// (barrow dress round 19: every rooms reviewer read loose stones as confetti).
pub(crate) const CAUSE_M: f32 = 1.5;

/// The side of a rubble cell (`c`) a collapse beside it spills over, if one does.
pub(crate) fn fallen_from(dungeon: &Dungeon, looks: &[Look], c: usize) -> Option<usize> {
    let cell = &dungeon.layout.cells[c];
    (0..4).find(|&d| {
        looks[c].sockets[d] & RUBBLE != 0
            && dungeon
                .layout
                .cell(cell.at + STEPS[d])
                .is_some_and(|n| looks[n].class == Class::Collapse)
    })
}

/// The side a rubble cell spills from where no collapse is beside it: its rubble side, or
/// else a wall.
fn rubble_side(cell: &Cell, look: Look) -> usize {
    (0..4)
        .find(|&d| look.sockets[d] & RUBBLE != 0)
        .or_else(|| (0..4).find(|&d| cell.sides[d] == Side::Wall))
        .unwrap_or(look.turn)
}

/// The dead swept to a wall's foot: one of the kit's remains, lying along the wall.
fn drift(cell: &Cell, remains: &[String], out: &mut Stamp) {
    let Some(side) = (0..4).find(|&d| cell.sides[d] == Side::Wall) else {
        return;
    };
    let pick = hash(cell.at, 110) as usize % (remains.len() + 1);
    let Some(model) = remains.get(pick) else {
        return;
    };
    let f = Frame::new(cell.at, side);
    let x = MID + ((hash(cell.at, 111) % 5) as f32 - 2.0) * 0.4;
    out.props.push(Prop {
        model: model.clone(),
        at: f.point(x, 0.0, 2.0),
        facing: f.side(2),
        scale: 1.0,
    });
}

/// Stones fallen along the side the rubble spills from (`side`): a graded pile, a few big
/// ones against it and smaller ones out to `CAUSE_M`, fewer and smaller the farther, each its
/// own rock, turned and sized its own way. Half cubes of the wall's stone stood there before
/// and read as blockout boxes in every film (round 13).
fn rubble(cell: &Cell, side: usize, out: &mut Stamp) {
    let f = Frame::new(cell.at, side);
    let z0 = if cell.sides[side] == Side::Open {
        0.0
    } else {
        1.0
    };
    // How far out the farthest stone's middle lies, in cubes past its first row.
    let reach = CAUSE_M / super::CUBE - z0 - 0.4;
    let stones = 4 + hash(cell.at, 70) % 3;
    for k in 0..stones {
        // The first two lie against the side; the rest roll out, most of them near (the
        // square of an even draw), the smaller the farther.
        let big = k < 2;
        let x = 1.0 + (hash(cell.at, 50 + k) % 100) as f32 / 100.0 * (CELL as f32 - 2.5);
        let draw = (hash(cell.at, 60 + k) % 10) as f32 / 10.0;
        let t = if big { draw * 0.2 } else { draw * draw };
        let jitter = (hash(cell.at, 80 + k) % 5) as f32 * 0.05;
        let scale = if big {
            0.4 + jitter
        } else {
            0.38 - 0.22 * t + jitter * 0.5
        };
        out.props.push(Prop {
            model: rock(cell.at, 90 + k),
            at: f.point(x, 0.0, z0 + 0.4 + t * reach),
            facing: (hash(cell.at, 100 + k) % 4) as usize,
            scale,
        });
    }
}

/// The floor a cube lower from row `from` (of the frame's north) to the south edge, up to the
/// walls: the slab taken out and laid again a cube down.
fn sink(f: Frame, cell: &Cell, from: i32, kinds: &Kinds, ops: &mut Vec<BuildOp>) {
    let walled = |d: usize| cell.sides[f.side(d)] != Side::Open;
    let x0 = if walled(3) { 1 } else { 0 };
    let x1 = if walled(1) { INNER } else { EDGE };
    let z0 = if walled(0) { from.max(1) } else { from };
    let z1 = if walled(2) { INNER } else { EDGE };
    f.fill(ops, [x0, -1, z0], [x1, -1, z1], None);
    f.fill(ops, [x0, -2, z0], [x1, -2, z1], solid(kinds.floor));
}

/// A shore: dry on one side, a basin on the side its water is, a row of stair cubes between.
fn shore(cell: &Cell, look: Look, kinds: &Kinds, ops: &mut Vec<BuildOp>) {
    let Some(wet) = (0..4).find(|&d| look.sockets[d] & WATER != 0 && cell.sides[d] == Side::Open)
    else {
        return;
    };
    // Seen from the dry side, the water south.
    let f = Frame::new(cell.at, opposite(wet));
    let m = CELL / 2;
    sink(f, cell, m, kinds, ops);
    let walled = |d: usize| cell.sides[f.side(d)] != Side::Open;
    let x0 = if walled(3) { 1 } else { 0 };
    let x1 = if walled(1) { INNER } else { EDGE };
    f.fill(
        ops,
        [x0, -1, m - 1],
        [x1, -1, m - 1],
        shaped(kinds.floor, Shape::Stair(Facing::North)),
    );
}
