//! Block walls (docs/building-kits.md, D179, D197): a masonry kit's wall cubes, each a
//! block, drawn not as models but as the cubes' own surface, rounded and displaced by the
//! kit's material (`surface.rs`, D197), so any shape is one wall with no seams.
//!
//! **The layout** is worked out from the cubes alone (D181): a block for each solid wall
//! cube, from the bottom up, which is the order they go in and what a plan's progress
//! counts; and for each hole, a cube cut open or glazed, the door or window set in it once
//! the building is up (D193). Every exposed edge is dressed by its role as the kit says
//! (`dressing`, D215): lintels, sills and jamb stones round its openings, quoins, copings,
//! plinths, a cap where a thinner wall stands on it. A kit that dresses no opening's head
//! has a timber lintel over each.

use super::roles::Role;
use super::{Piece, Place, Scope, dressing};
use crate::kit::{Kit, Kits};
use crate::{Base, CELL_M, Fill, Kind};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;

/// How deep a timber lintel is over an opening, and how far it runs into the wall either
/// side.
const LINTEL_DEEP_M: f32 = 0.18;
const LINTEL_BEARING_M: f32 = 0.2;

/// Lays every cube of a base in the kit's kind as blocks: its walls, floors and footings,
/// one surface; its openings; and its dress.
pub(crate) fn lay(base: &Base, kit: &Kit, kits: &Kits, scope: Scope) -> Vec<Piece> {
    let kind = kit.kind();
    let solid = |at: IVec3| {
        base.cube(at)
            .is_some_and(|cube| cube.kind == kind && super::drawn_fill(&cube) == Fill::Solid)
    };
    let cubes = scope.cubes(base, None);
    let mut pieces: Vec<Piece> = cubes
        .iter()
        .map(|&(at, _)| at)
        // A post's top laid into a timber deck is the deck's (`floors::post_in_deck`).
        .filter(|&at| solid(at) && super::floors::post_in_deck(base, kits, at).is_none())
        .map(|cube| Piece::Block { cube })
        .collect();
    // A framed kind (D236): a core under each whole cube of its walls' infill, in before it,
    // and under each being cut out, the daub off it first; and its frame, which frames its
    // openings itself.
    if let Some(framing) = &kit.framing {
        if framing.core.is_some() {
            pieces.extend(cubes.iter().filter_map(|&(at, cube)| {
                let whole = cube.kind == kind && cube.shape == crate::Shape::Whole;
                let cutting = cube.pending == crate::Pending::Carve
                    && cube.fill == Fill::Solid
                    && cube.level() > 0;
                (whole && (solid(at) || cutting)).then_some(Piece::Wattle { cube: at })
            }));
        }
        super::frames::lay(base, kit, framing, scope, &mut pieces);
    } else {
        let lintels = !dressing::dresses_role(kit, Role::Head);
        openings(base, kind, lintels, &solid, scope, &mut pieces);
    }
    dressing::lay(base, kit, kits, scope, &mut pieces);
    pieces
}

/// How far round a scope its openings are looked for, in cubes: more than an opening is
/// across, so one reaching into it is found whole.
const OPENING_REACH: i32 = 16;

/// Each hole in the walls, as the door or window set in it, and a timber lintel over it
/// where there is wall above if `lintels`.
fn openings(
    base: &Base,
    kind: Kind,
    lintels: bool,
    solid: &impl Fn(IVec3) -> bool,
    scope: Scope,
    pieces: &mut Vec<Piece>,
) {
    let wider = Scope {
        low: scope.low - IVec3::splat(OPENING_REACH),
        high: scope.high + IVec3::splat(OPENING_REACH),
    };
    let holes: BTreeSet<[i32; 3]> = wider
        .cubes(base, None)
        .into_iter()
        .filter(|(at, cube)| {
            cube.kind == kind && super::drawn_fill(cube) != Fill::Solid && base.in_wall(*at)
        })
        .map(|(at, _)| at.to_array())
        .collect();
    let mut seen = BTreeSet::new();
    for &start in &holes {
        let start = IVec3::from_array(start);
        if !seen.insert(start.to_array()) {
            continue;
        }
        // The hole's cubes, joined side by side and up and down.
        let mut region = vec![start];
        let mut next = vec![start];
        while let Some(at) = next.pop() {
            for step in [
                IVec3::X,
                IVec3::NEG_X,
                IVec3::Y,
                IVec3::NEG_Y,
                IVec3::Z,
                IVec3::NEG_Z,
            ] {
                let beside = at + step;
                if holes.contains(&beside.to_array()) && seen.insert(beside.to_array()) {
                    region.push(beside);
                    next.push(beside);
                }
            }
        }
        let low = region.iter().copied().reduce(IVec3::min).unwrap_or(start);
        let high = region.iter().copied().reduce(IVec3::max).unwrap_or(start);
        if !scope.reaches(low - IVec3::ONE, high + IVec3::ONE) {
            continue;
        }
        let (min, max) = (
            low.as_vec3() * CELL_M,
            (high + IVec3::ONE).as_vec3() * CELL_M,
        );
        let along_x = high.x > low.x || (high.z == low.z && base.runs_along(low) == IVec3::X);
        let over = solid(IVec3::new(low.x, high.y + 1, low.z));
        // A timber lintel, into the wall either side, where the wall goes on over it; none
        // over a door's or shutters' own frame.
        let door = region.iter().any(|&at| super::framed(base, at));
        if lintels && over && !door {
            let reach = if along_x { Vec3::X } else { Vec3::Z } * LINTEL_BEARING_M;
            let across = if along_x { Vec3::Z } else { Vec3::X };
            let inset = across * CELL_M * 0.1;
            pieces.push(Piece::Board {
                min: Place::near(Vec3::new(min.x, max.y - 0.02, min.z) - reach + inset),
                max: Place::near(Vec3::new(max.x, max.y + LINTEL_DEEP_M, max.z) + reach - inset),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::roles::{Dir, End};
    use super::*;
    use crate::kit::Pattern;
    use crate::{BuildOp, Panel};

    fn kit() -> Kit {
        Kit {
            id: 2,
            pattern: Pattern::Masonry,
            ..Kit::default()
        }
    }

    fn blocks(pieces: &[Piece]) -> BTreeSet<[i32; 3]> {
        pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Block { cube } => Some(cube.to_array()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_room_of_stone_is_a_block_for_every_cube() {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [7, 0, 5],
            kind: kit().kind(),
        });
        let pieces = lay(&base, &kit(), &Kits(vec![kit()]), Scope::ALL);
        // Its walls, its floor and its flat roof, one surface.
        let all: BTreeSet<[i32; 3]> = base.cubes().map(|(at, _)| at.to_array()).collect();
        assert_eq!(blocks(&pieces), all);
        assert_eq!(pieces.len(), all.len());
    }

    #[test]
    fn a_door_is_a_hole_with_a_lintel_over_it() {
        let mut base = Base::default();
        for x in 0..6 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 0],
                panel: Some(if x == 2 { Panel::Door } else { Panel::Solid }),
                kind: kit().kind(),
            });
        }
        let pieces = lay(&base, &kit(), &Kits(vec![kit()]), Scope::ALL);
        assert_eq!(
            pieces
                .iter()
                .filter(|p| matches!(p, Piece::Board { .. }))
                .count(),
            1
        );
        let solid: BTreeSet<[i32; 3]> = base
            .cubes()
            .filter(|(_, cube)| cube.fill == Fill::Solid)
            .map(|(at, _)| at.to_array())
            .collect();
        assert_eq!(blocks(&pieces), solid);
    }

    /// The stone kit dressing its roles as `stone.ron` does.
    fn dressed() -> Kit {
        use crate::kit::Dress;
        let profile = |name: &str| Dress::Profile {
            name: name.into(),
            extend: 0.0,
            joints: true,
            material: None,
        };
        let mut kit = kit();
        kit.dress.roles = [
            (Role::Crest, profile("coping")),
            (Role::Foot, profile("plinth")),
            (Role::Sill, profile("sill")),
            (Role::Head, profile("lintel")),
            (Role::Meet, profile("cap")),
            (
                Role::Arris,
                Dress::Corners {
                    long: vec![[0.42, 0.23], [0.46, 0.24]],
                    short: Vec::new(),
                    proud: 0.018,
                    bevel: 0.016,
                    joint: 0.007,
                },
            ),
            (
                Role::Jamb,
                Dress::Corners {
                    long: vec![[0.32, 0.25]],
                    short: vec![[0.19, 0.25]],
                    proud: 0.014,
                    bevel: 0.014,
                    joint: 0.007,
                },
            ),
        ]
        .into_iter()
        .collect();
        kit
    }

    fn trims(pieces: &[Piece], role: Role) -> usize {
        pieces
            .iter()
            .filter(|p| matches!(p, Piece::Trim { role: r, .. } if *r == role))
            .count()
    }

    #[test]
    fn a_lintel_over_an_opening_at_a_walls_end_turns_its_corner_and_runs_on_past_none() {
        // A window at a stone wall's free end, as the village's U-shaped farmstead has: the
        // lintels over it on the wall's two faces and its end meet at the end's corners,
        // where nothing of the wall runs on for them to bear on. Each turns its corner, mitred
        // out to the next; one stopping square there ran on past the corner into the air
        // (13 cm) and through the other, both their undersides in one plane.
        let mut base = Base::default();
        for x in 0..4 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 0],
                panel: Some(if x == 0 { Panel::WINDOW } else { Panel::Solid }),
                kind: kit().kind(),
            });
        }
        let kit = dressed();
        let pieces = lay(&base, &kit, &Kits(vec![kit.clone()]), Scope::ALL);
        let heads: Vec<(IVec3, Dir, [End; 2])> = pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Trim {
                    cube,
                    role: Role::Head,
                    side,
                    ends,
                    ..
                } => Some((cube, side, ends)),
                _ => None,
            })
            .collect();
        // The wall's end face's, and the two faces' along it.
        let end_face: Vec<_> = heads.iter().filter(|h| h.1 == Dir::West).collect();
        assert!(!end_face.is_empty(), "{heads:?}");
        for (_, _, ends) in &end_face {
            assert_eq!(*ends, [End::Out, End::Out], "{heads:?}");
        }
        let along: Vec<_> = heads
            .iter()
            .filter(|h| matches!(h.1, Dir::North | Dir::South))
            .collect();
        assert_eq!(along.len(), 2, "{heads:?}");
        for (_, _, ends) in along {
            // Its west end turns the corner; its east end is borne on the wall past it.
            assert_eq!(*ends, [End::Out, End::Square], "{heads:?}");
        }
    }

    #[test]
    fn a_window_in_stone_has_a_lintel_a_sill_and_jamb_stones_and_no_timber_lintel() {
        let mut base = Base::default();
        for x in 0..6 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 0],
                panel: Some(if x == 2 { Panel::WINDOW } else { Panel::Solid }),
                kind: kit().kind(),
            });
        }
        let kit = dressed();
        let pieces = lay(&base, &kit, &Kits(vec![kit.clone()]), Scope::ALL);
        // The window is a cube wide and two high: a lintel and a sill on each face.
        assert_eq!(trims(&pieces, Role::Head), 2);
        assert_eq!(trims(&pieces, Role::Sill), 2);
        // Jamb stones up both its sides, on both faces, a piece a cube.
        let jambs = pieces
            .iter()
            .filter(|p| {
                matches!(
                    p,
                    Piece::Corner {
                        role: Role::Jamb,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(jambs, 2 * 2 * 2);
        assert!(!pieces.iter().any(|p| matches!(p, Piece::Board { .. })));
        // Every dressed piece belongs to a cube of the wall.
        for piece in &pieces {
            if let Piece::Trim { cube, .. } | Piece::Corner { cube, .. } = piece {
                assert!(base.cube(*cube).is_some_and(|c| c.fill == Fill::Solid));
            }
        }
    }

    #[test]
    fn a_role_a_material_turns_off_lays_nothing() {
        let mut base = Base::default();
        for x in 0..6 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 0],
                panel: Some(if x == 2 { Panel::WINDOW } else { Panel::Solid }),
                kind: kit().kind(),
            });
        }
        let mut kit = dressed();
        kit.dress.roles.insert(Role::Jamb, crate::kit::Dress::Off);
        let pieces = lay(&base, &kit, &Kits(vec![kit.clone()]), Scope::ALL);
        assert!(!pieces.iter().any(|p| matches!(
            p,
            Piece::Corner {
                role: Role::Jamb,
                ..
            }
        )));
        // The rest as they were.
        assert_eq!(trims(&pieces, Role::Head), 2);
    }

    #[test]
    fn a_cross_carved_in_a_wall_has_one_lintel_and_one_sill_across_its_width() {
        let kit = dressed();
        let mut base = Base::default();
        base.apply(BuildOp::Fill {
            min: [0, 0, 0],
            max: [6, 6, 0],
            cube: Some(crate::Cube::new(kit.kind(), Fill::Solid)),
        });
        // A cross: an upright three cubes high at x = 3 and an arm across it at y = 3.
        let open = Some(crate::Cube::new(kit.kind(), Fill::Open));
        for (min, max) in [([3, 2, 0], [3, 4, 0]), ([2, 3, 0], [4, 3, 0])] {
            base.apply(BuildOp::Fill {
                min,
                max,
                cube: open,
            });
        }
        let pieces = lay(&base, &kit, &Kits(vec![kit.clone()]), Scope::ALL);
        // One lintel over its top and one sill under its foot on each face, each across the
        // three cubes the arm spans, with nothing over or under the arm's ends.
        let laid = |role: Role| -> Vec<(IVec3, [End; 2])> {
            pieces
                .iter()
                .filter_map(|p| match p {
                    Piece::Trim {
                        cube,
                        role: r,
                        side,
                        ends,
                        ..
                    } if *r == role && *side == Dir::South => Some((*cube, *ends)),
                    _ => None,
                })
                .collect()
        };
        for (role, y) in [(Role::Head, 5), (Role::Sill, 1)] {
            let mut run = laid(role);
            run.sort_by_key(|(cube, _)| cube.x);
            assert_eq!(run.len(), 3, "{role:?}: {run:?}");
            assert!(run.iter().all(|(cube, _)| cube.y == y && cube.z == 0));
            assert_eq!(
                run.iter().map(|(_, ends)| *ends).collect::<Vec<_>>(),
                vec![
                    [End::Square, End::On],
                    [End::On, End::On],
                    [End::On, End::Square]
                ]
            );
        }
        assert_eq!(trims(&pieces, Role::Head), 3 * 2);
        assert_eq!(trims(&pieces, Role::Sill), 3 * 2);
    }

    #[test]
    fn a_thinner_wall_of_another_kind_on_stone_meets_it_with_a_cap() {
        let mut base = Base::default();
        let logs = Kind(1);
        for x in 0..4 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 0],
                panel: Some(Panel::Solid),
                kind: logs,
            });
        }
        // The bottom metre of it in stone.
        base.apply(BuildOp::Fill {
            min: [0, 0, 0],
            max: [3, 1, 0],
            cube: Some(crate::Cube::new(kit().kind(), Fill::Solid)),
        });
        let kits = Kits(vec![Kit::default(), dressed()]);
        let pieces = lay(&base, &dressed(), &kits, Scope::ALL);
        // Along both faces of its top, a cube at a time, and across its ends.
        assert_eq!(trims(&pieces, Role::Meet), 4 * 2 + 2);
        assert_eq!(trims(&pieces, Role::Crest), 0);
    }
}
