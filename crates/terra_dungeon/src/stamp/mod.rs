//! Stage 5, the stamp: a dressed dungeon laid in building's half-metre cubes (D181), with what
//! the game spawns in it (props, lights, mobs, chests, the goal, keys and door leaves), where the
//! player starts and a tour through it. Plain data: the sandbox digs the boxes, applies the ops
//! and spawns the rest.
//!
//! The frame: metres, x east, y up, z south, from the north-west corner of cell (0, 0, 0) at
//! y 0, the top of the roof and of the stair up to the surface. A cell is 6 by 6 cubes; level
//! L (cell.y, counted down) has its floor top at cube height -8 (L + 1), its floor slab the
//! layer under that and its ceiling slab 7 layers above (the next level's floor), 3.5 m clear.
//!
//! - `shell`: slabs, walls, doorways, the stairs and the surface stair's parapet and arch.
//! - `modules`: what each module of the dressing becomes in cubes and props, and the features.
//! - `runin`: door leaves, chests, mobs, torches, and the tour.

mod modules;
mod runin;
mod shell;
#[cfg(test)]
mod tests;

use crate::Dungeon;
use crate::dress::ModuleSet;
use crate::place::Layout;
use crate::theme::Theme;
use glam::{IVec3, Vec3};
use std::collections::HashMap;
use terra_build::{BuildOp, Cube, Facing, Fill, Kind, Shape};

/// A building cube's edge, in metres.
pub const CUBE: f32 = 0.5;
/// A doorway's opening, across and up, in metres: what its door's leaves fill, a door
/// object standing at the middle of its sill.
pub const DOORWAY_M: [f32; 2] = [
    shell::DOORWAY[0] as f32 * CUBE,
    shell::DOORWAY[1] as f32 * CUBE,
];
/// Cubes across a cell.
pub(crate) const CELL: i32 = 6;
/// Cubes from one level's floor to the next.
pub(crate) const LEVEL: i32 = 8;
/// Lights a dungeon gets at most; the renderer draws the nearest 16 fires.
pub(crate) const LIGHTS: usize = 30;

/// A box of terrain to dig away before the cubes are laid.
#[derive(Clone, Debug)]
pub struct Dig {
    pub centre: Vec3,
    pub half: Vec3,
}

/// A model standing on the floor.
#[derive(Clone, Debug)]
pub struct Prop {
    /// From the asset root: `models/props/VaseRow.glb`.
    pub model: String,
    pub at: Vec3,
    /// The side its front faces, 0..4 = north, east, south, west.
    pub facing: usize,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKind {
    /// On a wall at 2 m.
    Torch,
    /// A key feature to light.
    Brazier,
    /// The rest room's fire, on the floor.
    Campfire,
}

#[derive(Clone, Debug)]
pub struct Light {
    pub at: Vec3,
    pub kind: LightKind,
    /// The side it faces: away from its wall.
    pub facing: usize,
}

#[derive(Clone, Debug)]
pub struct MobSpawn {
    pub species: String,
    /// On the floor.
    pub at: Vec3,
    pub room: usize,
}

#[derive(Clone, Debug)]
pub struct ChestSpawn {
    pub at: Vec3,
    pub facing: usize,
    /// Items of `assets/data/items.ron` and how many.
    pub items: Vec<(String, u32)>,
    pub room: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectRole {
    /// What the dungeon is for: the relic, the trapped miner.
    Goal,
    /// What opens the gate: an altar, a lever, a brazier.
    Key,
    /// A door leaf in a doorway.
    Door,
}

/// A dungeon object the game makes live.
#[derive(Clone, Debug)]
pub struct Object {
    /// The feature's module (`ossuary_altar`), or the door's link (`locked_door`).
    pub name: String,
    pub role: ObjectRole,
    pub at: Vec3,
    pub facing: usize,
    /// From the asset root.
    pub model: String,
    /// A door that opens when every key of its lock is held.
    pub locked: bool,
}

/// A dungeon laid out for the game.
#[derive(Clone, Debug)]
pub struct Stamp {
    /// One box a level and one for the surface stair.
    pub digs: Vec<Dig>,
    /// In cubes, applied in order (a later fill overwrites an earlier one).
    pub ops: Vec<BuildOp>,
    pub props: Vec<Prop>,
    pub lights: Vec<Light>,
    pub mobs: Vec<MobSpawn>,
    pub chests: Vec<ChestSpawn>,
    pub objects: Vec<Object>,
    /// On the surface at the top of the stair down.
    pub start: Vec3,
    pub start_facing: usize,
    /// A tour a camera can follow: down the stair, through the key rooms, to the goal.
    pub route: Vec<Vec3>,
}

/// Lays a dungeon in cubes; `kind` names the building kinds of the theme's walls, floors and
/// trim.
pub fn stamp(dungeon: &Dungeon, theme: &Theme, kind: &dyn Fn(&str) -> Kind) -> Stamp {
    let set = ModuleSet::new(theme).expect("the dungeon was dressed from these modules");
    let kinds = Kinds {
        wall: kind(&theme.walls),
        floor: kind(&theme.floors),
        trim: kind(&theme.trim),
    };
    let looks = modules::looks(dungeon, theme, &set);
    let mut out = Stamp {
        digs: digs(&dungeon.layout, &looks),
        ops: Vec::new(),
        props: Vec::new(),
        lights: Vec::new(),
        mobs: Vec::new(),
        chests: Vec::new(),
        objects: Vec::new(),
        start: Vec3::ZERO,
        start_facing: 2,
        route: Vec::new(),
    };
    shell::lay(&dungeon.layout, &kinds, &mut out.ops);
    modules::lay(dungeon, theme, &looks, &kinds, &mut out);
    let map = cubes(&out.ops);
    runin::run_in(dungeon, &looks, &map, &mut out);
    out
}

/// The theme's building kinds.
pub(crate) struct Kinds {
    pub wall: Kind,
    pub floor: Kind,
    pub trim: Kind,
}

pub(crate) fn solid(kind: Kind) -> Option<Cube> {
    Some(Cube::new(kind, Fill::Solid))
}

/// An opening's cube: part of the wall, so the space stays closed, but walked through.
pub(crate) fn opening(kind: Kind) -> Option<Cube> {
    Some(Cube::new(kind, Fill::Open))
}

pub(crate) fn shaped(kind: Kind, shape: Shape) -> Option<Cube> {
    Some(Cube {
        shape,
        ..Cube::new(kind, Fill::Solid)
    })
}

/// A level's floor top, in cubes.
pub(crate) fn floor_of(level: i32) -> i32 {
    -LEVEL * (level + 1)
}

fn turn_facing(facing: Facing, turn: usize) -> Facing {
    let at = Facing::ALL.iter().position(|&f| f == facing).unwrap_or(0);
    Facing::ALL[(at + turn) % 4]
}

/// A cell seen from one of its sides: positions written as if that side were north (x east
/// 0..6, z south 0..6, y cubes above the floor top), turned a quarter clockwise a step, north
/// to east, as modules turn.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    corner: IVec3,
    turn: usize,
}

impl Frame {
    pub fn new(cell: IVec3, turn: usize) -> Frame {
        Frame {
            corner: IVec3::new(cell.x * CELL, floor_of(cell.y), cell.z * CELL),
            turn: turn % 4,
        }
    }

    /// The cube at a position of the frame.
    pub fn cube(self, x: i32, y: i32, z: i32) -> IVec3 {
        let (mut x, mut z) = (x, z);
        for _ in 0..self.turn {
            (x, z) = (CELL - 1 - z, x);
        }
        self.corner + IVec3::new(x, y, z)
    }

    /// A point of the frame, in cubes, in metres.
    pub fn point(self, x: f32, y: f32, z: f32) -> Vec3 {
        let (mut x, mut z) = (x, z);
        for _ in 0..self.turn {
            (x, z) = (CELL as f32 - z, x);
        }
        (self.corner.as_vec3() + Vec3::new(x, y, z)) * CUBE
    }

    /// The world's side for a side of the frame.
    pub fn side(self, d: usize) -> usize {
        (d + self.turn) % 4
    }

    /// Fills a block of the frame, corners inclusive, a shape turned with it.
    pub fn fill(self, ops: &mut Vec<BuildOp>, a: [i32; 3], b: [i32; 3], cube: Option<Cube>) {
        let (p, q) = (self.cube(a[0], a[1], a[2]), self.cube(b[0], b[1], b[2]));
        let turn = self.turn;
        let cube = cube.map(|c| Cube {
            shape: match c.shape {
                Shape::Stair(f) => Shape::Stair(turn_facing(f, turn)),
                Shape::Slope(f) => Shape::Slope(turn_facing(f, turn)),
                other => other,
            },
            ..c
        });
        ops.push(BuildOp::Fill {
            min: p.min(q).to_array(),
            max: p.max(q).to_array(),
            cube,
        });
    }
}

/// A hash of a cell and a salt, for picks that must repeat.
pub(crate) fn hash(at: IVec3, salt: u32) -> u32 {
    terra_gen::hash::of(&[at.x as u32, at.y as u32, at.z as u32, salt])
}

/// Every cube the fills leave, by position.
pub(crate) fn cubes(ops: &[BuildOp]) -> HashMap<IVec3, Cube> {
    let mut map = HashMap::new();
    for op in ops {
        if let BuildOp::Fill { min, max, cube } = op {
            for x in min[0]..=max[0] {
                for y in min[1]..=max[1] {
                    for z in min[2]..=max[2] {
                        let at = IVec3::new(x, y, z);
                        match cube {
                            Some(c) => map.insert(at, *c),
                            None => map.remove(&at),
                        };
                    }
                }
            }
        }
    }
    map
}

/// Air or an opening: a walker passes.
pub(crate) fn clear(map: &HashMap<IVec3, Cube>, at: IVec3) -> bool {
    map.get(&at).is_none_or(|c| c.fill == Fill::Open)
}

/// A walker stands here: a solid cube under it and four cubes (2 m) clear above.
pub(crate) fn stands(map: &HashMap<IVec3, Cube>, at: IVec3) -> bool {
    map.get(&(at - IVec3::Y))
        .is_some_and(|c| c.fill == Fill::Solid)
        && (0..4).all(|k| clear(map, at + IVec3::Y * k))
}

fn dig(min: IVec3, max: IVec3) -> Dig {
    let (min, max) = (min.as_vec3() * CUBE, max.as_vec3() * CUBE);
    Dig {
        centre: (min + max) * 0.5,
        half: (max - min) * 0.5,
    }
}

/// How far the dug ground lies under a floor's top: creatures stand on the ground, not on
/// cubes (`planet.ground_at`), so it is the floor they walk on, hidden just under the slab.
const UNDER: f32 = 0.03;

/// The cells' bounds, if any.
fn bounds<'a>(cells: impl Iterator<Item = &'a IVec3>) -> Option<(IVec3, IVec3)> {
    cells.fold(None, |b, &c| {
        Some(b.map_or((c, c), |(lo, hi): (IVec3, IVec3)| (lo.min(c), hi.max(c))))
    })
}

/// One box a level, its cells' bounds and a cube round them, from just under the floor's top
/// to the ceiling slab's top, and one a level under its basins and shores, down to just under
/// a basin's floor; and one up the surface stair to 3 m above the surface.
fn digs(layout: &Layout, looks: &[modules::Look]) -> Vec<Dig> {
    let mut out = Vec::new();
    for level in 0..layout.levels {
        let on = layout.cells.iter().filter(|c| c.at.y == level);
        let Some((lo, hi)) = bounds(on.map(|c| &c.at)) else {
            continue;
        };
        let f = floor_of(level);
        let mut level_box = dig(
            IVec3::new(lo.x * CELL - 1, f, lo.z * CELL - 1),
            IVec3::new((hi.x + 1) * CELL + 1, f + LEVEL, (hi.z + 1) * CELL + 1),
        );
        level_box.centre.y -= UNDER * 0.5;
        level_box.half.y += UNDER * 0.5;
        out.push(level_box);
        let wet = layout.cells.iter().zip(looks).filter(|(c, look)| {
            c.at.y == level && matches!(look.class, modules::Class::Basin | modules::Class::Shore)
        });
        if let Some((lo, hi)) = bounds(wet.map(|(c, _)| &c.at)) {
            let mut basin = dig(
                IVec3::new(lo.x * CELL, f - 1, lo.z * CELL),
                IVec3::new((hi.x + 1) * CELL, f, (hi.z + 1) * CELL),
            );
            basin.centre.y -= UNDER * 0.5;
            basin.half.y += UNDER * 0.5;
            out.push(basin);
        }
    }
    if let Some(stair) = layout.stairs.iter().find(|s| s.edge.is_none()) {
        let lo = stair.cells.iter().fold(IVec3::MAX, |m, &c| m.min(c));
        let hi = stair.cells.iter().fold(IVec3::MIN, |m, &c| m.max(c));
        let mut min = IVec3::new(lo.x * CELL - 1, floor_of(lo.y) - 1, lo.z * CELL - 1);
        let mut max = IVec3::new((hi.x + 1) * CELL + 1, 6, (hi.z + 1) * CELL + 1);
        // Not over the room it comes up from.
        match crate::place::opposite(stair.up) {
            0 => min.z += 1,
            1 => max.x -= 1,
            2 => max.z -= 1,
            _ => min.x += 1,
        }
        out.push(dig(min, max));
    }
    out
}
