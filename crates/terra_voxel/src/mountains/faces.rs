//! A mountain wall's rock faces (D426): its beds stepped in the height by range kind.

use super::Range;
use glam::Vec3;
use terra_gen::smoothstep;

/// A kind of rock face (research/2026-10-19-mountain-look.md, plan step 2): where a wall's
/// slope passes its cliff angle, the ground stands up in risers over ledges, bed by bed of its
/// rock, as Gaea's Stratify and World Machine's Terrace nodes step a heightfield, applied on the
/// faces alone. The beds are warped and dip, so a ledge climbs and falls along a wall. Every
/// ledge still falls outward a share of the wall's own fall, so it drains over its edge and
/// holds no pond (the terraces of round one did).
///
/// What tells kinds apart, seen across a valley: how thick the beds are (a few great slabs or
/// many thin bands), how much of each is a sheer riser, how hard the wall is stepped, and how
/// steeply the beds dip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Faces {
    pub name: &'static str,
    /// How often a range takes it, against the theme's other kinds.
    pub weight: f32,
    /// The slope, in degrees, from which a wall stands up in faces whole (`FACE_FROM_DEG`).
    pub cliff_deg: f32,
    /// How thick its rock's beds are on average, in metres: a riser and the ledge under it.
    pub beds_m: f32,
    /// The share of a bed its riser takes on average, the rest its ledge.
    pub riser: f32,
    /// How far a wall is stepped, 0 to 1: below 1 each ledge keeps that share less of the
    /// wall's fall, so it still drains outward.
    pub stepped: f32,
    /// How steeply its beds dip, a metre in a metre, their way turning over kilometres.
    pub dip: f32,
}

impl Faces {
    /// Granite horns and arêtes, as the High Tatras' (D425): a few great slabby beds, their
    /// risers sheer and broad, nearly level.
    pub const HORNS: Faces = Faces {
        name: "horns",
        weight: 1.0,
        cliff_deg: 40.0,
        beds_m: 75.0,
        riser: 0.5,
        stepped: 0.6,
        dip: 0.05,
    };
    /// Saw ridges of thin tilted beds, as the Witcher 3's Skellige and the Valais' schist (D425):
    /// many bands, each a short riser over a ledge, steeply dipping, crumbling.
    pub const SAW: Faces = Faces {
        name: "saw",
        weight: 1.0,
        cliff_deg: 36.0,
        beds_m: 22.0,
        riser: 0.4,
        stepped: 0.7,
        dip: 0.09,
    };
}

/// How far across the patches are that take one kind of face (`Mountains::faces`), as a share
/// of `range_m`, and how far either side of a kind's edge, on the noise, two are blended.
const KINDS_ACROSS: f32 = 0.6;
const KINDS_BLEND: f32 = 0.06;
/// How a riser climbs its bed: steepening to its lip, so it never stands above the ground's
/// own climb and the faces only cut down.
const RISER_POWER: f32 = 1.6;
/// A face fades in from a slope of so many degrees, whole at its kind's cliff angle, and up a
/// wall's height (`upper`), never across it: fading across a ledge lifts its outer part by the
/// depth the face cut, and a face fading fast with the slope lifted ledges into lips that held
/// ponds.
const FACE_FROM_DEG: f32 = 12.0;
/// The beds' warp, in metres of height, over each distance, and how far across their dip
/// turns, all in metres (`Faces`): the first two fold the beds over kilometres and hundreds of
/// metres, the last wavers them. Gentle, its fall at most a few tenths of a metre a metre, so
/// no ledge's outward fall (a third of a wall's) is undone by it.
const BEDS_WARP_M: [f32; 3] = [120.0, 45.0, 6.0];
const BEDS_WARP_ACROSS_M: [f32; 3] = [2000.0, 800.0, 330.0];
const DIP_TURNS_M: f32 = 4500.0;
/// About the most the beds' warp tilts them, a metre in a metre: each fold's height over its
/// width, by the noise's steepest climb, about three (a ledge climbed 0.42 m in a metre).
const BEDS_TILT: f32 = 0.45;
/// How far each bed's top strays from where even beds would put it, as a share of a bed: so
/// beds run from a third of the kind's thickness to five thirds, thin bands between massive
/// ones, as a real wall's strata do; even, the wall read as a terraced hill's contours.
const BED_STRAY: f32 = 0.7;
/// How far each bed's riser share strays from its kind's, as a share of it, either way.
const RISER_STRAY: f32 = 0.35;
/// The gullies' lines down a wall (the coarsest set's) cut its faces: the faces stand whole on
/// the ribs and fade out down a gully's line, from the first share of the way into it to the
/// second, so a wall's beds stand in buttresses between smooth couloirs, not as one band
/// along the whole wall. Fading across the fall, it never lifts a ledge's outer edge.
pub(super) const RIB_FADE: (f32, f32) = (0.45, 0.95);
/// Within so many metres of a kept spot no face is stepped, whole past the second.
const FACES_KEPT_M: (f32, f32) = (700.0, 1500.0);

impl Range {
    /// How much a wall's faces (`Mountains::faces`) raise or lower ground of this height and
    /// fall, in metres: 0 under the kinds' cliff angles, round the kept spots and down the
    /// gullies' lines (`rib` 0, whole 1 on the ribs between).
    pub(super) fn stepped(
        &self,
        point: Vec3,
        height_m: f32,
        slope: Vec3,
        (upper, rib): (f32, f32),
    ) -> f32 {
        let steepness = slope.length();
        let kinds = self.mountains.faces;
        if kinds.is_empty() || upper <= 0.0 || rib <= 0.0 || steepness < 0.2 {
            return 0.0;
        }
        let away = self.away_from_kept(point, FACES_KEPT_M);
        if away <= 0.0 {
            return 0.0;
        }
        // Which kind: the kinds' weights laid end to end over a noise patches some kilometres
        // across, each its share of it, blended only near the edges between.
        let pick = self
            .strata
            .fbm(point / (self.mountains.range_m * KINDS_ACROSS), 2, 2.0, 0.5);
        let pick = smoothstep(-0.35, 0.35, pick);
        let total: f32 = kinds.iter().map(|k| k.weight).sum();
        let mut from = 0.0;
        let mut below = 1.0;
        let mut sum = 0.0;
        for (i, kind) in kinds.iter().enumerate() {
            from += kind.weight / total;
            let above = if i + 1 == kinds.len() {
                0.0
            } else {
                smoothstep(from - KINDS_BLEND, from + KINDS_BLEND, pick)
            };
            let share = below - above;
            below = above;
            if share > 1e-4 {
                sum += share * self.stepped_by(kind, point, height_m, slope);
            }
        }
        sum * away * upper * rib
    }

    /// [`Range::stepped`] for one kind of face.
    fn stepped_by(&self, kind: &Faces, point: Vec3, height_m: f32, slope: Vec3) -> f32 {
        let steepness = slope.length();
        let face = smoothstep(
            libm::tanf(FACE_FROM_DEG.to_radians()),
            libm::tanf(kind.cliff_deg.to_radians()),
            steepness,
        );
        if face <= 0.0 || kind.stepped <= 0.0 || kind.beds_m <= 0.0 {
            return 0.0;
        }
        let s = &self.strata;
        let mut warp = 0.0;
        for (i, (m, across)) in BEDS_WARP_M.iter().zip(BEDS_WARP_ACROSS_M).enumerate() {
            warp += m * s.sample(point / across + Vec3::splat(3.1 + 4.6 * i as f32));
        }
        let turn = s.sample(point / DIP_TURNS_M + Vec3::splat(11.3)) * std::f32::consts::TAU;
        // The beds dip as a plane: `point` lies along the up, so its dot with a level direction
        // climbs across the ground as that direction's share in the ground's plane. Taking the
        // fall's share out of it did nothing but add the point's tiny lean off the plane times
        // the planet's radius, which jumped where the fall turned at a crease (a 0.3 m step).
        // A kind's dip is kept under the share of a wall's fall its ledges keep, so they drain.
        let dip = Vec3::new(libm::cosf(turn), 0.0, libm::sinf(turn)) * kind.dip;
        let level = (height_m + warp + point.dot(dip)) / kind.beds_m;
        // Which bed, its tops strayed each by its own hash, so the beds run thick and thin.
        let top = |k: i32| k as f32 + BED_STRAY * (s.point_in(k, 41, 0).x - 0.5);
        let n = libm::floorf(level) as i32;
        let k = if level < top(n) {
            n - 1
        } else if level >= top(n + 1) {
            n + 1
        } else {
            n
        };
        let (from, to) = (top(k), top(k + 1));
        let thick = to - from;
        let within = ((level - from) / thick).clamp(0.0, 1.0);
        let share = kind.riser * (1.0 + RISER_STRAY * (2.0 * s.point_in(k, 43, 0).y - 1.0));
        let share = share.clamp(0.1, 0.9);
        // A ledge, then its riser climbing the whole bed: the ground's own climb kept by the
        // share left, so the ledge still falls outward. Cut by the bed's own thickness, the
        // ledge's fall is the same in a thin bed as in a thick one, and the ground meets
        // itself at every bed's top.
        let riser = libm::powf(((within - (1.0 - share)) / share).max(0.0), RISER_POWER);
        // The beds' own tilt, their warp and dip, climbs across a ledge too: on the gentler
        // walls a ledge keeps only so much of the fall that it still drains over its edge
        // with the beds tilted back against it as far as they ever are.
        let tilt = BEDS_TILT + kind.dip;
        let stepped = kind.stepped.min(steepness / (steepness + tilt));
        face * stepped * kind.beds_m * thick * (riser - within)
    }
}
