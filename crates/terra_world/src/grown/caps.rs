//! Cap trees (docs/realm-two.md, D642): what stands where trees would in the far realm. A
//! great fungus grown by rule as a tree is: a stalk swollen and fluted at its foot and
//! leaning a little, a ring hanging from it, a wide cap, smaller caps in tiers down the
//! stalk and young ones at its foot. No cap is a lathe's circle: each has an age, and by it
//! a shape (research/2026-11-09-great-fungi.md): a young one a tall narrow bell, a grown
//! one a broad dome, an old one flat and dished with its rim lifted, waved and split. Its
//! numbers are its row's (`CapDef`, `data/flora.ron`); it is drawn in a skin of its own
//! (`cap_skin.wesl`), at the trees' levels of detail.

use super::build::{Build, Limb};
use super::pivots::Pivots;
use super::tree::Tree;
use super::{CapDef, TreeDef};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, TAU};
use terra_gen::Rng;

/// How a stalk's radius goes from its foot to its neck, against its `trunk_m`.
const STALK: [f32; 7] = [1.8, 1.3, 1.0, 0.9, 0.82, 0.76, 0.72];
/// A cap's rounds from its rim to its crown, in its limb's outline.
const ROUNDS: usize = 7;
/// A split in a rim is so wide to each side, in turns: two sides of a near cap's 48.
const SPLIT: f32 = 1.0 / 24.0;
/// How far under a cap's top its gills hang, as a share of the top's own rise.
const UNDER: f32 = 0.45;

/// What a cap tree grew besides its limbs: how each cap and stalk among them is shaped.
#[derive(Default, Clone, Debug)]
pub struct Fungus {
    pub caps: Vec<Cap>,
    pub stalks: Vec<Stalk>,
}

/// One cap: where it sits and how it is shaped, by its age.
#[derive(Clone, Debug)]
pub struct Cap {
    /// Which of the tree's limbs it is.
    pub limb: usize,
    /// Where its stalk meets it, and the way it faces.
    pub at: Vec3,
    pub axis: Vec3,
    /// Its rim's radius and its top's rise over `at`, in metres; its stalk's radius there.
    pub rim: f32,
    pub deep: f32,
    pub neck: f32,
    /// 0 a button just risen, 0.5 grown, 1 old.
    pub age: f32,
    /// How square its dome's shoulder is: 2 a ball's, more a bell's, less a tent's.
    pub round: f32,
    /// How far its middle has sunk and how high a knob stands in it, as shares of its rise.
    pub dish: f32,
    pub umbo: f32,
    /// Its outline's lobes: how many round, how far in and out, and where they start.
    pub waves: [(f32, f32, f32); 3],
    /// Its rim's rise and fall: how many round, how far as a share of its radius, where.
    pub ruffle: (f32, f32, f32),
    /// How far its rim hangs, as a share of its radius; lifted where below zero.
    pub droop: f32,
    /// Its rim's splits: where round it, in turns, and how deep, as a share of its radius.
    pub splits: Vec<(f32, f32)>,
    pub tint: Vec3,
    /// How far up its stalk it sits, in metres, and its stalk's colour: its collar's skin.
    pub collar: (f32, Vec3),
}

/// One stalk: its flutes, its colour, and how far up it a ring hangs, if one does.
#[derive(Clone, Debug)]
pub struct Stalk {
    pub limb: usize,
    pub flutes: [f32; 3],
    pub ring: Option<f32>,
    pub tint: Vec3,
}

/// A number by age: `young` at 0, `grown` at a half, `old` at 1.
fn aged(age: f32, young: f32, grown: f32, old: f32) -> f32 {
    if age < 0.5 {
        young + (grown - young) * age * 2.0
    } else {
        grown + (old - grown) * (age * 2.0 - 1.0)
    }
}

impl Cap {
    /// A cap of an age over a point: `across` and `depth` what its kind's grown cap is.
    #[allow(clippy::too_many_arguments)]
    fn grown(
        dice: &mut Rng,
        limb: usize,
        at: Vec3,
        (across, depth): (f32, f32),
        neck: f32,
        age: f32,
        kind: Vec3,
        collar: (f32, Vec3),
    ) -> Cap {
        let lean = Vec3::new(dice.range(-1.0, 1.0), 0.0, dice.range(-1.0, 1.0));
        let mut lobe = |lobes: f32| {
            (
                lobes,
                aged(age, 0.008, 0.035, 0.085) * dice.range(0.3, 1.0),
                dice.range(0.0, TAU),
            )
        };
        let waves = [lobe(2.0), lobe(3.0), lobe(5.0)];
        let ruffle = (
            4.0 + dice.below(4) as f32,
            aged(age, 0.0, 0.012, 0.05) * dice.range(0.5, 1.0),
            dice.range(0.0, TAU),
        );
        // An old rim splits, in up to three places, no two in one.
        let mut splits: Vec<(f32, f32)> = Vec::new();
        let worn = ((age - 0.6) / 0.4).clamp(0.0, 1.0);
        for _ in 0..dice.below(4) {
            let split = (dice.below(24) as f32 * SPLIT, dice.range(0.15, 0.35) * worn);
            let apart = |other: &(f32, f32)| {
                let off = (other.0 - split.0).abs();
                off.min(1.0 - off) > 2.5 * SPLIT
            };
            if worn > 0.0 && splits.iter().all(apart) {
                splits.push(split);
            }
        }
        // Deeper in colour young; paler and browner old.
        let grey = kind.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let faded = kind.lerp(Vec3::new(1.25, 1.05, 0.85) * grey, 0.35);
        let tint = if age < 0.5 {
            kind * (0.8 + 0.4 * age)
        } else {
            kind.lerp(faded, age * 2.0 - 1.0)
        };
        Cap {
            limb,
            at,
            axis: (Vec3::Y + lean * aged(age, 0.02, 0.05, 0.14)).normalize(),
            rim: across / 2.0 * aged(age, 0.42, 1.0, 1.08),
            deep: depth * aged(age, 1.9, 1.0, 0.6),
            neck,
            age,
            round: aged(age, 2.6, 2.0, 1.6),
            dish: aged(age, 0.0, 0.0, 0.45) * dice.range(0.4, 1.0),
            umbo: if dice.unit() < 0.4 {
                dice.range(0.08, 0.16)
            } else {
                0.0
            },
            waves,
            ruffle,
            droop: aged(age, 0.1, 0.03, -0.07) + dice.range(-0.02, 0.02),
            splits,
            tint: tint * dice.range(0.92, 1.08),
            collar,
        }
    }

    /// How deep its rim is split toward `round`, in turns, as a share of its radius.
    fn split(&self, round: f32) -> f32 {
        (self.splits.iter())
            .map(|&(at, deep)| {
                let off = (round - at).abs();
                deep * (1.0 - off.min(1.0 - off) / SPLIT).max(0.0)
            })
            .fold(0.0, f32::max)
    }

    /// A point of its top, or of its gills' face under it: `share` of the way from its
    /// middle to its rim, `round` it in turns.
    pub fn point(&self, share: f32, round: f32, top: bool) -> Vec3 {
        let angle = round * TAU;
        let wave: f32 = (self.waves.iter())
            .map(|(lobes, far, from)| far * libm::cosf(lobes * angle + from))
            .sum();
        let reach =
            self.rim * share * (1.0 + wave * share) * (1.0 - self.split(round) * share * share);
        let dome = libm::powf(
            (1.0 - libm::powf(share, self.round)).max(0.0),
            1.0 / self.round,
        );
        let bell = |across: f32| libm::expf(-(share / across) * (share / across));
        let mut rise = dome - self.dish * bell(0.4) + self.umbo * bell(0.13);
        if !top {
            rise = rise * UNDER - 0.06 * (1.0 - share);
        }
        let (count, far, from) = self.ruffle;
        let edge = share * share * share;
        let lift = self.rim * edge * (far * libm::cosf(count * angle + from) - self.droop);
        let side = self.axis.any_orthonormal_vector();
        let other = self.axis.cross(side);
        self.at
            + (side * libm::cosf(angle) + other * libm::sinf(angle)) * reach
            + self.axis * (self.deep * rise + lift)
    }

    /// Its limb: from the neck out under it to its rim, then over the dome to its crown,
    /// its outline round its own axis without its waves.
    fn limb(&self, level: u32) -> Limb {
        let foot = self.at - self.axis * self.deep * 0.08;
        let mut points = vec![foot, self.at + self.axis * self.deep * 0.02];
        let mut radii = vec![self.neck, self.rim * 0.97];
        for round in 0..=ROUNDS {
            let turn = round as f32 / ROUNDS as f32 * FRAC_PI_2;
            points.push(self.at + self.axis * self.deep * (0.06 + 0.94 * libm::sinf(turn)));
            radii.push((self.rim * libm::cosf(turn)).max(0.01));
        }
        Limb {
            points,
            radii,
            depth: level,
        }
    }
}

/// A stalk from a foot to its neck, leaning as it rises: its points, and its radius there.
fn stalk(foot: Vec3, lean: Vec3, rise: f32, girth: f32, level: u32) -> Limb {
    let points = (0..STALK.len())
        .map(|i| {
            let t = i as f32 / (STALK.len() - 1) as f32;
            foot + Vec3::Y * rise * t + lean * t * t
        })
        .collect();
    Limb {
        points,
        radii: STALK.iter().map(|share| girth * share).collect(),
        depth: level,
    }
}

/// Grows a cap tree into `tree`: its stalk, its cap, the tiers under it and its young.
pub fn grow(dice: &mut Rng, def: &TreeDef, tree: &mut Tree) {
    let CapDef {
        across,
        depth,
        tiers,
        young,
        tint,
        stalk: pale,
        ..
    } = def.cap.unwrap_or_default();
    let (height, girth) = (tree.height, tree.trunk_m);
    let (kind, pale) = (Vec3::from(tint), Vec3::from(pale));
    let lean = Vec3::new(dice.range(-0.07, 0.07), 0.0, dice.range(-0.07, 0.07)) * height;
    // Each tree's great cap is of an age of its own, from just opened to old.
    let age = dice.range(0.35, 1.0);
    let size = (across * height, depth * height);
    let rise = height - size.1 * aged(age, 1.9, 1.0, 0.6);
    let mut fungus = Fungus::default();
    let flutes = |dice: &mut Rng| [0.0; 3].map(|_| dice.range(0.0, TAU));
    fungus.stalks.push(Stalk {
        limb: tree.limbs.len(),
        flutes: flutes(dice),
        ring: (dice.unit() < 0.7).then(|| dice.range(0.74, 0.86)),
        tint: pale,
    });
    tree.limbs.push(stalk(Vec3::ZERO, lean, rise, girth, 0));
    let neck = girth * STALK[STALK.len() - 1];
    let cap = |tree: &mut Tree, fungus: &mut Fungus, cap: Cap, level: u32| {
        tree.limbs.push(cap.limb(level));
        fungus.caps.push(cap);
    };
    let great = Cap::grown(
        dice,
        tree.limbs.len(),
        Vec3::Y * rise + lean,
        size,
        neck,
        age,
        kind,
        (rise, pale),
    );
    cap(tree, &mut fungus, great, 1);
    // Smaller caps round the stalk below, each narrower and younger than the one over it.
    for tier in 1..tiers {
        let t = 1.0 - tier as f32 / (tiers as f32 + 0.6);
        let share = 0.62 - 0.12 * tier as f32;
        let lower = Cap::grown(
            dice,
            tree.limbs.len(),
            Vec3::Y * rise * t + lean * t * t,
            (size.0 * share, size.1 * 0.55),
            girth,
            (age * 0.8).max(0.4),
            kind,
            (rise * t, pale),
        );
        cap(tree, &mut fungus, lower, 1);
    }
    // Young ones at its foot, each a whole small one of its kind, a button or a bell.
    for _ in 0..young {
        let turn = Quat::from_rotation_y(dice.range(0.0, TAU));
        let foot = turn * Vec3::X * girth * dice.range(2.2, 4.0);
        let small = height * dice.range(0.1, 0.24);
        let lean = (turn * Vec3::X) * small * dice.range(0.05, 0.2);
        let age = dice.range(0.0, 0.32);
        let size = (across * small * 1.5, depth * small);
        let rise = small - size.1 * aged(age, 1.9, 1.0, 0.6);
        fungus.stalks.push(Stalk {
            limb: tree.limbs.len(),
            flutes: flutes(dice),
            ring: None,
            tint: pale,
        });
        tree.limbs.push(stalk(foot, lean, rise, girth * 0.3, 2));
        let button = Cap::grown(
            dice,
            tree.limbs.len(),
            foot + Vec3::Y * rise + lean,
            size,
            girth * 0.3 * STALK[STALK.len() - 1],
            age,
            kind,
            (rise, pale),
        );
        cap(tree, &mut fungus, button, 3);
    }
    tree.fungus = fungus;
}

/// Rows of points round an axis as a skin, each row `sides + 1` long, its last over its
/// first: their normals are the sheet's own, its front the side a row's turn and the next
/// row's rise make (`fallback` where a row closes to a point).
fn sheet(
    build: &mut Build,
    points: &[Vec3],
    uvs: &[Vec2],
    sides: u32,
    height: f32,
    tint: Vec3,
    fallback: Vec3,
) {
    let start = build.positions.len() as u32;
    let (ring, round) = (sides as usize + 1, sides as usize);
    let rows = points.len() / ring;
    let at = |row: usize, side: usize| points[row * ring + side % round];
    for row in 0..rows {
        for side in 0..ring {
            let up = at((row + 1).min(rows - 1), side) - at(row.saturating_sub(1), side);
            let turn = at(row, side + 1) - at(row, side + round - 1);
            let point = points[row * ring + side];
            let bend = (point.y / height).clamp(0.0, 1.0);
            build.vertex(
                point,
                turn.cross(up).normalize_or(fallback),
                uvs[row * ring + side],
                Vec2::new(bend * bend, 0.0),
                tint,
            );
        }
    }
    for row in 0..rows as u32 - 1 {
        for side in 0..sides {
            let a = start + row * ring as u32 + side;
            let b = a + ring as u32;
            // Wound so its outside is its front: a top from above, gills from below.
            build.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
}

/// A cap's mesh: its collar in its stalk's skin, then its gills' face from its neck out to
/// its rim and its top from the rim over to its crown, one sheet, so its rim is a rounded
/// lip. Its texture coordinates say what each is (`cap_skin.wesl`): how far round, and
/// minus one less how far out for gills (0 to 1), or less two and how far in for the top.
fn cap_mesh(build: &mut Build, cap: &Cap, detail: usize, sides: u32, height: f32) {
    let (under, over) = ([6, 4, 3][detail], [9, 6, 4][detail]);
    let necked = (cap.neck * 1.25 / cap.rim).min(0.5);
    // Its gills' face, out to the rim's own edge, and its top from there: two sheets, as
    // they meet at an edge, a young bell's as sharp as a fin's.
    let below = (0..under).map(|row| {
        let out = libm::powf(row as f32 / (under - 1) as f32, 0.8);
        (necked + (0.97 - necked) * out, false)
    });
    let above = (0..=over).map(|row| {
        let turn = row as f32 / over as f32 * FRAC_PI_2;
        (libm::cosf(turn).max(0.01 / cap.rim), true)
    });
    let round = |side: u32| (side % sides) as f32 / sides as f32;
    for rows in [
        below.chain([(1.0, true)]).collect::<Vec<_>>(),
        above.collect(),
    ] {
        let gills = !rows[0].1;
        let (mut points, mut uvs) = (Vec::new(), Vec::new());
        for &(share, top) in &rows {
            for side in 0..=sides {
                points.push(cap.point(share, round(side), top));
                let along = if gills { share } else { 2.0 - share };
                uvs.push(Vec2::new(side as f32 / sides as f32, -1.0 - along));
            }
        }
        sheet(build, &points, &uvs, sides, height, cap.tint, cap.axis);
    }
    // Its collar: the stalk widening up into the gills.
    let (up, pale) = cap.collar;
    let side = cap.axis.any_orthonormal_vector();
    let other = cap.axis.cross(side);
    let (mut points, mut uvs) = (Vec::new(), Vec::new());
    for row in 0..2 {
        for step in 0..=sides {
            let angle = round(step) * TAU;
            let out = side * libm::cosf(angle) + other * libm::sinf(angle);
            let foot = cap.at + out * cap.neck - cap.axis * cap.deep * 0.05;
            let point = if row == 0 {
                foot
            } else {
                cap.point(necked, round(step), false)
            };
            points.push(point);
            let along = up + (point - foot).length();
            uvs.push(Vec2::new(step as f32 / sides as f32, STALK_SKIN - along));
        }
    }
    sheet(build, &points, &uvs, sides, height, pale, cap.axis);
}

/// A stalk's skin says so in its texture coordinates: how far round, and this less how far
/// up it, in metres. Its ring's: `RING_SKIN` less how far out its skirt, 0 to 1, and back
/// under it, to 2.
const STALK_SKIN: f32 = -4.0;
const RING_SKIN: f32 = -100.0;

/// A stalk's mesh: its limb as a tube, fluted at its foot, in its own skin, and its ring,
/// a skirt hanging from it with a waved edge.
fn stalk_mesh(
    build: &mut Build,
    limb: &Limb,
    stalk: &Stalk,
    detail: usize,
    sides: u32,
    height: f32,
) {
    let last = limb.points.len() - 1;
    let round = |side: u32| (side % sides) as f32 / sides as f32 * TAU;
    // Its frame at each point, carried along it so it does not twist.
    let mut side = (limb.points[1] - limb.points[0]).any_orthonormal_vector();
    let mut frames = Vec::new();
    let mut along = 0.0;
    for (i, &point) in limb.points.iter().enumerate() {
        let forward = (limb.points[(i + 1).min(last)] - limb.points[i.saturating_sub(1)])
            .normalize_or(Vec3::Y);
        side = (side - forward * side.dot(forward)).normalize_or(forward.any_orthonormal_vector());
        if i > 0 {
            along += point.distance(limb.points[i - 1]);
        }
        frames.push((point, forward, side, forward.cross(side), along));
    }
    let (mut points, mut uvs) = (Vec::new(), Vec::new());
    for (i, &(point, _, side, other, along)) in frames.iter().enumerate() {
        // Fluted where it swells at its foot, near smooth above.
        let deep = 0.03 + 0.11 * (1.0 - i as f32 / last as f32 * 2.5).max(0.0);
        for step in 0..=sides {
            let angle = round(step);
            let [a, b, c] = stalk.flutes;
            let flute = 0.5 * libm::cosf(3.0 * angle + a)
                + 0.3 * libm::cosf(5.0 * angle + b)
                + 0.2 * libm::cosf(8.0 * angle + c);
            let out = side * libm::cosf(angle) + other * libm::sinf(angle);
            points.push(point + out * limb.radii[i] * (1.0 + deep * flute));
            uvs.push(Vec2::new(step as f32 / sides as f32, STALK_SKIN - along));
        }
    }
    sheet(build, &points, &uvs, sides, height, stalk.tint, Vec3::Y);
    // Its ring, gone far off: a skirt, from under it out and down to a waved edge and
    // back up over it to the stalk.
    let Some(up) = stalk.ring.filter(|_| detail < 2) else {
        return;
    };
    let place = up * last as f32;
    let (below, part) = ((place as usize).min(last - 1), place.fract());
    let (from, forward, side, other, _) = frames[below];
    let centre = from.lerp(frames[below + 1].0, part);
    let girth = limb.radii[below] + (limb.radii[below + 1] - limb.radii[below]) * part;
    let sides = sides * 2;
    let round = |side: u32| (side % sides) as f32 / sides as f32 * TAU;
    // Each row: how far out, in the stalk's radii, how far down, in the skirt's drop, and
    // how much of the edge's wave it takes.
    const SKIRT: [(f32, f32, f32); 7] = [
        (1.0, 0.14, 0.0),
        (1.4, 0.3, 0.2),
        (1.95, 0.72, 0.7),
        (2.35, 1.0, 1.0),
        (2.0, 0.7, 0.7),
        (1.45, 0.22, 0.2),
        (1.0, 0.0, 0.0),
    ];
    let drop = girth * 2.6;
    let [a, b, c] = stalk.flutes;
    let (mut points, mut uvs) = (Vec::new(), Vec::new());
    for (row, &(reach, down, waved)) in SKIRT.iter().enumerate() {
        for step in 0..=sides {
            let angle = round(step);
            let wave = 0.09 * libm::cosf(5.0 * angle + b)
                + 0.06 * libm::cosf(9.0 * angle + c)
                + 0.04 * libm::cosf(14.0 * angle + a);
            let out = side * libm::cosf(angle) + other * libm::sinf(angle);
            points.push(
                centre + out * girth * reach * (1.0 + wave * waved)
                    - forward * drop * down * (1.0 + 2.0 * wave * waved),
            );
            let along = row as f32 / 3.0;
            uvs.push(Vec2::new(step as f32 / sides as f32, RING_SKIN - along));
        }
    }
    sheet(build, &points, &uvs, sides, height, stalk.tint, Vec3::Y);
}

/// A cap tree's mesh at a level of detail: fewer sides and rows each level out, its young
/// ones and its ring gone at the furthest.
pub fn wood(tree: &Tree, detail: usize, pivots: &Pivots) -> Build {
    let deepest = [3, 1, 1][detail];
    let mut build = Build::levelled(pivots);
    for stalk in &tree.fungus.stalks {
        let limb = &tree.limbs[stalk.limb];
        if limb.depth > deepest {
            continue;
        }
        build.pivot = pivots.limbs.get(stalk.limb).copied();
        let sides = if limb.depth == 0 {
            [16, 10, 6][detail]
        } else {
            [8, 6, 5][detail]
        };
        stalk_mesh(&mut build, limb, stalk, detail, sides, tree.height);
    }
    for cap in &tree.fungus.caps {
        let level = tree.limbs[cap.limb].depth;
        if level > deepest {
            continue;
        }
        build.pivot = pivots.limbs.get(cap.limb).copied();
        let sides = if level == 1 {
            [48, 24, 12][detail]
        } else {
            [20, 12, 8][detail]
        };
        cap_mesh(&mut build, cap, detail, sides, tree.height);
    }
    build
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grown::Habit;
    use terra_gen::Seed;

    fn def(cap: CapDef) -> TreeDef {
        TreeDef {
            habit: Habit::Cap,
            bark: "bark".into(),
            bark_m: 2.0,
            bark_tint: (1.0, 1.0, 1.0),
            leaves: "oak".into(),
            tint: (1.0, 1.0, 1.0),
            clump_m: 1.0,
            trunk: 0.05,
            cap: Some(cap),
        }
    }

    fn row() -> CapDef {
        CapDef {
            across: 0.8,
            depth: 0.25,
            tiers: 3,
            young: 2,
            tint: (0.6, 0.2, 0.4),
            ..default()
        }
    }

    /// A cap of an age, 8 m across grown and 2 m deep, on a neck of 0.3 m.
    fn cap_of(age: f32, seed: u64) -> Cap {
        let at = Vec3::Y * 10.0;
        let kind = Vec3::new(0.6, 0.2, 0.4);
        Cap::grown(
            &mut Seed(seed).rng(),
            1,
            at,
            (8.0, 2.0),
            0.3,
            age,
            kind,
            (10.0, Vec3::ONE),
        )
    }

    /// A cap tree is as tall as it is grown, its great cap about as wide as its row says
    /// (a grown one's width, an old one's a little over it), with a cap for each tier and a
    /// stalk and a cap for each young one, every one smaller than the great one; and it is
    /// built at every level of detail, fewer triangles each level out.
    #[test]
    fn a_cap_tree_is_its_rows_numbers_and_simpler_further_off() {
        for seed in 0..8 {
            let tree = crate::grown::tree::grow(&mut Seed(seed).rng(), &def(row()), 20.0);
            assert_eq!(tree.limbs.len(), 1 + 3 + 2 * 2);
            assert_eq!((tree.fungus.stalks.len(), tree.fungus.caps.len()), (3, 5));
            let pivots = crate::grown::tree::pivots(&tree);
            let builds = [0, 1, 2].map(|detail| wood(&tree, detail, &pivots));
            let top = builds[0].height();
            assert!((top - 20.0).abs() < 1.2, "seed {seed}: {top} m tall");
            let great = &tree.fungus.caps[0];
            assert!(
                (0.8..=1.1).contains(&(great.rim / 8.0)),
                "seed {seed}: its cap {} m across at age {}",
                great.rim * 2.0,
                great.age
            );
            for small in &tree.fungus.caps[1..] {
                assert!(small.rim < great.rim * 0.7, "a small cap {} m", small.rim);
            }
            let triangles = builds.each_ref().map(Build::triangles);
            assert!(
                triangles[0] > triangles[1] && triangles[1] > triangles[2] && triangles[2] > 0,
                "{triangles:?}"
            );
        }
    }

    /// A cap's skin has its outside its front at every age: every triangle faces as its
    /// corners' normals do, its top up and its gills down. And all of it says it is skin.
    #[test]
    fn a_caps_outside_is_its_front_and_its_gills_face_down() {
        for age in [0.0, 0.5, 1.0] {
            let cap = cap_of(age, 5);
            let pivots = Pivots::default();
            let mut build = Build::levelled(&pivots);
            cap_mesh(&mut build, &cap, 0, 24, 12.0);
            let at = |i: u32| Vec3::from(build.positions[i as usize]);
            let normal = |i: u32| Vec3::from(build.normals[i as usize]);
            let (mut down, mut up) = (0, 0);
            for corners in build.indices.chunks(3) {
                let (a, b, c) = (corners[0], corners[1], corners[2]);
                let face = (at(b) - at(a)).cross(at(c) - at(a));
                if face.length() < 1.0e-6 {
                    continue;
                }
                let mean = normal(a) + normal(b) + normal(c);
                assert!(face.dot(mean) > 0.0, "age {age}: a triangle faces into it");
                // By where it is on the cap, which its texture coordinates say.
                let along = -1.0 - build.uvs[a as usize][1];
                if (0.3..0.9).contains(&along) {
                    assert!(mean.y < 0.0, "age {age}: gills face up at {along}");
                    down += 1;
                } else if (1.2..2.0).contains(&along) {
                    // Up, or on a bell's steep side out from its axis.
                    let out = (at(a) - cap.at).reject_from(cap.axis);
                    assert!(
                        mean.y > 0.0 || mean.dot(out) > 0.0,
                        "age {age}: its top faces in at {along}"
                    );
                    up += 1;
                }
            }
            assert!(down >= 24 && up >= 24, "{down} down, {up} up");
            assert!(build.uvs.iter().all(|uv| uv[1] <= -1.0));
        }
    }

    /// No cap is a lathe's circle, and its age is seen in its shape: a button is a narrow
    /// tall bell whose rim hangs, a grown cap a broad dome, an old one wider and flatter
    /// with its rim waved up and down and in and out, far more than a young one's.
    #[test]
    fn a_caps_shape_is_its_ages_and_never_a_circle() {
        // How far its rim is from its axis and above its foot, round it: each one's least
        // and most.
        let rim = |cap: &Cap| {
            let (mut out, mut up) = ((f32::MAX, 0.0_f32), (f32::MAX, f32::MIN));
            for step in 0..96 {
                let off = cap.point(1.0, step as f32 / 96.0, true) - cap.at;
                let rise = off.dot(cap.axis);
                let reach = (off - cap.axis * rise).length();
                out = (out.0.min(reach), out.1.max(reach));
                up = (up.0.min(rise), up.1.max(rise));
            }
            (out, up)
        };
        for seed in 0..12 {
            let (young, grown, old) = (cap_of(0.05, seed), cap_of(0.5, seed), cap_of(1.0, seed));
            let crown = |cap: &Cap| (cap.point(0.0, 0.0, true) - cap.at).dot(cap.axis);
            // A bell rises more than its rim's radius; a grown dome's rim is well wider
            // than it rises; an old cap is wider still, and lower.
            assert!(crown(&young) > young.rim, "seed {seed}: a button's bell");
            assert!(grown.rim > 1.7 * crown(&grown), "seed {seed}: a grown dome");
            assert!(
                old.rim > grown.rim && crown(&old) < crown(&grown),
                "seed {seed}"
            );
            let ((near, far), (low, high)) = rim(&old);
            assert!(
                far / near > 1.04,
                "seed {seed}: an old rim {near} to {far} m"
            );
            assert!(
                high - low > 0.08,
                "seed {seed}: an old rim {low} to {high} m up"
            );
            let ((near_young, far_young), _) = rim(&young);
            assert!(far / near > far_young / near_young, "seed {seed}");
            // A young rim hangs under where its stalk meets it; an old one is lifted.
            assert!(rim(&young).1.1 < 0.0 && high > 0.0, "seed {seed}");
        }
        // An old cap's split is a notch in its rim: there it reaches less far than a
        // twelfth of a turn on.
        let split = (0..40)
            .map(|seed| cap_of(1.0, seed))
            .find(|cap| !cap.splits.is_empty())
            .expect("no old cap of forty is split");
        let (at, deep) = split.splits[0];
        let reach = |round: f32| {
            let off = split.point(1.0, round, true) - split.at;
            (off - split.axis * off.dot(split.axis)).length()
        };
        assert!(
            reach(at) < reach(at + SPLIT) * (1.0 - deep * 0.6),
            "a split {deep} deep: {} m against {} m beside it",
            reach(at),
            reach(at + SPLIT)
        );
        assert!(cap_of(0.3, 3).splits.is_empty(), "a young cap is split");
    }

    /// A stalk's outside is its front, its ring's too: drawn one sided, it showed its far
    /// wall from within when wound the other way. And a ring is a skirt wider than its
    /// stalk, gone at the furthest level.
    #[test]
    fn a_stalks_outside_is_its_front_and_its_ring_stands_out() {
        let limb = stalk(Vec3::ZERO, Vec3::X, 10.0, 0.5, 0);
        let ringed = Stalk {
            limb: 0,
            flutes: [0.3, 1.1, 2.0],
            ring: Some(0.8),
            tint: Vec3::ONE,
        };
        let pivots = Pivots::default();
        let built = |detail: usize| {
            let mut build = Build::levelled(&pivots);
            stalk_mesh(&mut build, &limb, &ringed, detail, 12, 12.0);
            build
        };
        let build = built(0);
        let at = |i: u32| Vec3::from(build.positions[i as usize]);
        let normal = |i: u32| Vec3::from(build.normals[i as usize]);
        for corners in build.indices.chunks(3) {
            let (a, b, c) = (corners[0], corners[1], corners[2]);
            let face = (at(b) - at(a)).cross(at(c) - at(a));
            assert!(face.dot(normal(a) + normal(b) + normal(c)) > 0.0);
        }
        let ring = |build: &Build| build.uvs.iter().filter(|uv| uv[1] < -50.0).count();
        assert!(ring(&build) > 0 && ring(&built(2)) == 0);
        // Its skirt's edge, against the stalk's radius there (0.5 m by 0.76 to 0.82).
        let widest = (build.positions.iter().zip(&build.uvs))
            .filter(|(_, uv)| uv[1] < -50.0)
            .map(|(point, _)| Vec3::from(*point))
            .map(|point| point.distance(Vec3::new(0.64, 8.0, 0.0)))
            .fold(0.0, f32::max);
        assert!(widest > 0.8, "its ring reaches {widest} m");
    }
}
