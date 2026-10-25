//! What each module of the dressing becomes: cubes in the cell and props on its floor, in the
//! module's frame (its north side as written, turned as the variant turns). A module is known
//! by words in its name, so a theme's new module that says `pillar`, `alcove` or `shore` is
//! drawn as one; a name not known is a plain floor. The features placement holds (the goal,
//! the keys, the rest's fire) are objects and lights here too.

use super::{Frame, Kinds, Light, LightKind, Object, ObjectRole, Prop, Stamp, hash, shaped, solid};
use crate::Dungeon;
use crate::dress::{ModuleSet, RUBBLE, WATER};
use crate::place::{Cell, Side, opposite};
use crate::theme::Theme;
use terra_build::{BuildOp, Facing, Shape};

/// What a cell is drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Plain,
    /// A square column of trim, floor to ceiling, at the middle.
    Pillar,
    /// A niche in a thickened north wall.
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
    pub sockets: [u8; 4],
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
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
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
        (&["vein", "ore_"], Class::Vein),
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
        let ops = &mut out.ops;
        match look.class {
            Class::Pillar => f.fill(ops, [2, 0, 2], [3, 6, 3], solid(kinds.trim)),
            Class::Alcove if walled(0) => {
                f.fill(ops, [1, 0, 1], [4, 6, 1], solid(kinds.wall));
                f.fill(ops, [2, 1, 1], [3, 3, 1], None);
            }
            Class::Collapse => collapse(cell, kinds, out),
            Class::Rubble => rubble(cell, look, kinds, out),
            Class::Basin => sink(Frame::new(cell.at, 0), cell, 0, kinds, ops),
            Class::Shore => shore(cell, look, kinds, ops),
            Class::Runner => {
                let z0 = if walled(0) { 1 } else { 0 };
                let z1 = if walled(2) { 4 } else { 5 };
                f.fill(ops, [2, -1, z0], [3, -1, z1], solid(kinds.trim));
            }
            Class::Sarcophagus if walled(0) => {
                f.fill(ops, [1, 0, 1], [4, 1, 2], solid(kinds.wall));
            }
            Class::Sarcophagus => f.fill(ops, [1, 0, 2], [4, 1, 3], solid(kinds.wall)),
            Class::Timber => {
                for (d, x) in [(1, 5), (3, 0)] {
                    if walled(d) {
                        f.fill(ops, [x, 0, 3], [x, 6, 3], solid(kinds.trim));
                    }
                }
                if walled(1) && walled(3) {
                    f.fill(ops, [0, 6, 3], [5, 6, 3], solid(kinds.trim));
                }
            }
            Class::Goal => {
                f.fill(ops, [2, 0, 2], [3, 0, 3], solid(kinds.trim));
                let name = &theme.features.goal;
                let model = if name.contains("miner") {
                    "models/character/mannequin.glb"
                } else {
                    "models/props/Chest.glb"
                };
                out.objects
                    .push(object(name, ObjectRole::Goal, f, 1.0, model));
            }
            Class::Key => key(f, theme, kinds, out),
            Class::Rest => out.lights.push(Light {
                at: f.point(3.0, 0.0, 3.0),
                kind: LightKind::Campfire,
                facing: 0,
            }),
            _ => props(f, cell, look.class, &mut out.props),
        }
    }
}

fn object(name: &str, role: ObjectRole, f: Frame, y: f32, model: &str) -> Object {
    Object {
        name: name.to_string(),
        role,
        at: f.point(3.0, y, 3.0),
        facing: f.side(2),
        model: model.to_string(),
        locked: false,
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
        f.fill(&mut out.ops, [2, 0, 2], [3, high - 1, 3], solid(kinds.trim));
    }
    out.objects
        .push(object(name, ObjectRole::Key, f, high as f32, model));
    if name.contains("brazier") {
        out.lights.push(Light {
            at: f.point(3.0, high as f32 + 1.0, 3.0),
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
            props.push(prop("models/props/VaseRow.glb", f, 3.0, 1.5, 2, 1.0));
            props.push(prop("models/props/VasePair.glb", f, 4.5, 3.0, 3, 1.0));
        }
        Class::Stores => {
            props.push(prop("models/props/Barrel.glb", f, 4.3, 1.7, 2, 1.0));
            props.push(prop("models/props/BoltCrate.glb", f, 2.5, 1.5, 2, 1.0));
        }
        Class::Rack => props.push(prop("models/props/AxesLaid.glb", f, 3.0, 1.5, 2, 1.0)),
        Class::Table => props.push(prop("models/props/Bench.glb", f, 3.0, 3.0, 1, 1.0)),
        Class::Vein => props.push(prop(&rock(cell.at, 1), f, 3.0, 1.3, 2, 0.6)),
        _ => {}
    }
}

/// A heap against the cell's most walled corner: a cube lower every two cubes out, from 2.5 m,
/// a slope on each column's top, and a few rocks on it.
fn collapse(cell: &Cell, kinds: &Kinds, out: &mut Stamp) {
    let walls = |t: usize| {
        (cell.sides[t] != Side::Open) as u32 + (cell.sides[(t + 1) % 4] != Side::Open) as u32
    };
    let turn = (0..4).max_by_key(|&t| (walls(t), 4 - t)).unwrap_or(0);
    let f = Frame::new(cell.at, turn);
    let walled = |d: usize| cell.sides[f.side(d)] != Side::Open;
    let height = |x: i32, z: i32| 5 - ((5 - x) + z) / 2;
    for x in 0..6 {
        for z in 0..6 {
            let edge = (z == 0 && walled(0))
                || (x == 5 && walled(1))
                || (z == 5 && walled(2))
                || (x == 0 && walled(3));
            let h = height(x, z);
            if edge || h <= 0 {
                continue;
            }
            if h > 1 {
                f.fill(&mut out.ops, [x, 0, z], [x, h - 2, z], solid(kinds.wall));
            }
            let rise = if hash(cell.at, (x * 6 + z) as u32).is_multiple_of(2) {
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
        let (x, z) = (1 + (hash(cell.at, 10 + k) % 4) as i32, 1 + k as i32);
        let at = f.point(x as f32 + 0.5, height(x, z) as f32, z as f32 + 0.5);
        out.props.push(Prop {
            model: rock(cell.at, 20 + k),
            at,
            facing: (hash(cell.at, 30 + k) % 4) as usize,
            scale: 0.5 + (hash(cell.at, 40 + k) % 4) as f32 * 0.1,
        });
    }
}

/// Half cubes and a rock or two along the side the rubble spills from.
fn rubble(cell: &Cell, look: Look, kinds: &Kinds, out: &mut Stamp) {
    let side = (0..4)
        .find(|&d| look.sockets[d] & RUBBLE != 0)
        .or_else(|| (0..4).find(|&d| cell.sides[d] == Side::Wall))
        .unwrap_or(look.turn);
    let f = Frame::new(cell.at, side);
    let z0 = if cell.sides[side] == Side::Open { 0 } else { 1 };
    for k in 0..3 {
        let x = 1 + (hash(cell.at, 50 + k) % 4) as i32;
        let z = z0 + (hash(cell.at, 60 + k) % 2) as i32;
        f.fill(
            &mut out.ops,
            [x, 0, z],
            [x, 0, z],
            shaped(kinds.wall, Shape::Half),
        );
    }
    for k in 0..1 + hash(cell.at, 70) % 2 {
        let x = 1.0 + (hash(cell.at, 80 + k) % 4) as f32;
        out.props.push(Prop {
            model: rock(cell.at, 90 + k),
            at: f.point(x + 0.5, 0.0, z0 as f32 + 1.5),
            facing: (hash(cell.at, 100 + k) % 4) as usize,
            scale: 0.3,
        });
    }
}

/// The floor a cube lower from row `from` (of the frame's north) to the south edge, up to the
/// walls: the slab taken out and laid again a cube down.
fn sink(f: Frame, cell: &Cell, from: i32, kinds: &Kinds, ops: &mut Vec<BuildOp>) {
    let walled = |d: usize| cell.sides[f.side(d)] != Side::Open;
    let x0 = if walled(3) { 1 } else { 0 };
    let x1 = if walled(1) { 4 } else { 5 };
    let z0 = if walled(0) { from.max(1) } else { from };
    let z1 = if walled(2) { 4 } else { 5 };
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
    sink(f, cell, 3, kinds, ops);
    let walled = |d: usize| cell.sides[f.side(d)] != Side::Open;
    let x0 = if walled(3) { 1 } else { 0 };
    let x1 = if walled(1) { 4 } else { 5 };
    f.fill(
        ops,
        [x0, -1, 2],
        [x1, -1, 2],
        shaped(kinds.floor, Shape::Stair(Facing::North)),
    );
}
