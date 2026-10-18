//! A town's walls and the castle over it (D381): a curtain of stone closing the town round a
//! few metres past its last houses and yards, towers standing out of it at its corners and
//! along its long sides, a gatehouse over each street where it goes out; and the castle (the
//! burg, `scenes::burg`) on the highest ground beside the town, its gate toward it. Each a
//! building of the town's, after its houses (`Town::works`), the castle last, so a settlement
//! rising walls itself and crowns itself as it is finished, as towns that grew rich did.

use super::*;
use crate::BuildOp;
use crate::scenes::burg;

/// How far out past its outermost building, yard or the square the wall's outer face runs.
const WALL_OUT_M: f32 = 10.0;
/// How far in a corner of the wall's line may be cut off to make it plainer, at most.
const CUT_M: f32 = 3.0;
/// The longest length of curtain put up as one piece, so each stands level on its own ground.
const PIECE_M: f32 = 12.0;
/// How far apart the towers along a side stand, about.
const TOWER_EVERY_M: f32 = 48.0;
/// Half a tower's width and a gatehouse's, and how far a length of curtain keeps from either.
const TOWER_HALF_M: f32 = 2.5;
const GATE_HALF_M: f32 = 4.0;
const GAP_M: f32 = 0.1;
/// How far out from the tower's foot the curtain's outer face runs (`burg::wall_tower`).
const TOWER_OUT_M: f32 = 1.5;
/// How far past the wall the castle's gate may stand, the least and the most, and how many
/// ways round the town its place is looked for.
const CASTLE_OUT_M: (f32, f32) = (25.0, 60.0);
const CASTLE_WAYS: u32 = 48;

impl Town {
    /// The town's wall, its towers and gates, and the castle, as buildings after the rest.
    pub(super) fn walls(&mut self, seed: u32, picks: impl Picks + Copy, heights: Heights) {
        let kinds = picks.pick(hash::of(&[seed, 0x3a11]));
        let m = burg::Materials {
            stone: kinds.masonry,
            beams: kinds.beams,
            logs: kinds.timber,
            shakes: kinds.timber_roof,
        };
        let mut points: Vec<Vec2> = self
            .footprints
            .iter()
            .flatten()
            .flatten()
            .copied()
            .collect();
        points.extend(self.yards.iter().flat_map(|(_, yard)| yard.iter().copied()));
        points.extend(self.square.outline.iter().copied());
        let ring = wall_line(&points);
        if ring.len() < 3 {
            return;
        }
        let middle = ring.iter().copied().sum::<Vec2>() / ring.len() as f32;
        let mut works: Vec<(String, Vec2, f32, Vec<BuildOp>)> = Vec::new();
        for i in 0..ring.len() {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            let prev = ring[(i + ring.len() - 1) % ring.len()];
            let long = a.distance(b);
            let along = (b - a) / long;
            let out = Vec2::new(-along.y, along.x);
            // The tower at its first corner, square to the corner's halfway way out.
            let corner_out = (out + Vec2::new(-(a - prev).y, (a - prev).x).normalize()).normalize();
            works.push((
                "a tower of the town wall".into(),
                a + corner_out * TOWER_OUT_M,
                facing_of(-corner_out),
                burg::wall_tower(m),
            ));
            // Where streets go out through it: a gatehouse over each.
            let gates: Vec<f32> = self
                .streets
                .iter()
                .filter_map(|street| {
                    // A street ending short of the wall goes on out through it.
                    let mut line = street.points.clone();
                    if let [.., p, q] = line[..] {
                        line.push(q + (q - p).normalize_or_zero() * 4.0 * WALL_OUT_M);
                    }
                    line.windows(2)
                        .find_map(|pair| crossing(pair[0], pair[1], a, b))
                })
                // Clear of the towers at its corners, near where it crosses.
                .map(|t| t * long)
                .filter(|_| long > 2.0 * (TOWER_HALF_M + GATE_HALF_M + GAP_M))
                .map(|t| {
                    t.clamp(
                        TOWER_HALF_M + GATE_HALF_M + GAP_M,
                        long - TOWER_HALF_M - GATE_HALF_M - GAP_M,
                    )
                })
                .collect();
            let mut stops: Vec<(f32, f32)> = vec![(0.0, TOWER_HALF_M), (long, TOWER_HALF_M)];
            for &t in &gates {
                works.push((
                    "a gatehouse of the town wall".into(),
                    a + along * t,
                    facing_of(out),
                    burg::town_gate(m),
                ));
                stops.push((t, GATE_HALF_M));
            }
            // Towers along a long side, clear of its gates.
            let towers = libm::roundf(long / TOWER_EVERY_M).max(1.0) as usize;
            for k in 1..towers {
                let t = long * k as f32 / towers as f32;
                if gates
                    .iter()
                    .any(|&g| (g - t).abs() < GATE_HALF_M + TOWER_HALF_M + 4.0)
                {
                    continue;
                }
                works.push((
                    "a tower of the town wall".into(),
                    a + along * t + out * TOWER_OUT_M,
                    facing_of(-out),
                    burg::wall_tower(m),
                ));
                stops.push((t, TOWER_HALF_M));
            }
            stops.sort_by(|p, q| p.0.total_cmp(&q.0));
            // The curtain between them, in pieces of at most `PIECE_M`.
            for pair in stops.windows(2) {
                let (from, to) = (pair[0].0 + pair[0].1 + GAP_M, pair[1].0 - pair[1].1 - GAP_M);
                if to - from < 2.0 * CELL_M {
                    continue;
                }
                let pieces = libm::ceilf((to - from) / PIECE_M).max(1.0) as usize;
                let each = (to - from) / pieces as f32;
                for p in 0..pieces {
                    let cubes = libm::roundf(each / CELL_M) as i32;
                    let mid = from + each * (p as f32 + 0.5);
                    works.push((
                        "a length of the town wall".into(),
                        a + along * mid,
                        libm::atan2f(-along.y, along.x),
                        burg::curtain(m, cubes),
                    ));
                }
            }
        }
        // The castle, last: on the highest ground a little way out past the wall, its gate
        // toward the town.
        let reach = |way: Vec2| {
            ring.iter()
                .map(|&p| (p - middle).dot(way))
                .fold(0.0f32, f32::max)
        };
        let castle = (0..CASTLE_WAYS)
            .flat_map(|w| {
                let angle = w as f32 / CASTLE_WAYS as f32 * core::f32::consts::TAU;
                let way = Vec2::new(libm::cosf(angle), libm::sinf(angle));
                [0.0, 0.5, 1.0].map(|s| {
                    middle
                        + way
                            * (reach(way) + CASTLE_OUT_M.0 + (CASTLE_OUT_M.1 - CASTLE_OUT_M.0) * s)
                })
            })
            .max_by(|p, q| heights(*p).total_cmp(&heights(*q)));
        if let Some(at) = castle {
            works.push((
                "the castle".into(),
                at,
                facing_of(middle - at),
                burg::castle(m),
            ));
        }
        for (name, at, facing, ops) in works {
            let building = village::Building {
                name,
                plan: Plan::Wall,
                scheme: Scheme::Masonry,
                at: at.to_array(),
                facing,
                ops,
            };
            let rects = ground_rects(&building);
            self.footprints.push(footprint(&building, &rects));
            self.buildings.push(building);
        }
    }
}

/// The line of a wall's outer face round points: their hull, every side moved out by
/// `WALL_OUT_M`, its corners that turn least cut off while that takes no more than `CUT_M`
/// in, in order round it with the town on each side's left of its way (+90° from it, in
/// metres east and south, being out).
fn wall_line(points: &[Vec2]) -> Vec<Vec2> {
    let hull = hull(points);
    let n = hull.len();
    if n < 3 {
        return hull;
    }
    let normal = |a: Vec2, b: Vec2| {
        let along = (b - a).normalize();
        Vec2::new(-along.y, along.x)
    };
    let mut ring: Vec<Vec2> = (0..n)
        .map(|i| {
            let (prev, at, next) = (hull[(i + n - 1) % n], hull[i], hull[(i + 1) % n]);
            let (n1, n2) = (normal(prev, at), normal(at, next));
            at + (n1 + n2) * (WALL_OUT_M / (1.0 + n1.dot(n2)).max(0.3))
        })
        .collect();
    loop {
        let n = ring.len();
        if n <= 4 {
            break;
        }
        let cut = |i: usize| {
            let (prev, at, next) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
            point_segment(at, prev, next)
        };
        let Some(least) = (0..n).min_by(|&i, &j| cut(i).total_cmp(&cut(j))) else {
            break;
        };
        if cut(least) > CUT_M {
            break;
        }
        ring.remove(least);
    }
    ring
}

/// The convex hull of points, in order round it with the inside on each side's right in
/// x east, y south terms (so +90° from a side's way is out).
fn hull(points: &[Vec2]) -> Vec<Vec2> {
    let mut sorted: Vec<Vec2> = points.to_vec();
    sorted.sort_by(|p, q| p.x.total_cmp(&q.x).then(p.y.total_cmp(&q.y)));
    sorted.dedup();
    if sorted.len() < 3 {
        return sorted;
    }
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut lower: Vec<Vec2> = Vec::new();
    for &p in &sorted {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<Vec2> = Vec::new();
    for &p in sorted.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    // Counter-clockwise in x, y: the inside on each side's left, (-y, x) from its way, so
    // turned round, (-y, x) is out.
    lower.reverse();
    lower
}

/// Where a line from `p` to `q` crosses one from `a` to `b`, as a share of the way from `a`.
fn crossing(p: Vec2, q: Vec2, a: Vec2, b: Vec2) -> Option<f32> {
    let (r, s) = (q - p, b - a);
    let d = r.perp_dot(s);
    if d.abs() < 1e-6 {
        return None;
    }
    let t = (a - p).perp_dot(s) / d;
    let u = (a - p).perp_dot(r) / d;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(u)
}
