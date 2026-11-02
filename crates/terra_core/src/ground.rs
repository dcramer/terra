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
        march(
            |point| self.distance(point),
            origin,
            direction,
            max_distance,
        )
    }
}

/// The first place a ray meets the ground whose distance `distance` gives, if it does
/// within `max_distance` (sphere tracing, Hart 1996): what [`Ground::raycast`] does, given
/// the distance to ask, so a ground that shares work between nearby asks (a memo of its
/// heights) can march with one. Each point along the ray is asked once. `direction` must be
/// unit length.
pub fn march(
    mut distance: impl FnMut(Vec3) -> f32,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<RayHit> {
    // `distance` may overestimate on slopes, so advance by less than it says, then close in
    // on the crossing by halving.
    const CAUTION: f32 = 0.5;
    const SMALLEST_STEP: f32 = 0.05;
    let mut here = distance(origin);
    if here <= 0.0 {
        return Some(RayHit {
            point: origin,
            distance: 0.0,
        });
    }
    let mut travelled = 0.0;
    while travelled < max_distance {
        let step = (here * CAUTION).max(SMALLEST_STEP);
        let next = (travelled + step).min(max_distance);
        let there = distance(origin + direction * next);
        if there <= 0.0 {
            let (mut above, mut below) = (travelled, next);
            for _ in 0..16 {
                let middle = (above + below) * 0.5;
                if distance(origin + direction * middle) > 0.0 {
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
        (travelled, here) = (next, there);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat ground at height zero, counting how often it is asked.
    struct Floor(std::cell::Cell<usize>);

    impl Floor {
        fn distance(&self, point: Vec3) -> f32 {
            self.0.set(self.0.get() + 1);
            point.y
        }
    }

    #[test]
    fn a_march_meets_the_ground_where_the_ray_crosses_it_asking_each_point_once() {
        let floor = Floor(Default::default());
        let direction = Vec3::new(1.0, -0.1, 0.0).normalize();
        let hit = march(
            |p| floor.distance(p),
            Vec3::new(0.0, 1.7, 0.0),
            direction,
            200.0,
        )
        .expect("the ray goes down into the floor");
        // Known another way: the ray drops 0.1 m a metre across, so meets y = 0 at 17 m.
        assert!((hit.point.x - 17.0).abs() < 0.01, "{hit:?}");
        assert!(hit.point.y.abs() < 0.001, "{hit:?}");
        // The origin, a point a step (half the height above the floor, along the ray), then
        // 16 halvings: no point is asked twice.
        let drop = -direction.y;
        let (mut height, mut steps) = (1.7_f32, 0);
        while height > 0.0 {
            height -= (height * 0.5).max(0.05) * drop;
            steps += 1;
        }
        assert_eq!(floor.0.get(), 1 + steps + 16);
    }

    #[test]
    fn a_ray_that_never_comes_down_meets_nothing_and_one_that_starts_inside_meets_at_once() {
        let floor = Floor(Default::default());
        let up = march(|p| floor.distance(p), Vec3::Y, Vec3::Y, 50.0);
        assert!(up.is_none());
        let inside = march(|p| floor.distance(p), -Vec3::Y, Vec3::X, 50.0).unwrap();
        assert_eq!(inside.distance, 0.0);
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
