//! A spatial hash: things filed by the cube of space they are in, so what is near a place
//! is found by looking in the few cubes round it rather than at everything (D127). Used
//! for what bodies bump into, which there are many of and which are asked about every
//! frame.

use bevy::{math::IVec3, platform::collections::HashMap, prelude::Vec3};

/// Things at points in space, each with a value, filed by cube.
#[derive(Clone, Debug)]
pub struct SpatialHash<T> {
    cube_m: f32,
    cubes: HashMap<IVec3, Vec<(Vec3, T)>>,
    len: usize,
}

impl<T> SpatialHash<T> {
    /// Cubes `cube_m` on a side. About the distance usually asked about works best.
    pub fn new(cube_m: f32) -> Self {
        Self {
            cube_m,
            cubes: HashMap::default(),
            len: 0,
        }
    }

    fn cube(&self, at: Vec3) -> IVec3 {
        (at / self.cube_m).floor().as_ivec3()
    }

    pub fn insert(&mut self, at: Vec3, value: T) {
        let cube = self.cube(at);
        self.cubes.entry(cube).or_default().push((at, value));
        self.len += 1;
    }

    /// Removes whatever is at exactly this point and `goes` picks. Returns how many went.
    pub fn remove(&mut self, at: Vec3, mut goes: impl FnMut(&T) -> bool) -> usize {
        let cube = self.cube(at);
        let Some(things) = self.cubes.get_mut(&cube) else {
            return 0;
        };
        let before = things.len();
        things.retain(|(point, value)| !(*point == at && goes(value)));
        let gone = before - things.len();
        if things.is_empty() {
            self.cubes.remove(&cube);
        }
        self.len -= gone;
        gone
    }

    pub fn clear(&mut self) {
        self.cubes.clear();
        self.len = 0;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Everything within `reach` of a point, and perhaps a little further: whatever is in
    /// the cubes that reach touches. The caller measures for itself.
    pub fn near(&self, at: Vec3, reach: f32) -> impl Iterator<Item = &(Vec3, T)> {
        let low = self.cube(at - Vec3::splat(reach));
        let high = self.cube(at + Vec3::splat(reach));
        (low.x..=high.x)
            .flat_map(move |x| {
                (low.y..=high.y)
                    .flat_map(move |y| (low.z..=high.z).map(move |z| IVec3::new(x, y, z)))
            })
            .filter_map(|cube| self.cubes.get(&cube))
            .flatten()
    }

    pub fn iter(&self) -> impl Iterator<Item = &(Vec3, T)> {
        self.cubes.values().flatten()
    }
}

/// Cubes four metres on a side: about as far as a body reaches into its neighbours.
impl<T> Default for SpatialHash<T> {
    fn default() -> Self {
        Self::new(4.0)
    }
}

impl<T> Extend<(Vec3, T)> for SpatialHash<T> {
    fn extend<I: IntoIterator<Item = (Vec3, T)>>(&mut self, things: I) {
        for (at, value) in things {
            self.insert(at, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_near_is_found_and_what_is_far_is_not() {
        let mut hash = SpatialHash::new(4.0);
        hash.insert(Vec3::new(1.0, 0.0, 0.0), "near");
        hash.insert(Vec3::new(-3.9, 0.5, 0.0), "across a cube's edge");
        hash.insert(Vec3::new(40.0, 0.0, 0.0), "far");
        let found: Vec<_> = hash.near(Vec3::ZERO, 4.0).map(|(_, name)| *name).collect();
        assert!(found.contains(&"near"));
        assert!(found.contains(&"across a cube's edge"));
        assert!(!found.contains(&"far"));
        assert_eq!(hash.len(), 3);
    }

    #[test]
    fn removing_takes_only_what_is_at_the_point() {
        let mut hash = SpatialHash::new(4.0);
        let at = Vec3::new(2.0, 2.0, 2.0);
        hash.insert(at, 1);
        hash.insert(at, 2);
        hash.insert(at + Vec3::X * 0.1, 3);
        assert_eq!(hash.remove(at, |&value| value == 1), 1);
        assert_eq!(hash.remove(at, |_| true), 1);
        assert_eq!(hash.len(), 1);
        assert_eq!(
            hash.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            [3]
        );
    }

    #[test]
    fn it_works_on_a_planet_far_from_the_origin() {
        let mut hash = SpatialHash::new(4.0);
        let surface = Vec3::new(0.0, 2000.0, 0.0);
        for i in 0..100 {
            hash.insert(surface + Vec3::X * i as f32, i);
        }
        assert_eq!(
            hash.near(surface, 2.0)
                .filter(|(at, _)| at.distance(surface) <= 2.0)
                .count(),
            3
        );
    }
}
