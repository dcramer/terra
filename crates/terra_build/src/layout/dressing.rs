//! A kit's dress laid by role (D215, research/role-pieces.md): for every edge of its cubes
//! with a role (`roles`), the piece its file dresses that role with, belonging to that edge's
//! cube: a profile swept along it (`Piece::Trim`), or corner stones up it a course at a time
//! (`Piece::Corner`), those under a foot's plinth left out. Long members by role are laid
//! with the rest of a timber kind's (`runs`), and drawn by `timber`.
//!
//! An opening's head and sill are its own, not each cube's (D236): however irregular its
//! hole, one lintel runs across its full width over its highest row and one sill under its
//! lowest, as a builder bridges a hole with one beam, each a run of its cubes' edges laid
//! on as one; its steps between have none.
//!
//! A cube cut to a shape (D193) dresses its own lines with bands (`Piece::Band`) of the
//! same profiles: a stair a nose on each of its two treads, a half a coping round its open
//! top, a slope a coping raking up each open side, and a pillar a plinth round its base and
//! a coping round its cap.

use super::roles::{Beyond, Dir, Dressed, Edge, End, Role, Roles};
use super::{Piece, Place, Scope, TICKS};
use crate::kit::{Dress, Kit, Kits};
use crate::{Base, Fill, Shape};
use glam::IVec3;
use std::collections::BTreeSet;
use terra_gen::hash;

/// Whether a kit dresses any role with pieces of its own.
pub(super) fn dresses(kit: &Kit) -> bool {
    kit.dress
        .roles
        .values()
        .any(|dress| matches!(dress, Dress::Profile { .. } | Dress::Corners { .. }))
}

/// What lays a band: its role, its line, the face it looks out of and its ends.
type BandFn<'a> = dyn FnMut(Role, ([i32; 3], [i32; 3]), Dir, [End; 2]) + 'a;

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
    let merged = |role: Role| matches!(kit.dress.roles.get(&role), Some(Dress::Profile { .. }));
    for cube in cubes {
        for dressed in roles.dressed(cube) {
            // An opening's head and sill are laid whole, below.
            let edge = dressed.edge;
            if matches!(edge.role, Role::Head | Role::Sill)
                && merged(edge.role)
                && roles.beyond(cube + edge.other.step()) == Beyond::Hole
            {
                continue;
            }
            if let Some(piece) = piece(&roles, kit, &dressed) {
                pieces.push(piece);
            }
        }
        shaped(base, &roles, kit, kits, cube, pieces);
    }
    for role in [Role::Head, Role::Sill] {
        if merged(role) {
            across_openings(base, &roles, kit, role, scope, pieces);
        }
    }
}

/// How far round a scope openings are looked for, in cubes: more than one is across, so
/// one reaching into it is found whole.
const OPENING_REACH: i32 = 16;

/// Each opening's one lintel (`Role::Head`) or sill (`Role::Sill`) on each face it shows on:
/// over its highest row of holes, or under its lowest, across the opening's full width, laid
/// as a run of trims on the cubes there, which belong to those cubes. None where that row has
/// no wall over or under it, as a doorway's foot.
fn across_openings(
    base: &Base,
    roles: &Roles,
    kit: &Kit,
    role: Role,
    scope: Scope,
    pieces: &mut Vec<Piece>,
) {
    let kind = kit.kind();
    let hole = |at: IVec3| {
        base.cube(at)
            .is_some_and(|cube| cube.kind == kind && super::drawn_fill(&cube) != Fill::Solid)
    };
    let wide = Scope {
        low: scope.low - IVec3::splat(OPENING_REACH),
        high: scope.high + IVec3::splat(OPENING_REACH),
    };
    let mut left: BTreeSet<[i32; 3]> = wide
        .cubes(base, None)
        .into_iter()
        .filter(|&(at, _)| hole(at))
        .map(|(at, _)| at.to_array())
        .collect();
    let (other, toward) = if role == Role::Head {
        (Dir::Down, 1)
    } else {
        (Dir::Up, -1)
    };
    while let Some(&first) = left.iter().next() {
        // The opening: its holes, joined face to face.
        left.remove(&first);
        let mut opening = vec![IVec3::from_array(first)];
        let mut i = 0;
        while i < opening.len() {
            for dir in Dir::ALL {
                let next = opening[i] + dir.step();
                if left.remove(&next.to_array()) {
                    opening.push(next);
                }
            }
            i += 1;
        }
        let Some(y) = (if role == Role::Head {
            opening.iter().map(|h| h.y).max()
        } else {
            opening.iter().map(|h| h.y).min()
        }) else {
            continue;
        };
        let row = y + toward;
        for side in Dir::LEVEL {
            let edge = Edge { role, side, other };
            // The face's layer, from an edge of the kind over or under the row's holes.
            let Some(layer) = opening
                .iter()
                .filter(|h| h.y == y)
                .map(|&h| h + IVec3::Y * toward)
                .find(|&at| roles.edges(at).contains(&edge))
                .map(|at| at[side.axis()])
            else {
                continue;
            };
            let u_axis = 2 - side.axis();
            let (low, high) = opening.iter().fold((i32::MAX, i32::MIN), |(l, h), at| {
                (l.min(at[u_axis]), h.max(at[u_axis]))
            });
            let cube_at = |u: i32| {
                let mut at = IVec3::new(0, row, 0);
                at[u_axis] = u;
                at[side.axis()] = layer;
                at
            };
            let bears = |at: IVec3| {
                base.cube(at).is_some_and(|cube| {
                    cube.kind == kind
                        && super::drawn_fill(&cube) == Fill::Solid
                        && !cube.shape.is_cut()
                }) && roles.beyond(at + side.step()).open()
            };
            let mut u = low;
            while u <= high {
                if !bears(cube_at(u)) {
                    u += 1;
                    continue;
                }
                let start = u;
                while u < high && bears(cube_at(u + 1)) {
                    u += 1;
                }
                for along in start..=u {
                    let cube = cube_at(along);
                    if !scope.reaches(cube, cube) {
                        continue;
                    }
                    // Each end stops square, borne on the wall past it, but where the wall
                    // ends there over the opening and the lintel turns its own outside corner
                    // onto the next face's: mitred out to that one, as a coping is, not run
                    // on past the corner into the air and through the other.
                    let end = |toward: i32| {
                        let turned = Edge {
                            side: Dir::of(u_axis, toward),
                            ..edge
                        };
                        if roles.edges(cube).contains(&turned) {
                            End::Out
                        } else {
                            End::Square
                        }
                    };
                    let ends = [
                        if along == start { end(-1) } else { End::On },
                        if along == u { end(1) } else { End::On },
                    ];
                    pieces.push(Piece::Trim {
                        cube,
                        role,
                        side,
                        other,
                        ends,
                        lift: 0,
                        sink: 0,
                        seed: hash::of(&[
                            cube.x as u32,
                            cube.y as u32,
                            cube.z as u32,
                            role as u32,
                            side as u32,
                            other as u32,
                        ]),
                    });
                }
                u += 1;
            }
        }
    }
}

/// The bands a cube cut to a shape is dressed with, as the kit dresses their roles.
fn shaped(
    base: &Base,
    roles: &Roles,
    kit: &Kit,
    kits: &Kits,
    cube: IVec3,
    pieces: &mut Vec<Piece>,
) {
    // A post's top laid into a timber deck is the deck's, and the post's cap is under it.
    let in_deck = |at: IVec3| super::floors::post_in_deck(base, kits, at).is_some();
    if in_deck(cube) {
        return;
    }
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
            seed: hash::of(&[
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
    // How high a face's line is at each end, low end first, in ticks over the cube's foot:
    // its top's, or its underside's.
    let heights = |side: Dir, under: bool| {
        let axis = 3 - side.axis() - 1;
        [0.0f32, 1.0].map(|t| {
            let (mut u, mut v) = (0.5, 0.5);
            match side {
                Dir::East => u = 1.0,
                Dir::West => u = 0.0,
                Dir::South => v = 1.0,
                _ => v = 0.0,
            }
            if axis == 0 {
                u = t;
            } else {
                v = t;
            }
            let h = if under {
                this.shape.bottom(u, v)
            } else {
                this.shape.top(u, v)
            };
            (h * TICKS as f32).round() as i32
        })
    };
    // A planed top or underside (D231): a band along each open side's line, level or
    // raking, as a half's coping, a slope's or a corner's raking up its sides, or under an
    // upper half or an inverted slope a fascia; a whole top a coping where it is open over.
    let planed = |role: Role, under: bool, band: &mut BandFn| {
        for side in Dir::LEVEL {
            if !open(cube + side.step()) || shape_at(cube + side.step()) == Some(this.shape) {
                continue;
            }
            let [a, b] = heights(side, under);
            let (mut from, mut to) = top(side);
            from[1] = cube.y * TICKS + a;
            to[1] = cube.y * TICKS + b;
            match (a, b) {
                (0, 0) if !under => {}
                (TICKS, TICKS) if under => {}
                _ if a == b => band(role, (from, to), side, ends(side, true)),
                _ => band(role, (from, to), side, [End::Square; 2]),
            }
        }
    };
    let crest_over = |band: &mut BandFn| {
        if open(cube + IVec3::Y) {
            for side in Dir::LEVEL {
                if open(cube + side.step()) && shape_at(cube + side.step()) != Some(this.shape) {
                    band(Role::Crest, top(side), side, ends(side, true));
                }
            }
        }
    };
    match this.shape {
        shape if shape.planed_top() && !matches!(shape, Shape::Slope(_)) => {
            planed(Role::Crest, false, &mut band);
        }
        Shape::UpperHalf | Shape::InvertedSlope(_) => {
            planed(Role::Soffit, true, &mut band);
            crest_over(&mut band);
        }
        Shape::InvertedStair(facing) => {
            // A stair upside down: a fascia under each of its two steps' fronts.
            let front = Dir::facing(facing).opposite();
            let ends = ends(front, false);
            let foot = super::roles::line(
                cube,
                Edge {
                    role: Role::Soffit,
                    side: front,
                    other: Dir::Down,
                },
            );
            if open(cube + front.step()) {
                band(
                    Role::Soffit,
                    moved(foot, IVec3::new(0, half, 0)),
                    front,
                    ends,
                );
            }
            if shape_at(cube - IVec3::Y).is_none() {
                let back = Dir::facing(facing).step() * half;
                band(Role::Soffit, moved(foot, back), front, ends);
            }
            crest_over(&mut band);
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
            // A slope's side shows over what is beside it where that is open, or a half of the
            // kind, whose top the slope rises past: the coping runs on over it down to the
            // ramp's foot, its foot bedded in the half, not stopping a cube up.
            let shows_over = |at: IVec3| open(at) || shape_at(at) == Some(Shape::Half);
            for side in Dir::LEVEL {
                if side.axis() == rise.axis()
                    || !shows_over(cube + side.step())
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
                // Running on into the same slope's coping in the cube down the ramp from it
                // and the one up it, a flight's; squared off where it stops. Squared at
                // every cube, the part of each stone set into the ramp was cut back at both
                // its ends, and the coping stepped at every stone.
                let runs_on = |next: IVec3| {
                    shape_at(next) == Some(this.shape)
                        && shows_over(next + side.step())
                        && shape_at(next + side.step()) != Some(this.shape)
                };
                let step = rise.step() + IVec3::Y;
                // Else mitred where it meets a level coping, over the bend at its head and
                // under it at its foot, as a coping is at any change of direction.
                let level = |at: IVec3| {
                    shape_at(at) == Some(Shape::Whole)
                        && open(at + IVec3::Y)
                        && open(at + side.step())
                };
                let foot = if runs_on(cube - step) {
                    End::On
                } else if level(cube - step) {
                    End::Under
                } else {
                    End::Square
                };
                let head = if runs_on(cube + step) {
                    End::On
                } else if level(cube + rise.step()) {
                    End::Over
                } else {
                    End::Square
                };
                band(Role::Crest, (low, high), side, [foot, head]);
            }
        }
        Shape::Post => {
            let Some(Dress::Pillar { width }) = kit.dress.roles.get(&Role::Post) else {
                return;
            };
            let inset = (TICKS - super::ticks(*width)) / 2;
            for (role, other, end) in [(Role::Foot, Dir::Down, -1), (Role::Crest, Dir::Up, 1)] {
                let next = cube + IVec3::Y * end;
                if shape_at(next) == Some(Shape::Post) && !in_deck(next) {
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
        _ => {}
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
    let seed = hash::of(&[
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
            // Each face it shares with the cube's other upright edge on it, if that has corner
            // stones too: a pier a cube wide, a wall's end a cube thick.
            let others = roles.edges(cube);
            let shared = faces.map(|face| {
                others.iter().any(|other| {
                    *other != edge
                        && other.role.upright()
                        && matches!(
                            kit.dress.roles.get(&other.role),
                            Some(Dress::Corners { .. })
                        )
                        && (other.side == face || other.other == face)
                })
            });
            courses.contains(&true).then_some(Piece::Corner {
                cube,
                role: edge.role,
                faces,
                courses,
                shared,
                seed,
            })
        }
        Dress::Member { .. } | Dress::Pillar { .. } | Dress::Off => None,
    }
}

/// Whether a kit dresses a role with a piece of its own.
pub(super) fn dresses_role(kit: &Kit, role: Role) -> bool {
    matches!(
        kit.dress.roles.get(&role),
        Some(Dress::Profile { .. } | Dress::Corners { .. })
    )
}
