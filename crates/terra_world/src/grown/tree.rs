//! Trees grown by rules, as SpeedTree and the shipped games that use it make them: a trunk
//! and limbs as tubes of bark, branching by the habit of the species, and leaves as cards
//! of real sprays at the twigs, each tree drawn at several levels of detail with a sparse
//! stand-in for its shadow. Ported from the lush forest spike's oak (research/lush-forest-
//! spike.md), with the conifer's and birch's habits added.

use super::build::{Build, Limb, Placing, card, lump, tube};
use super::{Card, Habit, TreeDef};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};
use terra_gen::Rng;

/// A tree's shape, before it is made into meshes.
pub struct Tree {
    pub limbs: Vec<Limb>,
    pub sprays: Vec<Spray>,
    /// How many branches the sprays are grouped by, each a lump far off.
    pub branches: usize,
    pub trunk_m: f32,
    pub height: f32,
    pub habit: Habit,
}

/// A spray of leaves: where, how big, on which branch, and, for a conifer's, which way its
/// twig runs, which its card lies along; a broadleaf's spray is a clump of cards turned
/// every way.
pub struct Spray {
    pub at: Vec3,
    pub along: Option<Vec3>,
    pub size: f32,
    pub branch: usize,
}

pub fn grow(dice: &mut Rng, def: &TreeDef, height: f32) -> Tree {
    let mut tree = Tree {
        limbs: Vec::new(),
        sprays: Vec::new(),
        branches: 0,
        trunk_m: height * def.trunk,
        height,
        habit: def.habit,
    };
    match def.habit {
        Habit::Spreading => {
            let trunk = tree.trunk_m;
            spread(
                dice,
                Vec3::ZERO,
                Vec3::Y,
                height * 0.34,
                trunk,
                0,
                0,
                def.clump_m,
                &mut tree,
            );
        }
        Habit::Conical | Habit::Upright => leader(dice, def, &mut tree),
    }
    tree
}

/// The foot of a trunk flares into its roots.
fn flare(points: &[Vec3], radii: &mut [f32]) {
    for (point, r) in points.iter().zip(radii.iter_mut()) {
        *r *= 1.0 + 1.1 * libm::expf(-point.y / 0.9);
    }
}

/// A spreading tree, as an oak: a stout trunk that splits into a few great limbs reaching
/// wider than they rise, branching three times more, with clumps of leaves at the ends.
#[allow(clippy::too_many_arguments)]
fn spread(
    dice: &mut Rng,
    from: Vec3,
    heading: Vec3,
    length: f32,
    radius: f32,
    depth: u32,
    branch: usize,
    clump_m: f32,
    tree: &mut Tree,
) {
    let height = tree.height;
    let steps = [7, 6, 5, 4][depth.min(3) as usize];
    let wander = [0.12, 0.28, 0.4, 0.5][depth.min(3) as usize];
    let mut direction = heading;
    let mut at = from;
    let mut points = vec![at];
    let mut radii = vec![radius];
    for step in 1..=steps {
        let jitter = dice.unit_vector() * wander;
        // Limbs reach out and then up toward the light; the trunk stays nearly upright.
        let lift = if depth == 0 { 0.15 } else { 0.08 };
        direction = (direction + jitter + Vec3::Y * lift).normalize();
        at += direction * length / steps as f32;
        let t = step as f32 / steps as f32;
        points.push(at);
        radii.push(radius * (1.0 - 0.6 * t));
    }
    if depth == 0 {
        flare(&points, &mut radii);
    }
    tree.limbs.push(Limb {
        points: points.clone(),
        radii: radii.clone(),
        depth,
    });
    match depth {
        // The trunk splits into great limbs, spreading out.
        0 => {
            let count = 3 + dice.below(3);
            for i in 0..count {
                let around = i as f32 / count as f32 * TAU + dice.range(-0.4, 0.4);
                let out = Vec3::new(libm::cosf(around), 0.0, libm::sinf(around));
                let rise = dice.range(0.55, 1.0);
                let heading = (out + Vec3::Y * rise).normalize();
                let length = height * dice.range(0.38, 0.5);
                spread(
                    dice,
                    at,
                    heading,
                    length,
                    radius * 0.62,
                    1,
                    0,
                    clump_m,
                    tree,
                );
            }
        }
        // Limbs and branches put out branches along their length, and fork at the end.
        1 | 2 => {
            let count = if depth == 1 {
                4 + dice.below(3)
            } else {
                3 + dice.below(3)
            };
            for _ in 0..count {
                let t = dice.range(0.3, 1.0);
                let index = ((points.len() - 1) as f32 * t) as usize;
                let side = dice.unit_vector();
                let heading = (direction * 0.5 + side + Vec3::Y * 0.25).normalize();
                // Each branch off a great limb, with all it carries, is one lump far off.
                let branch = if depth == 1 {
                    tree.branches += 1;
                    tree.branches - 1
                } else {
                    branch
                };
                let next_length = length * dice.range(0.4, 0.6);
                spread(
                    dice,
                    points[index],
                    heading,
                    next_length,
                    radii[index] * 0.6,
                    depth + 1,
                    branch,
                    clump_m,
                    tree,
                );
            }
        }
        // Twigs end in a clump of leaves.
        _ => tree.sprays.push(Spray {
            at,
            along: None,
            size: clump_m * dice.range(0.8, 1.2),
            branch,
        }),
    }
    if depth == 2 {
        tree.sprays.push(Spray {
            at: points[points.len() / 2],
            along: None,
            size: clump_m * dice.range(0.65, 0.95),
            branch,
        });
    }
}

/// A tree whose trunk runs on to the top, with branches off it all the way up: a conifer's
/// whorls, longest at the bottom, dipping and turning up at their tips, needles along them
/// (`Conical`); or a birch's single branches climbing steeply, their twigs hanging in fine
/// sprays (`Upright`).
fn leader(dice: &mut Rng, def: &TreeDef, tree: &mut Tree) {
    let height = tree.height;
    let conical = def.habit == Habit::Conical;
    let top = height * if conical { 1.0 } else { 0.92 };
    // The trunk, nearly straight, tapering to a point.
    let steps = 14;
    let lean = Vec3::new(dice.range(-1.0, 1.0), 0.0, dice.range(-1.0, 1.0)) * 0.015;
    let mut points = vec![Vec3::ZERO];
    let mut radii = vec![tree.trunk_m];
    let mut at = Vec3::ZERO;
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let wander = Vec3::new(dice.range(-1.0, 1.0), 0.0, dice.range(-1.0, 1.0))
            * if conical { 0.012 } else { 0.03 };
        at = Vec3::new(at.x, 0.0, at.z) + lean + wander + Vec3::Y * top * t;
        points.push(at);
        radii.push((tree.trunk_m * (1.0 - 0.9 * t)).max(0.025));
    }
    flare(&points, &mut radii);
    let trunk_at = |y: f32| {
        let i = ((y / top) * steps as f32).clamp(0.0, steps as f32 - 1.0) as usize;
        let f = (y / top * steps as f32 - i as f32).clamp(0.0, 1.0);
        (
            points[i].lerp(points[i + 1], f),
            radii[i] + (radii[i + 1] - radii[i]) * f,
        )
    };
    let base = height
        * if conical {
            dice.range(0.22, 0.36)
        } else {
            dice.range(0.3, 0.42)
        };
    let spacing = if conical {
        (height * 0.055).clamp(0.45, 1.1)
    } else {
        (height * 0.04).clamp(0.3, 0.7)
    };
    let clump = def.clump_m;
    let mut y = base;
    let mut turn = dice.range(0.0, TAU);
    while y < top - spacing * 0.6 {
        let t = (y - base) / (top - base);
        let count = if conical {
            4 + dice.below(3)
        } else {
            1 + u32::from(dice.unit() < 0.3)
        };
        for k in 0..count {
            let around = turn + k as f32 / count as f32 * TAU + dice.range(-0.3, 0.3);
            let out = Vec3::new(libm::cosf(around), 0.0, libm::sinf(around));
            let (length, rise, droop) = if conical {
                (
                    height * 0.27 * libm::powf(1.0 - t, 0.95) + 0.35,
                    -0.35 + 0.8 * t,
                    0.11,
                )
            } else {
                (
                    height * 0.22 * libm::sinf(PI * (0.12 + 0.8 * t)) * dice.range(0.8, 1.1) + 0.3,
                    0.62 + 0.4 * t,
                    -0.02,
                )
            };
            let (from, trunk_r) = trunk_at(y);
            let radius = (trunk_r * if conical { 0.32 } else { 0.4 }).max(0.02);
            let mut direction = (out * libm::cosf(rise) + Vec3::Y * libm::sinf(rise)).normalize();
            let mut at = from;
            let mut branch_points = vec![at];
            let mut branch_radii = vec![radius];
            let steps = 5;
            for step in 1..=steps {
                direction = (direction + dice.unit_vector() * 0.12 - Vec3::Y * droop).normalize();
                // A pine's branch turns up again at its tip.
                if conical && step == steps {
                    direction = (direction + Vec3::Y * 0.35).normalize();
                }
                at += direction * length / steps as f32;
                branch_points.push(at);
                branch_radii.push(radius * (1.0 - 0.8 * step as f32 / steps as f32));
            }
            let branch = tree.branches;
            tree.branches += 1;
            if conical {
                // Sprays of needles along the outer part of the branch, each along its twig.
                let mut along = length * 0.22;
                while along < length {
                    let s = along / length * steps as f32;
                    let i = (s as usize).min(steps - 1);
                    let point = branch_points[i].lerp(branch_points[i + 1], s - i as f32);
                    let twig = (branch_points[i + 1] - branch_points[i]).normalize();
                    tree.sprays.push(Spray {
                        at: point,
                        along: Some(twig),
                        size: clump * dice.range(0.85, 1.1),
                        branch,
                    });
                    along += clump * 0.32;
                }
            } else {
                // A birch's branch puts out twigs that hang, each a few sprays of leaves.
                for _ in 0..3 + dice.below(3) {
                    let s = dice.range(0.3, 1.0) * steps as f32;
                    let i = (s as usize).min(steps - 1);
                    let start = branch_points[i].lerp(branch_points[i + 1], s - i as f32);
                    let side = dice.unit_vector();
                    let mut heading = (direction * 0.5 + side * 0.6 - Vec3::Y * 0.5).normalize();
                    let twig_length = length * dice.range(0.35, 0.55);
                    let mut at = start;
                    let mut twig = vec![at];
                    let mut twig_radii = vec![radius * 0.4];
                    for step in 1..=3 {
                        heading = (heading - Vec3::Y * 0.3).normalize();
                        at += heading * twig_length / 3.0;
                        twig.push(at);
                        twig_radii.push(radius * 0.4 * (1.0 - 0.25 * step as f32));
                    }
                    for (f, scale) in [(0.45, 0.8), (1.0, 1.0)] {
                        tree.sprays.push(Spray {
                            at: start.lerp(at, f),
                            along: None,
                            size: clump * scale * dice.range(0.85, 1.15),
                            branch,
                        });
                    }
                    tree.limbs.push(Limb {
                        points: twig,
                        radii: twig_radii,
                        depth: 2,
                    });
                }
                tree.sprays.push(Spray {
                    at,
                    along: None,
                    size: clump * dice.range(0.9, 1.2),
                    branch,
                });
            }
            tree.limbs.push(Limb {
                points: branch_points,
                radii: branch_radii,
                depth: 1,
            });
        }
        y += spacing * dice.range(0.8, 1.2);
        turn += if conical { dice.range(0.5, 1.0) } else { 2.4 };
    }
    // The leading shoot: a conifer's top is a spike of needles, a birch's a last spray.
    let branch = tree.branches;
    tree.branches += 1;
    if conical {
        for k in 0..3 {
            let out = Vec3::new(libm::cosf(k as f32 * 2.1), 0.0, libm::sinf(k as f32 * 2.1));
            tree.sprays.push(Spray {
                at: at - Vec3::Y * clump * 0.3 * k as f32,
                along: Some((Vec3::Y + out * 0.35).normalize()),
                size: clump * 0.9,
                branch,
            });
        }
    } else {
        tree.sprays.push(Spray {
            at,
            along: None,
            size: clump,
            branch,
        });
    }
    tree.limbs.push(Limb {
        points,
        radii,
        depth: 0,
    });
}

/// A tree's wood at a level of detail: all of it, then without the twigs, then only the
/// trunk and great limbs; rounder near the camera. A conifer's many thin branches are left
/// out at the last level, where its sprays are big enough to hide that.
pub fn wood(tree: &Tree, detail: usize, bark_m: f32) -> Build {
    let (deepest, sides) = match tree.habit {
        Habit::Spreading => (
            [3, 2, 1][detail],
            [[10, 7, 5, 3], [7, 5, 4, 3], [5, 4, 3, 3]][detail],
        ),
        Habit::Conical => (
            [1, 1, 0][detail],
            [[10, 3, 3, 3], [7, 3, 3, 3], [5, 3, 3, 3]][detail],
        ),
        Habit::Upright => (
            [2, 1, 0][detail],
            [[10, 5, 3, 3], [7, 4, 3, 3], [5, 3, 3, 3]][detail],
        ),
    };
    let mut build = Build::default();
    for limb in tree.limbs.iter().filter(|limb| limb.depth <= deepest) {
        tube(
            &mut build,
            limb,
            sides[limb.depth.min(3) as usize],
            tree.height,
            bark_m,
            Vec3::ONE,
        );
    }
    build
}

/// How the cards of one crown are lit and moved.
pub struct Canopy {
    /// The middle of the crown, which the cards' normals lean away from.
    pub centre: Vec3,
    /// How far its outermost spray is from the middle.
    pub reach: f32,
    pub height: f32,
    /// How far its top bends in the wind, against a tree's.
    pub bend: f32,
    /// Its leaves' colour, as a multiple of the picture's.
    pub tint: Vec3,
    /// Whether its normals lean out from its trunk, as a cone's do, rather than from its
    /// middle.
    pub from_axis: bool,
}

impl Canopy {
    pub fn of(tree: &Tree, tint: Vec3) -> Self {
        let sprays = &tree.sprays;
        let centre =
            sprays.iter().fold(Vec3::ZERO, |sum, s| sum + s.at) / sprays.len().max(1) as f32;
        let reach = sprays
            .iter()
            .map(|s| s.at.distance(centre))
            .fold(0.0, f32::max)
            .max(1.0);
        Self {
            centre,
            reach,
            height: tree.height,
            bend: 1.0,
            tint,
            from_axis: tree.habit == Habit::Conical,
        }
    }

    /// Which way the crown faces at a point: out of it, so it shades as one.
    fn out(&self, at: Vec3) -> Vec3 {
        if self.from_axis {
            (Vec3::new(at.x, 0.0, at.z).normalize_or(Vec3::X) + Vec3::Y * 0.4).normalize()
        } else {
            (at - self.centre).normalize_or(Vec3::Y)
        }
    }

    /// How a card at a point is coloured: darker deep and low in the crown, which the rest
    /// shades, a little different card to card.
    fn colour(&self, dice: &mut Rng, at: Vec3) -> Vec3 {
        let depth = if self.from_axis {
            let axis = Vec3::new(0.0, at.y, 0.0);
            (at.distance(axis) / (self.reach * 0.6)).clamp(0.0, 1.0)
        } else {
            (at.distance(self.centre) / self.reach).clamp(0.0, 1.0)
        };
        let rise = ((at.y - self.centre.y) / self.reach * 0.5 + 0.5).clamp(0.0, 1.0);
        let light = 0.72 + 0.2 * depth + 0.14 * rise + dice.range(-0.05, 0.05);
        Vec3::new(
            light * dice.range(0.95, 1.05),
            light,
            light * dice.range(0.92, 1.02),
        ) * self.tint
    }

    /// How much of the sky a point in or under the crown sees: most at its edge and top,
    /// least deep inside it and low on the trunk under it (D216).
    pub fn sky(&self, at: Vec3) -> f32 {
        let depth = if self.from_axis {
            Vec2::new(at.x, at.z).length() / (self.reach * 0.6)
        } else {
            at.distance(self.centre) / self.reach
        };
        let rise = (at.y / self.height).clamp(0.0, 1.0);
        (0.25 + 0.5 * depth.clamp(0.0, 1.0) + 0.35 * rise).min(1.0)
    }

    fn sway(&self, dice: &mut Rng, at: Vec3) -> Vec2 {
        let bend = (at.y / self.height).clamp(0.0, 1.0);
        Vec2::new(bend * bend * self.bend, dice.range(0.6, 1.0))
    }
}

/// A clump of cards round a point, each turned at random but mostly facing out of the
/// crown.
fn clump_cards(dice: &mut Rng, canopy: &Canopy, at: Vec3, size: f32, cards: u32) -> Vec<Placing> {
    (0..cards)
        .map(|_| {
            let middle = at + dice.unit_vector() * size * dice.range(0.2, 0.75);
            let out = canopy.out(middle);
            let facing = (dice.unit_vector() + out * 0.7 + Vec3::Y * 0.3).normalize();
            let spin = Quat::from_axis_angle(facing, dice.range(0.0, TAU));
            let right = spin * facing.any_orthonormal_vector();
            let up = facing.cross(right);
            let half = size * dice.range(0.45, 0.65);
            let flip = if dice.unit() < 0.5 { -1.0 } else { 1.0 };
            Placing {
                pin: Vec2::splat(0.5),
                at: middle,
                right: right * 2.0 * half * flip,
                down: -up * 2.0 * half,
                normal: (out * 0.75 + facing * 0.25).normalize(),
                sway: canopy.sway(dice, middle),
                colour: canopy.colour(dice, middle),
            }
        })
        .collect()
}

/// A clump of cards round a point, drawn.
pub fn clump(
    build: &mut Build,
    dice: &mut Rng,
    leaf: &Card,
    canopy: &Canopy,
    at: Vec3,
    size: f32,
    cards: u32,
) {
    for place in clump_cards(dice, canopy, at, size, cards) {
        card(build, leaf, &place);
    }
}

/// A conifer's spray: a card lying along its twig, its base toward the branch, rolled a
/// little about the twig so a branch seen from the side is not edge-on.
fn twig_card(
    dice: &mut Rng,
    canopy: &Canopy,
    at: Vec3,
    along: Vec3,
    size: f32,
    roll: f32,
) -> Placing {
    let flat = along.cross(Vec3::Y).normalize_or(Vec3::X);
    let side = Quat::from_axis_angle(along, roll) * flat;
    let facing = side.cross(along).normalize();
    let out = canopy.out(at);
    Placing {
        pin: Vec2::new(0.5, 0.75),
        at,
        right: side * size,
        down: -along * size,
        normal: (out * 0.6 + facing * 0.4).normalize(),
        sway: canopy.sway(dice, at),
        colour: canopy.colour(dice, at),
    }
}

/// A card drawn grown about its pin by so much.
fn enlarged(place: &Placing, grow: f32) -> Placing {
    Placing {
        right: place.right * grow,
        down: place.down * grow,
        ..*place
    }
}

/// A crown's cards, placed once: for each spray, its cards in the order the simpler levels
/// keep them. A simpler level keeps the first few in the same places, each enlarged to cover
/// as much as all of them did, as SpeedTree's levels do, so crossfading between levels
/// shows one crown thinning, not two different crowns (D221).
pub fn crown_cards(dice: &mut Rng, tree: &Tree, canopy: &Canopy) -> Vec<Vec<Placing>> {
    tree.sprays
        .iter()
        .map(|spray| match spray.along {
            None => {
                let many = if tree.habit == Habit::Upright {
                    5 + dice.below(3)
                } else {
                    6 + dice.below(4)
                };
                clump_cards(dice, canopy, spray.at, spray.size, many)
            }
            // A conifer's spray along its twig, and near, a second across it for depth.
            Some(along) => {
                let roll = dice.range(-0.5, 0.5);
                vec![
                    twig_card(dice, canopy, spray.at, along, spray.size, roll),
                    twig_card(dice, canopy, spray.at, along, spray.size * 0.8, roll + 1.4),
                ]
            }
        })
        .collect()
}

/// A tree's crown at a level of detail: every card near; then three of each clump, or
/// every other conifer spray, enlarged; then one, or every third.
pub fn crown(cards: &[Vec<Placing>], tree: &Tree, leaf: &Card, detail: usize) -> Build {
    let mut build = Build::default();
    for (i, (spray, set)) in tree.sprays.iter().zip(cards).enumerate() {
        let (keep, grow) = if spray.along.is_some() {
            let (every, grow) = [(1, 1.0), (2, 1.45), (3, 2.0)][detail];
            if i % every != 0 {
                continue;
            }
            ([2, 1, 1][detail], grow)
        } else {
            let keep = [set.len(), 3, 1][detail].min(set.len());
            // Each grows by the root of how many it stands for, a little less, since the
            // kept cards overlap less than all of them did.
            let grow = if keep == set.len() {
                1.0
            } else {
                0.85 * libm::sqrtf(set.len() as f32 / keep as f32).min(2.6)
            };
            (keep, grow)
        };
        for place in &set[..keep.min(set.len())] {
            card(&mut build, leaf, &enlarged(place, grow));
        }
    }
    build
}

/// What a crown's shadow is cast from near the camera: the first two cards of every clump,
/// or every other conifer spray, the crown's own, enlarged as the simpler levels are, so
/// the light comes through in flecks where it does through the crown.
pub fn shadow_cards(cards: &[Vec<Placing>], tree: &Tree, leaf: &Card) -> Build {
    let mut build = Build::default();
    for (i, (spray, set)) in tree.sprays.iter().zip(cards).enumerate() {
        let (keep, grow) = if spray.along.is_some() {
            if i % 2 != 0 {
                continue;
            }
            (1, 1.4)
        } else {
            let keep = 2.min(set.len());
            (
                keep,
                libm::sqrtf(set.len() as f32 / keep as f32).min(2.6) * 0.8,
            )
        };
        for place in &set[..keep] {
            card(&mut build, leaf, &enlarged(place, grow));
        }
    }
    build
}

/// What a crown is far off, and what its shadow is cast from farther off: a rounded lump
/// for each branch, as big as most of its leaves times `scale`, solid so the GPU can skip
/// what is hidden, in the leaves' colour, darker underneath. A conifer's many branches are
/// taken a few whorls at a time, its lumps stacked up its trunk.
pub fn lumps(tree: &Tree, scale: f32, colour: Vec3) -> Build {
    let mut build = Build::default();
    let groups = if tree.branches > 24 {
        10
    } else {
        tree.branches
    };
    let crown =
        tree.sprays.iter().fold(Vec3::ZERO, |sum, s| sum + s.at) / tree.sprays.len().max(1) as f32;
    for group in 0..groups {
        let sprays: Vec<&Spray> = tree
            .sprays
            .iter()
            .filter(|s| s.branch * groups / tree.branches.max(1) == group)
            .collect();
        if sprays.is_empty() {
            continue;
        }
        let centre = sprays.iter().fold(Vec3::ZERO, |sum, s| sum + s.at) / sprays.len() as f32;
        let size = sprays.iter().map(|s| s.size).sum::<f32>() / sprays.len() as f32;
        let spread = sprays
            .iter()
            .map(|s| (s.at - centre).abs())
            .fold(Vec3::ZERO, Vec3::max);
        let radii = (spread * 0.85 + Vec3::splat(size * 0.75)) * Vec3::new(1.0, 0.8, 1.0) * scale;
        let rise = (centre.y / tree.height).clamp(0.0, 1.0);
        lump(
            &mut build,
            centre,
            radii,
            crown,
            colour * (0.8 + 0.2 * rise),
        );
    }
    build
}
