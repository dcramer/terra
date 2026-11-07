//! The forest's lawns as a ground cover of clumps (the lead's blind judge, 2026-11-06: "a
//! flat, lurid, overexposed yellow-green carpet; le-river's ferns have dark depth and
//! shade"): broad-leaved rosettes, fern fronds and grass tufts of varied height and hue,
//! each leaf dark at its foot (the clump's own occlusion) over darker leaves lying flat
//! beneath, blue-green under the oaks, warmer in the open, a few flowers in the sun. As
//! Last Epoch's and Path of Exile 2's undergrowth is built: masses of cards over the ground
//! whose gaps fall to near black. The fight's trodden ground stays bare, so the cast reads.

use super::clearing::trodden;
use crate::forest::{BRAMBLE, BUTTERCUP, CAMPION, Cards, DAISY, Rng, Spot};
use crate::terrain::{self, Land, smooth};
use bevy::prelude::*;

/// The atlas's cells (materials/foliage/cards.png): a broad leaf, a fern frond (diagonal in
/// its cell, its foot at the lower left), a grass tuft.
const LEAF: Vec2 = Vec2::new(0.5, 0.25);
const FROND: Vec2 = Vec2::new(0.25, 0.25);
const TUFT: Vec2 = Vec2::new(0.75, 0.75);
/// The ground the lawn is tried over (world x, z: the frame and a margin) and how many
/// clumps a square metre are tried.
const LO: Vec2 = Vec2::new(-12.0, -10.0);
const HI: Vec2 = Vec2::new(20.0, 17.0);
const TRIES: f32 = 7.0;
/// How far the lawn keeps from each of the cast (m), beyond the trodden ground.
const CAST: f32 = 2.2;
/// A leaf's foot over its tip: the clump's own occlusion.
const FOOT: f32 = 0.2;
/// The leaves lying flat under a clump, over its tip's tint.
const UNDER: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Leaves,
    Fern,
    Tuft,
    /// Matted dead grass (`mat`): no leaves beneath.
    Mat,
    /// A shrub of leafy sprigs, one of `SPRIGS` (the forest's undergrowth): many hand-sized
    /// leaves on each card, where a `Leaves` clump is a few broad leaves (forest judge
    /// 261107-0250: "big flat leaf cards ... oversized against the hero").
    Sprig(usize),
}

/// The forest's shrubs (`Kind::Sprig`): each one's atlas cells (diagonal in their cell,
/// the stem's foot at the lower left, as the fern's), how many sprigs a clump, how far
/// they lean from upright (radians) and its radius range (m; a sprig runs 1.2 to 1.7 of
/// it): a bramble's arching mound, a hazel's or bilberry's upright sprigs, holly low.
/// At 0.5 to 1 m a card a bramble's or hazel's leaf is 6 to 12 cm, a hand's.
struct Shrub {
    cells: [Vec2; 2],
    count: (f32, f32),
    lean: (f32, f32),
    radius: (f32, f32),
}

const SPRIGS: [Shrub; 3] = [
    Shrub {
        cells: [BRAMBLE, BRAMBLE],
        count: (8.0, 13.0),
        lean: (0.9, 1.4),
        radius: (0.35, 0.6),
    },
    Shrub {
        cells: [Vec2::new(0.75, 0.0), Vec2::new(0.0, 0.25)],
        count: (6.0, 10.0),
        lean: (0.3, 0.85),
        radius: (0.38, 0.62),
    },
    Shrub {
        cells: [Vec2::new(0.25, 0.75), Vec2::new(0.75, 0.0)],
        count: (5.0, 8.0),
        lean: (0.6, 1.15),
        radius: (0.28, 0.45),
    },
];

/// Each kind's cell's mean linear colour over its alpha-cut texels (measured from
/// materials/foliage/cards.png, iso-kit j55): the frond's green is twice its red and its blue
/// a seventh of it, so a green tint over it doubled the hue into lime (judge 261107-0556:
/// "sunlit grass and ferns blotchy lime-yellow").
fn cell_mean(kind: Kind) -> Vec3 {
    match kind {
        Kind::Fern => Vec3::new(0.070, 0.146, 0.021),
        Kind::Tuft | Kind::Mat => Vec3::new(0.067, 0.104, 0.023),
        Kind::Leaves => Vec3::new(0.051, 0.126, 0.032),
        Kind::Sprig(0) => Vec3::new(0.064, 0.106, 0.028),
        Kind::Sprig(1) => Vec3::new(0.095, 0.154, 0.038),
        Kind::Sprig(_) => Vec3::new(0.021, 0.047, 0.011),
    }
}

/// How far a forest clump's tint takes its cell's own hue out (1: a grey card the tint
/// colours alone), so the tint, not the atlas, sets the green.
const NEUTRAL: f32 = 0.8;

/// The factor that takes `kind`'s cell `NEUTRAL` of the way to a grey of its own luminance,
/// its value unchanged.
fn grade(kind: Kind) -> Vec3 {
    let m = cell_mean(kind);
    let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    let l = luma(m);
    let g = (Vec3::splat(l) / m).powf(NEUTRAL);
    g * l / luma(m * g)
}

/// A size between `lo` and `hi`, log-uniform, so small ones are as common as large.
fn spread(rng: &mut Rng, (lo, hi): (f32, f32)) -> f32 {
    lo * (hi / lo).powf(rng.f())
}

/// One clump: where, how wide (m), its kind, its tint and whether a flower stands in it.
#[derive(Clone, Copy, Debug)]
pub struct Clump {
    pub at: Vec2,
    pub radius: f32,
    pub kind: Kind,
    pub tint: Vec3,
    pub flower: Option<Vec2>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn.after(crate::scene::build));
}

/// Whether the lawn may grow at `p`: dry, gentle, untrodden, off the camp's bare ground and
/// clear of the cast, the oaks' feet and the moose's walk (dress::free).
pub fn grows(land: &Land, p: Vec2) -> bool {
    let cast = [super::KNIGHT, super::FOE, super::TARGET, super::SECOND];
    super::dress::free(p)
        && trodden(p) < 0.02
        && terrain::bare(p) < 0.35
        && land.slope(p) < 0.6
        && cast.iter().all(|c| c.distance(p) > CAST)
}

/// The lawn's clumps, the same every run.
pub fn clumps(land: &Land, trees: &[Spot]) -> Vec<Clump> {
    let mut rng = Rng::new(0x1a_3e5);
    let area = (HI.x - LO.x) * (HI.y - LO.y);
    let mut out = Vec::new();
    for _ in 0..(area * TRIES) as usize {
        let p = LO + Vec2::new(rng.f(), rng.f()) * (HI - LO);
        // In masses with a few gaps, never an even lawn.
        let mass = terrain::fbm(p * 0.45, 71);
        if rng.f() > 0.55 + 0.45 * smooth(0.3, 0.6, mass) || !grows(land, p) {
            continue;
        }
        // Shade under the oaks (scene::canopy: 1 in the open) turns the green blue and dark.
        let open = smooth(0.55, 0.95, crate::scene::canopy(p, trees));
        let hue = terrain::fbm(p * 0.21, 73);
        // Over a near-grey card (`grade`) the tint is the leaf's hue: dark blue-green in the
        // shade, an olive to a mid green in the sun, as the refs' woodland floor (their sunlit
        // greens' saturation 0.25 to 0.26 against the kept still's 0.63, iso-kit j55).
        let deep = Vec3::new(0.36, 0.54, 0.42).lerp(Vec3::new(0.42, 0.56, 0.34), hue);
        let warm = Vec3::new(0.62, 0.7, 0.46).lerp(Vec3::new(0.5, 0.66, 0.46), hue);
        let tint = deep.lerp(warm, open * 0.7) * rng.range(0.45, 0.9) * (0.75 + 0.25 * open);
        // Shrubs of hand-sized leaves in the main, ferns at true size (fronds 0.6 to 1.2 m),
        // grass, and a few broad-leaved herbs at a burdock's 0.25 to 0.4 m; each clump its own
        // size (log-uniform) and turn, so none repeats.
        let pick = rng.f();
        let (kind, radius) = if pick < 0.45 {
            let s = (rng.f() * 3.0) as usize % 3;
            (Kind::Sprig(s), spread(&mut rng, SPRIGS[s].radius))
        } else if pick < 0.75 {
            (Kind::Fern, spread(&mut rng, (0.42, 0.6)))
        } else if pick < 0.9 {
            (Kind::Tuft, spread(&mut rng, (0.25, 0.45)))
        } else {
            (Kind::Leaves, spread(&mut rng, (0.2, 0.3)))
        };
        let flower = (open > 0.6 && rng.f() < 0.08)
            .then(|| [DAISY, BUTTERCUP, CAMPION][(rng.f() * 3.0) as usize % 3]);
        out.push(Clump {
            at: p,
            radius: radius * (0.9 + 0.2 * mass),
            kind,
            tint: tint * grade(kind),
            flower,
        });
    }
    out
}

/// What holds on the fight's trodden ground (judge 261107-0250: "one flat dark-brown sheet
/// ... no tufts"): small grass tufts, plantain-sized rosettes and young ferns, patchy, most
/// at the clearing's rim and in its middle's islands, none within reach of the cast.
pub fn floor(land: &Land) -> Vec<Clump> {
    let mut rng = Rng::new(0x7f1_00e);
    let (lo, hi) = (Vec2::new(-3.0, -1.0), Vec2::new(10.0, 12.0));
    let area = (hi.x - lo.x) * (hi.y - lo.y);
    let cast = [super::KNIGHT, super::FOE, super::TARGET, super::SECOND];
    let mut out = Vec::new();
    // 6.5 tries a m2 (was 4): the paths' sunlit edges read as a smooth blur with a few
    // blades (judge 261107-0556: "path and ground read as mush"), so more tufts hold there.
    for _ in 0..(area * 6.5) as usize {
        let p = lo + Vec2::new(rng.f(), rng.f()) * (hi - lo);
        let t = trodden(p);
        let islands = smooth(0.35, 0.6, terrain::fbm(p * 0.8, 151));
        let holds = smooth(0.0, 0.12, t) * (smooth(0.75, 0.35, t) + 0.6 * islands).min(1.0);
        if rng.f() > holds * 0.6
            || terrain::pond(p) > -0.08
            || p.distance(crate::scene::FIRE) < 1.6
            || cast.iter().any(|c| c.distance(p) < 0.9)
            || land.slope(p) > 0.6
        {
            continue;
        }
        let hue = terrain::fbm(p * 0.4, 153);
        // Trodden, dusty and half dry: duller and browner than the lawn's. At a tuft's 0.1 to
        // 0.2 m and a tint under 0.5 they vanished into the earth (round 261107-0343): the
        // rush's blades thinner than a pixel, mipped under the alpha cut.
        let tint =
            Vec3::new(0.55, 0.65, 0.42).lerp(Vec3::new(0.66, 0.64, 0.4), hue) * rng.range(0.7, 1.1);
        let pick = rng.f();
        let (kind, radius) = if pick < 0.55 {
            (Kind::Tuft, spread(&mut rng, (0.18, 0.32)))
        } else if pick < 0.8 {
            (Kind::Leaves, spread(&mut rng, (0.11, 0.18)))
        } else {
            (Kind::Fern, spread(&mut rng, (0.22, 0.35)))
        };
        out.push(Clump {
            at: p,
            radius,
            kind,
            tint: tint * grade(kind),
            flower: None,
        });
    }
    out
}

fn rgba(c: Vec3) -> [f32; 4] {
    [c.x, c.y, c.z, 1.0]
}

/// A clump's cards: leaves into `leaves` (they flutter), tufts and flowers into `stems`
/// (they bend from the foot).
fn build(c: &Clump, land: &Land, rng: &mut Rng, leaves: &mut Cards, stems: &mut Cards) {
    let root = Vec3::new(c.at.x, land.height(c.at) - 0.02, c.at.y);
    let up = |p: Vec3| ((p - root).with_y(0.0).normalize_or_zero() * 0.5 + Vec3::Y).normalize();
    let tip = rgba(c.tint);
    let foot = rgba(c.tint * FOOT);
    let spin = rng.f() * std::f32::consts::TAU;
    let under = if c.kind == Kind::Mat {
        mat(c, root, spin, rng, stems);
        0
    } else {
        3
    };
    // Dark leaves lying flat beneath, so the gaps between the clump's leaves fall dark: a
    // shrub's its own sprigs, not one broad leaf the clump's width.
    let beneath = match c.kind {
        Kind::Sprig(s) => SPRIGS[s].cells[0],
        _ => LEAF,
    };
    for k in 0..under {
        let yaw = spin + k as f32 * 2.1 + rng.f();
        let out = Vec3::new(yaw.cos(), 0.0, yaw.sin());
        let len = c.radius * rng.range(1.1, 1.5);
        let a = Vec3::new(-out.z, 0.0, out.x) * len * 0.8;
        let b = (out + Vec3::Y * 0.08).normalize() * len;
        let dark = rgba(c.tint * UNDER * FOOT);
        leaves.graded(
            root + b * 0.5 + Vec3::Y * 0.01,
            a,
            b,
            beneath,
            |_| Vec3::Y,
            dark,
            rgba(c.tint * UNDER),
        );
    }
    match c.kind {
        Kind::Leaves => {
            let n = 5 + (rng.f() * 4.0) as usize;
            for k in 0..n {
                let yaw = spin + k as f32 * std::f32::consts::TAU / n as f32 + rng.range(-0.3, 0.3);
                let tilt = rng.range(0.6, 1.25);
                let out = Vec3::new(yaw.cos(), 0.0, yaw.sin());
                let dir = out * tilt.sin() + Vec3::Y * tilt.cos();
                let len = c.radius * rng.range(1.0, 1.4);
                let a = Vec3::new(-out.z, 0.0, out.x) * len * 0.85;
                let b = dir * len;
                leaves.graded(root + b * 0.5, a, b, LEAF, up, foot, tip);
            }
        }
        Kind::Fern => {
            let n = 6 + (rng.f() * 4.0) as usize;
            for k in 0..n {
                let yaw =
                    spin + k as f32 * std::f32::consts::TAU / n as f32 + rng.range(-0.25, 0.25);
                let tilt = rng.range(0.8, 1.3);
                let out = Vec3::new(yaw.cos(), 0.0, yaw.sin());
                let dir = out * tilt.sin() + Vec3::Y * tilt.cos();
                let len = c.radius * rng.range(1.4, 2.0);
                // The frond runs corner to corner of its cell: a + b along it, a - b across.
                let along = dir * len;
                let across = Vec3::new(-out.z, 0.0, out.x) * len;
                let (a, b) = ((along + across) * 0.5, (along - across) * 0.5);
                leaves.graded(root + along * 0.5, a, b, FROND, up, foot, tip);
            }
        }
        Kind::Sprig(s) => {
            let shrub = &SPRIGS[s];
            let n = rng.range(shrub.count.0, shrub.count.1) as usize;
            for k in 0..n {
                let yaw = spin + k as f32 * std::f32::consts::TAU / n as f32 + rng.range(-0.4, 0.4);
                let tilt = rng.range(shrub.lean.0, shrub.lean.1);
                let out = Vec3::new(yaw.cos(), 0.0, yaw.sin());
                let dir = out * tilt.sin() + Vec3::Y * tilt.cos();
                let len = c.radius * rng.range(1.2, 1.7);
                let along = dir * len;
                let across = Vec3::new(-out.z, 0.0, out.x) * len;
                let (a, b) = ((along + across) * 0.5, (along - across) * 0.5);
                let cell = shrub.cells[(rng.f() * 2.0) as usize % 2];
                // Each sprig a little lighter or darker, so the mass has depth.
                let v = rng.range(0.8, 1.15);
                let (f, t) = (rgba(c.tint * FOOT * v), rgba(c.tint * v));
                leaves.graded(root + along * 0.5, a, b, cell, up, f, t);
            }
        }
        Kind::Mat => {}
        Kind::Tuft => {
            for k in 0..3 {
                let yaw = spin + k as f32 * 1.05;
                let a = Vec3::new(yaw.cos(), 0.0, yaw.sin()) * c.radius * 1.6;
                let b = Vec3::Y * c.radius * rng.range(1.1, 1.6);
                stems.graded(root + b * 0.5, a, b, TUFT, up, foot, tip);
            }
        }
    }
    if let Some(cell) = c.flower {
        let yaw = rng.f() * std::f32::consts::TAU;
        let a = Vec3::new(yaw.cos(), 0.0, yaw.sin()) * 0.3;
        let b = Vec3::Y * rng.range(0.3, 0.42);
        let bloom = rgba(Vec3::splat(0.85));
        stems.graded(
            root + b * 0.5,
            a,
            b,
            cell,
            up,
            rgba(Vec3::splat(0.85 * FOOT)),
            bloom,
        );
    }
}

/// A mat of dead grass and reed (`Kind::Mat`): blades lodged flat over one another in
/// layers, darker the deeper they lie, and a few still leaning up out of it, as a marsh's
/// or a moor's thatch lies after a winter (the tufts stand up over broad leaves instead).
fn mat(c: &Clump, root: Vec3, spin: f32, rng: &mut Rng, stems: &mut Cards) {
    for k in 0..4 {
        let yaw = spin + k as f32 * 1.7 + rng.range(-0.4, 0.4);
        let out = Vec3::new(yaw.cos(), 0.0, yaw.sin());
        let rise = rng.range(0.12, 0.35);
        let b = (out * rise.cos() + Vec3::Y * rise.sin()) * c.radius * rng.range(1.3, 1.9);
        let a = Vec3::new(-out.z, 0.0, out.x) * c.radius * 1.5;
        let deep = 0.45 + 0.15 * k as f32;
        stems.graded(
            root + b * 0.5 + Vec3::Y * (0.01 + 0.015 * k as f32),
            a,
            b,
            TUFT,
            |_| Vec3::Y,
            rgba(c.tint * deep * 0.5),
            rgba(c.tint * deep),
        );
    }
    let up = |p: Vec3| ((p - root).with_y(0.0).normalize_or_zero() * 0.5 + Vec3::Y).normalize();
    for k in 0..3 {
        let yaw = spin + 0.5 + k as f32 * 2.1 + rng.range(-0.3, 0.3);
        let out = Vec3::new(yaw.cos(), 0.0, yaw.sin());
        let lean = rng.range(0.35, 0.8);
        let b = (out * lean.sin() + Vec3::Y * lean.cos()) * c.radius * rng.range(1.0, 1.5);
        let a = Vec3::new(-out.z, 0.0, out.x) * c.radius * 1.3;
        stems.graded(
            root + b * 0.5,
            a,
            b,
            TUFT,
            up,
            rgba(c.tint * FOOT * 2.0),
            rgba(c.tint),
        );
    }
}

fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    land: Res<Land>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::foliage::FoliageMaterial>>,
    mut mats: ResMut<crate::foliage::Mats>,
) {
    let mut trees = crate::forest::tree_spots(&land);
    trees.extend(
        super::dress::OAKS
            .iter()
            .map(|&(at, scale, yaw)| Spot { at, scale, yaw }),
    );
    let mut all = clumps(&land, &trees);
    all.extend(floor(&land));
    draw(
        &mut commands,
        &asset,
        &land,
        &mut meshes,
        &mut materials,
        &mut mats,
        &all,
        0x1a_3e6,
    );
}

/// The cards of `clumps`, each card's turn drawn from `seed`: the leaves, the tufts and
/// flowers, and each clump's triangles in order (the scatter's budget counts them).
pub fn cards(land: &Land, clumps: &[Clump], seed: u32) -> (Cards, Cards, Vec<usize>) {
    let mut leaves = Cards::default();
    leaves.leaf = true;
    let mut stems = Cards::default();
    let mut rng = Rng::new(seed);
    let mut each = Vec::with_capacity(clumps.len());
    for c in clumps {
        let before = leaves.triangles() + stems.triangles();
        build(c, land, &mut rng, &mut leaves, &mut stems);
        each.push(leaves.triangles() + stems.triangles() - before);
    }
    (leaves, stems, each)
}

/// Spawns `clumps` as two meshes of cards on the tree crowns' atlas (the leaves fluttering,
/// the tufts and flowers bending), each card's turn drawn from `seed`: the forest's lawn and
/// the marsh's (marsh/lawn.rs). Returns each clump's triangles, as `cards`.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    commands: &mut Commands,
    asset: &AssetServer,
    land: &Land,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<crate::foliage::FoliageMaterial>,
    mats: &mut crate::foliage::Mats,
    clumps: &[Clump],
    seed: u32,
) -> Vec<usize> {
    let (leaves, stems, each) = cards(land, clumps, seed);
    // The tree crowns' cards' material, its leaves fluttering a little and its stems bending.
    let cards = StandardMaterial {
        base_color_texture: Some(asset.load("materials/foliage/cards.png")),
        alpha_mode: AlphaMode::Mask(0.45),
        cull_mode: None,
        perceptual_roughness: 0.8,
        reflectance: 0.2,
        diffuse_transmission: 0.15,
        ..default()
    };
    let flutter = crate::foliage::material(materials, mats, cards.clone(), 0.0, 0.01, false);
    let bend = crate::foliage::material(materials, mats, cards, 0.05, 0.0, false);
    for (mesh, material) in [(leaves.mesh(), flutter), (stems.mesh(), bend)] {
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::default(),
        ));
    }
    each
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mat lies low and lays no broad leaves: most of its cards top out under 0.8 of its
    /// radius over the ground, where every card of a tuft as wide stands above that.
    #[test]
    fn a_mat_lies_low_with_no_leaves_beneath() {
        let land = Land::new();
        let tops = |kind| {
            let c = Clump {
                at: Vec2::new(3.0, 4.0),
                radius: 0.4,
                kind,
                tint: Vec3::splat(0.5),
                flower: None,
            };
            let (mut leaves, mut stems) = (Cards::default(), Cards::default());
            build(&c, &land, &mut Rng::new(7), &mut leaves, &mut stems);
            let ground = land.height(c.at);
            let ys = |cards: Cards| {
                let Some(bevy::mesh::VertexAttributeValues::Float32x3(pos)) =
                    cards.mesh().attribute(Mesh::ATTRIBUTE_POSITION).cloned()
                else {
                    return Vec::new();
                };
                // A card's four corners: its highest over the ground.
                pos.chunks(4)
                    .map(|q| q.iter().map(|p| p[1] - ground).fold(f32::MIN, f32::max))
                    .collect::<Vec<_>>()
            };
            (ys(leaves), ys(stems))
        };
        let (under, mat) = tops(Kind::Mat);
        assert!(
            under.is_empty(),
            "a mat lays {} leaves beneath",
            under.len()
        );
        let low = mat.iter().filter(|&&y| y < 0.32).count();
        assert!(
            low * 2 > mat.len(),
            "{low} of a mat's {} cards lie low: {mat:?}",
            mat.len()
        );
        let (_, tuft) = tops(Kind::Tuft);
        assert!(tuft.iter().all(|&y| y > 0.32), "a tuft's cards: {tuft:?}");
    }

    /// A graded cell keeps its value and loses most of its own hue: the fern's mean, lime
    /// (green over twice its red, blue a seventh), comes out near grey at the same luminance.
    #[test]
    fn grading_takes_a_cells_hue_out_and_keeps_its_value() {
        let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let chroma = |c: Vec3| (c.max_element() - c.min_element()) / c.max_element();
        for kind in [
            Kind::Fern,
            Kind::Tuft,
            Kind::Leaves,
            Kind::Sprig(0),
            Kind::Sprig(2),
        ] {
            let m = cell_mean(kind);
            let out = m * grade(kind);
            assert!(
                (luma(out) - luma(m)).abs() < 1e-4 * luma(m).max(1.0),
                "{kind:?}: {out}"
            );
            assert!(
                chroma(out) < 0.5 * chroma(m),
                "{kind:?}: chroma {} from {}",
                chroma(out),
                chroma(m)
            );
        }
    }

    /// The lawn covers most of the open ground in the frame and none of the fight's: no clump
    /// on trodden earth or within reach of the cast, and most open points have one near.
    #[test]
    fn the_lawn_covers_the_open_ground_and_leaves_the_fight_bare() {
        let land = Land::new();
        let trees = crate::forest::tree_spots(&land);
        let all = clumps(&land, &trees);
        let cast = [
            super::super::KNIGHT,
            super::super::FOE,
            super::super::TARGET,
            super::super::SECOND,
        ];
        for c in &all {
            assert!(
                trodden(c.at) < 0.02,
                "a clump at {} stands on the fight's ground",
                c.at
            );
            for f in cast {
                assert!(
                    f.distance(c.at) > CAST - 1e-3,
                    "a clump at {} is on a fighter at {f}",
                    c.at
                );
            }
        }
        // Open lawn points (where it may grow) with a clump within 0.6 m.
        let (mut open, mut covered) = (0, 0);
        for i in 0..60 {
            for j in 0..60 {
                let p = LO + (HI - LO) * Vec2::new(i as f32 + 0.5, j as f32 + 0.5) / 60.0;
                if !grows(&land, p) {
                    continue;
                }
                open += 1;
                if all.iter().any(|c| c.at.distance(p) < 0.6) {
                    covered += 1;
                }
            }
        }
        assert!(open > 500, "only {open} open points");
        let share = covered as f32 / open as f32;
        assert!(share > 0.8, "the lawn covers {share} of the open ground");
    }
}
