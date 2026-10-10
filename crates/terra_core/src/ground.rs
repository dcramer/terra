use crate::MaterialId;
use glam::Vec3;

/// Where the ground is and what it is made of.
///
/// This is all that most of the engine needs to know about terrain. The voxel terrain
/// implements it from voxel data; the stand-in implements it from a formula.
pub trait Ground: Send + Sync {
    /// How far a point is from the ground's surface, in metres: positive above it,
    /// negative inside it. An estimate that is accurate near the surface and may be
    /// loose far from it or on steep slopes.
    fn distance(&self, point: Vec3) -> f32;

    /// What the ground is made of at a point.
    fn material(&self, point: Vec3) -> MaterialId;

    /// The direction the surface faces near a point: away from the ground.
    fn normal(&self, point: Vec3) -> Vec3 {
        const STEP: f32 = 0.25;
        let slope =
            |axis: Vec3| self.distance(point + axis * STEP) - self.distance(point - axis * STEP);
        Vec3::new(slope(Vec3::X), slope(Vec3::Y), slope(Vec3::Z)).normalize_or(Vec3::Y)
    }

    /// The first place a ray meets the ground, if it does within `max_distance`.
    /// `direction` must be unit length.
    fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<RayHit> {
        // `distance` may overestimate on slopes, so advance by less than it says, then
        // close in on the crossing by halving.
        const CAUTION: f32 = 0.5;
        const SMALLEST_STEP: f32 = 0.05;
        if self.distance(origin) <= 0.0 {
            return Some(RayHit {
                point: origin,
                distance: 0.0,
            });
        }
        let mut travelled = 0.0;
        while travelled < max_distance {
            let step = (self.distance(origin + direction * travelled) * CAUTION).max(SMALLEST_STEP);
            let next = (travelled + step).min(max_distance);
            if self.distance(origin + direction * next) <= 0.0 {
                let (mut above, mut below) = (travelled, next);
                for _ in 0..16 {
                    let middle = (above + below) * 0.5;
                    if self.distance(origin + direction * middle) > 0.0 {
                        above = middle;
                    } else {
                        below = middle;
                    }
                }
                return Some(RayHit {
                    point: origin + direction * below,
                    distance: below,
                });
            }
            if next >= max_distance {
                break;
            }
            travelled = next;
        }
        None
    }
}

/// Where a ray met the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub point: Vec3,
    /// Distance travelled along the ray.
    pub distance: f32,
}

/// Where the water is.
///
/// Asked by position, not read as one number (D45): with a single sea the answer is the
/// same everywhere, and if pools arrive (D44) it varies by place. Anywhere the ground is
/// below the local level there is water.
pub trait Water: Send + Sync {
    /// Distance from the planet's centre to the water surface in a direction, or `None`
    /// where there is no water.
    fn level(&self, direction: Vec3) -> Option<f32>;
}
