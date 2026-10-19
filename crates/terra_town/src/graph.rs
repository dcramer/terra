//! Routes on a town's path graph (`crate::town::Paths`): A* from one node to another,
//! the straight walks between them already known clear of anything built.

use glam::Vec2;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// A path graph: places, and for each the places a straight walk reaches and how far.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Vec2>,
    links: Vec<Vec<(u32, f32)>>,
    /// Which connected part each node is in, as the lowest node in it.
    parts: Vec<u32>,
}

/// A node waiting to be looked at, the cheapest first.
#[derive(Clone, Copy, PartialEq)]
struct Open {
    guess: f32,
    node: u32,
}

impl Eq for Open {}

impl Ord for Open {
    fn cmp(&self, other: &Self) -> Ordering {
        // A max-heap: the lowest guess first, and for a tie the lowest node, so the same
        // graph always gives the same route.
        other
            .guess
            .total_cmp(&self.guess)
            .then_with(|| other.node.cmp(&self.node))
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Graph {
    pub fn new(nodes: Vec<Vec2>, edges: &[[u32; 2]]) -> Self {
        let mut links = vec![Vec::new(); nodes.len()];
        for &[a, b] in edges {
            let length = nodes[a as usize].distance(nodes[b as usize]);
            links[a as usize].push((b, length));
            links[b as usize].push((a, length));
        }
        // Each node's part: the lowest node reached from it.
        let mut parts = vec![u32::MAX; nodes.len()];
        for start in 0..nodes.len() {
            if parts[start] != u32::MAX {
                continue;
            }
            let mut stack = vec![start as u32];
            parts[start] = start as u32;
            while let Some(node) = stack.pop() {
                for &(next, _) in &links[node as usize] {
                    if parts[next as usize] == u32::MAX {
                        parts[next as usize] = start as u32;
                        stack.push(next);
                    }
                }
            }
        }
        Graph {
            nodes,
            links,
            parts,
        }
    }

    /// Whether a node is in the graph's largest part, which a town's ways all are.
    pub fn in_main(&self, node: u32) -> bool {
        let mut sizes = std::collections::BTreeMap::<u32, usize>::new();
        for &part in &self.parts {
            *sizes.entry(part).or_default() += 1;
        }
        let main = sizes
            .iter()
            .max_by_key(|(part, size)| (**size, u32::MAX - **part))
            .map(|(part, _)| *part);
        self.parts.get(node as usize).copied() == main
    }

    /// Whether a way joins two nodes.
    pub fn joined(&self, a: u32, b: u32) -> bool {
        self.parts.get(a as usize) == self.parts.get(b as usize)
    }

    /// The nodes from one to another along the shortest way, both ends included; nothing if
    /// no way joins them.
    pub fn route(&self, from: u32, to: u32) -> Option<Vec<u32>> {
        let count = self.nodes.len();
        if from as usize >= count || to as usize >= count {
            return None;
        }
        let goal = self.nodes[to as usize];
        let mut cost = vec![f32::INFINITY; count];
        let mut came = vec![u32::MAX; count];
        let mut open = BinaryHeap::new();
        cost[from as usize] = 0.0;
        open.push(Open {
            guess: self.nodes[from as usize].distance(goal),
            node: from,
        });
        while let Some(Open { node, guess }) = open.pop() {
            if node == to {
                let mut route = vec![to];
                let mut at = to;
                while at != from {
                    at = came[at as usize];
                    route.push(at);
                }
                route.reverse();
                return Some(route);
            }
            let here = cost[node as usize];
            if guess - self.nodes[node as usize].distance(goal) > here + 1e-3 {
                continue;
            }
            for &(next, length) in &self.links[node as usize] {
                let through = here + length;
                if through < cost[next as usize] {
                    cost[next as usize] = through;
                    came[next as usize] = node;
                    open.push(Open {
                        guess: through + self.nodes[next as usize].distance(goal),
                        node: next,
                    });
                }
            }
        }
        None
    }

    /// How long a route is, in metres.
    pub fn length(&self, route: &[u32]) -> f32 {
        route
            .windows(2)
            .map(|pair| self.nodes[pair[0] as usize].distance(self.nodes[pair[1] as usize]))
            .sum()
    }

    /// Of the `tries` nodes nearest a point, the nearest that `keep` lets through: for a
    /// test too dear to put to every node.
    pub fn nearest_of(&self, at: Vec2, tries: usize, keep: impl Fn(u32) -> bool) -> Option<u32> {
        let mut near: Vec<(f32, u32)> = (0..self.nodes.len() as u32)
            .map(|node| (self.nodes[node as usize].distance_squared(at), node))
            .collect();
        let tries = tries.min(near.len());
        if tries == 0 {
            return None;
        }
        near.select_nth_unstable_by(tries - 1, |a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        near.truncate(tries);
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        near.into_iter()
            .map(|(_, node)| node)
            .find(|&node| keep(node))
    }

    /// The node nearest a point among those `keep` lets through.
    pub fn nearest(&self, at: Vec2, keep: impl Fn(u32) -> bool) -> Option<u32> {
        (0..self.nodes.len() as u32)
            .filter(|&node| keep(node))
            .min_by(|&a, &b| {
                self.nodes[a as usize]
                    .distance_squared(at)
                    .total_cmp(&self.nodes[b as usize].distance_squared(at))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square of four nodes with a long way round and a short cut, and one off alone.
    fn graph() -> Graph {
        Graph::new(
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(10.0, 0.0),
                Vec2::new(10.0, 10.0),
                Vec2::new(0.0, 10.0),
                Vec2::new(50.0, 50.0),
            ],
            &[[0, 1], [1, 2], [2, 3], [0, 3], [0, 2]],
        )
    }

    #[test]
    fn the_shortest_way_is_found_and_none_where_nothing_joins() {
        let graph = graph();
        // Across the diagonal rather than round two sides.
        assert_eq!(graph.route(0, 2), Some(vec![0, 2]));
        assert!((graph.length(&[0, 2]) - 200f32.sqrt()).abs() < 1e-4);
        assert_eq!(graph.route(1, 3).map(|r| graph.length(&r)), Some(20.0));
        assert_eq!(graph.route(0, 4), None);
        assert_eq!(graph.route(2, 2), Some(vec![2]));
    }

    #[test]
    fn the_nearest_node_is_found_among_those_allowed() {
        let graph = graph();
        assert_eq!(graph.nearest(Vec2::new(9.0, 1.0), |_| true), Some(1));
        // Without the first, the next nearest: 7 m off rather than 9.5.
        assert_eq!(graph.nearest(Vec2::new(9.0, 3.0), |n| n != 1), Some(2));
    }
}
