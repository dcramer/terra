//! The plane geometry a town is laid out with: the ground's lie, a building's turn, and
//! distances between points, segments and polygons.

use super::*;

/// Which way the ground runs level at a place, and which way is up it: the contour and the
/// slope, each a unit direction east and south. On level ground, east and south.
pub fn contour(heights: Heights, at: Vec2, across_m: f32) -> (Vec2, Vec2) {
    let slope = slope(heights, at, across_m);
    if slope.length() < 1e-3 {
        return (Vec2::X, Vec2::Y);
    }
    let uphill = slope.normalize();
    (uphill.perp(), uphill)
}

/// How the ground rises at a place, in metres a metre east and south.
pub fn slope(heights: Heights, at: Vec2, across_m: f32) -> Vec2 {
    let d = across_m;
    Vec2::new(
        heights(at + Vec2::X * d) - heights(at - Vec2::X * d),
        heights(at + Vec2::Y * d) - heights(at - Vec2::Y * d),
    ) / (2.0 * d)
}

/// The way a facing looks: turned from south (+y) toward east (+x).
pub fn looking(facing: f32) -> Vec2 {
    Vec2::new(libm::sinf(facing), libm::cosf(facing))
}

/// The facing that looks a way.
pub fn facing_of(way: Vec2) -> f32 {
    libm::atan2f(way.x, way.y)
}

/// A point in a building's or thing's own metres (x across its front, y out of its door)
/// where it stands in the town, its door or middle at `at` looking the way `facing` turns.
pub fn place(at: Vec2, facing: f32, local: Vec2) -> Vec2 {
    let (s, c) = (libm::sinf(facing), libm::cosf(facing));
    at + Vec2::new(c, -s) * local.x + Vec2::new(s, c) * local.y
}

/// A box from `low` to `high` in a thing's own metres, where it stands, in order round it.
pub(super) fn turned_box(at: Vec2, facing: f32, low: Vec2, high: Vec2) -> [Vec2; 4] {
    [
        place(at, facing, low),
        place(at, facing, Vec2::new(high.x, low.y)),
        place(at, facing, high),
        place(at, facing, Vec2::new(low.x, high.y)),
    ]
}

/// Whether a point is inside a convex polygon, given in order round it either way.
pub fn inside(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut sign = 0.0f32;
    for (index, &a) in polygon.iter().enumerate() {
        let b = polygon[(index + 1) % polygon.len()];
        let side = (b - a).perp_dot(point - a);
        if side.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = side.signum();
        } else if side.signum() != sign {
            return false;
        }
    }
    true
}

/// How far a point is from a segment.
pub fn point_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let along = if ab.length_squared() > 0.0 {
        ((point - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    point.distance(a + ab * along)
}

/// How far two segments are apart: nothing where they cross.
pub fn segments(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> f32 {
    let (ab, cd) = (b - a, d - c);
    let denominator = ab.perp_dot(cd);
    if denominator.abs() > 1e-9 {
        let s = (c - a).perp_dot(cd) / denominator;
        let t = (c - a).perp_dot(ab) / denominator;
        if (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&t) {
            return 0.0;
        }
    }
    point_segment(a, c, d)
        .min(point_segment(b, c, d))
        .min(point_segment(c, a, b))
        .min(point_segment(d, a, b))
}

/// How far a segment is from a convex polygon: nothing where it touches or is inside.
pub fn segment_polygon(a: Vec2, b: Vec2, polygon: &[Vec2]) -> f32 {
    if inside(a, polygon) || inside(b, polygon) {
        return 0.0;
    }
    (0..polygon.len())
        .map(|i| segments(a, b, polygon[i], polygon[(i + 1) % polygon.len()]))
        .fold(f32::MAX, f32::min)
}

/// How far two convex polygons are apart: nothing where they overlap.
pub fn polygons(p: &[Vec2], q: &[Vec2]) -> f32 {
    if p.iter().any(|&v| inside(v, q)) || q.iter().any(|&v| inside(v, p)) {
        return 0.0;
    }
    (0..p.len())
        .map(|i| segment_polygon(p[i], p[(i + 1) % p.len()], q))
        .fold(f32::MAX, f32::min)
}

/// How far a polygon is from a street's centre line.
pub(super) fn from_street(polygon: &[Vec2], street: &Street) -> f32 {
    street
        .points
        .windows(2)
        .map(|pair| segment_polygon(pair[0], pair[1], polygon))
        .fold(f32::MAX, f32::min)
}

/// A point a way along a line of points, and the way the line runs there.
pub(super) fn along_line(points: &[Vec2], distance: f32) -> Option<(Vec2, Vec2)> {
    let mut left = distance;
    for pair in points.windows(2) {
        let length = pair[0].distance(pair[1]);
        if left <= length && length > 0.0 {
            let way = (pair[1] - pair[0]) / length;
            return Some((pair[0] + way * left, way));
        }
        left -= length;
    }
    None
}

pub(super) fn line_length(points: &[Vec2]) -> f32 {
    points
        .windows(2)
        .map(|pair| pair[0].distance(pair[1]))
        .sum()
}
