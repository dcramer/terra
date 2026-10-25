//! Bodies apart by mass (docs/controller.md, step 6; D53's departure from Jolt, whose
//! characters treat each other as walls that never give way): after every body has moved,
//! each pair whose pushboxes overlap is separated along the level line between them, each by
//! its share of the overlap inversely to its mass, a fixed body (infinite mass) not at all.
//! Pairs are taken in ID order for a few rounds, and each push is swept against the ground,
//! so a push never puts a body into a wall; what a wall stops is left for the other to take in
//! a later round.

use super::Body;
use super::query::{Capsule, Query};

fn inverse_mass(body: &Body) -> f32 {
    if body.kind.mass.is_finite() && body.kind.mass > 0.0 {
        1.0 / body.kind.mass
    } else {
        0.0
    }
}

/// A push of a body along a motion, as far as the ground lets it.
fn shove(body: &mut Body, by: glam::Vec3, ground: &dyn Query) {
    let length = by.length();
    if length < 1e-7 {
        return;
    }
    let capsule = Capsule::standing(body.feet, body.up, body.kind.radius, body.kind.height);
    let fraction = ground.sweep(&capsule, by).map_or(1.0, |hit| {
        (hit.fraction - body.kind.padding_m / length).max(0.0)
    });
    body.feet += by * fraction;
}

/// Separates overlapping bodies, kept in ID order, for so many rounds.
pub fn separate(bodies: &mut [Body], ground: &dyn Query, rounds: usize) {
    for _ in 0..rounds {
        let mut moved = false;
        for i in 0..bodies.len() {
            for j in i + 1..bodies.len() {
                let (head, tail) = bodies.split_at_mut(j);
                let (a, b) = (&mut head[i], &mut tail[0]);
                let (ia, ib) = (inverse_mass(a), inverse_mass(b));
                if ia + ib <= 0.0 {
                    continue;
                }
                let up = (a.up + b.up).normalize_or(a.up);
                let apart = b.feet - a.feet;
                let (ha, hb) = (0.0, apart.dot(up));
                if hb > ha + a.kind.height || hb + b.kind.height < ha {
                    continue;
                }
                let level = apart - up * hb;
                let distance = level.length();
                let overlap = a.kind.radius + b.kind.radius - distance;
                if overlap <= 0.0 {
                    continue;
                }
                let dir = if distance > 1e-6 {
                    level / distance
                } else {
                    a.facing
                };
                let share = ia / (ia + ib);
                shove(a, -dir * overlap * share, ground);
                shove(b, dir * overlap * (1.0 - share), ground);
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}
