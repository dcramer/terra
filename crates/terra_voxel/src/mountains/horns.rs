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
    Range,
};
use glam::{IVec3, Vec3};
use std::f32::consts::TAU;
use terra_gen::smoothstep;

/// How far apart a face's ribs stand, as a share of the horn's reach.
const RIB_SPACING: f32 = 0.12;
/// How far a rib stands out of the couloirs beside it, as a share of the reach of face
/// distance, half up and half down: the ground's relief is about this times the horn's
/// height.
const RIB_DEPTH: f32 = 0.05;
/// How far apart an arête's towers stand, as a share of the reach.
const TOWER_SPACING: f32 = 0.09;
/// How deep the notches between them are cut, as a share of the reach of face distance.
const NOTCH_DEPTH: f32 = 0.03;
/// How close two faces' distances are where an arête's towers stand, as a share of the reach.
const ARETE: f32 = 0.08;

/// The face distance `d` with the arête's towers laid in: `second` the next face's distance
/// at the point (equal to `d` on an arête), `out` how far from the horn's point, level, and
/// `key` the horn's own 0 to 1, so each horn's towers fall their own way.
fn towered(d: f32, second: f32, out: f32, reach: f32, key: f32) -> f32 {
    let x = (d / reach).clamp(0.0, 1.0);
    let on_arete = 1.0 - smoothstep(0.0, reach * ARETE, d - second);
    // The towers stand at spacings of their own, about 1 to 3, and their notches cut 1 to 4
    // deep, a few to cols: evenly spaced, an arête read as a castle's battlements (the r9
    // review), where the Odle's gendarmes stand in a broken row.
    let along = out / (reach * TOWER_SPACING);
    let phase = along + key * 5.0 + 1.2 / TAU * libm::sinf(TAU * along / 2.3 + key * 11.0);
    let notch = 0.5 - 0.5 * libm::cosf(TAU * phase);
    let depth = 0.25 + 0.75 * (0.5 + 0.5 * libm::cosf(TAU * along / 3.1 + key * 3.0)).powi(2);
    let towers = smoothstep(0.02, 0.1, x) * (1.0 - smoothstep(0.4, 0.7, x));
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
    pub(super) fn horns(&self, point: Vec3, along: f32, inside: f32) -> (f32, f32) {
        let m = &self.mountains;
        let crest = smoothstep(HORNS_FROM.0, HORNS_FROM.1, along) * smoothstep(0.0, 0.25, inside);
        if crest <= 0.0 {
            return (0.0, 0.0);
        }
        let kept = if self.kept.is_empty() {
            1.0
        } else {
            self.away_from_kept(point, KEPT_M)
        };
        if kept <= 0.0 {
            return (0.0, 0.0);
        }
        let across = m.horns_across_m;
        let reach = across * HORN_REACH;
        let up = point.normalize_or(Vec3::Y);
        let cell = (point / across).floor().as_ivec3();
        let mut highest: f32 = 0.0;
        // The highest with the faces' buttress ribs (`ribbed`).
        let mut with_ribs: f32 = 0.0;
        // How much of a horn stands over the point, 0 between them to 1 at a point.
        let mut shape: f32 = 0.0;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let c = cell + IVec3::new(dx, dy, dz);
                    let jitter = self.tops.point_in(c.x, c.y, c.z);
                    let site =
                        (c.as_vec3() + Vec3::splat(0.5) + (jitter - 0.5) * HORN_JITTER) * across;
                    let offset = point - site;
                    // Only sites near the ground: the lattice is laid through space.
                    if offset.dot(up).abs() > across * 0.5 {
                        continue;
                    }
                    let flat = offset - up * offset.dot(up);
                    let out = flat.length();
                    if out >= reach {
                        continue;
                    }
                    let pick = self.faces.point_in(c.x, c.y, c.z);
                    if pick.x > HORNS_KEPT {
                        continue;
                    }
                    // The cirques' ways round the site, three or four, turned at random.
                    let site_up = site.normalize_or(Vec3::Y);
                    let east = site_up.cross(Vec3::Y).normalize_or(Vec3::X);
                    let north = site_up.cross(east);
                    let count = if pick.y < 0.5 { 3 } else { 4 };
                    let turn = pick.z * std::f32::consts::TAU;
                    let (mut d, mut second, mut way_at) = (f32::MIN, f32::MIN, east);
                    for i in 0..count {
                        let a = turn + std::f32::consts::TAU * i as f32 / count as f32;
                        let way = east * libm::cosf(a) + north * libm::sinf(a);
                        let along_way = flat.dot(way);
                        let side = (flat - way * along_way).length_squared();
                        let e = along_way - HORN_BOWL * side / reach;
                        if e > d {
                            (second, d, way_at) = (d, e, way);
                        } else {
                            second = second.max(e);
                        }
                    }
                    let across_face = flat.dot(site_up.cross(way_at));
                    let d = towered(d, second, out, reach, pick.y);
                    let fade = 1.0 - smoothstep(0.55, 1.0, out / reach);
                    let height = m.horns_m * (0.7 + 0.3 * jitter.y);
                    let stand =
                        |d: f32| libm::powf(1.0 - (d / reach).clamp(0.0, 1.0), HORN_PROFILE) * fade;
                    let standing = stand(d);
                    highest = highest.max(height * standing);
                    let rib_d = ribbed(d, second, across_face, reach, pick.y);
                    with_ribs = with_ribs.max(height * stand(rib_d));
                    shape = shape.max(standing);
                }
            }
        }
        // The ground the horns leave between them is cut down into cirques, so the massif's
        // upper slopes are eaten back into bowls under arêtes and cols rather than standing
        // as a dome the horns sit on (the r6 review).
        let at = crest * kept;
        (
            (highest - CIRQUES * m.horns_m * (1.0 - shape)) * at,
            (with_ribs - highest) * at,
        )
    }
}
