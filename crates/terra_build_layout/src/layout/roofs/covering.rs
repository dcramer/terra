//! A roof's covering, a square at a time, its courses' lower edges level lines a course's
//! rise apart up from each roof's lowest edge, and carried on into whatever stands beside
//! it.

use super::frame::CREST_FALL_M;
use super::{Beside, COVERING, RoofPart, Roofs, SIDES, Segment, level_cut, normal};
use crate::layout::{Piece, Place, ticks};
use glam::{IVec3, Vec3};
use terra_build::kit::Kit;
use terra_build::{Base, CELL_M, Fill, Shape};

/// Lays the covering of a roof kind's squares, a square at a time, tucked into whatever
/// stands beside each, and gives back the battens under it, a line under the foot of every
/// course, for the frame to lay.
pub(super) fn lay(
    base: &Base,
    kit: &Kit,
    roofs: &Roofs,
    ours: &dyn Fn(usize) -> bool,
    pieces: &mut Vec<Piece>,
) -> Vec<Segment> {
    let r = &kit.roof;
    let count = roofs.squares.len();
    // Battens and covering, a square at a time, the courses' lower edges level lines a
    // course's rise apart up from each roof's lowest edge.
    let texture = kit
        .textures
        .get(&terra_build::Material::Covering)
        .map_or(1.0, |texture| texture.metres[1]);
    let exposure = r.exposure(texture);
    // Every slope rises a cube a cube: a course's rise is its exposure up a 45° slope.
    let rise = exposure * std::f32::consts::FRAC_1_SQRT_2;
    let mut battens: Vec<Segment> = Vec::new();
    for q in (0..count).filter(|&q| ours(q)) {
        let eave = roofs.span[roofs.roof[q]].0;
        let corners = [0, 1, 2, 3].map(|k| Place::near(roofs.point(q, k)));
        let mut open = roofs.beside[q].map(|b| !matches!(b, Beside::Square(_)));
        let courses = (ticks(eave), ticks(rise).max(1));
        // Tucked into whatever stands beside it, so a wall or a post thinner than its cube
        // meets covering rather than a slot down past its face.
        if !roofs.squares[q].covered {
            for (side, open) in open.iter_mut().enumerate() {
                if roofs.beside[q][side] != Beside::Against {
                    continue;
                }
                if let Some(strip) = tuck(base, kit, roofs, q, side, courses) {
                    *open = false;
                    pieces.push(strip);
                }
            }
        }
        pieces.push(Piece::Covering {
            corners,
            middle: Place::near(roofs.middle(q)),
            courses,
            open,
            cell: roofs.cell(q, COVERING),
        });
        // A batten under the foot of every course across the square, each joined to the
        // next square's into one along the face.
        let (base_m, step) = (
            crate::layout::metres(courses.0),
            crate::layout::metres(courses.1),
        );
        // How far up the slope a batten reaches, as it is modelled: across it from its line,
        // and how far it hangs under the roof's deck, whose plane leans back toward the crest.
        let (reach_up, reach_down) = kit
            .roof
            .parts
            .get(&RoofPart::Batten)
            .and_then(|part| kit.dress.made.as_deref()?.modules.get(&part.module))
            .map_or((r.batten.0, 0.0), |module| {
                module
                    .near
                    .iter()
                    .flat_map(|mesh| &mesh.positions)
                    .fold((r.batten.0, 0.0f32), |(up, down), p| {
                        (up.max(p[2]), down.max(-p[1]))
                    })
            });
        for side in 0..4 {
            let t = roofs.triangle(q, side);
            let n = normal(&t);
            let upslope = n.cross(Vec3::Y.cross(n).normalize_or(Vec3::X));
            let low = t.iter().map(|p| p.y).fold(f32::MAX, f32::min);
            let high = t.iter().map(|p| p.y).fold(f32::MIN, f32::max);
            // None on a level triangle, a whole or half square's top: no course's foot
            // crosses it, and a level cut through its own plane would lay battens along its
            // edges and diagonals, crossing one another (gap 11).
            if high - low < 1e-4 {
                continue;
            }
            let first = libm::ceilf((low - base_m) / step) as i32;
            let last = libm::floorf((high - base_m) / step) as i32;
            for course in first..=last {
                let Some((p, r)) = level_cut(&t, base_m + course as f32 * step) else {
                    continue;
                };
                if p.distance(r) < 1e-3 {
                    continue;
                }
                // None at a crest, where the roof falls away within the batten's reach up the
                // slope from it: the far slope's own top batten would cross it there. Past
                // that reach the roof falls on the way on, as it never does on its own plane
                // nor past a hip, beyond which it runs level the way this face rises.
                let middle = (p + r) * 0.5;
                let way = Vec3::new(upslope.x, 0.0, upslope.z).normalize_or_zero();
                let reach = (upslope * reach_up - n * reach_down).dot(way);
                let top = |past: f32| {
                    let at = middle + way * (reach + past);
                    roofs.top_over(at.x, at.z, middle.y - CELL_M)
                };
                match (top(0.01), top(0.02)) {
                    (Some(near), Some(far)) if far > near - CREST_FALL_M => {}
                    _ => continue,
                }
                let beside = Place::near(middle + upslope * 0.1);
                battens.push(Segment {
                    part: RoofPart::Batten,
                    from: Place::near(p),
                    to: Place::near(r),
                    sides: [beside, beside],
                    square: q,
                });
            }
        }
    }
    battens
}

/// How far the covering is tucked into what stands beside the roof, in metres: past any
/// timber wall's or post's face in its cube (a log wall's is 10 cm in, a post's 11), short
/// of the middle of a cube, so two roofs either side of a wall a cube thick never meet in it.
pub const TUCK_M: f32 = 0.15;

/// Whether a cube stands up beside a roof for its covering to be tucked into: something not
/// a roof's, solid and in, filling its column's height (whole, or a post).
pub(super) fn stands_beside(base: &Base, at: IVec3) -> bool {
    base.cube(at).is_some_and(|cube| {
        !cube.kind.is_roof()
            && cube.level() > 0
            && crate::layout::drawn_fill(&cube) == Fill::Solid
            && matches!(cube.shape, Shape::Whole | Shape::Post)
    })
}

/// A strip of a square's covering carried on past a side something stands beside
/// ([`TUCK_M`]), on the plane of the square's triangle there, as a roofer runs the covering
/// up under a wall's flashing: only where what stands there is as high as the strip and its
/// covering all along it. Where what stands there turns away at an end of the side (an
/// outside corner) the strip's end is mitred back, and where it turns toward the roof (an
/// inside corner) carried on into the corner, mitred, so the strips of two sides meet on
/// the diagonal, neither in the other.
pub(super) fn tuck(
    base: &Base,
    kit: &Kit,
    roofs: &Roofs,
    q: usize,
    side: usize,
    courses: (i32, i32),
) -> Option<Piece> {
    let t = roofs.triangle(q, side);
    let (a, b) = (t[1], t[2]);
    let (step, _, _) = SIDES[side];
    let out = Vec3::new(step[0] as f32, 0.0, step[1] as f32);
    let n = normal(&t);
    let along = (b - a).with_y(0.0).normalize_or_zero();
    let cell_of = |p: Vec3| (p / CELL_M).floor().as_ivec3();
    let solid_at = |p: Vec3| stands_beside(base, cell_of(p));
    // Each end: carried on (1) into an inside corner, mitred back (-1) round an outside
    // one, or square (0) where the roof and what stands beside it run on together.
    let end = |p: Vec3, away: Vec3| -> f32 {
        let beyond = p + away * 0.05 + Vec3::Y * 0.02;
        if !solid_at(beyond + out * 0.05) {
            -1.0
        } else if solid_at(beyond - out * 0.05) && solid_at(beyond + away * TUCK_M) {
            1.0
        } else {
            0.0
        }
    };
    let (ka, kb) = (end(a, -along), end(b, along));
    // A far corner: out by the tuck, and along by it into a corner or back from one, on
    // the plane.
    let far = |p: Vec3, k: f32, way: Vec3| {
        let d = out * TUCK_M + way * (k * TUCK_M);
        p + d + Vec3::Y * (-(n.x * d.x + n.z * d.z) / n.y.max(0.2))
    };
    let (a2, b2) = (far(a, ka, -along), far(b, kb, along));
    let corners = [a, b, b2, a2];
    // As high and as low as what it is tucked into goes, all over it.
    let r = &kit.roof;
    let high = corners.iter().map(|p| p.y).fold(f32::MIN, f32::max) + r.thick + r.relief + 0.02;
    let low = corners.iter().map(|p| p.y).fold(f32::MAX, f32::min) - 0.02;
    let centre = corners.iter().sum::<Vec3>() / 4.0;
    for p in [
        a + out * 0.02 + along * 0.02,
        b + out * 0.02 - along * 0.02,
        a2.lerp(centre, 0.1),
        b2.lerp(centre, 0.1),
    ] {
        let column = cell_of(p);
        for y in libm::floorf(low / CELL_M) as i32..=libm::floorf(high / CELL_M) as i32 {
            if !stands_beside(base, IVec3::new(column.x, y, column.z)) {
                return None;
            }
        }
    }
    let middle = centre;
    Some(Piece::Covering {
        corners: corners.map(Place::near),
        middle: Place::near(middle),
        courses,
        // Its edges all inside what it is tucked into, where none is seen, so none is closed
        // down to the roof and far off its sheet is lifted plumb all over, as the square's
        // side it runs on from is, and as the next strip's is where two meet in a corner.
        open: [false; 4],
        cell: roofs.cell(q, COVERING),
    })
}
