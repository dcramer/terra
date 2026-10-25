//! The species' tests (`species.rs`), apart so the trees stay readable.

use super::*;
use terra_gen::Seed;

/// A forest oak's crown starts well up its trunk, its outline is irregular, and its
/// clumps are spread along its branches, not only at their ends.
#[test]
fn an_oak_has_a_bole_and_masses() {
    for (n, form) in [Form::Tall, Form::Broad, Form::Forked]
        .into_iter()
        .enumerate()
    {
        let mut dice = Seed(5).index(n as u64).rng();
        let tree = oak(&mut dice, 22.0, form, false);
        let lowest = tree
            .tips
            .iter()
            .map(|(at, _)| at.y)
            .fold(f32::MAX, f32::min);
        assert!(lowest > 22.0 * 0.15, "{form:?}: leaves down to {lowest} m");
        assert!(
            tree.masses.len() >= 4,
            "{form:?}: {} masses",
            tree.masses.len()
        );
        assert!(
            tree.tips.len() > 120,
            "{form:?}: {} clumps",
            tree.tips.len()
        );
        assert_eq!(tree.tips.len(), tree.mass_of.len());
        assert!(tree.limbs.iter().filter(|l| l.depth == 1).count() >= 4);
    }
}

/// A pine's crown is high on a long clean bole; a spruce's comes nearly to the ground.
#[test]
fn conifers_have_their_habits() {
    let mut dice = Seed(6).rng();
    let pine = conifer(&mut dice, 28.0, Species::Pine, Form::Tall, false);
    let spruce = conifer(&mut dice, 24.0, Species::Spruce, Form::Tall, false);
    let lowest = |t: &Tree| t.tips.iter().map(|(at, _)| at.y).fold(f32::MAX, f32::min);
    assert!(
        lowest(&pine) > 28.0 * 0.45,
        "pine leaves down to {}",
        lowest(&pine)
    );
    assert!(
        lowest(&spruce) < 24.0 * 0.4,
        "spruce leaves only from {}",
        lowest(&spruce)
    );
    // Below its live crown, a spruce's dead branches.
    let dead = spruce
        .limbs
        .iter()
        .filter(|l| l.depth == 2 && l.points[0].y < lowest(&spruce) - 0.5)
        .count();
    assert!(dead >= 6, "{dead} dead branches");
    assert!(spruce.branches <= 16);
}

/// A grown Scots pine's crown is pads of needles on crooked limbs left one by one round its
/// bole, not whorls of spokes in tiers (D318): its limbs leave the bole at heights of their
/// own, and its clumps lie in compact pads, each a cushion round its middle, many to a
/// crown, high on its bole.
#[test]
fn a_scots_pines_crown_is_pads_on_its_limbs() {
    let mut dice = Seed(18).rng();
    for form in [Form::Tall, Form::Broad, Form::Forked, Form::Broken] {
        let height = 28.0;
        let pine = conifer(&mut dice, height, Species::Pine, form, false);
        let feet: Vec<f32> = pine
            .limbs
            .iter()
            .enumerate()
            .filter(|(i, l)| l.depth == 1 && !pine.dead.contains(i))
            // Those that leave the bole, not the forks off them: a fork starts on another
            // limb, which may pass close by a broken pine's leaning leader.
            .filter(|(i, l)| {
                let foot = l.points[0];
                !pine.limbs.iter().enumerate().any(|(j, other)| {
                    j != *i
                        && other.depth == 1
                        && other.points.windows(2).any(|s| {
                            let (a, b) = (s[0], s[1]);
                            let t = ((foot - a).dot(b - a) / (b - a).length_squared().max(1e-6))
                                .clamp(0.0, 1.0);
                            (a + (b - a) * t).distance(foot) < 0.02
                        })
                })
            })
            .map(|(_, l)| l.points[0])
            .filter(|foot| {
                pine.limbs.iter().filter(|l| l.depth == 0).any(|stem| {
                    let (at, r) = by_height(&stem.points, &stem.radii, foot.y);
                    Vec2::new(foot.x - at.x, foot.z - at.z).length() < r + 0.4
                })
            })
            .map(|foot| foot.y)
            .collect();
        assert!(feet.len() >= 12, "{form:?}: {} limbs", feet.len());
        // A whorl leaves four to six limbs at one height; here none shares its height
        // (within 0.1 m) with more than two others.
        let most = feet
            .iter()
            .map(|y| feet.iter().filter(|other| (*other - y).abs() < 0.1).count() - 1)
            .max()
            .unwrap_or(0);
        assert!(most <= 2, "{form:?}: {most} limbs at one limb's height");
        assert!(
            (15..=150).contains(&pine.branches),
            "{form:?}: {} pads",
            pine.branches
        );
        for (i, (at, _)) in pine.tips.iter().enumerate() {
            let middle = pine.masses[pine.mass_of[i]];
            assert!(
                at.distance(middle) < 1.6,
                "{form:?}: a clump {} m from its pad's middle",
                at.distance(middle)
            );
            assert!(at.y > height * 0.55, "{form:?}: a clump at {} m", at.y);
        }
    }
}

/// A young spruce in the shade is tiers of drooping boughs with gaps between them, its
/// needles on their outer part, not one even mass of needles to its trunk (D289): beside
/// a grown spruce's, which runs its needles nearly to the trunk, few of its clumps are
/// near its trunk, and some of its height between whorls holds none.
#[test]
fn a_young_spruce_stands_in_open_tiers() {
    let mut dice = Seed(12).rng();
    for k in 0..4 {
        let height = 4.0 + k as f32;
        let young = conifer(&mut dice, height, Species::Spruce, Form::Tall, true);
        let below_top = |at: &Vec3| at.y < height - 1.0;
        let clumps: Vec<Vec3> = young
            .tips
            .iter()
            .map(|(at, _)| *at)
            .filter(below_top)
            .collect();
        // How far out each clump is, of the farthest of its tier's (within 0.3 m).
        let out = |at: &Vec3| Vec2::new(at.x, at.z).length();
        let inner = clumps
            .iter()
            .filter(|at| {
                let tier = clumps
                    .iter()
                    .filter(|other| (other.y - at.y).abs() < 0.3)
                    .map(out)
                    .fold(0.0, f32::max);
                out(at) < tier * 0.3
            })
            .count();
        assert!(
            (inner as f32) < clumps.len() as f32 * 0.06,
            "{k}: {inner} of {} clumps by the trunk",
            clumps.len()
        );
        // Gaps between tiers: tenths of a metre up the crown with no clump's middle.
        let (low, high) = clumps.iter().fold((f32::MAX, f32::MIN), |(l, h), at| {
            (l.min(at.y), h.max(at.y))
        });
        let bands = ((high - low) / 0.1).ceil() as usize;
        let mut filled = vec![false; bands.max(1)];
        for at in &clumps {
            filled[(((at.y - low) / 0.1) as usize).min(bands - 1)] = true;
        }
        let empty = filled.iter().filter(|f| !**f).count();
        assert!(
            empty as f32 > bands as f32 * 0.12,
            "{k}: only {empty} of {bands} bands empty"
        );
    }
}

/// The trunk's highest point, and how far out from its foot its top is for its height.
fn top_of(tree: &Tree) -> (f32, f32) {
    let trunk = &tree
        .limbs
        .iter()
        .find(|l| l.depth == 0)
        .expect("a trunk")
        .points;
    let top = trunk[trunk.len() - 1];
    (top.y, Vec2::new(top.x, top.z).length())
}

/// Pines of a grove are not one pine: some slender, some stout, some leaning hard, each
/// with grey dead branches under its live crown (D276).
#[test]
fn pines_differ_one_from_the_next() {
    let mut dice = Seed(9).rng();
    // Sixteen, as one in four leans hard: of eight, one in ten draws had none.
    let pines: Vec<Tree> = (0..16)
        .map(|_| conifer(&mut dice, 26.0, Species::Pine, Form::Tall, false))
        .collect();
    let girths: Vec<f32> = pines.iter().map(|p| p.trunk_m / 26.0).collect();
    let (thin, stout) = girths
        .iter()
        .fold((f32::MAX, 0.0f32), |(a, b), &g| (a.min(g), b.max(g)));
    assert!(stout > thin * 1.3, "girths {girths:?}");
    let leans: Vec<f32> = pines.iter().map(|p| top_of(p).1 / 26.0).collect();
    assert!(
        leans.iter().any(|&l| l > 0.06) && leans.iter().any(|&l| l < 0.04),
        "leans {leans:?}"
    );
    for pine in &pines {
        let lowest = pine
            .tips
            .iter()
            .map(|(at, _)| at.y)
            .fold(f32::MAX, f32::min);
        let shed = pine
            .dead
            .iter()
            .map(|&i| &pine.limbs[i])
            .filter(|l| {
                let (a, b) = (l.points[0], l.points[l.points.len() - 1]);
                a.y < lowest && a.distance(b) > 0.7
            })
            .count();
        assert!(shed >= 3, "{shed} dead branches under the crown");
    }
}

/// A forked pine has two leaders, a broken one a dead spike where its top was and a
/// branch below turned up to lead in its place.
#[test]
fn pines_fork_and_break() {
    let mut dice = Seed(10).rng();
    let forked = conifer(&mut dice, 26.0, Species::Pine, Form::Forked, false);
    let leaders = forked
        .limbs
        .iter()
        .filter(|l| l.depth == 0 && l.points[l.points.len() - 1].y > 26.0 * 0.92)
        .count();
    assert_eq!(leaders, 2, "{leaders} leaders");
    let broken = conifer(&mut dice, 26.0, Species::Pine, Form::Broken, false);
    let (snapped, _) = top_of(&broken);
    assert!(snapped < 26.0 * 0.88, "its trunk runs to {snapped} m");
    assert!(
        broken
            .dead
            .iter()
            .any(|&i| broken.limbs[i].points[0].y >= snapped - 0.01),
        "no dead spike at its top"
    );
    let highest = broken.tips.iter().map(|(at, _)| at.y).fold(0.0, f32::max);
    assert!(highest > snapped + 1.0, "its leaves end at {highest} m");
}

/// Points every few centimetres along every limb of a tree, its wood as drawn.
fn wood_of(tree: &Tree) -> Vec<Vec3> {
    tree.limbs
        .iter()
        .flat_map(|limb| {
            limb.points.windows(2).flat_map(|w| {
                let steps = (w[0].distance(w[1]) / 0.05).ceil().max(1.0) as usize;
                (0..=steps).map(move |k| w[0].lerp(w[1], k as f32 / steps as f32))
            })
        })
        .collect()
}

fn leaf() -> Leaf {
    Leaf {
        cell: Vec2::ZERO,
        across: 0.25,
        outline: [(0.0, 0.0); 8],
    }
}

/// A crown's leaves hang on its twigs (D309): every clump is on wood, and nine in ten of
/// its cards have their stalk within a third of a metre of a twig, where cards hung up to
/// half a clump off read from below as floating.
#[test]
fn a_crowns_cards_hang_on_its_twigs() {
    let mut dice = Seed(14).rng();
    for (species, tree) in [
        (Species::Oak, oak(&mut dice, 22.0, Form::Tall, false)),
        (Species::Oak, oak(&mut dice, 20.0, Form::Broad, false)),
        (
            Species::Pine,
            conifer(&mut dice, 26.0, Species::Pine, Form::Tall, false),
        ),
    ] {
        let wood = wood_of(&tree);
        let off_wood = |at: Vec3| wood.iter().map(|w| w.distance(at)).fold(f32::MAX, f32::min);
        // (A pine's leading shoot's clumps hang straight down its top, a little off its
        // leaning trunk.)
        for (tip, _) in &tree.tips {
            let off = off_wood(*tip);
            assert!(off < 0.1, "{species:?}: a clump {off} m off its wood");
        }
        let cards = crown_cards(&mut dice, &tree, 22.0, leaf());
        let mut feet: Vec<f32> = cards
            .iter()
            .flatten()
            .step_by(5)
            .map(|card| off_wood(card.middle - card.up * card.half * 0.75))
            .collect();
        feet.sort_by(f32::total_cmp);
        let most = feet[feet.len() * 9 / 10];
        assert!(
            most < 0.35,
            "{species:?}: one card in ten {most} m from a twig"
        );
    }
}

/// A grown Scots pine's bole carries the stubs of the whorls it shed, not a clean pole
/// (D309).
#[test]
fn a_scots_pine_has_a_stubbed_bole() {
    let mut dice = Seed(16).rng();
    let pine = conifer(&mut dice, 26.0, Species::Pine, Form::Tall, false);
    let lowest = pine
        .tips
        .iter()
        .map(|(at, _)| at.y)
        .fold(f32::MAX, f32::min);
    let stubs = pine
        .dead
        .iter()
        .filter(|&&i| {
            let limb = &pine.limbs[i];
            limb.points[0].y < lowest && limb.points[0].distance(limb.points[1]) < 0.8
        })
        .count();
    assert!(stubs >= 10, "{stubs} stubs on its bole");
}
