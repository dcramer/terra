//! Flights of steps in a timber kind (D215, D220, research/role-pieces.md): a stack of
//! the kind's cubes stepping up is laid as open stairs, as a carpenter builds them, not as
//! a stepped wall: a thick tread on each step, its nose standing a little out, housed
//! between two stringers, one up each side of the flight seated on the floor and bearing at
//! its head on what stands behind its top step, a deck's rim (D306), and posts from the floor
//! under the stringers only where their span needs one;
//! nothing under the treads. A step is a column of the kind's cubes topped by a cube cut as
//! a stair (two treads) or a slope (boards up the ramp, a cleat across them) (D193), or,
//! where the kit dresses a step's nose with a member, a whole cube whose top edge is a
//! [`Role::Nose`] (`roles`), and the column the top step's nose leads onto. The cubes under
//! the treads stay whole cubes, walked as cubes are (D220); only their drawing is open.

use super::roles::{Dir, Role, Roles};
use super::{Layout, Piece, Place, Split, runs, ticks};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use terra_build::kit::{Kit, Kits};
use terra_build::{Base, CELL_M, Fill, Shape};
use terra_gen::hash;

/// What tops a step's column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    /// A whole cube: one tread at its top.
    Whole,
    /// A cube cut as a stair: two treads, half a cube apart.
    Stair,
    /// A cube cut as a slope: a ramp of boards.
    Slope,
}

/// A step's column: its cubes from `bottom` to `top`, the way its nose looks out, and what
/// tops it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub bottom: i32,
    pub top: i32,
    pub out: Dir,
    pub cap: Cap,
}

impl Step {
    /// The height of its front nose, the lowest of its treads' fronts, in metres; a ramp's
    /// foot.
    fn nose(&self) -> f32 {
        match self.cap {
            Cap::Whole => (self.top + 1) as f32 * CELL_M,
            Cap::Stair => (self.top as f32 + 0.5) * CELL_M,
            Cap::Slope => self.top as f32 * CELL_M,
        }
    }
}

/// How thick a tread is, how far its nose stands out over the step below, and how far a
/// stringer stands out of the flight's side.
const TREAD_M: f32 = 0.07;
const NOSING_M: f32 = 0.03;
/// A little in, not out: a post under it, centred under it and wider, then stands out of
/// the flight's cubes no further than a trim may (D192).
const STRINGER_PROUD_M: f32 = -0.01;
/// A stringer: how thick and how deep a board, and how far its top edge stands over the
/// line of the treads' noses.
const STRINGER_M: (f32, f32) = (0.07, 0.32);
const STRINGER_ABOVE_M: f32 = 0.06;
/// How high a stringer's underside stands over the floor before a post under it is worth
/// standing.
const POSTS_FROM_M: f32 = 1.0;
/// How far a stringer spans, level, between what it bears on, before a post carries it.
const STRINGER_SPAN_M: f32 = 1.8;
/// A ramp's boards: how wide and thick; the cleat across them, how wide and thick.
const RAMP_BOARD_M: f32 = 0.125;
const RAMP_THICK_M: f32 = 0.035;
const CLEAT_M: (f32, f32) = (0.05, 0.03);

/// Every step of a base's flights in a kit's kind, by column: every column topped by a cube
/// cut as a stair or a slope, and where the kit dresses a nose with a member, every stack
/// stepping up.
pub fn find(base: &Base, kit: &Kit, kits: &Kits, within: super::Scope) -> BTreeMap<[i32; 2], Step> {
    let mut steps = BTreeMap::new();
    let ours = |at: IVec3| {
        base.cube(at)
            .is_some_and(|cube| cube.kind == kit.kind() && cube.fill == Fill::Solid)
    };
    let whole = |at: IVec3| {
        base.cube(at).is_some_and(|cube| {
            cube.kind == kit.kind() && cube.fill == Fill::Solid && cube.shape == Shape::Whole
        })
    };
    let column = |top: IVec3, out: Dir, cap: Cap| {
        let mut bottom = top;
        while whole(bottom - IVec3::Y) {
            bottom -= IVec3::Y;
        }
        Step {
            bottom: bottom.y,
            top: top.y,
            out,
            cap,
        }
    };
    // Only the columns in `within`, of every height: a step is its column's, and a nose
    // the column it leads up onto's too.
    let cubes = super::Scope {
        low: within.low.with_y(i32::MIN / 4),
        high: within.high.with_y(i32::MAX / 4),
    }
    .cubes(base, None);
    // A cube cut as a stair or a slope is a step whatever is on it: something set on its
    // top hides part of it, but it is drawn as what it is. The highest in a column tops it.
    for &(at, cube) in &cubes {
        if cube.kind != kit.kind() || cube.fill != Fill::Solid {
            continue;
        }
        let (facing, cap) = match cube.shape {
            Shape::Stair(facing) => (facing, Cap::Stair),
            Shape::Slope(facing) => (facing, Cap::Slope),
            _ => continue,
        };
        let step = column(at, Dir::facing(facing).opposite(), cap);
        steps
            .entry([at.x, at.z])
            .and_modify(|was: &mut Step| {
                if was.top < step.top {
                    *was = step;
                }
            })
            .or_insert(step);
    }
    if kit.dress.member(Role::Nose).is_none() {
        return steps;
    }
    let roles = Roles::new(base, kit.kind(), kits);
    for &(at, cube) in &cubes {
        if cube.kind != kit.kind() || cube.shape != Shape::Whole || ours(at + IVec3::Y) {
            continue;
        }
        let Some(nose) = roles
            .edges(at)
            .into_iter()
            .find(|edge| edge.role == Role::Nose)
        else {
            continue;
        };
        steps
            .entry([at.x, at.z])
            .or_insert(column(at, nose.side, Cap::Whole));
        // The step it leads up onto, if that is the flight's top.
        let above = at - nose.side.step() + IVec3::Y;
        let mut top = above;
        while ours(top + IVec3::Y) {
            top += IVec3::Y;
        }
        if top == above && whole(above) {
            steps
                .entry([above.x, above.z])
                .or_insert(column(above, nose.side, Cap::Whole));
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

/// Lays every flight as open stairs, as a carpenter builds them: a tread of thick boards on
/// each step, a ramp of boards up a slope, between stringers up each side of the flight,
/// and posts under the stringers where their span needs one; nothing under the treads,
/// so the flight reads as a stair and not a stepped wall.
pub(super) fn lay(base: &Base, kit: &Kit, steps: &BTreeMap<[i32; 2], Step>, layout: &mut Layout) {
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
        let forward_of = step.out.step().as_vec3();
        let point = |along: f32, height: f32, forward: f32| {
            let mut p = middle + forward_of * forward;
            p[axis] = along;
            p.y = height;
            p
        };
        // The treads run from one stringer to the other.
        let (from, to) = (
            first[axis] as f32 * CELL_M,
            (last[axis] + 1) as f32 * CELL_M,
        );
        let tread = |height: f32, forward: f32, deep: f32, salt: u32| Piece::Plank {
            from: Place::near(point(from, height - TREAD_M * 0.5, forward)),
            to: Place::near(point(to, height - TREAD_M * 0.5, forward)),
            wide: ticks(deep),
            high: ticks(TREAD_M),
            seed: hash::of(&[x as u32, z as u32, salt, 19]),
        };
        let top = (step.top + 1) as f32 * CELL_M;
        let half = CELL_M * 0.5;
        match step.cap {
            // One tread at its top, the whole cube deep, its nose standing a little out.
            Cap::Whole => layout
                .pieces
                .push(tread(top, NOSING_M * 0.5, CELL_M + NOSING_M, 0)),
            // A tread on each of its two steps, each half a cube deep and a nosing more.
            Cap::Stair => {
                layout.pieces.push(tread(
                    top - half,
                    half * 0.5 + NOSING_M * 0.5,
                    half + NOSING_M,
                    0,
                ));
                layout
                    .pieces
                    .push(tread(top, -half * 0.5 + NOSING_M * 0.5, half + NOSING_M, 1));
            }
            Cap::Slope => ramp(
                step,
                &point,
                (from, to),
                hash::of(&[x as u32, z as u32, 29]),
                layout,
            ),
        }
    }
    stringers(base, kit, steps, layout);
}

/// A slope's ramp: boards up it side by side across the flight, from its foot at the front
/// to its top at the back, and a cleat across them.
fn ramp(
    step: &Step,
    point: &dyn Fn(f32, f32, f32) -> Vec3,
    (from, to): (f32, f32),
    seed: u32,
    layout: &mut Layout,
) {
    let bottom = step.top as f32 * CELL_M;
    // Down from the ramp's face, square to it, by half a board's thickness.
    let sink = RAMP_THICK_M * 0.5 * std::f32::consts::SQRT_2;
    let width = to - from;
    let boards = ((width / RAMP_BOARD_M).round() as i32).max(1);
    let wide = width / boards as f32;
    for board in 0..boards {
        let along = from + wide * (board as f32 + 0.5);
        layout.pieces.push(Piece::Plank {
            from: Place::near(point(along, bottom - sink, CELL_M * 0.5)),
            to: Place::near(point(along, bottom + CELL_M - sink, -CELL_M * 0.5)),
            wide: ticks(wide - 0.008),
            high: ticks(RAMP_THICK_M),
            seed: hash::of(&[seed, board as u32]),
        });
    }
    // A cleat across the middle of it, on the boards.
    let (cleat_wide, cleat_high) = CLEAT_M;
    let rise = CELL_M * 0.5 + cleat_high * 0.5 * std::f32::consts::SQRT_2;
    layout.pieces.push(Piece::Plank {
        from: Place::near(point(from, bottom + rise, 0.0)),
        to: Place::near(point(to, bottom + rise, 0.0)),
        wide: ticks(cleat_wide),
        high: ticks(cleat_high),
        seed: hash::of(&[seed, 0xc1ea7]),
    });
}

/// A stringer up each side of each flight, the treads housed between them, seated on the
/// floor at the flight's foot and its head against the back of its top step, where it
/// bears on the rim of a deck or anything else standing there, as a carpenter hangs a
/// stringer from a deck's rim joist; and posts from the floor under each stringer only where
/// its span between what it bears on is longer than a stringer spans alone, evenly along it,
/// and under its head where nothing stands behind its top step.
fn stringers(base: &Base, kit: &Kit, steps: &BTreeMap<[i32; 2], Step>, layout: &mut Layout) {
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
        let across = face - sign as f32 * (STRINGER_M.0 * 0.5 - STRINGER_PROUD_M);
        // Each step's nose: its front face, at its lowest tread; a ramp's foot.
        let nose = |at: i32, step: Step| {
            let front = (at + i32::from(out.sign() > 0)) as f32 * CELL_M;
            let mut p = Vec3::ZERO;
            p[out.axis()] = front;
            p[side_axis] = across;
            p.y = step.nose();
            p
        };
        let (a, b) = (nose(low_at, low), nose(high_at, high));
        let rise = (b - a).normalize_or_zero();
        // Its middle under the noses' line, so its top edge stands a little over the treads
        // and its depth covers their backs, as a closed stringer houses them.
        let level = (1.0 - rise.y * rise.y).sqrt().max(0.2);
        let drop = Vec3::Y * ((STRINGER_M.1 * 0.5 - STRINGER_ABOVE_M) / level);
        let (a, b) = (a - drop, b - drop);
        // Down from the lowest nose to the floor, and up past the top nose to the back of
        // the top step.
        let floor = flight
            .iter()
            .map(|(_, step)| step.bottom)
            .min()
            .unwrap_or(low.bottom) as f32
            * CELL_M;
        let down = if rise.y > 1e-3 {
            (a.y - floor - STRINGER_M.1 * 0.5 * level).max(0.0) / rise.y
        } else {
            0.0
        };
        // Up past the top nose until the stringer's lower corner meets the plane of the
        // top step's back, where a deck's rim stands: its square-cut end bears on it.
        let up = ((CELL_M - rise.y * STRINGER_M.1 * 0.5) / level).max(0.0);
        // Whether anything stands behind the top step at its top, for the head to bear on.
        let top_column = {
            let mut at = IVec3::ZERO;
            at[out.axis()] = high_at;
            at[side_axis] = line;
            at.with_y(high.top)
        };
        let bears = base
            .cube(top_column - out.step())
            .is_some_and(|cube| cube.fill == Fill::Solid);
        // With nothing to bear on, no further than its top edge meets the top step's top,
        // so it does not stand up past the flight into the air over it. Where a deck stands
        // behind, its head is cut level with the deck as it is drawn (`timber::plank`).
        let up = if bears || rise.y < 1e-3 {
            up
        } else {
            let top = (high.top + 1) as f32 * CELL_M;
            let edge_over = level * STRINGER_M.1 * 0.5;
            up.min(((top - (b.y + edge_over)) / rise.y).max(0.0))
        };
        let (foot, head) = (a - rise * down, b + rise * up);
        let seed = hash::of(&[line as u32, sign as u32, out as u32, 23]);
        layout.pieces.push(Piece::Plank {
            from: Place::near(foot),
            to: Place::near(head),
            wide: ticks(STRINGER_M.0),
            high: ticks(STRINGER_M.1),
            seed,
        });
        let run = Vec3::new(head.x - foot.x, 0.0, head.z - foot.z).length();
        if run < 1e-3 {
            continue;
        }
        // Where posts stand along its run from its foot: evenly between its bearings, as
        // many as keep each span within a stringer's; and under its head if it bears on
        // nothing there.
        let spans = (run / STRINGER_SPAN_M).ceil().max(1.0) as i32;
        let mut along: Vec<f32> = (1..spans).map(|k| run * k as f32 / spans as f32).collect();
        if !bears {
            along.push(run - width * 0.5);
        }
        for (n, at_run) in along.into_iter().enumerate() {
            let at = foot + (head - foot) * (at_run / run);
            // The underside over the post's upper face, so its top meets the stringer with
            // no gap; the stringer, thinner than the post, hides the rest of its top.
            let rising = rise.y / level;
            let under = at.y - STRINGER_M.1 * 0.5 / level + rising * width * 0.5;
            if under - floor < POSTS_FROM_M {
                continue;
            }
            let mut post = runs::log(
                Vec3::new(at.x, floor, at.z),
                Vec3::new(at.x, under, at.z),
                Split::Whole,
                hash::of(&[seed, n as u32, 0x9057]),
                None,
            );
            runs::set_role(&mut post, Some(Role::Jamb));
            layout.pieces.push(post);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::lay;
    use terra_build::kit::{Dress, Profile};
    use terra_build::pieces::Material;
    use terra_build::{BuildOp, Cube};

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
    fn a_cut_cube_with_something_on_it_is_still_drawn() {
        // A stair and a slope on the ground, each with a whole cube set on it.
        let mut base = Base::default();
        let put = |base: &mut Base, at: [i32; 3], shape: Shape| {
            base.apply(BuildOp::Fill {
                min: at,
                max: at,
                cube: Some(Cube {
                    shape,
                    ..Cube::new(kit().kind(), Fill::Solid)
                }),
            });
        };
        put(
            &mut base,
            [0, 0, 0],
            Shape::Stair(terra_build::Facing::North),
        );
        put(
            &mut base,
            [4, 0, 0],
            Shape::Slope(terra_build::Facing::North),
        );
        put(&mut base, [0, 1, 0], Shape::Whole);
        put(&mut base, [4, 1, 0], Shape::Whole);
        let layout = lay(&base, &kit());
        let under = |x: i32| {
            layout
                .pieces
                .iter()
                .filter(|p| matches!(p, Piece::Plank { .. }) && p.anchor() == IVec3::new(x, 0, 0))
                .count()
        };
        assert_eq!(under(0), 2, "the stair's two treads");
        assert!(under(4) >= 2, "the slope's boards");
    }
}
