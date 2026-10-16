//! Flashing, where a roof's covering meets something standing on or beside it: which sort
//! each line is, where two meet in a corner, and how wide its board lies where the roof
//! folds away under it.

use super::{RoofPart, Roofs, Segment, planar_normal};
use crate::CELL_M;
use crate::kit::Kit;
use crate::layout::{Place, ticks};
use glam::Vec3;

/// Which flashing a line where the covering meets something standing is, by the line and
/// the roof's triangle on its open side: a step up a sloping line, else an apron where the
/// roof rises to it and a back gutter where it falls to it.
pub(super) fn flashing(open: &[Vec3; 3]) -> RoofPart {
    let [middle, p, r] = *open;
    if (p.y - r.y).abs() > 1e-4 {
        RoofPart::Step
    } else if middle.y > p.y + 1e-4 {
        RoofPart::Gutter
    } else {
        RoofPart::Flashing
    }
}

/// Whether a part is flashing of any sort, where the covering meets something standing.
pub fn is_flashing(part: RoofPart) -> bool {
    matches!(part, RoofPart::Flashing | RoofPart::Step | RoofPart::Gutter)
}

/// A line's ends, a flashing's pulled back where it meets another in an inside corner, both
/// lying on the roof toward one another: the one more along the north and south butts the
/// other's side, short of the corner by its width, so the two are not laid one in the other.
/// Round an outside corner each lies on its own side and stops at the corner.
pub(super) fn flashing_butted(segment: &Segment, all: &[Segment], kit: &Kit) -> (Place, Place) {
    let (from, to) = (segment.from.metres(), segment.to.metres());
    if !is_flashing(segment.part) {
        return (segment.from, segment.to);
    }
    let along = to - from;
    let plan = along.with_y(0.0);
    if plan.length() < 1e-4 || plan.z.abs() <= plan.x.abs() {
        return (segment.from, segment.to);
    }
    let wide = kit
        .roof
        .parts
        .get(&segment.part)
        .or_else(|| kit.roof.parts.get(&RoofPart::Flashing))
        .and_then(|part| kit.dress.made.as_deref()?.modules.get(&part.module))
        .map_or(0.2, |module| module.size()[1]);
    // The way onto the roof from the line, level.
    let onto = (segment.sides[0].metres() - from)
        .reject_from(along)
        .with_y(0.0)
        .normalize_or_zero();
    let butted = |end: Vec3, other_end: Vec3| -> Vec3 {
        let meets = all.iter().any(|other| {
            if std::ptr::eq(other, segment) || !is_flashing(other.part) {
                return false;
            }
            let (a, b) = (other.from.metres(), other.to.metres());
            let way = (b - a).with_y(0.0).normalize_or_zero();
            if way.dot(plan.normalize_or_zero()).abs() > 0.5 {
                return false;
            }
            // It runs from this end out over this one's roof.
            let away = if a.distance(end) < 0.01 {
                way
            } else if b.distance(end) < 0.01 {
                -way
            } else {
                return false;
            };
            away.dot(onto) > 0.5
        });
        if meets {
            end + (other_end - end).normalize_or_zero() * (wide * along.length() / plan.length())
        } else {
            end
        }
    };
    (Place::near(butted(from, to)), Place::near(butted(to, from)))
}

/// How wide a flashing's board may lie across the roof from its line, in ticks, where the
/// roof folds away under it within its modelled width (over a hip or a ridge beside what
/// it flashes), so it lies on the roof rather than standing off it: as far as the roof keeps
/// to the plane it lies on. None where the whole width lies on it.
pub(super) fn flashing_wide(roofs: &Roofs, segment: &Segment, kit: &Kit) -> Option<i32> {
    let wide = kit
        .roof
        .parts
        .get(&segment.part)
        .or_else(|| kit.roof.parts.get(&RoofPart::Flashing))
        .and_then(|part| kit.dress.made.as_deref()?.modules.get(&part.module))
        .map(|module| module.size()[1])?;
    let (from, to) = (segment.from.metres(), segment.to.metres());
    let side = segment.sides[0].metres();
    let n = planar_normal(from, to, side);
    let onto = (side - from)
        .reject_from(to - from)
        .with_y(0.0)
        .normalize_or_zero();
    if onto == Vec3::ZERO || n.y < 0.2 {
        return None;
    }
    // The roof's plane at a point level from a point of the line.
    let plane = |p: Vec3, q: Vec3| p.y - (n.x * (q.x - p.x) + n.z * (q.z - p.z)) / n.y;
    let mut reach = wide;
    for t in [0.1, 0.5, 0.9] {
        let p = from.lerp(to, t);
        let mut d = FLASHING_STEP_M;
        while d <= wide {
            let q = p + onto * d;
            let falls = roofs
                .top_over(q.x, q.z, p.y - CELL_M)
                .is_none_or(|top| top < plane(p, q) - FLASHING_LIFT_M);
            if falls {
                reach = reach.min(d - FLASHING_STEP_M);
                break;
            }
            d += FLASHING_STEP_M;
        }
    }
    (reach < wide - 1e-4).then(|| ticks(reach.max(FLASHING_NARROWEST_M)))
}

/// How finely a flashing's board is fitted to where the roof folds away under it.
pub(super) const FLASHING_STEP_M: f32 = 0.02;
/// How far the roof may fall away under a flashing's board before it stands off it.
pub(super) const FLASHING_LIFT_M: f32 = 0.005;
/// The narrowest a flashing's board is laid.
pub(super) const FLASHING_NARROWEST_M: f32 = 0.04;
