//! A timber kind's cubes cut to a shape (D193), other than a flight's stairs and slopes,
//! which `stairs` lays: halves as low decks, boards on joists half a cube up with a rim
//! round their open sides (`floors`); and posts, each column of them one upright of the
//! kit's timber as its file dresses a post, on a pad of boards where it stands on something
//! else, and under a bolster across its top where something rests on it.

use super::roles::Role;
use super::{Layout, Piece, Place, Scope, Split, Walls, hash, runs, ticks};
use crate::kit::{Kit, Kits};
use crate::{CELL_M, Fill, Shape};
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
