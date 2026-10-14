//! A timber kind's cubes cut to a shape (D193), other than a flight's stairs and slopes,
//! which `stairs` lays: halves as low decks, boards on joists half a cube up with a rim
//! round their open sides (`floors`); and posts, each column of them one upright of the
//! kit's timber as its file dresses a post, on a pad of boards where it stands on something
//! else, and under a bolster across its top where something rests on it.

use super::roles::Role;
use super::{Layout, Piece, Place, Scope, Split, Walls, hash, runs, ticks};
use crate::kit::{Kit, Kits};
use crate::{CELL_M, Facing, Fill, Shape};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;

/// A post's pad: how far across, as a fraction of the cube, and how thick.
const PAD_ACROSS: f32 = 0.8;
const PAD_M: f32 = 0.06;

/// Lays a timber kind's halves and posts.
pub(super) fn lay(walls: &Walls, kit: &Kit, _kits: &Kits, scope: Scope, layout: &mut Layout) {
    let base = walls.base;
    let kind = kit.kind();
    let shaped = |shape: Shape| -> BTreeSet<[i32; 3]> {
        base.cubes()
            .filter(|(_, cube)| {
                cube.kind == kind && cube.fill == Fill::Solid && cube.shape == shape
            })
            .map(|(at, _)| at.to_array())
            .collect()
    };
    let halves = shaped(Shape::Half);
    if !halves.is_empty() && kit.textures.contains_key(&crate::mesh::Material::Floor) {
        super::floors::regions(base, kit, &halves, CELL_M * 0.5, scope, layout);
    }
    let posts = shaped(Shape::Post);
    for &at in &posts {
        let at = IVec3::from_array(at);
        // From the foot of each column.
        if posts.contains(&(at - IVec3::Y).to_array()) {
            continue;
        }
        let mut top = at;
        while posts.contains(&(top + IVec3::Y).to_array()) {
            top += IVec3::Y;
        }
        if !scope.reaches(at, top) {
            continue;
        }
        post(walls, kit, at, top, layout);
    }
    // The shapes of D231, each boarded as a box of planks.
    for (at, cube) in base.cubes() {
        if cube.kind == kind
            && cube.fill == Fill::Solid
            && boarded(cube.shape)
            && scope.reaches(at, at)
        {
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
fn board(base: &crate::Base, at: IVec3, shape: Shape, layout: &mut Layout) {
    let low = at.as_vec3() * CELL_M;
    let point = |u: f32, h: f32, v: f32| low + Vec3::new(u, h, v) * CELL_M;
    let seed = |salt: u32| hash(&[at.x as u32, at.y as u32, at.z as u32, 0xb0c, salt]);
    let mut salt = 0u32;
    let mut plank = |layout: &mut Layout, from: Vec3, to: Vec3, wide: f32, high: f32| {
        salt += 1;
        layout.pieces.push(Piece::Plank {
            from: Place::near(from),
            to: Place::near(to),
            wide: ticks(wide),
            high: ticks(high),
            seed: seed(salt),
        });
    };
    let open = |step: IVec3| {
        base.cube(at + step)
            .is_none_or(|cube| cube.fill != Fill::Solid)
    };
    let width = CELL_M / BOX_BOARDS as f32;
    let half_thick = BOX_BOARD_M * 0.5;
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
                        let (u, v) = uv(rise, r, across, t);
                        point(u, r, v) - Vec3::Y * half_thick * std::f32::consts::SQRT_2
                    });
                    plank(layout, ends[0], ends[1], width - 0.008, BOX_BOARD_M);
                }
            }
        }
        _ if open(IVec3::Y) => {
            for board in 0..BOX_BOARDS {
                let v = (board as f32 + 0.5) / BOX_BOARDS as f32;
                let y = 1.0 - half_thick / CELL_M;
                plank(
                    layout,
                    point(0.0, y, v),
                    point(1.0, y, v),
                    width - 0.008,
                    BOX_BOARD_M,
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
            for board in 0..BOX_BOARDS {
                let t = (board as f32 + 0.5) / BOX_BOARDS as f32 * reach;
                let (u0, v0) = uv(along, 0.0, rise, t);
                let (u1, v1) = uv(along, 1.0, rise, t);
                let y = 0.5 + half_thick / CELL_M;
                plank(
                    layout,
                    point(u0, y, v0),
                    point(u1, y, v1),
                    width * reach - 0.008,
                    BOX_BOARD_M,
                );
            }
            if let Shape::InvertedStair(facing) = shape {
                // The riser: courses across the stair's middle, under its back half.
                for course in 0..BOX_COURSES / 2 {
                    let h = (course as f32 + 0.5) / BOX_COURSES as f32;
                    let side = facing.turned(1);
                    let (u0, v0) = uv(side, 0.0, facing, 0.5 + half_thick / CELL_M);
                    let (u1, v1) = uv(side, 1.0, facing, 0.5 + half_thick / CELL_M);
                    plank(
                        layout,
                        point(u0, h, v0),
                        point(u1, h, v1),
                        BOX_BOARD_M,
                        CELL_M / BOX_COURSES as f32 - 0.006,
                    );
                }
            }
        }
        Shape::InvertedSlope(facing) => {
            let across = facing.turned(1);
            for board in 0..BOX_BOARDS {
                let t = (board as f32 + 0.5) / BOX_BOARDS as f32;
                let ends = [(0.0f32, 1.0f32), (1.0, 0.0)].map(|(r, h)| {
                    let (u, v) = uv(facing, r, across, t);
                    point(u, h, v) + Vec3::Y * half_thick * std::f32::consts::SQRT_2
                });
                plank(layout, ends[0], ends[1], width - 0.008, BOX_BOARD_M);
            }
        }
        _ => {}
    }
    // The open sides, clad in courses, each as long as the shape is there.
    for side in Facing::ALL {
        if !open(side.step()) {
            continue;
        }
        let along = side.turned(1);
        for course in 0..BOX_COURSES {
            let h = (course as f32 + 0.5) / BOX_COURSES as f32;
            // Where along the side the shape stands at this height.
            let solid = |t: f32| {
                let (u, v) = uv(side, 1.0, along, t);
                shape.bottom(u, v) <= h && h <= shape.top(u, v)
            };
            let samples: Vec<f32> = (0..=16).map(|i| i as f32 / 16.0).collect();
            let inside: Vec<f32> = samples.iter().copied().filter(|&t| solid(t)).collect();
            let (Some(&first), Some(&last)) = (inside.first(), inside.last()) else {
                continue;
            };
            if last - first < 0.1 {
                continue;
            }
            let depth = 1.0 - half_thick / CELL_M;
            let ends = [first, last].map(|t| {
                let (u, v) = uv(side, depth, along, t);
                point(u, h, v)
            });
            plank(
                layout,
                ends[0],
                ends[1],
                BOX_BOARD_M,
                CELL_M / BOX_COURSES as f32 - 0.006,
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

/// One post up a column of post cubes from `foot` to `top`.
fn post(walls: &Walls, kit: &Kit, foot: IVec3, top: IVec3, layout: &mut Layout) {
    let base = walls.base;
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
            seed: hash(&[foot.x as u32, foot.y as u32, foot.z as u32, 0x9ad]),
        });
    }
    // A bolster across its top, under what rests on it, the way that rests along.
    let above = top + IVec3::Y;
    let carries = base
        .cube(above)
        .is_some_and(|cube| cube.fill == Fill::Solid && cube.shape != Shape::Post);
    let bolster = kit
        .dress
        .member(Role::Crest)
        .map_or(kit.diameter, |(_, scale, _)| kit.diameter * scale)
        .min(CELL_M * 0.7);
    let shaft_top = if carries { height - bolster } else { height };
    let mut shaft = runs::log(
        middle.with_y(bottom + pad),
        middle.with_y(shaft_top),
        Split::Whole,
        hash(&[foot.x as u32, foot.y as u32, foot.z as u32, 0x905]),
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
            hash(&[foot.x as u32, top.y as u32, foot.z as u32, 0xb01]),
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
    use crate::{Base, BuildOp, Cube};

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
