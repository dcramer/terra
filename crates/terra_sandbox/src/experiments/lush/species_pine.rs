//! A grown Scots pine's live crown (D318, D598): its crooked limbs and the pads of shoots at
//! their ends, apart from species.rs so that file stays within its size (D396).

use super::*;

/// How big a clump of a Scots pine's needles is: a shoot's tuft, many to a pad (D318). Half a
/// metre, a real shoot's, not the 0.7 m cards that from under a crown read as a few large fans
/// magnified (D598: flat foliage 45% looking up, Enshrouded's crowns many smaller sprays).
pub(super) const PAD_CLUMP_M: f32 = 0.5;
/// How far apart the pads along a Scots pine's limb are: a little under a pad across, so they
/// meet into one clothed limb (D598).
pub(super) const PAD_STEP_M: f32 = 0.6;

/// A grown Scots pine's live crown (D318), as an old pine's is: not whorls of even spokes,
/// which it had as a sapling and has lost, but a dozen or so crooked limbs left at their own
/// heights round the bole, low ones near level and long, high ones climbing, each forking
/// toward its end, and every end carrying a pad, a domed mass of upturned shoots dense with
/// needles, with sky between the pads; flat-topped, its highest limbs as long as any, on a
/// broad old pine (SpeedTree's pines are built so: needle clusters along a branch's upturned
/// ends, in masses). Each pad is a mass and a branch of its own, so it lights from its own
/// middle and is one lump far off. Returns how many pads it has.
pub(super) fn pine_crown(
    dice: &mut Rng,
    tree: &mut Tree,
    stem_at: &dyn Fn(f32, usize) -> (Vec3, f32),
    (base, top): (f32, f32),
    reach: f32,
    flat: bool,
    (light, favour): (Vec3, f32),
) -> usize {
    let limbs = 14 + dice.below(7) + if flat { 3 } else { 0 };
    let mut turn = dice.range(0.0, TAU);
    let mut pads = 0;
    // Its limbs left in loose whorls of two or three, a year's growth apart, so the crown
    // lies in layers (SpeedTree's conifers: whorled, drooping branches carrying their needle
    // clusters along their length); a whorl's limbs a little apart in height and each its
    // own length and rise, never a sapling's even spokes.
    let whorls = (limbs as f32 / 2.5).ceil();
    let mut heights: Vec<f32> = (0..limbs)
        .map(|k| {
            let whorl = (k as f32 * whorls / limbs as f32).floor();
            ((whorl + 0.5 + dice.range(-0.25, 0.25)) / whorls).min(0.96)
        })
        .collect();
    heights.sort_by(f32::total_cmp);
    for k in 0..limbs {
        let t = heights[k as usize];
        let y = base + (top - base) * t;
        let (at, r) = stem_at(y, k as usize);
        // Round the bole about as a spiral leaves them.
        turn += 2.4 + dice.range(-0.6, 0.6);
        let out = round(turn);
        // Its outline: broadest low and rounding off at its top; on a broad old pine, as
        // long at the top as anywhere, an umbrella.
        let profile = if flat {
            0.6 + 0.4 * libm::powf(libm::sinf(PI * (0.3 + 0.6 * t)), 0.5)
        } else {
            libm::powf(libm::sinf(PI * (0.12 + 0.78 * t)), 0.7)
        };
        let length =
            (reach * profile * dice.range(0.6, 1.05) * (1.0 + favour * out.dot(light))).max(1.2);
        // The low limbs out level or a little down and sagging along their length under
        // their foliage, their ends turned up; the high ones climbing.
        let low = 1.0 - t;
        let rise = dice.range(-0.15, 0.15) + if flat { 0.25 } else { 0.6 } * t - 0.15 * low;
        let heading = out * libm::cosf(rise) + Vec3::Y * libm::sinf(rise);
        let (points, radii, directions) = line(
            dice,
            at + out * r * 0.5,
            heading,
            length,
            (r * 0.28).max(0.04),
            0.8,
            6,
            0.12,
            0.3,
            0.16,
            0.06 + 0.16 * low,
        );
        let mut ends = vec![(
            points[points.len() - 1],
            directions[directions.len() - 1],
            1.0,
        )];
        // Its needles run along its outer two thirds, a pad every stride or so, to one side
        // and the other, overlapping the next, so a limb is clothed, not a tuft on a bare
        // spoke (D359): pads only at its ends and forks left sky between every tuft ("bonsai
        // on broomsticks", the looks review of 2026-10-25).
        // From a quarter of the way out, so a crown is masses on its limbs rather than pads
        // on bare spokes (D598: "pads on poles").
        let mut s = length * dice.range(0.22, 0.32);
        let mut side_of = if dice.unit() < 0.5 { -1.0 } else { 1.0 };
        while s < length - PAD_STEP_M * 0.7 {
            let (at, _, direction) = along(&points, &radii, s / length);
            let side = direction.cross(Vec3::Y).normalize_or(Vec3::X) * side_of;
            ends.push((
                at + side * dice.range(0.1, 0.25),
                (direction + side * 0.5).normalize(),
                0.7,
            ));
            side_of = -side_of;
            s += PAD_STEP_M * dice.range(0.85, 1.15);
        }
        // It forks toward its end, a pad on each fork.
        if length > 2.0 {
            for _ in 0..1 + dice.below(3) {
                let (from, rf, direction) = along(&points, &radii, dice.range(0.4, 0.85));
                let side = direction.cross(Vec3::Y).normalize_or(Vec3::X)
                    * if dice.unit() < 0.5 { -1.0 } else { 1.0 };
                let fork = length * dice.range(0.25, 0.45);
                let (q, qr, qd) = line(
                    dice,
                    from,
                    (direction * 0.6 + side * 0.8 + Vec3::Y * 0.35).normalize(),
                    fork,
                    (rf * 0.6).max(0.025),
                    0.7,
                    3,
                    0.15,
                    0.2,
                    0.25,
                    0.0,
                );
                ends.push((q[q.len() - 1], qd[qd.len() - 1], 1.0));
                // Drawn as far as its limb is, so the pad at its end never floats where the
                // trunk's second level leaves its finer wood out (D359).
                tree.limb(q, qr, 1);
            }
        }
        for (end, direction, full) in ends {
            pad(dice, tree, end, direction, full, pads);
            pads += 1;
        }
        tree.limb(points, radii, 1);
    }
    pads
}

/// A pad of a Scots pine's needles at a branch's end (D318): many short shoots crowded about
/// the end, fanning out ahead and to the sides and turning up, some steeply, each a tuft at
/// its tip, so they lie as one domed cushion a metre or two across whose edge is the tufts'
/// needles. A few long shoots fanned apart read as a palmate leaf, lobes on bare stalks.
/// Its shoots short and crowded, so their tufts overlap and the pad closes into one mass
/// with a needled fringe, as a pine's pads do from below; spread wide apart, each tuft stood
/// alone, a pom-pom with sky round it. `full` is how big a pad it is: one at a limb's end or
/// fork in full, those along the limb smaller.
fn pad(dice: &mut Rng, tree: &mut Tree, end: Vec3, direction: Vec3, full: f32, id: usize) {
    let ahead = Vec3::new(direction.x, 0.0, direction.z).normalize_or(Vec3::X);
    let side = ahead.cross(Vec3::Y);
    // Many small tufts, so a pad is a mass of sprays with a ragged edge (D598), half again as
    // many as when each tuft was 0.7 m.
    let shoots = if full < 1.0 {
        6 + dice.below(3)
    } else {
        11 + dice.below(5)
    };
    let fan = dice.range(2.2, 3.0);
    for _ in 0..shoots {
        let a = dice.range(-0.5, 0.5) * fan;
        // Most near level, a few turned up or hanging, so a pad is a flat plate with a ragged
        // fringe, not a dome (D359).
        let rise = dice.range(-0.1, 0.45);
        let heading = ((ahead * libm::cosf(a) + side * libm::sinf(a)) * libm::cosf(rise)
            + Vec3::Y * libm::sinf(rise))
        .normalize();
        let length = dice.range(0.22, 0.5) * full;
        let from = end - ahead * dice.range(0.0, 0.35) + side * dice.range(-0.18, 0.18);
        let (q, qr, qd) = line(
            dice, from, heading, length, 0.018, 0.6, 2, 0.15, 0.0, 0.3, 0.0,
        );
        tree.clump(
            q[q.len() - 1],
            PAD_CLUMP_M * dice.range(0.9, 1.15) * (0.85 + 0.15 * full),
            qd[qd.len() - 1],
            id,
            id,
        );
        // Inside its tuft, hidden by its own needles, so not drawn: as wood it was a third of
        // a pine's near triangles (`wood` draws a conifer's limbs to depth 2).
        tree.limb(q, qr, 3);
    }
}
