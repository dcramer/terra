//! A massif's neighbours (D444, mountains rounds 25-28): the lesser tops round each horn and the
//! ridges out to them, so a horn stands in a range rather than alone on its plain.
//!
//! The lake's hero peak rose straight out of a level treeline with nothing beside it: "that
//! mountain is just a little bit off ... theres nothing around it, thats not usually how
//! mountains work" (the developer, 2026-10-22). Real peaks stand in ranges: the Matterhorn
//! with the Dent d'Hérens and the Breithorn along its ridges, Krywań at the end of a ridge
//! running on to Hlinská and the Kôprovský štít. Here each horn sends ridges out along two
//! or three of its arêtes, falling to cols and rising to a lesser top of its own faces at
//! the end of each, at half to most of its height, out to about twice its reach. They stand
//! by the horn's own site, so a horn on a range's edge has its neighbours too, over the
//! range's own ground wherever the range is, and never out of it. They rise out of the ground
//! the horns and their cirques leave, by a smooth maximum (Inigo Quilez's `smax`), so where
//! they meet a horn's flank or a cirque's floor no crease is left for the faces to step into
//! a lip (`faces_stand_sheer_over_ledges_that_still_drain`).
//!
//! What the neighbours of the horns round a point are depends only on the lattice cell the
//! point is in, so each thread works them out once a cell and keeps them (`STANDING`): the
//! ground's samples, metres apart, ask the same cell thousands of times.

use super::horns::{FaceSet, HORN_LEAST, faces_from, horn_profile, horn_ways, towered};
use super::{CREST, HORN_JITTER, HORN_REACH, HORNS_FROM, HORNS_KEPT, Range, VALLEY};
use glam::{IVec3, Vec3};
use std::cell::{Cell, RefCell};
use std::f32::consts::TAU;
use terra_gen::{hash, smoothstep};

/// How many cells of the horns' lattice round a point's are looked at either way, and how far
/// past a horn's site its neighbours reach, as a share of its reach.
const SPAN: i32 = 2;
const EXTENT: f32 = OUT.1 + TOP_REACH;
/// How many of the cells the massifs round a point are remembered by (`STANDING`) go across
/// one of the horns' lattice, either way: the smaller the cell, the fewer massifs can reach
/// into it, so the fewer each sample looks at (55 at the lake with the lattice's own cells).
const FINE: i32 = 4;
/// How many neighbours a horn has, one on each of so many of its arêtes.
const NEIGHBOURS: (usize, usize) = (2, 3);
/// How far out along an arête a neighbour stands, as shares of the horn's reach, and how far
/// its way strays off the arête's either way, in radians.
const OUT: (f32, f32) = (1.9, 2.8);
const STRAY: f32 = 0.45;
/// How high a neighbour stands, as shares of the horn's height, and how far its own faces
/// reach, as a share of the horn's.
const SHARE: (f32, f32) = (0.5, 0.78);
const TOP_REACH: f32 = 0.7;
/// From how far out, as a share of its reach, a neighbour's foot eases into the ground, wider
/// than a horn's (0.55): it rises out of a cirque's floor, hundreds of metres under the ground
/// its height is taken from, and eased as narrowly, its foot bent so sharply that the faces,
/// stepped by the slope above it, lifted the ground below into a 3 cm lip.
const TOP_FADE: f32 = 0.45;
/// The ridge out to a neighbour: from how far out on the horn, as a share of its reach; how
/// wide either side, the same; how high it starts, as a share of the horn's height, and how
/// high it ends, as a share of the neighbour's, falling to a col under the neighbour's faces;
/// and the power its sides fall by.
const RIDGE_FROM: f32 = 0.3;
const RIDGE_WIDE: f32 = 0.45;
const RIDGE_HIGH: (f32, f32) = (0.55, 0.6);
const RIDGE_PROFILE: f32 = 1.5;
/// A spur down each of a horn's arêtes that has no neighbour, a ridge of its own falling from
/// the arête's shoulder to its foot, as the Matterhorn's Hörnli ridge steps down to the
/// Hörnli: how far out it runs, as shares of the horn's reach, how high its crest starts and ends, as a
/// share of the horn's height, and how far its way strays off the arête's either way, in
/// radians. Without, such an arête fell off the horn's cap into the cirque under it in a
/// cliff and a notch (the lake's peak, mountains round 31: 106 px over 50 px).
const SPUR_OUT: (f32, f32) = (1.3, 1.7);
const SPUR_HIGH: f32 = 0.75;
const SPUR_END: (f32, f32) = (0.12, 0.25);
const SPUR_STRAY: f32 = 0.15;
/// The teeth a ridge's crest breaks into between the horn and its neighbour, as the Tatras'
/// ridges do (Orla Perć's gendarmes): about how far apart, as a share of the horn's reach,
/// and the most a notch between two cuts into the crest's height over the horn's ground. A
/// crest falling evenly from the arête to the col read as a flat-topped wall (round 28's
/// review: "pale, smeared and flat-topped").
const TEETH_SPACING: f32 = 0.2;
const TEETH_DEPTH: f32 = 0.22;
/// From how far out, as a share of its width, a ridge's side eases into the ground, as a
/// horn's foot does.
const RIDGE_FADE: f32 = 0.55;
/// The most a neighbour or a ridge stands over the range's ground, as a share of its horn's
/// height, where the range falls away under it.
const LIFT_MOST: f32 = 1.0;
/// How much taller than its own height over its horn's ground a neighbour or a ridge stands
/// over ground under the horn's, for each horn's height that ground lies under it.
const OVER_GROUND: f32 = 0.5;
/// Over what depth of ground under the horn's, as shares of its height, a neighbour or a
/// ridge turns from standing its whole gap over that ground to standing by its own profile:
/// shallower ground keeps the gap, so the range's inner walls keep their sheer risers.
const OVER_GROUND_FROM: (f32, f32) = (0.4, 0.9);
/// Over how many metres either way a neighbour's or a ridge's foot eases into the ground
/// under it, and its top under its most: a hard `max` left a crease where they met.
const EASE_M: f32 = 40.0;
/// How many horns' sites, and how many lattice cells' standing horns, each thread remembers.
const SITE_SLOTS: usize = 4096;
const CELL_SLOTS: usize = 256;

thread_local! {
    /// Whether a site's horn stands and the ground under it, by its tag (`Range::site_stands`):
    /// the answer takes the ridged sum at the site.
    static SITES: RefCell<Vec<(u64, (f32, f32))>> = RefCell::new(vec![(0, (0.0, 0.0)); SITE_SLOTS]);
    /// The standing horns round a lattice cell, by its tag (`Range::standing_round`).
    static STANDING: RefCell<Vec<(u64, Vec<Massif>)>> =
        RefCell::new((0..CELL_SLOTS).map(|_| (0, Vec::new())).collect());
    /// The last cell asked of `STANDING`, its planet's radius' bits and range's key, and its tag.
    static LAST: Cell<((IVec3, u32, u64), u64)> = const { Cell::new(((IVec3::ZERO, 0, 0), 0)) };
}

/// A key from a seed and its parts, for telling ranges and sites apart.
pub(super) fn key(seed: u64, parts: impl IntoIterator<Item = u64>) -> u64 {
    parts
        .into_iter()
        .fold(hash::splitmix64(seed), |k, part| hash::splitmix64(k ^ part))
}

/// The bits of a list of spots, as a key's parts.
pub(super) fn bits(spots: &[Vec3]) -> impl Iterator<Item = u64> + '_ {
    spots
        .iter()
        .flat_map(|s| [s.x, s.y, s.z])
        .map(|v| u64::from(v.to_bits()))
}

/// A standing horn's massif, laid out once: its site and the site's distance from the planet's
/// middle, how far its faces reach and how high it stands, in metres, the ground under it,
/// its profile's power, and its neighbours.
struct Massif {
    site: Vec3,
    length: f32,
    reach: f32,
    tall: f32,
    base: f32,
    profile: f32,
    tops: Vec<Neighbour>,
}

/// One neighbour: its way off the horn, level, how far out it stands and how high, in metres,
/// its three faces' ways and steepness, and its own 0 to 1 for its towers.
struct Neighbour {
    way: Vec3,
    out: f32,
    top: f32,
    faces: [(Vec3, f32); 4],
    key: f32,
    /// Whether its top and the ridge out to it can reach a point of the remembered cell it is
    /// kept for (`Range::standing_round`).
    top_near: bool,
    ridge_near: bool,
    /// A spur (`SPUR_*`): only its ridge, falling to `top` at its end, with no top of its own.
    spur: bool,
}

impl Range {
    /// How far the neighbours of the horns round a point and the ridges out to them stand over
    /// the ground the horns leave there, `horns` metres over the range's own `ground`, in
    /// metres, 0 off them, and how far they stand there as walls, 0 to 1. Their heights are
    /// taken from the ground the horn stands on, so a horn on a range's falling edge keeps its
    /// neighbours near its own height.
    pub(super) fn neighbours(&self, point: Vec3, ground: f32, horns: f32) -> (f32, f32) {
        let across = self.mountains.horns_across_m;
        let cell = (point / (across / FINE as f32)).floor().as_ivec3();
        let length = point.length();
        let radius = libm::roundf(length / 1000.0) * 1000.0;
        // The samples come in runs over one cell, so the last cell's tag is kept to save
        // hashing it again.
        let asked = (cell, radius.to_bits(), self.key);
        let tag = match LAST.get() {
            (last, tag) if last == asked => tag,
            _ => {
                let parts = [cell.x, cell.y, cell.z].map(|v| u64::from(v as u32));
                let tag = key(
                    self.key,
                    parts.into_iter().chain([u64::from(radius.to_bits())]),
                ) | 1;
                LAST.set((asked, tag));
                tag
            }
        };
        let slot = (tag % CELL_SLOTS as u64) as usize;
        STANDING.with(|standing| {
            if standing.borrow()[slot].0 != tag {
                let massifs = self.standing_round(cell, radius);
                standing.borrow_mut()[slot] = (tag, massifs);
            }
            let standing = standing.borrow();
            let up = point.normalize_or(Vec3::Y);
            let mut most = (0.0, 0.0);
            for massif in &standing[slot].1 {
                // By the site's own height off the ground, which unlike the offset along the
                // point's up does not turn over across their reach.
                if (massif.length - length).abs() > across * 0.5 {
                    continue;
                }
                let offset = point - massif.site;
                let flat = offset - up * offset.dot(up);
                let extent = massif.reach * EXTENT;
                if flat.length_squared() >= extent * extent {
                    continue;
                }
                most = higher(most, massif.lift(flat, ground, horns));
            }
            most
        })
    }

    /// The massifs of the standing horns round a remembered cell (`FINE`) that can reach a
    /// point in it, on a planet of `radius` metres.
    fn standing_round(&self, fine: IVec3, radius: f32) -> Vec<Massif> {
        let m = &self.mountains;
        let across = m.horns_across_m;
        let reach = across * HORN_REACH;
        let extent = reach * EXTENT;
        let cell = fine.div_euclid(IVec3::splat(FINE));
        // How near and far from the planet's middle the cell's points lie.
        let size = across / FINE as f32;
        let (low, high) = (fine.as_vec3() * size, (fine + IVec3::ONE).as_vec3() * size);
        let nearest = Vec3::ZERO.clamp(low, high).length();
        let farthest = low.abs().max(high.abs()).length();
        let mut massifs = Vec::new();
        for dx in -SPAN..=SPAN {
            for dy in -SPAN..=SPAN {
                for dz in -SPAN..=SPAN {
                    let c = cell + IVec3::new(dx, dy, dz);
                    let jitter = self.tops.point_in(c.x, c.y, c.z);
                    let site =
                        (c.as_vec3() + Vec3::splat(0.5) + (jitter - 0.5) * HORN_JITTER) * across;
                    let length = site.length();
                    if length < nearest - across * 0.5 || length > farthest + across * 0.5 {
                        continue;
                    }
                    // A massif reaches a point only within `extent` of its site laid level and
                    // half a lattice cell up or down (`Range::neighbours`); laid level, the
                    // sphere falls away under the site by at most extent^2 / length more. Farther
                    // from the whole cell than both together, it reaches none of it.
                    let up_down = across * 0.5 + extent * extent / length.max(1.0);
                    if site.distance(site.clamp(low, high)) > libm::hypotf(extent, up_down) {
                        continue;
                    }
                    let pick = self.faces.point_in(c.x, c.y, c.z);
                    if pick.x > HORNS_KEPT {
                        continue;
                    }
                    let (stands, base) = self.site_stands(c, site, radius);
                    if stands <= 0.0 {
                        continue;
                    }
                    let height = m.horns_m * (HORN_LEAST + (1.0 - HORN_LEAST) * jitter.y);
                    let count = if pick.y < 0.5 { 3 } else { 4 };
                    let turn = pick.z * TAU;
                    let tall = height * stands;
                    // Only the neighbours whose top or ridge can reach the cell, as the massif
                    // itself: each within its own reach of the point laid level, so within that
                    // and `up_down` of it in space.
                    let mut tops =
                        self.neighbours_of(c, horn_ways(site), (reach, tall), count, turn);
                    let (middle, half) = ((low + high) * 0.5, (high - low).length() * 0.5);
                    for n in &mut tops {
                        let top = site + n.way * n.out;
                        n.top_near = !n.spur
                            && top.distance(top.clamp(low, high)) < reach * TOP_REACH + up_down;
                        let from = site + n.way * (reach * RIDGE_FROM);
                        let along = (middle - from)
                            .dot(n.way)
                            .clamp(0.0, n.out - reach * RIDGE_FROM);
                        n.ridge_near = middle.distance(from + n.way * along) - half
                            < reach * RIDGE_WIDE + up_down;
                    }
                    tops.retain(|n| n.top_near || n.ridge_near);
                    if tops.is_empty() {
                        continue;
                    }
                    massifs.push(Massif {
                        site,
                        length,
                        reach,
                        tall,
                        base,
                        profile: horn_profile(pick.x),
                        tops,
                    });
                }
            }
        }
        massifs
    }

    /// A horn's neighbours, from its cell's hash: on two or three of its `count` arêtes turned
    /// from `turn`, laid level in its `(east, north)`, by its `reach` and how `tall` it stands.
    fn neighbours_of(
        &self,
        c: IVec3,
        (east, north): (Vec3, Vec3),
        (reach, tall): (f32, f32),
        count: i32,
        turn: f32,
    ) -> Vec<Neighbour> {
        let pick = self.leans(c, -3);
        let count = count as usize;
        let spread = (NEIGHBOURS.1 - NEIGHBOURS.0 + 1) as f32;
        let many = (NEIGHBOURS.0 + (pick.x * spread) as usize)
            .min(NEIGHBOURS.1)
            .min(count);
        let first = (pick.y * count as f32) as usize;
        let neighbours = (0..many).map(|k| {
            let i = (first + k) % count;
            let key = self.leans(c, 20 + i as i32);
            let a = turn + TAU * (i as f32 + 0.5) / count as f32 + STRAY * (2.0 * key.z - 1.0);
            Neighbour {
                way: east * libm::cosf(a) + north * libm::sinf(a),
                out: reach * (OUT.0 + (OUT.1 - OUT.0) * key.x),
                top: tall * (SHARE.0 + (SHARE.1 - SHARE.0) * key.y),
                faces: self.face_ways(
                    c,
                    (east, north),
                    FaceSet::new(3, a + key.z * TAU, 30 + 3 * i as i32),
                ),
                key: key.z,
                top_near: true,
                ridge_near: true,
                spur: false,
            }
        });
        // A spur down each arête left without a neighbour, so the horn steps down it rather
        // than falling off its cap into the cirque below.
        let spurs = (many..count).map(|k| {
            let i = (first + k) % count;
            let key = self.leans(c, 40 + i as i32);
            let a = turn + TAU * (i as f32 + 0.5) / count as f32 + SPUR_STRAY * (2.0 * key.z - 1.0);
            Neighbour {
                way: east * libm::cosf(a) + north * libm::sinf(a),
                out: reach * (SPUR_OUT.0 + (SPUR_OUT.1 - SPUR_OUT.0) * key.x),
                top: tall * (SPUR_END.0 + (SPUR_END.1 - SPUR_END.0) * key.y),
                faces: [(east, 1.0); 4],
                key: key.z,
                top_near: false,
                ridge_near: true,
                spur: true,
            }
        });
        neighbours.chain(spurs).collect()
    }

    /// How much a site's horn stands, 0 to 1, and the ground it stands on, in metres: as
    /// `Range::horns` raises a horn at a point, by how near the ridges are to their crests
    /// there and how far it is in the range, taken at the site itself on the planet's sphere.
    /// Remembered by each thread, since every cell round the site asks it.
    fn site_stands(&self, cell: IVec3, site: Vec3, radius: f32) -> (f32, f32) {
        let parts = [cell.x, cell.y, cell.z].map(|v| u64::from(v as u32));
        let tag = key(
            self.key,
            parts.into_iter().chain([u64::from(radius.to_bits())]),
        ) | 1;
        let slot = (tag % SITE_SLOTS as u64) as usize;
        let known = SITES.with(|sites| {
            let (held, value) = sites.borrow()[slot];
            (held == tag).then_some(value)
        });
        if let Some(value) = known {
            return value;
        }
        let at = site.normalize_or(Vec3::Y) * radius;
        let (inside, _) = self.inside_and_front(at);
        let along = (self.ridges(at / self.mountains.ridges_m) - VALLEY) / (CREST - VALLEY);
        let stands = smoothstep(HORNS_FROM.0, HORNS_FROM.1, along) * smoothstep(0.0, 0.25, inside);
        let value = (stands, self.ridged_upper_and_foot_with(at, false).0);
        SITES.with(|sites| sites.borrow_mut()[slot] = (tag, value));
        value
    }
}

impl Massif {
    /// How far its neighbours and the ridges out to them stand at a point `flat` off its site,
    /// laid level, over the ground the horns leave there, `horns` metres over the range's own
    /// `ground`, in metres.
    fn lift(&self, flat: Vec3, ground: f32, horns: f32) -> (f32, f32) {
        // A shape `rise` metres over the horn's ground and standing `share` of it, over the
        // horns' ground there: the share of the way from that ground up to its height, and
        // how far it stands as a wall, the share where it stands out of that ground.
        // How far a shape `rise` metres over the horn's ground stands over the horns' ground
        // there, once its top is eased under its most; none when its foot is wholly under it.
        // A neighbour or the ridge out to it, standing on its own over ground far under the
        // horn's, rises over that ground in proportion, so it keeps its peaked profile where
        // the cap would flatten it into a block; a spur, the horn's own arête, keeps the
        // horn's ground (its crest stays the arête's, unnotched).
        let over = |rise: f32, own: bool| {
            let under = self.base - ground;
            let over = if under > 0.0 && own {
                let deep = smoothstep(OVER_GROUND_FROM.0, OVER_GROUND_FROM.1, under / self.tall);
                let own = rise * (1.0 + OVER_GROUND * under / self.tall);
                rise + under + (own - rise - under) * deep
            } else {
                rise + under
            };
            let height = smooth_min(over, self.tall * LIFT_MOST, EASE_M);
            Some(height - horns).filter(|&over| over > -EASE_M)
        };
        let stand = |over: f32, share: f32| {
            (
                share * smooth_max(over, 0.0, EASE_M),
                share * smoothstep(0.0, 2.0 * EASE_M, over),
            )
        };
        let mut most = (0.0, 0.0);
        for n in &self.tops {
            // The neighbour: a lesser horn of three faces of its own.
            let reach = self.reach * TOP_REACH;
            let off_top = flat - n.way * n.out;
            let from_top = off_top.length();
            if n.top_near
                && from_top < reach
                && let Some(o) = over(n.top, !n.spur)
            {
                let (d, second, _) = faces_from(&n.faces[..3], off_top, reach);
                let d = towered(d, second, from_top, reach, n.key);
                if d < reach {
                    let fade = 1.0 - smoothstep(TOP_FADE, 1.0, from_top / reach);
                    let share = libm::powf(1.0 - d.max(0.0) / reach, self.profile) * fade;
                    most = higher(most, stand(o, share));
                }
            }
            // The ridge out to it, its crest falling from the horn's arête to a col under it.
            if !n.ridge_near {
                continue;
            }
            let from = self.reach * RIDGE_FROM;
            let t = ((flat.dot(n.way) - from) / (n.out - from)).clamp(0.0, 1.0);
            let off = (flat - n.way * (from + t * (n.out - from))).length();
            let wide = self.reach * RIDGE_WIDE;
            let start = self.tall * if n.spur { SPUR_HIGH } else { RIDGE_HIGH.0 };
            let end = if n.spur { n.top } else { n.top * RIDGE_HIGH.1 };
            let crest = (start + (end - start) * t)
                * teeth(flat.dot(n.way) / (self.reach * TEETH_SPACING), t, n.key);
            if off < wide
                && let Some(o) = over(crest, !n.spur)
            {
                // Its sides eased out at their feet as a horn's are: the power alone bent
                // sharpest there, so the faces, stepped by the slope above the foot, lifted the
                // ground below it into a lip.
                let x = off / wide;
                let side =
                    libm::powf(1.0 - x, RIDGE_PROFILE) * (1.0 - smoothstep(RIDGE_FADE, 1.0, x));
                most = higher(most, stand(o, side));
            }
        }
        most
    }
}

/// How much of a ridge's crest stands `along` its length, counted in teeth spacings, `t` of
/// the way from the horn to its neighbour, by the neighbour's own 0 to 1: sharp tips (where
/// `1 - |cos|` peaks, a corner) over rounded notches, spaced unevenly and cut unlike deep as
/// the horns' towers are (`towered`), faded out at either end, where the horn's arête and the
/// neighbour's faces stand.
fn teeth(along: f32, t: f32, key: f32) -> f32 {
    let ends = smoothstep(0.05, 0.25, t) * (1.0 - smoothstep(0.75, 0.95, t));
    if ends <= 0.0 {
        return 1.0;
    }
    let phase = along + key * 7.0 + 0.9 / TAU * libm::sinf(TAU * along / 2.7 + key * 13.0);
    let tip = 1.0 - libm::fabsf(libm::cosf(std::f32::consts::PI * phase));
    let depth = 0.35 + 0.65 * (0.5 + 0.5 * libm::cosf(TAU * along / 3.3 + key * 5.0));
    1.0 - TEETH_DEPTH * depth * (1.0 - tip) * ends
}

/// The larger of each of two pairs, part by part.
fn higher(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0.max(b.0), a.1.max(b.1))
}

/// The larger of `a` and `b`, eased over `k` either side of where they meet: Inigo Quilez's
/// quadratic smooth maximum, which leaves no crease.
fn smooth_max(a: f32, b: f32, k: f32) -> f32 {
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.max(b) + h * h * k * 0.25
}

/// The smaller of `a` and `b`, eased the same way.
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    -smooth_max(-a, -b, k)
}

#[cfg(test)]
mod tests {
    use super::super::Mountains;
    use super::*;
    use terra_gen::Seed;

    const RADIUS: f32 = 30_000.0;

    /// The teeth along a ground profile: its highs standing `least_m` or more over the lowest
    /// ground between them and the next higher point either way (their prominence).
    fn teeth_in(profile: &[f32], least_m: f32) -> usize {
        let col = |h: f32, side: &mut dyn Iterator<Item = &f32>| {
            let mut low = h;
            for &p in side {
                if p > h {
                    return low;
                }
                low = low.min(p);
            }
            low
        };
        (1..profile.len() - 1)
            .filter(|&i| {
                let h = profile[i];
                if profile[i - 1] > h || profile[i + 1] > h {
                    return false;
                }
                let left = col(h, &mut profile[..i].iter().rev());
                let right = col(h, &mut profile[i + 1..].iter());
                h - left.max(right) >= least_m
            })
            .count()
    }

    /// The remembered cells round the lake's hero peak (as in `every_massif_has_neighbours`)
    /// and `deep` deep in the range.
    fn lake_and_deep(range: &Range, deep: usize) -> Vec<IVec3> {
        let lake_peak = Vec3::new(-0.488394, 0.796379, -0.356723).normalize();
        let across = range.mountains.horns_across_m;
        let fine = |d: Vec3| (d * RADIUS / (across / FINE as f32)).floor().as_ivec3();
        let mut cells = vec![fine(lake_peak)];
        cells.extend(
            (0..400)
                .map(|i| {
                    let y = 1.0 - 2.0 * (i as f32 + 0.5) / 400.0;
                    let flat = libm::sqrtf(1.0 - y * y);
                    let around = i as f32 * 2.399_963;
                    Vec3::new(flat * libm::cosf(around), y, flat * libm::sinf(around))
                })
                .filter(|&d| range.inside(d * RADIUS) > 0.9)
                .take(deep)
                .map(fine),
        );
        cells
    }

    #[test]
    fn a_neighbour_over_ground_far_under_its_horn_falls_by_its_own_faces_not_a_wall() {
        // Mountains round 37: the lake's right-hand block, a neighbour on the range's falling
        // front over ground about a horn's height under its horn's, stood the horn's whole
        // height over that ground out to the edge of its foot and ended there in a sheer face.
        let range = Range::new(Mountains::START, Seed(1));
        let (mut checked, step) = (0, 10.0);
        for cell in lake_and_deep(&range, 40) {
            for m in range.standing_round(cell, RADIUS) {
                for n in m.tops.iter().filter(|n| !n.spur && n.top_near) {
                    // Out from the neighbour's top along its way to past its foot, over ground
                    // level with the horn's and over ground a horn's height under it.
                    let steepest = |ground: f32| {
                        let lift = |s: f32| m.lift(n.way * (n.out + s), ground, 0.0).0;
                        let (mut most, mut s) = (0.0_f32, 0.0);
                        while s < m.reach * TOP_REACH * 1.2 {
                            most = most.max(lift(s) - lift(s + 5.0 * step));
                            s += step;
                        }
                        most
                    };
                    let (level, under) = (steepest(m.base), steepest(m.base - m.tall));
                    // It may stand as much taller over the low ground as the rule says, and so
                    // fall as much more steeply, and no more.
                    let grows = 1.0 + OVER_GROUND;
                    assert!(
                        under <= level * grows * 1.02 + 1.0,
                        "a neighbour {:.0} m high falls {under:.0} m in 50 m over low ground, {level:.0} m over level ground",
                        n.top
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 10, "{checked} neighbours checked");
    }

    #[test]
    fn neighbours_stand_lower_than_their_horn_on_crests_broken_into_teeth() {
        // A range's lesser tops stand well under its great one, and the ridges between break
        // into teeth, as Orla Perć's gendarmes do: round 28's neighbours at the lake read
        // "pale, smeared and flat-topped". Round the lake's hero peak (as in
        // `every_massif_has_neighbours`) and three cells deep in the range, along every ridge
        // out to a neighbour, the ground itself is walked every 10 m from a fifth to four
        // fifths of the way out, where it stands a third of the horn's height over its ground.
        let range = Range::new(Mountains::START, Seed(1));
        let lake_peak = Vec3::new(-0.488394, 0.796379, -0.356723).normalize();
        let ground = |at: Vec3| range.rise(at.normalize(), RADIUS).height_m;
        let cells = lake_and_deep(&range, 3);
        let (mut walked_m, mut teeth) = (0.0, 0);
        for cell in cells {
            for massif in range.standing_round(cell, RADIUS) {
                let summit = ground(massif.site);
                for n in massif.tops.iter().filter(|n| !n.spur) {
                    let from = massif.reach * RIDGE_FROM;
                    let steps = ((n.out - from) / 10.0) as usize;
                    let standing = massif.base + massif.tall / 3.0;
                    let profile: Vec<f32> = (steps / 5..steps * 4 / 5)
                        .map(|k| ground(massif.site + n.way * (from + 10.0 * k as f32)))
                        .filter(|&h| h > standing)
                        .collect();
                    if profile.len() < 30 {
                        continue;
                    }
                    walked_m += profile.len() as f32 * 10.0;
                    teeth += teeth_in(&profile, 25.0);
                    // The lake hero's tops under its own, by a share of its height.
                    let top = ground(massif.site + n.way * n.out);
                    if massif.site.normalize().distance(lake_peak) * RADIUS < 200.0 {
                        let share = (top - massif.base) / (summit - massif.base);
                        assert!(
                            share <= 0.85,
                            "a neighbour stands {share:.2} of its horn's height"
                        );
                    }
                }
            }
        }
        let per_km = teeth as f32 / (walked_m / 1000.0);
        assert!(walked_m >= 3000.0, "only {walked_m} m of ridge walked");
        assert!(
            per_km >= 2.0,
            "{teeth} teeth over {walked_m} m of ridge, {per_km:.1} a km: crests too smooth"
        );
    }

    #[test]
    fn a_horn_steps_down_its_spurs_without_a_cliff_or_a_notch() {
        // An arête with no neighbour out along it steps down a spur to its foot, as the
        // Hörnli ridge does, rather than falling off the horn's cap in one cliff into the
        // cirque under it, a notch, and climbing again to the next top (the lake's peak,
        // mountains round 31: its skyline dropped 106 px over 50 px into a V; along that arête
        // the ground fell 1.1 of the horn's height in 330 m, to 0.45 of it under its foot).
        // Round the lake's peak and twelve cells deep, along every spur of a horn standing at
        // its site, the ground is walked every 10 m from the site to the spur's end.
        let range = Range::new(Mountains::START, Seed(1));
        let ground = |at: Vec3| range.rise(at.normalize(), RADIUS).height_m;
        let (mut spurs, mut steepest, mut deepest) = (0, 0.0f32, 0.0f32);
        for cell in lake_and_deep(&range, 12) {
            for massif in range.standing_round(cell, RADIUS) {
                let summit = ground(massif.site) - massif.base;
                if massif.tall < 0.5 * range.mountains.horns_m || summit < 0.8 * massif.tall {
                    continue;
                }
                for n in massif.tops.iter().filter(|n| n.spur) {
                    let profile: Vec<f32> = (0..=(n.out / 10.0) as usize)
                        .map(|k| ground(massif.site + n.way * (10.0 * k as f32)) - massif.base)
                        .map(|h| h / massif.tall)
                        .collect();
                    // The most it falls over 300 m, and the deepest dip under the ground both
                    // before and past it, each as a share of the horn's height.
                    let fall = (0..profile.len() - 30)
                        .map(|k| profile[k] - profile[k + 30])
                        .fold(0.0, f32::max);
                    let dip = (1..profile.len() - 1)
                        .map(|k| {
                            let before = profile[..k].iter().fold(f32::MIN, |a, &b| a.max(b));
                            let past = profile[k + 1..].iter().fold(f32::MIN, |a, &b| a.max(b));
                            before.min(past) - profile[k]
                        })
                        .fold(0.0, f32::max);
                    steepest = steepest.max(fall);
                    deepest = deepest.max(dip);
                    spurs += 1;
                }
            }
        }
        assert!(spurs >= 4, "only {spurs} spurs walked");
        assert!(
            steepest <= 0.7,
            "a spur falls {steepest:.2} of its horn's height in 300 m: a cliff"
        );
        assert!(
            deepest <= 0.15,
            "a spur dips {deepest:.2} of its horn's height into a notch"
        );
    }
}
