//! Flights of steps in a timber kind (D215, research/role-pieces.md): where the kit dresses
//! a step's nose with a member, a stack of its cubes stepping up is laid as stairs, not as a
//! stepped wall. Each step a short stack of logs across the flight, the top one along its
//! nose, a tread of boards over it; and a stringer up each open side of the flight, through
//! its noses. Found from the roles (`roles`): a step is a column of the kind's cubes whose
//! top edge is a [`Role::Nose`], and the column the top step's nose leads onto.

use super::roles::{Dir, Role, Roles};
use super::{Layout, Piece, Place, Split, hash, runs, ticks};
use crate::kit::{Kit, Kits};
use crate::{Base, CELL_M, Fill};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// A step's column: its cubes from `bottom` to `top`, and the way its nose looks out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Step {
    pub bottom: i32,
    pub top: i32,
    pub out: Dir,
}

/// How thick a tread's boards are, and how far a stringer stands out of the flight's side.
const TREAD_M: f32 = 0.05;
const STRINGER_PROUD_M: f32 = 0.03;

/// Every step of a base's flights in a kit's kind, by column; none if the kit does not
/// dress noses with a member.
pub(super) fn find(base: &Base, kit: &Kit, kits: &Kits) -> BTreeMap<[i32; 2], Step> {
    let mut steps = BTreeMap::new();
    if kit.dress.member(Role::Nose).is_none() {
        return steps;
    }
    let roles = Roles::new(base, kit.kind(), kits);
    let ours = |at: IVec3| {
        base.cube(at)
            .is_some_and(|cube| cube.kind == kit.kind() && cube.fill == Fill::Solid)
    };
    let column = |top: IVec3, out: Dir| {
        let mut bottom = top;
        while ours(bottom - IVec3::Y) {
            bottom -= IVec3::Y;
        }
        Step {
            bottom: bottom.y,
            top: top.y,
            out,
        }
    };
    for (at, cube) in base.cubes() {
        if cube.kind != kit.kind() || ours(at + IVec3::Y) {
            continue;
        }
        let Some(nose) = roles
            .edges(at)
            .into_iter()
            .find(|edge| edge.role == Role::Nose)
        else {
            continue;
        };
        steps.insert([at.x, at.z], column(at, nose.side));
        // The step it leads up onto, if that is the flight's top.
        let above = at - nose.side.step() + IVec3::Y;
        let mut top = above;
        while ours(top + IVec3::Y) {
            top += IVec3::Y;
        }
        if top == above {
            steps
                .entry([above.x, above.z])
                .or_insert(column(above, nose.side));
        }
    }
    steps
}

/// The cubes of a base's flights, which its walls leave alone.
pub(super) fn cubes(steps: &BTreeMap<[i32; 2], Step>) -> BTreeSet<[i32; 3]> {
    steps
        .iter()
        .flat_map(|(&[x, z], step)| (step.bottom..=step.top).map(move |y| [x, y, z]))
        .collect()
}

/// Lays every flight: its steps' logs, noses and treads, and its stringers.
pub(super) fn lay(kit: &Kit, steps: &BTreeMap<[i32; 2], Step>, layout: &mut Layout) {
    let radius = kit.radius();
    // Steps side by side across a flight, at the same height, laid as one.
    let mut done = BTreeSet::new();
    for (&[x, z], step) in steps {
        if !done.insert([x, z]) {
            continue;
        }
        let across = if step.out.axis() == 0 {
            Dir::South
        } else {
            Dir::East
        };
        let a = across.step();
        let same = |c: [i32; 2]| steps.get(&c) == Some(step);
        let mut first = IVec3::new(x, 0, z);
        while same([first.x - a.x, first.z - a.z]) {
            first -= a;
        }
        let mut last = IVec3::new(x, 0, z);
        while same([last.x + a.x, last.z + a.z]) {
            last += a;
            done.insert([last.x, last.z]);
        }
        let axis = across.axis();
        let middle = (first.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL_M;
        let point = |along: f32, height: f32, forward: f32| {
            let mut p = middle + step.out.step().as_vec3() * forward;
            p[axis] = along;
            p.y = height;
            p
        };
        let (from, to) = (
            first[axis] as f32 * CELL_M - kit.overhang,
            (last[axis] + 1) as f32 * CELL_M + kit.overhang,
        );
        let courses = ((step.top - step.bottom + 1) as f32 * CELL_M / kit.course).round() as i32;
        for course in 0..courses {
            let top = course + 1 == courses;
            let centre = step.bottom as f32 * CELL_M + kit.course * (course as f32 + 0.5);
            // The top one along the nose, as far forward as the step's face.
            let forward = if top {
                CELL_M * 0.5 - radius * 1.1
            } else {
                0.0
            };
            let mut log = runs::log(
                point(from, centre, forward),
                point(to, centre, forward),
                if course == 0 {
                    Split::Top
                } else {
                    Split::Whole
                },
                hash(&[x as u32, z as u32, course as u32, 17]),
                None,
            );
            if top {
                runs::set_role(&mut log, Some(Role::Nose));
            }
            layout.pieces.push(log);
        }
        // A tread of two boards over it, from the nose back.
        let height = (step.top + 1) as f32 * CELL_M + TREAD_M * 0.5;
        for board in 0..2 {
            let forward = CELL_M * (0.25 - 0.5 * board as f32) * 0.98;
            layout.pieces.push(Piece::Plank {
                from: Place::near(point(from + kit.overhang, height, forward)),
                to: Place::near(point(to - kit.overhang, height, forward)),
                wide: ticks(CELL_M * 0.48),
                high: ticks(TREAD_M),
                seed: hash(&[x as u32, z as u32, board, 19]),
            });
        }
    }
    stringers(kit, steps, layout);
}

/// A stringer up each open side of each flight, through its noses, under its treads.
fn stringers(kit: &Kit, steps: &BTreeMap<[i32; 2], Step>, layout: &mut Layout) {
    let width = runs::post_width(kit).unwrap_or(kit.diameter * 0.6);
    // Each side line: the steps whose neighbour across that way is no step of the flight,
    // gathered by the line they are on and the way they look.
    let mut lines: BTreeMap<(i32, i32, Dir), Vec<(i32, Step)>> = BTreeMap::new();
    for (&[x, z], step) in steps {
        let across = if step.out.axis() == 0 {
            Dir::South
        } else {
            Dir::East
        };
        for side in [across, across.opposite()] {
            let s = side.step();
            if steps
                .get(&[x + s.x, z + s.z])
                .is_some_and(|other| other.out == step.out)
            {
                continue;
            }
            let (line, along) = if side.axis() == 0 { (x, z) } else { (z, x) };
            lines
                .entry((line, side.sign(), step.out))
                .or_default()
                .push((along, *step));
        }
    }
    for ((line, sign, out), mut flight) in lines {
        flight.sort_by_key(|(_, step)| step.top);
        let (Some(&(low_at, low)), Some(&(high_at, high))) = (flight.first(), flight.last()) else {
            continue;
        };
        if high.top == low.top {
            continue;
        }
        let side_axis = if out.axis() == 0 { 2 } else { 0 };
        let face = (line + i32::from(sign > 0)) as f32 * CELL_M;
        let across = face - sign as f32 * (width * 0.5 - STRINGER_PROUD_M);
        // Each step's nose: its front face, at its top.
        let nose = |at: i32, step: Step| {
            let front = (at + i32::from(out.sign() > 0)) as f32 * CELL_M;
            let mut p = Vec3::ZERO;
            p[out.axis()] = front;
            p[side_axis] = across;
            p.y = (step.top + 1) as f32 * CELL_M - width * 0.5 - TREAD_M - 0.05;
            p
        };
        let mut stringer = runs::log(
            nose(low_at, low),
            nose(high_at, high),
            Split::Whole,
            hash(&[line as u32, sign as u32, out as u32, 23]),
            None,
        );
        runs::set_role(&mut stringer, Some(Role::Jamb));
        layout.pieces.push(stringer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::{Dress, Profile};
    use crate::layout::lay;
    use crate::mesh::Material;
    use crate::{BuildOp, Cube};

    fn kit() -> Kit {
        let mut kit = Kit::default();
        let member = |scale: f32| Dress::Member {
            profile: Profile::Hewn,
            scale,
            material: Material::Log,
        };
        kit.dress.roles = [(Role::Nose, member(1.0)), (Role::Jamb, member(0.6))]
            .into_iter()
            .collect();
        kit
    }

    #[test]
    fn a_flight_of_timber_is_laid_as_steps_with_noses_treads_and_stringers() {
        // Four steps two cubes wide, rising north, each a cube higher.
        let mut base = Base::default();
        for step in 0..4 {
            base.apply(BuildOp::Fill {
                min: [0, 0, -step],
                max: [1, step, -step],
                cube: Some(Cube::new(kit().kind(), Fill::Solid)),
            });
        }
        let kits = Kits(vec![kit()]);
        let steps = find(&base, &kit(), &kits);
        assert_eq!(steps.len(), 8, "{steps:?}");
        assert!(steps.values().all(|step| step.out == Dir::South));
        let layout = lay(&base, &kit());
        let role = |which: Option<Role>| {
            layout
                .pieces
                .iter()
                .filter(|p| matches!(p, Piece::Log { role, .. } if *role == which))
                .count()
        };
        // A nose along each step, across both its cubes as one.
        assert_eq!(role(Some(Role::Nose)), 4);
        // A stringer up each side.
        assert_eq!(role(Some(Role::Jamb)), 2);
        // Every log of a step lies across the flight, along x: none is a stepped wall.
        for piece in &layout.pieces {
            if let Piece::Log { from, to, role, .. } = piece
                && *role != Some(Role::Jamb)
            {
                assert_eq!(from.0[2], to.0[2], "{piece:?}");
            }
        }
        let treads = layout
            .pieces
            .iter()
            .filter(|p| matches!(p, Piece::Plank { .. }))
            .count();
        assert_eq!(treads, 4 * 2);
    }
}
