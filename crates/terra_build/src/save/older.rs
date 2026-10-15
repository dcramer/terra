//! Saves from before the snapshot of now, read and turned into what is built now: the list
//! of shortcuts that made a base, and before cubes walls on the edges between cells, and
//! before that rooms (D152); and roofs as they were before they were cubes (D209), turned
//! into roof cubes as they are read (D241, [`roofs_to_cubes`]). None of it is written.

use super::Snapshot;
use crate::cubes::{Cube, Fill, Kind, PartId};
use crate::{Base, Bill, BuildOp, Face, Panel, Part, STOREY_CUBES, Terms, wall_layer};
use glam::{IVec3, Vec2};
use std::collections::{BTreeMap, BTreeSet};

/// How a pitched roof sloped, as saves before D241 held it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RoofStyle {
    Hip,
    Gable,
    LeanTo,
}

/// A pitched roof over a block of tiles on one storey, as saves before D241 held it.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RoofSection {
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub style: RoofStyle,
    pub along_x: bool,
    pub pitch: f32,
}

/// A roof section's number, as saves before D241 held it.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct RoofId(pub u32);

/// A roof section and how far each of its cells was up, east then south, from 0 to 3, as
/// saves before D241 held it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Roof {
    pub section: RoofSection,
    pub kind: Option<Kind>,
    pub per_stage: Bill,
    pub stages: Vec<u8>,
}

impl Roof {
    /// A cell's stage, if it is under the roof.
    fn stage(&self, cell: [i32; 2]) -> Option<u8> {
        let (low, high) = (self.section.min, self.section.max);
        let inside = (low[0]..=high[0]).contains(&cell[0]) && (low[2]..=high[2]).contains(&cell[1]);
        inside.then(|| {
            let index = ((cell[0] - low[0]) * (high[2] - low[2] + 1) + cell[1] - low[2]) as usize;
            self.stages.get(index).copied().unwrap_or(0)
        })
    }

    /// Its height over a point, in cubes over its walls' tops, the point in cubes east and
    /// south: carried down from its sections' sides a cube a cube, as roof cubes slope.
    fn height(&self, p: Vec2) -> Option<f32> {
        let s = &self.section;
        let style = match s.style {
            RoofStyle::Hip => crate::roofer::Style::Hip,
            RoofStyle::Gable => crate::roofer::Style::Gable,
            RoofStyle::LeanTo => crate::roofer::Style::LeanTo,
        };
        crate::roofer::Rectangle {
            min: [s.min[0], s.min[2]],
            max: [s.max[0], s.max[2]],
        }
        .height(style, s.along_x, p)
    }
}

/// Roofs from before roofs were cubes (D209) laid as roof cubes (D241), as the roofer lays
/// them: over each storey, the roof's height the highest of its sections' at each point, a
/// cube a cube from their sides, so a wing's roof meets the main one's in a valley as it
/// did; each cube in the roof kind `roof_kind` gives for the kind its section was built in,
/// or that of most of the walls' tops under it; built where its cell's covering was on,
/// else planned and as far in as its cell was, in a part of its own whose steps cost what a
/// cell's stage did. The walls under its gable ends are carried up to it, in the kind of the
/// wall's top under each and in its part.
pub fn roofs_to_cubes(base: &mut Base, roofs: &[Roof], roof_kind: &dyn Fn(Kind) -> Kind) {
    let mut storeys: BTreeMap<i32, Vec<&Roof>> = BTreeMap::new();
    for roof in roofs {
        storeys.entry(roof.section.min[1]).or_default().push(roof);
    }
    for (storey, roofs) in storeys {
        let mut footprint = BTreeSet::new();
        for roof in &roofs {
            let (low, high) = (roof.section.min, roof.section.max);
            for x in low[0]..=high[0] {
                for z in low[2]..=high[2] {
                    footprint.insert([x, z]);
                }
            }
        }
        let height = |p: Vec2| {
            roofs
                .iter()
                .filter_map(|roof| roof.height(p))
                .fold(0.0, f32::max)
        };
        let layer = wall_layer(storey) + STOREY_CUBES;
        let plan = crate::roofer::cubes(&footprint, layer, &height);
        // Each roof's kind, and a part for each not all built.
        let kinds: Vec<Kind> = roofs
            .iter()
            .map(|roof| roof_kind(old_kind(base, roof)))
            .collect();
        let mut parts: BTreeMap<usize, PartId> = BTreeMap::new();
        for (at, shape) in plan.roof {
            let middle = Vec2::new(at.x as f32 + 0.5, at.z as f32 + 0.5);
            // The section highest over it.
            let Some(index) = (0..roofs.len())
                .filter(|&i| roofs[i].height(middle).is_some())
                .max_by(|&a, &b| {
                    let height = |i: usize| roofs[i].height(middle).unwrap_or(0.0);
                    height(a).total_cmp(&height(b))
                })
            else {
                continue;
            };
            let roof = roofs[index];
            let stage = roof.stage([at.x, at.z]).unwrap_or(0);
            let level = (u16::from(stage.min(3)) * 16 / 3) as u8;
            let cube = if stage >= 3 {
                Cube {
                    shape,
                    ..Cube::new(kinds[index], Fill::Solid)
                }
            } else {
                let next = base.next_part();
                let part = *parts.entry(index).or_insert(next);
                base.parts.entry(part).or_insert_with(|| {
                    Part::new(
                        kinds[index],
                        Terms {
                            steps: 4,
                            per_step: roof.per_stage.clone(),
                        },
                    )
                });
                Cube {
                    shape,
                    ..Cube::planned(kinds[index], Some(part))
                }
                .at_level(level.min(12))
            };
            if base.cube(at).is_none() {
                base.put(at, Some(cube));
            }
        }
        for at in plan.gables {
            // The top of the wall under the column.
            let Some(wall) = (layer - STOREY_CUBES..layer)
                .rev()
                .find_map(|y| base.cube(IVec3::new(at.x, y, at.z)))
            else {
                continue;
            };
            if base.cube(at).is_none() && !wall.kind.is_roof() {
                let cube = Cube {
                    shape: crate::Shape::Whole,
                    fill: Fill::Solid,
                    pending: crate::Pending::None,
                    ..wall
                };
                base.put(at, Some(cube));
            }
        }
    }
    base.settle();
}

/// The kind a roof from before roofs were cubes was built in: its own, or for one the
/// shortcuts put up, that of most of the walls' tops under it, the lowest of those as many.
fn old_kind(base: &Base, roof: &Roof) -> Kind {
    roof.kind
        .or_else(|| {
            let (min, max) = (roof.section.min, roof.section.max);
            let mut counts: BTreeMap<Kind, usize> = BTreeMap::new();
            for x in min[0]..=max[0] {
                for z in min[2]..=max[2] {
                    let tile = IVec3::new(x, min[1], z);
                    let top = wall_layer(tile.y);
                    if base.filling(tile).is_some()
                        && let Some(kind) = (top..top + STOREY_CUBES)
                            .rev()
                            .find_map(|layer| base.cube(IVec3::new(tile.x, layer, tile.z)))
                            .map(|cube| cube.kind)
                    {
                        *counts.entry(kind).or_default() += 1;
                    }
                }
            }
            let most = counts.values().copied().max()?;
            counts
                .into_iter()
                .find(|&(_, n)| n == most)
                .map(|(kind, _)| kind)
        })
        .unwrap_or(Kind::PLAIN)
}

/// An operation on a roof section from before roofs were cubes.
#[derive(Clone, Debug, PartialEq)]
pub enum RoofChange {
    AddRoof {
        roof: RoofId,
        section: RoofSection,
        kind: Option<Kind>,
    },
    RemoveRoof {
        roof: RoofId,
    },
    RoofStage {
        roof: RoofId,
        cells: Vec<[i32; 2]>,
        up: bool,
    },
}

/// One change to a base, as saves before snapshots hold it: a shortcut of now, in cubes
/// (D181); or one of the operations saves held before, with walls on the edges between
/// cells, and before that rooms (D152), which are read and turned into those of now and
/// never written.
#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Clone, Copy)]
pub enum SavedOp {
    Op(Shortcut),
    SetWall(Face, Option<Panel>),
    SetFloor {
        min: [i32; 3],
        max: [i32; 3],
        laid: bool,
    },
    SetRoof {
        min: [i32; 3],
        max: [i32; 3],
        laid: bool,
    },
    /// A room's inside, its walls on the edges round it.
    AddRoom {
        min: [i32; 3],
        max: [i32; 3],
    },
    /// A roof over a room's inside.
    AddRoof(RoofSection),
    /// What filled a cell of a room's outside wall.
    SetPanel(Face, Panel),
    /// A wall inside a room.
    SetPartition(Face, Option<Panel>),
    /// A room's wall pushed out or pulled in: there is no way to say which walls that
    /// meant without the old rooms, so these are left out.
    Grow(Face),
    Shrink(Face),
}

/// A shortcut as saves held it: one of now, or a pitched roof as it was laid before roofs
/// were cubes, which is read and turned into roof cubes (D241). Written as the shortcut
/// of now is.
#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Clone, Copy)]
#[serde(from = "Written", into = "Written")]
pub enum Shortcut {
    Now(BuildOp),
    /// A pitched roof over a block of tiles, put up at once.
    Then(RoofSection),
}

/// Every shortcut as it is written, those of now as [`BuildOp`] writes them, and the
/// pitched roof of before.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy)]
enum Written {
    Fill {
        min: [i32; 3],
        max: [i32; 3],
        cube: Option<Cube>,
    },
    SetWall {
        tile: [i32; 3],
        panel: Option<Panel>,
        kind: Kind,
    },
    SetFloor {
        min: [i32; 3],
        max: [i32; 3],
        laid: bool,
        kind: Kind,
    },
    SetRoof {
        min: [i32; 3],
        max: [i32; 3],
        laid: bool,
        kind: Kind,
    },
    AddRoom {
        min: [i32; 3],
        max: [i32; 3],
        kind: Kind,
    },
    AddRoof(RoofSection),
}

impl From<Written> for Shortcut {
    fn from(written: Written) -> Self {
        Shortcut::Now(match written {
            Written::Fill { min, max, cube } => BuildOp::Fill { min, max, cube },
            Written::SetWall { tile, panel, kind } => BuildOp::SetWall { tile, panel, kind },
            Written::SetFloor {
                min,
                max,
                laid,
                kind,
            } => BuildOp::SetFloor {
                min,
                max,
                laid,
                kind,
            },
            Written::SetRoof {
                min,
                max,
                laid,
                kind,
            } => BuildOp::SetRoof {
                min,
                max,
                laid,
                kind,
            },
            Written::AddRoom { min, max, kind } => BuildOp::AddRoom { min, max, kind },
            Written::AddRoof(section) => return Shortcut::Then(section),
        })
    }
}

impl From<Shortcut> for Written {
    fn from(shortcut: Shortcut) -> Self {
        match shortcut {
            Shortcut::Now(BuildOp::Fill { min, max, cube }) => Written::Fill { min, max, cube },
            Shortcut::Now(BuildOp::SetWall { tile, panel, kind }) => {
                Written::SetWall { tile, panel, kind }
            }
            Shortcut::Now(BuildOp::SetFloor {
                min,
                max,
                laid,
                kind,
            }) => Written::SetFloor {
                min,
                max,
                laid,
                kind,
            },
            Shortcut::Now(BuildOp::SetRoof {
                min,
                max,
                laid,
                kind,
            }) => Written::SetRoof {
                min,
                max,
                laid,
                kind,
            },
            Shortcut::Now(BuildOp::AddRoom { min, max, kind }) => {
                Written::AddRoom { min, max, kind }
            }
            Shortcut::Then(section) => Written::AddRoof(section),
        }
    }
}

impl From<BuildOp> for SavedOp {
    fn from(op: BuildOp) -> Self {
        SavedOp::Op(Shortcut::Now(op))
    }
}

/// What a save of shortcuts holds, as now: the shortcuts, and the roofs from before roofs
/// were cubes, which [`Converted::base`] lays as roof cubes once the shortcuts are laid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Converted {
    pub ops: Vec<BuildOp>,
    pub roofs: Vec<Roof>,
    /// How many changes were left out.
    pub left_out: usize,
}

impl Converted {
    /// The base it makes, its roofs laid in the roof kinds `roof_kind` gives, each in the
    /// place of the flat roof under it, as the shortcut put it up.
    pub fn base(&self, roof_kind: &dyn Fn(Kind) -> Kind) -> Base {
        let mut base = Base::from_shortcuts(&self.ops);
        for roof in &self.roofs {
            let (low, high) = (roof.section.min, roof.section.max);
            for x in low[0]..=high[0] {
                for z in low[2]..=high[2] {
                    let tile = IVec3::new(x, low[1], z);
                    if base.filling(tile).is_none() {
                        let at = [x, crate::floor_layer(low[1] + 1), z];
                        base.apply(BuildOp::Fill {
                            min: at,
                            max: at,
                            cube: None,
                        });
                    }
                }
            }
        }
        roofs_to_cubes(&mut base, &self.roofs, roof_kind);
        base
    }
}

/// The shortcuts of now from what was saved, laying what was saved before cubes in a
/// kind, and the roofs from before roofs were cubes. A wall that stood on an edge becomes
/// the tile on its outside, away from the room it closed, and a room or roof grows by a
/// tile all round, so rooms keep their size inside.
pub fn convert(saved: &[SavedOp], kind: Kind) -> Converted {
    let mut base = Base::default();
    let mut ops = Vec::new();
    let mut roofs = Vec::new();
    let mut left_out = 0;
    let grow = |min: [i32; 3], max: [i32; 3]| {
        (
            [min[0] - 1, min[1], min[2] - 1],
            [max[0] + 1, max[1], max[2] + 1],
        )
    };
    for &op in saved {
        let wall = |base: &Base, face: Face, panel: Option<Panel>| {
            let (inner, outer) = (face.tile(), face.tile() + face.side.offset());
            // In a room: the air cube at the foot of the tile's storey (D261).
            let roomed = |tile: IVec3| base.contains(tile.with_y(crate::wall_layer(tile.y)));
            let tile = match (roomed(inner), roomed(outer)) {
                (true, false) => outer,
                (false, true) => inner,
                _ if base.filling(outer).is_some() && base.filling(inner).is_none() => outer,
                _ => inner,
            };
            BuildOp::SetWall {
                tile: tile.to_array(),
                panel,
                kind,
            }
        };
        let roof = |section: RoofSection| Roof {
            section,
            kind: None,
            per_stage: Bill::default(),
            stages: vec![3; rows(&section)],
        };
        let now = match op {
            SavedOp::Op(Shortcut::Now(op)) => op,
            SavedOp::Op(Shortcut::Then(section)) => {
                roofs.push(roof(section));
                continue;
            }
            SavedOp::SetWall(face, panel) | SavedOp::SetPartition(face, panel) => {
                wall(&base, face, panel)
            }
            SavedOp::SetPanel(face, panel) => wall(&base, face, Some(panel)),
            SavedOp::SetFloor { min, max, laid } => BuildOp::SetFloor {
                min,
                max,
                laid,
                kind,
            },
            SavedOp::SetRoof { min, max, laid } => BuildOp::SetRoof {
                min,
                max,
                laid,
                kind,
            },
            SavedOp::AddRoom { min, max } => {
                let (min, max) = grow(min, max);
                BuildOp::AddRoom { min, max, kind }
            }
            SavedOp::AddRoof(mut section) => {
                (section.min, section.max) = grow(section.min, section.max);
                roofs.push(roof(section));
                continue;
            }
            SavedOp::Grow(_) | SavedOp::Shrink(_) => {
                left_out += 1;
                continue;
            }
        };
        base.apply(now);
        ops.push(now);
    }
    Converted {
        ops,
        roofs,
        left_out,
    }
}

/// How many cells a roof section covers.
pub(super) fn rows(section: &RoofSection) -> usize {
    let (low, high) = (section.min, section.max);
    ((high[0] - low[0] + 1) * (high[2] - low[2] + 1)).max(0) as usize
}

/// A save of shortcuts, of now or before, as a snapshot, its roofs in the first roof kind.
pub fn snapshot_of(saved: &[SavedOp], kind: Kind) -> Snapshot {
    convert(saved, kind).base(&|_| Kind::FIRST_ROOF).snapshot()
}
