//! Putting a building up by the operations (D108, D280): what a village's houses, the
//! scale scenes and any building made by the shortcuts are, as plans laid and built by the
//! same checked operations a player's are, so every rule a player meets (nothing floats,
//! built from the bottom, a roof on what holds it) holds for them too, and what is drawn
//! and measured is what the operations built.
//!
//! The shortcuts say what a building is; this lays it as a plan, a part a kind, opens its
//! openings, and fills it a layer at a time from the bottom, every step checked.

use crate::cost::Rules;
use crate::cubes::{Fill, Kind, PartId};
use crate::ops::{Context, Op, Planned};
use crate::{Base, ops::Refusal};
use std::collections::BTreeMap;

impl Base {
    /// The operations that build this base's cubes from nothing: a plan a kind, each a part
    /// of its own with the ground under it as sampled, its openings made open, then every
    /// layer filled from the bottom up, a step at a time, until all of it is built.
    /// Fills are worked out against `rules`' terms, as the base built by them would be.
    pub fn operations(&self, rules: &Rules) -> Vec<Op> {
        let mut by_kind: BTreeMap<Kind, Vec<Planned>> = BTreeMap::new();
        let mut openings = Vec::new();
        for (at, cube) in self.cubes() {
            by_kind.entry(cube.kind).or_default().push(Planned {
                at: at.to_array(),
                shape: cube.shape,
            });
            if cube.fill == Fill::Open {
                openings.push(at.to_array());
            }
        }
        let ground: Vec<_> = self.ground().samples().collect();
        let part_of: BTreeMap<Kind, PartId> = by_kind
            .keys()
            .enumerate()
            .map(|(i, &kind)| (kind, PartId(i as u32)))
            .collect();
        let mut ops: Vec<Op> = by_kind
            .into_iter()
            .map(|(kind, cubes)| Op::LayPlan {
                part: Some(part_of[&kind]),
                kind,
                cubes,
                ground: ground.clone(),
            })
            .collect();
        if !openings.is_empty() {
            ops.push(Op::Open { cubes: openings });
        }
        // Each layer from the bottom, its cubes a step at a time until all are in.
        let mut layers: BTreeMap<i32, Vec<([i32; 3], u8)>> = BTreeMap::new();
        for (at, cube) in self.cubes().filter(|(_, cube)| cube.fill != Fill::Open) {
            let steps = crate::cubes::FULL / rules.of(cube.kind).step();
            layers.entry(at.y).or_default().push((at.to_array(), steps));
        }
        for cubes in layers.into_values() {
            let most = cubes.iter().map(|&(_, steps)| steps).max().unwrap_or(0);
            for step in 0..most {
                let fill: Vec<[i32; 3]> = cubes
                    .iter()
                    .filter(|&&(_, steps)| step < steps)
                    .map(|&(at, _)| at)
                    .collect();
                ops.push(Op::Fill { cubes: fill });
            }
        }
        ops
    }

    /// This base put up again from nothing by the checked operations ([`Base::operations`]),
    /// with nothing in the way and everything to hand: the same cubes, built, each in its
    /// kind's part. The first operation refused, and why, if the rules would not let it
    /// stand as it is.
    pub fn put_up(&self, rules: &Rules) -> Result<Base, (Op, Refusal)> {
        let cx = Context::free(rules);
        let mut base = Base::default();
        for op in self.operations(rules) {
            if let Err(why) = base.act(&op, &cx) {
                return Err((op, why));
            }
        }
        Ok(base)
    }

    /// Whether two bases have the same cubes in the same places, whatever parts they are
    /// in.
    pub fn same_cubes(&self, other: &Base) -> bool {
        let strip = |base: &Base| -> Vec<_> {
            base.cubes()
                .map(|(at, cube)| (at, crate::Cube { part: None, ..cube }))
                .collect()
        };
        strip(self) == strip(other)
    }
}

#[cfg(test)]
mod tests {
    use crate::cost::{Bill, Rules, Terms};
    use crate::village::{Kinds, town, village};
    use crate::{Base, Kind};
    use std::collections::BTreeMap;

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

    /// The kits' terms as the game's files give their steps: timber two courses a cube,
    /// blocks one, roofs four stages.
    fn rules() -> Rules {
        let steps = [
            (1, 2),
            (2, 1),
            (3, 2),
            (4, 1),
            (5, 1),
            (0x800, 4),
            (0x801, 4),
            (0x802, 4),
        ];
        Rules {
            kinds: steps
                .iter()
                .map(|&(kind, steps)| (Kind(kind), Terms::new(steps, Bill::default())))
                .collect::<BTreeMap<_, _>>(),
            props: BTreeMap::new(),
            toughness: BTreeMap::new(),
        }
    }

    #[test]
    fn every_village_building_town_and_castle_stands_put_up_by_the_operations() {
        // D108: put up by the same checked operations as a player's buildings, every rule
        // holding, the cubes the same as the shortcuts say.
        let mut buildings = village(7, KINDS);
        buildings.extend(town(3, 40, KINDS));
        let mut all: Vec<(String, Vec<crate::BuildOp>)> = buildings
            .into_iter()
            .map(|building| (building.name, building.ops))
            .collect();
        all.push((
            "castle".into(),
            crate::castle::castle(KINDS.masonry, KINDS.beams, KINDS.masonry_roof),
        ));
        for (name, ops) in all {
            let shortcuts = Base::from_shortcuts(&ops);
            let built = shortcuts
                .put_up(&rules())
                .unwrap_or_else(|(op, why)| panic!("{name}: {why:?} at {op:?}"));
            assert!(built.same_cubes(&shortcuts), "{name}: not the same cubes");
            assert!(built.floating().is_empty(), "{name}");
            assert_eq!(built.store().planned(), 0, "{name}: not all built");
        }
    }
}
