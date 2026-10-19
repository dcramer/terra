//! A roof's frame: the beams under its creases, a rafter up every slope along every cube's
//! edge, and the battens across them under every course, each cut where it would run into
//! a wall.

use super::lines::crease_met;
use super::{Beside, RoofPart, Roofs, STEP_M, Segment, join, normal, planar_normal};
use crate::layout::{Piece, Place};
use glam::{IVec3, Vec3};
use terra_build::kit::Kit;
use terra_build::{Base, CELL_M, Fill};
use terra_gen::hash;

/// How far a beam drawn from the kit's rafter module reaches from its line: as it is
/// modelled, which may be wider than the rafter's laid size.
pub(super) fn beam_half(kit: &Kit, rafter: f32) -> f32 {
    kit.roof
        .parts
        .get(&RoofPart::Rafter)
        .and_then(|part| kit.dress.made.as_deref()?.modules.get(&part.module))
        .map_or(rafter, |module| {
            let [high, wide] = module.size();
            rafter.max(high.max(wide) * 0.5)
        })
}

/// How far under a crease its beam is laid: the battens' and the rafters' depth, and how far
/// a beam reaches from its line, so one meeting another butts its side.
#[derive(Clone, Copy)]
pub(super) struct Under {
    pub down: f32,
    pub beam_half: f32,
}

/// The beam under a ridge, a hip or a valley, cut where it would run into a wall, laid in
/// the frame's stage.
pub(super) fn crease_beams(
    base: &Base,
    roofs: &Roofs,
    segment: &Segment,
    every: &[Segment],
    seed: u32,
    under: Under,
    pieces: &mut Vec<Piece>,
) {
    let Under { down, beam_half } = under;
    let (a, b) = (segment.sides[0].metres(), segment.sides[1].metres());
    let (from, to) = (segment.from.metres(), segment.to.metres());
    let down =
        (planar_normal(from, to, a) + planar_normal(from, to, b)).normalize_or(Vec3::Y) * down;
    // A ridge's beam bears on the gables' inner faces, inside the roof; a hip's
    // and a valley's run on down to the eaves as the rafters do.
    let level = segment.part == RoofPart::Ridge;
    let beams = framed(base, from - down, to - down, level);
    let last = beams.len().saturating_sub(1);
    // Its ends a little short of the line's, where the battens and rafters along it
    // end too, so its sawn ends are not in their planes; and where its crease meets
    // another, as a wing's ridge meets the main one at a T, short of that one's beam
    // by half its width, so it butts its side rather than running into it.
    let met = [segment.from, segment.to].map(|end| crease_met(segment, end, every));
    let short = met.map(|met| BEAM_SHORT_M + if met { beam_half } else { 0.0 });
    for (i, (mut from, mut to)) in beams.into_iter().enumerate() {
        let along = (to - from).normalize_or_zero();
        let level = Vec3::new(along.x, 0.0, along.z).length().max(0.2);
        // Where its crease meets another, its end so far back in plan from where
        // they meet, measured level from that point, whatever the beam's drop under the
        // roof carried it toward it; elsewhere only drawn short of its end.
        let back = |end: Vec3, meet: Vec3, way: Vec3, short: f32, met: bool| {
            if !met {
                return end + way * (short / level);
            }
            let level_way = Vec3::new(way.x, 0.0, way.z).normalize_or_zero();
            let now = (end - meet).with_y(0.0).dot(level_way);
            end + way * ((short - now).max(0.0) / level)
        };
        if i == 0 {
            from = back(from, segment.from.metres(), along, short[0], met[0]);
        }
        if i == last {
            to = back(to, segment.to.metres(), -along, short[1], met[1]);
        }
        pieces.push(Piece::Roof {
            part: RoofPart::Rafter,
            from: Place::near(from),
            to: Place::near(to),
            sides: segment.sides,
            seed: hash::of(&[seed, 1]),
            cell: roofs.cell(segment.square, 1),
            capped: [true; 2],
            met: [false; 2],
            wide: None,
        });
    }
}

/// The battens, each over the roof's own cubes, joined along the face and cut where they
/// would run into a wall.
pub(super) fn battens(
    base: &Base,
    kit: &Kit,
    roofs: &Roofs,
    battens: Vec<Segment>,
    pieces: &mut Vec<Piece>,
) {
    // Each over the roof's own cubes, from wall to wall: none out under the eaves and
    // verges, where the boards under the covering are all that shows, nor through a gable.
    // Capped where it stops, not at a joint with the next of its line, where the two run on
    // as one and a cap on each stood in the other.
    let joined = join(battens);
    for segment in &joined {
        let joint = |end: Place| {
            joined.iter().any(|other| {
                !std::ptr::eq(other, segment)
                    && (other.from == end || other.to == end)
                    && (other.to.metres() - other.from.metres())
                        .normalize_or_zero()
                        .dot((segment.to.metres() - segment.from.metres()).normalize_or_zero())
                        .abs()
                        > 0.99
            })
        };
        let (start, end) = (segment.from.metres(), segment.to.metres());
        // Its ends at a hip in by its width: laid square across the face, its upper edge
        // reached past the hip, where the next face's batten at the same height lies.
        let along = (end - start).normalize_or_zero();
        let at_hip = |p: Vec3, out: Vec3| {
            let beyond = p + out * 0.05;
            roofs
                .top_over(beyond.x, beyond.z, p.y - CELL_M)
                .is_some_and(|h| h < p.y - 0.01)
        };
        let wide = kit.roof.batten.0;
        let start = if at_hip(start, -along) {
            start + along * wide
        } else {
            start
        };
        let end = if at_hip(end, along) {
            end - along * wide
        } else {
            end
        };
        if start.distance(end) < 1e-3 || (end - start).dot(along) <= 0.0 {
            continue;
        }
        for (from, to) in framed(base, start, end, true) {
            let capped = [
                !(from.distance(start) < 1e-3 && joint(segment.from)),
                !(to.distance(end) < 1e-3 && joint(segment.to)),
            ];
            pieces.push(Piece::Roof {
                part: RoofPart::Batten,
                from: Place::near(from),
                to: Place::near(to),
                sides: segment.sides,
                seed: 0,
                cell: roofs.cell(segment.square, 2),
                capped,
                met: [false; 2],
                wide: None,
            });
        }
    }
}

/// How far apart a frame's piece is tried along it, in metres, for where it may run.
pub(super) const FRAME_STEP_M: f32 = 0.02;
/// The shortest piece of frame left once cut, in metres.
pub(super) const FRAME_SHORTEST_M: f32 = 0.1;

/// Where a piece of a roof's frame from `from` to `to` may run, cut into the pieces between:
/// never inside a wall, a gable carried up into the roof included, so it stops at the
/// wall's face and its end is never seen through it; and a `level` one (a batten, a
/// ridge's beam) only over the roof's own cubes, so it stops inside the roof rather than
/// running on out under a verge to end in the air. Rafters and the beams under hips and
/// valleys, which fall to the eaves, run on out over them as rafters' tails do.
pub(super) fn framed(base: &Base, from: Vec3, to: Vec3, level: bool) -> Vec<(Vec3, Vec3)> {
    let length = from.distance(to);
    if length < 1e-4 {
        return Vec::new();
    }
    // Whether a cell may hold it, asked of the base once a cell: the tries come a few
    // centimetres apart, many to a cell.
    let known: std::cell::RefCell<Option<(IVec3, bool)>> = std::cell::RefCell::new(None);
    let inside_cell = |at: IVec3| {
        let wall = |at: IVec3| {
            base.cube(at).is_some_and(|c| {
                !c.kind.is_roof()
                    && crate::layout::drawn_fill(&c) == Fill::Solid
                    && !c.shape.is_cut()
            })
        };
        let here = base.cube(at);
        // A wall's cube, or a roof's standing on one: the wall carried up into it.
        let in_wall = wall(at) || (here.is_some_and(|c| c.kind.is_roof()) && wall(at - IVec3::Y));
        let over_roof = !level
            || (-1..=1).any(|dy| {
                base.cube(at + IVec3::Y * dy)
                    .is_some_and(|c| c.kind.is_roof() && c.fill != Fill::Open)
            });
        !in_wall && over_roof
    };
    let inside = |p: Vec3| {
        let at = (p / CELL_M).floor().as_ivec3();
        if let Some((cell, ok)) = *known.borrow()
            && cell == at
        {
            return ok;
        }
        let ok = inside_cell(at);
        *known.borrow_mut() = Some((at, ok));
        ok
    };
    let steps = libm::ceilf(length / FRAME_STEP_M).max(1.0) as usize;
    let at = |t: f32| from.lerp(to, t);
    let t = |i: usize| i as f32 / steps as f32;
    // Where it passes from inside to out between two tries, to a millimetre or so.
    let edge = |mut good: f32, mut bad: f32| {
        for _ in 0..6 {
            let middle = (good + bad) * 0.5;
            if inside(at(middle)) {
                good = middle;
            } else {
                bad = middle;
            }
        }
        good
    };
    let mut out = Vec::new();
    let mut start: Option<f32> = None;
    for i in 0..=steps {
        let ok = inside(at(t(i)));
        match (ok, start) {
            (true, None) => start = Some(if i == 0 { 0.0 } else { edge(t(i), t(i - 1)) }),
            (false, Some(s)) => {
                out.push((at(s), at(edge(t(i - 1), t(i)))));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((at(s), to));
    }
    out.retain(|(a, b)| a.distance(*b) >= FRAME_SHORTEST_M);
    out
}

/// How much shorter at each end than its crease the beam under a ridge, a hip or a valley is.
pub(super) const BEAM_SHORT_M: f32 = 0.006;

/// How far the roof falls over a centimetre on past a batten's reach up the slope, where a
/// crest is within that reach: more than rounding, less than the centimetre a 45-degree roof
/// falls there.
pub(super) const CREST_FALL_M: f32 = 0.002;

/// The rafters: one up every slope along every cube's edge, from its foot at the eaves to
/// the ridge, hip or valley it meets, under the battens.
pub(super) fn rafters(
    roofs: &Roofs,
    ours: &dyn Fn(usize) -> bool,
    below: f32,
    half: f32,
    pieces: &mut Vec<Piece>,
) {
    let mut segments = Vec::new();
    for q in (0..roofs.squares.len()).filter(|&q| ours(q)) {
        let place = roofs.squares[q].place;
        // Its east side on a cube's edge, a line north and south; its south side, east and
        // west.
        for (side, on_edge, along) in [
            (1, (place[0] + 1).rem_euclid(2) == 0, 2),
            (2, (place[1] + 1).rem_euclid(2) == 0, 0),
        ] {
            let Beside::Square(other) = roofs.beside[q][side] else {
                continue;
            };
            if !on_edge || !ours(other) {
                continue;
            }
            let across = 2 - along;
            // None along a verge: where a square either side is carried out past the edge
            // of its cube across the slope, the line is a gable's face or out beyond it,
            // where a rafter would lie on the wall or hang in the air under the verge.
            let past = |q: usize| {
                let square = &roofs.squares[q];
                let (place, cube) = if across == 0 {
                    (square.place[0], square.cube.x)
                } else {
                    (square.place[1], square.cube.z)
                };
                place.div_euclid(2) != cube
            };
            if past(q) || past(other) {
                continue;
            }
            let mine = roofs.triangle(q, side);
            let theirs = roofs.triangle(other, (side + 2) % 4);
            // Each falls along the line, the same way.
            let fall = |n: Vec3| {
                (n[along].abs() > n[across].abs() + 1e-4 && n.y < 1.0 - 1e-4)
                    .then_some(n[along] > 0.0)
            };
            let n = normal(&mine);
            match (fall(n), fall(normal(&theirs))) {
                (Some(a), Some(b)) if a == b => {}
                _ => continue,
            }
            let down = n * below;
            let centre = Place::near((mine[0] + mine[1] + mine[2]) / 3.0);
            segments.push(Segment {
                part: RoofPart::Rafter,
                from: Place::near(mine[1] - down),
                to: Place::near(mine[2] - down),
                sides: [centre, centre],
                square: q,
            });
        }
    }
    for segment in join(segments) {
        let seed = hash::of(&[
            segment.from.0[0] as u32,
            segment.from.0[2] as u32,
            segment.from.0[1] as u32,
            3,
        ]);
        // Its top end, where the roof stops past it, as a lean-to's high edge does: laid
        // under the roof, square to it, it stood out past the edge by as much as it is deep,
        // so it stops short by as much.
        let (mut from, mut to) = (segment.from.metres(), segment.to.metres());
        let axis = (to - from).normalize_or(Vec3::Y);
        let (top, step) = if to.y >= from.y {
            (&mut to, axis)
        } else {
            (&mut from, -axis)
        };
        let level = Vec3::new(step.x, 0.0, step.z).normalize_or_zero();
        let beyond = *top + level * STEP_M * 0.5;
        let carries_on = roofs
            .top_over(beyond.x, beyond.z, top.y - CELL_M)
            .is_some_and(|h| h < top.y + below + CELL_M);
        // Where it carries on over a ridge or a hip, laid under the roof square to it, it
        // reached past the ridge's plumb line by as much as it is under the roof and as
        // deep again, through the rafter of the slope beyond in the same plane: it stops
        // where its foot's edge meets that line, against the beam under the crease.
        let (from, to) = if level == Vec3::ZERO {
            (segment.from, segment.to)
        } else if carries_on {
            let rise = step.y / Vec3::new(step.x, 0.0, step.z).length().max(1e-4);
            *top -= step * ((below + half) * rise.abs());
            (Place::near(from), Place::near(to))
        } else {
            *top -= step * (below * 2.0);
            (Place::near(from), Place::near(to))
        };
        pieces.push(Piece::Roof {
            part: RoofPart::Rafter,
            from,
            to,
            sides: segment.sides,
            seed,
            cell: roofs.cell(segment.square, 1),
            capped: [true; 2],
            met: [false; 2],
            wide: None,
        });
    }
}
