//! Door steps (construction-001's ground cases, D194): a flight of stone steps from each
//! ground-floor doorway in an outside wall down to the ground, its threshold the footing
//! under the doorway. The ground is not changed: the steps stand on it, set a little into
//! it. They are worked out from the cubes and the ground's height, which the caller gives,
//! since the ground beyond a building is the world's, not the base's.

use crate::{Base, CELL_M, Panel};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;

/// How high a step rises at most.
const RISE_M: f32 = 0.18;
/// How deep a step's tread is.
const TREAD_M: f32 = 0.32;
/// How much wider than the doorway a flight is.
const WIDER_M: f32 = 0.5;
/// The most risers a flight has, however far the ground falls away.
const MOST_RISERS: usize = 12;
/// How far a step reaches down into the ground under it.
const INTO_GROUND_M: f32 = 0.3;

/// A flight of steps down from a doorway, in the base's own space.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    /// The middle of the doorway's threshold, on its outer face.
    pub top: Vec3,
    /// Level, away from the door.
    pub outward: Vec3,
    /// How far out it reaches, and how far its foot is below the threshold.
    pub length: f32,
    pub drop: f32,
    pub width: f32,
    /// Each step's box, from the door outward, down into the ground.
    pub steps: Vec<(Vec3, Vec3)>,
}

impl Flight {
    /// The height of the flight's slope under a point, if the point is over it: what a
    /// walker treats it as.
    pub fn height_at(&self, point: Vec3) -> Option<f32> {
        let offset = point - self.top;
        let along = offset.dot(self.outward);
        let across = offset.dot(self.outward.cross(Vec3::Y));
        let on = (0.0..=self.length).contains(&along) && across.abs() <= self.width * 0.5;
        on.then(|| self.top.y - self.drop * along / self.length)
    }
}

/// A flight from every ground-floor doorway in an outside wall whose threshold is a step or
/// more above the ground; `ground` gives the ground's height under a point of the base's own
/// space, in its own space.
pub fn flights(base: &Base, ground: impl Fn(Vec3) -> f32) -> Vec<Flight> {
    // One flight for each doorway, however many tiles wide it is.
    let doorways: BTreeSet<Vec<[i32; 3]>> = base
        .walls()
        .filter(|&(tile, panel)| tile.y == 0 && panel == Panel::Door)
        .map(|(tile, _)| base.run(tile).iter().map(|t| t.to_array()).collect())
        .collect();
    let mut found = Vec::new();
    for doorway in doorways {
        let (first, last) = (
            IVec3::from_array(doorway[0]),
            IVec3::from_array(doorway[doorway.len() - 1]),
        );
        let Some(outward) = base.outward(first) else {
            continue;
        };
        let outward = outward.as_vec3();
        let foot = |tile: IVec3| {
            (tile.with_y(0).as_vec3() + Vec3::new(0.5, 0.0, 0.5) + outward * 0.5) * CELL_M
        };
        let along = (last - first).as_vec3().normalize_or_zero() * CELL_M * 0.5;
        let (near, far) = (foot(first) - along, foot(last) + along);
        let top = (near + far) * 0.5;
        let width = near.distance(far) + WIDER_M;
        if let Some(flight) = flight(top, outward, width, &ground) {
            found.push(flight);
        }
    }
    found
}

/// A flight from a threshold down to the ground: as many risers as the drop needs, the last
/// down onto the ground, worked out again as the ground falls away under the longer flight.
fn flight(top: Vec3, outward: Vec3, width: f32, ground: &impl Fn(Vec3) -> f32) -> Option<Flight> {
    let risers_at = |length: f32| {
        let drop = top.y - ground(top + outward * length);
        (
            drop,
            ((drop / RISE_M).ceil().max(1.0) as usize).min(MOST_RISERS),
        )
    };
    let mut length = TREAD_M;
    for _ in 0..4 {
        length = TREAD_M * risers_at(length).1.saturating_sub(1).max(1) as f32;
    }
    let (drop, risers) = risers_at(length);
    if risers < 2 {
        return None;
    }
    let treads = risers - 1;
    let rise = drop / risers as f32;
    let across = outward.cross(Vec3::Y) * (width * 0.5);
    let steps = (0..treads)
        .map(|i| {
            let near = top + outward * (TREAD_M * i as f32);
            let far = near + outward * TREAD_M;
            let height = top.y - rise * (i + 1) as f32;
            let bottom = ground(near).min(ground(far)) - INTO_GROUND_M;
            let (a, b) = (near - across, far + across);
            (a.min(b).with_y(bottom), a.max(b).with_y(height))
        })
        .collect();
    Some(Flight {
        top,
        outward,
        length: TREAD_M * treads as f32,
        drop: rise * treads as f32,
        width,
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BuildOp;
    use crate::layout::tests::{cabin, kit};

    fn with_door() -> Base {
        let mut base = cabin();
        for x in [3, 4] {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 5],
                panel: Some(Panel::Door),
                kind: kit().kind(),
            });
        }
        base
    }

    #[test]
    fn a_door_over_falling_ground_has_steps_down_to_it_each_a_riser_apart() {
        let base = with_door();
        // The ground falls away south of the cabin, 40 cm a metre, from 30 cm down.
        let ground = |at: Vec3| -0.3 - 0.4 * (at.z - 3.0).max(0.0);
        let flights = flights(&base, ground);
        assert_eq!(flights.len(), 1, "one flight for a doorway two tiles wide");
        let flight = &flights[0];
        assert_eq!(flight.outward, Vec3::Z);
        assert!((flight.top - Vec3::new(2.0, 0.0, 3.0)).length() < 1e-5);
        let tops: Vec<f32> = flight.steps.iter().map(|(_, max)| max.y).collect();
        assert!(tops.len() >= 2, "{tops:?}");
        // Each a riser below the one before, from the threshold down, none too high, the
        // last a riser above the ground where it ends.
        let mut above = 0.0;
        for &top in &tops {
            assert!(
                above - top > 0.05 && above - top <= RISE_M + 1e-4,
                "{tops:?}"
            );
            above = top;
        }
        let end = flight.top + flight.outward * flight.length;
        assert!(above - ground(end) <= RISE_M + 1e-4);
        // Down into the ground under each, and wider than the doorway.
        for &(min, max) in &flight.steps {
            assert!(min.y < ground(min).min(ground(max)));
            assert!(max.x - min.x > 1.0);
        }
        // A walker on it is between the threshold and the ground.
        let middle = flight.top + flight.outward * (flight.length * 0.5);
        let on = flight.height_at(middle).unwrap();
        assert!(on < 0.0 && on > ground(end));
    }

    #[test]
    fn a_door_on_level_ground_needs_no_steps_and_a_wall_without_a_door_has_none() {
        assert!(flights(&with_door(), |_| -0.1).is_empty());
        assert!(flights(&cabin(), |_| -1.0).is_empty());
    }
}
