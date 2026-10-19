//! A timber kind's cubes cut to a shape (D193), other than a flight's stairs and slopes,
//! which `stairs` lays: halves as low decks, boards on joists half a cube up with a rim
//! round their open sides (`floors`); and posts, each column of them one upright of the
//! kit's timber as its file dresses a post, on a pad of boards where it stands on something
//! else, and under a bolster across its top where something rests on it.

use super::roles::Role;
use super::{Layout, Piece, Place, Scope, Split, Walls, runs, ticks};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;
use terra_build::kit::{Kit, Kits};
use terra_build::{CELL_M, Facing, Fill, Shape};
use terra_gen::hash;

/// A post's pad: how far across, as a fraction of the cube, and how thick.
const PAD_ACROSS: f32 = 0.8;
const PAD_M: f32 = 0.06;

/// Lays a timber kind's halves and posts.
pub(super) fn lay(walls: &Walls, kit: &Kit, _kits: &Kits, scope: Scope, layout: &mut Layout) {
    let base = walls.base;
    let kind = kit.kind();
    let solid = |cube: &terra_build::Cube| cube.kind == kind && cube.fill == Fill::Solid;
    let is = |at: IVec3, shape: Shape| base.cube(at).is_some_and(|c| solid(&c) && c.shape == shape);
    // Its cubes in the scope, not the base's: a castle's are a hundred thousand.
    let mine = scope.cubes_where(base, None, solid);
    // Every run of halves joined side by side on its layer with one in the scope, whole,
    // as laying the whole base finds them.
    let mut halves: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut next: Vec<IVec3> = mine
        .iter()
        .filter(|(_, cube)| cube.shape == Shape::Half)
        .map(|&(at, _)| at)
        .collect();
    halves.extend(next.iter().map(|at| at.to_array()));
    while let Some(at) = next.pop() {
        for side in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
            let beside = at + side;
            if !halves.contains(&beside.to_array()) && is(beside, Shape::Half) {
                halves.insert(beside.to_array());
                next.push(beside);
            }
        }
    }
    if !halves.is_empty()
        && kit
            .textures
            .contains_key(&terra_build::pieces::Material::Floor)
    {
        super::floors::regions(walls, kit, &halves, CELL_M * 0.5, scope, layout);
    }
    // Each column of posts reaching into the scope, from its foot.
    let mut feet: BTreeSet<[i32; 3]> = BTreeSet::new();
    for &(at, cube) in &mine {
        if cube.shape != Shape::Post {
            continue;
        }
        let mut foot = at;
        while is(foot - IVec3::Y, Shape::Post) {
            foot -= IVec3::Y;
        }
        feet.insert(foot.to_array());
    }
    for at in feet {
        let at = IVec3::from_array(at);
        let mut top = at;
        while is(top + IVec3::Y, Shape::Post) {
            top += IVec3::Y;
        }
        if !scope.reaches(at, top) {
            continue;
        }
        post(walls, kit, at, top, layout);
    }
    // The shapes of D231, each boarded as a box of planks.
    for &(at, cube) in &mine {
        if boarded(cube.shape) {
            board(base, at, cube.shape, layout);
        }
    }
}

/// Whether a timber kind lays a shape as a box of boards: the upper half, the inverted
/// stair and slope, and the corner slopes (D231).
fn boarded(shape: Shape) -> bool {
    matches!(
        shape,
        Shape::UpperHalf
            | Shape::InvertedStair(_)
            | Shape::InvertedSlope(_)
            | Shape::OuterCorner(_)
            | Shape::InnerCorner(_)
    )
}

/// How thick a box's boards are, how many courses clad a side and boards cover a top.
const BOX_BOARD_M: f32 = 0.04;
const BOX_COURSES: i32 = 4;
const BOX_BOARDS: i32 = 4;

/// A cube of a shape as a timber kind boards it (D231): its top a deck of boards, or where
/// it is a ramp or a corner of two, boards up each ramp cut at the crease; its underside
/// boarded the same, flat or raking; and each open side clad in courses of boards, each as
/// long as the shape is wide at its height there, so the box is closed whatever its cut.
///
/// Its corners are butt joints, as a carpenter boxes a cube: the courses of a side facing
/// east or west run through to the cube's edges, and every other board that ends where a
/// side is clad, a course of a side facing north or south, a board of its top or its
/// underside, a riser, stops against that cladding's inside face, so no board's sawn end
/// lies in the plane of another's face (the voxel contract's gap 10).
fn board(base: &terra_build::Base, at: IVec3, shape: Shape, layout: &mut Layout) {
    let low = at.as_vec3() * CELL_M;
    // A point of the cube in fractions of it: east, up, south.
    let point = |p: Vec3| low + p * CELL_M;
    let seed = |salt: u32| hash::of(&[at.x as u32, at.y as u32, at.z as u32, 0xb0c, salt]);
    let open = |step: IVec3| {
        base.cube(at + step)
            .is_none_or(|cube| cube.fill != Fill::Solid)
    };
    let width = CELL_M / BOX_BOARDS as f32;
    let half_thick = BOX_BOARD_M * 0.5;
    let thick = BOX_BOARD_M / CELL_M;
    let depth = 1.0 - half_thick / CELL_M;
    // Where each open side is clad, course by course: from and to how far along it.
    let spans: [[Option<(f32, f32)>; BOX_COURSES as usize]; 4] = Facing::ALL.map(|side| {
        std::array::from_fn(|course| {
            if !open(side.step()) {
                return None;
            }
            let along = side.turned(1);
            let h = (course as f32 + 0.5) / BOX_COURSES as f32;
            let solid = |t: f32| {
                let (u, v) = uv(side, 1.0, along, t);
                shape.bottom(u, v) <= h && h <= shape.top(u, v)
            };
            let inside: Vec<f32> = (0..=16)
                .map(|i| i as f32 / 16.0)
                .filter(|&t| solid(t))
                .collect();
            let (&first, &last) = (inside.first()?, inside.last()?);
            (last - first >= 0.1).then_some((first, last))
        })
    });
    // Whether a side's cladding covers a point of its face at its height.
    let clad = |side: Facing, p: Vec3| {
        let course = ((p.y * BOX_COURSES as f32) as i32).clamp(0, BOX_COURSES - 1) as usize;
        let t = side.turned(1).toward(p.x, p.z);
        spans[side as usize][course].is_some_and(|(a, b)| a - 1e-3 <= t && t <= b + 1e-3)
    };
    // An end of a level board on a face of the cube clad anywhere across the board's width
    // (`across` each way), its square end butting that face: brought back along the board
    // to the cladding's inside face.
    let butted = |end: Vec3, other: Vec3, across: f32| {
        let axis = end - other;
        let side = if axis.y.abs() > 1e-4 {
            None
        } else if axis.z.abs() < 1e-4 && end.x >= 1.0 - 1e-3 && axis.x > 0.0 {
            Some(Facing::East)
        } else if axis.z.abs() < 1e-4 && end.x <= 1e-3 && axis.x < 0.0 {
            Some(Facing::West)
        } else if axis.x.abs() < 1e-4 && end.z >= 1.0 - 1e-3 && axis.z > 0.0 {
            Some(Facing::South)
        } else if axis.x.abs() < 1e-4 && end.z <= 1e-3 && axis.z < 0.0 {
            Some(Facing::North)
        } else {
            None
        };
        let sideways = Vec3::new(axis.z, 0.0, -axis.x).normalize_or_zero();
        match side {
            Some(side)
                if [-across, 0.0, across]
                    .iter()
                    .any(|&a| clad(side, end + sideways * a)) =>
            {
                end - axis.normalize() * thick
            }
            _ => end,
        }
    };
    let mut salt = 0u32;
    // A board between two points of the cube; one running through, a course of a side
    // facing east or west, keeps its ends where they are.
    let mut plank =
        |layout: &mut Layout, from: Vec3, to: Vec3, wide: f32, high: f32, through: bool| {
            salt += 1;
            let (from, to) = if through {
                (from, to)
            } else {
                let across = wide * 0.5 / CELL_M;
                (butted(from, to, across), butted(to, from, across))
            };
            layout.pieces.push(Piece::Plank {
                from: Place::near(point(from)),
                to: Place::near(point(to)),
                wide: ticks(wide),
                high: ticks(high),
                seed: seed(salt),
            });
        };
    let at_uv = |(u, v): (f32, f32), h: f32| Vec3::new(u, h, v);
    // Boards laid side by side along `along` at a height lie between the claddings of the
    // sides along them: how far in from `side` the outer board's edge stops, at the inside
    // face of that side's cladding where any of it is clad there.
    let edge = |along: Facing, side: Facing, y: f32| {
        let clad_there = [0.1, 0.5, 0.9]
            .iter()
            .any(|&a| clad(side, at_uv(uv(along, a, side, 1.0), y)));
        if clad_there { thick } else { 0.0 }
    };
    // The top: a deck at the cube's top, or boards up the ramps of a corner.
    match shape {
        Shape::OuterCorner(facing) | Shape::InnerCorner(facing) => {
            let outer = matches!(shape, Shape::OuterCorner(_));
            // Each ramp's half of the cube: boards up toward its side, across toward the
            // other's, from the crease or the cube's edge to the other.
            for (rise, across) in [(facing, facing.turned(1)), (facing.turned(1), facing)] {
                for board in 0..BOX_BOARDS {
                    let t = (board as f32 + 0.5) / BOX_BOARDS as f32;
                    // How far up toward `rise` the board runs: to the crease at `t`.
                    let (a, b) = if outer { (0.0, t) } else { (t, 1.0) };
                    let ends = [a, b].map(|r| {
                        at_uv(uv(rise, r, across, t), r)
                            - Vec3::Y * half_thick * std::f32::consts::SQRT_2 / CELL_M
                    });
                    plank(layout, ends[0], ends[1], width - 0.008, BOX_BOARD_M, false);
                }
            }
        }
        _ if open(IVec3::Y) => {
            let y = 1.0 - half_thick / CELL_M;
            let (first, last) = (
                edge(Facing::East, Facing::North, y),
                1.0 - edge(Facing::East, Facing::South, y),
            );
            let each = (last - first) / BOX_BOARDS as f32;
            for board in 0..BOX_BOARDS {
                let v = first + (board as f32 + 0.5) * each;
                plank(
                    layout,
                    Vec3::new(0.0, y, v),
                    Vec3::new(1.0, y, v),
                    each * CELL_M - 0.008,
                    BOX_BOARD_M,
                    false,
                );
            }
        }
        _ => {}
    }
    // The underside: flat under an upper half and an inverted stair's front, raking under
    // an inverted slope; a riser up an inverted stair's back half.
    match shape {
        Shape::UpperHalf | Shape::InvertedStair(_) => {
            // Boards along `along`, side by side toward `rise` over so much of the cube:
            // all of an upper half, an inverted stair's front half across its width.
            let (along, rise, reach) = match shape {
                Shape::InvertedStair(facing) => (facing.turned(1), facing, 0.5),
                _ => (Facing::East, Facing::South, 1.0),
            };
            // Between the claddings of the sides along them: an outer board's edge stops
            // at a cladding's inside face, as a board's end does.
            let y = 0.5 + half_thick / CELL_M;
            let first = edge(along, rise.opposite(), y);
            let last = if reach < 1.0 {
                reach
            } else {
                1.0 - edge(along, rise, y)
            };
            let each = (last - first) / BOX_BOARDS as f32;
            for board in 0..BOX_BOARDS {
                let t = first + (board as f32 + 0.5) * each;
                plank(
                    layout,
                    at_uv(uv(along, 0.0, rise, t), y),
                    at_uv(uv(along, 1.0, rise, t), y),
                    each * CELL_M - 0.008,
                    BOX_BOARD_M,
                    false,
                );
            }
            if let Shape::InvertedStair(facing) = shape {
                // The riser: courses across the stair's middle, under its back half.
                for course in 0..BOX_COURSES / 2 {
                    let h = (course as f32 + 0.5) / BOX_COURSES as f32;
                    let side = facing.turned(1);
                    let r = 0.5 + half_thick / CELL_M;
                    plank(
                        layout,
                        at_uv(uv(side, 0.0, facing, r), h),
                        at_uv(uv(side, 1.0, facing, r), h),
                        BOX_BOARD_M,
                        CELL_M / BOX_COURSES as f32 - 0.006,
                        false,
                    );
                }
            }
        }
        Shape::InvertedSlope(facing) => {
            // Boards down the ramp, their undersides on it: a board's middle at `r`
            // toward the side, half its thickness out from the ramp. Its head stops under
            // the top's deck, its highest corner at the deck's underside, and its foot,
            // cut plumb as a sloping plank is, at the inside of the side's cladding.
            let across = facing.turned(1);
            let out = half_thick / CELL_M * std::f32::consts::FRAC_1_SQRT_2;
            for board in 0..BOX_BOARDS {
                let t = (board as f32 + 0.5) / BOX_BOARDS as f32;
                let middle = |r: f32| at_uv(uv(facing, r + out, across, t), 1.0 - r + out);
                let head = thick * (1.0 + std::f32::consts::FRAC_1_SQRT_2);
                let foot_clad = clad(facing, at_uv(uv(facing, 1.0, across, t), 0.0));
                let foot = if foot_clad { 1.0 - thick } else { 1.0 } - out;
                plank(
                    layout,
                    middle(head),
                    middle(foot),
                    width - 0.008,
                    BOX_BOARD_M,
                    false,
                );
            }
        }
        _ => {}
    }
    // The open sides, clad in courses, each as long as the shape is there: those facing
    // east or west through to the cube's edges, the others butting them.
    for side in Facing::ALL {
        let along = side.turned(1);
        let through = matches!(side, Facing::East | Facing::West);
        for course in 0..BOX_COURSES {
            let Some((first, last)) = spans[side as usize][course as usize] else {
                continue;
            };
            let h = (course as f32 + 0.5) / BOX_COURSES as f32;
            let ends = [first, last].map(|t| at_uv(uv(side, depth, along, t), h));
            plank(
                layout,
                ends[0],
                ends[1],
                BOX_BOARD_M,
                CELL_M / BOX_COURSES as f32 - 0.006,
                through,
            );
        }
    }
}

/// A point of a cube's footprint as fractions east and south, from how far it is toward
/// one side and toward another square to it.
fn uv(first: Facing, a: f32, second: Facing, b: f32) -> (f32, f32) {
    let (mut u, mut v) = (0.5, 0.5);
    for (facing, t) in [(first, a), (second, b)] {
        match facing {
            Facing::North => v = 1.0 - t,
            Facing::East => u = t,
            Facing::South => v = t,
            Facing::West => u = 1.0 - t,
        }
    }
    (u, v)
}

/// One post up a column of post cubes from `foot` to `top`: on a pad where it stands on
/// something; up to the deck its top cube is laid into, or the deck on it, ending under the
/// deck's bearer (`floors`); else under a bolster across its top where something rests on
/// it, set to the bolster's real depth so the two meet.
fn post(walls: &Walls, kit: &Kit, foot: IVec3, top: IVec3, layout: &mut Layout) {
    let base = walls.base;
    // A top cube laid into a deck beside it is the deck's: the post ends under it.
    let in_deck = super::floors::post_in_deck(base, walls.kits, top).is_some();
    if in_deck && top == foot {
        return;
    }
    let top = if in_deck { top - IVec3::Y } else { top };
    let middle = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL_M;
    let (bottom, height) = (foot.y as f32 * CELL_M, (top.y + 1) as f32 * CELL_M);
    // A pad under it where it stands on anything, and the post on the pad.
    let stands = base
        .cube(foot - IVec3::Y)
        .is_some_and(|cube| cube.fill == Fill::Solid);
    let pad = if stands { PAD_M } else { 0.0 };
    if stands {
        layout.pieces.push(Piece::Plank {
            from: Place::near(
                middle.with_y(bottom + PAD_M * 0.5) - Vec3::X * CELL_M * PAD_ACROSS * 0.5,
            ),
            to: Place::near(
                middle.with_y(bottom + PAD_M * 0.5) + Vec3::X * CELL_M * PAD_ACROSS * 0.5,
            ),
            wide: ticks(CELL_M * PAD_ACROSS),
            high: ticks(PAD_M),
            seed: hash::of(&[foot.x as u32, foot.y as u32, foot.z as u32, 0x9ad]),
        });
    }
    // A bolster across its top, under what rests on it, the way that rests along; none
    // under a deck, whose bearer it carries.
    let above = top + IVec3::Y;
    let decked = in_deck || super::floors::decked(base, walls.kits, above).is_some();
    let carries = !decked
        && base
            .cube(above)
            .is_some_and(|cube| cube.fill == Fill::Solid && cube.shape != Shape::Post);
    let (_, bolster) = terra_build::pieces::section(kit, Some(Role::Crest));
    let bolster = bolster.min(CELL_M * 0.7);
    let shaft_top = if carries { height - bolster } else { height };
    let mut shaft = runs::log(
        middle.with_y(bottom + pad),
        middle.with_y(shaft_top),
        Split::Whole,
        hash::of(&[foot.x as u32, foot.y as u32, foot.z as u32, 0x905]),
        None,
    );
    runs::set_role(&mut shaft, Some(Role::Post));
    layout.pieces.push(shaft);
    if carries {
        let solid = |at: IVec3| base.cube(at).is_some_and(|cube| cube.fill == Fill::Solid);
        let along = if solid(above + IVec3::Z) || solid(above - IVec3::Z) {
            if solid(above + IVec3::X) || solid(above - IVec3::X) {
                Vec3::X
            } else {
                Vec3::Z
            }
        } else {
            Vec3::X
        };
        let centre = middle.with_y(height - bolster * 0.5);
        let reach = along * (CELL_M * 0.5 - 0.01);
        let mut cap = runs::log(
            centre - reach,
            centre + reach,
            Split::Whole,
            hash::of(&[foot.x as u32, top.y as u32, foot.z as u32, 0xb01]),
            None,
        );
        runs::set_role(&mut cap, Some(Role::Crest));
        layout.pieces.push(cap);
    }
}

#[cfg(test)]
mod tests {
    use super::super::lay;
    use super::super::tests::textured;
    use super::*;
    use terra_build::{Base, BuildOp, Cube};

    fn put(base: &mut Base, min: [i32; 3], max: [i32; 3], shape: Shape) {
        base.apply(BuildOp::Fill {
            min,
            max,
            cube: Some(Cube {
                shape,
                ..Cube::new(textured().kind(), Fill::Solid)
            }),
        });
    }

    #[test]
    fn a_post_is_one_upright_under_a_bolster_and_a_half_a_low_deck() {
        let mut base = Base::default();
        // A post five cubes high carrying a deck of halves on the floor above it, and a
        // run of halves on the ground beside it.
        put(&mut base, [0, 0, 0], [0, 4, 0], Shape::Post);
        put(&mut base, [-1, 5, -1], [1, 5, 1], Shape::Whole);
        put(&mut base, [2, 0, 3], [4, 0, 4], Shape::Half);
        let layout = lay(&base, &textured());
        let roles: Vec<Option<Role>> = layout
            .pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Log { role, .. } => Some(*role),
                _ => None,
            })
            .collect();
        assert_eq!(roles.iter().filter(|r| **r == Some(Role::Post)).count(), 1);
        assert_eq!(roles.iter().filter(|r| **r == Some(Role::Crest)).count(), 1);
        // No log lies across the post's cubes as a wall would.
        for piece in &layout.pieces {
            if let Piece::Log { from, to, role, .. } = piece
                && role.is_none()
            {
                panic!("a log laid as a wall: {from:?} {to:?}");
            }
        }
        // The halves' boards half a cube up, nowhere over a cube's top.
        let boards: Vec<f32> = layout
            .pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Plank { from, to, .. } if from.metres().x >= 1.0 && to.metres().x >= 1.0 => {
                    Some(from.metres().y)
                }
                _ => None,
            })
            .collect();
        assert!(!boards.is_empty());
        assert!(
            boards.iter().all(|&y| y < CELL_M * 0.5 + 0.01),
            "{boards:?}"
        );
    }
}
