//! A base as saved and sent (construction-001, gap 4): a snapshot of its sections, parts,
//! props and the ground under them, plus the operations since, folded in now and then, as
//! D21 says for terrain. Older saves are read and turned into what is built now
//! (`older`).

use crate::cubes::{BadSection, Cubes, Kind, PartId, Replacement, SavedSection};
use crate::ground::Sample;
use crate::{Base, Bill, BuildOp, Part, Prop};
use std::collections::BTreeMap;

mod older;
use older::rows;
pub use older::{
    Converted, Roof, RoofChange, RoofId, RoofSection, RoofStyle, SavedOp, Shortcut, convert,
    roofs_to_cubes, snapshot_of,
};

/// Bumped when the snapshot changes in a way older ones cannot be read in: 2 when a cube's
/// shape took five bits (D231), whose older snapshots are converted as they are read.
pub const VERSION: u32 = 2;

/// A base, whole.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub sections: Vec<SavedSection>,
    pub parts: Vec<(PartId, Part)>,
    /// What each cube marked to change changes to.
    pub replacing: Vec<([i32; 3], Replacement)>,
    /// Roofs as snapshots held them before roofs were cubes, turned into roof cubes as they
    /// are read (D241); none are written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roofs: Vec<(RoofId, Roof)>,
    pub props: Vec<Prop>,
    /// What was paid for each prop, by its lowest cube.
    #[serde(default)]
    pub prop_paid: Vec<([i32; 3], crate::Bill)>,
    pub ground: Vec<Sample>,
    /// How much damage each damaged cube has taken (`loss`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub damage: Vec<([i32; 3], u16)>,
}

/// Why a snapshot could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BadSnapshot {
    Version(u32),
    Section(BadSection),
    /// A cube is in a part the snapshot does not have.
    Part(PartId),
    /// What a part, a roof or a prop cost is an amount no cost could be
    /// ([`crate::Amount::in_bounds`]).
    Amount(crate::Amount),
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
            roofs: Vec::new(),
            props: self.props.values().copied().collect(),
            prop_paid: self
                .prop_paid
                .iter()
                .map(|(&at, paid)| (at, paid.clone()))
                .collect(),
            ground: self.ground.samples().collect(),
            damage: self
                .damage
                .iter()
                .map(|(&at, &damage)| (at, damage))
                .collect(),
        }
    }

    /// The base a snapshot holds, every section of it marked to be laid, its roofs from
    /// before roofs were cubes laid in the first roof kind.
    pub fn from_snapshot(snapshot: &Snapshot) -> Result<Base, BadSnapshot> {
        Base::from_snapshot_with(snapshot, &|_| Kind::FIRST_ROOF)
    }

    /// As [`Base::from_snapshot`], each roof from before roofs were cubes laid in the roof
    /// kind `roof_kind` gives for the kind it was built in, or the walls' under it.
    pub fn from_snapshot_with(
        snapshot: &Snapshot,
        roof_kind: &dyn Fn(Kind) -> Kind,
    ) -> Result<Base, BadSnapshot> {
        // A snapshot from before five shape bits, its cubes repacked.
        let converted: Vec<SavedSection>;
        let sections = match snapshot.version {
            VERSION => &snapshot.sections,
            1 => {
                converted = snapshot
                    .sections
                    .iter()
                    .map(SavedSection::from_v1)
                    .collect();
                &converted
            }
            other => return Err(BadSnapshot::Version(other)),
        };
        // What it says things cost arrives over the network: an amount past what any real
        // cost comes to, where working with it could overflow, is refused.
        let bills = (snapshot.parts.iter().map(|(_, part)| &part.terms.per_step))
            .chain(snapshot.roofs.iter().map(|(_, roof)| &roof.per_stage))
            .chain(snapshot.prop_paid.iter().map(|(_, paid)| paid));
        if let Some(amount) = bills.filter_map(crate::Bill::out_of_bounds).next() {
            return Err(BadSnapshot::Amount(amount));
        }
        let mut base = Base {
            cubes: Cubes::load(sections).map_err(BadSnapshot::Section)?,
            parts: snapshot.parts.iter().cloned().collect(),
            replacing: snapshot.replacing.iter().copied().collect(),
            props: snapshot
                .props
                .iter()
                .map(|prop| (prop.min, *prop))
                .collect(),
            prop_paid: snapshot.prop_paid.iter().cloned().collect(),
            damage: snapshot
                .damage
                .iter()
                .copied()
                .filter(|&(_, damage)| damage > 0)
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
        base.count_columns();
        base.settle();
        let roofs: Vec<Roof> = snapshot
            .roofs
            .iter()
            .map(|(_, roof)| roof.clone())
            .collect();
        roofs_to_cubes(&mut base, &roofs, roof_kind);
        Ok(base)
    }

    /// The base the shortcuts of a save make, laid in order.
    pub fn from_shortcuts(ops: &[BuildOp]) -> Base {
        let mut base = Base::default();
        base.apply_shortcuts(ops);
        base
    }
}

/// An operation done since a snapshot, as saves hold them: one of now, or one of a roof's
/// from before roofs were cubes (D209), read and turned into roof cubes ([`restore`]).
/// Written as the operation of now is.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(from = "WrittenChange", into = "WrittenChange")]
pub enum Change {
    Now(crate::ops::Op),
    Roof(RoofChange),
}

impl From<crate::ops::Op> for Change {
    fn from(op: crate::ops::Op) -> Self {
        Change::Now(op)
    }
}

/// Every operation as it is written, those of now as [`crate::ops::Op`] writes them, and a
/// roof section's of before.
macro_rules! written_changes {
    ($($name:ident { $($field:ident: $ty:ty),* }),* ; $($tuple:ident($inner:ty)),*) => {
        #[derive(serde::Serialize, serde::Deserialize)]
        enum WrittenChange {
            $($name { $($field: $ty),* },)*
            $($tuple($inner),)*
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

        impl From<WrittenChange> for Change {
            fn from(written: WrittenChange) -> Self {
                use crate::ops::Op;
                match written {
                    $(WrittenChange::$name { $($field),* } => Change::Now(Op::$name { $($field),* }),)*
                    $(WrittenChange::$tuple(inner) => Change::Now(Op::$tuple(inner)),)*
                    WrittenChange::AddRoof { roof, section, kind } => {
                        Change::Roof(RoofChange::AddRoof { roof, section, kind })
                    }
                    WrittenChange::RemoveRoof { roof } => {
                        Change::Roof(RoofChange::RemoveRoof { roof })
                    }
                    WrittenChange::RoofStage { roof, cells, up } => {
                        Change::Roof(RoofChange::RoofStage { roof, cells, up })
                    }
                }
            }
        }

        impl From<Change> for WrittenChange {
            fn from(change: Change) -> Self {
                use crate::ops::Op;
                match change {
                    $(Change::Now(Op::$name { $($field),* }) => WrittenChange::$name { $($field),* },)*
                    $(Change::Now(Op::$tuple(inner)) => WrittenChange::$tuple(inner),)*
                    Change::Roof(RoofChange::AddRoof { roof, section, kind }) => {
                        WrittenChange::AddRoof { roof, section, kind }
                    }
                    Change::Roof(RoofChange::RemoveRoof { roof }) => {
                        WrittenChange::RemoveRoof { roof }
                    }
                    Change::Roof(RoofChange::RoofStage { roof, cells, up }) => {
                        WrittenChange::RoofStage { roof, cells, up }
                    }
                }
            }
        }
    };
}

written_changes!(
    LayPlan {
        part: Option<PartId>,
        kind: Kind,
        cubes: Vec<crate::ops::Planned>,
        ground: Vec<Sample>
    },
    Unplan { cubes: Vec<[i32; 3]> },
    Fill { cubes: Vec<[i32; 3]> },
    Empty { cubes: Vec<[i32; 3]> },
    SetShape {
        cubes: Vec<[i32; 3]>,
        shape: crate::Shape
    },
    Replace {
        cubes: Vec<[i32; 3]>,
        kind: Kind,
        part: Option<PartId>
    },
    Swap { cubes: Vec<[i32; 3]> },
    Changed {
        cubes: Vec<[i32; 3]>,
        kind: Kind,
        part: Option<PartId>
    },
    Carve { cubes: Vec<[i32; 3]> },
    Cancel { cubes: Vec<[i32; 3]> },
    RemoveProp { at: [i32; 3] },
    SetOpen { at: [i32; 3], open: bool },
    Open { cubes: Vec<[i32; 3]> },
    Close { cubes: Vec<[i32; 3]> },
    Ground { samples: Vec<Sample> },
    Regroup {
        cubes: Vec<[i32; 3]>,
        part: PartId
    },
    Damage {
        cubes: Vec<[i32; 3]>,
        amount: u16
    },
    Repair { cubes: Vec<[i32; 3]> },
    Subside { samples: Vec<Sample> };
    FitProp(Prop),
    Restore(Box<crate::loss::Patch>)
);

/// A base restored from a snapshot and the changes saved since, as a saved game is loaded:
/// each operation of now applied in order, refused ones left out and said, and the roofs
/// from before roofs were cubes, the snapshot's and those the changes since put up, taken
/// down or staged, laid as roof cubes at the end in the kinds `roof_kind` gives
/// ([`roofs_to_cubes`]). The operations applied, and why any were left out.
pub fn restore(
    snapshot: &Snapshot,
    since: &[Change],
    cx: &crate::ops::Context,
    roof_kind: &dyn Fn(Kind) -> Kind,
) -> Result<(Base, Vec<crate::ops::Op>, Vec<String>), BadSnapshot> {
    let mut base = Base::from_snapshot_with(
        &Snapshot {
            roofs: Vec::new(),
            ..snapshot.clone()
        },
        roof_kind,
    )?;
    let mut roofs: BTreeMap<RoofId, Roof> = snapshot.roofs.iter().cloned().collect();
    let (mut applied, mut left_out) = (Vec::new(), Vec::new());
    for change in since {
        match change {
            Change::Now(op) => match base.act(op, cx) {
                Ok(_) => applied.push(op.clone()),
                Err(why) => left_out.push(format!("{op:?}: {why:?}")),
            },
            Change::Roof(RoofChange::AddRoof {
                roof,
                section,
                kind,
            }) => {
                let cells = rows(section);
                roofs.insert(
                    *roof,
                    Roof {
                        section: *section,
                        kind: *kind,
                        per_stage: Bill::default(),
                        stages: vec![0; cells],
                    },
                );
            }
            Change::Roof(RoofChange::RemoveRoof { roof }) => {
                roofs.remove(roof);
            }
            Change::Roof(RoofChange::RoofStage { roof, cells, up }) => {
                let Some(roof) = roofs.get_mut(roof) else {
                    continue;
                };
                let (low, high) = (roof.section.min, roof.section.max);
                for &[x, z] in cells {
                    if !(low[0]..=high[0]).contains(&x) || !(low[2]..=high[2]).contains(&z) {
                        continue;
                    }
                    let index = ((x - low[0]) * (high[2] - low[2] + 1) + z - low[2]) as usize;
                    if let Some(stage) = roof.stages.get_mut(index) {
                        *stage = if *up {
                            (*stage + 1).min(3)
                        } else {
                            stage.saturating_sub(1)
                        };
                    }
                }
            }
        }
    }
    let roofs: Vec<Roof> = roofs.into_values().collect();
    roofs_to_cubes(&mut base, &roofs, roof_kind);
    Ok((base, applied, left_out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Panel;
    use crate::Shape;
    use crate::cubes::Fill;
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
        let converted = convert(&saved, Kind::PLAIN);
        assert_eq!((converted.ops.len(), converted.left_out), (3, 1));
        let base = Base::from_shortcuts(&converted.ops);
        // The room keeps its sixteen tiles inside, less the wall across it; its door is in
        // the tile outside the edge it was on.
        assert_eq!(base.room_cubes(), 15 * 5);
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
        let base = Base::from_shortcuts(&convert(&saved, Kind::PLAIN).ops);
        let cube = base.cube(IVec3::ZERO).unwrap();
        assert_eq!(cube, crate::Cube::new(Kind(1), Fill::Open));
        // And a cube of now is written as it was before, when it is as cubes were then.
        let written = ron::to_string(&cube).unwrap();
        assert_eq!(written, "(kind:(1),fill:Open)");
    }

    #[test]
    fn a_snapshot_reads_back_as_the_base_it_was_taken_of() {
        let saved = vec![
            SavedOp::from(BuildOp::AddRoom {
                min: [0, 0, 0],
                max: [5, 0, 4],
                kind: Kind(1),
            }),
            SavedOp::from(BuildOp::SetWall {
                tile: [2, 0, 4],
                panel: Some(Panel::Door),
                kind: Kind(1),
            }),
        ];
        let snapshot = snapshot_of(&saved, Kind::PLAIN);
        let text = ron::to_string(&snapshot).unwrap();
        assert!(!text.contains("roofs"), "none are written");
        let read: Snapshot = ron::from_str(&text).unwrap();
        assert_eq!(read, snapshot);
        let base = Base::from_snapshot(&read).unwrap();
        assert_eq!(
            base,
            convert(&saved, Kind::PLAIN).base(&|_| Kind::FIRST_ROOF)
        );
        assert!(!base.store().dirty().is_empty());
        let old = Snapshot {
            version: 0,
            ..snapshot
        };
        assert_eq!(Base::from_snapshot(&old), Err(BadSnapshot::Version(0)));
    }

    /// The roof kind of a test's logs.
    const SHAKES: Kind = Kind(0x800);
    const SLATE: Kind = Kind(0x801);

    fn roof_of(kind: Kind) -> Kind {
        if kind == Kind(2) { SLATE } else { SHAKES }
    }

    #[test]
    fn roof_sections_saved_before_roofs_were_cubes_are_read_as_roof_cubes() {
        // As a snapshot held them before D241: a log room with a gable over it, all of it
        // up; and a stone room beside it under a hip with only its frame on.
        let rules = crate::Rules::default();
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind: Kind(1),
        });
        base.apply(BuildOp::AddRoom {
            min: [20, 0, 0],
            max: [29, 0, 7],
            kind: Kind(2),
        });
        for min in [[1, 5, 1], [21, 5, 1]] {
            base.apply(BuildOp::Fill {
                min,
                max: [min[0] + 7, 5, 6],
                cube: None,
            });
        }
        let section = |x: i32, style: RoofStyle| RoofSection {
            min: [x, 0, 0],
            max: [x + 9, 0, 7],
            style,
            along_x: true,
            pitch: 0.7,
        };
        let mut snapshot = base.snapshot();
        let text = ron::to_string(&snapshot).unwrap();
        // The roofs as they were written then.
        let roofs = "roofs:[((0),(section:(min:(0,0,0),max:(9,0,7),style:Gable,along_x:true,pitch:0.7),kind:None,per_stage:({}),stages:[STAGES])),((1),(section:(min:(20,0,0),max:(29,0,7),style:Hip,along_x:true,pitch:0.7),kind:Some((2)),per_stage:({\"Wood\":(1,32)}),stages:[FRAMES]))],";
        let roofs = roofs
            .replace("STAGES", &vec!["3"; 80].join(","))
            .replace("FRAMES", &vec!["1"; 80].join(","));
        let old = text.replacen("props:", &format!("{roofs}props:"), 1);
        let read: Snapshot = ron::from_str(&old).unwrap_or_else(|why| panic!("{why}: {old}"));
        assert_eq!(read.roofs.len(), 2);
        assert_eq!(read.roofs[0].1.section, section(0, RoofStyle::Gable));
        snapshot.roofs = read.roofs.clone();
        let base = Base::from_snapshot_with(&read, &roof_of).unwrap();
        // The gable: slopes up from the eaves to the ridge two metres in, of shakes, built.
        let cube = |x, y, z| base.cube(IVec3::new(x, y, z)).unwrap();
        assert_eq!(cube(4, 6, 0).shape, Shape::Slope(crate::Facing::South));
        assert_eq!(cube(4, 9, 3).shape, Shape::Slope(crate::Facing::South));
        assert_eq!(cube(4, 9, 4).shape, Shape::Slope(crate::Facing::North));
        assert!(cube(4, 9, 3).kind == SHAKES && cube(4, 9, 3).is_built());
        // Its gable ends of logs, carried up to it.
        assert_eq!(cube(0, 8, 3).kind, Kind(1));
        // The hip of slate, planned, its frame in, in a part whose steps cost a stage's.
        let hip = cube(24, 6, 0);
        assert_eq!(hip.kind, SLATE);
        assert_eq!(hip.shape, Shape::Slope(crate::Facing::South));
        assert!(!hip.is_built() && hip.level() == 5);
        let part = base.part(hip.part.unwrap()).unwrap();
        assert_eq!(part.terms.steps, 4);
        assert_eq!(format!("{}", part.terms.per_step), "1/32 Wood");
        assert_eq!(
            cube(20, 6, 0).shape,
            Shape::OuterCorner(crate::Facing::East)
        );
        // No gables under a hip, and nothing built floats.
        assert!(base.cube(IVec3::new(20, 7, 3)).is_none());
        let built = base.view(|cube| cube.is_built());
        assert!(built.floating().is_empty(), "{:?}", built.floating());
        // The roof is the roofer's gable over the same footprint.
        let footprint = (0..10).flat_map(|x| (0..8).map(move |z| [x, z])).collect();
        let gable = crate::roofer::over(&footprint, 6, crate::roofer::Style::Gable, None);
        for (at, shape) in gable.roof {
            assert_eq!(base.cube(at).map(|c| c.shape), Some(shape), "{at}");
        }
        // Read again and written, it is cubes alone.
        assert!(base.snapshot().roofs.is_empty());
        let _ = rules;
    }

    #[test]
    fn a_roof_put_up_and_staged_since_a_snapshot_before_roofs_were_cubes_is_read_as_cubes() {
        // A log room, snapshotted; then, as saved since: a cube laid, a roof planned over
        // it and its frame put up, and another planned and taken away.
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind: Kind(1),
        });
        base.apply(BuildOp::Fill {
            min: [1, 5, 1],
            max: [8, 5, 6],
            cube: None,
        });
        let snapshot = base.snapshot();
        let cells: Vec<String> = (0..10)
            .flat_map(|x| (0..8).map(move |z| format!("({x},{z})")))
            .collect();
        let text = format!(
            "[LayPlan(part:Some((3)),kind:(1),cubes:[(at:(20,0,0),shape:Whole)],ground:[]),\
             AddRoof(roof:(0),section:(min:(0,0,0),max:(9,0,7),style:Gable,along_x:true,pitch:0.7),kind:None),\
             RoofStage(roof:(0),cells:[{}],up:true),\
             AddRoof(roof:(1),section:(min:(30,0,0),max:(31,0,1),style:Hip,along_x:true,pitch:0.7),kind:None),\
             RemoveRoof(roof:(1))]",
            cells.join(",")
        );
        let since: Vec<Change> = ron::from_str(&text).unwrap_or_else(|why| panic!("{why}"));
        assert!(matches!(since[0], Change::Now(_)));
        let rules = crate::Rules::default();
        let cx = crate::ops::Context::free(&rules);
        let (base, applied, left_out) = restore(&snapshot, &since, &cx, &roof_of).unwrap();
        assert_eq!((applied.len(), left_out.len()), (1, 0));
        // The gable's cubes, its frame in.
        let eave = base.cube(IVec3::new(4, 6, 0)).unwrap();
        assert_eq!((eave.kind, eave.level()), (SHAKES, 5));
        assert!(base.cube(IVec3::new(30, 6, 0)).is_none());
        // An operation of now is written as it always was.
        let written = ron::to_string(&Change::from(crate::ops::Op::Fill {
            cubes: vec![[1, 2, 3]],
        }))
        .unwrap();
        assert_eq!(written, "Fill(cubes:[(1,2,3)])");
    }

    #[test]
    fn a_roof_shortcut_saved_before_roofs_were_cubes_is_read_as_roof_cubes() {
        let text = "[
            Op(AddRoom(min: (0, 0, 0), max: (9, 0, 7), kind: (1))),
            Op(AddRoof((min: (0, 0, 0), max: (9, 0, 7), style: Hip, along_x: true, pitch: 0.7))),
        ]";
        let saved: Vec<SavedOp> = ron::from_str(text).unwrap();
        let converted = convert(&saved, Kind::PLAIN);
        assert_eq!((converted.ops.len(), converted.roofs.len()), (1, 1));
        let base = converted.base(&roof_of);
        assert_eq!(
            base.cube(IVec3::new(0, 6, 0)).map(|c| (c.kind, c.shape)),
            Some((SHAKES, Shape::OuterCorner(crate::Facing::East)))
        );
        // In the place of the flat roof the room had.
        assert!(base.cube(IVec3::new(4, 5, 4)).is_none());
        assert!(base.floating().is_empty());
    }

    #[test]
    fn a_snapshot_from_before_five_shape_bits_is_converted() {
        use crate::ops::{Context, Op, Planned};
        use crate::{Cube, Facing, Shape};
        let rules = crate::Rules::default();
        let mut base = Base::default();
        // A stair, a slope, a half and a post planned, a stone cube built over a carving,
        // an opening built and one planned: everything a snapshot could hold before.
        let shapes = [
            Shape::Stair(Facing::West),
            Shape::Slope(Facing::South),
            Shape::Half,
            Shape::Post,
        ];
        let cubes = shapes
            .iter()
            .enumerate()
            .map(|(i, &shape)| Planned {
                at: [i as i32, 0, 0],
                shape,
            })
            .collect();
        let plan = Op::LayPlan {
            part: Some(PartId(4)),
            kind: Kind(1),
            cubes,
            ground: Vec::new(),
        };
        base.act(&plan, &Context::free(&rules)).unwrap();
        base.apply(BuildOp::Fill {
            min: [0, 0, 2],
            max: [2, 1, 2],
            cube: Some(Cube::new(Kind(2), Fill::Solid)),
        });
        base.apply(BuildOp::Fill {
            min: [0, 0, 3],
            max: [0, 0, 3],
            cube: Some(Cube::new(Kind(2), Fill::Open)),
        });
        base.apply(BuildOp::Fill {
            min: [1, 0, 3],
            max: [1, 0, 3],
            cube: Some(Cube::new(Kind(2), Fill::Open).at_level(0)),
        });
        base.act(
            &Op::Carve {
                cubes: vec![[1, 0, 2]],
            },
            &Context::free(&rules),
        )
        .unwrap();
        let now = base.snapshot();
        // The same base as it was saved before: each cube's word packed the old way.
        let v1 = |word: u32| {
            if word == 0 {
                return 0;
            }
            let shape = match (word >> 12) & 0x1F {
                30 => 14,
                31 => 15,
                code => code,
            };
            (word & 0xFFF)
                | shape << 12
                | ((word >> 17) & 0xF) << 16
                | ((word >> 21) & 1) << 20
                | ((word >> 22) & 0x3) << 21
                | (word & 0xFF00_0000)
        };
        let old = Snapshot {
            version: 1,
            sections: now
                .sections
                .iter()
                .map(|section| SavedSection {
                    runs: section.runs.iter().map(|&(n, w)| (n, v1(w))).collect(),
                    ..section.clone()
                })
                .collect(),
            ..now.clone()
        };
        assert_ne!(old.sections, now.sections);
        let read = Base::from_snapshot(&old).unwrap();
        assert_eq!(read, base);
        assert_eq!(read.snapshot(), now);
        assert_eq!(read.cube(IVec3::new(1, 0, 0)).unwrap().shape, shapes[1]);
    }

    #[test]
    fn a_snapshot_holding_glass_loads_it_as_an_opening() {
        // A window's glass, packed as 30 before D225 took glass out, in a snapshot of now.
        let mut base = Base::default();
        base.apply(BuildOp::Fill {
            min: [0, 0, 0],
            max: [1, 0, 0],
            cube: Some(crate::Cube::new(Kind(2), Fill::Open)),
        });
        let now = base.snapshot();
        let glass = |word: u32| {
            if (word >> 12) & 0x1F == 31 {
                word & !(1 << 12)
            } else {
                word
            }
        };
        let old = Snapshot {
            sections: now
                .sections
                .iter()
                .map(|section| SavedSection {
                    runs: section.runs.iter().map(|&(n, w)| (n, glass(w))).collect(),
                    ..section.clone()
                })
                .collect(),
            ..now.clone()
        };
        assert_ne!(old.sections, now.sections);
        let read = Base::from_snapshot(&old).unwrap();
        assert_eq!(read, base);
        assert_eq!(read.snapshot(), now);
    }

    #[test]
    fn a_snapshot_with_a_cost_past_any_real_one_is_refused() {
        use crate::ops::{Context, Op, Planned};
        use crate::{Amount, Bill, Rules, Terms};
        // Costs as a kit's are, and a prop paid for in whole items.
        let rules = Rules {
            kinds: BTreeMap::from([(Kind(1), Terms::new(2, Bill::of("Wood", Amount::new(1, 16))))]),
            ..Rules::default()
        };
        let cx = Context::free(&rules);
        let mut base = Base::default();
        let plan = Op::LayPlan {
            part: Some(PartId(0)),
            kind: Kind(1),
            cubes: vec![Planned {
                at: [0, 0, 0],
                shape: crate::Shape::Whole,
            }],
            ground: Vec::new(),
        };
        base.act(&plan, &cx).unwrap();
        let mut real = base.snapshot();
        // A roof as snapshots held them before roofs were cubes.
        real.roofs = vec![(
            RoofId(0),
            Roof {
                section: RoofSection {
                    min: [0, 0, 0],
                    max: [1, 0, 1],
                    style: RoofStyle::Gable,
                    along_x: true,
                    pitch: 0.7,
                },
                kind: Some(Kind(1)),
                per_stage: Bill::of("Wood", Amount::new(1, 32)),
                stages: vec![0; 4],
            },
        )];
        real.prop_paid = vec![([0, 0, 0], Bill::of("Door", Amount::whole(12)))];
        assert!(Base::from_snapshot(&real).is_ok());
        // A share whose denominator times another such is past 64 bits, a share so fine
        // and an amount so large that adding a whole item to either overflows.
        let hostile = [
            Amount::new(1, (1 << 32) + 1),
            Amount::new(1, 1 << 63),
            Amount::whole(1 << 63),
        ];
        for amount in hostile {
            let bill = Bill::of("Wood", amount);
            let mut part = real.clone();
            part.parts[0].1.terms.per_step = bill.clone();
            let mut roof = real.clone();
            roof.roofs[0].1.per_stage = bill.clone();
            let mut prop = real.clone();
            prop.prop_paid[0].1.add(&bill);
            for snapshot in [part, roof, prop] {
                assert_eq!(
                    Base::from_snapshot(&snapshot),
                    Err(BadSnapshot::Amount(amount))
                );
            }
        }
    }

    #[test]
    fn a_snapshot_naming_a_part_it_does_not_have_is_refused() {
        use crate::ops::{Context, Op, Planned};
        let rules = crate::Rules::default();
        let mut base = Base::default();
        base.act(
            &Op::LayPlan {
                part: Some(PartId(3)),
                kind: Kind(1),
                cubes: vec![Planned {
                    at: [0, 0, 0],
                    shape: crate::Shape::Whole,
                }],
                ground: Vec::new(),
            },
            &Context::free(&rules),
        )
        .unwrap();
        let snapshot = base.snapshot();
        assert!(Base::from_snapshot(&snapshot).is_ok());
        // A cube in it, with the part left out.
        let without = Snapshot {
            parts: Vec::new(),
            ..snapshot.clone()
        };
        assert_eq!(
            Base::from_snapshot(&without),
            Err(BadSnapshot::Part(PartId(3)))
        );
        // A cube marked to change to it.
        let mut replacing = snapshot;
        replacing.replacing = vec![([0, 0, 0], (Kind(1), Some(PartId(8))))];
        assert_eq!(
            Base::from_snapshot(&replacing),
            Err(BadSnapshot::Part(PartId(8)))
        );
    }
}
