//! Bodies apart by mass (docs/controller.md, step 6; D53's departure from Jolt, whose
//! characters treat each other as walls that never give way): after every body has moved,
//! each pair whose pushboxes overlap is separated along the level line between them, each by
//! its share of the overlap inversely to its mass, a fixed body (infinite mass) not at all.
//! Pairs are taken in ID order for a few rounds, and each push is swept against the ground,
//! so a push never puts a body into a wall; what a wall stops is left for the other to take in
//! a later round.

use super::grid::Grid;
use super::query::{Capsule, Query};
use super::{Body, CROWD_FROM};
use glam::{DVec3, Vec3};

fn inverse_mass(body: &Body) -> f32 {
    if body.kind.mass.is_finite() && body.kind.mass > 0.0 {
        1.0 / body.kind.mass
    } else {
        0.0
    }
}

/// A push of a body along a motion, as far as the ground lets it.
fn shove(body: &mut Body, by: Vec3, ground: &dyn Query) {
    let length = by.length();
    if length < 1e-7 {
        return;
    }
    let capsule = Capsule::standing(Vec3::ZERO, body.up, body.kind.radius, body.kind.height)
        .at(body.position);
    let fraction = ground.sweep(&capsule, by).map_or(1.0, |hit| {
        (hit.fraction - body.kind.padding_m / length).max(0.0)
    });
    body.position += (by * fraction).as_dvec3();
    body.feet = body.position.as_vec3();
}

/// How much farther apart than overlapping reach a body's partners are listed, m: the lists
/// hold until a body has been pushed half this far (a Verlet list's skin; Verlet 1967, as
/// molecular dynamics keeps its neighbour lists).
const SKIN_M: f64 = 1.0;

/// How far a body may have strayed from where the lists placed it, m, before a tick makes them
/// again ahead, beside the steps on another thread ([`ahead`]): short of half the skin, so the
/// lists made then from where the bodies stood as the tick began still hold after it.
const AHEAD_M: f64 = SKIN_M * 0.25;

/// The farthest apart two bodies' feet can be and their pushboxes overlap, m, and the lists'
/// reach, with their skin: two that overlap are less than both radii apart level and less than
/// the taller's height apart along up.
fn reach(bodies: &[Body]) -> f64 {
    let tallest = bodies.iter().map(|b| b.kind.height).fold(0.0, f32::max);
    let widest = bodies.iter().map(|b| b.kind.radius).fold(0.0, f32::max);
    f64::from((4.0 * widest * widest + tallest * tallest).sqrt()) + 1e-3 + SKIN_M
}

/// Every body's ID and place as a tick began, and the lists' reach: what the lists are made of
/// ahead, when they will soon be made again, so the pushes' one thread need not.
pub struct Ahead {
    at: Vec<(u32, DVec3)>,
    reach: f64,
}

/// What to make the lists of ahead this tick, if they will not hold much longer: the bodies or
/// their reach changed, or a body has strayed [`AHEAD_M`] from where they placed it. Lists made
/// of where the bodies stood are answered by the same pairs as any made after the steps (each
/// holds every pair within reach while it holds, and the rest never overlap); the pushes still
/// make them again, exactly as before, if a body went on to stray half the skin.
pub fn ahead(bodies: &[Body], lists: &Lists) -> Option<Ahead> {
    let reach = reach(bodies);
    (bodies.len() >= 2 && !lists.hold(bodies, reach, AHEAD_M)).then(|| Ahead {
        at: bodies.iter().map(|b| (b.id, b.position)).collect(),
        reach,
    })
}

impl Ahead {
    pub fn make(&self, lists: &mut Lists) {
        lists.make(self.at.iter().copied(), self.reach);
    }
}

/// Separates overlapping bodies, kept in ID order, for so many rounds. Each body's partners
/// are the later bodies listed near it from the broadphase ([`Grid`]), in ID order: the same
/// pairs as every later body, those left out too far apart to overlap. The lists are kept
/// from tick to tick, made again when the bodies or their reach changed or a body has moved
/// half the skin since. After the first round a pair is asked again only if it overlapped
/// when last asked or a body of it has been pushed since: otherwise its answer is the same,
/// apart, and asking it again would change nothing.
pub fn separate(bodies: &mut [Body], lists: &mut Lists, ground: &dyn Query, rounds: usize) {
    if bodies.len() < 2 {
        return;
    }
    let reach = reach(bodies);
    let strayed = |body: &Body, at: DVec3| (body.position - at).length() > SKIN_M * 0.5;
    if !lists.hold(bodies, reach, 0.0) {
        lists.make(bodies.iter().map(|b| (b.id, b.position)), reach);
    }
    // A clock of pairs asked: when each pair last answered apart (0 if it overlapped or has
    // not been asked), and when each body was last pushed.
    let mut clock = 0u64;
    lists.apart_at.clear();
    lists.apart_at.resize(lists.partners.len(), 0);
    lists.pushed_at.clear();
    lists.pushed_at.resize(bodies.len(), 0);
    for _ in 0..rounds {
        let mut moved = false;
        for i in 0..bodies.len() {
            let mut k = lists.from[i];
            while k < lists.from[i + 1] {
                let j = lists.partners[k] as usize;
                clock += 1;
                let apart = lists.apart_at[k];
                if apart > lists.pushed_at[i] && apart > lists.pushed_at[j] {
                    k += 1;
                    continue;
                }
                let (head, tail) = bodies.split_at_mut(j);
                let (a, b) = (&mut head[i], &mut tail[0]);
                if !pair(a, b, ground) {
                    lists.apart_at[k] = clock;
                    k += 1;
                    continue;
                }
                (lists.apart_at[k], lists.pushed_at[i], lists.pushed_at[j]) = (0, clock, clock);
                k += 1;
                moved = true;
                if strayed(&head[i], lists.at[i]) || strayed(&tail[0], lists.at[j]) {
                    lists.make(bodies.iter().map(|b| (b.id, b.position)), reach);
                    lists.apart_at.clear();
                    lists.apart_at.resize(lists.partners.len(), 0);
                    let after = &lists.partners[lists.from[i]..lists.from[i + 1]];
                    k = lists.from[i] + after.partition_point(|&p| p as usize <= j);
                }
            }
        }
        if !moved {
            break;
        }
    }
}

/// Each body's later partners within a reach of its feet, ascending, and where every body
/// stood when they were listed: the bodies' IDs then, and the reach. With them, a tick's
/// pushes' clock of when each pair last answered apart and each body was last pushed.
#[derive(Clone, Default)]
pub struct Lists {
    from: Vec<usize>,
    partners: Vec<u32>,
    at: Vec<DVec3>,
    ids: Vec<u32>,
    reach: f64,
    apart_at: Vec<u64>,
    pushed_at: Vec<u64>,
}

impl Lists {
    /// Whether the lists hold for these bodies with this reach: the same bodies, and none
    /// strayed further than `short` m short of half the skin from where they were listed.
    fn hold(&self, bodies: &[Body], reach: f64, short: f64) -> bool {
        self.reach == reach
            && self.ids.len() == bodies.len()
            && bodies
                .iter()
                .zip(self.ids.iter().zip(&self.at))
                .all(|(b, (&id, &at))| {
                    b.id == id && (b.position - at).length() <= SKIN_M * 0.5 - short
                })
    }

    fn make(&mut self, bodies: impl Iterator<Item = (u32, DVec3)>, reach: f64) {
        self.reach = reach;
        self.ids.clear();
        self.at.clear();
        for (id, at) in bodies {
            self.ids.push(id);
            self.at.push(at);
        }
        let count = self.ids.len();
        let few = count < CROWD_FROM;
        // Cells as wide as the reach: a body's partners lie in the cells around its own.
        let grid = Grid::of_cells(self.at.iter().copied().filter(|_| !few), 0.0, reach);
        let (mut near, r) = (Vec::new(), DVec3::splat(reach));
        self.from.clear();
        self.partners.clear();
        for (i, &at) in self.at.iter().enumerate() {
            self.from.push(self.partners.len());
            if few {
                self.partners.extend(i as u32 + 1..count as u32);
                continue;
            }
            grid.near(at - r, at + r, &mut near);
            let later = near.iter().filter(|&&j| j as usize > i);
            let within = later.filter(|&&j| (self.at[j as usize] - at).length() <= reach);
            self.partners.extend(within);
        }
        self.from.push(self.partners.len());
    }
}

/// Separates one pair if it overlaps: whether it did.
fn pair(a: &mut Body, b: &mut Body, ground: &dyn Query) -> bool {
    let (ia, ib) = (inverse_mass(a), inverse_mass(b));
    if ia + ib <= 0.0 {
        return false;
    }
    let apart = (b.position - a.position).as_vec3();
    // Overlapping, they are nearer than both radii level and the taller's height along up.
    let (sum, tall) = (
        a.kind.radius + b.kind.radius,
        a.kind.height.max(b.kind.height),
    );
    if apart.length_squared() >= (sum * sum + tall * tall) * 1.01 {
        return false;
    }
    let up = (a.up + b.up).normalize_or(a.up);
    let (ha, hb) = (0.0, apart.dot(up));
    if hb > ha + a.kind.height || hb + b.kind.height < ha {
        return false;
    }
    let level = apart - up * hb;
    let distance = level.length();
    let overlap = a.kind.radius + b.kind.radius - distance;
    if overlap <= 0.0 {
        return false;
    }
    let dir = if distance > 1e-6 {
        level / distance
    } else {
        a.facing
    };
    let share = ia / (ia + ib);
    shove(a, -dir * overlap * share, ground);
    shove(b, dir * overlap * (1.0 - share), ground);
    true
}

#[cfg(test)]
mod tests {
    use super::super::{Bodies, Kind, Sdf, World};
    use super::*;

    /// Every pair, as before the broadphase: the reference the grid's pairs must match.
    fn every_pair(bodies: &mut [Body], ground: &dyn Query, rounds: usize) {
        for _ in 0..rounds {
            let mut moved = false;
            for i in 0..bodies.len() {
                for j in i + 1..bodies.len() {
                    let (head, tail) = bodies.split_at_mut(j);
                    moved |= pair(&mut head[i], &mut tail[0], ground);
                }
            }
            if !moved {
                break;
            }
        }
    }

    #[test]
    fn the_grids_pairs_push_a_packed_crowd_exactly_as_every_pair_does() {
        let floor = Sdf {
            distance: |at: DVec3| at.y as f32,
            steepest: 1.0,
        };
        let none = |_: Vec3| None;
        let world = World {
            ground: &floor,
            water: &none,
            gravity: 9.81,
            centre: Vec3::new(0.0, -1.0e6, 0.0),
        };
        let mut crowd = Bodies::default();
        // Packed tight, some on top of each other, a few heavy and one that never gives way.
        for id in 0..300u32 {
            let h = terra_gen::hash::lowbias32(id);
            let (x, z) = ((h % 97) as f32 * 0.17, ((h >> 8) % 89) as f32 * 0.13);
            let mut kind = Kind::man();
            kind.mass = match id % 23 {
                0 => f32::INFINITY,
                5 | 11 => 320.0,
                _ => kind.mass,
            };
            crowd.add(id, kind, Vec3::new(x, 0.0, z), Vec3::X, &world);
        }
        let mut by_grid = crowd.bodies.clone();
        let mut by_all = crowd.bodies.clone();
        // Ticks apart, each body walked a little between them (a few under the lists' half
        // skin, so kept, then past it), the lists kept from one to the next.
        let mut lists = Lists::default();
        for tick in 0..8u32 {
            let before = by_grid.clone();
            separate(&mut by_grid, &mut lists, &floor, 4);
            every_pair(&mut by_all, &floor, 4);
            let moved = by_grid
                .iter()
                .zip(&before)
                .filter(|(a, b)| a.position != b.position)
                .count();
            assert!(moved > 20, "{tick} {moved}");
            for (a, b) in by_grid.iter().zip(&by_all) {
                assert_eq!(
                    a.position.to_array().map(f64::to_bits),
                    b.position.to_array().map(f64::to_bits),
                    "{tick} {}",
                    a.id
                );
            }
            for body in by_grid.iter_mut().chain(by_all.iter_mut()) {
                let h = terra_gen::hash::lowbias32(body.id ^ (tick << 16));
                let way = DVec3::new(f64::from(h % 7) - 3.0, 0.0, f64::from((h >> 4) % 7) - 3.0);
                body.position += way * 0.04;
            }
        }
    }
}
