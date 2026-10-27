//! Stage 5, the stamp: a dressed dungeon laid in building's half-metre cubes (D181), with what
//! the game spawns in it (props, lights, mobs, chests, the goal, keys and door leaves), where the
//! player starts and a tour through it. Plain data: the sandbox digs the boxes, applies the ops
//! and spawns the rest.
//!
//! The frame: metres, x east, y up, z south, from the north-west corner of cell (0, 0, 0) at
//! y 0, the top of the roof and of the stair up to the surface. A cell is 8 by 8 cubes, 4 m,
//! so a corridor is 3 m clear between walls a cube thick; level L (cell.y, counted down) has
//! its floor top at cube height -12 (L + 1), 6 m apart (D527), its floor slab the layer under
//! that and its ceiling slab 11 layers above (the next level's floor), 5.5 m clear, brought
//! down to the theme's headroom over its corridors and rooms (`Theme::headroom`).
//!
//! - `shell`: slabs, walls, doorways, the stairs and the surface stair's parapet and arch.
//! - `modules`: what each module of the dressing becomes in cubes and props, and the features.
//! - `runin`: door leaves, chests, mobs, torches, and the tour.
//! - `view`: the camera's way along the tour, kept off what stands round it.
//! - `look`: where the camera looks along it.

#[cfg(test)]
mod doorway_tests;
mod kit;
#[cfg(test)]
mod leaf_tests;
mod look;
#[cfg(test)]
mod look_tests;
mod modules;
mod runin;
mod shell;
#[cfg(test)]
mod tests;
mod view;

use crate::Dungeon;
use crate::dress::ModuleSet;
use crate::place::{Layout, Space};
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
/// Cubes across a cell: 4 m (D527).
pub(crate) const CELL: i32 = 8;
/// Cubes from one level's floor to the next: 6 m (D527).
pub(crate) const LEVEL: i32 = 12;
/// A cell's last row of cubes, where its far wall stands.
pub(crate) const EDGE: i32 = CELL - 1;
/// A walled cell's last clear row inside its walls.
pub(crate) const INNER: i32 = CELL - 2;
/// A cell's middle, in cubes from its corner.
pub(crate) const MID: f32 = CELL as f32 * 0.5;
// A stair climbs a level in its two cells: two flat columns at each end and a cube a step.
const _: () = assert!(2 * CELL - 4 == LEVEL);
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

/// A piece of the theme's dressed kit (D527): a vault, an arch, a pier, a niche's front,
/// placed by its model's origin and stretched along its own axes before it is turned.
#[derive(Clone, Debug)]
pub struct Piece {
    /// From the asset root: `models/props/barrow/BarrelVault.glb`.
    pub model: String,
    pub at: Vec3,
    /// The side its front (+Z) faces, 0..4 = north, east, south, west.
    pub facing: usize,
    pub scale: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKind {
    /// On a wall at 2 m.
    Torch,
    /// A key feature to light.
    Brazier,
    /// The rest room's fire, on the floor.
    Campfire,
    /// A fire bowl on a pillar's top, where `at` is: the goal's pair.
    Bowl,
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
    /// What a door asks before it is pushed open (D512); `None` for everything else.
    pub lock: crate::door::Lock,
    /// How far a door's leaves swing into its room, in quarter turns (`door::leaf`), the
    /// first hinged at the doorway's -X: two where the wall runs on a metre past the jamb,
    /// so the leaf folds flat on it, one where a side wall stands flush with the jamb, minus
    /// one where the room is solid just inside the jamb, so it swings out whichever way.
    pub opens: [f32; 2],
}

/// A dungeon laid out for the game.
#[derive(Clone, Debug)]
pub struct Stamp {
    /// One box a level and one for the surface stair.
    pub digs: Vec<Dig>,
    /// In cubes, applied in order (a later fill overwrites an earlier one).
    pub ops: Vec<BuildOp>,
    pub props: Vec<Prop>,
    /// The theme's kit over the cubes; none for a theme without one.
    pub pieces: Vec<Piece>,
    pub lights: Vec<Light>,
    pub mobs: Vec<MobSpawn>,
    pub chests: Vec<ChestSpawn>,
    pub objects: Vec<Object>,
    /// On the surface at the top of the stair down.
    pub start: Vec3,
    pub start_facing: usize,
    /// A tour a camera can follow: down the stair and by the longest way that enters no room
    /// twice to the goal, so it never turns round in a dead end.
    pub route: Vec<Vec3>,
    /// The middles of the doorways the tour passes, at the floor, in its order: the look
    /// leads to the next once it is in sight (`look::LEAD_M`).
    pub doorways: Vec<Vec3>,
    /// A camera's eye along the tour every `view::STEP_M`: `view::ROOM_M` from walls, pillars
    /// and open leaves in plan, as high as `view::EYE_M` allows with that room all round
    /// (`view::TIGHT_M` at the least, under a stair's slab), smoothed.
    pub view: Vec<Vec3>,
    /// Where the camera looks from each eye of `view`: a point `look::LOOK_M` out along its
    /// look, at the way `look::AHEAD_M` on, or drawn to a room's goal or key once near,
    /// turning at most `look::TURN_PER_M` but where the way ahead turns faster.
    pub look: Vec<Vec3>,
    /// The seconds a camera's dolly takes from each eye of `view` to the next, nought at the
    /// last: `look::DOLLY_M_S`, slowed so the look turns at most `look::TURN_MOST_PER_S`.
    pub pace: Vec<f32>,
}

/// Lays a dungeon in cubes; `kind` names the building kinds of the theme's walls, floors,
/// ceilings and trim.
pub fn stamp(dungeon: &Dungeon, theme: &Theme, kind: &dyn Fn(&str) -> Kind) -> Stamp {
    let set = ModuleSet::new(theme).expect("the dungeon was dressed from these modules");
    let kinds = Kinds {
        wall: kind(&theme.walls),
        floor: kind(&theme.floors),
        ceiling: kind(theme.ceilings.as_ref().unwrap_or(&theme.floors)),
        trim: kind(&theme.trim),
        head: [head_of(theme.headroom.0), head_of(theme.headroom.1)],
    };
    let looks = modules::looks(dungeon, theme, &set);
    let mut out = Stamp {
        digs: digs(&dungeon.layout, &looks),
        ops: Vec::new(),
        props: Vec::new(),
        pieces: Vec::new(),
        lights: Vec::new(),
        mobs: Vec::new(),
        chests: Vec::new(),
        objects: Vec::new(),
        start: Vec3::ZERO,
        start_facing: 2,
        route: Vec::new(),
        doorways: Vec::new(),
        view: Vec::new(),
        look: Vec::new(),
        pace: Vec::new(),
    };
    shell::lay(&dungeon.layout, &kinds, &mut out.ops);
    modules::lay(dungeon, theme, &looks, &kinds, &mut out);
    // Before the cubes are read: a pier stands on its own column of cubes.
    if let Some(kit) = &theme.kit {
        kit::lay(dungeon, kit, &kinds, &mut out);
    }
    let map = cubes(&out.ops);
    runin::run_in(dungeon, &looks, &map, theme.kit.is_some(), &mut out);
    out
}

/// The theme's building kinds.
pub(crate) struct Kinds {
    pub wall: Kind,
    pub floor: Kind,
    /// A ceiling no room stands on.
    pub ceiling: Kind,
    pub trim: Kind,
    /// The cubes clear over a corridor's floor and over a room's (`Theme::headroom`): the
    /// ceiling's underside.
    pub head: [i32; 2],
}

impl Kinds {
    /// The cubes clear over a cell's floor: its ceiling's underside; a stair's well is open
    /// to the slab.
    pub fn head(&self, space: Space) -> i32 {
        match space {
            Space::Corridor(_) => self.head[0],
            Space::Room(_) => self.head[1],
            Space::Stair(_) => LEVEL - 1,
        }
    }
}

/// A headroom in metres as cubes clear: above a doorway's lintel, under the slab.
fn head_of(metres: f32) -> i32 {
    ((metres / CUBE).round() as i32).clamp(shell::DOORWAY[1] + 1, LEVEL - 1)
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
/// 0..CELL, z south 0..CELL, y cubes above the floor top), turned a quarter clockwise a step, north
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

/// Where a walker standing at `at` can step: a cube along each way, up one or down two, with
/// their head clear on the way.
pub(crate) fn steps(map: &HashMap<IVec3, Cube>, at: IVec3) -> impl Iterator<Item = IVec3> + '_ {
    crate::place::STEPS.into_iter().flat_map(move |step| {
        [1, 0, -1, -2].into_iter().filter_map(move |dy| {
            let n = at + step + IVec3::Y * dy;
            let head = if dy > 0 {
                clear(map, at + IVec3::Y * 4)
            } else {
                (n.y + 4..=at.y + 3).all(|y| clear(map, IVec3::new(n.x, y, n.z)))
            };
            (stands(map, n) && head).then_some(n)
        })
    })
}

/// The cube a point in metres is in.
pub(crate) fn cube_of(p: Vec3) -> IVec3 {
    (p / CUBE).floor().as_ivec3()
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
/// a basin's floor; and one up the surface stair to the top of its arch.
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
        let arch = shell::DOORWAY[1] + 1;
        let mut max = IVec3::new((hi.x + 1) * CELL + 1, arch, (hi.z + 1) * CELL + 1);
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
