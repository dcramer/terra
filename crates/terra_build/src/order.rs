//! What goes in next and what comes out first (docs/building-tech.md, constructing): a
//! part goes up from the bottom, its lowest cubes not yet built first, and comes down
//! last in, first out. A long-member kind puts a course into every cube of the part on
//! that layer at once, as a log spans its wall; a block kind a cube at a time, the one
//! nearest the builder (D194). Roofs go up a stage at a time over every cell.
//!
//! These only choose; the operations they make are checked like any other, so what they
//! choose may still be refused, and the refusal says why.

use crate::cubes::{Cube, Fill, PartId, Pending};
use crate::{Base, CELL_M, ROOF_STAGES, RoofId};
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
        self.cubes()
            .filter(move |(_, cube)| cube.part == Some(part))
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
            .cubes_of(part)
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

    /// The cells of a roof that would go up a stage next: those least far up. `None` once
    /// it is all covered.
    pub fn next_stage(&self, roof: RoofId) -> Option<Vec<[i32; 2]>> {
        let roof = self.roof(roof)?;
        let least = *roof.stages.iter().min()?;
        (least < ROOF_STAGES).then(|| {
            roof.cells()
                .filter(|&cell| roof.stage(cell) == Some(least))
                .collect()
        })
    }

    /// The cells of a roof that would come down a stage first: those furthest up. `None`
    /// if none of it is up.
    pub fn last_stage(&self, roof: RoofId) -> Option<Vec<[i32; 2]>> {
        let roof = self.roof(roof)?;
        let most = *roof.stages.iter().max()?;
        (most > 0).then(|| {
            roof.cells()
                .filter(|&cell| roof.stage(cell) == Some(most))
                .collect()
        })
    }

    /// The roof over a column, if one covers it: the latest put up.
    pub fn roof_over(&self, column: [i32; 2]) -> Option<RoofId> {
        self.roofs()
            .filter(|(_, roof)| roof.stage(column).is_some())
            .map(|(id, _)| id)
            .last()
    }

    /// The same base with only some of its cubes and roofs, and no props or parts, for
    /// drawing what is built apart from what is planned.
    pub fn view(&self, cube: impl Fn(&Cube) -> bool, roof: impl Fn(&crate::Roof) -> bool) -> Base {
        let mut cubes = crate::Cubes::default();
        for (at, kept) in self.cubes().filter(|(_, each)| cube(each)) {
            cubes
                .set(at, Some(Cube { part: None, ..kept }))
                .expect("no parts");
        }
        let roofs = self
            .roofs
            .iter()
            .filter(|(_, each)| roof(each))
            .map(|(&id, each)| (id, each.clone()))
            .collect();
        Base::from_cubes(cubes, roofs)
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
    use crate::{RoofSection, RoofStyle};
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
    fn a_roof_goes_up_a_stage_over_every_cell() {
        let rules = rules();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        // Walls along both eaves of a block of tiles to hold it, built.
        for z in [0, 2] {
            base.apply(crate::BuildOp::Fill {
                min: [0, 0, z],
                max: [3, 5, z],
                cube: Some(Cube::new(STONE, Fill::Solid)),
            });
        }
        let roof = RoofId(0);
        base.act(
            &Op::AddRoof {
                roof,
                section: RoofSection {
                    min: [0, 0, 0],
                    max: [3, 0, 2],
                    style: RoofStyle::Gable,
                    along_x: true,
                    pitch: 0.7,
                },
                kind: Some(LOGS),
            },
            &cx,
        )
        .unwrap();
        assert_eq!(base.roof_over([1, 1]), Some(roof));
        assert_eq!(base.last_stage(roof), None);
        let mut stages = 0;
        while let Some(cells) = base.next_stage(roof) {
            assert_eq!(cells.len(), 12);
            base.act(
                &Op::RoofStage {
                    roof,
                    cells,
                    up: true,
                },
                &cx,
            )
            .unwrap();
            stages += 1;
        }
        assert_eq!(stages, ROOF_STAGES);
        assert_eq!(base.last_stage(roof).map(|cells| cells.len()), Some(12));
    }

    #[test]
    fn a_view_keeps_only_what_is_asked() {
        let rules = rules();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        wall(&mut base, 0, LOGS, &cx);
        let cubes = base.next_in(PartId(0), Vec3::ZERO, false).unwrap();
        base.act(&Op::Fill { cubes }, &cx).unwrap();
        // A roof up, by the shortcut, and one planned beside it.
        let section = |x: i32| RoofSection {
            min: [x, 0, 0],
            max: [x + 1, 0, 1],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        };
        base.apply(crate::BuildOp::AddRoof(section(0)));
        let planned_roof = base.next_roof();
        base.act(
            &Op::AddRoof {
                roof: planned_roof,
                section: section(4),
                kind: Some(LOGS),
            },
            &cx,
        )
        .unwrap();
        let built = base.view(|cube| cube.is_in(), crate::Roof::standing);
        assert_eq!(built.cubes().count(), 4);
        assert!(
            built
                .cubes()
                .all(|(at, cube)| at.y == 0 && cube.part.is_none())
        );
        assert_eq!(built.sections(), &[section(0)]);
        let planned = base.view(|cube| !cube.is_in(), |roof| !roof.standing());
        assert_eq!(planned.cubes().count(), 4);
        assert!(planned.cubes().all(|(at, _)| at.y == 1));
        assert_eq!(planned.sections(), &[section(4)]);
        assert_eq!(
            planned.roofs().map(|(id, _)| id).collect::<Vec<_>>(),
            [planned_roof]
        );
    }
}
