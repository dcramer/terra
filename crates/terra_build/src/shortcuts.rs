//! The quick tools' view of a base (D176, D277): shortcuts ([`BuildOp`]) that lay built
//! cubes at once, as the yard and the examples do, and wall *tiles*, a storey's
//! column of six cubes read as a panel, solid or with a window or doorway. What a cube is,
//! wall or floor, is read from the cubes alone (`Base::in_wall`); a tile is only the quick
//! tools' default height.

use crate::{
    Base, CELL_M, Cube, DOOR_HEIGHT_M, Fill, Kind, STOREY_CUBES, STOREY_M, Side, floor_layer,
    storey_of, wall_layer,
};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;

/// What fills a wall tile, as the shortcut lays it and as it is read back.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Panel {
    Solid,
    /// Open from `sill` metres above the floor to `top` metres below the ceiling, a hole
    /// framed by its jambs, sill and lintel (D215, D225). Neighbouring tiles with the same
    /// sill and head are one window.
    Window {
        sill: f32,
        top: f32,
    },
    /// An opening to walk through, a door's height. Neighbouring tiles are one doorway;
    /// a walker needs more than one.
    Door,
}

impl Panel {
    /// A window by default (D175, D181): a sill at a metre, a head at two, level with the
    /// doors.
    pub const WINDOW: Panel = Panel::Window {
        sill: 1.0,
        top: 1.0,
    };
    /// Open from half a metre over the floor to half a metre under the ceiling.
    pub const TALL_WINDOW: Panel = Panel::Window {
        sill: 0.5,
        top: 0.5,
    };

    /// The next kind in the order a player cycles through.
    pub fn next(self) -> Panel {
        match self {
            Panel::Solid => Panel::WINDOW,
            p if p == Panel::WINDOW => Panel::TALL_WINDOW,
            Panel::Window { .. } => Panel::Door,
            Panel::Door => Panel::Solid,
        }
    }

    /// What it puts in the cube so many layers up a wall tile.
    pub fn fill(self, layer: i32) -> Fill {
        let (bottom, top) = (layer as f32 * CELL_M, (layer + 1) as f32 * CELL_M);
        let within = |from: f32, to: f32| bottom >= from - 1e-3 && top <= to + 1e-3;
        match self {
            Panel::Door if within(0.0, DOOR_HEIGHT_M) => Fill::Open,
            Panel::Window { sill, top: below } if within(sill, STOREY_M - below) => Fill::Open,
            _ => Fill::Solid,
        }
    }
}

/// Something a player did to a base with a shortcut, laying built cubes at once.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BuildOp {
    /// Fill every cube of a block, counted in cubes, or empty them with `None`.
    Fill {
        min: [i32; 3],
        max: [i32; 3],
        cube: Option<Cube>,
    },
    /// Stand a wall on a tile in a kind, as a panel says, or take it away with `None`. Its
    /// cubes are the tile's column; the top one too, unless a floor or roof is there.
    SetWall {
        tile: [i32; 3],
        panel: Option<Panel>,
        kind: Kind,
    },
    /// Lay a floor under every tile of a block, or take them away.
    SetFloor {
        min: [i32; 3],
        max: [i32; 3],
        laid: bool,
        kind: Kind,
    },
    /// Lay a flat roof over every tile of a block, or take them away.
    SetRoof {
        min: [i32; 3],
        max: [i32; 3],
        laid: bool,
        kind: Kind,
    },
    /// The room tool's shortcut, over a footprint of tiles on each of its storeys: walls
    /// round its edge, a floor inside and a flat roof over it. Where it meets rooms
    /// already there on the same storey, the walls between are taken away, so they open
    /// into one another and a room drawn across another extends it.
    AddRoom {
        min: [i32; 3],
        max: [i32; 3],
        kind: Kind,
    },
}

const LEVEL: [Side; 4] = [Side::East, Side::West, Side::South, Side::North];

impl Base {
    /// Every tile with a floor under it that is not the top of a wall below, in a fixed
    /// order: a flat roof is the floor of the storey over it.
    pub fn floors(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.cubes
            .iter()
            .filter(|(at, cube)| (at.y + 1).rem_euclid(STOREY_CUBES) == 0 && !cube.kind.is_roof())
            .map(|(at, _)| IVec3::new(at.x, storey_of(at.y + 1), at.z))
            .filter(|tile| self.filling(*tile - IVec3::Y).is_none())
    }

    /// Every wall tile and what fills it, in a fixed order: the quick tools' view of the
    /// cubes, a storey's column at its default height ([`Base::filling`]).
    pub fn walls(&self) -> impl Iterator<Item = (IVec3, Panel)> + '_ {
        let tiles: BTreeSet<[i32; 3]> = self
            .cubes
            .iter()
            .filter(|(at, cube)| {
                !cube.kind.is_roof() && at.y - wall_layer(storey_of(at.y)) < STOREY_CUBES - 1
            })
            .map(|(at, _)| [at.x, storey_of(at.y), at.z])
            .collect();
        tiles.into_iter().filter_map(|tile| {
            let tile = IVec3::from_array(tile);
            self.filling(tile).map(|panel| (tile, panel))
        })
    }

    /// Applies a shortcut, laying built cubes at once, unchecked, as the yard builds.
    /// Returns whether it changed anything.
    pub fn apply(&mut self, op: BuildOp) -> bool {
        let mut changed = false;
        match op {
            BuildOp::Fill { min, max, cube } => {
                for_block(min, max, |at| changed |= self.set(at, cube));
            }
            BuildOp::SetWall { tile, panel, kind } => {
                changed |= self.set_wall(IVec3::from_array(tile), panel, kind);
            }
            BuildOp::SetFloor {
                min,
                max,
                laid,
                kind,
            } => {
                let cube = laid.then_some(Cube::new(kind, Fill::Solid));
                for_block(min, max, |[x, storey, z]| {
                    changed |= self.set([x, floor_layer(storey), z], cube)
                });
            }
            BuildOp::SetRoof {
                min,
                max,
                laid,
                kind,
            } => {
                let cube = laid.then_some(Cube::new(kind, Fill::Solid));
                for_block(min, max, |[x, storey, z]| {
                    changed |= self.set([x, floor_layer(storey + 1), z], cube)
                });
            }
            BuildOp::AddRoom { min, max, kind } => {
                changed |= self.add_room(IVec3::from_array(min), IVec3::from_array(max), kind)
            }
        }
        if changed {
            self.settle();
        }
        changed
    }

    /// Applies shortcuts in order, as [`Base::apply`] does each, but works out the walls and
    /// rooms once after a run of fills rather than after every one: what a fill lays does
    /// not depend on them, and the roofer lays a roof a cube a fill, which made a castle's
    /// roofs cost the square of its cubes. Says which of them changed anything.
    pub fn apply_shortcuts(&mut self, ops: &[BuildOp]) -> Vec<bool> {
        let mut unsettled = false;
        let mut out = Vec::with_capacity(ops.len());
        for &op in ops {
            let changed = match op {
                BuildOp::Fill { min, max, cube } => {
                    let mut changed = false;
                    for_block(min, max, |at| changed |= self.set(at, cube));
                    unsettled |= changed;
                    changed
                }
                op => {
                    if std::mem::take(&mut unsettled) {
                        self.settle();
                    }
                    self.apply(op)
                }
            };
            out.push(changed);
        }
        if unsettled {
            self.settle();
        }
        out
    }

    /// A wall tile's cubes as a panel says, or none. The top layer is left to a floor or
    /// roof there.
    fn set_wall(&mut self, tile: IVec3, panel: Option<Panel>, kind: Kind) -> bool {
        let base = wall_layer(tile.y);
        let mut changed = false;
        for step in 0..STOREY_CUBES {
            let at = [tile.x, base + step, tile.z];
            let top = step == STOREY_CUBES - 1;
            let cube = panel.map(|panel| Cube::new(kind, panel.fill(step)));
            // The top is the wall's, unless a floor or roof is laid there and the wall is
            // only being changed.
            if top
                && self.cubes.get(IVec3::from_array(at)).is_some()
                && self.filling(tile).is_none()
            {
                continue;
            }
            changed |= self.set(at, cube);
        }
        changed
    }

    fn add_room(&mut self, min: IVec3, max: IVec3, kind: Kind) -> bool {
        // The tiles whose foot is in a room.
        let was: BTreeSet<[i32; 3]> = self
            .room_cells()
            .filter(|(at, _)| at.y == wall_layer(storey_of(at.y)))
            .map(|(at, _)| [at.x, storey_of(at.y), at.z])
            .collect();
        let was_room = |tile: IVec3| was.contains(&tile.to_array());
        let edge =
            |tile: IVec3| tile.x == min.x || tile.x == max.x || tile.z == min.z || tile.z == max.z;
        let inside = |tile: IVec3| tile.cmpge(min).all() && tile.cmple(max).all() && !edge(tile);
        let solid = Cube::new(kind, Fill::Solid);
        let mut changed = false;
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                for z in min.z..=max.z {
                    let tile = IVec3::new(x, y, z);
                    let wall = self.filling(tile).is_some();
                    // A floor under all of it, the walls too, unless the top of a wall
                    // below is there.
                    if self.cubes.get(IVec3::new(x, floor_layer(y), z)).is_none() {
                        changed |= self.set([x, floor_layer(y), z], Some(solid));
                    }
                    if inside(tile) {
                        // Inside: open, but for a wall that already stood between two rooms.
                        let between_rooms = wall
                            && LEVEL.iter().any(|side| {
                                was_room(tile + side.offset()) && was_room(tile - side.offset())
                            });
                        if wall && !between_rooms {
                            changed |= self.set_wall(tile, None, kind);
                        }
                        changed |= self.set([x, floor_layer(y + 1), z], Some(solid));
                    } else if was_room(tile) {
                        // Across a room already there: it is extended, not walled.
                    } else if wall {
                        // A wall already there between this room and another opens.
                        let opens = LEVEL.iter().any(|side| {
                            inside(tile + side.offset()) && was_room(tile - side.offset())
                        });
                        if opens {
                            changed |= self.set_wall(tile, None, kind);
                            // Roofed over, as the rooms either side are.
                            changed |= self.set([x, floor_layer(y + 1), z], Some(solid));
                        }
                    } else {
                        changed |= self.set_wall(tile, Some(Panel::Solid), kind);
                    }
                }
            }
        }
        changed
    }

    /// What fills a wall tile, read from its cubes: the quick tools' view of a storey's
    /// column at its default height, six cubes (D176, D277). `None` where no cube but a
    /// roof's stands below its top layer.
    pub fn filling(&self, tile: IVec3) -> Option<Panel> {
        let foot = wall_layer(tile.y);
        let fills: Vec<Option<Fill>> = (0..STOREY_CUBES)
            .map(|step| {
                self.cube(IVec3::new(tile.x, foot + step, tile.z))
                    .filter(|cube| step < STOREY_CUBES - 1 && !cube.kind.is_roof())
                    .map(|cube| cube.fill)
            })
            .collect();
        fills.iter().any(Option::is_some).then(|| panel_of(&fills))
    }

    /// What fills a wall tile, or solid where there is none.
    pub fn panel(&self, tile: IVec3) -> Panel {
        self.filling(tile).unwrap_or(Panel::Solid)
    }

    /// The kind of a wall tile: of its lowest cube.
    pub fn kind(&self, tile: IVec3) -> Option<Kind> {
        let base = wall_layer(tile.y);
        (base..base + STOREY_CUBES)
            .find_map(|layer| self.cube(IVec3::new(tile.x, layer, tile.z)))
            .map(|cube| cube.kind)
    }

    /// The way a wall tile's wall runs along the floor: east and west where it has wall
    /// either side that way, else north and south where it has, else east and west.
    pub fn along(&self, tile: IVec3) -> IVec3 {
        let wall = |step: IVec3| self.filling(tile + step).is_some();
        if wall(IVec3::X) || wall(IVec3::NEG_X) {
            IVec3::X
        } else if wall(IVec3::Z) || wall(IVec3::NEG_Z) {
            IVec3::Z
        } else {
            IVec3::X
        }
    }

    /// The window or doorway a wall tile is part of: every tile in a line with it, along
    /// its wall, that has the same filling, in order. Just the tile itself if it is solid,
    /// and nothing if there is no wall there.
    pub fn run(&self, tile: IVec3) -> Vec<IVec3> {
        let Some(panel) = self.filling(tile) else {
            return Vec::new();
        };
        if panel == Panel::Solid {
            return vec![tile];
        }
        let along = self.along(tile);
        let same = |at: IVec3| self.filling(at) == Some(panel);
        let mut first = tile;
        while same(first - along) {
            first -= along;
        }
        let mut found = Vec::new();
        let mut at = first;
        while same(at) {
            found.push(at);
            at += along;
        }
        found
    }

    /// The rectangle up the middle of a wall tile, along the way its wall runs, in the
    /// base's own space: low-left, low-right, high-right, high-left as seen from the side
    /// it faces, south or east.
    pub fn corners(&self, tile: IVec3) -> [Vec3; 4] {
        corners(tile, self.along(tile))
    }
}

/// The rectangle up the middle of a wall tile running along `along`, in the base's own
/// space: low-left, low-right, high-right, high-left as seen from the south or east.
pub fn corners(tile: IVec3, along: IVec3) -> [Vec3; 4] {
    let low = Vec3::new(
        tile.x as f32,
        tile.y as f32 * STOREY_CUBES as f32,
        tile.z as f32,
    ) * CELL_M;
    let middle = low + Vec3::new(CELL_M, 0.0, CELL_M) * 0.5;
    let (from, right) = if along == IVec3::X {
        (Vec3::new(low.x, low.y, middle.z), Vec3::X)
    } else {
        (Vec3::new(middle.x, low.y, low.z + CELL_M), Vec3::NEG_Z)
    };
    let up = Vec3::Y * STOREY_M;
    [
        from,
        from + right * CELL_M,
        from + right * CELL_M + up,
        from + up,
    ]
}

/// What a wall tile's cubes, from its foot up, read as.
fn panel_of(fills: &[Option<Fill>]) -> Panel {
    let layers = |fill: Fill| -> Vec<i32> {
        (0..fills.len() as i32)
            .filter(|&i| fills[i as usize] == Some(fill))
            .collect()
    };
    // Open from the floor is a doorway; open higher up, a window.
    let open = layers(Fill::Open);
    if open.first() == Some(&0) {
        return Panel::Door;
    }
    match (open.first(), open.last()) {
        (Some(&first), Some(&last)) => Panel::Window {
            sill: first as f32 * CELL_M,
            top: STOREY_M - (last + 1) as f32 * CELL_M,
        },
        _ => Panel::Solid,
    }
}

fn for_block(min: [i32; 3], max: [i32; 3], mut each: impl FnMut([i32; 3])) {
    for x in min[0]..=max[0] {
        for y in min[1]..=max[1] {
            for z in min[2]..=max[2] {
                each([x, y, z]);
            }
        }
    }
}
