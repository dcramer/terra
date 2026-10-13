//! A kit's dress laid by role (D215, research/role-pieces.md): for every edge of its cubes
//! with a role (`roles`), the piece its file dresses that role with, belonging to that edge's
//! cube: a profile swept along it (`Piece::Trim`), or corner stones up it a course at a time
//! (`Piece::Corner`), those under a foot's plinth left out. Long members by role are laid
//! with the rest of a timber kind's (`runs`), and drawn by `timber`.

use super::roles::{Beyond, Dressed, Role, Roles};
use super::{Piece, Scope, TICKS, hash};
use crate::Base;
use crate::kit::{Dress, Kit, Kits};
use glam::IVec3;

/// Whether a kit dresses any role with pieces of its own.
pub(super) fn dresses(kit: &Kit) -> bool {
    kit.dress
        .roles
        .values()
        .any(|dress| matches!(dress, Dress::Profile { .. } | Dress::Corners { .. }))
}

/// Lays the dress of every cube of a kit's kind in a scope.
pub(super) fn lay(base: &Base, kit: &Kit, kits: &Kits, scope: Scope, pieces: &mut Vec<Piece>) {
    if !dresses(kit) {
        return;
    }
    let roles = Roles::new(base, kit.kind(), kits);
    let cubes: Vec<IVec3> = scope
        .cubes(base, None)
        .into_iter()
        .filter(|(_, cube)| cube.kind == kit.kind())
        .map(|(at, _)| at)
        .collect();
    for cube in cubes {
        for dressed in roles.dressed(cube) {
            if let Some(piece) = piece(&roles, kit, &dressed) {
                pieces.push(piece);
            }
        }
    }
}

/// The piece a kit dresses an edge with, if any.
fn piece(roles: &Roles, kit: &Kit, dressed: &Dressed) -> Option<Piece> {
    let Dressed {
        cube,
        edge,
        ends,
        lift,
        sink,
    } = *dressed;
    let seed = hash(&[
        cube.x as u32,
        cube.y as u32,
        cube.z as u32,
        edge.role as u32,
        edge.side as u32,
        edge.other as u32,
    ]);
    match kit.dress.roles.get(&edge.role)? {
        Dress::Profile { .. } => Some(Piece::Trim {
            cube,
            role: edge.role,
            side: edge.side,
            other: edge.other,
            ends,
            lift,
            sink,
            seed,
        }),
        Dress::Corners { .. } => {
            // A jamb's stones lie long and short along the wall's face, their other side
            // the reveal; a quoin's turn from one face to the other.
            let into_hole = roles.beyond(cube + edge.side.step()) == Beyond::Hole;
            let faces = if into_hole {
                [edge.other, edge.side]
            } else {
                [edge.side, edge.other]
            };
            // None under a foot's plinth, on either face, here or a cube down.
            let mut plinth = i32::MIN;
            for under in [cube, cube - IVec3::Y] {
                for foot in roles.dressed(under) {
                    if foot.edge.role == Role::Foot
                        && faces.contains(&foot.edge.side)
                        && dresses_role(kit, Role::Foot)
                    {
                        plinth = plinth.max(under.y * TICKS + foot.lift + TICKS / 2);
                    }
                }
            }
            let courses = [0, 1].map(|course| {
                cube.y * TICKS + course * TICKS / 2 >= plinth.saturating_sub(TICKS / 8)
            });
            courses.contains(&true).then_some(Piece::Corner {
                cube,
                role: edge.role,
                faces,
                courses,
                seed,
            })
        }
        Dress::Member { .. } => None,
    }
}

/// Whether a kit dresses a role with a piece of its own.
pub(super) fn dresses_role(kit: &Kit, role: Role) -> bool {
    matches!(
        kit.dress.roles.get(&role),
        Some(Dress::Profile { .. } | Dress::Corners { .. })
    )
}
