//! The lush forest's middle layer (D268): what stands between the knee-high floor and the
//! canopy, one to five metres, so a sightline at eye height ends in leaves within a few tens
//! of metres, as it does in a real wood and in Sons of the Forest, rather than running
//! between bare trunks across a park.
//!
//! Each kind is grown by the same rules as the game's trees (`terra_world::grown::tree`), as
//! SpeedTree grows its shrubs from its trees' generators: a hazel stool of many slender stems
//! leaning out from one root, holly as a small dense evergreen, young spruces, saplings,
//! bramble as a tangle of arching canes, and snags, small trees long dead, bare and broken.
//! Each is a few variants a kind, drawn at two levels of detail with a sparse stand-in for
//! its shadow that shrinks away with distance (D234).

use bevy::{mesh::VertexAttributeValues, prelude::*};
use std::f32::consts::{PI, TAU};
use terra_gen::Rng;
use terra_world::grown::build::{Build, Limb, Placing, tube};
use terra_world::grown::tree::{self as grown_tree, Canopy, Spray, Tree};
use terra_world::grown::{Card, Habit, TreeDef};

/// A plant of the middle layer, as meshes: its wood and leaves near and farther off, the
/// sparse cards its shadow is cast from, and, for something solid, its trunk on its own, so
/// a walker is stopped by the trunk rather than the reach of its branches.
pub(super) struct Shrub {
    pub wood: [Build; 2],
    pub leaves: Option<[Build; 2]>,
    pub shadow: Option<Build>,
    pub trunk: Option<Build>,
}

/// An empty tree to gather others into.
fn empty(habit: Habit) -> Tree {
    Tree {
        limbs: Vec::new(),
        sprays: Vec::new(),
        branches: 0,
        trunk_m: 0.0,
        height: 0.0,
        habit,
    }
}

/// A tree turned and moved: every limb's points and every spray.
fn moved(tree: &mut Tree, turn: Quat, by: Vec3) {
    for limb in &mut tree.limbs {
        for point in &mut limb.points {
            *point = turn * *point + by;
        }
    }
    for spray in &mut tree.sprays {
        spray.at = turn * spray.at + by;
        spray.along = spray.along.map(|along| turn * along);
    }
    tree.height = tree
        .limbs
        .iter()
        .flat_map(|limb| limb.points.iter().map(|p| p.y))
        .fold(tree.height, f32::max);
}

/// Gathers `other` into `into`, its branches numbered after `into`'s.
fn merge(into: &mut Tree, other: Tree) {
    let first = into.branches;
    into.branches += other.branches;
    into.limbs.extend(other.limbs);
    into.sprays
        .extend(other.sprays.into_iter().map(|spray| Spray {
            branch: spray.branch + first,
            ..spray
        }));
    into.trunk_m = into.trunk_m.max(other.trunk_m);
    into.height = into.height.max(other.height);
}

fn def(habit: Habit, leaves: &str, clump_m: f32, trunk: f32) -> TreeDef {
    TreeDef {
        habit,
        bark: "oak_bark".into(),
        bark_m: 1.0,
        bark_tint: (1.0, 1.0, 1.0),
        leaves: leaves.into(),
        tint: (1.0, 1.0, 1.0),
        clump_m,
        trunk,
    }
}

/// A leafy plant from its shape: its wood at two levels, its leaves' cards placed once and
/// drawn near and with fewer, larger cards farther off, as the trees' levels are (D221), and
/// its shadow's sparse cards.
fn leafy(dice: &mut Rng, tree: &Tree, leaf: &Card, tint: Vec3) -> Shrub {
    let canopy = Canopy::of(tree, tint);
    let placed = grown_tree::crown_cards(dice, tree, &canopy);
    drawn(tree, &canopy, &placed, leaf)
}

/// A leafy plant's cards, placed, drawn at its levels: every card of every clump near, as a
/// tree's nearest level, so its leaves stand at their own size a stride off rather than a
/// few enlarged (D269); past that the trees' middle level, three of each clump enlarged.
fn drawn(tree: &Tree, canopy: &Canopy, placed: &[Vec<Placing>], leaf: &Card) -> Shrub {
    let mut near = grown_tree::crown(placed, tree, leaf, 0);
    let mut far = grown_tree::crown(placed, tree, leaf, 1);
    let shadow = grown_tree::shadow_cards(placed, tree, leaf);
    let mut wood = [
        grown_tree::wood(tree, 0, 1.0),
        grown_tree::wood(tree, 1, 1.0),
    ];
    for build in [&mut near, &mut far].into_iter().chain(wood.iter_mut()) {
        build.see_sky(|at| canopy.sky(at));
    }
    Shrub {
        wood,
        leaves: Some([near, far]),
        shadow: Some(shadow),
        trunk: None,
    }
}

/// A tree bowed over toward `axis`'s turn, more the higher a point is, as a stem arches
/// under the weight of its leaves: `angle` at its top, none at its foot.
fn bowed(tree: &mut Tree, axis: Vec3, angle: f32) {
    let height = tree.height.max(0.1);
    let turn = |at: Vec3| {
        let share = (at.y / height).clamp(0.0, 1.0);
        Quat::from_axis_angle(axis, angle * share * share)
    };
    for limb in &mut tree.limbs {
        for point in &mut limb.points {
            *point = turn(*point) * *point;
        }
    }
    for spray in &mut tree.sprays {
        let by = turn(spray.at);
        spray.at = by * spray.at;
        spray.along = spray.along.map(|along| by * along);
    }
}

/// How a hazel stool has grown (D276), so that stools read apart rather than as one rounded
/// dome: hazel is cut to the ground every seven to twenty years and grows again as rods from
/// the stool, so a wood holds stools at every age since.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Stool {
    /// Five to nine stems leaning out all round, some bowing over at the top: a vase.
    Vase,
    /// Four to seven leaning one way, toward the light at a path's or a clearing's edge.
    Fan,
    /// Nine to fourteen thin straight rods a few years from the cut, nearly upright, their
    /// leaves only along their upper half.
    Rods,
    /// Two to four thick old stems left uncut for decades, bare for half their height under
    /// crowns of their own, like small trees from one root.
    Overstood,
}

/// A hazel stool `tallest` metres tall at most, grown as `stool` says: slender stems from one
/// root, each a small upright tree, leafy in gappy sprays along their upper part so the
/// stems show between them.
pub(super) fn coppice(
    dice: &mut Rng,
    leaf: &Card,
    tint: Vec3,
    stool: Stool,
    tallest: f32,
) -> Shrub {
    // How many stems, how far they lean, how thick they are for their height, their sprays'
    // size, the share of sprays kept, and how much of each stem is a bare pole under the
    // small tree it carries.
    let (stems, lean, trunk, clump, keep, bare) = match stool {
        Stool::Vase => (5 + dice.below(5), (0.15, 0.5), 0.011, 0.75, 0.75, 0.0),
        Stool::Fan => (4 + dice.below(4), (0.3, 0.7), 0.011, 0.75, 0.75, 0.0),
        Stool::Rods => (9 + dice.below(6), (0.03, 0.22), 0.012, 0.65, 0.6, 0.3),
        Stool::Overstood => (2 + dice.below(3), (0.1, 0.3), 0.025, 0.75, 0.8, 0.35),
    };
    let def = def(Habit::Upright, "hazel", clump, trunk);
    let turn = dice.range(0.0, TAU);
    let light = dice.range(0.0, TAU);
    let mut whole = empty(Habit::Upright);
    for k in 0..stems {
        let height = tallest
            * match stool {
                Stool::Rods => dice.range(0.7, 1.0),
                _ => dice.range(0.55, 1.0),
            };
        let pole = height * bare;
        let mut stem = grown_tree::grow(dice, &def, height - pole);
        stem.sprays.retain(|_| dice.unit() < keep);
        // Hazel holds its leaves in flat sprays along its shoots, out from its stem and
        // drooping a little, not in balls, so a stool reads as layers with gaps between.
        for spray in &mut stem.sprays {
            let out = Vec3::new(spray.at.x, 0.0, spray.at.z).normalize_or(Vec3::X);
            let wander = Vec3::new(dice.range(-0.5, 0.5), 0.0, dice.range(-0.5, 0.5));
            spray.along = Some((out + wander - Vec3::Y * dice.range(0.0, 0.35)).normalize());
        }
        if pole > 0.0 {
            // Its stem's foot no longer flares into roots up there.
            let r = stem.trunk_m;
            for limb in stem.limbs.iter_mut().filter(|limb| limb.depth == 0) {
                for radius in &mut limb.radii {
                    *radius = radius.min(r);
                }
            }
            moved(&mut stem, Quat::IDENTITY, Vec3::Y * pole);
            stem.limbs.push(Limb {
                points: vec![Vec3::ZERO, Vec3::Y * pole * 0.5, Vec3::Y * pole],
                radii: vec![r * 1.6, r * 1.1, r],
                depth: 0,
            });
        }
        let around = match stool {
            Stool::Fan => light + dice.range(-0.9, 0.9),
            _ => turn + k as f32 / stems as f32 * TAU + dice.range(-0.4, 0.4),
        };
        let out = Vec3::new(libm::cosf(around), 0.0, libm::sinf(around));
        let toward = Vec3::Y.cross(out);
        // A leaning stem of a vase or a fan may bow over under its leaves.
        if matches!(stool, Stool::Vase | Stool::Fan) && dice.unit() < 0.5 {
            bowed(&mut stem, toward, dice.range(0.2, 0.6));
        }
        let lean = Quat::from_axis_angle(toward, dice.range(lean.0, lean.1));
        let spin = Quat::from_rotation_y(dice.range(0.0, TAU));
        moved(&mut stem, lean * spin, out * dice.range(0.04, 0.3));
        merge(&mut whole, stem);
    }
    leafy(dice, &whole, leaf, tint)
}

/// Oak scrub: two to four small oaks from one root a metre or two tall, crooked and
/// spreading, as oak grows back where it was browsed or cut, gappy, its stems showing.
pub(super) fn scrub(dice: &mut Rng, leaf: &Card, tint: Vec3) -> Shrub {
    let def = def(Habit::Spreading, "oak", 0.6, 0.03);
    let mut whole = empty(Habit::Spreading);
    let turn = dice.range(0.0, TAU);
    let stems = 2 + dice.below(3);
    for k in 0..stems {
        let height = dice.range(0.9, 1.9);
        let mut stem = grown_tree::grow(dice, &def, height);
        stem.sprays.retain(|_| dice.unit() < 0.8);
        let around = turn + k as f32 / stems as f32 * TAU + dice.range(-0.5, 0.5);
        let out = Vec3::new(libm::cosf(around), 0.0, libm::sinf(around));
        let lean = Quat::from_axis_angle(Vec3::Y.cross(out), dice.range(0.1, 0.5));
        let spin = Quat::from_rotation_y(dice.range(0.0, TAU));
        moved(&mut stem, lean * spin, out * dice.range(0.05, 0.35));
        merge(&mut whole, stem);
    }
    leafy(dice, &whole, leaf, tint)
}

/// A small conifer-shaped evergreen, its branches in whorls to near the ground: holly, of
/// dark leathery leaves and red berries, or a young spruce.
pub(super) fn evergreen(
    dice: &mut Rng,
    leaf: &Card,
    tint: Vec3,
    height: f32,
    clump_m: f32,
) -> Shrub {
    let def = def(Habit::Conical, "", clump_m, 0.024);
    let tree = grown_tree::grow(dice, &def, height);
    let canopy = Canopy::of(&tree, tint);
    let mut placed = grown_tree::crown_cards(dice, &tree, &canopy);
    // A spruce's shoots are not flat fans: each branch's side shoots hang from it in a
    // curtain and turn every way round it, as Norway spruce's do, so a branch reads as a
    // rolled bottle-brush of needles from any side, not a horizontal plank (D269).
    for (spray, set) in tree.sprays.iter().zip(&mut placed) {
        let Some(along) = spray.along else { continue };
        let side = along.cross(Vec3::Y).normalize_or(Vec3::X);
        let hang = (along * 0.45 - Vec3::Y * 0.75 + side * dice.range(-0.5, 0.5)).normalize();
        let below = spray.at - Vec3::Y * spray.size * 0.12;
        let (size, roll) = (spray.size * dice.range(0.6, 0.8), dice.range(-0.8, 0.8));
        set.push(grown_tree::twig_card(
            dice, &canopy, below, hang, size, roll,
        ));
        let size = spray.size * dice.range(0.7, 0.9);
        let roll = if dice.unit() < 0.5 { -1.0 } else { 1.0 } * dice.range(0.9, 1.4);
        set.push(grown_tree::twig_card(
            dice, &canopy, spray.at, along, size, roll,
        ));
    }
    drawn(&tree, &canopy, &placed, leaf)
}

/// Holly: a small evergreen tree two to five metres tall, a short stem or two under a dense
/// egg of dark leaves nearly to the ground, its sprays all through the mass rather than at
/// the ends of branches, so it reads as one solid dark shape in the shade.
pub(super) fn holly(dice: &mut Rng, leaf: &Card, tint: Vec3) -> Shrub {
    let height = dice.range(2.2, 5.0);
    let width = height * dice.range(0.32, 0.5);
    let mut tree = empty(Habit::Upright);
    let lean = dice.unit_vector().with_y(0.0) * 0.08;
    let stem = |y: f32| (Vec3::Y + lean) * y;
    tree.limbs.push(Limb {
        points: (0..=5)
            .map(|i| stem(i as f32 / 5.0 * height * 0.85))
            .collect(),
        radii: (0..=5)
            .map(|i| height * 0.018 * (1.0 - 0.75 * i as f32 / 5.0))
            .collect(),
        depth: 0,
    });
    let sprays = (height * width * 7.0) as u32 + 10;
    for k in 0..sprays {
        let t = dice.range(0.12, 0.97);
        // Widest a little below the middle, rounded at the top.
        let most = width * libm::sinf(PI * libm::powf(t, 0.75)).max(0.15);
        // Mostly in its outer shell, where the light reaches the leaves and the eye sees
        // them; few deep inside, where they would only be drawn over each other.
        let r = libm::sqrtf(dice.range(0.35, 1.0)) * most;
        let around = dice.range(0.0, TAU);
        let at = stem(t * height) + Vec3::new(libm::cosf(around), 0.0, libm::sinf(around)) * r;
        tree.sprays.push(Spray {
            at,
            along: None,
            size: dice.range(0.45, 0.62),
            branch: (k / 4) as usize,
        });
        // A twig out to every other one.
        if k % 2 == 0 {
            let from = stem((t * height * 0.9).min(height * 0.85));
            tree.limbs.push(Limb {
                points: vec![from, from.lerp(at, 0.5) + Vec3::Y * 0.05, at],
                radii: vec![0.018, 0.012, 0.006],
                depth: 1,
            });
        }
    }
    tree.branches = sprays.div_ceil(4) as usize;
    tree.height = height;
    tree.trunk_m = height * 0.018;
    leafy(dice, &tree, leaf, tint)
}

/// A sapling: one slender upright stem and its few branches, oak or hazel leaves, two to
/// five metres.
pub(super) fn sapling(dice: &mut Rng, leaf: &Card, tint: Vec3) -> Shrub {
    let def = def(Habit::Upright, "", 0.7, 0.014);
    let height = dice.range(2.2, 5.2);
    let mut tree = grown_tree::grow(dice, &def, height);
    // Leaning a little toward the light it found.
    let out = dice.unit_vector().with_y(0.0).normalize_or(Vec3::X);
    moved(
        &mut tree,
        Quat::from_axis_angle(Vec3::Y.cross(out), dice.range(0.0, 0.12)),
        Vec3::ZERO,
    );
    leafy(dice, &tree, leaf, tint)
}

/// Bramble: a tangle of arching canes from one root, each rising steeply then bowing over
/// to root its tip in the ground a metre or two out, leafy along its outer part; waist to
/// chest high, two to four metres across.
pub(super) fn bramble(dice: &mut Rng, leaf: &Card, tint: Vec3) -> Shrub {
    let mut tangle = empty(Habit::Upright);
    let canes = 7 + dice.below(8);
    let turn = dice.range(0.0, TAU);
    for k in 0..canes {
        let around = turn + k as f32 / canes as f32 * TAU + dice.range(-0.5, 0.5);
        let out = Vec3::new(libm::cosf(around), 0.0, libm::sinf(around));
        let side = out.cross(Vec3::Y);
        let reach = dice.range(0.8, 2.0);
        let rise = dice.range(0.55, 1.45);
        let wobble = dice.range(-0.3, 0.3);
        let steps = 8;
        let point = |t: f32| {
            out * reach * t
                + side * wobble * libm::sinf(t * PI) * reach
                + Vec3::Y * (rise * libm::sinf(PI * libm::powf(t, 0.75)) + 0.03)
        };
        let points: Vec<Vec3> = (0..=steps)
            .map(|i| point(i as f32 / steps as f32))
            .collect();
        let radii = (0..=steps)
            .map(|i| 0.013 * (1.0 - 0.6 * i as f32 / steps as f32))
            .collect();
        let length = reach + rise;
        let mut along = length * 0.25;
        while along < length * 0.97 {
            let t = along / length;
            tangle.sprays.push(Spray {
                at: point(t) + dice.unit_vector() * 0.08,
                along: None,
                size: dice.range(0.3, 0.42),
                branch: k as usize,
            });
            along += dice.range(0.25, 0.4);
        }
        tangle.limbs.push(Limb {
            points,
            radii,
            depth: 0,
        });
    }
    tangle.branches = canes as usize;
    tangle.height = tangle
        .limbs
        .iter()
        .flat_map(|limb| limb.points.iter().map(|p| p.y))
        .fold(0.3, f32::max);
    tangle.trunk_m = 0.013;
    leafy(dice, &tangle, leaf, tint)
}

/// A snag: a small tree long dead, standing four to ten metres, broken off at a half to
/// nine tenths of its height, its twigs gone and its branches snapped short; a dead birch's
/// few climbing ones, or a dead spruce's many thin whorls. Its trunk is solid.
pub(super) fn snag(dice: &mut Rng, conifer: bool, tint: Vec3) -> Shrub {
    let habit = if conifer {
        Habit::Conical
    } else {
        Habit::Upright
    };
    let height = dice.range(4.0, 10.0);
    let trunk = dice.range(0.026, 0.04);
    let mut tree = grown_tree::grow(dice, &def(habit, "", 0.5, trunk), height);
    let cut = height * dice.range(0.5, 0.9);
    tree.sprays.clear();
    tree.limbs
        .retain(|limb| limb.depth == 0 || (limb.depth == 1 && limb.points[0].y < cut - 0.3));
    for limb in &mut tree.limbs {
        if limb.depth == 0 {
            // Broken at the cut, in a jagged point of splintered wood.
            let keep = limb.points.iter().take_while(|p| p.y < cut).count().max(2);
            limb.points.truncate(keep);
            limb.radii.truncate(keep);
            let (top, r) = (limb.points[keep - 1], limb.radii[keep - 1]);
            let lean = dice.unit_vector().with_y(0.0) * r;
            limb.points.extend([
                top + Vec3::Y * r * 1.5 + lean,
                top + Vec3::Y * r * 3.5 - lean,
            ]);
            limb.radii.extend([r * 0.6, r * 0.08]);
        } else {
            // Snapped short, the stubs pointing a little down with their weight gone.
            let keep = ((limb.points.len() as f32 * dice.range(0.3, 0.85)).ceil() as usize)
                .clamp(2, limb.points.len());
            limb.points.truncate(keep);
            limb.radii.truncate(keep);
        }
    }
    let sides = |depth: u32| if depth == 0 { 9 } else { 4 };
    let mut trunk = Build::default();
    let mut branches = Build::default();
    let mut far = Build::default();
    let mut shadow = Build::default();
    for limb in &tree.limbs {
        // Dead wood barely moves in the wind.
        let sway = height * 6.0;
        if limb.depth == 0 {
            tube(&mut trunk, limb, sides(0), sway, 1.0, tint);
            tube(&mut shadow, limb, 6, sway, 1.0, tint);
        } else {
            tube(&mut branches, limb, sides(1), sway, 1.0, tint);
            tube(&mut far, limb, 3, sway, 1.0, tint);
            tube(&mut shadow, limb, 3, sway, 1.0, tint);
        }
    }
    // Weathered wood sees the sky all round; its foot less.
    for build in [&mut trunk, &mut branches, &mut far] {
        build.see_sky(|at| (0.45 + 0.1 * at.y).min(1.0));
    }
    Shrub {
        wood: [branches, far],
        leaves: None,
        shadow: Some(shadow),
        trunk: Some(trunk),
    }
}

/// A copy of a mesh for a shadow stand-in that shrinks away with distance (D234): each
/// connected piece of it, a card or a limb, carries in its colour its offset from its own
/// middle, which the foliage shader draws it into.
pub(super) fn shrinking(mesh: &Mesh) -> Mesh {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return mesh.clone();
    };
    let indices: Vec<usize> = mesh
        .indices()
        .map(|indices| indices.iter().collect())
        .unwrap_or_default();
    let mut parent: Vec<usize> = (0..positions.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for triangle in indices.chunks(3) {
        for &other in &triangle[1..] {
            let (a, b) = (root(&mut parent, triangle[0]), root(&mut parent, other));
            if a != b {
                parent[b] = a;
            }
        }
    }
    let mut sums = vec![(Vec3::ZERO, 0u32); positions.len()];
    for (i, p) in positions.iter().enumerate() {
        let r = root(&mut parent, i);
        sums[r].0 += Vec3::from(*p);
        sums[r].1 += 1;
    }
    let colours: Vec<[f32; 4]> = positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (sum, count) = sums[root(&mut parent, i)];
            let offset = Vec3::from(*p) - sum / count.max(1) as f32;
            [offset.x, offset.y, offset.z, 1.0]
        })
        .collect();
    let mut out = mesh.clone();
    out.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_gen::Seed;

    fn leaf() -> Card {
        Card {
            cell: Vec2::ZERO,
            across: 0.25,
            outline: vec![(0.0, 0.0), (0.25, 0.0), (0.25, 0.25), (0.0, 0.25)],
            colour: Vec3::splat(0.3),
            leaf: None,
        }
    }

    const STOOLS: [Stool; 4] = [Stool::Vase, Stool::Fan, Stool::Rods, Stool::Overstood];

    /// How long the leaves on each card of a build are in the world: a card is its cell's
    /// outline scaled as the card is, so its size is the distance between two of its corners
    /// over theirs in the atlas, and its leaves the card's share of that.
    fn leaf_lengths(build: &Build, card: &Card) -> Vec<f32> {
        let n = card.outline.len();
        let share = card
            .leaf
            .expect("a composed card says how long its leaves are");
        build
            .positions
            .chunks(n)
            .zip(build.uvs.chunks(n))
            .map(|(at, uv)| {
                let world = Vec3::from(at[0]).distance(Vec3::from(at[n / 2]));
                let atlas = Vec2::from(uv[0]).distance(Vec2::from(uv[n / 2]));
                world / atlas * card.across * share
            })
            .collect()
    }

    /// A hazel's leaves are 6 to 12 cm long and 1 cm of stalk, near the camera at their own
    /// size, not three times it (D276).
    #[test]
    fn hazel_leaves_are_their_real_size() {
        let cards = terra_world::grown::Cards::read();
        let hazel = cards.get("hazel");
        let mut dice = Seed(7).rng();
        for stool in STOOLS {
            for tallest in [1.6, 3.6] {
                let shrub = coppice(&mut dice, hazel, Vec3::ONE, stool, tallest);
                let mut lengths = leaf_lengths(&shrub.leaves.expect("leafy")[0], hazel);
                lengths.sort_by(f32::total_cmp);
                let middle = lengths[lengths.len() / 2];
                assert!(
                    (0.07..0.13).contains(&middle),
                    "{stool:?} {tallest} m: leaves {middle} m long"
                );
            }
        }
    }

    /// A stool of rods or an overstood one stands on bare stems, its leaves only up them; a
    /// vase is leafy lower down; a fan's leaves lie off to one side of its root.
    #[test]
    fn stools_grow_apart() {
        let mut dice = Seed(8).rng();
        let leaves = |dice: &mut Rng, stool: Stool| {
            let shrub = coppice(dice, &leaf(), Vec3::ONE, stool, 3.5);
            let build = shrub
                .leaves
                .expect("leafy")
                .into_iter()
                .next()
                .expect("near");
            let points: Vec<Vec3> = build.positions.iter().map(|p| Vec3::from(*p)).collect();
            let lowest = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
            let middle = points.iter().sum::<Vec3>() / points.len() as f32;
            (
                lowest / build.height(),
                Vec2::new(middle.x, middle.z).length(),
            )
        };
        let mut low = [0.0f32; 4];
        let mut off = [0.0f32; 4];
        for _ in 0..4 {
            for (k, stool) in STOOLS.into_iter().enumerate() {
                let (lowest, aside) = leaves(&mut dice, stool);
                low[k] += lowest / 4.0;
                off[k] += aside / 4.0;
            }
        }
        assert!(
            low[2] > 0.25 && low[3] > 0.25,
            "rods and overstood leafy from {low:?}"
        );
        assert!(
            low[0] < low[2] && low[0] < low[3],
            "a vase leafy from {low:?}"
        );
        assert!(
            off[1] > off[0] * 1.5,
            "a fan's leaves {off:?} m off its root"
        );
    }

    #[test]
    fn the_middle_layer_stands_between_knee_and_canopy_height() {
        let mut dice = Seed(5).rng();
        for k in 0..6 {
            let stool = coppice(
                &mut dice,
                &leaf(),
                Vec3::ONE,
                STOOLS[k % 4],
                2.0 + k as f32 * 0.6,
            );
            let height = stool.leaves.as_ref().expect("leafy")[0].height();
            assert!((1.8..5.5).contains(&height), "hazel {k}: {height}");
            let tangle = bramble(&mut dice, &leaf(), Vec3::ONE);
            let height = tangle.leaves.as_ref().expect("leafy")[0].height();
            assert!((0.5..2.0).contains(&height), "bramble {k}: {height}");
            let dead = snag(&mut dice, k % 2 == 0, Vec3::ONE);
            let height = dead.trunk.as_ref().expect("solid").height();
            assert!((1.8..10.5).contains(&height), "snag {k}: {height}");
        }
    }

    #[test]
    fn a_shrinking_copy_carries_each_pieces_offset_from_its_middle() {
        let mut build = Build::default();
        for x in [0.0, 5.0] {
            for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                build.vertex(
                    Vec3::new(x + u, v, 0.0),
                    Vec3::Z,
                    Vec2::ZERO,
                    Vec2::ZERO,
                    Vec3::ONE,
                );
            }
            let b = build.positions.len() as u32 - 4;
            build.indices.extend([b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        let mesh = shrinking(&build.mesh(false));
        let Some(VertexAttributeValues::Float32x4(colours)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("no colours");
        };
        assert_eq!(colours[0], [-0.5, -0.5, 0.0, 1.0]);
        assert_eq!(colours[6], [0.5, 0.5, 0.0, 1.0]);
    }
}
