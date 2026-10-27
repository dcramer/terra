//! The camera's way along the tour: kept off walls, pillar corners and the open doors'
//! leaves, its eye lowered where the slab over a stair comes close. A cinematic camera's
//! collision as shipped games keep it (a sphere the camera may not enter, Unreal's
//! `ProbeSize` on its spring arm), worked out ahead of time over the cubes, as the tour is.
//!
//! - `across`: how far a point is from what stands in its own layer of cubes, in plan.
//! - `around`: how far a point is from anything, in three dimensions.
//! - `roomy`: a way the tour's legs may take, its eye `ROOM_M` clear of walls on both sides.
//! - `view`: the tour sampled every `STEP_M`, an eye over each foot as high as `EYE_M` allows
//!   with `ROOM_M` all round, smoothed so it never rises past what its neighbours allow.

use super::{CUBE, DOORWAY_M, Frame, Grid, Object, ObjectRole, clear, cube_of, stands};
use glam::{IVec3, Vec2, Vec3};

/// How far the camera keeps from what stands round it, in metres.
pub const ROOM_M: f32 = 0.6;
/// The least room all round where `ROOM_M` cannot be had: under the edge of the slab over a
/// stair, 1.4-1.5 m above its treads, a camera that keeps `ROOM_M` from the slab comes closer
/// than that to the treads, so there it keeps this from both and `ROOM_M` in plan.
pub const TIGHT_M: f32 = 0.4;
/// How high over the feet the eye goes: a walker's eye at most, a crouch at least.
pub const EYE_M: [f32; 2] = [0.9, 1.6];
/// How far apart the view's points are along the tour, in metres.
pub const STEP_M: f32 = 0.25;
/// How far the distances are measured; anything farther counts as this.
const REACH_M: f32 = 1.0;
/// Heights over the feet where a leg's way is kept `ROOM_M` from walls: under the slab
/// over a stair, which the eye ducks, and over the half cubes at a wall's foot.
const ROOMY_AT_M: [f32; 2] = [0.9, 1.2];
/// The eye's heights tried, from `EYE_M[0]` up.
const EYE_STEP_M: f32 = 0.05;
/// The running minimum of the allowed eye height reaches this far either way along the
/// tour, and the mean over it half as far: the mean of minima stays under every height a
/// point allows, so the eye is smooth and never higher than its own place allows.
const SMOOTH_M: f32 = 1.0;
/// How far a point of the tour is moved, at most, to a place with room round it, in cubes.
const SETTLE_CUBES: i32 = 3;
/// How far the camera's feet are pushed off the tour at most, in metres.
const PUSH_M: f32 = 0.75;
/// Steps a push takes at most.
const PUSH_STEPS: usize = 16;
/// A door leaf's half thickness.
const LEAF_HALF_M: f32 = 0.05;

/// A door leaf standing open: a thin wall from its hinge into the room, in plan, and its
/// height.
#[derive(Clone, Copy, Debug)]
struct Leaf {
    a: Vec2,
    b: Vec2,
    y: [f32; 2],
}

/// What a camera keeps clear of: the cubes that are not open, and the open doors' leaves.
pub(super) struct Room {
    map: Grid,
    leaves: Vec<Leaf>,
}

impl Room {
    pub fn of(map: Grid, objects: &[Object]) -> Room {
        let leaves = objects
            .iter()
            .filter(|o| o.role == ObjectRole::Door)
            .flat_map(|o| leaves(o.at, o.facing))
            .collect();
        Room { map, leaves }
    }

    /// The solid cubes it keeps clear of.
    pub fn cubes(&self) -> &Grid {
        &self.map
    }

    /// The plan distance from a point to the cubes not open in its own layer and to the door
    /// leaves at its height, up to `REACH_M`.
    pub fn across(&self, p: Vec3) -> f32 {
        let mut d = REACH_M;
        self.near(p, &mut |e, _| d = d.min(e));
        d.max(0.0)
    }

    /// Each cube not open in a point's layer and each leaf at its height, within `REACH_M`
    /// in plan: its distance and its nearest point, in plan.
    fn near(&self, p: Vec3, each: &mut dyn FnMut(f32, Vec2)) {
        let c = cube_of(p);
        let q = Vec2::new(p.x, p.z);
        for x in -2..=2 {
            for z in -2..=2 {
                let at = c + IVec3::new(x, 0, z);
                if !clear(&self.map, at) {
                    let lo = Vec2::new(at.x as f32, at.z as f32) * CUBE;
                    let n = q.clamp(lo, lo + Vec2::splat(CUBE));
                    each(q.distance(n), n);
                }
            }
        }
        for leaf in self
            .leaves
            .iter()
            .filter(|l| (l.y[0]..=l.y[1]).contains(&p.y))
        {
            let ab = leaf.b - leaf.a;
            let t = ((q - leaf.a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            let n = leaf.a + ab * t;
            each(q.distance(n) - LEAF_HALF_M, n);
        }
    }

    /// The least room in plan at `ROOMY_AT_M` over some feet.
    fn room_at(&self, feet: Vec3) -> f32 {
        ROOMY_AT_M
            .iter()
            .map(|&up| self.across(feet + Vec3::Y * up))
            .fold(f32::MAX, f32::min)
    }

    /// Feet moved in plan out of `ROOM_M` of what stands round them at `ROOMY_AT_M`, by
    /// the mean of the pushes of everything nearer, a step at a time (a sphere pushed out
    /// of what it overlaps, as a physics engine resolves contacts), to between walls on both
    /// sides where it cannot have that room; at most `PUSH_M`, never into a cube.
    fn pushed(&self, feet: Vec3) -> Vec3 {
        let mut best = (self.room_at(feet), feet);
        let mut p = feet;
        for _ in 0..PUSH_STEPS {
            if self.room_at(p) >= ROOM_M + 0.01 {
                break;
            }
            let (mut push, mut contacts) = (Vec2::ZERO, 0);
            let q = Vec2::new(p.x, p.z);
            for &up in &ROOMY_AT_M {
                self.near(p + Vec3::Y * up, &mut |d, n| {
                    if d < ROOM_M + 0.02 && q.distance(n) > 1e-4 {
                        push += (q - n).normalize() * (ROOM_M + 0.02 - d);
                        contacts += 1;
                    }
                });
            }
            if contacts == 0 {
                break;
            }
            let step = push / contacts as f32;
            let next = p + Vec3::new(step.x, 0.0, step.y);
            let off = Vec2::new(next.x - feet.x, next.z - feet.z);
            let next = if off.length() > PUSH_M {
                let off = off.normalize() * PUSH_M;
                feet + Vec3::new(off.x, 0.0, off.y)
            } else {
                next
            };
            if ROOMY_AT_M
                .iter()
                .any(|&up| !clear(&self.map, cube_of(next + Vec3::Y * up)))
            {
                break;
            }
            p = next;
            let room = self.room_at(p);
            if room > best.0 {
                best = (room, p);
            }
        }
        best.1
    }

    /// The distance from a point to anything not open or a leaf, up to `REACH_M`.
    pub fn around(&self, p: Vec3) -> f32 {
        let c = cube_of(p);
        let mut d2 = REACH_M * REACH_M;
        for x in -2..=2 {
            for y in -2..=2 {
                for z in -2..=2 {
                    let at = c + IVec3::new(x, y, z);
                    if !clear(&self.map, at) {
                        let lo = at.as_vec3() * CUBE;
                        let off = (lo - p).max(p - (lo + Vec3::splat(CUBE))).max(Vec3::ZERO);
                        d2 = d2.min(off.length_squared());
                    }
                }
            }
        }
        for leaf in &self.leaves {
            let up = (leaf.y[0] - p.y).max(p.y - leaf.y[1]).max(0.0);
            let flat = to_leaf(p, leaf).max(0.0);
            d2 = d2.min(flat * flat + up * up);
        }
        d2.sqrt()
    }

    /// Whether a walker standing at these feet has `ROOM_M` in plan at both of `ROOMY_AT_M`.
    pub fn roomy_at(&self, feet: Vec3) -> bool {
        ROOMY_AT_M
            .iter()
            .all(|&up| self.across(feet + Vec3::Y * up) >= ROOM_M)
    }

    /// Whether the way from one foot to another passes no cube at a body's chest or eye
    /// (`sight`) and keeps `ROOM_M` from walls and leaves in plan, every 0.1 m.
    pub fn roomy(&self, a: Vec3, b: Vec3) -> bool {
        let steps = ((b - a).length() / 0.1).ceil().max(1.0) as i32;
        super::runin::sight(&self.map, a, b)
            && (0..=steps).all(|k| self.roomy_at(a.lerp(b, k as f32 / steps as f32)))
    }

    /// Where a way is narrower than twice `ROOM_M` (a corridor by a heap), the feet moved
    /// across it, up to `PUSH_M` either way, to where it has the most room in plan, the
    /// nearest of the places that have `ROOM_M`; the feet as they are where they have it.
    fn widest(&self, feet: Vec3, side: Vec3) -> Vec3 {
        if side == Vec3::ZERO || self.room_at(feet) >= ROOM_M + 0.01 {
            return feet;
        }
        let steps = (PUSH_M / 0.05).round() as i32;
        let mut best = (self.room_at(feet), 0, feet);
        for k in (1..=steps).flat_map(|k| [k, -k]) {
            let q = feet + side * (k as f32 * 0.05);
            if ROOMY_AT_M
                .iter()
                .any(|&up| !clear(&self.map, cube_of(q + Vec3::Y * up)))
            {
                continue;
            }
            let room = self.room_at(q).min(ROOM_M + 0.01);
            if room > best.0 + 1e-3 {
                best = (room, k.abs(), q);
            }
        }
        best.2
    }

    /// The standing place nearest a point of the tour with `ROOM_M` round it in plan, within
    /// `SETTLE_CUBES` and a cube up or down: a doorway's step beside an alcove's wall, a
    /// room's middle at its pillar. The point itself if it has that room, or none near does.
    pub fn settle(&self, p: Vec3) -> Vec3 {
        if self.roomy_at(p) {
            return p;
        }
        let c = cube_of(p);
        let r = SETTLE_CUBES;
        let mut best: Option<(f32, Vec3)> = None;
        for x in -r..=r {
            for y in -1..=1 {
                for z in -r..=r {
                    let at = c + IVec3::new(x, y, z);
                    let feet = (at.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CUBE;
                    let d = feet.distance(p);
                    if best.is_none_or(|(b, _)| d < b)
                        && stands(&self.map, at)
                        && self.roomy_at(feet)
                    {
                        best = Some((d, feet));
                    }
                }
            }
        }
        best.map_or(p, |(_, q)| q)
    }

    /// The eye over a foot: the highest of `EYE_M` reached from its lowest through open cubes
    /// with `ROOM_M` all round; else the highest with `TIGHT_M` all round and `ROOM_M` in
    /// plan; else the one with the most room.
    fn eye(&self, feet: Vec3) -> f32 {
        let n = ((EYE_M[1] - EYE_M[0]) / EYE_STEP_M).round() as i32;
        let mut best = (f32::MIN, EYE_M[0]);
        let (mut high, mut tight) = (None, None);
        for k in 0..=n {
            let e = EYE_M[0] + k as f32 * EYE_STEP_M;
            let p = feet + Vec3::Y * e;
            if !clear(&self.map, cube_of(p)) {
                break;
            }
            let room = self.around(p);
            if room >= ROOM_M {
                high = Some(e);
            } else if room >= TIGHT_M && self.across(p) >= ROOM_M {
                tight = Some(e);
            }
            if room >= best.0 {
                best = (room, e);
            }
        }
        high.or(tight).unwrap_or(best.1)
    }

    /// The camera's way: the tour's feet every `STEP_M` and an eye over each.
    pub fn view(&self, route: &[Vec3]) -> Vec<Vec3> {
        let feet = resample(route, STEP_M);
        let n = feet.len();
        // Across the way at each foot, level.
        let side: Vec<Vec3> = (0..n)
            .map(|i| {
                let d = (feet[(i + 1).min(n - 1)] - feet[i.saturating_sub(1)])
                    * Vec3::new(1.0, 0.0, 1.0);
                Vec3::new(-d.z, 0.0, d.x).normalize_or_zero()
            })
            .collect();
        let placed = |i: usize, f: Vec3| self.widest(self.pushed(f), side[i]);
        // Each foot pushed off what stands round it, the pushes smoothed along the way so the
        // camera drifts rather than jumps, and pushed again where smoothing gave room back.
        let push: Vec<Vec3> = (0..n).map(|i| placed(i, feet[i]) - feet[i]).collect();
        let feet: Vec<Vec3> = (0..n)
            .map(|i| {
                let near = &push[i.saturating_sub(2)..(i + 3).min(n)];
                let mean = near.iter().sum::<Vec3>() / near.len() as f32;
                placed(i, feet[i] + mean)
            })
            .collect();
        let allowed: Vec<f32> = feet.iter().map(|&f| self.eye(f)).collect();
        let reach = (SMOOTH_M / STEP_M).round() as usize;
        let window = |i: usize, r: usize| i.saturating_sub(r)..(i + r + 1).min(feet.len());
        let low: Vec<f32> = (0..feet.len())
            .map(|i| {
                allowed[window(i, reach)]
                    .iter()
                    .copied()
                    .fold(f32::MAX, f32::min)
            })
            .collect();
        (0..feet.len())
            .map(|i| {
                let near = &low[window(i, reach / 2)];
                let eye = near.iter().sum::<f32>() / near.len() as f32;
                feet[i] + Vec3::Y * eye
            })
            .collect()
    }
}

/// The two leaves of a doorway's door standing open: each from its jamb, half the doorway
/// from the sill's middle either way, a leaf's width into the room it opens into: where the game's
/// leaves stood before they were hung to lie flat on a wall (`door::leaf`); the tours are
/// made keeping clear of them still.
fn leaves(sill: Vec3, facing: usize) -> [Leaf; 2] {
    let f = Frame::new(IVec3::ZERO, facing);
    let o = f.point(0.0, 0.0, 0.0);
    let across = f.point(1.0, 0.0, 0.0) - o;
    let into = f.point(0.0, 0.0, 1.0) - o;
    let flat = |v: Vec3| Vec2::new(v.x, v.z);
    let half = DOORWAY_M[0] * 0.5 / CUBE;
    [-half, half].map(|s| {
        let hinge = sill + across * s;
        Leaf {
            a: flat(hinge),
            b: flat(hinge + into * half),
            y: [sill.y, sill.y + DOORWAY_M[1]],
        }
    })
}

/// The plan distance from a point to a leaf's face.
fn to_leaf(p: Vec3, leaf: &Leaf) -> f32 {
    let q = Vec2::new(p.x, p.z);
    let ab = leaf.b - leaf.a;
    let t = ((q - leaf.a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    (q - (leaf.a + ab * t)).length() - LEAF_HALF_M
}

/// Points along a path every `step` metres from its start, and its end.
pub(super) fn resample(path: &[Vec3], step: f32) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = path.first().copied().into_iter().collect();
    let mut owed = step;
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = a.distance(b);
        let mut s = owed;
        while s <= length {
            out.push(a.lerp(b, s / length));
            s += step;
        }
        owed = s - length;
    }
    if let (Some(&end), Some(&last)) = (path.last(), out.last())
        && end.distance(last) > 1e-3
    {
        out.push(end);
    }
    out
}
