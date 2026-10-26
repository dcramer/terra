//! A shrub's wood, apart from the trees' (`grown_tree::wood`): a bare hazel or oak scrub in
//! winter is all wood a stride off, so what a tree's crown hides here shows. Each limb is
//! drawn round enough for its size, bent through a smooth curve rather than straight pieces,
//! narrowing to a point rather than ending in a cut stub, and near the camera its outer
//! branches carry fine twigs a few millimetres thick, the mass of shoots a bare shrub reads
//! by, as SpeedTree's shrubs carry fronds of fine twigs on their last branch level.

use bevy::prelude::*;
use terra_gen::hash;
use terra_world::grown::Habit;
use terra_world::grown::build::{Build, Limb, tube};
use terra_world::grown::tree::Tree;

/// Sides round a limb by its depth (stem, branch, twig, fine twig), near and farther off.
const SIDES: [[u32; 4]; 2] = [[10, 7, 5, 3], [7, 4, 3, 3]];

/// The longest straight piece of a limb, near and farther off: a curve longer than this is
/// cut into more pieces along a Catmull-Rom spline through its points.
const PIECE_M: [f32; 2] = [0.15, 0.4];

/// How thick a limb's tip is before its point, by depth: a hazel rod ends in a bud a few
/// millimetres across, not five centimetres of cut stem.
const TIP_R: [f32; 3] = [0.004, 0.003, 0.002];

/// A fine twig's radius at its foot (1 to 2 mm, so 2 to 4 mm thick) and its length.
const FINE_R: (f32, f32) = (0.001, 0.002);
const FINE_M: (f32, f32) = (0.1, 0.3);

/// A shrub's wood at a level of detail (0 near, 1 farther off), with fine twigs near when
/// it stands bare in winter.
pub(super) fn wood(tree: &Tree, detail: usize, fine: bool) -> Build {
    let deepest = match (tree.habit, detail) {
        (Habit::Conical, 0) => 1,
        (Habit::Conical, _) => 0,
        (_, 0) => 2,
        _ => 1,
    };
    let limbs = tapered(tree);
    let mut build = Build::default();
    for limb in limbs.iter().filter(|limb| limb.depth <= deepest) {
        let smooth = pointed(curved(limb, PIECE_M[detail]));
        tube(
            &mut build,
            &smooth,
            SIDES[detail][limb.depth.min(3) as usize],
            tree.height,
            1.0,
            Vec3::ONE,
        );
    }
    if fine && detail == 0 && tree.habit != Habit::Conical {
        for limb in limbs.iter().filter(|limb| matches!(limb.depth, 1 | 2)) {
            for twig in fine_twigs(limb) {
                tube(&mut build, &twig, SIDES[0][3], tree.height, 1.0, Vec3::ONE);
            }
        }
    }
    build
}

/// Lengths along a limb's points, as shares of its whole length.
fn shares(points: &[Vec3]) -> Vec<f32> {
    let mut along = vec![0.0];
    for pair in points.windows(2) {
        along.push(along.last().copied().unwrap_or(0.0) + pair[0].distance(pair[1]));
    }
    let length = along.last().copied().unwrap_or(0.0).max(1e-4);
    along.iter().map(|a| a / length).collect()
}

/// The tree's limbs narrowing as wood does: a stem from above its flared foot to a bud at
/// its tip, and a branch no thicker where it leaves than two thirds of what it leaves from,
/// narrowing to its own tip. The trees' growth holds every stem and branch to at least two
/// or five centimetres across (enough for a tree, not for a hazel rod), so a bare stool's
/// stems and branches ended in cut stubs of one thickness.
fn tapered(tree: &Tree) -> Vec<Limb> {
    let mut limbs: Vec<Limb> = Vec::with_capacity(tree.limbs.len());
    let mut order: Vec<usize> = (0..tree.limbs.len()).collect();
    order.sort_by_key(|&i| tree.limbs[i].depth);
    for i in order {
        let limb = &tree.limbs[i];
        let t = shares(&limb.points);
        let tip = TIP_R[(limb.depth as usize).min(2)];
        let mut radii = limb.radii.clone();
        if limb.depth == 0 {
            // From a sixth of the way up, past the flare, down to its tip.
            let from = t.iter().position(|&s| s >= 0.15).unwrap_or(0);
            let base = radii[from];
            let span = (1.0 - t[from]).max(1e-3);
            for k in from..radii.len() {
                let s = (t[k] - t[from]) / span;
                radii[k] = radii[k].min(tip + (base - tip) * libm::powf(1.0 - s, 1.2));
            }
        } else {
            let parent = limbs
                .iter()
                .filter(|other| other.depth + 1 == limb.depth)
                .flat_map(|other| other.points.iter().zip(&other.radii))
                .min_by(|a, b| {
                    let (da, db) = (a.0.distance(limb.points[0]), b.0.distance(limb.points[0]));
                    da.total_cmp(&db)
                })
                .map_or(radii[0], |(_, &r)| r);
            let base = radii[0].min(parent * 0.65).max(tip);
            for (k, radius) in radii.iter_mut().enumerate() {
                *radius = radius.min(tip + (base - tip) * libm::powf(1.0 - t[k], 1.2));
            }
        }
        limbs.push(Limb {
            points: limb.points.clone(),
            radii,
            depth: limb.depth,
        });
    }
    limbs
}

/// A limb bent through a Catmull-Rom spline through its points, each piece cut into as many
/// as keep it under `piece` metres, so a curved cane or stem bends smoothly rather than in
/// straight lengths with corners between.
fn curved(limb: &Limb, piece: f32) -> Limb {
    let p = &limb.points;
    let n = p.len();
    if n < 2 {
        return Limb {
            points: p.clone(),
            radii: limb.radii.clone(),
            depth: limb.depth,
        };
    }
    let mut points = vec![p[0]];
    let mut radii = vec![limb.radii[0]];
    for i in 0..n - 1 {
        let (p0, p1, p2) = (p[i.saturating_sub(1)], p[i], p[i + 1]);
        let p3 = p[(i + 2).min(n - 1)];
        let cuts = ((p1.distance(p2) / piece).ceil() as usize).clamp(1, 4);
        for c in 1..=cuts {
            let u = c as f32 / cuts as f32;
            let (u2, u3) = (u * u, u * u * u);
            let at = 0.5
                * (2.0 * p1
                    + (p2 - p0) * u
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * u3);
            points.push(at);
            radii.push(limb.radii[i] + (limb.radii[i + 1] - limb.radii[i]) * u);
        }
    }
    Limb {
        points,
        radii,
        depth: limb.depth,
    }
}

/// A limb ending in a point: one more ring of no width a little past its tip, so its end is
/// a cone, as a bud or a twig's tip is, rather than an open cut.
fn pointed(mut limb: Limb) -> Limb {
    let n = limb.points.len();
    if n >= 2 && limb.radii[n - 1] > 0.0 {
        let end = limb.points[n - 1];
        let toward = (end - limb.points[n - 2]).normalize_or(Vec3::Y);
        let r = limb.radii[n - 1];
        limb.points.push(end + toward * (r * 6.0).max(0.01));
        limb.radii.push(0.0);
    }
    limb
}

/// The fine twigs along a branch's or twig's outer part: one or two a piece of it, each a
/// slender shoot 10 to 30 cm long and 2 to 4 mm thick at its foot, angled forward and out
/// from its limb and up toward the light, bending a little along its length to a point.
/// Placed by a hash of the limb's own points, not the shrub's dice, so a kind's variants and
/// everything grown after its wood are the same with them as without.
fn fine_twigs(limb: &Limb) -> Vec<Limb> {
    let t = shares(&limb.points);
    let mut twigs = Vec::new();
    for i in 0..limb.points.len().saturating_sub(1) {
        if t[i + 1] < 0.3 {
            continue;
        }
        let (a, b) = (limb.points[i], limb.points[i + 1]);
        let along = (b - a).normalize_or(Vec3::Y);
        let key = (a * 1000.0).as_ivec3();
        let seed = hash::of(&[key.x as u32, key.y as u32, key.z as u32, limb.depth]);
        let count = 1 + u32::from(limb.depth == 2 && hash::unit(seed) < 0.5);
        for k in 0..count {
            let h = |salt: u32| hash::unit(hash::of(&[seed, k, salt]));
            let u = 0.15 + 0.8 * h(1);
            let foot = a.lerp(b, u);
            let limb_r = limb.radii[i] + (limb.radii[i + 1] - limb.radii[i]) * u;
            let radius = (FINE_R.0 + (FINE_R.1 - FINE_R.0) * h(2)).min(limb_r * 0.8);
            let length = FINE_M.0 + (FINE_M.1 - FINE_M.0) * h(3);
            let turn = h(4) * std::f32::consts::TAU;
            let side = along.any_orthonormal_vector();
            let side = Quat::from_axis_angle(along, turn) * side;
            let angle = 0.5 + 0.6 * h(5);
            let out =
                (along * libm::cosf(angle) + side * libm::sinf(angle) + Vec3::Y * 0.3).normalize();
            let bend = (out + Vec3::Y * (0.25 * h(6) - 0.05)).normalize();
            let mid = foot + out * length * 0.5;
            let tip = mid + bend * length * 0.5;
            twigs.push(Limb {
                points: vec![foot, mid, tip],
                radii: vec![radius, radius * 0.6, 0.0],
                depth: 3,
            });
        }
    }
    twigs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(limbs: Vec<Limb>) -> Tree {
        Tree {
            limbs,
            sprays: Vec::new(),
            branches: 0,
            trunk_m: 0.025,
            height: 3.0,
            habit: Habit::Upright,
        }
    }

    fn stem() -> Limb {
        let points: Vec<Vec3> = (0..=14).map(|i| Vec3::Y * i as f32 * 0.2).collect();
        Limb {
            radii: vec![0.025; points.len()],
            points,
            depth: 0,
        }
    }

    fn branch(from: Vec3) -> Limb {
        let points: Vec<Vec3> = (0..=5)
            .map(|i| from + Vec3::new(0.15, 0.1, 0.0) * i as f32)
            .collect();
        Limb {
            radii: vec![0.02; points.len()],
            points,
            depth: 1,
        }
    }

    #[test]
    fn a_stem_narrows_to_a_bud_and_a_branch_is_thinner_than_its_stem() {
        let grown = tree(vec![stem(), branch(Vec3::Y * 2.0)]);
        let limbs = tapered(&grown);
        let stem = &limbs[0];
        assert!(stem.radii[0] >= 0.025, "the foot keeps its thickness");
        assert!(
            *stem.radii.last().unwrap() <= 0.005,
            "the tip is a bud, not a stub"
        );
        assert!(stem.radii.windows(2).skip(2).all(|w| w[1] <= w[0]));
        // Where the branch leaves, the stem is about 1.1 cm; the branch is no thicker than
        // two thirds of that.
        let at = stem.radii[10];
        assert!(
            limbs[1].radii[0] <= at * 0.66,
            "{} vs {at}",
            limbs[1].radii[0]
        );
    }

    #[test]
    fn a_curved_limb_turns_by_small_angles_and_keeps_its_ends() {
        // A cane bent through four straight pieces turns 30 degrees at each corner.
        let points: Vec<Vec3> = (0..=4)
            .map(|i| {
                let a = i as f32 * 0.52;
                Vec3::new(libm::sinf(a), libm::cosf(a), 0.0) * 0.6
            })
            .collect();
        let limb = Limb {
            radii: vec![0.01; points.len()],
            points: points.clone(),
            depth: 0,
        };
        let smooth = curved(&limb, 0.15);
        assert_eq!(smooth.points[0], points[0]);
        assert!(smooth.points.last().unwrap().distance(points[4]) < 1e-5);
        let worst = smooth
            .points
            .windows(3)
            .map(|w| (w[1] - w[0]).angle_between(w[2] - w[1]))
            .fold(0.0, f32::max);
        assert!(worst < 0.25, "turns {worst} rad at a corner");
    }

    #[test]
    fn fine_twigs_are_millimetres_thick_and_end_in_points() {
        let grown = tree(vec![stem(), branch(Vec3::Y * 2.0)]);
        let twigs = fine_twigs(&tapered(&grown)[1]);
        assert!(twigs.len() >= 3, "{} twigs", twigs.len());
        for twig in &twigs {
            let thick = twig.radii[0] * 2.0;
            assert!(thick <= 0.0041, "{thick} m thick");
            assert_eq!(*twig.radii.last().unwrap(), 0.0);
            let length =
                twig.points[0].distance(twig.points[1]) + twig.points[1].distance(twig.points[2]);
            assert!((0.09..=0.31).contains(&length), "{length} m long");
        }
        assert_eq!(twigs.len(), fine_twigs(&tapered(&grown)[1]).len());
    }
}
