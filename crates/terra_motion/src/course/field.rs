//! The course's ground: analytic signed distance fields whose answers are known by hand
//! (steps, slopes, stairs, a tunnel, walls with a gap, a pool, a small round world), so a
//! criterion's number is measured against geometry, not against the mover's own idea of it.
//!
//! Every fixture but the round world lies near the north pole of a world [`RADIUS_M`] across,
//! in its own level frame: `u` forward, `h` up (along the radius) and `w` to the side, so down
//! is toward the centre as it is in the game (D63) and a flat floor is the sphere itself.

use crate::mover::Obstacle;
use glam::{DVec3, Vec2, Vec3};

/// The radius of the world the level fixtures lie on: large enough that 20 m of course is
/// flat to a few millimetres' curvature correction, small enough for f32 to keep a tenth of a
/// millimetre at its surface.
pub const RADIUS_M: f32 = 2000.0;

/// Gravity on the course, toward the centre (D63 leaves the world's value open; this is
/// today's, `terra_walker::movement`).
pub const GRAVITY: f32 = 16.0;

/// The ground a body is moved over, as the mover's query trait asks it (D53): how far a
/// point is from the solid, and the water over a point.
pub trait Field: Sync {
    /// How far a point is outside the solid, in metres, negative inside. Exact outside the
    /// solid for every fixture here, a bound inside.
    fn distance(&self, at: Vec3) -> f32;

    /// The same of a point in f64, as a mover that keeps its places in f64 asks it: a level
    /// fixture works its frame out in f64, so it answers to a micrometre at any radius.
    fn distance_at(&self, at: DVec3) -> f32 {
        self.distance(at.as_vec3())
    }

    /// How far from the centre the water's surface over a point is, where water is.
    fn water(&self, _at: Vec3) -> Option<f32> {
        None
    }

    /// What stops a body besides the ground: cubes, doors, props, trees and rocks, as the
    /// mover's query asks them (D53).
    fn obstacles(&self) -> &[Obstacle] {
        &[]
    }

    /// Gravity's strength toward the centre, m/s².
    fn gravity(&self) -> f32 {
        GRAVITY
    }

    /// Which way the surface faces near a point, by central differences of a centimetre.
    fn normal(&self, at: Vec3) -> Vec3 {
        const E: f32 = 0.01;
        let d = |axis: Vec3| self.distance(at + axis * E) - self.distance(at - axis * E);
        Vec3::new(d(Vec3::X), d(Vec3::Y), d(Vec3::Z)).normalize_or(at.normalize_or(Vec3::Y))
    }
}

/// A level frame on a world of a radius, worked out in f64: what [`world`] and [`local`] are
/// at [`RADIUS_M`] in f32, at any radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub radius: f64,
}

impl Frame {
    /// The world point of a place in the frame (`u` forward, `h` up, `w` to the side).
    pub fn world(&self, u: f64, h: f64, w: f64) -> DVec3 {
        DVec3::new(u, self.radius, w).normalize() * (self.radius + h)
    }

    /// A world point in the frame: (`u`, `h`, `w`).
    pub fn local(&self, at: DVec3) -> DVec3 {
        let d = at.normalize();
        DVec3::new(
            self.radius * d.x / d.y,
            at.length() - self.radius,
            self.radius * d.z / d.y,
        )
    }
}

/// The world point of a place in the level frame (`u` forward, `h` up, `w` to the side).
pub fn world(u: f32, h: f32, w: f32) -> Vec3 {
    Vec3::new(u, RADIUS_M, w).normalize() * (RADIUS_M + h)
}

/// A world point in the level frame: (`u`, `h`, `w`).
pub fn local(at: Vec3) -> Vec3 {
    let d = at.normalize();
    Vec3::new(
        RADIUS_M * d.x / d.y,
        at.length() - RADIUS_M,
        RADIUS_M * d.z / d.y,
    )
}

/// The level direction of the frame's `u` at a world point.
pub fn forward_at(at: Vec3) -> Vec3 {
    Vec3::X.reject_from(at.normalize()).normalize()
}

/// The level direction of the frame's `w` at a world point.
pub fn side_at(at: Vec3) -> Vec3 {
    Vec3::Z.reject_from(at.normalize()).normalize()
}

/// The exact distance from a point to an axis-aligned box, negative inside.
fn boxed(at: Vec3, min: Vec3, max: Vec3) -> f32 {
    let centre = (min + max) * 0.5;
    let q = (at - centre).abs() - (max - min) * 0.5;
    q.max(Vec3::ZERO).length() + q.max_element().min(0.0)
}

/// A piece of solid in the level frame.
#[derive(Clone, Debug)]
pub enum Solid {
    /// Ground under a height profile along `u` (a polyline of (`u`, `h`) corners, vertical
    /// runs allowed for steps), level across `w`, held beyond its ends at their heights.
    Profile(Vec<Vec2>),
    /// A box from one corner to the other.
    Block(Vec3, Vec3),
    /// Rock everywhere: a fixture dug out of it.
    Rock,
    /// A ball of ground: its middle and radius (a lump filled into the ground).
    Ball(Vec3, f32),
    /// Solid inside a polygon of (`u`, `h`) corners, level across `w`: a face leaning any way,
    /// an overhang's included, which a profile cannot hold.
    Polygon(Vec<Vec2>),
}

impl Solid {
    fn distance(&self, at: Vec3) -> f32 {
        match self {
            Solid::Profile(corners) => profile(corners, Vec2::new(at.x, at.y)),
            Solid::Block(min, max) => boxed(at, *min, *max),
            Solid::Rock => -1.0e3,
            Solid::Ball(centre, radius) => (at - *centre).length() - radius,
            Solid::Polygon(corners) => polygon(corners, Vec2::new(at.x, at.y)),
        }
    }
}

/// The exact distance from a point to a polygon, negative inside: the nearest edge, and the
/// sign by the crossings of a ray along `u`. Ported from Inigo Quilez's `sdPolygon`
/// (iquilezles.org/articles/distfunctions2d, MIT).
fn polygon(corners: &[Vec2], at: Vec2) -> f32 {
    let mut nearest = (at - corners[0]).length_squared();
    let mut sign = 1.0;
    let mut j = corners.len() - 1;
    for (i, &v) in corners.iter().enumerate() {
        let e = corners[j] - v;
        let w = at - v;
        let b = w - e * (w.dot(e) / e.length_squared()).clamp(0.0, 1.0);
        nearest = nearest.min(b.length_squared());
        let c = [at.y >= v.y, at.y < corners[j].y, e.x * w.y > e.y * w.x];
        if c.iter().all(|&x| x) || c.iter().all(|&x| !x) {
            sign = -sign;
        }
        j = i;
    }
    sign * nearest.sqrt()
}

/// The exact distance from a point to the ground under a profile, negative under it.
fn profile(corners: &[Vec2], at: Vec2) -> f32 {
    let (first, last) = (corners[0], corners[corners.len() - 1]);
    let far = Vec2::new(1.0e4, 0.0);
    let ends = [
        (Vec2::new(first.x - far.x, first.y), first),
        (last, Vec2::new(last.x + far.x, last.y)),
    ];
    let segments = corners
        .windows(2)
        .map(|pair| (pair[0], pair[1]))
        .chain(ends);
    let mut nearest = f32::INFINITY;
    let mut height = None;
    for (a, b) in segments {
        let along = b - a;
        let t = ((at - a).dot(along) / along.length_squared()).clamp(0.0, 1.0);
        nearest = nearest.min((at - (a + along * t)).length());
        if a.x < b.x && (a.x..b.x).contains(&at.x) {
            height = Some(a.y + (b.y - a.y) * (at.x - a.x) / (b.x - a.x));
        }
    }
    if at.y < height.unwrap_or(first.y) {
        -nearest
    } else {
        nearest
    }
}

/// Water standing over a box of the level frame, its surface at a height.
#[derive(Clone, Debug)]
pub struct Water {
    pub min: Vec2,
    pub max: Vec2,
    pub surface: f32,
}

/// A level fixture: the union of its solids, less what is dug out of them, and its water, on a
/// world of a radius ([`RADIUS_M`] unless set).
#[derive(Clone, Debug)]
pub struct Course {
    pub solids: Vec<Solid>,
    pub dug: Vec<(Vec3, Vec3)>,
    pub water: Option<Water>,
    /// What stands on it, answered apart from the ground.
    pub obstacles: Vec<Obstacle>,
    pub radius: f32,
}

impl Default for Course {
    fn default() -> Self {
        Course {
            solids: Vec::new(),
            dug: Vec::new(),
            water: None,
            obstacles: Vec::new(),
            radius: RADIUS_M,
        }
    }
}

impl Course {
    /// Level ground and nothing else.
    pub fn flat() -> Self {
        Course::of(Solid::Profile(vec![Vec2::ZERO, Vec2::X]))
    }

    pub fn of(solid: Solid) -> Self {
        Course {
            solids: vec![solid],
            ..Course::default()
        }
    }

    /// The same fixture on a world of another radius.
    pub fn on(self, radius: f32) -> Self {
        Course { radius, ..self }
    }

    /// Its level frame.
    pub fn frame(&self) -> Frame {
        Frame {
            radius: self.radius as f64,
        }
    }

    pub fn with(mut self, solid: Solid) -> Self {
        self.solids.push(solid);
        self
    }

    /// The distance in the level frame.
    pub fn local_distance(&self, at: Vec3) -> f32 {
        let solid = self
            .solids
            .iter()
            .map(|solid| solid.distance(at))
            .fold(f32::INFINITY, f32::min);
        self.dug
            .iter()
            .map(|(min, max)| -boxed(at, *min, *max))
            .fold(solid, f32::max)
    }
}

impl Field for Course {
    fn distance(&self, at: Vec3) -> f32 {
        self.distance_at(at.as_dvec3())
    }

    fn distance_at(&self, at: DVec3) -> f32 {
        self.local_distance(self.frame().local(at).as_vec3())
    }

    fn water(&self, at: Vec3) -> Option<f32> {
        let water = self.water.as_ref()?;
        let p = self.frame().local(at.as_dvec3()).as_vec3();
        let over =
            (water.min.x..water.max.x).contains(&p.x) && (water.min.y..water.max.y).contains(&p.z);
        over.then_some(self.radius + water.surface)
    }

    fn obstacles(&self) -> &[Obstacle] {
        &self.obstacles
    }
}

/// A small round world, for gravity toward its centre: a ball with a cave in it, a shell
/// [`Ball::CAVE`] metres up from its floor at [`Ball::CAVE_FLOOR`] from the centre, round the
/// equator on the +x side.
#[derive(Clone, Debug)]
pub struct Ball {
    pub radius: f32,
}

impl Ball {
    pub const CAVE_FLOOR: f32 = 30.0;
    pub const CAVE: f32 = 2.5;
    /// How far up and down the cave reaches from the equator.
    const CAVE_REACH: f32 = 6.0;

    /// The cave's air, as a distance: positive inside it.
    fn cave(&self, at: Vec3) -> f32 {
        let r = at.length();
        let shell = Self::CAVE * 0.5 - (r - Self::CAVE_FLOOR - Self::CAVE * 0.5).abs();
        shell.min(Self::CAVE_REACH - at.y.abs()).min(at.x)
    }
}

impl Field for Ball {
    fn distance(&self, at: Vec3) -> f32 {
        (at.length() - self.radius).max(self.cave(at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_profile_is_its_distance_by_hand() {
        // A 0.5 m step at u = 2.
        let step = vec![
            Vec2::ZERO,
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 0.5),
            Vec2::new(4.0, 0.5),
        ];
        let at = |u: f32, h: f32| profile(&step, Vec2::new(u, h));
        assert!((at(0.0, 1.0) - 1.0).abs() < 1e-6, "over the floor");
        assert!(
            (at(1.0, 0.3) - 0.3).abs() < 1e-6,
            "nearer the floor than the riser"
        );
        assert!((at(1.8, 0.3) - 0.2).abs() < 1e-6, "nearer the riser");
        // Past the step's edge, diagonally off its corner.
        assert!((at(1.7, 0.9) - 0.5).abs() < 1e-6);
        assert!((at(3.0, 0.4) + 0.1).abs() < 1e-6, "inside the step");
        assert!(
            (at(-50.0, 0.25) - 0.25).abs() < 1e-6,
            "held beyond its first corner"
        );
    }

    #[test]
    fn a_slope_profile_measures_square_to_its_face() {
        // 30 degrees up from u = 1.
        let thirty = std::f32::consts::FRAC_PI_6;
        let (s, c) = (libm::sinf(thirty), libm::cosf(thirty));
        let slope = vec![
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0 + 10.0 * c, 10.0 * s),
        ];
        let on = Vec2::new(1.0 + 4.0 * c, 4.0 * s);
        let normal = Vec2::new(-s, c);
        assert!((profile(&slope, on + normal * 0.3) - 0.3).abs() < 1e-5);
        assert!((profile(&slope, on - normal * 0.2) + 0.2).abs() < 1e-5);
    }

    #[test]
    fn the_level_frame_is_round_the_world_and_back() {
        let at = world(3.0, 0.5, -2.0);
        assert!((at.length() - RADIUS_M - 0.5).abs() < 1e-3);
        assert!((local(at) - Vec3::new(3.0, 0.5, -2.0)).length() < 1e-3);
        // A flat course's floor is the world's sphere.
        assert!((Course::flat().distance(world(7.0, 0.25, 4.0)) - 0.25).abs() < 1e-3);
    }

    #[test]
    fn a_tunnel_is_air_two_metres_high_in_rock() {
        let mut tunnel = Course::of(Solid::Rock);
        tunnel
            .dug
            .push((Vec3::new(-5.0, 0.0, -1.0), Vec3::new(20.0, 2.0, 1.0)));
        assert!((tunnel.local_distance(Vec3::new(0.0, 0.5, 0.0)) - 0.5).abs() < 1e-6);
        assert!((tunnel.local_distance(Vec3::new(0.0, 1.8, 0.0)) - 0.2).abs() < 1e-6);
        assert!(
            tunnel.local_distance(Vec3::new(0.0, 2.3, 0.0)) < 0.0,
            "in the roof"
        );
    }

    #[test]
    fn the_round_world_has_its_cave_on_the_equator_alone() {
        let ball = Ball { radius: 40.0 };
        assert!(
            (ball.distance(Vec3::Y * 42.0) - 2.0).abs() < 1e-5,
            "over the pole"
        );
        let in_cave = Vec3::X * (Ball::CAVE_FLOOR + 1.0);
        assert!(
            (ball.distance(in_cave) - 1.0).abs() < 1e-5,
            "a metre over the cave's floor"
        );
        assert!(ball.distance(-in_cave) < 0.0, "rock on the far side");
    }
}
