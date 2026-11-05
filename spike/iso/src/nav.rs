//! Where people can walk: a grid of half-metre cells over the map, blocked under every
//! house and prop, searched with A* and its path pulled straight where nothing is in the
//! way, as the Infinity Engine's search map does (there a painted bitmap, here the props').

use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub const CELL: f32 = 0.5;
pub const HALF: f32 = 32.0;
const N: i32 = (HALF * 2.0 / CELL) as i32;
/// How far a body keeps from what blocks it.
const BODY: f32 = 0.3;

#[derive(Resource)]
pub struct Nav {
    blocked: Vec<bool>,
}

impl Default for Nav {
    fn default() -> Self {
        Nav {
            blocked: vec![false; (N * N) as usize],
        }
    }
}

fn cell(p: Vec2) -> IVec2 {
    ((p + HALF) / CELL).floor().as_ivec2()
}

fn centre(c: IVec2) -> Vec2 {
    (c.as_vec2() + 0.5) * CELL - HALF
}

impl Nav {
    fn inside(c: IVec2) -> bool {
        c.x >= 0 && c.y >= 0 && c.x < N && c.y < N
    }

    pub fn is_blocked(&self, c: IVec2) -> bool {
        !Self::inside(c) || self.blocked[(c.y * N + c.x) as usize]
    }

    /// Blocks a rectangle `half` across each way about `at`, turned by `yaw`.
    pub fn block_rect(&mut self, at: Vec2, half: Vec2, yaw: f32) {
        let half = half + BODY;
        let reach = half.length();
        let (lo, hi) = (cell(at - reach), cell(at + reach));
        let rot = Vec2::from_angle(-yaw);
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let c = IVec2::new(x, y);
                if !Self::inside(c) {
                    continue;
                }
                // Local coordinates: a turn about Y by yaw maps local (x, z) to world.
                let d = centre(c) - at;
                let local = Vec2::new(rot.x * d.x + rot.y * d.y, -rot.y * d.x + rot.x * d.y);
                if local.x.abs() <= half.x && local.y.abs() <= half.y {
                    self.blocked[(y * N + x) as usize] = true;
                }
            }
        }
    }

    pub fn block_circle(&mut self, at: Vec2, radius: f32) {
        let r = radius + BODY;
        let (lo, hi) = (cell(at - r), cell(at + r));
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let c = IVec2::new(x, y);
                if Self::inside(c) && centre(c).distance(at) <= r {
                    self.blocked[(y * N + x) as usize] = true;
                }
            }
        }
    }

    /// The open cell's centre nearest `p`, searching outward a few metres.
    pub fn nearest_open(&self, p: Vec2) -> Option<Vec2> {
        let c = cell(p);
        if !self.is_blocked(c) {
            return Some(p);
        }
        for r in 1i32..12 {
            let mut best: Option<(f32, Vec2)> = None;
            for y in -r..=r {
                for x in -r..=r {
                    if x.abs() != r && y.abs() != r {
                        continue;
                    }
                    let n = c + IVec2::new(x, y);
                    if !self.is_blocked(n) {
                        let d = centre(n).distance_squared(p);
                        if best.is_none_or(|(b, _)| d < b) {
                            best = Some((d, centre(n)));
                        }
                    }
                }
            }
            if let Some((_, q)) = best {
                return Some(q);
            }
        }
        None
    }

    /// Whether a straight walk from `a` to `b` crosses no blocked cell.
    pub fn clear(&self, a: Vec2, b: Vec2) -> bool {
        let steps = (a.distance(b) / (CELL * 0.25)).ceil().max(1.0) as i32;
        (0..=steps).all(|i| !self.is_blocked(cell(a.lerp(b, i as f32 / steps as f32))))
    }

    /// The way from `from` to `to`, as the corners to walk through, `to` last.
    pub fn path(&self, from: Vec2, to: Vec2) -> Option<Vec<Vec2>> {
        let to = self.nearest_open(to)?;
        if self.clear(from, to) {
            return Some(vec![to]);
        }
        let (start, goal) = (cell(self.nearest_open(from)?), cell(to));
        let idx = |c: IVec2| (c.y * N + c.x) as usize;
        let mut cost = vec![f32::INFINITY; (N * N) as usize];
        let mut came = vec![u32::MAX; (N * N) as usize];
        let mut open = BinaryHeap::new();
        cost[idx(start)] = 0.0;
        open.push(Open { f: 0.0, c: start });
        let h = |c: IVec2| (c - goal).as_vec2().length();
        while let Some(Open { c, .. }) = open.pop() {
            if c == goal {
                break;
            }
            for (dx, dy) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ] {
                let n = c + IVec2::new(dx, dy);
                if self.is_blocked(n) {
                    continue;
                }
                // No cutting a blocked corner.
                if dx != 0
                    && dy != 0
                    && (self.is_blocked(c + IVec2::new(dx, 0))
                        || self.is_blocked(c + IVec2::new(0, dy)))
                {
                    continue;
                }
                let g = cost[idx(c)] + if dx != 0 && dy != 0 { 1.414 } else { 1.0 };
                if g < cost[idx(n)] {
                    cost[idx(n)] = g;
                    came[idx(n)] = idx(c) as u32;
                    open.push(Open { f: g + h(n), c: n });
                }
            }
        }
        if came[idx(goal)] == u32::MAX && start != goal {
            return None;
        }
        let mut cells = vec![goal];
        let mut at = idx(goal);
        while at != idx(start) {
            at = came[at] as usize;
            cells.push(IVec2::new(at as i32 % N, at as i32 / N));
        }
        cells.reverse();
        // Pull the path straight: from each corner, the furthest point still in sight.
        let points: Vec<Vec2> = cells.iter().map(|&c| centre(c)).chain([to]).collect();
        let mut out = Vec::new();
        let mut here = from;
        let mut i = 0;
        while i < points.len() - 1 {
            let mut j = points.len() - 1;
            while j > i + 1 && !self.clear(here, points[j]) {
                j -= 1;
            }
            here = points[j];
            out.push(here);
            i = j;
        }
        Some(out)
    }
}

struct Open {
    f: f32,
    c: IVec2,
}

impl PartialEq for Open {
    fn eq(&self, o: &Self) -> bool {
        self.f == o.f
    }
}
impl Eq for Open {}
impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Open {
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_goes_round_a_wall_and_never_through_it() {
        let mut nav = Nav::default();
        nav.block_rect(Vec2::ZERO, Vec2::new(0.5, 6.0), 0.0);
        let path = nav.path(Vec2::new(-4.0, 0.0), Vec2::new(4.0, 0.0)).unwrap();
        assert!(path.len() >= 2, "{path:?}");
        let mut here = Vec2::new(-4.0, 0.0);
        for &p in &path {
            assert!(nav.clear(here, p), "{here} to {p} crosses the wall");
            here = p;
        }
        assert!(here.distance(Vec2::new(4.0, 0.0)) < 0.01);
    }

    #[test]
    fn a_turned_rectangle_blocks_along_its_turned_length() {
        let mut nav = Nav::default();
        // A long thin box along local X, turned a quarter: it now lies along world Z.
        nav.block_rect(Vec2::ZERO, Vec2::new(4.0, 0.2), std::f32::consts::FRAC_PI_2);
        assert!(nav.is_blocked(cell(Vec2::new(0.0, 3.5))));
        assert!(!nav.is_blocked(cell(Vec2::new(3.5, 0.0))));
    }
}
