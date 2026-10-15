//! What goes in next and what comes out first (docs/building-tech.md, constructing): a
//! part goes up from the bottom, its lowest cubes not yet built first, and comes down
//! last in, first out. A long-member kind puts a course into every cube of the part on
//! that layer at once, as a log spans its wall; a block kind a cube at a time, the one
//! nearest the builder (D194). A roof kind goes up a layer at a time, a stage at a time:
//! frame, battens, covering and trims over every cube of the layer (D241).
//!
//! These only choose; the operations they make are checked like any other, so what they
//! choose may still be refused, and the refusal says why.

use crate::cubes::{Cube, Fill, PartId, Pending};
use crate::{Base, CELL_M};
use glam::{IVec3, Vec3};

/// How far a part has gone: its cubes, those built, and those with something in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub cubes: usize,
    pub built: usize,
    pub started: usize,
    /// Openings left in it, which are not built.
    pub openings: usize,
}

impl Progress {
    /// Whether every cube of it that is to be built is.
    pub fn done(&self) -> bool {
        self.built + self.openings == self.cubes
    }
}

impl Base {
    /// Every cube of a part, in order.
    pub fn cubes_of(&self, part: PartId) -> impl Iterator<Item = (IVec3, Cube)> + '_ {
        self.in_parts
            .range((part, i32::MIN)..=(part, i32::MAX))
            .flat_map(|(_, cubes)| cubes.iter())
            .filter_map(move |&at| {
                let at = IVec3::from_array(at);
                self.cube(at)
                    .filter(|cube| cube.part == Some(part))
                    .map(|cube| (at, cube))
            })
    }

    /// How far a part has gone.
    pub fn progress(&self, part: PartId) -> Progress {
        let mut progress = Progress::default();
        for (_, cube) in self.cubes_of(part) {
            progress.cubes += 1;
            if cube.fill == Fill::Open {
                progress.openings += 1;
            } else if cube.is_built() {
                progress.built += 1;
            } else if cube.level() > 0 {
                progress.started += 1;
            }
        }
        progress
    }

    /// The cubes of a part that would go in next: the lowest not yet built, those least
    /// far in, all of them on that layer or, a cube at a time, the nearest to `near` (in
    /// metres, the base's own space). `None` once it is all built.
    pub fn next_in(&self, part: PartId, near: Vec3, one_at_a_time: bool) -> Option<Vec<[i32; 3]>> {
        let waiting: Vec<(IVec3, Cube)> = self
            .unbuilt_of(part)
            .filter(|(_, cube)| {
                cube.fill == Fill::Solid && !cube.is_built() && cube.pending == Pending::None
            })
            .collect();
        let lowest = waiting.iter().map(|(at, _)| at.y).min()?;
        let least = waiting
            .iter()
            .filter(|(at, _)| at.y == lowest)
            .map(|(_, cube)| cube.level())
            .min()?;
        let layer = waiting
            .into_iter()
            .filter(|(at, cube)| at.y == lowest && cube.level() == least)
            .map(|(at, _)| at);
        Some(pick(layer, near, one_at_a_time))
    }

    /// The cubes of a part that would come out first: the highest with something in,
    /// those furthest in, all of them on that layer or, a cube at a time, the nearest to
    /// `near`. `None` if nothing of it is in.
    pub fn last_in(&self, part: PartId, near: Vec3, one_at_a_time: bool) -> Option<Vec<[i32; 3]>> {
        let ins: Vec<(IVec3, Cube)> = self
            .cubes_of(part)
            .filter(|(_, cube)| cube.is_in())
            .collect();
        let highest = ins.iter().map(|(at, _)| at.y).max()?;
        let most = ins
            .iter()
            .filter(|(at, _)| at.y == highest)
            .map(|(_, cube)| cube.level())
            .max()?;
        let layer = ins
            .into_iter()
            .filter(|(at, cube)| at.y == highest && cube.level() == most)
            .map(|(at, _)| at);
        Some(pick(layer, near, one_at_a_time))
    }

    /// What carving out the cubes marked to be carved does next (construction-001,
    /// experience 4): make those now empty an opening; else take a step out of the highest
    /// with something in, those furthest in, all of them on that layer, a course across the
    /// opening, or the nearest to `near` if its kind goes a cube at a time. `None` if none
    /// is marked.
    pub fn next_carve(
        &self,
        near: Vec3,
        one_at_a_time: impl Fn(crate::Kind) -> bool,
    ) -> Option<crate::ops::Op> {
        let marked: Vec<(IVec3, Cube)> = self
            .pending()
            .filter(|(_, cube)| cube.pending == Pending::Carve && cube.fill == Fill::Solid)
            .collect();
        let empty: Vec<[i32; 3]> = marked
            .iter()
            .filter(|(_, cube)| cube.level() == 0)
            .map(|(at, _)| at.to_array())
            .collect();
        if !empty.is_empty() {
            return Some(crate::ops::Op::Open { cubes: empty });
        }
        let highest = marked.iter().map(|(at, _)| at.y).max()?;
        let most = marked
            .iter()
            .filter(|(at, _)| at.y == highest)
            .map(|(_, cube)| cube.level())
            .max()?;
        let layer: Vec<(IVec3, Cube)> = marked
            .into_iter()
            .filter(|(at, cube)| at.y == highest && cube.level() == most)
            .collect();
        let one = layer.iter().any(|(_, cube)| one_at_a_time(cube.kind));
        let cubes = pick(layer.into_iter().map(|(at, _)| at), near, one);
        Some(crate::ops::Op::Empty { cubes })
    }

    /// What changing the cubes marked to change to another kind does next (D232), as
    /// carving does: clear the mark of those changed and all in; turn those emptied to
    /// their new kind; take a step out of the highest of the old with something in; else put
    /// a step of the new into the lowest not yet in, a cube at a time if its kind goes so.
    /// `None` if none is marked.
    pub fn next_change(
        &self,
        near: Vec3,
        one_at_a_time: impl Fn(crate::Kind) -> bool,
    ) -> Option<crate::ops::Op> {
        use crate::ops::Op;
        let marked: Vec<(IVec3, Cube)> = self
            .pending()
            .filter(|(_, cube)| cube.pending.changing() && cube.fill == Fill::Solid)
            .collect();
        let of = |pending: Pending, keep: &dyn Fn(&Cube) -> bool| -> Vec<(IVec3, Cube)> {
            marked
                .iter()
                .filter(|(_, cube)| cube.pending == pending && keep(cube))
                .copied()
                .collect()
        };
        let places = |cubes: &[(IVec3, Cube)]| cubes.iter().map(|(at, _)| at.to_array()).collect();
        let done = of(Pending::Swapped, &|cube| cube.is_built());
        if !done.is_empty() {
            return Some(Op::Cancel {
                cubes: places(&done),
            });
        }
        let emptied = of(Pending::Replace, &|cube| cube.level() == 0);
        if !emptied.is_empty() {
            return Some(Op::Swap {
                cubes: places(&emptied),
            });
        }
        let old = of(Pending::Replace, &|cube| cube.level() > 0);
        if let Some(highest) = old.iter().map(|(at, _)| at.y).max() {
            let most = old
                .iter()
                .filter(|(at, _)| at.y == highest)
                .map(|(_, cube)| cube.level())
                .max()?;
            let layer: Vec<(IVec3, Cube)> = old
                .into_iter()
                .filter(|(at, cube)| at.y == highest && cube.level() == most)
                .collect();
            let one = layer.iter().any(|(_, cube)| one_at_a_time(cube.kind));
            let cubes = pick(layer.into_iter().map(|(at, _)| at), near, one);
            return Some(Op::Empty { cubes });
        }
        let new = of(Pending::Swapped, &|cube| !cube.is_built());
        let lowest = new.iter().map(|(at, _)| at.y).min()?;
        let least = new
            .iter()
            .filter(|(at, _)| at.y == lowest)
            .map(|(_, cube)| cube.level())
            .min()?;
        let layer: Vec<(IVec3, Cube)> = new
            .into_iter()
            .filter(|(at, cube)| at.y == lowest && cube.level() == least)
            .collect();
        let one = layer.iter().any(|(_, cube)| one_at_a_time(cube.kind));
        let cubes = pick(layer.into_iter().map(|(at, _)| at), near, one);
        Some(Op::Fill { cubes })
    }

    /// A part's plan that can be taken away for nothing: its cubes with nothing in them,
    /// and its openings with nothing fitted. Empty if any of it is in.
    pub fn plan_of(&self, part: PartId) -> Vec<[i32; 3]> {
        let cubes: Vec<(IVec3, Cube)> = self.cubes_of(part).collect();
        if cubes
            .iter()
            .any(|(_, cube)| cube.fill != Fill::Open && cube.level() > 0)
        {
            return Vec::new();
        }
        cubes
            .into_iter()
            .filter(|(at, cube)| cube.fill != Fill::Open || self.prop_in(*at).is_none())
            .map(|(at, _)| at.to_array())
            .collect()
    }

    /// The same base with only some of its cubes, and no props or parts, for drawing what
    /// is built apart from what is planned.
    pub fn view(&self, cube: impl Fn(&Cube) -> bool) -> Base {
        let mut cubes = crate::Cubes::default();
        for (at, kept) in self.cubes().filter(|(_, each)| cube(each)) {
            cubes
                .set(at, Some(Cube { part: None, ..kept }))
                .expect("no parts");
        }
        Base::from_cubes(cubes)
    }
}

/// All of a layer's cubes, sorted, or the one nearest a point.
fn pick(layer: impl Iterator<Item = IVec3>, near: Vec3, one: bool) -> Vec<[i32; 3]> {
    let layer: Vec<IVec3> = layer.collect();
    if !one {
        return layer.into_iter().map(|at| at.to_array()).collect();
    }
    let middle = |at: IVec3| (at.as_vec3() + Vec3::splat(0.5)) * CELL_M;
    layer
        .into_iter()
        .min_by(|a, b| {
            middle(*a)
                .distance_squared(near)
                .total_cmp(&middle(*b).distance_squared(near))
        })
        .map(|at| vec![at.to_array()])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::{Amount, Bill, Rules, Terms};
    use crate::cubes::{Kind, Shape};
    use crate::ground::Sample;
    use crate::ops::{Context, Op, Planned};
    use std::collections::BTreeMap;

    const LOGS: Kind = Kind(1);
    const STONE: Kind = Kind(2);

    fn rules() -> Rules {
        Rules {
            kinds: BTreeMap::from([
                (LOGS, Terms::new(2, Bill::of("Wood", Amount::new(1, 16)))),
                (STONE, Terms::new(1, Bill::of("Stone", Amount::new(1, 4)))),
            ]),
            ..Rules::default()
        }
    }

    /// A wall four cubes long and two high, planned on level ground.
    fn wall(base: &mut Base, part: u32, kind: Kind, cx: &Context) {
        let cubes: Vec<Planned> = (0..4)
            .flat_map(|x| (0..2).map(move |y| [x, y, 0]))
            .map(|at| Planned {
                at,
                shape: Shape::Whole,
            })
            .collect();
        let ground = (0..4)
            .map(|x| Sample {
                column: [x, 0],
                height_cm: 0,
            })
            .collect();
        base.act(
            &Op::LayPlan {
                part: Some(PartId(part)),
                kind,
                cubes,
                ground,
            },
            cx,
        )
        .unwrap();
    }

    #[test]
    fn a_window_cut_into_a_built_wall_comes_out_a_course_at_a_time_then_opens() {
        let rules = rules();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        wall(&mut base, 0, LOGS, &cx);
        while let Some(cubes) = base.next_in(PartId(0), Vec3::ZERO, false) {
            base.act(&Op::Fill { cubes }, &cx).unwrap();
        }
        assert_eq!(base.next_carve(Vec3::ZERO, |_| false), None);
        // The lower layer's middle two cubes, under the layer that stays.
        let cut = vec![[1, 0, 0], [2, 0, 0]];
        base.act(&Op::Carve { cubes: cut.clone() }, &cx).unwrap();
        let mut steps = Vec::new();
        while let Some(op) = base.next_carve(Vec3::ZERO, |_| false) {
            steps.push(op.clone());
            base.act(&op, &cx).unwrap();
        }
        assert_eq!(
            steps,
            vec![
                Op::Empty { cubes: cut.clone() },
                Op::Empty { cubes: cut.clone() },
                Op::Open { cubes: cut.clone() },
            ]
        );
        assert!(cut.iter().all(|&at| {
            base.cube(IVec3::from_array(at))
                .is_some_and(|cube| cube.fill == Fill::Open)
        }));
        assert!(base.floating().is_empty());
    }

    #[test]
    fn a_log_wall_goes_up_a_course_at_a_time_and_comes_down_the_same() {
        let rules = rules();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        wall(&mut base, 0, LOGS, &cx);
        let part = PartId(0);
        let mut fills = 0;
        while let Some(cubes) = base.next_in(part, Vec3::ZERO, false) {
            assert_eq!(cubes.len(), 4, "a course across the whole wall");
            base.act(&Op::Fill { cubes }, &cx).unwrap();
            fills += 1;
        }
        // Two layers, each in two courses.
        assert_eq!(fills, 4);
        assert!(base.progress(part).done());
        let mut empties = 0;
        while let Some(cubes) = base.last_in(part, Vec3::ZERO, false) {
            assert!(cubes.iter().all(|at| at[1] == 1) || empties >= 2);
            base.act(&Op::Empty { cubes }, &cx).unwrap();
            empties += 1;
        }
        assert_eq!(empties, 4);
        assert_eq!(base.plan_of(part).len(), 8);
    }

    #[test]
    fn a_stone_wall_goes_in_a_cube_at_a_time_nearest_first() {
        let rules = rules();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        wall(&mut base, 0, STONE, &cx);
        let near = Vec3::new(1.9, 0.0, 0.0);
        let first = base.next_in(PartId(0), near, true).unwrap();
        assert_eq!(first, vec![[3, 0, 0]]);
        base.act(&Op::Fill { cubes: first }, &cx).unwrap();
        assert_eq!(base.next_in(PartId(0), near, true), Some(vec![[2, 0, 0]]));
        // Nothing of it to take away for nothing once some of it is in.
        assert!(base.plan_of(PartId(0)).is_empty());
    }

    #[test]
    fn an_opening_is_left_out_of_what_goes_in_and_goes_with_its_plan() {
        let rules = rules();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        wall(&mut base, 0, LOGS, &cx);
        base.act(
            &Op::Open {
                cubes: vec![[1, 0, 0], [1, 1, 0]],
            },
            &cx,
        )
        .unwrap();
        assert_eq!(base.next_in(PartId(0), Vec3::ZERO, false).unwrap().len(), 3);
        assert_eq!(base.progress(PartId(0)).openings, 2);
        let done = base
            .act(
                &Op::Unplan {
                    cubes: base.plan_of(PartId(0)),
                },
                &cx,
            )
            .unwrap();
        assert!(base.is_empty());
        base.act_all(&done.undo, &cx).unwrap();
        assert_eq!(base.progress(PartId(0)).openings, 2);
    }

    #[test]
    fn a_roof_goes_up_a_layer_at_a_time_a_stage_at_a_time() {
        const ROOF: Kind = Kind::FIRST_ROOF;
        let mut rules = rules();
        rules
            .kinds
            .insert(ROOF, Terms::new(4, Bill::of("Wood", Amount::new(1, 32))));
        let cx = Context::free(&rules);
        let mut base = Base::default();
        // Walls along both eaves of a gable, built, and its roof's cubes planned over them.
        for z in [0, 3] {
            base.apply(crate::BuildOp::Fill {
                min: [0, 0, z],
                max: [3, 5, z],
                cube: Some(Cube::new(STONE, Fill::Solid)),
            });
        }
        let footprint = (0..4).flat_map(|x| (0..4).map(move |z| [x, z])).collect();
        let plan = crate::roofer::over(&footprint, 6, crate::roofer::Style::Gable, None);
        let cubes: Vec<Planned> = plan
            .roof
            .iter()
            .map(|&(at, shape)| Planned {
                at: at.to_array(),
                shape,
            })
            .collect();
        base.act(
            &Op::LayPlan {
                part: Some(PartId(1)),
                kind: ROOF,
                cubes,
                ground: Vec::new(),
            },
            &cx,
        )
        .unwrap();
        // The eaves' layer first, a stage at a time over all of it, then the next up, held
        // by its ramp from the one below.
        let mut steps = Vec::new();
        while let Some(cubes) = base.next_in(PartId(1), Vec3::ZERO, false) {
            steps.push((cubes[0][1], cubes.len()));
            base.act(&Op::Fill { cubes }, &cx).unwrap();
        }
        assert_eq!(
            steps,
            vec![
                (6, 8),
                (6, 8),
                (6, 8),
                (6, 8),
                (7, 8),
                (7, 8),
                (7, 8),
                (7, 8)
            ]
        );
        assert!(base.floating().is_empty());
        // And down the other way: the ridge's layer first.
        let first = base.last_in(PartId(1), Vec3::ZERO, false).unwrap();
        assert!(first.iter().all(|at| at[1] == 7));
        // The eaves' cubes hold up the slopes over them: not taken while those are in.
        let eaves = Op::Empty {
            cubes: (0..4).flat_map(|x| [[x, 6, 0], [x, 6, 3]]).collect(),
        };
        for _ in 0..3 {
            base.act(&eaves, &cx).unwrap();
        }
        let refused = base.check(&eaves, &cx);
        assert!(
            matches!(refused, Err(crate::ops::Refusal::Holds(_))),
            "{refused:?}"
        );
    }

    #[test]
    fn a_view_keeps_only_what_is_asked() {
        let rules = rules();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        wall(&mut base, 0, LOGS, &cx);
        let cubes = base.next_in(PartId(0), Vec3::ZERO, false).unwrap();
        base.act(&Op::Fill { cubes }, &cx).unwrap();
        let built = base.view(|cube| cube.is_in());
        assert_eq!(built.cubes().count(), 4);
        assert!(
            built
                .cubes()
                .all(|(at, cube)| at.y == 0 && cube.part.is_none())
        );
        let planned = base.view(|cube| !cube.is_in());
        assert_eq!(planned.cubes().count(), 4);
        assert!(planned.cubes().all(|(at, _)| at.y == 1));
    }
}
