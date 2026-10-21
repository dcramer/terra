//! Walkers keeping out of each other's way and off what is built: optimal reciprocal
//! collision avoidance (ORCA), ported from the RVO2 Library (D196, D324).
//!
//! Each walker wants a velocity (toward the next place on its route). Every neighbour
//! within reach, and every wall edge facing it, rules out a half-plane of velocities that
//! would meet it within a time horizon, each walker taking half the turn for another walker;
//! the velocity kept is the one nearest the wanted that every half-plane allows, found by
//! the library's three small linear programs. Where crowding leaves no velocity allowed,
//! the one breaking the walkers' half-planes least is taken, walls always kept.
//!
//! Ported from `Agent.cpp` of the RVO2 Library 2.0.3 (`computeNewVelocity`,
//! `linearProgram1` to `3`) and its obstacle query (`KdTree.cpp`), as Rust over `glam`, the
//! k-d trees replaced by a grid of wall edges and a list of neighbours the caller gives:
//!
//! > Copyright 2008 University of North Carolina at Chapel Hill.
//! > Licensed under the Apache License, Version 2.0 (the "License"); you may not use this
//! > file except in compliance with the License. You may obtain a copy of the License at
//! > <https://www.apache.org/licenses/LICENSE-2.0>. Unless required by applicable law or
//! > agreed to in writing, software distributed under the License is distributed on an
//! > "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
//! > implied. See the License for the specific language governing permissions and
//! > limitations under the License.
//! >
//! > Please send all bug reports to <geom@cs.unc.edu>. The authors may be contacted via:
//! > Jur van den Berg, Stephen J. Guy, Jamie Snape, Ming C. Lin, Dinesh Manocha, Dept. of
//! > Computer Science, 201 S. Columbia St., Frederick P. Brooks, Jr. Computer Science Bldg.,
//! > Chapel Hill, N.C. 27599-3175, United States of America. <https://gamma.cs.unc.edu/RVO2/>
//!
//! Changes from the original: written in Rust; obstacles found from a grid of edges rather
//! than a k-d tree; vectors from `glam`; the wanted velocity leaned to the right when someone
//! comes the other way on the walker's line ([`keep_right`]), as the Recast/Detour crowd's
//! side penalty does, where the original leaves two on one line standing face to face.

use glam::Vec2;
use std::collections::BTreeMap;

const EPSILON: f32 = 1e-5;

/// A half-plane of allowed velocities: those on the left of a directed line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub point: Vec2,
    pub direction: Vec2,
}

/// A neighbour as a walker sees it: where it is, how it moves, and how wide.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub at: Vec2,
    pub velocity: Vec2,
    pub radius: f32,
}

/// A walker choosing its velocity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mover {
    pub at: Vec2,
    pub velocity: Vec2,
    /// The velocity it would have with nothing in the way.
    pub wanted: Vec2,
    pub radius: f32,
    pub fastest: f32,
}

/// How far ahead walkers look: for each other, and for walls, in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Horizons {
    pub walkers: f32,
    pub walls: f32,
}

impl Default for Horizons {
    fn default() -> Self {
        Horizons {
            walkers: 2.0,
            walls: 1.0,
        }
    }
}

/// One corner of a wall's outline: its point, the way to the next, whether the outline is
/// convex there, and its neighbours round the outline.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Corner {
    point: Vec2,
    unit: Vec2,
    convex: bool,
    next: usize,
    previous: usize,
}

/// Everything walkers must keep off, as outlines, each edge on a grid so the ones near a
/// walker are found at once.
#[derive(Clone, Debug, Default)]
pub struct Walls {
    corners: Vec<Corner>,
    grid: BTreeMap<(i32, i32), Vec<usize>>,
}

/// How big a cell of the walls' grid is, in metres.
const CELL_M: f32 = 8.0;

fn cell(at: Vec2) -> (i32, i32) {
    (
        libm::floorf(at.x / CELL_M) as i32,
        libm::floorf(at.y / CELL_M) as i32,
    )
}

/// The determinant of two vectors: positive when `b` turns left of `a`.
fn det(a: Vec2, b: Vec2) -> f32 {
    a.perp_dot(b)
}

/// How far `c` is left of the line from `a` to `b`, times its length.
fn left_of(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    det(a - c, b - a)
}

/// The squared distance from a point to a segment.
fn to_segment_squared(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    let ab = b - a;
    let r = (c - a).dot(ab) / ab.length_squared();
    if r < 0.0 {
        (c - a).length_squared()
    } else if r > 1.0 {
        (c - b).length_squared()
    } else {
        (c - (a + ab * r)).length_squared()
    }
}

impl Walls {
    /// Adds an outline, a polygon of two or more points in order round it either way.
    pub fn add(&mut self, outline: &[Vec2]) {
        if outline.len() < 2 {
            return;
        }
        // Counterclockwise, as the library needs: its inside on the left of each edge.
        let area: f32 = (0..outline.len())
            .map(|i| det(outline[i], outline[(i + 1) % outline.len()]))
            .sum();
        let mut points = outline.to_vec();
        if area < 0.0 {
            points.reverse();
        }
        let first = self.corners.len();
        let count = points.len();
        for i in 0..count {
            let next = (i + 1) % count;
            let previous = (i + count - 1) % count;
            let convex = count == 2 || left_of(points[previous], points[i], points[next]) >= 0.0;
            self.corners.push(Corner {
                point: points[i],
                unit: (points[next] - points[i]).normalize_or_zero(),
                convex,
                next: first + next,
                previous: first + previous,
            });
        }
        for i in 0..count {
            let (a, b) = (points[i], points[(i + 1) % count]);
            let (low, high) = (cell(a.min(b)), cell(a.max(b)));
            for x in low.0..=high.0 {
                for y in low.1..=high.1 {
                    self.grid.entry((x, y)).or_default().push(first + i);
                }
            }
        }
    }

    /// The edges within `range` of a point that face it, nearest first, as the corners they
    /// start at.
    fn near(&self, at: Vec2, range: f32) -> Vec<usize> {
        let (low, high) = (cell(at - range), cell(at + range));
        let mut found: Vec<(f32, usize)> = Vec::new();
        for x in low.0..=high.0 {
            for y in low.1..=high.1 {
                let Some(edges) = self.grid.get(&(x, y)) else {
                    continue;
                };
                for &edge in edges {
                    if found.iter().any(|&(_, e)| e == edge) {
                        continue;
                    }
                    let (a, b) = (
                        self.corners[edge].point,
                        self.corners[self.corners[edge].next].point,
                    );
                    // Only an edge the walker is outside of, as the library's tree query.
                    if left_of(a, b, at) >= 0.0 {
                        continue;
                    }
                    let distance = to_segment_squared(a, b, at);
                    if distance < range * range {
                        found.push((distance, edge));
                    }
                }
            }
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        found.into_iter().map(|(_, edge)| edge).collect()
    }
}

/// The velocity a walker takes, nearest the one it wants that keeps it off the walls and
/// out of its neighbours' way, over a step of `step` seconds.
pub fn velocity(
    me: &Mover,
    neighbours: &[Body],
    walls: &Walls,
    horizons: Horizons,
    step: f32,
) -> Vec2 {
    let mut lines: Vec<Line> = Vec::new();
    let inverse_walls = 1.0 / horizons.walls;
    let radius = me.radius;
    let range = horizons.walls * me.fastest + radius;
    for first in walls.near(me.at, range) {
        let mut one = walls.corners[first];
        let mut two = walls.corners[one.next];
        let (mut one_index, mut two_index) = (first, one.next);
        let relative1 = one.point - me.at;
        let relative2 = two.point - me.at;
        // Already ruled out by the lines so far?
        let covered = lines.iter().any(|line| {
            det(relative1 * inverse_walls - line.point, line.direction) - inverse_walls * radius
                >= -EPSILON
                && det(relative2 * inverse_walls - line.point, line.direction)
                    - inverse_walls * radius
                    >= -EPSILON
        });
        if covered {
            continue;
        }
        let (distance1, distance2) = (relative1.length_squared(), relative2.length_squared());
        let radius_squared = radius * radius;
        let edge = two.point - one.point;
        let s = (-relative1).dot(edge) / edge.length_squared();
        let line_distance = (-relative1 - edge * s).length_squared();
        if s < 0.0 && distance1 <= radius_squared {
            // Touching its first corner.
            if one.convex {
                lines.push(Line {
                    point: Vec2::ZERO,
                    direction: Vec2::new(-relative1.y, relative1.x).normalize_or_zero(),
                });
            }
            continue;
        } else if s > 1.0 && distance2 <= radius_squared {
            // Touching its second corner, unless the next edge takes care of it.
            if two.convex && det(relative2, two.unit) >= 0.0 {
                lines.push(Line {
                    point: Vec2::ZERO,
                    direction: Vec2::new(-relative2.y, relative2.x).normalize_or_zero(),
                });
            }
            continue;
        } else if (0.0..1.0).contains(&s) && line_distance <= radius_squared {
            // Touching the edge.
            lines.push(Line {
                point: Vec2::ZERO,
                direction: -one.unit,
            });
            continue;
        }
        // Not touching: the legs of the velocity obstacle, both from one corner when the
        // edge is seen end on.
        let leg = |relative: Vec2, distance: f32| {
            let along = libm::sqrtf((distance - radius_squared).max(0.0));
            (
                Vec2::new(
                    relative.x * along - relative.y * radius,
                    relative.x * radius + relative.y * along,
                ) / distance,
                Vec2::new(
                    relative.x * along + relative.y * radius,
                    -relative.x * radius + relative.y * along,
                ) / distance,
            )
        };
        let (mut left_leg, mut right_leg);
        if s < 0.0 && line_distance <= radius_squared {
            if !one.convex {
                continue;
            }
            two = one;
            two_index = one_index;
            (left_leg, right_leg) = leg(relative1, distance1);
        } else if s > 1.0 && line_distance <= radius_squared {
            if !two.convex {
                continue;
            }
            one = two;
            one_index = two_index;
            (left_leg, right_leg) = leg(relative2, distance2);
        } else {
            left_leg = if one.convex {
                leg(relative1, distance1).0
            } else {
                -one.unit
            };
            right_leg = if two.convex {
                leg(relative2, distance2).1
            } else {
                one.unit
            };
        }
        // A leg never points into the next edge round a convex corner: that edge's line is
        // taken instead, and a velocity projected on such a foreign leg adds nothing.
        let left_neighbour = walls.corners[one.previous];
        let (mut left_foreign, mut right_foreign) = (false, false);
        if one.convex && det(left_leg, -left_neighbour.unit) >= 0.0 {
            left_leg = -left_neighbour.unit;
            left_foreign = true;
        }
        if two.convex && det(right_leg, two.unit) <= 0.0 {
            right_leg = two.unit;
            right_foreign = true;
        }
        let same = one_index == two_index;
        let left_cutoff = (one.point - me.at) * inverse_walls;
        let right_cutoff = (two.point - me.at) * inverse_walls;
        let cutoff = right_cutoff - left_cutoff;
        let t = if same {
            0.5
        } else {
            (me.velocity - left_cutoff).dot(cutoff) / cutoff.length_squared()
        };
        let t_left = (me.velocity - left_cutoff).dot(left_leg);
        let t_right = (me.velocity - right_cutoff).dot(right_leg);
        if (t < 0.0 && t_left < 0.0) || (same && t_left < 0.0 && t_right < 0.0) {
            let unit = (me.velocity - left_cutoff).normalize_or_zero();
            lines.push(Line {
                direction: Vec2::new(unit.y, -unit.x),
                point: left_cutoff + unit * radius * inverse_walls,
            });
            continue;
        } else if t > 1.0 && t_right < 0.0 {
            let unit = (me.velocity - right_cutoff).normalize_or_zero();
            lines.push(Line {
                direction: Vec2::new(unit.y, -unit.x),
                point: right_cutoff + unit * radius * inverse_walls,
            });
            continue;
        }
        let to_cutoff = if !(0.0..=1.0).contains(&t) || same {
            f32::INFINITY
        } else {
            (me.velocity - (left_cutoff + cutoff * t)).length_squared()
        };
        let to_left = if t_left < 0.0 {
            f32::INFINITY
        } else {
            (me.velocity - (left_cutoff + left_leg * t_left)).length_squared()
        };
        let to_right = if t_right < 0.0 {
            f32::INFINITY
        } else {
            (me.velocity - (right_cutoff + right_leg * t_right)).length_squared()
        };
        let perpendicular = |direction: Vec2| Vec2::new(-direction.y, direction.x);
        if to_cutoff <= to_left && to_cutoff <= to_right {
            let direction = -one.unit;
            lines.push(Line {
                direction,
                point: left_cutoff + perpendicular(direction) * radius * inverse_walls,
            });
        } else if to_left <= to_right {
            if left_foreign {
                continue;
            }
            lines.push(Line {
                direction: left_leg,
                point: left_cutoff + perpendicular(left_leg) * radius * inverse_walls,
            });
        } else {
            if right_foreign {
                continue;
            }
            let direction = -right_leg;
            lines.push(Line {
                direction,
                point: right_cutoff + perpendicular(direction) * radius * inverse_walls,
            });
        }
    }
    let wall_lines = lines.len();
    walker_lines(me, neighbours, horizons, step, &mut lines);
    let mut result = Vec2::ZERO;
    let failed = program2(
        &lines,
        me.fastest,
        keep_right(me, neighbours),
        false,
        &mut result,
    );
    if failed < lines.len() {
        program3(&lines, wall_lines, failed, me.fastest, &mut result);
    }
    result
}

/// Each neighbour's half-plane: the velocities that keep clear of it within the walkers'
/// horizon, the walker taking half the turn (`computeNewVelocity`'s agent loop).
fn walker_lines(
    me: &Mover,
    neighbours: &[Body],
    horizons: Horizons,
    step: f32,
    lines: &mut Vec<Line>,
) {
    let radius = me.radius;
    let inverse = 1.0 / horizons.walkers;
    for other in neighbours {
        let relative = other.at - me.at;
        let relative_velocity = me.velocity - other.velocity;
        let distance = relative.length_squared();
        let combined = radius + other.radius;
        let combined_squared = combined * combined;
        let (direction, u);
        if distance > combined_squared {
            let w = relative_velocity - relative * inverse;
            let w_squared = w.length_squared();
            let dot1 = w.dot(relative);
            if dot1 < 0.0 && dot1 * dot1 > combined_squared * w_squared {
                // On the cut-off circle.
                let length = libm::sqrtf(w_squared);
                let unit = w / length;
                direction = Vec2::new(unit.y, -unit.x);
                u = unit * (combined * inverse - length);
            } else {
                // On a leg.
                let leg = libm::sqrtf(distance - combined_squared);
                direction = if det(relative, w) > 0.0 {
                    Vec2::new(
                        relative.x * leg - relative.y * combined,
                        relative.x * combined + relative.y * leg,
                    ) / distance
                } else {
                    -Vec2::new(
                        relative.x * leg + relative.y * combined,
                        -relative.x * combined + relative.y * leg,
                    ) / distance
                };
                u = direction * relative_velocity.dot(direction) - relative_velocity;
            }
        } else {
            // Already touching: on the cut-off circle of the step itself.
            let inverse_step = 1.0 / step;
            let w = relative_velocity - relative * inverse_step;
            let length = w.length().max(EPSILON);
            let unit = w / length;
            direction = Vec2::new(unit.y, -unit.x);
            u = unit * (combined * inverse_step - length);
        }
        lines.push(Line {
            point: me.velocity + u * 0.5,
            direction,
        });
    }
}

/// How far ahead a walker looks for someone coming the other way on its line, in metres.
const MEETING_M: f32 = 4.0;
/// How far across a walker leans for them at most, against its speed: about 22 degrees.
const LEAN: f32 = 0.4;

/// The velocity a walker wants, leaned to its right while someone comes the other way on
/// its line, or to its left if they are clearly on its right: each of two meeting head on
/// leans the same way, so they pass. ORCA alone leaves them face to face, each one's
/// half-plane square across their line, the velocity nearest the wanted a standstill.
/// After the side penalty of the Recast/Detour crowd (`dtObstacleAvoidanceQuery`'s
/// `weightSide`, whose side falls to one way when the other is dead ahead), as a lean of the
/// wanted velocity rather than a penalty on sampled ones; zlib licence, Copyright (c)
/// 2009-2010 Mikko Mononen.
fn keep_right(me: &Mover, neighbours: &[Body]) -> Vec2 {
    let speed = me.wanted.length();
    if speed < EPSILON {
        return me.wanted;
    }
    let ahead = me.wanted / speed;
    let mut lean = 0.0f32;
    for other in neighbours {
        let to = other.at - me.at;
        let along = to.dot(ahead);
        let coming = other.velocity.dot(-to.normalize_or_zero());
        if along <= 0.0 || along > MEETING_M || coming < 0.1 {
            continue;
        }
        // Off the line by as much as both are wide, they pass as they are.
        let apart = me.radius + other.radius;
        let across = det(ahead, to);
        if across.abs() >= apart {
            continue;
        }
        let side = if across < -0.05 { -1.0 } else { 1.0 };
        let weight = 1.0 - across.abs() / apart;
        if weight > lean.abs() {
            lean = side * weight;
        }
    }
    let right = Vec2::new(ahead.y, -ahead.x);
    (ahead + right * (lean * LEAN)).normalize() * speed
}

/// The best velocity on one line, within the speed limit and every line before it.
fn program1(
    lines: &[Line],
    index: usize,
    radius: f32,
    wanted: Vec2,
    direction: bool,
    result: &mut Vec2,
) -> bool {
    let line = lines[index];
    let dot = line.point.dot(line.direction);
    let discriminant = dot * dot + radius * radius - line.point.length_squared();
    if discriminant < 0.0 {
        return false;
    }
    let root = libm::sqrtf(discriminant);
    let (mut t_left, mut t_right) = (-dot - root, -dot + root);
    for other in &lines[..index] {
        let denominator = det(line.direction, other.direction);
        let numerator = det(other.direction, line.point - other.point);
        if denominator.abs() <= EPSILON {
            if numerator < 0.0 {
                return false;
            }
            continue;
        }
        let t = numerator / denominator;
        if denominator >= 0.0 {
            t_right = t_right.min(t);
        } else {
            t_left = t_left.max(t);
        }
        if t_left > t_right {
            return false;
        }
    }
    *result = if direction {
        if wanted.dot(line.direction) > 0.0 {
            line.point + line.direction * t_right
        } else {
            line.point + line.direction * t_left
        }
    } else {
        let t = line.direction.dot(wanted - line.point);
        line.point + line.direction * t.clamp(t_left, t_right)
    };
    true
}

/// The velocity nearest the wanted that every line allows; the index of the first line
/// that cannot be met if none does.
fn program2(
    lines: &[Line],
    radius: f32,
    wanted: Vec2,
    direction: bool,
    result: &mut Vec2,
) -> usize {
    *result = if direction {
        wanted * radius
    } else if wanted.length_squared() > radius * radius {
        wanted.normalize() * radius
    } else {
        wanted
    };
    for index in 0..lines.len() {
        if det(lines[index].direction, lines[index].point - *result) > 0.0 {
            let before = *result;
            if !program1(lines, index, radius, wanted, direction, result) {
                *result = before;
                return index;
            }
        }
    }
    lines.len()
}

/// Where no velocity is allowed, the one breaking the walkers' lines least, the walls' kept.
fn program3(lines: &[Line], walls: usize, begin: usize, radius: f32, result: &mut Vec2) {
    let mut distance = 0.0;
    for i in begin..lines.len() {
        if det(lines[i].direction, lines[i].point - *result) > distance {
            let mut projected: Vec<Line> = lines[..walls].to_vec();
            for j in walls..i {
                let determinant = det(lines[i].direction, lines[j].direction);
                let point = if determinant.abs() <= EPSILON {
                    if lines[i].direction.dot(lines[j].direction) > 0.0 {
                        continue;
                    }
                    (lines[i].point + lines[j].point) * 0.5
                } else {
                    lines[i].point
                        + lines[i].direction
                            * (det(lines[j].direction, lines[i].point - lines[j].point)
                                / determinant)
                };
                projected.push(Line {
                    point,
                    direction: (lines[j].direction - lines[i].direction).normalize_or_zero(),
                });
            }
            let before = *result;
            let toward = Vec2::new(-lines[i].direction.y, lines[i].direction.x);
            if program2(&projected, radius, toward, true, result) < projected.len() {
                *result = before;
            }
            distance = det(lines[i].direction, lines[i].point - *result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk(movers: &mut [Mover], walls: &Walls, steps: usize) -> Vec<Vec<Vec2>> {
        let step = 0.1;
        let mut tracks = vec![Vec::new(); movers.len()];
        for _ in 0..steps {
            let bodies: Vec<Body> = movers
                .iter()
                .map(|m| Body {
                    at: m.at,
                    velocity: m.velocity,
                    radius: m.radius,
                })
                .collect();
            let chosen: Vec<Vec2> = movers
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let others: Vec<Body> = bodies
                        .iter()
                        .enumerate()
                        .filter(|(j, b)| *j != i && b.at.distance(m.at) < 6.0)
                        .map(|(_, b)| *b)
                        .collect();
                    velocity(m, &others, walls, Horizons::default(), step)
                })
                .collect();
            for (i, m) in movers.iter_mut().enumerate() {
                m.velocity = chosen[i];
                m.at += m.velocity * step;
                tracks[i].push(m.at);
            }
        }
        tracks
    }

    fn mover(at: Vec2, goal: Vec2) -> Mover {
        Mover {
            at,
            velocity: Vec2::ZERO,
            wanted: (goal - at).normalize() * 1.3,
            radius: 0.3,
            fastest: 1.4,
        }
    }

    #[test]
    fn two_walkers_head_on_pass_each_other_without_touching() {
        let mut movers = [
            mover(Vec2::new(-6.0, 0.0), Vec2::new(6.0, 0.0)),
            mover(Vec2::new(6.0, 0.01), Vec2::new(-6.0, 0.01)),
        ];
        let tracks = walk(&mut movers, &Walls::default(), 120);
        let closest = (0..120)
            .map(|step| tracks[0][step].distance(tracks[1][step]))
            .fold(f32::MAX, f32::min);
        assert!(closest >= 0.6 - 0.02, "they came within {closest:.2} m");
        // And each got past the other.
        assert!(movers[0].at.x > 3.0 && movers[1].at.x < -3.0);
    }

    #[test]
    fn two_walkers_exactly_head_on_pass_each_other() {
        let mut movers = [
            mover(Vec2::new(-6.0, 0.0), Vec2::new(6.0, 0.0)),
            mover(Vec2::new(6.0, 0.0), Vec2::new(-6.0, 0.0)),
        ];
        // On one line, nothing tells them apart but the side each keeps to.
        let tracks = walk(&mut movers, &Walls::default(), 150);
        let closest = (0..150)
            .map(|step| tracks[0][step].distance(tracks[1][step]))
            .fold(f32::MAX, f32::min);
        assert!(closest >= 0.6 - 0.02, "they came within {closest:.2} m");
        assert!(
            movers[0].at.x > 3.0 && movers[1].at.x < -3.0,
            "they stand at {} and {}",
            movers[0].at,
            movers[1].at
        );
    }

    #[test]
    fn a_walker_slides_along_a_wall_rather_than_through_it() {
        let mut walls = Walls::default();
        // A wall across the way, from y -1 to 3, half a metre thick.
        walls.add(&[
            Vec2::new(0.0, -1.0),
            Vec2::new(0.5, -1.0),
            Vec2::new(0.5, 3.0),
            Vec2::new(0.0, 3.0),
        ]);
        let mut walker = [mover(Vec2::new(-4.0, 0.0), Vec2::new(4.0, 0.0))];
        let track = walk(&mut walker, &walls, 80).remove(0);
        for at in track {
            let inside = at.x > -0.3 && at.x < 0.8 && at.y > -1.3 && at.y < 3.3;
            let clear = !inside || at.y < -1.29;
            assert!(clear, "walked into the wall at {at:?}");
        }
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn a_crowd_crossing_keeps_apart() {
        // Eight walkers from round a circle to its far side, all through the middle.
        let mut movers: Vec<Mover> = (0..8)
            .map(|i| {
                let angle = i as f32 / 8.0 * std::f32::consts::TAU;
                let at = Vec2::new(libm::cosf(angle), libm::sinf(angle)) * 6.0;
                mover(at, -at)
            })
            .collect();
        let tracks = walk(&mut movers, &Walls::default(), 200);
        for step in 0..200 {
            for a in 0..8 {
                for b in a + 1..8 {
                    let apart = tracks[a][step].distance(tracks[b][step]);
                    assert!(apart > 0.5, "{a} and {b} {apart:.2} m apart at step {step}");
                }
            }
        }
    }
}
