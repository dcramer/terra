//! A base as saved and sent (construction-001, gap 4): a snapshot of its sections, parts,
//! props and the ground under them, plus the operations since, folded in now and then, as
//! D21 says for terrain. Only the format of now is read: a snapshot of any other version is
//! refused (D353).

use crate::cubes::{BadSection, Cubes, PartId, Replacement, SavedSection};
use crate::ground::Sample;
use crate::ops::Op;
use crate::{Base, BuildOp, Part, Prop};
use std::collections::BTreeMap;

/// Bumped when the snapshot changes; a snapshot of another version is refused (D353).
pub const VERSION: u32 = 2;

/// A base, whole.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub sections: Vec<SavedSection>,
    pub parts: Vec<(PartId, Part)>,
    /// What each cube marked to change changes to.
    pub replacing: Vec<([i32; 3], Replacement)>,
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
    /// Saved by another build, in a format this one does not read.
    Version(u32),
    Section(BadSection),
    /// A cube is in a part the snapshot does not have.
    Part(PartId),
    /// What a part or a prop cost is an amount no cost could be
    /// ([`crate::Amount::in_bounds`]).
    Amount(crate::Amount),
}

impl std::fmt::Display for BadSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BadSnapshot::Version(version) => write!(
                f,
                "saved by an older build (version {version}, not {VERSION}); start a new game"
            ),
            BadSnapshot::Section(why) => write!(f, "a section cannot be read: {why:?}"),
            BadSnapshot::Part(part) => write!(f, "a cube is in part {part:?}, which it lacks"),
            BadSnapshot::Amount(amount) => write!(f, "a cost of {amount:?} is past any real one"),
        }
    }
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

    /// The base a snapshot holds, every section of it marked to be laid.
    pub fn from_snapshot(snapshot: &Snapshot) -> Result<Base, BadSnapshot> {
        if snapshot.version != VERSION {
            return Err(BadSnapshot::Version(snapshot.version));
        }
        // What it says things cost arrives over the network: an amount past what any real
        // cost comes to, where working with it could overflow, is refused.
        let bills = (snapshot.parts.iter().map(|(_, part)| &part.terms.per_step))
            .chain(snapshot.prop_paid.iter().map(|(_, paid)| paid));
        if let Some(amount) = bills.filter_map(crate::Bill::out_of_bounds).next() {
            return Err(BadSnapshot::Amount(amount));
        }
        let mut base = Base {
            cubes: Cubes::load(&snapshot.sections).map_err(BadSnapshot::Section)?,
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
        Ok(base)
    }

    /// The base shortcuts make, laid in order.
    pub fn from_shortcuts(ops: &[BuildOp]) -> Base {
        let mut base = Base::default();
        base.apply_shortcuts(ops);
        base
    }
}

/// A base restored from a snapshot and the operations saved since, as a saved game is
/// loaded: each applied in order, refused ones left out and said. The operations applied,
/// and why any were left out.
pub fn restore(
    snapshot: &Snapshot,
    since: &[Op],
    cx: &crate::ops::Context,
) -> Result<(Base, Vec<Op>, Vec<String>), BadSnapshot> {
    let mut base = Base::from_snapshot(snapshot)?;
    let (mut applied, mut left_out) = (Vec::new(), Vec::new());
    for op in since {
        match base.act(op, cx) {
            Ok(_) => applied.push(op.clone()),
            Err(why) => left_out.push(format!("{op:?}: {why:?}")),
        }
    }
    Ok((base, applied, left_out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Kind;

    #[test]
    fn a_snapshot_reads_back_as_the_base_it_was_taken_of() {
        let base = Base::from_shortcuts(&[
            BuildOp::AddRoom {
                min: [0, 0, 0],
                max: [5, 0, 4],
                kind: Kind(1),
            },
            BuildOp::SetWall {
                tile: [2, 0, 4],
                panel: Some(crate::Panel::Door),
                kind: Kind(1),
            },
        ]);
        let snapshot = base.snapshot();
        let read: Snapshot = ron::from_str(&ron::to_string(&snapshot).unwrap()).unwrap();
        assert_eq!(read, snapshot);
        let back = Base::from_snapshot(&read).unwrap();
        assert_eq!(back, base);
        assert!(!back.store().dirty().is_empty());
        let old = Snapshot {
            version: 1,
            ..snapshot
        };
        assert_eq!(Base::from_snapshot(&old), Err(BadSnapshot::Version(1)));
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
            let mut prop = real.clone();
            prop.prop_paid[0].1.add(&bill);
            for snapshot in [part, prop] {
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
