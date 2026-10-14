//! What stands on the ground and is solid to a walker: a tree's trunk, a rock, a stump, a
//! fallen log (D267). Each is fitted once from its model, the way shipped games fit simple
//! colliders to a visual mesh rather than colliding with the mesh itself:
//!
//! - **Its relief**: the highest point of the model over each cell of a fine grid laid over
//!   its plan, worked out once a kind of model (`Relief::of`).
//! - **Its pieces**: convex outlines in plan round the cells that show above the ground,
//!   each with a rounded edge (a convex radius, as Jolt and Havok give their convex shapes)
//!   and its own top. A shape that a single outline would fill out with empty corners, as a
//!   bent log or an L of rock, is cut in two along its length until the outlines fit (a few
//!   rounds of the recursive splitting V-HACD does in 3D), and so is a long one, as a log,
//!   whose thin end may be low when its thick end is not. A circle is an outline of one
//!   point, a capsule one of two.
//! - **Its surface**: the cells themselves, which hold a walker up where it may stand.
//!
//! A walker is stopped by a piece whose top is more than a step above its feet, and steps
//! onto anything lower, as a character controller's step offset lets it (Unity's, Godot's).
//! A tall piece stops them at its foot however its sides slope, so it is not climbed a step
//! at a time; only a jump puts them on it.
//! Everything is measured in the plan of the thing's own place on the ground: across its
//! two level directions and up from the ground under its middle.

use glam::{Vec2, Vec3};

/// The most cells a relief is laid out in: its cell is as wide as this allows, within
/// `CELL_M`.
const CELLS: f32 = 600.0;
/// The finest and coarsest cell, in the model's own units.
const CELL_M: (f32, f32) = (0.02, 0.15);
/// Below this height over the ground a cell is not counted at all: it is the ground.
const GROUND_M: f32 = 0.03;
/// An outline is cut in two while it covers more than this many times the area of its
/// cells, up to `ROUNDS` times.
const LOOSE: f32 = 1.4;
const ROUNDS: u32 = 5;
/// An outline longer than this, and than twice its width, is cut in two too.
const LONG_M: f32 = 1.2;

/// The highest point of a model over each cell of a grid laid over its plan, in its own
/// frame (y up), as `(x, height, z)` at the point that was highest: its relief.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Relief {
    pub cells: Vec<Vec3>,
    /// How wide a cell is, in the model's units.
    pub cell: f32,
}

impl Relief {
    /// The relief of a model: its vertices, in its own frame, and its triangles as three
    /// indices each, sampled finely enough that no cell under them is missed. Without
    /// triangles, of the vertices alone. Its cells are as fine as a few hundred of them
    /// over its plan allow.
    pub fn of(positions: &[Vec3], triangles: &[u32]) -> Relief {
        let (low, high) = bounds(positions);
        let size = (high - low).max(Vec3::splat(1e-3));
        let cell = (size.x * size.z / CELLS).sqrt().clamp(CELL_M.0, CELL_M.1);
        Self::with_cell(positions, triangles, cell)
    }

    /// The relief of a model in cells `cell` wide. A hole closed in on all four sides, as
    /// the open top of a hollow tube is, is filled at the lowest of its sides' heights, so a
    /// stump is solid under its sawn top.
    pub fn with_cell(positions: &[Vec3], triangles: &[u32], cell: f32) -> Relief {
        if positions.is_empty() {
            return Relief::default();
        }
        let (low, high) = bounds(positions);
        let size = (high - low).max(Vec3::splat(1e-3));
        let across = (size.x / cell).floor() as usize + 1;
        let deep = (size.z / cell).floor() as usize + 1;
        let mut best: Vec<Option<Vec3>> = vec![None; across * deep];
        let mut keep = |p: Vec3| {
            let i = (((p.x - low.x) / cell).floor().max(0.0) as usize).min(across - 1);
            let j = (((p.z - low.z) / cell).floor().max(0.0) as usize).min(deep - 1);
            let slot = &mut best[j * across + i];
            if slot.is_none_or(|kept| p.y > kept.y) {
                *slot = Some(p);
            }
        };
        positions.iter().for_each(|p| keep(*p));
        for corners in triangles.chunks_exact(3) {
            let [a, b, c] = [0, 1, 2].map(|k| positions[corners[k] as usize]);
            // Steps of half a cell across the triangle.
            let span = [a.distance(b), b.distance(c), c.distance(a)]
                .into_iter()
                .fold(0.0, f32::max);
            let steps = ((span / (cell * 0.5)).ceil() as usize).clamp(1, 256);
            for i in 0..=steps {
                for j in 0..=steps - i {
                    let (u, v) = (i as f32 / steps as f32, j as f32 / steps as f32);
                    keep(a + (b - a) * u + (c - a) * v);
                }
            }
        }
        // The nearest kept cell's height each way along its row and column.
        let height = |i: usize, j: usize| best[j * across + i].map(|p| p.y);
        let mut filled = best.clone();
        for j in 0..deep {
            for i in 0..across {
                if best[j * across + i].is_some() {
                    continue;
                }
                let sides = [
                    (0..i).rev().find_map(|k| height(k, j)),
                    (i + 1..across).find_map(|k| height(k, j)),
                    (0..j).rev().find_map(|k| height(i, k)),
                    (j + 1..deep).find_map(|k| height(i, k)),
                ];
                if let [Some(a), Some(b), Some(c), Some(d)] = sides {
                    let middle =
                        low + Vec3::new((i as f32 + 0.5) * cell, 0.0, (j as f32 + 0.5) * cell);
                    filled[j * across + i] = Some(middle.with_y(a.min(b).min(c).min(d)));
                }
            }
        }
        Relief {
            cells: filled.into_iter().flatten().collect(),
            cell,
        }
    }
}

fn bounds(points: &[Vec3]) -> (Vec3, Vec3) {
    let first = points.first().copied().unwrap_or_default();
    points
        .iter()
        .fold((first, first), |(low, high), p| (low.min(*p), high.max(*p)))
}

/// Where a relief stands: the point on the ground under its frame's origin, the directions
/// its frame's x, z and y point in the world, how much it is scaled, and how high its origin
/// is over the ground (less than nothing when it is sunk into it).
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub ground: Vec3,
    pub x: Vec3,
    pub z: Vec3,
    pub up: Vec3,
    pub scale: f32,
    pub lift: f32,
}

/// One convex part of a solid thing, in plan: an outline, rounded by `radius`, how tall it
/// stands over the ground under it, `top`, and how high its highest point is over the level
/// of its thing's ground, `crest`.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    /// Its corners, anticlockwise; one for a circle, two for a capsule.
    pub hull: Vec<Vec2>,
    pub radius: f32,
    pub top: f32,
    pub crest: f32,
}

impl Piece {
    /// A round one standing on level ground, such as a trunk.
    pub fn round(at: Vec2, radius: f32, top: f32) -> Self {
        Self {
            hull: vec![at],
            radius,
            top,
            crest: top,
        }
    }

    /// How far a point is outside its edge, less than nothing inside it, and which way is out.
    pub fn distance(&self, at: Vec2) -> (f32, Vec2) {
        let (apart, out) = outline_distance(&self.hull, at);
        (apart - self.radius, out)
    }

    /// The middle of its outline and how far round it reaches.
    pub fn bounds(&self) -> (Vec2, f32) {
        let middle = self.hull.iter().copied().sum::<Vec2>() / self.hull.len().max(1) as f32;
        let reach = self
            .hull
            .iter()
            .map(|p| p.distance(middle))
            .fold(0.0, f32::max);
        (middle, reach + self.radius)
    }

    /// Whether it stops a walker whose feet are so high over the level of its thing's ground:
    /// when it is taller than a step, and its highest point more than a step above the feet.
    /// One no taller than a step is stepped onto from wherever the walker comes at it.
    pub fn stops(&self, feet: f32, step: f32) -> bool {
        self.top > step && self.crest - step > feet
    }
}

/// A solid thing standing on the ground, fitted to its model where it was placed.
#[derive(Clone, Debug)]
pub struct Solid {
    /// The point on the ground under its middle, which it is filed and removed by.
    pub at: Vec3,
    x: Vec3,
    z: Vec3,
    up: Vec3,
    /// Its convex parts.
    pub pieces: Vec<Piece>,
    /// What holds a walker up: across, height over the level of its ground, and along, in
    /// plan.
    surface: Vec<Vec3>,
    /// How far round `at`, in plan, any of it reaches.
    pub reach: f32,
}

/// A cell of a relief where it stands: across, height over the level of the thing's ground,
/// and along, in plan; and its height over the ground under it.
type Cell = (Vec3, f32);

impl Solid {
    /// A thing of a few given pieces with no surface to stand on, such as a trunk.
    pub fn of_pieces(at: Vec3, pieces: Vec<Piece>) -> Self {
        let up = at.normalize_or(Vec3::Y);
        let x = up.any_orthonormal_vector();
        Self::new(at, x, up.cross(x), up, pieces, Vec::new())
    }

    fn new(at: Vec3, x: Vec3, z: Vec3, up: Vec3, pieces: Vec<Piece>, surface: Vec<Vec3>) -> Self {
        let reach = pieces
            .iter()
            .map(|piece| {
                let (middle, reach) = piece.bounds();
                middle.length() + reach
            })
            .chain(surface.iter().map(|p| Vec2::new(p.x, p.z).length()))
            .fold(0.0, f32::max);
        Self {
            at,
            x,
            z,
            up,
            pieces,
            surface,
            reach,
        }
    }

    /// Fits a relief where it stands: pieces round all of it that shows above the ground,
    /// each as tall as the highest of it, and its surface. `ground` says how high the ground
    /// is over the level of the ground under its origin at a point in plan, so a thing on a
    /// hillside is as tall as it shows above the slope. `extra` adds pieces in its plan, such
    /// as a spruce's skirt. None when nothing of it shows above the ground.
    pub fn fit(
        relief: &Relief,
        place: Placement,
        ground: impl Fn(Vec2) -> f32,
        extra: Vec<Piece>,
    ) -> Option<Self> {
        let s = place.scale;
        let cells: Vec<Cell> = relief
            .cells
            .iter()
            .map(|c| {
                let (plan, height) = (Vec2::new(c.x, c.z) * s, c.y * s + place.lift);
                (Vec3::new(plan.x, height, plan.y), height - ground(plan))
            })
            .filter(|(_, over)| *over > GROUND_M)
            .collect();
        if cells.is_empty() && extra.is_empty() {
            return None;
        }
        let cell = relief.cell * s;
        let mut pieces = Vec::new();
        split(&cells, cell, 0, &mut pieces);
        pieces.extend(extra);
        let surface = cells.into_iter().map(|(c, _)| c).collect();
        Some(Self::new(
            place.ground,
            place.x,
            place.z,
            place.up,
            pieces,
            surface,
        ))
    }

    /// A point in the world in this thing's plan, and how high it is over the level of its
    /// ground.
    pub fn plan(&self, at: Vec3) -> (Vec2, f32) {
        let d = at - self.at;
        (Vec2::new(d.dot(self.x), d.dot(self.z)), d.dot(self.up))
    }

    /// The point in the world at a point in plan, so high over the level of its ground.
    pub fn point(&self, plan: Vec2, height: f32) -> Vec3 {
        self.at + self.world(plan) + self.up * height
    }

    /// A direction in plan, in the world.
    pub fn world(&self, across: Vec2) -> Vec3 {
        self.x * across.x + self.z * across.y
    }

    /// How tall its tallest piece stands over the ground.
    pub fn top(&self) -> Option<f32> {
        self.pieces.iter().map(|piece| piece.top).reduce(f32::max)
    }

    /// Moves a round body of `radius`, its feet at `at`, out of every piece of this thing it
    /// cannot step onto (`Piece::stops`). A body pushed out of one piece into another
    /// overlapping it is pushed again, a few times.
    pub fn push_out(&self, at: Vec3, radius: f32, step: f32) -> Vec3 {
        let (mut plan, feet) = self.plan(at);
        let start = plan;
        for _ in 0..3 {
            let mut moved = false;
            for piece in self.pieces.iter().filter(|piece| piece.stops(feet, step)) {
                let (apart, out) = piece.distance(plan);
                if apart < radius {
                    plan += out * (radius - apart);
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        at + self.world(plan - start)
    }

    /// Whether a round body of `radius`, its feet at `at`, stands in a piece it cannot step
    /// onto.
    pub fn blocks(&self, at: Vec3, radius: f32, step: f32) -> bool {
        let (plan, feet) = self.plan(at);
        self.pieces
            .iter()
            .any(|piece| piece.stops(feet, step) && piece.distance(plan).0 < radius - 1e-3)
    }

    /// How high over the level of this thing's ground a foot of `radius` at `at` stands on
    /// it: the highest of its surface within the foot that is no more than `step` above the
    /// feet. None when none of it is.
    pub fn surface(&self, at: Vec3, radius: f32, step: f32) -> Option<f32> {
        let (plan, feet) = self.plan(at);
        if plan.length() > self.reach + radius {
            return None;
        }
        let r2 = radius * radius;
        self.surface
            .iter()
            .filter(|c| c.y - feet <= step && Vec2::new(c.x, c.z).distance_squared(plan) <= r2)
            .map(|c| c.y)
            .reduce(f32::max)
    }
}

/// Fits pieces round cells, cutting them in two along their length while one outline would
/// cover much more than they do.
fn split(cells: &[Cell], cell: f32, round: u32, pieces: &mut Vec<Piece>) {
    if cells.is_empty() {
        return;
    }
    let plan: Vec<Vec2> = cells.iter().map(|(c, _)| Vec2::new(c.x, c.z)).collect();
    let hull = convex_hull(&plan);
    let covered = cells.len() as f32 * cell * cell;
    let middle = plan.iter().copied().sum::<Vec2>() / plan.len() as f32;
    let axis = long_axis(&plan, middle);
    let spread = |axis: Vec2| {
        let (low, high) = plan.iter().fold((f32::MAX, f32::MIN), |(low, high), p| {
            let t = (*p - middle).dot(axis);
            (low.min(t), high.max(t))
        });
        high - low
    };
    let (long, wide) = (spread(axis), spread(axis.perp()));
    // Loose, as a bent log's or an L's outline is; or long, as a log is, whose thin end may
    // be low enough to step onto when its thick end is not.
    let loose = area(&hull) > covered * LOOSE;
    let lengthy = long > (wide * 2.0).max(LONG_M);
    if round < ROUNDS && cells.len() > 8 && (loose || lengthy) {
        // Along its longest direction, at its middle; cells within a cell of the cut go
        // to both halves, so the two overlap and leave no seam.
        let along = |(c, _): &Cell| (Vec2::new(c.x, c.z) - middle).dot(axis);
        let one: Vec<Cell> = cells.iter().copied().filter(|c| along(c) < cell).collect();
        let two: Vec<Cell> = cells.iter().copied().filter(|c| along(c) > -cell).collect();
        if one.len() < cells.len() && two.len() < cells.len() {
            split(&one, cell, round + 1, pieces);
            split(&two, cell, round + 1, pieces);
            return;
        }
    }
    pieces.push(Piece {
        hull,
        // A cell's highest point may be up to a cell in from the model's edge.
        radius: cell * 0.7,
        top: cells.iter().map(|(_, over)| *over).fold(0.0, f32::max),
        crest: cells.iter().map(|(c, _)| c.y).fold(f32::MIN, f32::max),
    });
}

/// The direction points spread furthest in, from their covariance.
fn long_axis(points: &[Vec2], middle: Vec2) -> Vec2 {
    let (mut xx, mut xz, mut zz) = (0.0, 0.0, 0.0);
    for p in points {
        let d = *p - middle;
        xx += d.x * d.x;
        xz += d.x * d.y;
        zz += d.y * d.y;
    }
    // The larger eigenvector of [[xx, xz], [xz, zz]].
    let half = (xx - zz) * 0.5;
    let big = (xx + zz) * 0.5 + (half * half + xz * xz).sqrt();
    let axis = if xz.abs() > 1e-9 {
        Vec2::new(big - zz, xz)
    } else if xx >= zz {
        Vec2::X
    } else {
        Vec2::Y
    };
    axis.normalize_or(Vec2::X)
}

/// The convex hull of points, anticlockwise, by Andrew's monotone chain. Deterministic: ties
/// are broken by the other coordinate.
pub fn convex_hull(points: &[Vec2]) -> Vec<Vec2> {
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    sorted.dedup();
    if sorted.len() < 3 {
        return sorted;
    }
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut hull: Vec<Vec2> = Vec::with_capacity(sorted.len() * 2);
    for pass in 0..2 {
        let floor = hull.len();
        let chain = |p: Vec2, hull: &mut Vec<Vec2>| {
            while hull.len() >= floor + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        };
        if pass == 0 {
            sorted.iter().for_each(|p| chain(*p, &mut hull));
        } else {
            sorted.iter().rev().for_each(|p| chain(*p, &mut hull));
        }
        hull.pop();
    }
    if hull.len() < 2 {
        return sorted[..1].to_vec();
    }
    hull
}

fn area(hull: &[Vec2]) -> f32 {
    let n = hull.len();
    (0..n)
        .map(|i| hull[i].perp_dot(hull[(i + 1) % n]))
        .sum::<f32>()
        .abs()
        * 0.5
}

/// How far a point is from a convex outline (a point, a segment or an anticlockwise polygon),
/// less than nothing inside it, and which way is out.
fn outline_distance(hull: &[Vec2], at: Vec2) -> (f32, Vec2) {
    let toward = |nearest: Vec2| {
        let d = at - nearest;
        (d.length(), d.normalize_or(Vec2::X))
    };
    match hull.len() {
        0 => (f32::INFINITY, Vec2::X),
        1 => toward(hull[0]),
        2 => toward(nearest_on_segment(hull[0], hull[1], at)),
        n => {
            // Inside when on the inner side of every edge: then out through the nearest.
            let mut inside = true;
            let mut least = (f32::INFINITY, Vec2::X);
            let mut deepest = (f32::NEG_INFINITY, Vec2::X);
            for i in 0..n {
                let (a, b) = (hull[i], hull[(i + 1) % n]);
                let edge = b - a;
                let normal = Vec2::new(edge.y, -edge.x).normalize_or(Vec2::X);
                let out = (at - a).dot(normal);
                if out > 0.0 {
                    inside = false;
                }
                if out > deepest.0 {
                    deepest = (out, normal);
                }
                let near = toward(nearest_on_segment(a, b, at));
                if near.0 < least.0 {
                    least = near;
                }
            }
            if inside { deepest } else { least }
        }
    }
}

fn nearest_on_segment(a: Vec2, b: Vec2, at: Vec2) -> Vec2 {
    let ab = b - a;
    let t = ((at - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    a + ab * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_place(scale: f32, lift: f32) -> Placement {
        Placement {
            ground: Vec3::Y * 1000.0,
            x: Vec3::X,
            z: Vec3::Z,
            up: Vec3::Y,
            scale,
            lift,
        }
    }

    /// A box's top and sides, as a model's vertices: `w` wide, `d` deep and `h` high.
    fn block(w: f32, d: f32, h: f32) -> Vec<Vec3> {
        let mut points = Vec::new();
        for i in 0..=20 {
            for j in 0..=20 {
                let (x, z) = ((i as f32 / 20.0 - 0.5) * w, (j as f32 / 20.0 - 0.5) * d);
                points.push(Vec3::new(x, h, z));
                points.push(Vec3::new(x, 0.0, z));
            }
        }
        points
    }

    #[test]
    fn a_hull_holds_every_point_and_runs_anticlockwise() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 2.0),
            Vec2::new(0.0, 2.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
        ];
        let hull = convex_hull(&points);
        assert_eq!(hull.len(), 4);
        assert!(area(&hull) > 3.99);
        for p in points {
            assert!(outline_distance(&hull, p).0 <= 1e-5);
        }
        assert!((outline_distance(&hull, Vec2::new(3.0, 1.0)).0 - 1.0).abs() < 1e-5);
        // Deep inside, out through the nearest side.
        let (inside, out) = outline_distance(&hull, Vec2::new(1.8, 1.0));
        assert!((inside + 0.2).abs() < 1e-5);
        assert!(out.abs_diff_eq(Vec2::X, 1e-5));
    }

    fn fit(points: &[Vec3], scale: f32, lift: f32) -> Option<Solid> {
        Solid::fit(
            &Relief::of(points, &[]),
            flat_place(scale, lift),
            |_| 0.0,
            vec![],
        )
    }

    #[test]
    fn a_tall_block_stops_a_walker_at_its_side_and_a_low_one_holds_them_up() {
        let tall = fit(&block(2.0, 1.0, 1.2), 1.0, 0.0).unwrap();
        let feet = Vec3::new(1.1, 1000.0, 0.0);
        let pushed = tall.push_out(feet, 0.3, 0.4);
        assert!(pushed.x > 1.25 && pushed.x < 1.45, "stopped at {pushed}");
        assert!(
            tall.surface(Vec3::new(1.6, 1000.0, 0.0), 0.15, 0.4)
                .is_none()
        );

        let low = fit(&block(2.0, 1.0, 0.3), 1.0, 0.0).unwrap();
        assert!(low.top().unwrap() < 0.31);
        assert_eq!(low.push_out(feet, 0.3, 0.4), feet);
        let held = low.surface(Vec3::new(0.5, 1000.0, 0.0), 0.15, 0.4).unwrap();
        assert!((held - 0.3).abs() < 1e-4);
    }

    #[test]
    fn standing_on_a_tall_thing_it_holds_you_up_rather_than_pushing_you_off() {
        let tall = fit(&block(2.0, 2.0, 1.0), 1.0, 0.0).unwrap();
        let on_top = Vec3::new(0.2, 1001.0, 0.0);
        assert_eq!(tall.push_out(on_top, 0.3, 0.4), on_top);
        assert!((tall.surface(on_top, 0.15, 0.4).unwrap() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn sunk_and_scaled_it_stands_as_high_as_it_shows() {
        // Twice as big, sunk 0.6 m: 1.4 m of a 1 m block's 2 m shows.
        let solid = fit(&block(1.0, 1.0, 1.0), 2.0, -0.6).unwrap();
        assert!((solid.top().unwrap() - 1.4).abs() < 1e-4);
        // Sunk past its top, nothing of it shows.
        assert!(fit(&block(1.0, 1.0, 1.0), 1.0, -1.5).is_none());
    }

    #[test]
    fn a_log_thick_at_one_end_and_thin_at_the_other_is_stepped_onto_at_its_thin_end() {
        // Six metres long, 0.8 m high at one end and 0.2 m at the other.
        let points: Vec<Vec3> = (0..=60)
            .flat_map(|i| {
                let x = i as f32 * 0.1 - 3.0;
                let high = 0.5 - x * 0.1;
                (0..=6).map(move |j| Vec3::new(x, high, j as f32 * 0.05 - 0.15))
            })
            .collect();
        let log = fit(&points, 1.0, 0.0).unwrap();
        assert!(log.pieces.len() > 2);
        // Walking at its thin end, nothing stops the walker, and it holds them up.
        let thin = Vec3::new(2.8, 1000.0, 0.3);
        assert_eq!(log.push_out(thin, 0.3, 0.4), thin);
        assert!(
            log.surface(Vec3::new(2.8, 1000.0, 0.0), 0.15, 0.4)
                .is_some()
        );
        // At its thick end it does.
        assert!(log.blocks(Vec3::new(-2.8, 1000.0, 0.3), 0.3, 0.4));
    }

    #[test]
    fn on_a_hillside_a_low_thing_is_low_from_below_it_too() {
        // Ground rising one in four eastward, a block 0.3 m high sitting on it, sunk so it
        // shows 0.3 m all round.
        let place = flat_place(1.0, 0.0);
        let points: Vec<Vec3> = block(1.0, 1.0, 0.3)
            .into_iter()
            .map(|p| p + Vec3::Y * (p.x * 0.25))
            .collect();
        let solid = Solid::fit(&Relief::of(&points, &[]), place, |p| p.x * 0.25, vec![]).unwrap();
        assert!(solid.top().unwrap() < 0.31, "{:?}", solid.top());
        // Below it on the slope, its top is more than a step above the feet, but it is low.
        let below = Vec3::new(-0.6, 1000.0 - 0.15, 0.0);
        assert_eq!(solid.push_out(below, 0.3, 0.4), below);
        // And it holds a walker up at its own height over the slope.
        let (plan, _) = solid.plan(Vec3::new(0.3, 1000.0, 0.0));
        let held = solid
            .surface(Vec3::new(0.3, 1000.0, 0.0), 0.15, 0.4)
            .unwrap();
        assert!((solid.point(plan, held).y - (1000.0 + 0.3 + 0.075)).abs() < 0.03);
    }

    #[test]
    fn a_hollow_tube_is_solid_under_its_open_top() {
        // A wall 0.5 m high round a square 0.8 m across, as a stump's bark is without its top.
        let points: Vec<Vec3> = (0..=40)
            .flat_map(|k| {
                let t = k as f32 * 0.02 - 0.4;
                [(t, -0.4), (t, 0.4), (-0.4, t), (0.4, t)]
                    .into_iter()
                    .flat_map(|(x, z)| [0.0, 0.5].map(|y| Vec3::new(x, y, z)))
            })
            .collect();
        let stump = fit(&points, 1.0, 0.0).unwrap();
        let held = stump
            .surface(Vec3::new(0.0, 1000.5, 0.0), 0.1, 0.4)
            .unwrap();
        assert!((held - 0.5).abs() < 1e-3);
    }

    #[test]
    fn an_l_of_rock_is_cut_so_its_inner_corner_is_open() {
        let mut points = block(3.0, 1.0, 1.0);
        points.extend(
            block(1.0, 3.0, 1.0)
                .into_iter()
                .map(|p| p + Vec3::new(-1.0, 0.0, 1.0)),
        );
        let solid = Solid::fit(
            &Relief::of(&points, &[]),
            flat_place(1.0, 0.0),
            |_| 0.0,
            vec![],
        )
        .unwrap();
        assert!(solid.pieces.len() > 1);
        // The inside corner of the L, open ground, more than a body's width from either arm.
        let corner = Vec3::new(0.9, 1000.0, 1.9);
        assert_eq!(solid.push_out(corner, 0.3, 0.4), corner);
        assert!(solid.blocks(Vec3::new(-1.0, 1000.0, 2.0), 0.3, 0.4));
    }

    #[test]
    fn a_long_thing_has_its_axis_found() {
        let points: Vec<Vec2> = (0..50)
            .map(|i| Vec2::new(i as f32, i as f32 + if i % 2 == 0 { 0.3 } else { -0.3 }))
            .collect();
        let middle = points.iter().copied().sum::<Vec2>() / 50.0;
        let axis = long_axis(&points, middle);
        assert!(axis.dot(Vec2::ONE.normalize()).abs() > 0.99);
    }
}
