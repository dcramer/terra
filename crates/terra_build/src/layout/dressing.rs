//! A kit's dress laid by role (D215, research/role-pieces.md): for every edge of its cubes
//! with a role (`roles`), the piece its file dresses that role with, belonging to that edge's
//! cube: a profile swept along it (`Piece::Trim`), or corner stones up it a course at a time
//! (`Piece::Corner`), those under a foot's plinth left out. Long members by role are laid
//! with the rest of a timber kind's (`runs`), and drawn by `timber`.
//!
//! A cube cut to a shape (D193) dresses its own lines with bands (`Piece::Band`) of the
//! same profiles: a stair a nose on each of its two treads, a half a coping round its open
//! top, a slope a coping raking up each open side, and a pillar a plinth round its base and
//! a coping round its cap.

use super::roles::{Beyond, Dir, Dressed, Edge, End, Role, Roles};
use super::{Piece, Place, Scope, TICKS, hash};
use crate::kit::{Dress, Kit, Kits};
use crate::{Base, Fill, Shape};
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
        shaped(base, &roles, kit, cube, pieces);
    }
}

/// The bands a cube cut to a shape is dressed with, as the kit dresses their roles.
fn shaped(base: &Base, roles: &Roles, kit: &Kit, cube: IVec3, pieces: &mut Vec<Piece>) {
    let Some(this) = base
        .cube(cube)
        .filter(|c| c.kind == kit.kind() && c.fill == Fill::Solid && c.shape.is_cut())
    else {
        return;
    };
    let shape_at = |at: IVec3| {
        base.cube(at)
            .filter(|c| c.kind == kit.kind() && c.fill == Fill::Solid)
            .map(|c| c.shape)
    };
    let open = |at: IVec3| roles.beyond(at).open();
    let half = TICKS / 2;
    let mut band = |role: Role, (from, to): ([i32; 3], [i32; 3]), side: Dir, ends: [End; 2]| {
        if !dresses_role(kit, role) {
            return;
        }
        pieces.push(Piece::Band {
            cube,
            role,
            from: Place(from),
            to: Place(to),
            side,
            ends,
            seed: hash(&[
                cube.x as u32,
                cube.y as u32,
                cube.z as u32,
                role as u32,
                side as u32,
                (from[1] - cube.y * TICKS) as u32,
            ]),
        });
    };
    // A line of the cube's moved by so many ticks.
    let moved = |(from, to): ([i32; 3], [i32; 3]), by: IVec3| {
        (
            (IVec3::from_array(from) + by).to_array(),
            (IVec3::from_array(to) + by).to_array(),
        )
    };
    let top = |side: Dir| {
        super::roles::line(
            cube,
            Edge {
                role: Role::Crest,
                side,
                other: Dir::Up,
            },
        )
    };
    // Each end of a level line along `side`'s face: run on into the next cube's, if it is
    // of this shape and its face that way is open too; else turned round this cube's
    // corner where its face that way is open; else stopped.
    let ends = |side: Dir, corners: bool| {
        let axis = 3 - side.axis() - 1;
        [-1, 1].map(|toward| {
            let along = Dir::of(axis, toward);
            let next = cube + along.step();
            if shape_at(next) == Some(this.shape) && open(next + side.step()) {
                End::On
            } else if corners && open(next) {
                End::Out
            } else {
                End::Square
            }
        })
    };
    match this.shape {
        Shape::Half => {
            for side in Dir::LEVEL {
                if open(cube + side.step()) {
                    band(
                        Role::Crest,
                        moved(top(side), IVec3::new(0, -half, 0)),
                        side,
                        ends(side, true),
                    );
                }
            }
        }
        Shape::Stair(facing) => {
            let front = Dir::facing(facing).opposite();
            let ends = ends(front, false);
            if open(cube + front.step()) {
                band(
                    Role::Nose,
                    moved(top(front), IVec3::new(0, -half, 0)),
                    front,
                    ends,
                );
            }
            if shape_at(cube + IVec3::Y).is_none() {
                let back = Dir::facing(facing).step() * half;
                band(Role::Nose, moved(top(front), back), front, ends);
            }
        }
        Shape::Slope(facing) => {
            let rise = Dir::facing(facing);
            for side in Dir::LEVEL {
                if side.axis() == rise.axis()
                    || !open(cube + side.step())
                    || shape_at(cube + side.step()) == Some(this.shape)
                {
                    continue;
                }
                // Up the face from the slope's foot at its front to its top at the back.
                let (mut low, mut high) = top(side);
                let a = rise.axis();
                let (front, back) = if rise.sign() > 0 {
                    (cube[a] * TICKS, (cube[a] + 1) * TICKS)
                } else {
                    ((cube[a] + 1) * TICKS, cube[a] * TICKS)
                };
                low[a] = front;
                low[1] = cube.y * TICKS;
                high[a] = back;
                high[1] = (cube.y + 1) * TICKS;
                band(Role::Crest, (low, high), side, [End::Square; 2]);
            }
        }
        Shape::Post => {
            let Some(Dress::Pillar { width }) = kit.dress.roles.get(&Role::Post) else {
                return;
            };
            let inset = (TICKS - super::ticks(*width)) / 2;
            for (role, other, end) in [(Role::Foot, Dir::Down, -1), (Role::Crest, Dir::Up, 1)] {
                if shape_at(cube + IVec3::Y * end) == Some(Shape::Post) {
                    continue;
                }
                for side in Dir::LEVEL {
                    let (from, to) = super::roles::line(cube, Edge { role, side, other });
                    let axis = 3 - side.axis() - 1;
                    let mut shift = -side.step() * inset;
                    shift[1] = 0;
                    let (mut from, mut to) = moved((from, to), shift);
                    from[axis] += inset;
                    to[axis] -= inset;
                    band(role, (from, to), side, [End::Out; 2]);
                }
            }
        }
        Shape::Whole => {}
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
        Dress::Member { .. } | Dress::Pillar { .. } => None,
    }
}

/// Whether a kit dresses a role with a piece of its own.
pub(super) fn dresses_role(kit: &Kit, role: Role) -> bool {
    matches!(
        kit.dress.roles.get(&role),
        Some(Dress::Profile { .. } | Dress::Corners { .. })
    )
}
