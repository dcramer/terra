//! A range's horns (`Mountains::horns_m`): pyramids of cirque faces on the crests, their
//! faces broken into buttress ribs and their arêtes into towers (D426).
//!
//!
//! The faces of a horn as `Range::horns` lays them are smooth bowls meeting in clean arêtes,
//! so from across a valley the horns read as cones (the r8 review). A real horn's faces are
//! ribbed: buttresses running down the fall line with couloirs between them, and its arêtes
//! serrated into towers and notches, as the Matterhorn's Hörnli and the Tatras' ridges are.
//! Both are laid in the horn's own face distance `d` (out along the face's way, 0 at the
//! point to `reach` at its foot): taking from it raises the ground, adding to it lowers.

use super::{
    CIRQUES, HORN_BOWL, HORN_JITTER, HORN_PROFILE, HORN_REACH, HORNS_FROM, HORNS_KEPT, KEPT_M,
    NEIGHBOURS_KEPT_M, Range,
};
use glam::{IVec3, Vec3};
use std::cell::RefCell;
use std::f32::consts::TAU;
use terra_gen::smoothstep;

/// How far apart a face's ribs stand, as a share of the horn's reach.
const RIB_SPACING: f32 = 0.12;
/// How far a rib stands out of the couloirs beside it, as a share of the reach of face
/// distance, half up and half down: the ground's relief is about this times the horn's
/// height.
const RIB_DEPTH: f32 = 0.05;
/// How far apart an arête's towers stand, as a share of the reach.
const TOWER_SPACING: f32 = 0.15;
/// How deep the notches between them are cut, as a share of the reach of face distance.
const NOTCH_DEPTH: f32 = 0.05;
/// How close two faces' distances are where an arête's towers stand, as a share of the reach.
const ARETE: f32 = 0.08;
/// How far each face's steepness strays from the horn's, either way, and how far its way
/// turns from even spacing, as a share of the spacing: even, every horn stood as a clean
/// pyramid and two side by side as twins (the r20 review), where the Matterhorn's east face is
/// half as long as its north and Krywań leans.
const FACE_LEAN: f32 = 0.35;
const FACE_TURN: f32 = 0.35;
/// How far a horn's height and profile stray: its height from this share of `horns_m` to the
/// whole, its profile's power by this share of `HORN_PROFILE` either way, so some stand as
/// needles and some as broad, squat tops.
pub(super) const HORN_LEAST: f32 = 0.55;
const PROFILE_STRAY: f32 = 0.25;
/// A shoulder peak on one of a horn's arêtes, as Pic Tyndall stands on the Matterhorn's Lion
/// ridge: how far out along it, how far its own faces reach, both as shares
/// of the horn's reach, and the least and most it adds, as shares of the horn's height.
const SHOULDER_OUT: f32 = 0.42;
const SHOULDER_REACH: f32 = 0.34;
const SHOULDER_RISE: (f32, f32) = (0.12, 0.32);

/// The face distance `d` with the arête's towers laid in: `second` the next face's distance
/// at the point (equal to `d` on an arête), `out` how far from the horn's point, level, and
/// `key` the horn's own 0 to 1, so each horn's towers fall their own way.
pub(super) fn towered(d: f32, second: f32, out: f32, reach: f32, key: f32) -> f32 {
    let x = (d / reach).clamp(0.0, 1.0);
    let on_arete = 1.0 - smoothstep(0.0, reach * ARETE, d - second);
    let towers = smoothstep(0.02, 0.1, x) * (1.0 - smoothstep(0.4, 0.7, x));
    // Off the arêtes and their towered stretch, as most of a face is, nothing to lay in.
    if on_arete <= 0.0 || towers <= 0.0 {
        return d;
    }
    // The towers stand at spacings of their own, about 1 to 3, and their notches cut 1 to 4
    // deep, a few to cols: evenly spaced, an arête read as a castle's battlements (the r9
    // review), where the Odle's gendarmes stand in a broken row.
    let along = out / (reach * TOWER_SPACING);
    let phase = along + key * 5.0 + 1.2 / TAU * libm::sinf(TAU * along / 2.3 + key * 11.0);
    let notch = 0.5 - 0.5 * libm::cosf(TAU * phase);
    let depth = 0.25 + 0.75 * (0.5 + 0.5 * libm::cosf(TAU * along / 3.1 + key * 3.0)).powi(2);
    d + NOTCH_DEPTH * reach * depth * notch * notch * on_arete * towers
}

/// The face distance with the face's buttress ribs laid in, `across` how far across the face
/// the point lies: off the arêtes, faded before the point and toward the foot. The ribs are
/// laid over the ground after its gullies and faces (`Range::rise`), which follow the slope
/// without them: stepped by a slope that turned across every rib, the faces' ledges climbed
/// half a metre into lips that held water (`faces_stand_sheer_over_ledges_that_still_drain`).
/// Over a rib, a ledge drains sideways into the couloir beside it.
fn ribbed(d: f32, second: f32, across: f32, reach: f32, key: f32) -> f32 {
    let x = (d / reach).clamp(0.0, 1.0);
    let upper = smoothstep(0.03, 0.15, x) * (1.0 - smoothstep(0.45, 0.75, x));
    let rib = 0.5 + 0.5 * libm::cosf(TAU * (across / (reach * RIB_SPACING) + key * 7.0));
    let on_arete = 1.0 - smoothstep(0.0, reach * ARETE, d - second);
    d - RIB_DEPTH * reach * (rib - 0.5) * upper * (1.0 - on_arete)
}

impl Range {
    /// How much the horns raise the ground at a point, or the cirques between them lower it
    /// (`Mountains::horns_m`), in metres.
    ///
    /// A horn is a top three or four cirques have eaten back into from its sides, as the
    /// Matterhorn's and the Tatras' Mnich and Krywań are: each cirque's headwall a face
    /// steepest just under the point and easing out to its floor, the faces meeting in sharp
    /// arêtes. Laid here as a pyramid on the crest, one face for each cirque: the distance out
    /// along the face's way, less a share of the distance across it squared, so each face is
    /// a bowl hollowed into the mountain and the arêtes between stand proud of it; a concave
    /// profile, `(1 - d)^HORN_PROFILE`, down from the point. Sites on a jittered lattice
    /// `horns_across_m` apart, some left out, only where the ridges already stand near their
    /// crests, so a horn rises from a crest and never from a valley's side. No permissive
    /// implementation was found to port (D196): Gaea's, World Machine's and Houdini's are
    /// closed; this follows the landform as glaciology describes it.
    ///
    /// The horns' height over the range's `ground` at a point, in metres, their ribs, and how
    /// far their massifs' neighbours stand there as walls, 0 to 1.
    pub(super) fn horns(
        &self,
        point: Vec3,
        along: f32,
        inside: f32,
        ground: f32,
    ) -> (f32, f32, f32) {
        // The neighbours round a horn (`massif`) stand wherever the range does, past its
        // crests; the horn itself only where the ridges are near their crests.
        let edge = smoothstep(0.0, 0.25, inside);
        if edge <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let kept = if self.kept.is_empty() {
            1.0
        } else {
            self.away_from_kept(point, KEPT_M)
        };
        if kept <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let crest = smoothstep(HORNS_FROM.0, HORNS_FROM.1, along) * edge;
        let (horns, ribs) = if crest > 0.0 {
            self.horn_tops(point, crest * kept)
        } else {
            (0.0, 0.0)
        };
        // The neighbours and the ridges out to them rise out of the ground the horns and their
        // cirques leave, easing into it at their feet, so no crease is left where they meet.
        // Round a spot kept on a valley floor out of the ranges, a town's, they stand only
        // far off, so its skyline is the range's own (`NEIGHBOURS_KEPT_M`).
        let kept = if self.kept.is_empty() {
            kept
        } else {
            kept.min(self.away_from_kept_out_of_ranges(point, NEIGHBOURS_KEPT_M))
        };
        if kept <= 0.0 {
            return (horns, ribs, 0.0);
        }
        let (lift, walled) = self.neighbours(point, ground, horns);
        let by = kept * edge;
        (horns + lift * by, ribs, walled * by)
    }

    /// The horns at a point and their ribs, in metres, by `at`, how near the ridges are to
    /// their crests there and how far from the kept spots: [`Range::horns`] without the
    /// massifs' neighbours.
    fn horn_tops(&self, point: Vec3, at: f32) -> (f32, f32) {
        let m = &self.mountains;
        let across = m.horns_across_m;
        let reach = across * HORN_REACH;
        let up = point.normalize_or(Vec3::Y);
        let cell = (point / across).floor().as_ivec3();
        let mut highest: f32 = 0.0;
        // The highest with the faces' buttress ribs (`ribbed`).
        let mut with_ribs: f32 = 0.0;
        // How much of a horn stands over the point, 0 between them to 1 at a point.
        let mut shape: f32 = 0.0;
        AROUND.with_borrow_mut(|around| {
            for site in around.round(self, cell) {
                let offset = point - site.site;
                // Only sites near the ground: the lattice is laid through space.
                if offset.dot(up).abs() > across * 0.5 {
                    continue;
                }
                let flat = offset - up * offset.dot(up);
                let out = flat.length();
                if out >= reach {
                    continue;
                }
                let Some(horn) = site.horn(self) else {
                    continue;
                };
                let (d, second, way_at) =
                    faces_from(&horn.ways[..horn.count as usize], flat, reach);
                let across_face = flat.dot(horn.site_up.cross(way_at));
                let d = towered(d, second, out, reach, horn.key);
                let fade = 1.0 - smoothstep(0.55, 1.0, out / reach);
                let stand = |d: f32, reach: f32| {
                    libm::powf(1.0 - (d / reach).clamp(0.0, 1.0), horn.profile) * fade
                };
                // The shoulder (`Horn::arete`): a small horn of its own faces added onto the
                // arête's fall, so the arête rises to it and dips to a col behind it.
                let s_reach = reach * SHOULDER_REACH;
                let s_flat = flat - horn.arete * (reach * SHOULDER_OUT);
                let s_out = s_flat.length();
                let s_stand = if s_out < s_reach {
                    let (s_d, s_second, _) = faces_from(&horn.shoulder_ways[..3], s_flat, s_reach);
                    let s_d = towered(s_d, s_second, s_out, s_reach, horn.shoulder_key);
                    stand(s_d, s_reach) * (1.0 - smoothstep(0.55, 1.0, s_out / s_reach))
                } else {
                    0.0
                };
                let rise = horn.shoulder_rise;
                let shouldered = horn.height * rise * s_stand;
                let standing = (stand(d, reach) + rise * s_stand).min(1.0);
                highest = highest.max(horn.height * stand(d, reach) + shouldered);
                let rib_d = ribbed(d, second, across_face, reach, horn.key);
                with_ribs = with_ribs.max(horn.height * stand(rib_d, reach) + shouldered);
                shape = shape.max(standing);
            }
        });
        // The ground the horns leave between them is cut down into cirques, so the massif's
        // upper slopes are eaten back into bowls under arêtes and cols rather than standing
        // as a dome the horns sit on (the r6 review).
        (
            (highest - CIRQUES * m.horns_m * (1.0 - shape)) * at,
            (with_ribs - highest) * at,
        )
    }

    /// A horn's site in lattice cell `c`, as [`Range::horn_tops`] lays it.
    fn horn_site(&self, c: IVec3) -> HornSite {
        let jitter = self.tops.point_in(c.x, c.y, c.z);
        let across = self.mountains.horns_across_m;
        HornSite {
            c,
            site: (c.as_vec3() + Vec3::splat(0.5) + (jitter - 0.5) * HORN_JITTER) * across,
            tall: jitter.y,
            horn: None,
        }
    }

    /// The horn at a site, laid out once, or none where the site is left out (`HORNS_KEPT`).
    fn horn_at(&self, c: IVec3, site: Vec3, tall: f32) -> Option<Horn> {
        let m = &self.mountains;
        let pick = self.faces.point_in(c.x, c.y, c.z);
        if pick.x > HORNS_KEPT {
            return None;
        }
        // The cirques' ways round the site, three or four, turned at random, each face its own
        // steepness and its way strayed from even (`FACE_LEAN`).
        let (east, north) = horn_ways(site);
        let count = if pick.y < 0.5 { 3 } else { 4 };
        let turn = pick.z * TAU;
        let way_of = |a: f32| east * libm::cosf(a) + north * libm::sinf(a);
        // The shoulder, on the arête after a face chosen by the horn's hash.
        let shoulder = self.leans(c, -1);
        let arete = turn + TAU * ((count as f32 * shoulder.x).floor() + 0.5) / count as f32;
        let s_turn = turn + shoulder.y * TAU;
        Some(Horn {
            site_up: site.normalize_or(Vec3::Y),
            count,
            ways: self.face_ways(c, (east, north), FaceSet::new(count, turn, 0)),
            key: pick.y,
            height: m.horns_m * (HORN_LEAST + (1.0 - HORN_LEAST) * tall),
            profile: horn_profile(pick.x),
            arete: way_of(arete),
            shoulder_ways: self.face_ways(c, (east, north), FaceSet::new(3, s_turn, 7)),
            shoulder_key: shoulder.z,
            shoulder_rise: SHOULDER_RISE.0 + (SHOULDER_RISE.1 - SHOULDER_RISE.0) * shoulder.z,
        })
    }

    /// A horn's hash for one of its faces or parts (`i`), each 0 to 1.
    pub(super) fn leans(&self, c: IVec3, i: i32) -> Vec3 {
        self.faces.point_in(c.x + 101 * (i + 1), c.y, c.z)
    }

    /// The ways and steepness of a horn's `set` of cirque faces (at most four), laid level in
    /// its `(east, north)`, each its own steepness and its way strayed from even by the horn's
    /// hash, as [`faces_from`] takes them.
    pub(super) fn face_ways(
        &self,
        c: IVec3,
        (east, north): (Vec3, Vec3),
        FaceSet { count, turn, salt }: FaceSet,
    ) -> [(Vec3, f32); 4] {
        let mut ways = [(east, 1.0); 4];
        for (i, slot) in ways.iter_mut().enumerate().take(count as usize) {
            let i = i as i32;
            let lean = self.leans(c, i + salt);
            let a = turn + TAU * (i as f32 + FACE_TURN * (lean.x - 0.5)) / count as f32;
            let way = east * libm::cosf(a) + north * libm::sinf(a);
            *slot = (way, 1.0 + FACE_LEAN * (2.0 * lean.y - 1.0));
        }
        ways
    }
}

/// A horn's set of cirque faces: how many (at most four), the way they are turned from, in
/// radians, and which of the horn's hashes lean them (`Range::leans`).
#[derive(Clone, Copy)]
pub(super) struct FaceSet {
    count: i32,
    turn: f32,
    salt: i32,
}

impl FaceSet {
    pub(super) fn new(count: i32, turn: f32, salt: i32) -> Self {
        Self { count, turn, salt }
    }
}

/// How many neighbourhoods of the horns' lattice each thread remembers (`AROUND`): a rise's
/// slope asks points `SLOPE_STEP_M` either way of its own, in at most two cells each way.
const AROUND_SLOTS: usize = 8;

thread_local! {
    /// The horns' sites round the lattice cells last asked, by the cell and the range's key:
    /// what a site holds depends only on its cell, and the ground's samples, metres apart, and
    /// each rise's slope, asking seven points 20 m apart, ask the same cells many times over.
    static AROUND: RefCell<Around> = const { RefCell::new(Around { held: Vec::new(), next: 0 }) };
}

/// The neighbourhoods a thread remembers, each the 27 sites round a cell, in the order
/// [`Range::horn_tops`] looks at them, and which is replaced next.
struct Around {
    held: Vec<((IVec3, u64), [HornSite; 27])>,
    next: usize,
}

impl Around {
    /// The 27 sites round `cell` of a range, worked out the first time they are asked.
    fn round(&mut self, range: &Range, cell: IVec3) -> &mut [HornSite; 27] {
        let tag = (cell, range.key);
        let slot = match self.held.iter().position(|(held, _)| *held == tag) {
            Some(slot) => slot,
            None => {
                let mut sites = [HornSite::default(); 27];
                let mut k = 0;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            sites[k] = range.horn_site(cell + IVec3::new(dx, dy, dz));
                            k += 1;
                        }
                    }
                }
                if self.held.len() < AROUND_SLOTS {
                    self.held.push((tag, sites));
                    self.held.len() - 1
                } else {
                    let slot = self.next;
                    self.next = (self.next + 1) % AROUND_SLOTS;
                    self.held[slot] = (tag, sites);
                    slot
                }
            }
        };
        &mut self.held[slot].1
    }
}

/// A lattice cell's horn site: the cell, where the site is, its hash for how tall a horn it
/// holds, and the horn once asked for (`Range::horn_at`), `Some(None)` where it is left out.
#[derive(Clone, Copy, Default)]
struct HornSite {
    c: IVec3,
    site: Vec3,
    tall: f32,
    horn: Option<Option<Horn>>,
}

impl HornSite {
    fn horn(&mut self, range: &Range) -> Option<&Horn> {
        let (c, site, tall) = (self.c, self.site, self.tall);
        self.horn
            .get_or_insert_with(|| range.horn_at(c, site, tall))
            .as_ref()
    }
}

/// A horn laid out at its site, as each point it reaches takes it: the site's up, its faces'
/// count, ways and steepness, its hash for its towers and ribs, how high it stands and its
/// profile's power, and its shoulder's way out along its arête, faces, hash and rise.
#[derive(Clone, Copy)]
struct Horn {
    site_up: Vec3,
    count: i32,
    ways: [(Vec3, f32); 4],
    key: f32,
    height: f32,
    profile: f32,
    arete: Vec3,
    shoulder_ways: [(Vec3, f32); 4],
    shoulder_key: f32,
    shoulder_rise: f32,
}

/// The face distance at a point `flat` off a horn's site of its faces' `ways` and steepness
/// ([`Range::face_ways`]), each face a bowl (`HORN_BOWL`) hollowed into the mountain: the
/// largest face distance, the next, and the way of the face it is on.
pub(super) fn faces_from(ways: &[(Vec3, f32)], flat: Vec3, reach: f32) -> (f32, f32, Vec3) {
    let (mut d, mut second, mut way_at) = (f32::MIN, f32::MIN, ways[0].0);
    for &(way, steep) in ways {
        let along_way = flat.dot(way);
        let side = (flat - way * along_way).length_squared();
        let e = (along_way - HORN_BOWL * side / reach) * steep;
        if e > d {
            (second, d, way_at) = (d, e, way);
        } else {
            second = second.max(e);
        }
    }
    (d, second, way_at)
}

/// The way east and north, level, at a horn's site.
pub(super) fn horn_ways(site: Vec3) -> (Vec3, Vec3) {
    let site_up = site.normalize_or(Vec3::Y);
    let east = site_up.cross(Vec3::Y).normalize_or(Vec3::X);
    (east, site_up.cross(east))
}

/// The power a horn's profile falls by, by its hash `pick` (0 to `HORNS_KEPT`).
pub(super) fn horn_profile(pick: f32) -> f32 {
    HORN_PROFILE * (1.0 + PROFILE_STRAY * (2.0 * pick / HORNS_KEPT - 1.0))
}
