//! People's routes through a town: on to the path graph, along it in their own lane, and
//! round anyone standing in the way.

use super::{Detail, Life, Person, SPOT_CLEAR_M, State};
use crate::town::{self, On};
use glam::Vec2;

impl Life {
    /// A route from where someone is to a spot: on to the graph at the node nearest them
    /// that they can walk straight to, along it, and to the spot from its node, each
    /// street's and square's place moved across by their own lane where that is clear.
    pub(super) fn route_to(&self, person: &Person, spot: usize) -> Vec<Vec2> {
        let target = &self.spots[spot];
        let joined = |n: u32| self.graph.joined(n, target.node);
        // From a spot, back the way it is come to; else the nearest node they can walk to.
        let leaving = person
            .last_spot
            .map(|spot| &self.spots[spot])
            .filter(|spot| spot.at.distance(person.at) < 0.6 && joined(spot.node))
            .map(|spot| spot.node);
        // On to the graph clear of where others stand at spots, as its ways are, if any
        // near node is: someone drawing at a well stands between it and a way alongside.
        let onto = |past: bool| {
            self.graph.nearest_of(person.at, 12, |n| {
                let node = self.graph.nodes[n as usize];
                joined(n)
                    && self.clear.walkable(person.at, node)
                    && (!past || self.past_spots(person.at, node))
            })
        };
        let from = leaving
            .or_else(|| onto(true))
            .or_else(|| onto(false))
            .or_else(|| self.graph.nearest(person.at, joined));
        let Some(from) = from else {
            return vec![target.at];
        };
        let Some(nodes) = self.graph.route(from, target.node) else {
            return vec![target.at];
        };
        let mut points: Vec<Vec2> = Vec::with_capacity(nodes.len() + 1);
        for (k, &node) in nodes.iter().enumerate() {
            let at = self.graph.nodes[node as usize];
            let on = self.town.paths.nodes[node as usize].1;
            let mut point = at;
            if matches!(on, On::Street(_) | On::Square) && k > 0 && k + 1 < nodes.len() {
                let before = self.graph.nodes[nodes[k - 1] as usize];
                let after = self.graph.nodes[nodes[k + 1] as usize];
                let way = (after - before).normalize_or_zero();
                let moved = at + way.perp() * person.lane;
                let previous = points.last().copied().unwrap_or(person.at);
                if self.clear.walkable(previous, moved)
                    && self.clear.walkable(moved, after)
                    && self.past_spots(previous, moved)
                    && self.past_spots(moved, after)
                {
                    point = moved;
                }
            }
            points.push(point);
        }
        // Not back to the first node when the second is as straight a walk from here.
        if points.len() >= 2
            && person.at.distance(points[1]) <= points[0].distance(points[1])
            && self.clear.walkable(person.at, points[1])
            && self.past_spots(person.at, points[1])
        {
            points.remove(0);
        }
        points.push(target.at);
        self.round_standing(person, points)
    }

    /// A route stepping round anyone standing at a spot on its way, on whichever side is
    /// clear, as someone steps round a neighbour drawing water: a standing walker is an
    /// obstacle to plan round, as a crowd's stationary agents are (Unity's carving
    /// `NavMeshObstacle`), where avoidance alone would press them between it and a wall.
    fn round_standing(&self, person: &Person, mut points: Vec<Vec2>) -> Vec<Vec2> {
        let standing: Vec<(Vec2, f32)> = self
            .people
            .iter()
            .filter(|other| !std::ptr::eq(*other, person) && other.detail != Detail::Far)
            .filter(|other| matches!(other.state, State::Doing { .. }))
            .map(|other| (other.at, person.radius + other.radius))
            .collect();
        let mut from = person.at;
        let mut k = 0;
        // Each stood round once, so a crowd does not send them in circles.
        let mut rounded: Vec<Vec2> = Vec::new();
        while k < points.len() {
            let to = points[k];
            let way = (to - from).normalize_or_zero();
            let along = |at: Vec2| (at - from).dot(way);
            let in_way = standing
                .iter()
                .filter(|(at, _)| at.distance(to) > 0.05 && !rounded.contains(at))
                .filter(|&&(at, apart)| town::point_segment(at, from, to) < apart + 0.1)
                .min_by(|a, b| along(a.0).total_cmp(&along(b.0)));
            let Some(&(at, apart)) = in_way else {
                from = to;
                k += 1;
                continue;
            };
            // To the side of them the way already leans, if it is clear, else the other.
            let across = way.perp();
            let lean = if (from + way * along(at) - at).dot(across) >= 0.0 {
                1.0
            } else {
                -1.0
            };
            // Clear of what is built, and of everyone else standing: no nearer any than its
            // ends are, so a ring of people talking is gone round, not into.
            let open = |a: Vec2, b: Vec2| {
                self.clear.walkable(a, b)
                    && standing.iter().all(|&(other, apart)| {
                        let ends = other.distance(a).min(other.distance(b)) - 0.05;
                        town::point_segment(other, a, b) >= apart.min(ends)
                    })
            };
            let round = [lean, -lean].into_iter().find_map(|side| {
                let wide = at + across * side * (apart + 0.5);
                let pair = [wide - way * 0.6, wide + way * 0.6];
                let clear = open(from, pair[0]) && open(pair[0], pair[1]) && open(pair[1], to);
                clear.then_some(pair)
            });
            let Some([before, after]) = round else {
                from = to;
                k += 1;
                continue;
            };
            points.splice(k..k, [before, after]);
            rounded.push(at);
            from = after;
            k += 2;
        }
        points
    }

    /// Whether a straight walk keeps clear of where anyone would stand at a spot, as the
    /// ways between nodes do (`SPOT_CLEAR_M`), or at least comes no nearer one than its ends
    /// are: one set off from or gone to, or already brushed against.
    fn past_spots(&self, a: Vec2, b: Vec2) -> bool {
        self.spots.iter().all(|spot| {
            let ends = spot.at.distance(a).min(spot.at.distance(b)) - 0.05;
            spot.shared || town::point_segment(spot.at, a, b) >= SPOT_CLEAR_M.min(ends)
        })
    }
}
