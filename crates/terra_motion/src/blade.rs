//! Hit volumes against hurt zones (docs/movement.md, "Bodies that do not pass through"): a
//! blade is a segment from hilt to tip, swept from where it was last tick to where it is now
//! so a fast blade cannot skip through a body between ticks, and a body is an upright capsule.
//! Plain geometry on glam's vectors, which Bevy shares.

use glam::Vec3;

/// A sword's blade along its hand socket's up: from the hilt to the tip, metres, as
/// `tools/characters/phases.ts` measured the clips with it.
pub const HILT_M: f32 = 0.15;
pub const TIP_M: f32 = 0.83;

/// A blade's place: its hilt and its tip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub hilt: Vec3,
    pub tip: Vec3,
}

/// A capsule: a segment and a radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub from: Vec3,
    pub to: Vec3,
    pub radius: f32,
}

/// The nearest points of two segments, as (on the first, on the second).
pub fn nearest(p0: Vec3, p1: Vec3, q0: Vec3, q1: Vec3) -> (Vec3, Vec3) {
    let d1 = p1 - p0;
    let d2 = q1 - q0;
    let r = p0 - q0;
    let a = d1.length_squared();
    let e = d2.length_squared();
    let f = d2.dot(r);
    let (s, t) = if a <= 1e-9 && e <= 1e-9 {
        (0.0, 0.0)
    } else if a <= 1e-9 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= 1e-9 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s = if denom > 1e-9 {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    (p0 + d1 * s, q0 + d2 * t)
}

/// Where a blade swept from `was` to `now` first meets a capsule, if it does: the sweep
/// cut into enough steps that no step moves the tip further than the capsule is thick.
pub fn sweep(was: Edge, now: Edge, body: Capsule) -> Option<Vec3> {
    let travel = (now.tip - was.tip)
        .length()
        .max((now.hilt - was.hilt).length());
    let steps = ((travel / body.radius.max(0.02)).ceil() as usize).clamp(1, 32);
    for step in 0..=steps {
        let k = step as f32 / steps as f32;
        let hilt = was.hilt.lerp(now.hilt, k);
        let tip = was.tip.lerp(now.tip, k);
        let (on_blade, on_body) = nearest(hilt, tip, body.from, body.to);
        if on_blade.distance(on_body) <= body.radius {
            return Some(on_blade);
        }
    }
    None
}

/// A body's hurt capsule, standing on its feet: from knee to crown, as thick as its trunk.
pub fn body_at(feet: Vec3, up: Vec3) -> Capsule {
    Capsule {
        from: feet + up * 0.45,
        to: feet + up * 1.55,
        radius: 0.27,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_points_of_crossing_and_parallel_segments() {
        let (a, b) = nearest(
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, -1.0, 0.5),
            Vec3::new(0.0, 1.0, 0.5),
        );
        assert!(a.distance(Vec3::ZERO) < 1e-5 && b.distance(Vec3::new(0.0, 0.0, 0.5)) < 1e-5);
        let (a, b) = nearest(
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(3.0, 1.0, 0.0),
            Vec3::new(4.0, 1.0, 0.0),
        );
        assert!((a.distance(b) - Vec3::new(2.0, 1.0, 0.0).length()).abs() < 1e-5);
    }

    #[test]
    fn a_fast_blade_that_skips_past_a_body_between_ticks_still_strikes_it() {
        let body = body_at(Vec3::ZERO, Vec3::Y);
        // A cut across the body's front at chest height, from a metre to its right to a
        // metre to its left in one tick: neither end touches it.
        let was = Edge {
            hilt: Vec3::new(1.2, 1.1, 0.9),
            tip: Vec3::new(1.0, 1.1, 0.1),
        };
        let now = Edge {
            hilt: Vec3::new(-1.2, 1.1, 0.9),
            tip: Vec3::new(-1.0, 1.1, 0.1),
        };
        assert!(sweep(was, was, body).is_none());
        assert!(sweep(now, now, body).is_none());
        let at = sweep(was, now, body).expect("the sweep strikes");
        assert!(at.x.abs() < 0.4 && (at.y - 1.1).abs() < 1e-4);
    }

    #[test]
    fn a_blade_held_out_of_reach_never_strikes() {
        let body = body_at(Vec3::ZERO, Vec3::Y);
        let was = Edge {
            hilt: Vec3::new(1.0, 1.1, 2.0),
            tip: Vec3::new(1.0, 1.1, 1.2),
        };
        let now = Edge {
            hilt: Vec3::new(-1.0, 1.1, 2.0),
            tip: Vec3::new(-1.0, 1.1, 1.2),
        };
        // The tip passes 1.2 m in front of a body 0.27 m round.
        assert!(sweep(was, now, body).is_none());
    }
}
