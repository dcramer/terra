//! Casting a ray into a base's cubes: what the hammer aims at.

use crate::cubes::FULL;
use crate::{Base, CELL_M, Cube, Face, Fill, Kind, Side, storey_of};
use glam::{IVec3, Vec3};

/// Where a ray met a base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildHit {
    /// The cube met, counted in cubes.
    pub cube: IVec3,
    /// A wall tile met on a side; or the tile whose floor (`Down`, met from above) or roof
    /// (`Up`, met from below) was met.
    pub face: Face,
    /// Distance along the ray, in metres.
    pub distance: f32,
    /// Whether what was met is a wall, not a floor or roof.
    pub wall: bool,
    /// The way out of the cube's face that was met: the cube beyond it is where something
    /// set against it goes.
    pub normal: IVec3,
}

impl Base {
    /// The first cube a ray meets, from any side, if any; openings are passed through.
    /// `origin` and `direction` are in the base's own space, in metres; `direction` is
    /// unit length.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<BuildHit> {
        self.raycast_where(origin, direction, max_distance, |cube| {
            cube.fill != Fill::Open
        })
    }

    /// As [`Base::raycast`], meeting only the cubes `meets` says, such as openings too.
    pub fn raycast_where(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        meets: impl Fn(&Cube) -> bool,
    ) -> Option<BuildHit> {
        self.first_cube_along(origin, direction, max_distance, &meets)
            .map(|(cube, distance, normal)| {
                let wall = self.in_wall(cube);
                let face = if wall {
                    Face::new(
                        IVec3::new(cube.x, storey_of(cube.y), cube.z),
                        Side::of(normal),
                    )
                } else if normal.y >= 0 {
                    Face::new(
                        IVec3::new(cube.x, storey_of(cube.y + 1), cube.z),
                        if normal.y > 0 {
                            Side::Down
                        } else {
                            Side::of(normal)
                        },
                    )
                } else {
                    Face::new(
                        IVec3::new(cube.x, storey_of(cube.y + 1) - 1, cube.z),
                        Side::Up,
                    )
                };
                BuildHit {
                    cube,
                    face,
                    distance,
                    wall,
                    normal,
                }
            })
    }

    /// The first cube a ray enters that `meets` takes, how far along, and the face it
    /// enters by: the cells walked one by one along the ray, nearest first (Amanatides
    /// and Woo's grid traversal, as voxel engines cast rays), from where it enters the
    /// base's bounds, so it looks at the cubes on its way and no others.
    fn first_cube_along(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        meets: &dyn Fn(&Cube) -> bool,
    ) -> Option<(IVec3, f32, IVec3)> {
        let (low, high) = self.cubes.bounds()?;
        let (box_low, box_high) = (low.as_vec3() * CELL_M, (high + 1).as_vec3() * CELL_M);
        // Where the ray is inside the bounds, along it.
        let (mut enter, mut leave) = (0.0_f32, max_distance);
        for axis in 0..3 {
            if direction[axis].abs() < 1e-9 {
                if origin[axis] < box_low[axis] || origin[axis] > box_high[axis] {
                    return None;
                }
                continue;
            }
            let (a, b) = (
                (box_low[axis] - origin[axis]) / direction[axis],
                (box_high[axis] - origin[axis]) / direction[axis],
            );
            enter = enter.max(a.min(b));
            leave = leave.min(a.max(b));
        }
        if enter > leave {
            return None;
        }
        let start = origin + direction * enter;
        let mut cell = (start / CELL_M).floor().as_ivec3().clamp(low, high);
        let step = IVec3::new(
            direction.x.signum() as i32,
            direction.y.signum() as i32,
            direction.z.signum() as i32,
        );
        // How far along the ray each next boundary is, and how far apart they are.
        let mut next = Vec3::splat(f32::MAX);
        let mut apart = Vec3::splat(f32::MAX);
        for axis in 0..3 {
            if direction[axis].abs() < 1e-9 {
                continue;
            }
            let edge = (cell[axis] + i32::from(step[axis] > 0)) as f32 * CELL_M;
            next[axis] = (edge - origin[axis]) / direction[axis];
            apart[axis] = CELL_M / direction[axis].abs();
        }
        loop {
            if let Some(cube) = self.cubes.get(cell)
                && meets(&cube)
            {
                let corner = cell.as_vec3() * CELL_M;
                if let Some((distance, normal)) =
                    ray_box(origin, direction, corner, corner + Vec3::splat(CELL_M))
                    && distance <= max_distance
                {
                    return Some((cell, distance, normal));
                }
            }
            let axis = if next.x <= next.y && next.x <= next.z {
                0
            } else if next.y <= next.z {
                1
            } else {
                2
            };
            if next[axis] > leave {
                return None;
            }
            cell[axis] += step[axis];
            next[axis] += apart[axis];
            if cell[axis] < low[axis] || cell[axis] > high[axis] {
                return None;
            }
        }
    }
}

/// Where a ray enters a box, and the side of the box it enters by, if it does.
fn ray_box(origin: Vec3, direction: Vec3, low: Vec3, high: Vec3) -> Option<(f32, IVec3)> {
    let mut near = f32::MIN;
    let mut far = f32::MAX;
    let mut normal = IVec3::ZERO;
    for axis in 0..3 {
        if direction[axis].abs() < 1e-9 {
            if origin[axis] < low[axis] || origin[axis] > high[axis] {
                return None;
            }
            continue;
        }
        let (a, b) = (
            (low[axis] - origin[axis]) / direction[axis],
            (high[axis] - origin[axis]) / direction[axis],
        );
        let (enter, leave) = (a.min(b), a.max(b));
        if enter > near {
            near = enter;
            normal = IVec3::ZERO;
            normal[axis] = if direction[axis] > 0.0 { -1 } else { 1 };
        }
        far = far.min(leave);
    }
    (near <= far && near > 0.0).then_some((near, normal))
}

impl Base {
    /// A box holding every cube, in cubes, to the sections they are in, if there are any.
    pub fn bounds(&self) -> Option<(IVec3, IVec3)> {
        self.cubes.bounds()
    }

    /// The height of the top of whatever is built under a point, in metres in the base's own
    /// space: the highest cube with something in it at or below it in its column, by its
    /// shape and how far in it is, within a storey and a half; none if nothing is.
    pub fn top_under(&self, at: Vec3) -> Option<f32> {
        self.top_under_of(at).map(|(top, _)| top)
    }

    /// As [`Base::top_under`], with the kind of the cube it is the top of: a roof's is drawn
    /// higher, by its covering (`Roofing::thick`).
    pub fn top_under_of(&self, at: Vec3) -> Option<(f32, Kind)> {
        let cell = (at / CELL_M).floor().as_ivec3();
        let (u, w) = (at.x / CELL_M - cell.x as f32, at.z / CELL_M - cell.z as f32);
        for y in (cell.y - 9..=cell.y).rev() {
            let place = IVec3::new(cell.x, y, cell.z);
            let Some(cube) = self.cube(place).filter(Cube::is_in) else {
                continue;
            };
            let fill = f32::from(cube.level()) / f32::from(FULL);
            let top = cube.shape.top(u, w).min(fill);
            return Some(((y as f32 + top) * CELL_M, cube.kind));
        }
        None
    }

    /// Whether a point is inside a cube with something in it, under its top.
    pub fn solid_at(&self, at: Vec3) -> bool {
        let cell = (at / CELL_M).floor().as_ivec3();
        let Some(cube) = self.cube(cell).filter(Cube::is_in) else {
            return false;
        };
        let (u, v, w) = (
            at.x / CELL_M - cell.x as f32,
            at.y / CELL_M - cell.y as f32,
            at.z / CELL_M - cell.z as f32,
        );
        v <= cube.shape.top(u, w) && v >= cube.shape.bottom(u, w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Kind;

    #[test]
    fn a_ray_walked_cell_by_cell_meets_the_cube_testing_every_cube_does() {
        // Every cube tested against the ray, the nearest it enters taken: the plain way.
        fn plain(
            base: &Base,
            origin: Vec3,
            direction: Vec3,
            max: f32,
        ) -> Option<(IVec3, f32, IVec3)> {
            base.cubes()
                .filter(|(_, cube)| cube.fill != Fill::Open)
                .filter_map(|(at, _)| {
                    let low = at.as_vec3() * CELL_M;
                    let (distance, normal) =
                        ray_box(origin, direction, low, low + Vec3::splat(CELL_M))?;
                    (distance <= max).then_some((at, distance, normal))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
        }
        let kinds = |_| Kind::FIRST_ROOF;
        let mut random = terra_gen::Rng::new(terra_gen::Seed(11));
        let (mut hits, mut misses) = (0, 0);
        for (_, ops) in crate::bench_village(Kind(1), Kind(2), Kind(3), &kinds)
            .into_iter()
            .take(12)
        {
            let mut base = Base::default();
            base.apply_shortcuts(&ops);
            let (low, high) = base.store().bounds().unwrap();
            let (low, high) = (low.as_vec3() * CELL_M, (high + 1).as_vec3() * CELL_M);
            for _ in 0..300 {
                // From anywhere round it, inside it too, any way, some straight along an axis.
                let origin = Vec3::new(
                    low.x - 4.0 + random.unit() * (high.x - low.x + 8.0),
                    low.y - 2.0 + random.unit() * (high.y - low.y + 4.0),
                    low.z - 4.0 + random.unit() * (high.z - low.z + 8.0),
                );
                let mut direction = Vec3::new(
                    random.range(-1.0, 1.0),
                    random.range(-1.0, 1.0),
                    random.range(-1.0, 1.0),
                );
                if random.unit() < 0.1 {
                    direction = Vec3::new(direction.x, 0.0, 0.0);
                }
                let direction = direction.normalize_or(Vec3::X);
                let max = 2.0 + random.unit() * 30.0;
                let walked = base
                    .raycast(origin, direction, max)
                    .map(|hit| (hit.cube, hit.distance, hit.normal));
                let tested = plain(&base, origin, direction, max);
                match (walked, tested) {
                    (Some(a), Some(b)) => {
                        // The same cube, or one entered at the same distance, as at an edge.
                        assert!(
                            a == b || (a.1 - b.1).abs() < 1e-5,
                            "{origin} {direction}: {a:?} {b:?}"
                        );
                        hits += 1;
                    }
                    (None, None) => misses += 1,
                    (a, b) => panic!("{origin} {direction} {max}: walked {a:?}, tested {b:?}"),
                }
            }
        }
        assert!(hits > 300 && misses > 300, "{hits} hits, {misses} misses");
    }
}
