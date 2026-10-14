//! The lush forest's kinds of tree, grown by rules as SpeedTree grows them: an oak of
//! crooked great limbs with its leaves in masses along its outer branches, a Scots pine
//! with a long clean bole under a high, ragged crown, and a Norway spruce, a cone of
//! drooping whorls to the ground. Each kind is grown as several variants (D182), told
//! apart by form, height and the luck of their branching (docs/biomes.md).
//!
//! What made the first oak read as "dumb", and what each rule here answers (D235):
//!
//! - **One fork.** The trunk split into all its limbs at one point, a slingshot. Here the
//!   trunk runs on up into the crown and the great limbs leave it one by one, spiralling.
//! - **A ball of leaves to the ground.** Clumps hung everywhere, crowns met the grass. A
//!   forest oak has a clear bole, its crown starting a third of the way up.
//! - **Leaves the size of plates.** Cards two to four metres across carried twelve leaves
//!   each. A card here is a branch tip of forty smaller leaves, about 1.4 m across.
//! - **Clumps only at tips, lit as one ball.** Here leaves gather along the outer branches
//!   in masses, one a great limb, with gaps between them, each mass lit from its own middle
//!   as well as the crown's.

use super::{Card, Leaf, Limb, Tree};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};
use terra_gen::Rng;

/// What a tree is: its habit, bark and leaves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Species {
    Oak,
    Pine,
    Spruce,
}

/// How an oak grew: tall and narrow among others, broad where it had room, or forked
/// low into two stems.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    Tall,
    Broad,
    Forked,
}

/// Rotates `v` about `axis` by `angle`.
fn turned(v: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    Quat::from_axis_angle(axis.normalize_or(Vec3::X), angle) * v
}

/// A direction level with the ground at an angle round the trunk.
fn round(angle: f32) -> Vec3 {
    Vec3::new(libm::cosf(angle), 0.0, libm::sinf(angle))
}

/// A line grown from a point: a few steps, each turned a little at random and, now and
/// then, kinked, as an oak's limbs are; `lift` bends it toward the light along the way, a
/// share of each step, and `sag` down under its weight near its foot.
#[allow(clippy::too_many_arguments)]
fn line(
    dice: &mut Rng,
    from: Vec3,
    heading: Vec3,
    length: f32,
    radius: f32,
    taper: f32,
    steps: usize,
    wander: f32,
    kink: f32,
    lift: f32,
    sag: f32,
) -> (Vec<Vec3>, Vec<f32>, Vec<Vec3>) {
    let mut direction = heading.normalize_or(Vec3::Y);
    let mut at = from;
    let (mut points, mut radii, mut directions) = (vec![at], vec![radius], vec![direction]);
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let across = direction.any_orthonormal_vector();
        let axis = turned(across, direction, dice.range(0.0, TAU));
        if dice.unit() < kink {
            direction = turned(direction, axis, dice.range(0.22, 0.45));
        } else {
            direction = (direction + dice.unit_vector() * wander).normalize();
        }
        direction = (direction + Vec3::Y * lift * t - Vec3::Y * sag * (1.0 - t)).normalize();
        at += direction * length / steps as f32;
        points.push(at);
        radii.push(radius * (1.0 - taper * t));
        directions.push(direction);
    }
    (points, radii, directions)
}

/// The point and radius a distance along a line of points, as a share of its length.
fn along(points: &[Vec3], radii: &[f32], t: f32) -> (Vec3, f32, Vec3) {
    let s = t.clamp(0.0, 1.0) * (points.len() - 1) as f32;
    let i = (s as usize).min(points.len() - 2);
    let f = s - i as f32;
    (
        points[i].lerp(points[i + 1], f),
        radii[i] + (radii[i + 1] - radii[i]) * f,
        (points[i + 1] - points[i]).normalize_or(Vec3::Y),
    )
}

/// The foot of a trunk flares into its roots.
fn flare(points: &[Vec3], radii: &mut [f32], strength: f32) {
    for (point, r) in points.iter().zip(radii.iter_mut()) {
        *r *= 1.0 + strength * libm::expf(-point.y / 0.9);
    }
}

impl Tree {
    fn new(species: Species, trunk_m: f32) -> Self {
        Self {
            limbs: Vec::new(),
            tips: Vec::new(),
            branch_of: Vec::new(),
            branches: 0,
            trunk_m,
            along: Vec::new(),
            mass_of: Vec::new(),
            masses: Vec::new(),
            species,
        }
    }

    fn limb(&mut self, points: Vec<Vec3>, radii: Vec<f32>, depth: u32) {
        self.limbs.push(Limb {
            points,
            radii,
            depth,
            pointed: depth > 0,
        });
    }

    /// A clump of leaves at a point, its twig running `along`, in a branch's lump (for
    /// the far crown and its shadow) and a mass of the crown (for its light).
    fn clump(&mut self, at: Vec3, size: f32, along: Vec3, branch: usize, mass: usize) {
        self.tips.push((at, size));
        self.along.push(along.normalize_or(Vec3::Y));
        self.branch_of.push(branch);
        self.mass_of.push(mass);
        self.branches = self.branches.max(branch + 1);
    }

    /// Takes away every clump lower than `y`.
    fn bare_below(&mut self, y: f32) {
        let keep: Vec<bool> = self.tips.iter().map(|(at, _)| at.y >= y).collect();
        let mut k = keep.iter();
        self.tips.retain(|_| *k.next().unwrap_or(&true));
        let mut k = keep.iter();
        self.along.retain(|_| *k.next().unwrap_or(&true));
        let mut k = keep.iter();
        self.branch_of.retain(|_| *k.next().unwrap_or(&true));
        let mut k = keep.iter();
        self.mass_of.retain(|_| *k.next().unwrap_or(&true));
    }

    /// Each mass's middle, from its clumps.
    fn settle(&mut self) {
        let count = self.mass_of.iter().copied().max().map_or(0, |m| m + 1);
        let mut sums = vec![(Vec3::ZERO, 0.0f32); count];
        for ((at, _), &mass) in self.tips.iter().zip(&self.mass_of) {
            sums[mass].0 += *at;
            sums[mass].1 += 1.0;
        }
        self.masses = sums.into_iter().map(|(sum, n)| sum / n.max(1.0)).collect();
    }
}

/// An oak, `height` tall. A forest oak's trunk runs up into its crown, nearly to the top,
/// putting out great limbs one by one in a spiral, low ones reaching out, high ones
/// climbing; each limb is crooked, and branches along its length, and those branch again
/// into twigs; leaves are in clumps along the outer branches and at their tips, each great
/// limb's a mass of its own. The crown's outline bulges in lobes, a few to a tree.
pub fn oak(dice: &mut Rng, height: f32, form: Form, young: bool) -> Tree {
    // Stout, as an oak is beside a pine of its height: a 22 m forest oak is about 1.7 m
    // through at the foot, a broad one 2 m (D247).
    let girth = match (form, young) {
        (_, true) => 0.03,
        (Form::Tall, _) => 0.038,
        (Form::Broad, _) => 0.046,
        (Form::Forked, _) => 0.042,
    };
    let mut tree = Tree::new(Species::Oak, height * girth * dice.range(0.9, 1.1));
    let r0 = tree.trunk_m;
    let bole = height
        * match (form, young) {
            (_, true) => dice.range(0.22, 0.32),
            (Form::Tall, _) => dice.range(0.27, 0.35),
            (Form::Broad, _) => dice.range(0.16, 0.22),
            (Form::Forked, _) => dice.range(0.22, 0.28),
        };
    // A crown as wide as the tree is tall or more, as an oak's is, not a ball on a stick.
    let reach = height
        * match (form, young) {
            (_, true) => dice.range(0.34, 0.42),
            (Form::Tall, _) => dice.range(0.3, 0.36),
            (Form::Broad, _) => dice.range(0.4, 0.47),
            (Form::Forked, _) => dice.range(0.34, 0.4),
        };
    // The side it found most light on, which it leans to and reaches farthest toward, so
    // no crown is round about its trunk.
    let light = round(dice.range(0.0, TAU));
    let favour = dice.range(0.25, 0.5);
    // Where the trunk gives out into the crown's top.
    let lead = bole + (height - bole) * dice.range(0.45, 0.6);
    // The crown bulges in a few lobes, so no two outlines are alike.
    let lobes: Vec<(Vec3, f32)> = (0..3)
        .map(|_| (round(dice.range(0.0, TAU)), dice.range(0.15, 0.4)))
        .collect();
    let lobe = |direction: Vec3| {
        let flat = Vec3::new(direction.x, 0.0, direction.z).normalize_or(Vec3::X);
        (0.85
            + lobes
                .iter()
                .map(|(towards, gain)| gain * libm::powf(flat.dot(*towards).max(0.0), 3.0))
                .sum::<f32>())
            * (1.0 + favour * flat.dot(light))
    };
    let clump_m = if young { 1.05 } else { 1.45 };

    // The trunk, or for a forked oak the trunk to its fork and two stems from there,
    // leaning toward its light and wandering more as it rises.
    let lean = (light + round(dice.range(0.0, TAU)) * 0.5).normalize() * dice.range(0.03, 0.1);
    let fork = if form == Form::Forked && !young {
        Some(bole + dice.range(0.3, 1.2))
    } else {
        None
    };
    let top = fork.unwrap_or(lead);
    let steps = ((top / 1.0).ceil() as usize).clamp(4, 14);
    let (mut points, mut radii) = (vec![Vec3::ZERO], vec![r0]);
    let mut direction = (Vec3::Y + lean).normalize();
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let wobble = Vec3::new(dice.range(-1.0, 1.0), 0.0, dice.range(-1.0, 1.0));
        direction = (direction + wobble * (0.03 + 0.08 * t) + Vec3::Y * 0.03).normalize();
        let at = points[step - 1] + direction * top / steps as f32 / direction.y.max(0.5);
        points.push(at);
        let share = at.y / lead;
        radii.push(r0 * (1.0 - 0.62 * libm::powf(share.clamp(0.0, 1.0), 0.8)));
    }
    flare(&points, &mut radii, if young { 0.7 } else { 1.1 });
    let stems: Vec<(Vec<Vec3>, Vec<f32>)> = if let Some(fork) = fork {
        // Two stems from the fork, each about as thick as the other, parting.
        let foot = *points.last().unwrap_or(&Vec3::Y);
        let r = *radii.last().unwrap_or(&r0);
        let apart = dice.range(0.0, TAU);
        let mut stems = Vec::new();
        for side in [0.0, PI] {
            let out = round(apart + side + dice.range(-0.3, 0.3));
            let heading = (Vec3::Y + out * dice.range(0.35, 0.6)).normalize();
            let length = (lead - fork) * dice.range(1.0, 1.25);
            let (p, mut rr, _) = line(
                dice,
                foot,
                heading,
                length,
                r * 0.78,
                0.45,
                7,
                0.08,
                0.2,
                0.06,
                0.0,
            );
            rr[0] = r * 0.82;
            stems.push((p, rr));
        }
        stems
    } else {
        Vec::new()
    };
    tree.limb(points.clone(), radii.clone(), 0);
    let mut stems_all = vec![(points, radii)];
    for (p, r) in stems {
        tree.limb(p.clone(), r.clone(), 0);
        stems_all.push((p, r));
    }
    // The stems the limbs grow from: the trunk, or above a fork the two stems.
    let sources: Vec<usize> = if fork.is_some() { vec![1, 2] } else { vec![0] };

    // A few dead stubs on the bole, where lower limbs were shaded out and fell.
    for _ in 0..1 + dice.below(3) {
        let (trunk, trunk_r) = (&stems_all[0].0, &stems_all[0].1);
        let t = dice.range(0.25, 0.85) * bole / top.max(1.0);
        let (at, r, _) = along(trunk, trunk_r, t);
        let out = round(dice.range(0.0, TAU));
        let heading = (out + Vec3::Y * dice.range(-0.1, 0.4)).normalize();
        let length = dice.range(0.3, 0.8);
        let (p, rr, _) = line(
            dice,
            at + out * r * 0.6,
            heading,
            length,
            (r * 0.16).max(0.04),
            0.5,
            2,
            0.2,
            0.0,
            0.0,
            0.0,
        );
        tree.limb(p, rr, 2);
    }

    // The great limbs, in a spiral up the stems, and the trunk's own top.
    let count = match (form, young) {
        (_, true) => 4 + dice.below(2),
        (Form::Tall, _) => 6 + dice.below(3),
        (Form::Broad, _) => 7 + dice.below(3),
        (Form::Forked, _) => 4 + dice.below(2),
    } as usize;
    let mut turn = dice.range(0.0, TAU);
    let mut mass = 0;
    for &source in &sources {
        let (stem, stem_r) = (stems_all[source].0.clone(), stems_all[source].1.clone());
        let stem_top = stem.last().map_or(lead, |p| p.y);
        let stem_foot = stem[0].y;
        let first = if source == 0 { bole } else { stem_foot + 0.6 };
        // A grown oak's lowest one or two limbs are its greatest: thick, near level and
        // reaching out past the rest, as an oak's do where it once had room.
        let great = if source == 0 && !young && form != Form::Forked {
            1 + dice.below(2) as usize
        } else {
            0
        };
        for k in 0..count {
            // Unevenly up the stem, not a limb every so many metres.
            let s = (k as f32 + dice.range(0.05, 0.95)) / count as f32;
            let s = if k < great { s * 0.4 } else { s };
            let y = first + (stem_top - first) * s * 0.95;
            let t = ((y - stem_foot) / (stem_top - stem_foot).max(0.1)).clamp(0.0, 1.0);
            let (at, r, _) = along(&stem, &stem_r, t);
            let crown_t = ((y - bole) / (height - bole)).clamp(0.0, 1.0);
            turn += 2.4 + dice.range(-0.7, 0.7);
            // More limbs on the side it found light on.
            let spiral = (round(turn) + light * favour * 0.8).normalize();
            let out = if source == 0 {
                spiral
            } else {
                // Above a fork, each stem's limbs lean away from the other stem.
                let away = Vec3::new(at.x, 0.0, at.z).normalize_or(spiral);
                (spiral + away * 0.9).normalize()
            };
            let rise = if k < great {
                dice.range(0.15, 0.35)
            } else {
                let climb = match form {
                    Form::Broad => 0.2 + 0.7 * crown_t,
                    _ => 0.35 + 0.65 * crown_t,
                };
                climb + dice.range(-0.15, 0.15)
            };
            let heading = (out * libm::cosf(rise) + Vec3::Y * libm::sinf(rise)).normalize();
            let wide = reach
                * lobe(heading)
                * dice.range(0.8, 1.1)
                * (1.0 - 0.45 * crown_t * crown_t)
                * if k < great { 1.15 } else { 1.0 };
            let mut length = wide / libm::cosf(rise).max(0.4);
            length = length.min((height * 0.97 - y) / (libm::sinf(rise) * 0.85 + 0.25));
            length = length.max(height * 0.12);
            let thickness = r * if k < great {
                dice.range(0.62, 0.72)
            } else {
                dice.range(0.5, 0.64)
            };
            limb(
                dice,
                &mut tree,
                at,
                heading,
                length,
                thickness,
                clump_m,
                mass,
                rise < 0.6 && k >= great,
            );
            mass += 1;
        }
        // The stem's own top runs on up into the crown.
        let (at, r, direction) = along(&stem, &stem_r, 1.0);
        let heading = (direction + Vec3::Y).normalize();
        let length = (height - at.y) * dice.range(0.75, 0.9);
        limb(
            dice,
            &mut tree,
            at,
            heading,
            length.max(1.0),
            r * 0.85,
            clump_m,
            mass,
            false,
        );
        mass += 1;
    }
    // A great limb's branches that sag toward the ground lose their leaves there, shaded,
    // as a forest oak's lowest do.
    tree.bare_below((bole * 0.7).max(height * 0.16));
    tree.settle();
    tree
}

/// A great limb of an oak and all it carries: crooked, branching along its length into
/// branches that themselves put out twigs, leaves in clumps at their ends and along the
/// outer branches, all one mass. Its branches fall into three lumps far off.
#[allow(clippy::too_many_arguments)]
fn limb(
    dice: &mut Rng,
    tree: &mut Tree,
    from: Vec3,
    heading: Vec3,
    length: f32,
    radius: f32,
    clump_m: f32,
    mass: usize,
    low: bool,
) {
    let sag = if low { 0.07 } else { 0.0 };
    let (points, radii, directions) = line(
        dice, from, heading, length, radius, 0.78, 7, 0.1, 0.35, 0.07, sag,
    );
    let lump = tree.branches;
    let count = ((length / 1.3).round() as usize).clamp(3, 9);
    let mut side = 1.0;
    for k in 0..count {
        let t = 0.18 + 0.82 * (k as f32 + dice.range(0.1, 0.9)) / count as f32;
        let (at, r, direction) = along(&points, &radii, t);
        side = -side;
        let flat = direction.cross(Vec3::Y).normalize_or(Vec3::X) * side;
        let heading = (direction * 0.55
            + flat * 0.85
            + Vec3::Y * dice.range(0.05, 0.45)
            + dice.unit_vector() * 0.15)
            .normalize();
        let branch_length = (length * dice.range(0.32, 0.5) * (1.15 - 0.55 * t)).max(1.1);
        let branch = lump + (k * 3 / count).min(2);
        twigs(
            dice,
            tree,
            at,
            heading,
            branch_length,
            (r * 0.55).max(0.03),
            clump_m,
            branch,
            mass,
        );
    }
    // The limb's own end, a clump.
    let end = points[points.len() - 1];
    tree.clump(
        end,
        clump_m * dice.range(0.9, 1.15),
        directions[directions.len() - 1],
        lump + 2,
        mass,
    );
    tree.limb(points, radii, 1);
}

/// A branch off a great limb, with its twigs and clumps.
#[allow(clippy::too_many_arguments)]
fn twigs(
    dice: &mut Rng,
    tree: &mut Tree,
    from: Vec3,
    heading: Vec3,
    length: f32,
    radius: f32,
    clump_m: f32,
    branch: usize,
    mass: usize,
) {
    let (points, radii, directions) = line(
        dice, from, heading, length, radius, 0.7, 4, 0.22, 0.25, 0.1, 0.0,
    );
    let count = 2 + dice.below(3);
    for _ in 0..count {
        let t = dice.range(0.35, 1.0);
        let (at, r, direction) = along(&points, &radii, t);
        let heading = (direction * 0.5 + dice.unit_vector() * 0.8 + Vec3::Y * 0.2).normalize();
        let twig = length * dice.range(0.35, 0.6);
        let (p, rr, d) = line(
            dice,
            at,
            heading,
            twig,
            (r * 0.5).max(0.015),
            0.7,
            3,
            0.3,
            0.0,
            0.1,
            0.0,
        );
        tree.clump(
            p[p.len() - 1],
            clump_m * dice.range(0.85, 1.1),
            d[d.len() - 1],
            branch,
            mass,
        );
        tree.limb(p, rr, 3);
    }
    tree.clump(
        points[points.len() - 1],
        clump_m * dice.range(0.9, 1.15),
        directions[directions.len() - 1],
        branch,
        mass,
    );
    if length > 2.0 {
        let (at, _, direction) = along(&points, &radii, 0.55);
        tree.clump(at, clump_m * dice.range(0.75, 0.9), direction, branch, mass);
    }
    tree.limb(points, radii, 2);
}

/// Where a modelled tree's leaf clump goes, as tools/blender/tips.py finds them along its
/// outer branches: its place, the way its branch runs there, its size and its mass.
pub type Tip = ((f32, f32, f32), (f32, f32, f32), f32, usize);

/// An oak whose trunk and limbs are a model: only its clumps, from where its branches run
/// out, each mass also one lump far off (D235).
pub fn modelled(tips: &[Tip], trunk_m: f32) -> Tree {
    let mut tree = Tree::new(Species::Oak, trunk_m);
    for &((x, y, z), (dx, dy, dz), size, mass) in tips {
        tree.clump(
            Vec3::new(x, y, z),
            size * 1.25,
            Vec3::new(dx, dy, dz),
            mass,
            mass,
        );
    }
    tree.settle();
    tree
}

/// A conifer, `height` tall: a trunk straight to the top, branches off it in whorls. A
/// Scots pine sheds its lower branches, its bole long and clean under a high crown that
/// widens and then rounds off, its branches climbing and turning up, sparse, needles in
/// tufts along their outer half. A Norway spruce keeps them nearly to the ground: a cone of
/// whorls longest at the foot, drooping, tips turned up, needles all along.
pub fn conifer(dice: &mut Rng, height: f32, species: Species, young: bool) -> Tree {
    let pine = species == Species::Pine;
    let mut tree = Tree::new(
        species,
        height * if pine { 0.021 } else { 0.024 } * dice.range(0.9, 1.1),
    );
    let r0 = tree.trunk_m;
    let steps = 16;
    // A pine leans, and sweeps: its lean grows up the bole, as a trunk that once bent toward
    // the light and righted itself; a spruce stands nearly straight (D247).
    let lean = round(dice.range(0.0, TAU))
        * if pine {
            dice.range(0.03, 0.1)
        } else {
            dice.range(0.0, 0.025)
        };
    let (mut points, mut radii) = (vec![Vec3::ZERO], vec![r0]);
    let mut at = Vec3::ZERO;
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let wobble = Vec3::new(dice.range(-1.0, 1.0), 0.0, dice.range(-1.0, 1.0))
            * if pine { 0.05 } else { 0.015 };
        let sweep = if pine { 0.4 + 1.2 * t } else { 1.0 };
        at = Vec3::new(at.x, 0.0, at.z)
            + (lean * sweep + wobble) * height / steps as f32 * 0.4
            + Vec3::Y * height * t;
        points.push(at);
        // Nearly a cylinder up the bole, tapering fast in the crown, not a straight cone.
        radii.push((r0 * (1.0 - 0.5 * t - 0.45 * libm::powf(t, 2.5))).max(0.03));
    }
    flare(&points, &mut radii, if pine { 0.8 } else { 0.6 });
    let trunk_at = |y: f32| along(&points, &radii, y / height);
    let base = height
        * match (pine, young) {
            (true, false) => dice.range(0.55, 0.66),
            (true, true) => dice.range(0.3, 0.4),
            // In a stand a spruce's lowest branches are shaded out and die, standing on as
            // bare grey twigs below its live crown (D247).
            (false, false) => dice.range(0.2, 0.32),
            (false, true) => dice.range(0.02, 0.06),
        };
    let reach = height
        * if pine {
            dice.range(0.13, 0.17)
        } else {
            dice.range(0.16, 0.2)
        };
    // A spruce's whorls a little apart, so its crown reads as tiers.
    let spacing = if pine {
        dice.range(0.9, 1.3)
    } else {
        dice.range(0.75, 1.0)
    } * if young { 0.6 } else { 1.0 };
    let clump_m = if young { 0.6 } else { 0.8 };
    let bands = if pine { 5 } else { 8 };

    // Dead stubs up a pine's clean bole.
    if pine && !young {
        for _ in 0..4 + dice.below(6) {
            let y = dice.range(0.3, 0.98) * base;
            let (at, r, _) = trunk_at(y);
            let out = round(dice.range(0.0, TAU));
            let heading = (out + Vec3::Y * dice.range(-0.4, 0.1)).normalize();
            let length = dice.range(0.3, 1.0);
            let (p, rr, _) = line(
                dice,
                at + out * r * 0.5,
                heading,
                length,
                0.05,
                0.6,
                2,
                0.1,
                0.0,
                0.0,
                0.0,
            );
            tree.limb(p, rr, 2);
        }
    }

    // A spruce's dead lower branches: whorls of thin bare twigs, short and broken, drooping,
    // each with a few twigs of its own off it, a tangle rather than spokes (D247).
    if !pine && !young {
        let mut y = dice.range(1.2, 2.0);
        while y < base - 0.3 {
            let (at, r, _) = trunk_at(y);
            let turn = dice.range(0.0, TAU);
            let count = 4 + dice.below(4);
            for k in 0..count {
                let out = round(turn + k as f32 * TAU / count as f32 + dice.range(-0.4, 0.4));
                let heading = (out + Vec3::Y * dice.range(-0.45, 0.0)).normalize();
                let length = dice.range(0.3, 1.0) * (0.6 + 0.4 * y / base) * reach * 0.5;
                let (p, rr, _) = line(
                    dice,
                    at + out * r * 0.8,
                    heading,
                    length.max(0.3),
                    (r * 0.05).max(0.012),
                    0.8,
                    3,
                    0.25,
                    0.3,
                    0.0,
                    0.05,
                );
                for _ in 0..1 + dice.below(3) {
                    let (from, rf, direction) = along(&p, &rr, dice.range(0.3, 0.8));
                    let side = direction.cross(Vec3::Y).normalize_or(Vec3::X)
                        * if dice.unit() < 0.5 { -1.0 } else { 1.0 };
                    let twig = length * dice.range(0.2, 0.4);
                    let (q, qr, _) = line(
                        dice,
                        from,
                        (direction + side * 0.9 - Vec3::Y * 0.2).normalize(),
                        twig,
                        rf * 0.6,
                        0.8,
                        2,
                        0.3,
                        0.0,
                        0.0,
                        0.0,
                    );
                    tree.limb(q, qr, 2);
                }
                tree.limb(p, rr, 2);
            }
            y += spacing * dice.range(0.5, 1.0);
        }
    }

    let mut y = base;
    let mut turn = dice.range(0.0, TAU);
    while y < height - spacing * 0.5 {
        let t = (y - base) / (height - base);
        let count = if pine {
            4 + dice.below(3)
        } else {
            5 + dice.below(3)
        };
        let (at, r, _) = trunk_at(y);
        let band = ((t * bands as f32) as usize).min(bands - 1);
        for k in 0..count {
            // A pine's crown is ragged: a branch here and there never grew.
            if pine && dice.unit() < 0.15 {
                continue;
            }
            let around = turn + k as f32 / count as f32 * TAU + dice.range(-0.35, 0.35);
            let out = round(around);
            let (length, rise, droop, upturn) = if pine {
                (
                    reach
                        * libm::powf(libm::sinf(PI * (0.18 + 0.72 * t)), 0.7)
                        * dice.range(0.75, 1.15)
                        + 0.3,
                    dice.range(0.05, 0.35) + 0.45 * t,
                    -0.06,
                    0.25,
                )
            } else {
                // Out level or a little down, sagging under its own weight along its
                // length, its tip turned up: a spruce's drooping layered whorls (D247).
                (
                    reach * libm::powf(1.0 - t, 0.85) * dice.range(0.85, 1.1) + 0.35,
                    -0.12 + 0.5 * t + dice.range(-0.1, 0.1),
                    0.16,
                    0.45,
                )
            };
            let heading = (out * libm::cosf(rise) + Vec3::Y * libm::sinf(rise)).normalize();
            let thickness = (r * if pine { 0.33 } else { 0.3 }).max(0.025);
            let (mut p, rr, mut d) = line(
                dice, at, heading, length, thickness, 0.8, 4, 0.08, 0.0, 0.0, droop,
            );
            // The tip turns up toward the light.
            if let (Some(last), Some(dir)) = (p.len().checked_sub(1), d.last().copied()) {
                let up = (dir + Vec3::Y * upturn).normalize();
                p[last] = p[last - 1] + up * length / 4.0;
                d[last] = up;
            }
            let half = (around.rem_euclid(TAU) > PI) as usize;
            let branch = band * 2 + half;
            let gap = clump_m * if pine { 0.85 } else { 0.75 };
            let start = length * if pine { 0.35 } else { 0.25 };
            let mut s = start + dice.range(0.0, gap * 0.5);
            while s < length {
                let (point, _, direction) = along(&p, &rr, s / length);
                tree.clump(
                    point,
                    clump_m * dice.range(0.85, 1.1),
                    direction,
                    branch,
                    branch,
                );
                s += gap;
            }
            tree.clump(p[p.len() - 1], clump_m, d[d.len() - 1], branch, branch);
            // A pine's branch forks once or twice toward its end, filling out its tuft.
            if pine && length > 1.5 {
                for _ in 0..1 + dice.below(2) {
                    let (from, rf, direction) = along(&p, &rr, dice.range(0.45, 0.8));
                    let side = direction.cross(Vec3::Y).normalize_or(Vec3::X)
                        * if dice.unit() < 0.5 { -1.0 } else { 1.0 };
                    let heading = (direction + side * 0.9 + Vec3::Y * 0.3).normalize();
                    let (q, qr, qd) = line(
                        dice,
                        from,
                        heading,
                        length * 0.35,
                        rf * 0.6,
                        0.7,
                        2,
                        0.1,
                        0.0,
                        0.1,
                        0.0,
                    );
                    tree.clump(
                        q[q.len() - 1],
                        clump_m * 0.9,
                        qd[qd.len() - 1],
                        branch,
                        branch,
                    );
                    tree.limb(q, qr, 2);
                }
            }
            tree.limb(p, rr, 1);
        }
        y += spacing * dice.range(0.8, 1.2);
        turn += dice.range(0.5, 1.2);
    }
    // The leading shoot.
    let top = points[points.len() - 1];
    let branch = (bands - 1) * 2;
    for k in 0..3 {
        let out = round(k as f32 * 2.1);
        tree.clump(
            top - Vec3::Y * clump_m * 0.4 * k as f32,
            clump_m * 0.9,
            (Vec3::Y + out * 0.3).normalize(),
            branch,
            branch,
        );
    }
    tree.limb(points, radii, 0);
    tree.settle();
    tree
}

/// How a crown's cards are lit and moved: its middle, which its cards' normals lean out of
/// with their own mass's, how far it reaches, and whether it leans out from its trunk, as a
/// cone does.
struct Crown {
    centre: Vec3,
    reach: f32,
    height: f32,
    axial: bool,
}

/// A crown's cards: for each clump, its cards in the order the levels of detail keep them
/// (D221, D223). An oak's lie along their twig, its stalk toward the branch and its leaves
/// outward, turned every way but mostly facing out of their mass; a conifer's lie along its
/// branch, fanning out to the sides and up, a spruce's hanging too.
pub fn crown_cards(dice: &mut Rng, tree: &Tree, height: f32, leaf: Leaf) -> Vec<Vec<Card>> {
    let tips = &tree.tips;
    let centre =
        tips.iter().fold(Vec3::ZERO, |sum, (tip, _)| sum + *tip) / tips.len().max(1) as f32;
    let crown = Crown {
        centre,
        reach: tips
            .iter()
            .map(|(tip, _)| Vec2::new(tip.x, tip.z).distance(Vec2::new(centre.x, centre.z)))
            .fold(1.0, f32::max),
        height,
        axial: tree.species != Species::Oak,
    };
    // Each mass a slightly different green.
    let hues: Vec<Vec3> = tree
        .masses
        .iter()
        .map(|_| {
            let h = dice.range(-1.0, 1.0);
            Vec3::new(1.0 + 0.05 * h, 1.0 + 0.02 * h, 1.0 - 0.08 * h)
        })
        .collect();
    tips.iter()
        .enumerate()
        .map(|(i, &(tip, size))| {
            let mass = tree.masses[tree.mass_of[i]];
            let hue = hues[tree.mass_of[i]];
            let along = tree.along[i];
            let count = match tree.species {
                Species::Oak => 7 + dice.below(4),
                Species::Pine => 4 + dice.below(2),
                Species::Spruce => 3 + dice.below(3),
            };
            (0..count)
                .map(|k| {
                    card(
                        dice,
                        tree.species,
                        &crown,
                        mass,
                        hue,
                        tip,
                        along,
                        size,
                        k,
                        leaf,
                    )
                })
                .collect()
        })
        .collect()
}

/// One card of a clump at `tip`.
#[allow(clippy::too_many_arguments)]
fn card(
    dice: &mut Rng,
    species: Species,
    crown: &Crown,
    mass: Vec3,
    hue: Vec3,
    tip: Vec3,
    along: Vec3,
    size: f32,
    rank: u32,
    leaf: Leaf,
) -> Card {
    let out_crown = if crown.axial {
        (Vec3::new(tip.x, 0.0, tip.z).normalize_or(Vec3::X) + Vec3::Y * 0.35).normalize()
    } else {
        (tip - crown.centre).normalize_or(Vec3::Y)
    };
    let (base, up, facing, half) = match species {
        Species::Oak => {
            let base = tip + dice.unit_vector() * size * dice.range(0.1, 0.55);
            let out_mass = (base - mass).normalize_or(out_crown);
            let up = (along * 0.75 + dice.unit_vector() * 0.7 + out_mass * 0.45).normalize();
            let wanted = out_mass * 0.6 + Vec3::Y * 0.35 + dice.unit_vector() * 0.55;
            let facing = (wanted - up * wanted.dot(up)).normalize_or(up.any_orthonormal_vector());
            (base, up, facing, size * dice.range(0.48, 0.62))
        }
        _ => {
            let base = tip + dice.unit_vector() * 0.12;
            let side = along.cross(Vec3::Y).normalize_or(Vec3::X);
            let spread = if rank == 0 { 0.15 } else { 0.8 };
            let mut up =
                (along + side * dice.range(-spread, spread) + Vec3::Y * dice.range(0.05, 0.45))
                    .normalize();
            // A spruce's side shoots hang from its branches like curtains.
            if species == Species::Spruce && rank >= 2 {
                up = (along * 0.45 - Vec3::Y * 0.75 + side * dice.range(-0.5, 0.5)).normalize();
            }
            let wanted = Vec3::Y * 0.7 + out_crown * 0.3 + dice.unit_vector() * 0.3;
            let facing = (wanted - up * wanted.dot(up)).normalize_or(up.any_orthonormal_vector());
            (base, up, facing, size * dice.range(0.62, 0.78))
        }
    };
    // Its stalk at the branch, its leaves running out from there.
    let middle = base + up * half * 0.75;
    let right = up.cross(facing);
    let out_mass = (middle - mass).normalize_or(out_crown);
    let normal = match species {
        Species::Oak => (out_mass * 0.45 + out_crown * 0.35 + facing * 0.2).normalize(),
        _ => (out_crown * 0.55 + Vec3::Y * 0.25 + facing * 0.2).normalize(),
    };
    // Leaves deep in their mass and in the crown, and low in it, are shaded by the rest.
    let flat = Vec2::new(middle.x - crown.centre.x, middle.z - crown.centre.z).length();
    let rise = (middle.y / crown.height).clamp(0.0, 1.0);
    // A cone narrows to its top, which sees the most sky of all.
    let outer = if crown.axial {
        (flat / (crown.reach * 0.7))
            .max(rise * rise)
            .clamp(0.0, 1.0)
    } else {
        (middle.distance(crown.centre) / (crown.reach * 1.1)).clamp(0.0, 1.0)
    };
    let in_mass = (middle.distance(mass) / (size * 3.0)).clamp(0.0, 1.0);
    let light = 0.44 + 0.22 * outer + 0.16 * in_mass + 0.16 * rise + dice.range(-0.07, 0.07);
    let mut tint = Vec3::new(
        light * dice.range(0.95, 1.05),
        light,
        light * dice.range(0.9, 1.0),
    ) * hue;
    // Here and there a leaf turning.
    if species == Species::Oak && dice.unit() < 0.05 {
        tint *= Vec3::new(1.2, 1.08, 0.65);
    }
    let bend = (middle.y / crown.height).clamp(0.0, 1.0);
    let flutter = if species == Species::Oak {
        dice.range(0.6, 1.0)
    } else {
        dice.range(0.25, 0.5)
    };
    Card {
        middle,
        right,
        up,
        half,
        normal,
        tint,
        sway: Vec2::new(bend * bend, flutter),
        flip: if dice.unit() < 0.5 { -1.0 } else { 1.0 },
        // Deep in the crown it sees about a third of the sky (D247).
        sky: (0.3 + 0.7 * outer.max(in_mass * 0.8)).min(1.0),
        leaf,
    }
}

#[cfg(test)]
mod tests {
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
        let pine = conifer(&mut dice, 28.0, Species::Pine, false);
        let spruce = conifer(&mut dice, 24.0, Species::Spruce, false);
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
        assert!(pine.branches <= 10 && spruce.branches <= 16);
    }
}
