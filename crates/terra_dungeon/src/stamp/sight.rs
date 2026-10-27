//! What an eye in a stamp sees of the outside: the dungeon's one portal to the surface is the
//! top of its stair down, and an eye under the ground sees out only along a line through the
//! cubes' air to that opening. The game hides the surface while no such line is in view, as
//! engines cull an exterior from an interior through its portals (Godot's rooms and portals,
//! Source's areaportals). Lines walk the cubes one at a time (Amanatides and Woo, "A Fast Voxel
//! Traversal Algorithm for Ray Tracing", 1987). Doors and props are left out, so a closed door
//! never hides the surface: the answer errs toward drawing it.

use super::{CUBE, Grid, Solid, Stamp, cube_of, cubes};
use glam::{IVec3, Vec3};

/// A stamp's solid cubes and the air at the top of its stair, in the stamp's frame.
pub struct Sight {
    grid: Grid,
    /// The digs' boxes, as their least and most corners: an eye in none is outside.
    digs: Vec<(Vec3, Vec3)>,
    /// The middles of the air cubes in the stair's dig from the ground up: the opening.
    opening: Vec<Vec3>,
}

impl Sight {
    pub fn new(stamp: &Stamp) -> Sight {
        let map = cubes(&stamp.ops);
        let grid = Grid::new(&map);
        let digs: Vec<(Vec3, Vec3)> = stamp
            .digs
            .iter()
            .map(|d| (d.centre - d.half, d.centre + d.half))
            .collect();
        // The stair's dig: the one round the stair's top, the highest if more are.
        let top = stamp.start;
        let stair = digs
            .iter()
            .filter(|(lo, hi)| (lo.x..=hi.x).contains(&top.x) && (lo.z..=hi.z).contains(&top.z))
            .max_by(|a, b| a.1.y.total_cmp(&b.1.y));
        let mut opening = Vec::new();
        if let Some(&(lo, hi)) = stair {
            let (a, b) = (cube_of(lo), cube_of(hi - Vec3::splat(CUBE * 0.5)));
            // From the cube just under the ground's level, where the stair's top step meets it.
            for y in (cube_of(Vec3::new(0.0, top.y, 0.0)).y - 1).max(a.y)..=b.y {
                for z in a.z..=b.z {
                    for x in a.x..=b.x {
                        let at = IVec3::new(x, y, z);
                        if !grid.solid(at) {
                            opening.push((at.as_vec3() + Vec3::splat(0.5)) * CUBE);
                        }
                    }
                }
            }
        }
        Sight {
            grid,
            digs,
            opening,
        }
    }

    /// The opening's points, the middles of its air cubes.
    pub fn opening(&self) -> &[Vec3] {
        &self.opening
    }

    /// Whether `eye` is in the dungeon's dug ground.
    pub fn inside(&self, eye: Vec3) -> bool {
        self.digs
            .iter()
            .any(|(lo, hi)| eye.cmpge(*lo).all() && eye.cmple(*hi).all())
    }

    /// Whether an eye in the dungeon may see the outside: some point of the opening that
    /// `in_view` takes is reached by a line through no solid cube. Outside the dungeon, always.
    pub fn sees_out(&self, eye: Vec3, in_view: impl Fn(Vec3) -> bool) -> bool {
        !self.inside(eye)
            || self
                .opening
                .iter()
                .any(|&p| in_view(p) && self.clear(eye, p))
    }

    /// Whether the line from `from` to `to` passes through no solid cube, the cubes at either
    /// end included.
    pub fn clear(&self, from: Vec3, to: Vec3) -> bool {
        let (a, b) = (from / CUBE, to / CUBE);
        let mut at = a.floor().as_ivec3();
        let end = b.floor().as_ivec3();
        let d = b - a;
        let step = IVec3::new(sign(d.x), sign(d.y), sign(d.z));
        // How far along the line, as a share of it, each axis's next cube boundary is, and
        // how far between boundaries.
        let next = |axis: usize| {
            let (p, v) = (a[axis], d[axis]);
            if v > 0.0 {
                (p.floor() + 1.0 - p) / v
            } else if v < 0.0 {
                (p - p.floor()) / -v
            } else {
                f32::INFINITY
            }
        };
        let mut t = Vec3::new(next(0), next(1), next(2));
        let delta = Vec3::new(1.0 / d.x.abs(), 1.0 / d.y.abs(), 1.0 / d.z.abs());
        let most = (end - at).abs().element_sum() + 1;
        for _ in 0..=most {
            if self.grid.solid(at) {
                return false;
            }
            if at == end {
                return true;
            }
            let axis = if t.x <= t.y && t.x <= t.z {
                0
            } else if t.y <= t.z {
                1
            } else {
                2
            };
            if t[axis] > 1.0 {
                return true;
            }
            at[axis] += step[axis];
            t[axis] += delta[axis];
        }
        true
    }
}

fn sign(v: f32) -> i32 {
    if v > 0.0 {
        1
    } else if v < 0.0 {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::super::{stamp, tests::kind};
    use super::*;
    use crate::theme::{Theme, assets};
    use crate::{ModuleSet, generate};
    use terra_gen::Seed;

    const SEEDS: u64 = 24;

    fn stamps() -> Vec<Stamp> {
        let mut out = Vec::new();
        for theme in Theme::all(&assets()).unwrap() {
            let set = ModuleSet::new(&theme).unwrap();
            let seeds: Vec<u64> = (0..SEEDS).collect();
            out.extend(
                terra_gen::par::map(&seeds, |&s| {
                    let dungeon = generate(&theme, &set, Seed(s)).ok()?;
                    Some(stamp(&dungeon, &theme, &kind))
                })
                .into_iter()
                .flatten(),
            );
        }
        out
    }

    /// A line the cube walk calls clear passes through no solid cube, as sampling it every
    /// 2 cm finds; one sampling finds blocked, the walk calls blocked.
    #[test]
    fn a_line_is_clear_where_no_point_on_it_is_in_a_solid_cube() {
        let (mut clear, mut blocked) = (0, 0);
        for stamp in stamps().iter().take(12) {
            let sight = Sight::new(stamp);
            for (i, &eye) in stamp.view.iter().enumerate().step_by(7) {
                let to = sight.opening()[i % sight.opening().len()];
                let sampled = (0..=((to - eye).length() / 0.02) as usize).all(|k| {
                    let p = eye.lerp(to, (k as f32 * 0.02 / (to - eye).length()).min(1.0));
                    !sight.grid.solid(cube_of(p))
                });
                let walked = sight.clear(eye, to);
                if !sampled {
                    assert!(!walked, "a line through a solid cube walked clear");
                }
                if walked { clear += 1 } else { blocked += 1 }
            }
        }
        assert!(
            clear > 10 && blocked > 10,
            "{clear} clear, {blocked} blocked"
        );
    }

    /// From the top of the stair the camera sees out; from the goal, at the tour's end, in
    /// nearly every stamp it does not, whichever way it looks.
    #[test]
    fn the_surface_is_seen_from_the_stair_and_not_from_the_goal() {
        let stamps = stamps();
        let mut hidden = 0;
        for stamp in &stamps {
            let sight = Sight::new(stamp);
            assert!(!sight.opening().is_empty(), "a stair has an opening");
            assert!(
                sight.sees_out(stamp.view[0], |_| true),
                "seen from the stair's top"
            );
            assert!(
                !sight.inside(stamp.start + Vec3::Y * 20.0),
                "the sky is outside"
            );
            let goal = *stamp.view.last().unwrap();
            assert!(sight.inside(goal), "the goal is inside");
            hidden += usize::from(!sight.sees_out(goal, |_| true));
        }
        println!("the surface hidden from {hidden} of {} goals", stamps.len());
        assert!(
            hidden * 10 >= stamps.len() * 9,
            "hidden from {hidden} of {} goals",
            stamps.len()
        );
    }
}
