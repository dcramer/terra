//! Everyone walking a step: the velocity each wants toward the next place on their route,
//! kept out of the others' way and off the walls (ORCA), then each moved on along it.

use super::{
    ARRIVED_M, Detail, EDGE_S, Life, PASSED_M, REROUTE_S, SQUEEZE_S, STANDING_MPS, State,
    THRESHOLD_M,
};
use crate::orca::{self, Body, Horizons, Mover};
use glam::Vec2;

impl Life {
    /// Everyone walking a step on: each wanting toward the next place on their route,
    /// every one kept out of the others' way and off the walls; arrivals taking up their
    /// spot.
    pub(super) fn walk(&mut self, hours: f64, step: f32) {
        let bodies: Vec<Option<Body>> = self
            .people
            .iter()
            .map(|p| {
                let out =
                    !matches!(p.state, State::Inside | State::Away) && p.detail != Detail::Far;
                out.then_some(Body {
                    at: p.at,
                    velocity: p.velocity,
                    radius: p.radius,
                })
            })
            .collect();
        // A coarse grid of who is where, for neighbours.
        let mut grid: std::collections::BTreeMap<(i32, i32), Vec<usize>> = Default::default();
        let key = |at: Vec2| {
            (
                libm::floorf(at.x / 4.0) as i32,
                libm::floorf(at.y / 4.0) as i32,
            )
        };
        for (index, body) in bodies.iter().enumerate() {
            if let Some(body) = body {
                grid.entry(key(body.at)).or_default().push(index);
            }
        }
        let mut chosen: Vec<Option<Vec2>> = vec![None; self.people.len()];
        for (index, person) in self.people.iter().enumerate() {
            let State::Walking { run } = person.state else {
                continue;
            };
            if person.detail == Detail::Far {
                continue;
            }
            let Some(&target) = person.route.get(person.next) else {
                continue;
            };
            let last = person.next + 1 == person.route.len();
            let fastest = if run { person.run } else { person.walk };
            let toward = target - person.at;
            let distance = toward.length();
            let speed = if last {
                fastest * (distance / 1.2).clamp(0.6, 1.0)
            } else {
                fastest
            };
            let wanted = toward.normalize_or_zero() * speed;
            let (cx, cy) = key(person.at);
            let mut neighbours: Vec<(f32, Body)> = Vec::new();
            for x in cx - 1..=cx + 1 {
                for y in cy - 1..=cy + 1 {
                    for &other in grid.get(&(x, y)).into_iter().flatten() {
                        if other == index {
                            continue;
                        }
                        if let Some(mut body) = bodies[other] {
                            let d = body.at.distance(person.at);
                            // Held up long, someone edges between those standing about,
                            // as people in a crowd turn a shoulder.
                            if person.held > EDGE_S
                                && matches!(self.people[other].state, State::Doing { .. })
                            {
                                body.radius *= 0.6;
                            }
                            if d < 5.0 {
                                neighbours.push((d, body));
                            }
                        }
                    }
                }
            }
            // Visitors, who do not give way: seen a little wider, so a walker leaves them
            // room though it takes only its share of the avoiding.
            for visitor in &self.visitors {
                let d = visitor.at.distance(person.at);
                if d < 5.0 {
                    neighbours.push((
                        d,
                        Body {
                            radius: visitor.radius + 0.15,
                            ..*visitor
                        },
                    ));
                }
            }
            neighbours.sort_by(|a, b| a.0.total_cmp(&b.0));
            neighbours.truncate(10);
            let others: Vec<Body> = neighbours.into_iter().map(|(_, b)| b).collect();
            // Held up a while, they squeeze past, shoulders brushing, as people do.
            let squeeze = if person.held > SQUEEZE_S { 0.5 } else { 1.0 };
            let mover = Mover {
                at: person.at,
                velocity: person.velocity,
                wanted,
                radius: person.radius * squeeze,
                fastest,
            };
            let velocity = orca::velocity(&mover, &others, &self.walls, Horizons::default(), step);
            // Barely moving is standing: the body plays a walk only at a walking speed.
            let velocity = if velocity.length() < STANDING_MPS {
                Vec2::ZERO
            } else {
                velocity
            };
            chosen[index] = Some(velocity);
        }
        self.step_on(chosen, hours, step);
    }

    /// Each walker moved by the velocity chosen for them, on along their route as each place
    /// is passed, routed again when held up long, and taking up their spot on arriving.
    fn step_on(&mut self, chosen: Vec<Option<Vec2>>, hours: f64, step: f32) {
        for (index, velocity) in chosen.into_iter().enumerate() {
            let Some(velocity) = velocity else {
                continue;
            };
            let p = &mut self.people[index];
            p.velocity = velocity;
            p.at += velocity * step;
            if velocity.length() > 0.2 {
                let turn = 1.0 - libm::expf(-step * 6.0);
                p.facing = p
                    .facing
                    .lerp(velocity.normalize(), turn)
                    .normalize_or(p.facing);
            }
            // On along the route as each place is passed; a door or a gate, which many go
            // through, is reached at its threshold.
            let through = p.spot.is_some_and(|spot| self.spots[spot].shared);
            while let Some(&target) = p.route.get(p.next) {
                let last = p.next + 1 == p.route.len();
                let reach = match (last, through) {
                    (false, _) => PASSED_M,
                    (true, true) => THRESHOLD_M,
                    (true, false) => ARRIVED_M,
                };
                if p.at.distance(target) <= reach {
                    p.next += 1;
                } else {
                    break;
                }
            }
            // Headway: anyone held for a while is routed again from where they are, once
            // each REROUTE_S, still held: their squeezing and edging past go on meanwhile.
            let was = p.held;
            if p.at.distance(p.held_at) > 0.5 {
                p.held = 0.0;
                p.held_at = p.at;
            } else {
                p.held += step;
            }
            let held = p.held;
            if p.next >= p.route.len() {
                self.arrive(index, hours);
            } else if (held / REROUTE_S).floor() > (was / REROUTE_S).floor() {
                let Some(spot) = self.people[index].spot else {
                    continue;
                };
                let route = self.route_to(&self.people[index], spot);
                let p = &mut self.people[index];
                p.route = route;
                p.next = 0;
            }
        }
    }
}
