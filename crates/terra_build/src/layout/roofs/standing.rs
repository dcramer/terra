//! What stands on a roof: a block kind's walls carried up into the roof cubes over them and
//! down into those under them (gables, chimneys, a dormer's cheeks), and a timber kind's
//! walls, posts and shapes on curbs cut to the roof's deck.

use super::{on_top, standing_on, surface};
use crate::kit::Kit;
use crate::layout::{Piece, Place, Scope, ticks};
use crate::{Base, CELL_M, Cube, Fill, Shape};
use glam::{IVec3, Vec3};

/// A block kind's cubes carried into the roof cubes they meet (`Piece::Gable`), each to the
/// roof's deck: up into every roof cube standing on one of its whole cubes, as a gable end
/// rises to its verges and a wall's top closes the wedge under its eaves, and down into
/// every roof cube one of its cubes stands on, as a chimney or a dormer's cheek stands on
/// the roof (docs/building-plan.md, where roofs and walls meet).
pub(crate) fn gables(base: &Base, kit: &Kit, scope: Scope, pieces: &mut Vec<Piece>) {
    for (at, cube) in scope.cubes_where(base, None, |cube| cube.kind.is_roof()) {
        if !cube.kind.is_roof() || cube.fill == Fill::Open {
            continue;
        }
        let below = base.cube(at - IVec3::Y).is_some_and(|c| {
            c.kind == kit.kind()
                && crate::layout::drawn_fill(&c) == Fill::Solid
                && !c.shape.is_cut()
        });
        let above = on_top(base, at, &cube)
            && standing_on(base, at).is_some_and(|over| over.kind == kit.kind());
        let under = deck(at, cube);
        let (low, high) = under
            .iter()
            .flatten()
            .fold((i32::MAX, i32::MIN), |(l, h), &t| (l.min(t), h.max(t)));
        let up = below && crate::layout::metres(high) > at.y as f32 * CELL_M + 0.02;
        let down = above && crate::layout::metres(low) < (at.y + 1) as f32 * CELL_M - 0.02;
        let carried = match (up, down) {
            (true, true) => crate::layout::Carried::Through,
            (true, false) => crate::layout::Carried::Up,
            (false, true) => crate::layout::Carried::Down,
            (false, false) => continue,
        };
        pieces.push(Piece::Gable {
            cube: at,
            under,
            carried,
        });
    }
}

/// Where a block kind's cube carried into a roof cube is cut over a point, lowest and
/// highest, in metres: under the deck for one carried up, over it for one carried down,
/// nowhere for one the walls over and under it fill.
pub fn carried_cut(
    carried: crate::layout::Carried,
    cube: IVec3,
    under: &[[i32; 3]; 3],
    at: Vec3,
) -> [f32; 2] {
    match carried {
        crate::layout::Carried::Up => [f32::MIN, gable_cut(cube, under, at)],
        crate::layout::Carried::Down => [gable_cut(cube, under, at), f32::MAX],
        crate::layout::Carried::Through => [f32::MIN, f32::MAX],
    }
}

/// A roof cube's deck, the plane its covering lies on, over its corners, the middles of its
/// sides and its middle, west to east and north to south, in ticks: the boards under the
/// roof, its eaves and its verges are drawn on it, so a wall carried to it meets them in
/// one straight line.
pub(crate) fn deck(at: IVec3, cube: Cube) -> [[i32; 3]; 3] {
    let under = |u: f32, v: f32| ticks((at.y as f32 + surface(cube.shape, u, v)) * CELL_M);
    [0.0, 0.5, 1.0].map(|v| [0.0, 0.5, 1.0].map(|u| under(u, v)))
}

/// A roof cube's deck ([`deck`]), if any of it is over the cube's bottom: what a wall
/// carried up into it is cut at.
pub(crate) fn underside(at: IVec3, cube: Cube) -> Option<[[i32; 3]; 3]> {
    let rows = deck(at, cube);
    let highest = rows.iter().flatten().copied().max().unwrap_or(i32::MIN);
    (crate::layout::metres(highest) > at.y as f32 * CELL_M + 0.02).then_some(rows)
}

/// How far in from its cube's side a curb under a wall of logs keeps: to the logs' widest,
/// less a hair, as eave blocking's face is (`runs`).
pub(super) const CURB_IN_M: f32 = 0.004;
/// How far in from its cube's side a curb keeps where a wall's logs end at that side.
pub(super) const CURB_END_IN_M: f32 = 0.008;
/// How far in from its cube's sides a curb under a shape of a timber kind keeps: behind the
/// boards it is clad in.
pub(super) const CURB_SHAPE_IN_M: f32 = 0.02;

/// A timber kind's walls, posts and shapes standing on roof cubes, each carried down into
/// its roof cube by a curb of its boards cut to the roof's deck (`Piece::Curb`), as a
/// dormer's cheek or a post stands on a sole on the roof: across a wall's run as wide as its
/// logs, to its cube's side where the wall runs on, as narrow as a post under a post, and
/// the whole cube under a shape.
pub(crate) fn curbs(base: &Base, kit: &Kit, scope: Scope, pieces: &mut Vec<Piece>) {
    let same = |at: IVec3| {
        base.cube(at).is_some_and(|c| {
            c.kind == kit.kind() && c.level() > 0 && crate::layout::drawn_fill(&c) == Fill::Solid
        })
    };
    // Half a member's widest across, less a hair: an upright's section stands either way.
    let half_of = |role: Option<crate::layout::Role>| {
        let (wide, high) = crate::pieces::section(kit, role);
        wide.max(high) * 0.5 - CURB_IN_M
    };
    // A wall's foot on the roof is its sill where the kit dresses one, wider than its logs.
    let foot = kit
        .dress
        .member(crate::layout::Role::Foot)
        .map_or(f32::MIN, |_| half_of(Some(crate::layout::Role::Foot)));
    // A column of whole cubes alone is an upright of the wall's members (`runs::posts`), a
    // post cube the kit's post.
    // A kit that dresses its arrises with uprights stands one at each corner of a column.
    let column = if kit.dress.member(crate::layout::Role::Arris).is_some() {
        CELL_M * 0.5 - CURB_IN_M
    } else {
        half_of(None)
    };
    let (wall, post) = (
        half_of(None).max(foot),
        half_of(Some(crate::layout::Role::Post)),
    );
    for (at, cube) in scope.cubes_where(base, None, |cube| cube.kind.is_roof()) {
        if !cube.kind.is_roof() || !on_top(base, at, &cube) {
            continue;
        }
        let Some(over) = standing_on(base, at).filter(|over| over.kind == kit.kind()) else {
            continue;
        };
        let under = deck(at, cube);
        let lowest = under.iter().flatten().copied().min().unwrap_or(i32::MAX);
        let top = (at.y + 1) as f32 * CELL_M;
        if crate::layout::metres(lowest) >= top - 0.02 {
            continue;
        }
        let standing = at + IVec3::Y;
        let dirs = [IVec3::NEG_X, IVec3::X, IVec3::NEG_Z, IVec3::Z];
        let runs_x = same(standing + IVec3::X) || same(standing - IVec3::X);
        let runs_z = same(standing + IVec3::Z) || same(standing - IVec3::Z);
        let through_x = same(standing + IVec3::X) && same(standing - IVec3::X);
        let through_z = same(standing + IVec3::Z) && same(standing - IVec3::Z);
        let alone = (over.shape == Shape::Post)
            .then_some(post)
            .or((over.shape == Shape::Whole && !runs_x && !runs_z).then_some(column));
        // How far in from each side, west, east, north and south: a wall's faces across its
        // run where it runs on through the cube both ways; the whole cube at its end, where
        // its logs are sawn flush and a post or a corner's upright stands, and round a corner
        // or a T, where they run through to its outer faces.
        let inset = dirs.map(|d| {
            let across = if d.x != 0 {
                through_z && !runs_x
            } else {
                through_x && !runs_z
            };
            if let Some(half) = alone {
                CELL_M * 0.5 - half
            } else if over.shape.is_cut() {
                // Behind a shape's cladding, which is laid on its cube's faces.
                CURB_SHAPE_IN_M
            } else if across {
                CELL_M * 0.5 - wall
            } else if same(standing + d) {
                0.0
            } else {
                // Out of the planes of the logs' ends sawn flush with the cube's side, and of
                // the daub a hair in from them.
                CURB_END_IN_M
            }
        });
        let low = at.as_vec3() * CELL_M;
        let (x0, x1) = (low.x + inset[0], low.x + CELL_M - inset[1]);
        let (z0, z1) = (low.z + inset[2], low.z + CELL_M - inset[3]);
        pieces.push(Piece::Curb {
            min: Place([ticks(x0), lowest, ticks(z0)]),
            max: Place([ticks(x1), ticks(top), ticks(z1)]),
            under,
        });
    }
}

/// How high a gable's cube may go at a point of it, in metres: the roof's underside over the
/// point, from the heights over the cube's top in its piece (`Piece::Gable`). Everything of
/// the cube over it is brought down to it, so a face cut away entirely comes to nothing.
pub fn gable_cut(cube: IVec3, under: &[[i32; 3]; 3], at: Vec3) -> f32 {
    let x = ((at.x / CELL_M - cube.x as f32) * 2.0).clamp(0.0, 2.0);
    let z = ((at.z / CELL_M - cube.z as f32) * 2.0).clamp(0.0, 2.0);
    let (i, k) = ((x as usize).min(1), (z as usize).min(1));
    let (tx, tz) = (x - i as f32, z - k as f32);
    // Each quarter in two triangles split along its diagonal through the cube's middle,
    // where a corner's hip or valley runs, so the cut follows the crease exactly: across it,
    // a valley's cut stood up through the roof and a hip's fell short of it.
    let h = |dx: usize, dz: usize| crate::layout::metres(under[k + dz][i + dx]);
    let (h00, h10, h01, h11) = (h(0, 0), h(1, 0), h(0, 1), h(1, 1));
    if (i == 0) == (k == 0) {
        // The diagonal from (0, 0) to (1, 1).
        if tx >= tz {
            h00 + tx * (h10 - h00) + tz * (h11 - h10)
        } else {
            h00 + tz * (h01 - h00) + tx * (h11 - h01)
        }
    } else if tx + tz <= 1.0 {
        // The diagonal from (1, 0) to (0, 1).
        h00 + tx * (h10 - h00) + tz * (h01 - h00)
    } else {
        h11 + (1.0 - tx) * (h01 - h11) + (1.0 - tz) * (h10 - h11)
    }
}
