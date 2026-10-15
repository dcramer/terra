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

/// How a tree grew. An oak: tall and narrow among others, broad where it had room, or
/// forked low into two stems. A Scots pine (D276): its leader running on to a pointed top;
/// broad, an old pine's flat umbrella of long level branches; forked high into two leaders;
/// or broken, its top snapped off and a branch below turned up to lead in its place.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    Tall,
    Broad,
    Forked,
    Broken,
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

/// The point and radius of a rising line of points at a height, between the two points
/// either side of it; at an end below or above it, that end.
pub(super) fn by_height(points: &[Vec3], radii: &[f32], y: f32) -> (Vec3, f32) {
    for i in 1..points.len() {
        let (a, b) = (points[i - 1], points[i]);
        if y <= b.y || i == points.len() - 1 {
            let f = ((y - a.y) / (b.y - a.y).max(1e-4)).clamp(0.0, 1.0);
            return (a.lerp(b, f), radii[i - 1] + (radii[i] - radii[i - 1]) * f);
        }
    }
    (points[0], radii[0])
}

/// The foot of a trunk flares a little all round; its roots swell out of the flare as
/// ridges of their own (lush.rs, `Shape`), with the trunk drawn in between them, so the
/// foot is not one smooth skirt (D264).
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
            dead: Vec::new(),
            plates: Vec2::new(0.3, 1.0),
        }
    }

    /// A limb of dead wood, bark gone grey, as a pine's shed branches and broken top are.
    fn dead_limb(&mut self, points: Vec<Vec3>, radii: Vec<f32>, depth: u32) {
        self.dead.push(self.limbs.len());
        self.limb(points, radii, depth);
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
        (Form::Tall | Form::Broken, _) => 0.038,
        (Form::Broad, _) => 0.046,
        (Form::Forked, _) => 0.042,
    };
    let mut tree = Tree::new(Species::Oak, height * girth * dice.range(0.9, 1.1));
    let r0 = tree.trunk_m;
    let bole = height
        * match (form, young) {
            (_, true) => dice.range(0.22, 0.32),
            (Form::Tall | Form::Broken, _) => dice.range(0.27, 0.35),
            (Form::Broad, _) => dice.range(0.16, 0.22),
            (Form::Forked, _) => dice.range(0.22, 0.28),
        };
    // A crown as wide as the tree is tall or more, as an oak's is, not a ball on a stick.
    let reach = height
        * match (form, young) {
            (_, true) => dice.range(0.34, 0.42),
            (Form::Tall | Form::Broken, _) => dice.range(0.3, 0.36),
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
    flare(&points, &mut radii, if young { 0.4 } else { 0.6 });
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
        (Form::Tall | Form::Broken, _) => 6 + dice.below(3),
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
    // Every great limb bows a little under its own weight, a low one more, most near its foot
    // as a loaded beam does, and its outer part turns up toward the light; it narrows nearly
    // to a twig's width at its end, where a blunt one read as a sawn-off cylinder (D289).
    let sag = if low { 0.13 } else { 0.05 };
    let (points, radii, directions) = line(
        dice, from, heading, length, radius, 0.88, 7, 0.1, 0.35, 0.1, sag,
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
        dice, from, heading, length, radius, 0.84, 5, 0.2, 0.3, 0.12, 0.06,
    );
    // Twigs all along the branch's outer four fifths, evenly rather than where the dice fall,
    // each leafy along its own length as well as at its end, so the crown is leaves between
    // its branches, not clumps at their ends on bare sticks (D309). Two to four at random from
    // its middle out left the inside of a crown seen from below bare and its clumps floating.
    let count = 2 + dice.below(3);
    for k in 0..count {
        let t = 0.2 + 0.8 * (k as f32 + dice.range(0.15, 0.85)) / count as f32;
        let (at, r, direction) = along(&points, &radii, t);
        let heading = (direction * 0.5 + dice.unit_vector() * 0.8 + Vec3::Y * 0.2).normalize();
        let twig = length * dice.range(0.35, 0.6);
        // Thick enough to be seen to carry its leaves from the ground (a centimetre and a half
        // at its end), not a hair (D309).
        let (p, rr, d) = line(
            dice,
            at,
            heading,
            twig,
            (r * 0.5).max(0.025),
            0.55,
            3,
            0.3,
            0.0,
            0.12,
            0.0,
        );
        tree.clump(
            p[p.len() - 1],
            clump_m * dice.range(0.85, 1.1),
            d[d.len() - 1],
            branch,
            mass,
        );
        if twig > 1.3 {
            let (at, _, direction) = along(&p, &rr, 0.5);
            tree.clump(
                at,
                clump_m * dice.range(0.65, 0.85),
                direction,
                branch,
                mass,
            );
        }
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
/// tufts along their outer half; each pine has its own girth, lean and sweep, crown depth,
/// reach, outline and lopsidedness, its top as `form` says, and dead branches hanging grey
/// under its live crown (D276). A Norway spruce keeps them nearly to the ground: a cone of
/// whorls longest at the foot, drooping, tips turned up, needles all along.
pub fn conifer(dice: &mut Rng, height: f32, species: Species, form: Form, young: bool) -> Tree {
    let pine = species == Species::Pine;
    // A pine from slender to stout, a quarter of a metre through to over four tenths for
    // every ten of its height, so a 26 m pine is 0.7 to 1.15 m through at its foot, as an old
    // Scots pine is (D276, D309): a third to a half read as redwoods.
    let girth = if pine {
        dice.range(0.013, 0.022)
    } else {
        0.024 * dice.range(0.9, 1.1)
    };
    let mut tree = Tree::new(species, height * girth);
    let r0 = tree.trunk_m;
    // Where its grey-brown bark turns to orange plates, from a fifth of its height to a
    // half, and how orange they are.
    if pine {
        tree.plates = Vec2::new(dice.range(0.22, 0.5), dice.range(0.7, 1.25));
    }
    let steps = 16;
    // A pine leans, and sweeps: its lean grows up the bole, as a trunk that once bent toward
    // the light and righted itself; one in four leans hard, its top a twelfth to a seventh of
    // its height out, and its bole bows a little one way between foot and top (D276). A
    // spruce stands nearly straight (D247).
    let lean_to = round(dice.range(0.0, TAU));
    let lean = lean_to
        * if pine {
            if dice.unit() < 0.25 {
                dice.range(0.08, 0.15)
            } else {
                dice.range(0.015, 0.06)
            }
        } else {
            dice.range(0.0, 0.025)
        };
    let (bow_to, bow) = if pine {
        (round(dice.range(0.0, TAU)), dice.range(0.0, 0.02))
    } else {
        (Vec3::X, 0.0)
    };
    let (mut points, mut radii) = (vec![Vec3::ZERO], vec![r0]);
    let mut at = Vec3::ZERO;
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let wobble = Vec3::new(dice.range(-1.0, 1.0), 0.0, dice.range(-1.0, 1.0))
            * if pine { 0.05 } else { 0.015 };
        let drift = if pine {
            let sweep = 0.4 + 1.2 * t;
            lean * sweep + bow_to * bow * PI * libm::cosf(PI * t) + wobble * 0.4
        } else {
            (lean + wobble) * 0.4
        };
        at = Vec3::new(at.x, 0.0, at.z) + drift * height / steps as f32 + Vec3::Y * height * t;
        points.push(at);
        // Nearly a cylinder up the bole, tapering fast in the crown, not a straight cone.
        radii.push((r0 * (1.0 - 0.5 * t - 0.45 * libm::powf(t, 2.5))).max(0.03));
    }
    flare(&points, &mut radii, if pine { 0.45 } else { 0.35 });
    let grown = pine && !young;
    let flat = grown && form == Form::Broad;
    let base = height
        * match (pine, young) {
            (true, false) if flat => dice.range(0.62, 0.74),
            (true, false) => dice.range(0.5, 0.68),
            (true, true) => dice.range(0.3, 0.4),
            // In a stand a spruce's lowest branches are shaded out and die, standing on as
            // bare grey twigs below its live crown (D247).
            (false, false) => dice.range(0.2, 0.32),
            (false, true) => dice.range(0.02, 0.06),
        };
    let reach = height
        * if pine {
            dice.range(0.11, 0.2) * if flat { 1.25 } else { 1.0 }
        } else {
            dice.range(0.16, 0.2)
        };
    // A young spruce in a wood's shade (D289): its whorls a year's growth apart, with dark
    // gaps between the tiers, some whorls weaker than the next, a branch here and there
    // missing, each bough drooping and turning up at its tip, its needles on its outer part,
    // the inner shed in the shade; grown densely all over it read as a mass of broccoli.
    let sapling = !pine && young;
    // A spruce's whorls a little apart, so its crown reads as tiers.
    let spacing = if pine {
        dice.range(0.9, 1.3)
    } else {
        dice.range(0.75, 1.0)
    } * if sapling {
        0.85
    } else if young {
        0.6
    } else {
        1.0
    };
    let clump_m = if sapling {
        0.55
    } else if young {
        0.6
    } else {
        0.8
    };
    let bands = if pine { 5 } else { 8 };
    // A pine's crown is lopsided toward the side it found light on, and its outline its own:
    // where along the crown its branches are longest, and how sharply they shorten off it.
    let (light, favour, from, sharp) = if pine {
        (
            round(dice.range(0.0, TAU)),
            dice.range(0.0, 0.35),
            dice.range(0.1, 0.3),
            dice.range(0.45, 1.0),
        )
    } else {
        (Vec3::X, 0.0, 0.18, 0.7)
    };

    // Above a fork or a break the crown grows on other stems: two leaders parting from the
    // fork, or a branch below the break turned up to lead, the broken top standing on above
    // it as a dead spike.
    let fork = (grown && form == Form::Forked).then(|| height * dice.range(0.7, 0.82));
    let broken = (grown && form == Form::Broken).then(|| height * dice.range(0.74, 0.86));
    let cut = fork.or(broken).unwrap_or(f32::INFINITY);
    if cut.is_finite() {
        let keep = points.iter().take_while(|p| p.y < cut).count().max(2);
        let (end, r) = by_height(&points, &radii, cut);
        points.truncate(keep);
        radii.truncate(keep);
        points.push(end);
        radii.push(r);
    }
    let mut upper: Vec<(Vec<Vec3>, Vec<f32>)> = Vec::new();
    let split = if let Some(fork) = fork {
        let (top, r) = (points[points.len() - 1], radii[radii.len() - 1]);
        let apart = dice.range(0.0, TAU);
        for side in [0.0, PI] {
            let out = round(apart + side + dice.range(-0.3, 0.3));
            let heading = (Vec3::Y + out * dice.range(0.3, 0.5)).normalize();
            let length = (height - fork) * dice.range(1.0, 1.1);
            let (p, rr, _) = line(
                dice,
                top,
                heading,
                length,
                r * 0.75,
                0.85,
                6,
                0.04,
                0.0,
                0.08,
                0.0,
            );
            upper.push((p, rr));
        }
        fork
    } else if let Some(broken) = broken {
        // Snapped in a jagged point of splintered wood.
        let (top, r) = (points[points.len() - 1], radii[radii.len() - 1]);
        let jag = round(dice.range(0.0, TAU)) * r;
        tree.dead_limb(
            vec![
                top,
                top + Vec3::Y * r * 1.5 + jag,
                top + Vec3::Y * r * 3.5 - jag,
            ],
            vec![r, r * 0.6, r * 0.08],
            1,
        );
        let from = broken - dice.range(0.6, 1.2);
        let (foot, r) = by_height(&points, &radii, from);
        let out = round(dice.range(0.0, TAU));
        let length = (height - from) * dice.range(0.55, 0.8);
        let (p, rr, _) = line(
            dice,
            foot + out * r * 0.5,
            (out * 0.55 + Vec3::Y).normalize(),
            length,
            r * 0.55,
            0.85,
            6,
            0.05,
            0.0,
            0.12,
            0.0,
        );
        upper.push((p, rr));
        from
    } else {
        f32::INFINITY
    };
    // The point and radius at a height on the trunk, or above the split on the upper stem
    // `which`.
    let trunk_top = points[points.len() - 1].y;
    let stem_at = |y: f32, which: usize| -> (Vec3, f32) {
        if y < split || upper.is_empty() {
            by_height(&points, &radii, y)
        } else {
            let (p, r) = &upper[which % upper.len()];
            by_height(p, r, y)
        }
    };
    let crown_top = upper
        .iter()
        .map(|(p, _)| p[p.len() - 1].y)
        .fold(trunk_top, f32::max);

    // Dead stubs up a pine's clean bole, and under its live crown the dead branches it is
    // shedding, bare and grey, drooping, a few twigs still on them (D276).
    if grown {
        // A pine self-prunes from the foot up, so its bole carries the stubs of every whorl
        // it shed: a dozen to twenty, closer toward the crown, most snapped short, a few
        // longer, each as thick as a branch of that whorl was (D309). Four to nine thin sticks
        // read from a few metres as a clean redwood's bole.
        for _ in 0..12 + dice.below(9) {
            let y = libm::sqrtf(dice.range(0.04, 0.96)) * base;
            let (at, r) = stem_at(y, 0);
            let out = round(dice.range(0.0, TAU));
            let heading = (out + Vec3::Y * dice.range(-0.4, 0.15)).normalize();
            // Its drawn tip runs on a few of its widths past this (lush.rs, `rings`).
            let length = if dice.unit() < 0.45 {
                dice.range(0.08, 0.3)
            } else {
                dice.range(0.5, 1.1)
            };
            let thick = (r * dice.range(0.07, 0.12)).max(0.03);
            let (p, rr, _) = line(
                dice,
                at + out * r * 0.5,
                heading,
                length,
                thick,
                0.5,
                2,
                0.1,
                0.0,
                0.0,
                0.0,
            );
            tree.dead_limb(p, rr, 2);
        }
        for _ in 0..4 + dice.below(6) {
            let y = base * dice.range(0.7, 0.99);
            let (at, r) = stem_at(y, 0);
            let out = round(dice.range(0.0, TAU));
            let heading = (out + Vec3::Y * dice.range(-0.35, 0.05)).normalize();
            let length = dice.range(0.8, 2.4) * (0.6 + 0.4 * y / base);
            let (p, rr, _) = line(
                dice,
                at + out * r * 0.6,
                heading,
                length,
                (r * 0.1).max(0.025),
                0.8,
                3,
                0.15,
                0.3,
                0.0,
                0.05,
            );
            for _ in 0..1 + dice.below(3) {
                let (from, rf, direction) = along(&p, &rr, dice.range(0.3, 0.85));
                let side = direction.cross(Vec3::Y).normalize_or(Vec3::X)
                    * if dice.unit() < 0.5 { -1.0 } else { 1.0 };
                let twig = length * dice.range(0.15, 0.3);
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
                tree.dead_limb(q, qr, 2);
            }
            // As thick a limb as a live branch, so the trunk's second level keeps it: a shadow
            // cannot dither, and one gone at once where that level takes over pops.
            tree.dead_limb(p, rr, 1);
        }
    }

    // A spruce's dead lower branches: whorls of thin bare twigs, short and broken, drooping,
    // each with a few twigs of its own off it, a tangle rather than spokes (D247); a third of
    // a metre to under a metre, not sticks reaching as far as the live boughs (D264).
    if !pine && !young {
        let mut y = dice.range(1.2, 2.0);
        while y < base - 0.3 {
            let (at, r) = stem_at(y, 0);
            let turn = dice.range(0.0, TAU);
            let count = 3 + dice.below(3);
            for k in 0..count {
                let out = round(turn + k as f32 * TAU / count as f32 + dice.range(-0.4, 0.4));
                let heading = (out + Vec3::Y * dice.range(-0.45, 0.0)).normalize();
                let length = dice.range(0.15, 0.6) * (0.5 + 0.5 * y / base) * reach * 0.35;
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
            y += spacing * dice.range(0.7, 1.3);
        }
    }

    let mut y = base;
    let mut turn = dice.range(0.0, TAU);
    let mut whorl = 0;
    while y < crown_top - spacing * 0.5 {
        let t = (y - base) / (crown_top - base);
        let count = if pine || sapling {
            4 + dice.below(3)
        } else {
            5 + dice.below(3)
        };
        let (at, r) = stem_at(y, whorl);
        whorl += 1;
        let band = ((t * bands as f32) as usize).min(bands - 1);
        // How well the year it grew in went: a young spruce's tiers are not all alike.
        let vigour = if sapling { dice.range(0.7, 1.15) } else { 1.0 };
        for k in 0..count {
            // A pine's crown is ragged: a branch here and there never grew; a young spruce's
            // too, shaded out.
            if (pine && dice.unit() < 0.15) || (sapling && dice.unit() < 0.2) {
                continue;
            }
            let around = turn + k as f32 / count as f32 * TAU + dice.range(-0.35, 0.35);
            let out = round(around);
            let (length, rise, droop, upturn) = if pine {
                // An old pine's flat top: its highest branches as long as any, near level.
                // Any other grown pine's crown rounds off at its top rather than coming to a
                // spruce's point (D309).
                let profile = if flat {
                    libm::powf(libm::sinf(PI * (0.35 + 0.55 * t)), 0.4)
                } else {
                    libm::powf(libm::sinf(PI * (from + (0.8 - from) * t)), sharp)
                };
                (
                    reach * profile * dice.range(0.75, 1.15) * (1.0 + favour * out.dot(light))
                        + 0.3,
                    if flat {
                        dice.range(0.0, 0.2) + 0.2 * t
                    } else {
                        dice.range(0.05, 0.35) + 0.45 * t
                    },
                    -0.06,
                    0.25,
                )
            } else {
                // Out level or a little down, sagging under its own weight along its
                // length, its tip turned up: a spruce's drooping layered whorls (D247).
                (
                    (reach * libm::powf(1.0 - t, 0.85) * dice.range(0.85, 1.1) + 0.35) * vigour,
                    -0.12 + 0.5 * t + dice.range(-0.1, 0.1),
                    if sapling { 0.24 } else { 0.16 },
                    if sapling { 0.6 } else { 0.45 },
                )
            };
            let heading = (out * libm::cosf(rise) + Vec3::Y * libm::sinf(rise)).normalize();
            let thickness = (r * if pine { 0.33 } else { 0.3 }).max(0.025);
            let (mut p, rr, mut d) = line(
                dice, at, heading, length, thickness, 0.86, 4, 0.08, 0.0, 0.0, droop,
            );
            // The tip turns up toward the light.
            if let (Some(last), Some(dir)) = (p.len().checked_sub(1), d.last().copied()) {
                let up = (dir + Vec3::Y * upturn).normalize();
                p[last] = p[last - 1] + up * length / 4.0;
                d[last] = up;
            }
            let half = (around.rem_euclid(TAU) > PI) as usize;
            let branch = band * 2 + half;
            let gap = clump_m
                * if pine {
                    0.85
                } else if sapling {
                    1.05
                } else {
                    0.75
                };
            // A spruce's needles run nearly to its trunk, so its boughs show no bare stick
            // (D264); a young one's in the shade only along their outer part.
            let start = length * if pine || sapling { 0.35 } else { 0.12 };
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
    // The leading shoot, or each leader's.
    let branch = (bands - 1) * 2;
    let tops: Vec<Vec3> = if upper.is_empty() {
        vec![points[points.len() - 1]]
    } else {
        upper.iter().map(|(p, _)| p[p.len() - 1]).collect()
    };
    for top in tops {
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
    }
    tree.limb(points, radii, 0);
    for (p, r) in upper {
        tree.limb(p, r, 0);
    }
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
                Species::Oak => 6 + dice.below(2),
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
            // Its stalk on its twig, near where the clump is: up to half a clump off, cards
            // hung in the air beside a twig's end, which from below read as floating (D309).
            let base = tip - along * size * 0.1 + dice.unit_vector() * size * dice.range(0.05, 0.3);
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
    let light = 0.44 + 0.22 * outer + 0.16 * in_mass + 0.16 * rise + dice.range(-0.13, 0.09);
    let mut tint = Vec3::new(
        light * dice.range(0.95, 1.05),
        light,
        light * dice.range(0.9, 1.0),
    ) * hue;
    // Older leaves a darker olive (D269), and here and there a leaf turning.
    if dice.unit() < 0.3 {
        tint *= Vec3::new(0.86, 0.88, 0.72);
    }
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
        // Deep in the crown it sees little of the sky, falling off steeply inside the
        // outermost leaves of its mass and of the crown (D247, D269).
        sky: {
            // A conifer's open whorls let light in to its middle: it sees about a third of
            // the sky deep in, as it did (D247).
            let t = ((outer.max(in_mass * 0.85) - 0.3) / 0.65).clamp(0.0, 1.0);
            if crown.axial {
                (0.3 + 0.7 * outer.max(in_mass * 0.8)).min(1.0)
            } else {
                0.15 + 0.85 * t * t * (3.0 - 2.0 * t)
            }
        },
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
        assert!(pine.branches <= 10 && spruce.branches <= 16);
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
        let pines: Vec<Tree> = (0..8)
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
}
