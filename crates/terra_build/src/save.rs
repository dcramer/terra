//! A base as saved and sent (construction-001, gap 4): a snapshot of its sections, parts,
//! roofs, props and the ground under them, plus the operations since, folded in now and
//! then, as D21 says for terrain. And older saves, which held the list of shortcuts that
//! made a base, and before cubes walls on the edges between cells and before that rooms
//! (D152), read and turned into what is built now.

use crate::cubes::{BadSection, Cubes, Kind, PartId, Replacement, SavedSection};
use crate::ground::Sample;
use crate::{Base, BuildOp, Face, Panel, Part, Prop, Roof, RoofId, RoofSection};
use std::collections::BTreeMap;

/// Bumped when the snapshot changes in a way older ones cannot be read in.
pub const VERSION: u32 = 1;

/// A base, whole.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub sections: Vec<SavedSection>,
    pub parts: Vec<(PartId, Part)>,
    /// What each cube marked to change changes to.
    pub replacing: Vec<([i32; 3], Replacement)>,
    pub roofs: Vec<(RoofId, Roof)>,
    pub props: Vec<Prop>,
    pub ground: Vec<Sample>,
}

/// Why a snapshot could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BadSnapshot {
    Version(u32),
    Section(BadSection),
    /// A cube is in a part the snapshot does not have.
    Part(PartId),
}

impl Base {
    /// The base as a snapshot.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            version: VERSION,
            sections: self.cubes.save(),
            parts: self
                .parts
                .iter()
                .map(|(&id, part)| (id, part.clone()))
                .collect(),
            replacing: self.replacing.iter().map(|(&at, &to)| (at, to)).collect(),
            roofs: self
                .roofs
                .iter()
                .map(|(&id, roof)| (id, roof.clone()))
                .collect(),
            props: self.props.values().copied().collect(),
            ground: self.ground.samples().collect(),
        }
    }

    /// The base a snapshot holds, every section of it marked to be laid.
    pub fn from_snapshot(snapshot: &Snapshot) -> Result<Base, BadSnapshot> {
        if snapshot.version != VERSION {
            return Err(BadSnapshot::Version(snapshot.version));
        }
        let mut base = Base {
            cubes: Cubes::load(&snapshot.sections).map_err(BadSnapshot::Section)?,
            parts: snapshot.parts.iter().cloned().collect(),
            replacing: snapshot.replacing.iter().copied().collect(),
            roofs: snapshot.roofs.iter().cloned().collect(),
            props: snapshot
                .props
                .iter()
                .map(|prop| (prop.min, *prop))
                .collect(),
            ..Base::default()
        };
        for sample in &snapshot.ground {
            base.ground.set(sample.column, Some(sample.height_cm));
        }
        let mut uses: BTreeMap<PartId, u32> = BTreeMap::new();
        let used = base
            .cubes
            .iter()
            .filter_map(|(_, cube)| cube.part)
            .chain(base.replacing.values().filter_map(|&(_, part)| part));
        for part in used {
            *uses.entry(part).or_default() += 1;
        }
        for (id, count) in uses {
            let part = base.parts.get_mut(&id).ok_or(BadSnapshot::Part(id))?;
            part.uses = count;
        }
        base.settle();
        Ok(base)
    }

    /// The base the shortcuts of a save make, laid in order.
    pub fn from_shortcuts(ops: &[BuildOp]) -> Base {
        let mut base = Base::default();
        for &op in ops {
            base.apply(op);
        }
        base
    }
}

/// One change to a base, as saves before snapshots hold it: a shortcut of now, in cubes
/// (D181); or one of the operations saves held before, with walls on the edges between
/// cells, and before that rooms (D152), which are read and turned into those of now and
/// never written.
#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Clone, Copy)]
pub enum SavedOp {
    Op(BuildOp),
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

impl From<BuildOp> for SavedOp {
    fn from(op: BuildOp) -> Self {
        SavedOp::Op(op)
    }
}

/// The shortcuts of now from what was saved, laying what was saved before cubes in a
/// kind, and how many changes were left out. A wall that stood on an edge becomes the
/// tile on its outside, away from the room it closed, and a room or roof grows by a tile
/// all round, so rooms keep their size inside.
pub fn convert(saved: &[SavedOp], kind: Kind) -> (Vec<BuildOp>, usize) {
    let mut base = Base::default();
    let mut ops = Vec::new();
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
            let tile = match (base.contains(inner), base.contains(outer)) {
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
        let now = match op {
            SavedOp::Op(op) => op,
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
                BuildOp::AddRoof(section)
            }
            SavedOp::Grow(_) | SavedOp::Shrink(_) => {
                left_out += 1;
                continue;
            }
        };
        base.apply(now);
        ops.push(now);
    }
    (ops, left_out)
}

/// A save of shortcuts, of now or before, as a snapshot.
pub fn snapshot_of(saved: &[SavedOp], kind: Kind) -> Snapshot {
    Base::from_shortcuts(&convert(saved, kind).0).snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Fill, RoofStyle};
    use glam::IVec3;

    #[test]
    fn a_base_saved_before_cubes_is_read_as_cubes() {
        // As saves held it before D152: a room, a door in its outside wall, a wall inside
        // it, and a wall pushed out.
        let text = "[
            AddRoom(min: (0, 0, 0), max: (3, 0, 3)),
            SetPanel((tile: (1, 0, 3), side: South), Door),
            SetPartition((tile: (1, 0, 0), side: East), Some(Solid)),
            Grow((tile: (3, 0, 1), side: East)),
        ]";
        let saved: Vec<SavedOp> = ron::from_str(text).unwrap();
        let (ops, left_out) = convert(&saved, Kind::PLAIN);
        assert_eq!((ops.len(), left_out), (3, 1));
        let base = Base::from_shortcuts(&ops);
        // The room keeps its sixteen tiles inside, less the wall across it; its door is in
        // the tile outside the edge it was on.
        assert_eq!(base.tile_count(), 15);
        assert_eq!(base.filling(IVec3::new(1, 0, 4)), Some(Panel::Door));
        assert_eq!(base.filling(IVec3::new(1, 0, 0)), Some(Panel::Solid));
        // What is written is the shortcuts of now.
        let written = ron::to_string(&SavedOp::from(BuildOp::SetWall {
            tile: [0, 0, 0],
            panel: None,
            kind: Kind::PLAIN,
        }))
        .unwrap();
        assert!(written.starts_with("Op("), "{written}");
    }

    #[test]
    fn a_cube_saved_before_states_reads_as_built() {
        let text =
            "[Op(Fill(min: (0, 0, 0), max: (0, 0, 0), cube: Some((kind: (1), fill: Open))))]";
        let saved: Vec<SavedOp> = ron::from_str(text).unwrap();
        let base = Base::from_shortcuts(&convert(&saved, Kind::PLAIN).0);
        let cube = base.cube(IVec3::ZERO).unwrap();
        assert_eq!(cube, crate::Cube::new(Kind(1), Fill::Open));
        // And a cube of now is written as it was before, when it is as cubes were then.
        let written = ron::to_string(&cube).unwrap();
        assert_eq!(written, "(kind:(1),fill:Open)");
    }

    #[test]
    fn a_snapshot_reads_back_as_the_base_it_was_taken_of() {
        let saved = vec![
            SavedOp::Op(BuildOp::AddRoom {
                min: [0, 0, 0],
                max: [5, 0, 4],
                kind: Kind(1),
            }),
            SavedOp::Op(BuildOp::SetWall {
                tile: [2, 0, 4],
                panel: Some(Panel::Door),
                kind: Kind(1),
            }),
            SavedOp::Op(BuildOp::AddRoof(RoofSection {
                min: [0, 0, 0],
                max: [5, 0, 4],
                style: RoofStyle::Gable,
                along_x: true,
                pitch: 0.7,
            })),
        ];
        let snapshot = snapshot_of(&saved, Kind::PLAIN);
        let text = ron::to_string(&snapshot).unwrap();
        let read: Snapshot = ron::from_str(&text).unwrap();
        assert_eq!(read, snapshot);
        let base = Base::from_snapshot(&read).unwrap();
        assert_eq!(base, Base::from_shortcuts(&convert(&saved, Kind::PLAIN).0));
        assert_eq!(base.sections().len(), 1);
        assert!(!base.store().dirty().is_empty());
        let old = Snapshot {
            version: 0,
            ..snapshot
        };
        assert_eq!(Base::from_snapshot(&old), Err(BadSnapshot::Version(0)));
    }
}
