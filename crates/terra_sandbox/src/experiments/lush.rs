//! Spike: can the world feel like a rich forest, as The Witcher or Sons of the Forest do, on
//! our engine at 60 frames a second? Asked by the developers on 2026-10-12 as a test, when the
//! art direction was still flat colour; it is now textured, lightly stylised realism
//! (docs/art-style.md), which this fits. Findings: research/lush-forest-spike.md.
//!
//! The way big games make forests, in the least code that shows it:
//!
//! - **Trees grown in code.** A trunk that splits into limbs, branches and twigs, each a
//!   tube with bark on it, so every tree differs and none is modelled by hand.
//! - **Leaves as cards.** Small squares, each a picture of a spray of oak leaves cut out by
//!   its transparency, in clumps at the ends of the branches. Their normals lean out of the
//!   crown so it lights as one; leaves deep and low in it are darker; light passes through
//!   them; and they move in the wind.
//! - **Levels of detail.** Each tree is drawn three ways, finer near the camera, and not at
//!   all past the draw distance, dithering from one to the next.
//! - **Shadows from a stand-in.** Drawing every leaf card into the shadow maps took nearly
//!   all of the first version's time. A crown casts its shadow from a sparse stand-in that
//!   only the sun sees, and only near the camera.
//! - **Undergrowth near the camera.** Grass is built in squares around wherever the camera
//!   is and taken away behind it; bushes fade out at a distance; paths wind through.
//!
//! Trees and bushes are the game's to cut: they stop the player, and cut as the flora file's
//! leafy trees and bushes do, so the great ones need an iron axe (D96).
//!
//! At one in the afternoon, when sun comes through the leaves in flecks:
//!
//!   cargo run --release -p terra_sandbox -- --experiment play --experiment lush --level yard --time 13
//!
//! Its cost is measured by `cargo xtask bench forest forest-walk`.

use bevy::{
    camera::visibility::{RenderLayers, VisibilityRange},
    image::{ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    light::NotShadowCaster,
    mesh::MeshTag,
    pbr::ParallaxMappingMethod,
    platform::collections::HashMap,
    prelude::*,
    render::{render_resource::TextureFormat, storage::ShaderBuffer},
};
use std::f32::consts::TAU;
use std::sync::Arc;
use terra_gen::smoothstep;
use terra_gen::{Noise, Rng, Seed};
use terra_world::cover::{CoverRules, CoverSource, Growing};
use terra_world::detail::DetailScale;
use terra_world::flora_kinds::Flora;
use terra_world::grown::Cards;
use terra_world::instance::InstanceApp;
use terra_world::mesh_sink::MeshSink;
use terra_world::planet::Planet;
use terra_world::sky_occlusion::{self, SkyOccluders};

pub mod conifers;
mod far;
mod felled;
mod floor;
mod footing;
mod kinds;
mod overgrowth;
mod pivots;
mod seasons;
mod shade;
mod species;
#[cfg(test)]
mod sun_tests;
mod tally;
mod thicket;
mod woods_floor;

use floor::{Crowns, Stand};
use footing::{Footing, Shapes, SolidShape};
use species::{Form, Species};
// The trees' shapes and meshes are the world's (`terra_world::trees`).
use terra_world::trees::build::Build;
use terra_world::trees::card::{Card, Leaf};
use terra_world::trees::crown::{crown_far, crown_near, crown_stand_in};
use terra_world::trees::lump::{crown_clumps, lumps_of};
use terra_world::trees::wood::{FINE, Shape, curved, tube, wood};
use terra_world::trees::{Limb, Tree, crown_seen, skirt};

/// The forest is drawn in grown plants' material, its climate and its bark's relief kept up
/// by `terra_world::grown`.
use terra_world::grown::{Foliage, FoliageExtension};

/// The forest grows between these distances from the start, so the player starts in a
/// clearing with the forest round it.
const CLEARING_M: f32 = 14.0;
const FOREST_M: f32 = 220.0;
/// No two grown trees closer than this.
const SPACING_M: f32 = 7.0;
/// Where a 20 m tree's detail drops, in metres from the camera: full, then fewer branches
/// and cards, then fewer again, then far off its crown as solid lumps; past the last, it is
/// not drawn. A tree's distances grow with its height, so detail follows how big it is on
/// screen rather than how far it is (D203).
const DETAIL_M: [f32; 4] = [45.0, 100.0, 200.0, 420.0];
/// The height the distances are for.
const DETAIL_FOR_M: f32 = 20.0;
/// One level of detail dithers into the next over this share of the distance where they
/// meet, so a change is spread over many metres of walking.
const FADE: f32 = 0.2;
/// A crown casts a dappled shadow from a sparse stand-in of its own cards to the first
/// distance, a solid one from rounded clumps, which the GPU draws far more cheaply, to the
/// second; beyond, none. For a 20 m tree; they grow with its height as its levels do. Card
/// shadows farther out cost 4 ms a frame on the RTX 3080 (D221).
const SHADOW_M: [f32; 2] = [30.0, 100.0];
/// The lumps shrink away over the last of this share of SHADOW_M[1].
const SHADOW_END: f32 = 0.3;
/// A crown's cards drop out between these distances, from all of them to two a clump; past
/// the second it is drawn as those two (D223).
const CARDS_DROP_M: [f32; 2] = [18.0, 110.0];
/// A conifer's cards drop out sooner, to its least by 70 m (D318): its crown is high over a
/// long bole, never seen near, and a pine's pads carry nearly twice the cards its whorls did;
/// drawn whole out to 110 m they were 1.3 million triangles a pass and half a millisecond of
/// the walk's GPU time, for no difference the look measures.
const NEEDLES_DROP_M: [f32; 2] = [18.0, 70.0];
/// Crowns' shadows hand over from their cards to solid lumps between these distances, for
/// every tree alike (D234).
const SHADOW_HANDOVER_M: [f32; 2] = [14.0, 26.0];
/// How much of a trunk one repeat of the bark's picture covers: round it, and along it.
const BARK_M: Vec2 = Vec2::new(2.15, 2.15);
/// The trees' barks (D247): the scan in `materials`, how many metres across its picture is
/// on a trunk `BARK_FOR_M` round, its tint, how deep its furrows are, in metres, and how much
/// moss it grows. Three for oaks, two for pines, two for spruces, in that order. Each tint
/// takes its scan to bark's real albedo, about 0.08 to 0.1 linear on average (sRGB 80 to 90)
/// whatever the scan's own (D264): tinted as the scans came, under Khronos PBR Neutral (D254)
/// trunks read near black. A pine's plates are a scan of 2 m laid over 3, so a big bole's
/// plates are a hand across, as Sons of the Forest's are, not a fine weave (D309); furrows
/// relief-mapped eight centimetres deep from a height drawn out to its full range
/// (`trunk_depth.jpg`), where the scans' own spread a few millimetres either side (D309).
#[allow(clippy::type_complexity)]
const BARKS: [(&str, f32, (f32, f32, f32), f32, f32); 7] = [
    ("oak_bark", 2.15, (0.68, 0.66, 0.63), 0.09, 1.0),
    ("oak_bark", 1.5, (0.62, 0.61, 0.6), 0.08, 1.0),
    ("bark_brown_02", 1.0, (0.86, 0.82, 0.8), 0.05, 1.0),
    ("pine_bark", 3.0, (0.84, 0.76, 0.7), 0.09, 0.35),
    ("pine_bark", 3.0, (0.9, 0.8, 0.72), 0.09, 0.3),
    ("bark_brown_02", 1.0, (0.78, 0.77, 0.76), 0.04, 0.6),
    ("pine_bark", 2.0, (0.74, 0.72, 0.7), 0.06, 0.6),
];
/// The radius of trunk a bark's metres are for: a stouter trunk's bark is coarser, its plates
/// and ridges larger, as an older tree's are, to half as large again; a slender one's finer,
/// to four fifths (D309). One scale on every trunk drew a great oak's furrows as a sapling's.
const BARK_FOR_M: f32 = 0.5;
/// The solid shadow lumps are this much of the crown's size, so a low sun, which looks
/// through many crowns, still comes through in places.
const SHADOW_LUMP: f32 = 0.6;
/// How wide the gaps in a solid shadow lump's shade are, in metres: a Scots pine's tufts with
/// sky between (D359). Cast whole, a pad's lump drew a round black disc on the floor.
const DAPPLE_M: f32 = 0.45;
/// The floor's cards' reflectance (ferns and herbs): a third of a leaf's.
const FLOOR_CARD_REFLECTANCE: f32 = 0.1;
/// The render layer only the sun sees, which the stand-ins are on.
const SUN_ONLY: usize = 1;
/// Veteran oaks' modelled trunks: each model in `models/nature/forest` (near, and `_far`
/// beyond the first level of detail, both made by tools/blender/trunk.py), its clumps in
/// `data/trees` (D235), and which of `BARKS` it is drawn in (D264); how tall the models are,
/// and how thick round their foot.
const VETERANS: [(&str, &str, usize); 2] = [
    ("OakBough", "oak_bough", 2),
    ("OakBough2", "oak_bough_2", 0),
];
const VETERAN_M: f32 = 24.0;
const VETERAN_TRUNK_M: f32 = 2.0;
/// Tree feet are filed in squares this wide, to keep trees apart.
const FILED_M: f32 = 8.0;
/// Where the leaves' picture is cut: more opaque than this is leaf.
const LEAF_CUT: f32 = 0.45;
/// How the crowns' leaves are shaded (D247): their pictures neither greyed nor darkened,
/// since the atlas's leaves are graded to living leaves' colour (D292), where once they were
/// greyed and darkened by 15% from a pale pastel; how brightly a leaf glows lit through
/// toward the sun, a fifth more than when the leaves were paler, as the glow takes their colour;
/// and how dark the inside of a crown goes in the sun's light, at least.
const LEAF: Vec4 = Vec4::new(0.0, 0.0, 1.8, 0.3);
/// A conifer's needles (D318): as a crown's leaves, but lit through far less, as needles are
/// thick and waxy; lit through as a broadleaf's, a pine's crown seen against the sun glowed
/// lime.
const NEEDLES: Vec4 = Vec4::new(0.0, 0.0, 0.6, 0.3);
/// The middle layer's leaves (D359): their cards composed and graded as the crowns' (D292),
/// so neither greyed nor darkened, lit through two thirds as brightly as a crown's, and darker
/// inside than a crown, a bush being dense and the sun seeing into it less. Darkened by a
/// tenth and lit through at less than half a crown's, as when the cards were paler (D269),
/// the bushes beside a path read as flat dark fills a stride off; lit through as a crown's,
/// they glowed evenly from rim to core, as if lit from within.
const THICKET: Vec4 = Vec4::new(0.0, 0.0, 1.2, 0.15);
/// Ivy on the trunks (D309): darkened as the middle layer's leaves, lit through less, and
/// darker where it sees less of the sky, but not as a crown's inside.
const IVY: Vec4 = Vec4::new(0.0, 0.1, 0.5, 0.55);
/// How much of the light through a leaf is Bevy's own diffuse transmission (D279): little, as it
/// takes the open sky through every leaf, unshaded by the crowns, which washed the forest's
/// middle distance teal; the light through a leaf is the foliage shader's, from the sun and
/// from the sky occlusion volume's light above it (D247, D272).
const BEVY_THROUGH: f32 = 0.05;
/// The far crowns' colours, oak, pine and spruce: the leaves' average, as the farthest level
/// of a tree and the far forest past the meadow show them (D273).
const FAR_CROWNS: [Color; 3] = [
    Color::srgb(0.19, 0.3, 0.085),
    Color::srgb(0.11, 0.155, 0.1),
    Color::srgb(0.13, 0.21, 0.12),
];
/// Each kind's crown cards tinted, oak, pine and spruce (D247, D248); the spruce's as it read
/// when the crowns' leaves were greyed and darkened (D292); the Scots pine's tufts are drawn in
/// their own dark grey-green (D318); each `warmed` (D598).
const CROWN_TINTS: [Vec3; 3] = [
    warmed(Vec3::new(0.92, 0.95, 0.85)),
    warmed(Vec3::new(1.05, 1.0, 1.0)),
    warmed(Vec3::new(1.48, 1.36, 0.67)),
];
/// A living crown's olive (D598): every tree's leaves, its far levels' colours with them, have
/// their linear red and blue scaled so. The crowns' median hue was 71 to 78 degrees on a clear
/// day against Sons of the Forest's and Enshrouded's 62 to 63, teal against olive. The sky's
/// light through the leaves (`sky_through` in foliage_fragment.wesl) was not the cause: drawn
/// grey, or not at all, it moved the hue under 1.5 degrees; red 1.35 and blue 0.6 took it
/// to 67, these to 65 (five views at 15:00).
pub(super) const WARM: Vec3 = Vec3::new(1.6, 1.0, 0.5);

/// `colour` warmed to a living crown's olive (`WARM`).
pub(super) const fn warmed(colour: Vec3) -> Vec3 {
    Vec3::new(colour.x * WARM.x, colour.y * WARM.y, colour.z * WARM.z)
}
/// The atlas's oakbranch, Scots pine and spruce pictures' average colour where they are leaf,
/// linear, measured from `materials/foliage/cards.png` by hand-run script: what the far
/// forest's crowns are coloured from, tinted and darkened as the crowns' cards are (D273).
const LEAF_MEANS: [Vec3; 3] = [
    Vec3::new(0.079, 0.128, 0.036),
    Vec3::new(0.015, 0.031, 0.018),
    Vec3::new(0.006, 0.014, 0.007),
];
/// Paths are filed in squares this wide, to find the nearest quickly.
const PATH_SQUARE_M: f32 = 12.0;

pub fn plugin(app: &mut App) {
    app.add_plugins((floor::plugin, seasons::plugin))
        .init_resource::<Unmipped>()
        // Its kinds of tree are the app's, grown once (`kinds`).
        .init_resource::<kinds::Grown>()
        // The forest is the world's (D469): planted again in each world entered.
        .world_resource::<Forest>()
        .add_systems(
            Update,
            (
                // The forest is the world's: planted once its land is made (D623).
                (
                    plant.run_if(not(resource_exists::<Forest>)),
                    far_land.run_if(resource_added::<Forest>),
                    follow_detail,
                )
                    .run_if(terra_world::instance::in_world),
                add_mipmaps,
            ),
        );
}

/// Past the far forest, the same forest's low rolling hills to the horizon in the sky (D273).
fn far_land(mut sky: ResMut<terra_world::sky::Sky>) {
    sky.far_land = Some(terra_world::sky::FarLand {
        canopy: Color::srgb(0.18, 0.27, 0.12),
        hills: 1.1,
        peaks: 0.0,
    });
}

/// The near crowns' cards drop out between distances scaled as every other level of
/// detail is (D230).
fn follow_detail(
    detail: Res<DetailScale>,
    forest: Option<Res<Forest>>,
    mut materials: ResMut<Assets<Foliage>>,
) {
    let Some(forest) = forest else { return };
    if !(detail.is_changed() || forest.is_added()) {
        return;
    }
    scale_detail(
        [&forest.dropping, &forest.dropping_needles],
        [&forest.stand_in, &forest.lumps],
        detail.0,
        &mut materials,
    );
}

/// Scales the distances the crowns' cards, needles first, drop out between, and their
/// shadows' stand-ins, the cards then the lumps, hand over between, by `scale` (D230).
fn scale_detail(
    [leaves, needles]: [&Handle<Foliage>; 2],
    [stand_in, lumps]: [&Handle<Foliage>; 2],
    scale: f32,
    materials: &mut Assets<Foliage>,
) {
    for (dropping, range) in [(leaves, CARDS_DROP_M), (needles, NEEDLES_DROP_M)] {
        if let Some(mut material) = materials.get_mut(dropping) {
            material.extension.cards = (Vec2::from(range) * scale).extend(1.0);
        }
    }
    let [from, to] = SHADOW_HANDOVER_M;
    let end = SHADOW_M[1];
    if let Some(mut material) = materials.get_mut(stand_in) {
        material.extension.shrink = Vec4::new(from, to, 0.0, 0.0) * scale;
    }
    if let Some(mut material) = materials.get_mut(lumps) {
        material.extension.shrink = Vec4::new(end * (1.0 - SHADOW_END), end, from, to) * scale;
    }
}

/// Pictures still to be given smaller copies of themselves once loaded, and the alpha a
/// picture cut out by its transparency is cut at.
#[derive(Resource, Default)]
struct Unmipped(Vec<(Handle<Image>, Option<f32>)>);

/// Gives each picture its mipmaps as it loads: a chain of copies, each half the size, which
/// the GPU reads at a distance instead of the full picture. Without them, bark and leaves
/// far off glitter, and reading the full picture for a few pixels wastes bandwidth.
/// Bevy makes none for a PNG.
fn add_mipmaps(mut unmipped: ResMut<Unmipped>, mut images: ResMut<Assets<Image>>) {
    unmipped.0.retain(|(handle, cut)| {
        let Some(mut image) = images.get_mut(handle) else {
            return true;
        };
        with_mipmaps(&mut image, *cut);
        false
    });
}

/// Adds the chain of halved copies to an 8-bit sRGB picture. Colour is averaged as light
/// adds, in linear terms, and weighted by alpha so the empty parts of a cut-out picture do
/// not bleed into its edges. In a cut-out picture, averaging alone makes the leaves thin
/// out with distance until the crown goes see-through, so each copy's alpha is scaled to
/// keep as much of it above the cut as the full picture has (Castaño, 2010).
fn with_mipmaps(image: &mut Image, cut: Option<f32>) {
    if image.texture_descriptor.format != TextureFormat::Rgba8UnormSrgb
        || image.texture_descriptor.mip_level_count > 1
    {
        return;
    }
    let Some(data) = image.data.clone() else {
        return;
    };
    let linear = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            libm::powf((c + 0.055) / 1.055, 2.4)
        }
    };
    let encoded = |c: f32| {
        let c = if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * libm::powf(c, 1.0 / 2.4) - 0.055
        };
        (c * 255.0).round().clamp(0.0, 255.0) as u8
    };
    let (mut width, mut height) = (image.width() as usize, image.height() as usize);
    let mut level: Vec<[f32; 4]> = data
        .chunks_exact(4)
        .map(|p| {
            [
                linear(p[0]),
                linear(p[1]),
                linear(p[2]),
                f32::from(p[3]) / 255.0,
            ]
        })
        .collect();
    let covered = |pixels: &[[f32; 4]], scale: f32, cut: f32| {
        pixels.iter().filter(|p| p[3] * scale > cut).count() as f32 / pixels.len() as f32
    };
    let wanted = cut.map(|cut| covered(&level, 1.0, cut));
    let mut all = data;
    let mut levels = 1;
    while width > 1 || height > 1 {
        let (half_w, half_h) = ((width / 2).max(1), (height / 2).max(1));
        let mut next = vec![[0.0; 4]; half_w * half_h];
        for y in 0..half_h {
            for x in 0..half_w {
                let mut sum = [0.0f32; 4];
                let mut plain = [0.0f32; 3];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p =
                        level[(y * 2 + dy).min(height - 1) * width + (x * 2 + dx).min(width - 1)];
                    for c in 0..3 {
                        sum[c] += p[c] * p[3];
                        plain[c] += p[c];
                    }
                    sum[3] += p[3];
                }
                next[y * half_w + x] = if sum[3] > 0.0 {
                    [
                        sum[0] / sum[3],
                        sum[1] / sum[3],
                        sum[2] / sum[3],
                        sum[3] / 4.0,
                    ]
                } else {
                    [plain[0] / 4.0, plain[1] / 4.0, plain[2] / 4.0, 0.0]
                };
            }
        }
        // The scale that keeps as much above the cut as the full picture, found by halving.
        let scale = match (cut, wanted) {
            (Some(cut), Some(wanted)) => {
                let (mut low, mut high) = (0.25f32, 8.0f32);
                for _ in 0..16 {
                    let middle = (low + high) / 2.0;
                    if covered(&next, middle, cut) < wanted {
                        low = middle;
                    } else {
                        high = middle;
                    }
                }
                high
            }
            _ => 1.0,
        };
        for p in &next {
            all.extend([encoded(p[0]), encoded(p[1]), encoded(p[2])]);
            all.push(((p[3] * scale).min(1.0) * 255.0).round() as u8);
        }
        level = next;
        (width, height) = (half_w, half_h);
        levels += 1;
    }
    image.data = Some(all);
    image.texture_descriptor.mip_level_count = levels;
}

/// The forest, once planted. Its grass is terra_world's ground cover, told by the forest's
/// map where it grows.
#[derive(Resource)]
struct Forest {
    /// The near crowns' leaves, whose cards drop out between distances that follow how far
    /// detail reaches (D230).
    dropping: Handle<Foliage>,
    /// And the conifers' needles' (D318).
    dropping_needles: Handle<Foliage>,
    /// The shadow stand-ins, whose handover follows how far detail reaches too.
    stand_in: Handle<Foliage>,
    lumps: Handle<Foliage>,
}

/// The floor's own materials, the forest walk's and the woods' alike (D619).
#[derive(Resource, Clone)]
pub(super) struct FloorLooks {
    /// Oak bark, which logs, stumps, sticks and saplings are of.
    bark: Handle<Foliage>,
    /// The floor's cards and the middle layer's.
    floor_cards: Handle<Foliage>,
    thicket_cards: Handle<Foliage>,
    /// Pine bark, for logs and stumps among the conifers, and a young spruce's.
    pine_bark: Handle<Foliage>,
    spruce_bark: Handle<Foliage>,
}

impl FloorLooks {
    fn new(
        assets: &AssetServer,
        materials: &mut Assets<Foliage>,
        wind: &FoliageExtension,
        looks: &Looks,
    ) -> Self {
        // Scanned furrowed bark, as an old oak's is, its colour in the range real bark has
        // (D214), taken to the grey-brown of an old oak, about sRGB 88 (D221, D264).
        let bark = bark_material(
            assets,
            materials,
            wind,
            "oak_bark",
            Color::srgb(0.68, 0.66, 0.63),
            0.03,
            1.0,
        );
        // The floor's and the middle layer's: greyed and darkened a little, so the ferns' and
        // herbs' pictures' spring green reads as leaves in a wood's shade, not lime, and lit
        // through less, as backlit they glowed lime too (D268). Their reflection a third of a
        // leaf's (D359): a fern's frond drawn as one flat card, seen at a grazing angle toward
        // a low sun, mirrored the bright haze round the sun whole.
        let floor_cards = materials.add({
            let mut floor = looks.crown(Vec3::ZERO, Vec4::new(0.12, 0.1, 0.0, 0.0), BEVY_THROUGH);
            floor.base.reflectance = FLOOR_CARD_REFLECTANCE;
            floor
        });
        // The middle layer's, shaded as a crown is: its inside dark, its leaves lit through
        // where the sun is behind them (`THICKET`, D269).
        let thicket_cards = materials.add(looks.crown(Vec3::ZERO, THICKET, BEVY_THROUGH));
        Self {
            bark,
            floor_cards,
            thicket_cards,
            pine_bark: looks.barks[3].0.clone(),
            spruce_bark: looks.barks[5].0.clone(),
        }
    }
}

/// The forest's grass, as terra_world's ground cover asks it, from the floor's map (D246).
struct LushCover(Arc<floor::Floor<Path>>);

impl CoverSource for LushCover {
    /// None on dug ground (a dungeon's digs).
    fn growing(&self, planet: &Planet, ground: Vec3, at: Vec2) -> Growing {
        if planet.dug(ground) {
            return Growing::default();
        }
        self.0.growing(at)
    }

    fn reaches(&self, middle: Vec2, half: f32) -> bool {
        middle.length() < FOREST_M + floor::MEADOW_M + half
    }
}

/// The square of a grid this wide a place on the map is in.
fn square(at: Vec2, side: f32) -> (i32, i32) {
    ((at.x / side).floor() as i32, (at.y / side).floor() as i32)
}

/// How far from the nearest of some points filed by grass square, up to `within`.
fn nearest(filed: &HashMap<(i32, i32), Vec<Vec2>>, at: Vec2, within: f32) -> f32 {
    let (i, j) = square(at, FILED_M);
    let mut nearest = within;
    for di in -1..=1 {
        for dj in -1..=1 {
            for point in filed.get(&(i + di, j + dj)).into_iter().flatten() {
                nearest = nearest.min(point.distance(at));
            }
        }
    }
    nearest
}

/// How open the forest is at a place, low in glades, where only young trees grow.
fn glade_at(glades: &Noise, at: Vec2) -> f32 {
    glades.fbm(Vec3::new(at.x, 0.0, at.y) / 45.0, 2, 2.0, 0.5)
}

/// Which kind of tree grows at a place (D301, D318): the tall stand is Scots pine nearly
/// everywhere, Norway spruces in its damper hollows, where a second field of noise is high,
/// and oaks the exception, in the odd patch where a broad field is lowest; `mixed` moves a
/// patch's edge, so its edges mix.
fn species_at(groves: &Noise, at: Vec2, mixed: f32) -> Species {
    let grove = groves.fbm(Vec3::new(at.x, 0.0, at.y) / 70.0, 2, 2.0, 0.5);
    if grove < OAKS_BELOW + mixed {
        Species::Oak
    } else if groves.sample(Vec3::new(at.x, 40.0, at.y) / 35.0) > SPRUCES_OVER {
        Species::Spruce
    } else {
        Species::Pine
    }
}

/// Where oaks and spruces stand among the pines: below and above these values of their
/// fields of noise (`species_at`), so about one grown tree in ten is an oak and one in five
/// a spruce (D318).
const OAKS_BELOW: f32 = -0.2;
const SPRUCES_OVER: f32 = 0.3;
/// How far apart grown pines stand, of `SPACING_M`, before their knots: closer than the oaks
/// were, as a tall stand's poles are, so a walk among them is among trunks (D301, D318).
const PINES_APART: f32 = 0.62;

/// One kind of tree at each level of detail, and the stand-in its shadow is cast from.
#[derive(Clone)]
struct TreeModel {
    wood: [Handle<Mesh>; 3],
    /// Every card, dropping out with distance, and far off two a clump (D223).
    crown_near: Handle<Mesh>,
    crown_far: Handle<Mesh>,
    shadow: Handle<Mesh>,
    clumps: Handle<Mesh>,
    /// The solid lumps the crown's shadow is cast from beyond the cards: smaller than the
    /// crown, so a low sun still finds gaps between its branches.
    lumps: Handle<Mesh>,
    /// Those lumps' middles and half sizes, for where the sun falls under the crowns.
    shadow_lumps: Arc<sky_occlusion::Lumps>,
    height: f32,
    trunk_m: f32,
    /// Its crown's middle, level with its foot, and how far its leaves reach from it, for
    /// the floor's shade (D246).
    crown: (Vec3, f32),
    /// How far its branches and leaves reach round its foot, within a body's height of the
    /// ground: a spruce's skirt, which stops a walker as its trunk does (D248).
    skirt_m: f32,
    species: Species,
    young: bool,
    bark: Handle<Foliage>,
    /// The far crown's material, in its leaves' colour.
    far: Handle<StandardMaterial>,
    /// Whether its wood is a model rather than grown: then `wood[0]` near the camera and
    /// `wood[1]` beyond, in `bark` laid on from three sides (D264).
    modelled: bool,
    /// Its near level's finest wood, drawn with it but casting no shadow (`wood`, D309).
    fine: Option<Handle<Mesh>>,
    /// The ivies it may wear, each up its trunk to its own height; none where its kind wears
    /// none (D309).
    ivy: Vec<Handle<Mesh>>,
}

/// The kinds of tree the forest grows, each grown once and drawn many times: what it is,
/// its form, whether young, and the heights it is grown between (D182, D235). Grown oaks
/// are tall forest oaks, broad ones, and ones forked low; Scots pines, most of the forest, a
/// tall stand 24 to 34 m high, pointed, flat-topped, forked and broken (D276, D318), Norway
/// spruces as tall in its damper hollows, young spruces under them; oaks the exception.
const KINDS: [(Species, Form, bool, (f32, f32)); 30] = [
    (Species::Oak, Form::Tall, false, (21.0, 27.0)),
    (Species::Oak, Form::Broad, false, (17.0, 23.0)),
    (Species::Oak, Form::Forked, false, (19.0, 25.0)),
    (Species::Oak, Form::Tall, false, (20.0, 26.0)),
    (Species::Oak, Form::Tall, false, (18.0, 24.0)),
    (Species::Oak, Form::Broad, false, (18.0, 22.0)),
    (Species::Oak, Form::Forked, false, (20.0, 26.0)),
    (Species::Oak, Form::Tall, false, (22.0, 28.0)),
    (Species::Oak, Form::Tall, true, (8.0, 10.5)),
    (Species::Oak, Form::Tall, true, (9.0, 12.0)),
    (Species::Oak, Form::Broad, true, (8.0, 11.0)),
    (Species::Pine, Form::Tall, false, (27.0, 31.0)),
    (Species::Pine, Form::Tall, false, (28.0, 33.0)),
    (Species::Pine, Form::Broad, false, (25.0, 29.0)),
    (Species::Pine, Form::Forked, false, (29.0, 34.0)),
    (Species::Spruce, Form::Tall, false, (24.0, 29.0)),
    (Species::Spruce, Form::Tall, false, (26.0, 31.0)),
    (Species::Spruce, Form::Tall, false, (22.0, 27.0)),
    (Species::Spruce, Form::Tall, true, (3.5, 5.0)),
    (Species::Spruce, Form::Tall, true, (4.5, 6.5)),
    (Species::Spruce, Form::Tall, true, (5.5, 8.0)),
    // Added later, after the rest, so each kind above keeps the seed it was grown from.
    (Species::Pine, Form::Tall, false, (24.0, 28.0)),
    (Species::Pine, Form::Broad, false, (28.0, 33.0)),
    (Species::Pine, Form::Forked, false, (26.0, 31.0)),
    (Species::Pine, Form::Broken, false, (24.0, 29.0)),
    (Species::Spruce, Form::Tall, false, (27.0, 32.0)),
    // The tall stand's pines, most of the forest, in more variants (D318).
    (Species::Pine, Form::Tall, false, (26.0, 30.0)),
    (Species::Pine, Form::Tall, false, (30.0, 34.0)),
    (Species::Pine, Form::Broad, false, (26.0, 31.0)),
    (Species::Pine, Form::Forked, false, (27.0, 32.0)),
];

/// A bark's material, `materials/<name>`, tinted: its colour, normal, roughness and
/// crevices, and its furrows relief-mapped from its height, `relief` deep (D218), and how
/// much moss it grows, 0 to 1: a pine's bark grows little. Under materials/, so
/// terra_world gives it its mipmaps.
fn bark_material(
    assets: &AssetServer,
    materials: &mut Assets<Foliage>,
    wind: &FoliageExtension,
    name: &str,
    tint: Color,
    relief: f32,
    moss: f32,
) -> Handle<Foliage> {
    let map = |file: &str, colour: bool| -> Handle<Image> {
        assets
            .load_builder()
            .with_settings(move |settings: &mut ImageLoaderSettings| settings.is_srgb = colour)
            .load(format!("materials/{name}/{file}"))
    };
    let arm = map("arm.jpg", false);
    materials.add(Foliage {
        base: StandardMaterial {
            base_color: tint,
            base_color_texture: Some(map("color.jpg", true)),
            normal_map_texture: Some(map("normal.jpg", false)),
            // Occlusion in red, roughness in green, metal in blue.
            occlusion_texture: Some(arm.clone()),
            metallic_roughness_texture: Some(arm),
            perceptual_roughness: 1.0,
            metallic: 0.0,
            reflectance: 0.3,
            // The furrows read as cut into the trunk, not painted on it.
            depth_map: Some(map("trunk_depth.jpg", false)),
            parallax_depth_scale: relief,
            parallax_mapping_method: ParallaxMappingMethod::Relief { max_steps: 5 },
            max_parallax_layer_count: 16.0,
            ..default()
        },
        extension: FoliageExtension {
            bark: 2.0 - moss,
            ..wind.clone()
        },
    })
}

/// The seed the forest grows from: its kinds of tree, where they stand, and all the rest; the
/// world's own, derived for the forest (D596).
fn forest_seed(planet: &Planet) -> Seed {
    planet.seed().derive("lush forest")
}

/// The foliage shader's settings every material of the forest starts from: the wind blowing
/// along `direction`, and its plants making way as `parting` says.
fn wind(direction: Vec3, parting: Handle<ShaderBuffer>) -> FoliageExtension {
    FoliageExtension {
        direction,
        strength: 1.0,
        bark: 0.0,
        cards: Vec3::ZERO,
        shrink: Vec4::ZERO,
        leaf: Vec4::ZERO,
        parting,
        relief: 0.0,
        dapple: 0.0,
        leaf_normals: None,
        climate: default(),
        climate_ground: None,
        climate_leaves: None,
        // Ahead of the flora's plants in the alpha-mask prepass, as the tall crowns are, so
        // their depth rejects what they hide (D618).
        prepass_priority: -1,
    }
}

/// The materials grown trees are drawn in (D247, D318): their crowns' leaves and needles,
/// whole and dropping out with distance, the stand-ins only the sun sees, the far crowns,
/// each kind's barks, and each kind's picture in the atlas. The lush forest draws its trees
/// in them, and the woods of a world with mountains their conifers (`conifers`).
struct Looks {
    wind: FoliageExtension,
    atlas: Handle<Image>,
    cut: f32,
    leaf_normals: Handle<Image>,
    crown_leaves: Handle<Foliage>,
    /// The near crown's leaves, whose cards drop out with distance (D223), and a conifer's
    /// needles, whole and dropping out (D318).
    dropping_leaves: Handle<Foliage>,
    needle_leaves: Handle<Foliage>,
    dropping_needles: Handle<Foliage>,
    stand_in_leaves: Handle<Foliage>,
    shadow_lumps: Handle<Foliage>,
    far_crowns: [Handle<StandardMaterial>; 3],
    /// `BARKS`, each with how many metres its picture covers.
    barks: Vec<(Handle<Foliage>, Vec2)>,
    leaf_of: [Leaf; 3],
}

impl Looks {
    fn new(
        assets: &AssetServer,
        materials: &mut Assets<Foliage>,
        plain: &mut Assets<StandardMaterial>,
        cards: &Cards,
        wind: &FoliageExtension,
    ) -> Self {
        let mut looks = Self {
            wind: wind.clone(),
            atlas: assets.load(cards.atlas.clone()),
            cut: cards.cut,
            leaf_normals: terra_world::grown::leaf_normals(cards, assets),
            crown_leaves: Handle::default(),
            dropping_leaves: Handle::default(),
            needle_leaves: Handle::default(),
            dropping_needles: Handle::default(),
            stand_in_leaves: Handle::default(),
            shadow_lumps: Handle::default(),
            far_crowns: default(),
            barks: Vec::new(),
            leaf_of: [
                Leaf::of(cards, "oakbranch"),
                // A Scots pine's shoots drawn needle by needle (D318).
                Leaf::of(cards, "scots"),
                Leaf::of(cards, "spruce"),
            ],
        };
        looks.crown_leaves = materials.add(looks.crown(Vec3::ZERO, LEAF, BEVY_THROUGH));
        looks.dropping_leaves =
            materials.add(looks.crown(Vec2::from(CARDS_DROP_M).extend(1.0), LEAF, BEVY_THROUGH));
        looks.needle_leaves = materials.add(looks.crown(Vec3::ZERO, NEEDLES, BEVY_THROUGH));
        looks.dropping_needles = materials.add(looks.crown(
            Vec2::from(NEEDLES_DROP_M).extend(1.0),
            NEEDLES,
            BEVY_THROUGH,
        ));
        // The shadow stand-ins: their cards shrink away over the handover, and the lumps grow in
        // then shrink away at the end of the shadows (D234).
        let shrinking = |out: [f32; 2], grow: [f32; 2]| FoliageExtension {
            shrink: Vec4::new(out[0], out[1], grow[0], grow[1]),
            ..wind.clone()
        };
        looks.stand_in_leaves = materials.add(Foliage {
            base: looks.crown(Vec3::ZERO, LEAF, BEVY_THROUGH).base,
            extension: shrinking(SHADOW_HANDOVER_M, [0.0, 0.0]),
        });
        // The lumps cast a crown's broken, soft-rimmed shade, not discs (D359): alpha-tested, so
        // the shadow pass runs the fragment shader that breaks them up. They cast from their
        // half away from the sun, their front faces culled, as shadow casters often are against
        // acne: the shade they have always cast, the crowns' cards inside them left lit.
        looks.shadow_lumps = materials.add(Foliage {
            base: StandardMaterial {
                alpha_mode: AlphaMode::Mask(0.5),
                cull_mode: Some(bevy::render::render_resource::Face::Front),
                ..default()
            },
            extension: FoliageExtension {
                dapple: DAPPLE_M,
                ..shrinking(
                    [SHADOW_M[1] * (1.0 - SHADOW_END), SHADOW_M[1]],
                    SHADOW_HANDOVER_M,
                )
            },
        });
        // A far crown: its leaves' average colour, rough, lit through from behind a little.
        let mut far_crown = |colour: Color| {
            plain.add(StandardMaterial {
                base_color: colour,
                perceptual_roughness: 0.9,
                reflectance: 0.2,
                diffuse_transmission: 0.2,
                ..default()
            })
        };
        // Warmed as the near crowns are (D598), so a tree's last level keeps their colour.
        looks.far_crowns = FAR_CROWNS.map(|colour| {
            let warm = warmed(colour.to_linear().to_vec3());
            far_crown(Color::linear_rgb(warm.x, warm.y, warm.z))
        });
        // Each kind's bark in several scans or shades, so a stand of them is not one bark (D247):
        // old oaks' deep-furrowed willow, it again larger and greyer (Bark Willow, dark mud
        // without plates, read as fur, D264) and a grey-brown with knots; pines' pine
        // bark, darker and lighter; spruces' the grey-brown scan and a grey pine bark. Each is
        // `BARKS`' entry: what scan, how many metres its picture covers, tint, relief and moss.
        looks.barks = BARKS
            .iter()
            .map(|&(name, metres, (r, g, b), relief, moss)| {
                (
                    bark_material(
                        assets,
                        materials,
                        wind,
                        name,
                        Color::srgb(r, g, b),
                        // From metres to the picture's repeats, which the relief is measured in.
                        relief / metres,
                        moss,
                    ),
                    Vec2::splat(metres),
                )
            })
            .collect();
        looks
    }

    /// A crown's leaves: a little of the light through them from behind is Bevy's, the same
    /// from every angle; the glow toward the sun is the foliage shader's own (`LEAF`, D247),
    /// so a leaf lit from the front keeps most of its colour. The forest floor's plants keep
    /// Bevy's alone, as they had it.
    fn crown(&self, cards_drop: Vec3, leaf: Vec4, transmission: f32) -> Foliage {
        Foliage {
            base: StandardMaterial {
                base_color_texture: Some(self.atlas.clone()),
                alpha_mode: AlphaMode::Mask(self.cut),
                // Seen from both sides, but with the same normal on both, which leans out of
                // the crown, so a card does not flip from lit to dark as it turns.
                cull_mode: None,
                double_sided: false,
                diffuse_transmission: transmission,
                // Each leaf's roughness is the atlas's own, its reflectance a leaf's (D292).
                perceptual_roughness: 0.8,
                reflectance: terra_world::grown::LEAF_REFLECTANCE,
                ..default()
            },
            extension: FoliageExtension {
                cards: cards_drop,
                leaf,
                relief: terra_world::grown::RELIEF,
                leaf_normals: Some(self.leaf_normals.clone()),
                ..self.wind.clone()
            },
        }
    }
}

/// One of `KINDS`, grown from the forest's seed: its model at every level of detail, and
/// what stops a walker at its trunk and at its crown (D267). With `ivy_leaves`, the ivies
/// it may wear (D309). Its meshes go to `meshes`, so the kinds grow on many threads at once.
fn grow(
    kind: usize,
    seed: Seed,
    looks: &Looks,
    ivy_leaves: Option<&[Leaf; 3]>,
    meshes: &MeshSink,
) -> (TreeModel, SolidShape, SolidShape) {
    let (species, form, young, (low, high)) = KINDS[kind];
    let (leaf_of, far_crowns, barks) = (&looks.leaf_of, &looks.far_crowns, &looks.barks);
    let mut dice = seed.derive("tree").index(kind as u64).rng();
    let height = dice.range(low, high);
    let tree = match species {
        Species::Oak => species::oak(&mut dice, height, form, young),
        _ => species::conifer(&mut dice, height, species, form, young),
    };
    let (which, s) = kinds::materials_of(kind);
    let (leaf, tint, far_material) = (leaf_of[s], CROWN_TINTS[s], &far_crowns[s]);
    let (bark, bark_m) = barks[which].clone();
    let bark_m = bark_m * (tree.trunk_m / BARK_FOR_M).clamp(0.8, 1.5);
    // Each limb's and clump's pivot, for wind in levels (D597).
    let pivots = pivots::of(&tree);
    let wood = [0, 1, 2, FINE].map(|detail| wood(&tree, detail, height, bark_m, &pivots.limbs));
    // Every level of the crown, and its shadow, from one set of cards (D221).
    let mut cards = species::crown_cards(&mut dice, &tree, height, leaf);
    for card in cards.iter_mut().flatten() {
        card.tint *= tint;
    }
    // How many cards a clump its crown keeps far off: two for every kind. One for a Scots
    // pine, grown about twice to cover its clump, merged its separate tufts into a blob as
    // wide as a broadleaf's at 40 m (D318).
    let keep = 2;
    let tips: Vec<[[f32; 4]; 2]> = pivots
        .tips
        .iter()
        .zip(&pivots.twigs)
        .map(|(&limb, &twig)| [limb, twig])
        .collect();
    let near = crown_near(&cards, keep, &tips);
    let far = crown_far(&cards, keep, &tips);
    let shadow = crown_stand_in(&cards, &tips);
    let clumps = crown_clumps(&tree, 1.0, false, &pivots.tips);
    let lumps = crown_clumps(&tree, SHADOW_LUMP, true, &pivots.tips);
    info!(
        "lush: tree {kind}, {species:?} {form:?}{}, {height:.0} m: wood {} {} {} triangles, crown {} near and {} far, shadows {} {}",
        if young { " young" } else { "" },
        wood[0].triangles() + wood[FINE].triangles(),
        wood[1].triangles(),
        wood[2].triangles(),
        near.triangles(),
        far.triangles(),
        shadow.triangles(),
        clumps.triangles()
    );
    // What stops a walker, from the finest wood and the nearest crown (D267).
    let trunk = SolidShape::trunk(&wood[0].positions, &wood[0].indices, tree.trunk_m);
    let seen = SolidShape::crown(&near.positions, &near.indices);
    let wood = wood.map(|build| {
        // Bark has a normal map, which needs the tubes' tangents.
        let mut mesh = build.mesh();
        mesh.generate_tangents()
            .expect("the tubes have normals and texture places");
        meshes.add(mesh)
    });
    let [near_wood, mid_wood, far_wood, fine] = wood;
    let wood = [near_wood, mid_wood, far_wood];
    let crown_near = meshes.add(near.mesh());
    // Its ivies, each climbing to its own height, from dice of their own so the tree
    // itself grows as it did (D309).
    let grows = overgrowth::Overgrowth::of(species);
    let ivy = match ivy_leaves {
        Some(ivy_leaves) if !young && grows.ivy > 0.0 => (0..overgrowth::IVY_VARIANTS)
            .map(|variant| {
                let mut dice = seed
                    .derive("ivy")
                    .index((kind * overgrowth::IVY_VARIANTS + variant) as u64)
                    .rng();
                let (low, high) = grows.ivy_m;
                let share = (variant as f32 + dice.unit()) / overgrowth::IVY_VARIANTS as f32;
                let top = low + (high - low) * share;
                let mut build = Build::default();
                for card in overgrowth::ivy(&mut dice, &tree, height, top, ivy_leaves) {
                    card.draw(&mut build, 1.0);
                }
                meshes.add(build.mesh())
            })
            .collect(),
        _ => Vec::new(),
    };
    let model = TreeModel {
        fine: Some(fine),
        ivy,
        wood,
        crown_near,
        crown_far: meshes.add(far.mesh()),
        shadow: meshes.add(shadow.mesh()),
        clumps: meshes.add(clumps.mesh()),
        lumps: meshes.add(lumps.mesh()),
        shadow_lumps: Arc::new(sky_occlusion::Lumps::new(lumps_of(&tree, SHADOW_LUMP))),
        height,
        trunk_m: tree.trunk_m,
        crown: floor::crown_of(&tree.tips),
        skirt_m: skirt(&tree),
        species,
        young,
        bark,
        far: far_material.clone(),
        modelled: false,
    };
    (model, trunk, seen)
}

/// From one distance to another, crossfading at each end as the levels do.
fn between(from: f32, to: f32) -> VisibilityRange {
    VisibilityRange {
        start_margin: from * (1.0 - FADE)..from,
        end_margin: to * (1.0 - FADE)..to,
        use_aabb: false,
    }
}

/// A crown's shadow handing over from cards to lumps (D234): the cards shrink away and the
/// lumps grow in over SHADOW_HANDOVER_M, the same for every tree as Horizon Zero Dawn keeps
/// one shadow detail a cascade, then the lumps shrink away toward SHADOW_M. Bevy's shadow
/// passes do not dither, so a caster that simply ended would vanish at once.
fn handover(cards: bool, _size: f32) -> VisibilityRange {
    let [from, to] = SHADOW_HANDOVER_M;
    if cards {
        VisibilityRange::abrupt(0.0, to)
    } else {
        VisibilityRange::abrupt(from, SHADOW_M[1])
    }
}

/// Where a crown of two cards a clump gives way to lumps.
fn far_from(size: f32) -> f32 {
    (DETAIL_M[2] * size).max(CARDS_DROP_M[1] * 1.4)
}

/// Levels `first` to `last` of a tree this many times the height the distances are for.
fn detail(first: usize, last: usize, size: f32) -> VisibilityRange {
    let at = |level: usize| DETAIL_M[level] * size;
    let from = if first == 0 { 0.0 } else { at(first - 1) };
    VisibilityRange {
        start_margin: from * (1.0 - FADE)..from,
        end_margin: at(last) * (1.0 - FADE)..at(last),
        use_aabb: false,
    }
}

/// Draws a tree's levels of detail as children of its entity, which stands it where it
/// grows, `size` times the height the distances are for: its wood, its crown thinning card
/// by card and then far off, with `lumps`, as lumps, and the stand-ins its shadow is cast
/// from; with its band of moss (`moss_tag`, 0 for none) and its ivy and the ivy's material,
/// if it wears any. Without lumps, it ends where its crown's cards do, for a billboard to
/// stand for it beyond, as the woods' far cards do (D366). Gives how far it is drawn, in
/// metres from the eye.
fn draw(
    tree: &mut ChildSpawnerCommands,
    model: &TreeModel,
    looks: &Looks,
    size: f32,
    lumps: bool,
    moss_tag: u32,
    ivy: Option<(&Handle<Mesh>, &Handle<Foliage>)>,
) -> f32 {
    let end = if lumps {
        DETAIL_M[3] * size
    } else {
        far_from(size)
    };
    // A modelled trunk is its near mesh, then its far one at every distance
    // beyond, casting its shadow throughout.
    if model.modelled {
        for (level, last) in [(0, 0), (1, 3)] {
            tree.spawn((
                Mesh3d(model.wood[level].clone()),
                MeshMaterial3d(model.bark.clone()),
                detail(level, last, size),
            ));
        }
    }
    for level in 0..if model.modelled { 0 } else { 3 } {
        // The simplest trunk stands on under the far crown too.
        let last = if level == 2 { 3 } else { level };
        let mut range = detail(level, last, size);
        if level == 2 {
            range.end_margin = end * (1.0 - FADE)..end;
        }
        let mut wood = tree.spawn((
            Mesh3d(model.wood[level].clone()),
            MeshMaterial3d(model.bark.clone()),
            range,
        ));
        if level == 2 {
            wood.insert(NotShadowCaster);
        }
        // A band of moss round its foot, if this one has one (D309). Only a
        // grown tree's: a young one's tag is its fade (terra_world's parting).
        if moss_tag != 0 {
            wood.insert(MeshTag::new(moss_tag));
        }
    }
    if let Some(fine) = &model.fine {
        let mut twigs = tree.spawn((
            Mesh3d(fine.clone()),
            MeshMaterial3d(model.bark.clone()),
            detail(0, 0, size),
            NotShadowCaster,
        ));
        if moss_tag != 0 {
            twigs.insert(MeshTag::new(moss_tag));
        }
    }
    // Ivy up its trunk, if this one wears it, as far as its trunk's second
    // level is drawn (D309).
    if let Some((ivy, ivy_material)) = ivy {
        tree.spawn((
            Mesh3d(ivy.clone()),
            MeshMaterial3d(ivy_material.clone()),
            detail(0, 1, size),
            NotShadowCaster,
        ));
    }
    // The crown thins card by card out to CARDS_DROP_M[1], a conifer's to
    // NEEDLES_DROP_M[1], then is those it keeps a clump, the same cards at the
    // same size, then lumps (D223).
    let (dropping, kept, [_, dropped]) = if model.species == Species::Oak {
        (&looks.dropping_leaves, &looks.crown_leaves, CARDS_DROP_M)
    } else {
        (
            &looks.dropping_needles,
            &looks.needle_leaves,
            NEEDLES_DROP_M,
        )
    };
    tree.spawn((
        Mesh3d(model.crown_near.clone()),
        MeshMaterial3d(dropping.clone()),
        between(0.0, dropped),
        NotShadowCaster,
    ));
    tree.spawn((
        Mesh3d(model.crown_far.clone()),
        MeshMaterial3d(kept.clone()),
        between(dropped, far_from(size)),
        NotShadowCaster,
    ));
    // Far off, the crown as its solid lumps in the leaves' colour, so the
    // forest carries on past where its cards end.
    if lumps {
        tree.spawn((
            Mesh3d(model.clumps.clone()),
            MeshMaterial3d(model.far.clone()),
            between(far_from(size), end),
            NotShadowCaster,
        ));
    }
    tree.spawn((
        Mesh3d(model.shadow.clone()),
        MeshMaterial3d(looks.stand_in_leaves.clone()),
        handover(true, size),
        RenderLayers::layer(SUN_ONLY),
    ));
    tree.spawn((
        Mesh3d(model.lumps.clone()),
        MeshMaterial3d(looks.shadow_lumps.clone()),
        handover(false, size),
        RenderLayers::layer(SUN_ONLY),
    ));
    end
}

#[allow(clippy::too_many_arguments)]
fn plant(
    mut commands: Commands,
    planet: Option<Res<Planet>>,
    flora: Option<Res<Flora>>,
    sun: Option<Single<Entity, With<DirectionalLight>>>,
    assets: Res<AssetServer>,
    (mut shapes, mut grown, mut variants): (
        ResMut<Shapes>,
        ResMut<kinds::Grown>,
        ResMut<floor::Variants>,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Foliage>>,
    mut plain: ResMut<Assets<StandardMaterial>>,
    mut unmipped: ResMut<Unmipped>,
    mut mossy: ResMut<Assets<floor::Mossy>>,
    mut mossy_scans: ResMut<floor::MossyScans>,
    mut far_forest: ResMut<Assets<far::FarForest>>,
    mut trunks: ResMut<terra_world::flora::Trunks>,
    parting: Option<Res<terra_world::parting::Parting>>,
    // Where the sun stands as the floor is laid, for what grows in its patches.
    sky: (
        Option<Res<terra_world::daylight::SkyPath>>,
        Option<Res<terra_world::daylight::Clock>>,
    ),
) {
    let (Some(planet), Some(flora), Some(sun)) = (planet, flora, sun) else {
        return;
    };
    // The shapes of the meshes a world left behind (its logs and stumps, its rocks loaded
    // again), which nothing will ask for again (instances j10).
    shapes.0.retain(|mesh, _| meshes.contains(*mesh));
    // The sun sees the stand-ins as well as everything the camera sees.
    commands
        .entity(*sun)
        .insert(RenderLayers::from_layers(&[0, SUN_ONLY]));
    let start = planet.start();
    let up = start.normalize();
    // The map's directions are the game's, as --at and --face give them: east, and south,
    // which is away from where the yard's base faces.
    let east = up.cross(Vec3::X).normalize().cross(up);
    let south = east.cross(up);
    let seed = forest_seed(&planet);

    let parting = parting.map(|parting| parting.0.clone()).unwrap_or_default();
    let wind = wind(east, parting.clone());
    // Read at a slant, as leaves edge-on are, a picture blurs unless sampled along the
    // slant too (anisotropic filtering).
    let slanted = |settings: &mut ImageLoaderSettings| {
        settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            anisotropy_clamp: 8,
            ..ImageSamplerDescriptor::linear()
        });
    };
    let leaf_picture: Handle<Image> = assets
        .load_builder()
        .with_settings(slanted)
        .load("textures/forest_spike/leaves.png");
    unmipped.0.push((leaf_picture.clone(), Some(LEAF_CUT)));
    // Scanned furrowed bark, as an old oak's is, its colour in the range real bark has (D214),
    // with the relief and roughness that let it catch the sky's light in the shade. The scan
    // is at the top of bark's range; this takes it to the grey-brown of an old oak, about
    // sRGB 88, as the first of `BARKS` (D221, D264). Fallen logs are of it too.
    // The crowns' leaves, oak and conifer alike, are cards of the atlas grown plants use, so
    // all of them are drawn in one material (D222); the trees' materials are `Looks`.
    let cards = Cards::read();
    let looks = Looks::new(&assets, &mut materials, &mut plain, &cards, &wind);
    let FloorLooks {
        bark,
        floor_cards,
        thicket_cards,
        pine_bark,
        spruce_bark,
    } = FloorLooks::new(&assets, &mut materials, &wind, &looks);
    commands.insert_resource(felled::StumpLooks {
        bark: bark.clone(),
        pine_bark: pine_bark.clone(),
        end_grain: floor::end_grain(&assets, &mut plain),
    });
    // Ivy on the trunks (D309): the forest floor's ivy cards (litter.ron, D275), its own
    // material, shaded as the middle layer's leaves are but lit through less, as ivy's leaves
    // are thick, and less dark inside, as it lies on the outside of its trunk.
    let litter = Cards::read_file(terra_world::cover::litter::FILE);
    let ivy_leaves = [
        Leaf::of(&litter, "ivy"),
        Leaf::of(&litter, "ivydark"),
        Leaf::of(&litter, "ivydark"),
    ];
    let ivy_material = materials.add(Foliage {
        base: StandardMaterial {
            base_color_texture: Some(assets.load(litter.atlas.clone())),
            alpha_mode: AlphaMode::Mask(litter.cut),
            cull_mode: None,
            double_sided: false,
            diffuse_transmission: BEVY_THROUGH,
            perceptual_roughness: 0.7,
            reflectance: terra_world::grown::LEAF_REFLECTANCE,
            ..default()
        },
        extension: FoliageExtension {
            leaf: IVY,
            relief: terra_world::grown::RELIEF,
            leaf_normals: Some(terra_world::grown::leaf_normals(&litter, &assets)),
            ..wind.clone()
        },
    });

    // The kinds of tree, then the veteran oaks (`kinds`), in this world's materials.
    let trees = kinds::trees(
        &mut grown,
        &kinds::Grow {
            seed,
            looks: &looks,
            wind: &wind,
            ivy_leaves: &ivy_leaves,
            assets: &assets,
        },
        &mut meshes,
        &mut materials,
        &mut shapes,
    );
    let veterans_kinds = KINDS.len()..trees.len();

    let path = Path::new(seed.derive("path"));
    // For scripted runs: --face this to look, or walk, down a path from the start.
    let way = path.lines[1][path.lines[1].len() - 1].0;
    info!(
        "lush: a path leads out to {:.0},{:.0} (east, south).",
        way.x, way.y
    );

    let cut_as = |name: &str, size: f32| flora.cut_rule(name).map(|rule| rule.at(size));
    let place_on = |at: Vec2, dice: &mut Rng, sink: f32, scale: f32| {
        let ground = planet.ground_at(start + east * at.x + south * at.y);
        let up_here = ground.normalize();
        let turn = Quat::from_axis_angle(up_here, dice.range(0.0, TAU));
        Transform {
            translation: ground - up_here * sink,
            rotation: turn * Quat::from_rotation_arc(Vec3::Y, up_here),
            scale: Vec3::splat(scale),
        }
    };

    // Grown trees, tried at random over the patch and kept if far enough from the others
    // and off the paths; then young ones in the gaps between them. Pines grow nearly
    // everywhere, spruces in the stand's hollows, where a field of noise is high, oaks in the
    // odd patch where another is lowest, the patches' edges mixed (`species_at`, D318).
    let mut dice = seed.derive("places").rng();
    let glades = Noise::new(seed.derive("glades"));
    let groves = Noise::new(seed.derive("groves"));
    let clumps = Noise::new(seed.derive("clumps"));
    let trees = &trees;
    let pool = |species: Species, young: bool| -> Vec<usize> {
        let fits = |s: Species| -> Vec<usize> {
            (0..trees.len())
                .filter(|&k| trees[k].species == s && trees[k].young == young)
                .collect()
        };
        let kinds = fits(species);
        // No young pines: a young conifer is a spruce.
        if kinds.is_empty() {
            fits(Species::Spruce)
        } else {
            kinds
        }
    };
    let mut veterans: Vec<Vec2> = Vec::new();
    let pools: [[Vec<usize>; 2]; 3] =
        [Species::Oak, Species::Pine, Species::Spruce].map(|s| [pool(s, false), pool(s, true)]);
    let mut by_species = [0; 3];
    // The nearest grown tree of each kind to the start, for scripted runs to look at.
    let mut nearest_of = [Vec2::splat(f32::MAX); 3];
    let mut ivied = [(0, Vec2::splat(f32::MAX)); 3];
    let mut banded = [(0, Vec2::splat(f32::MAX)); 3];
    let mut feet: HashMap<(i32, i32), Vec<Vec2>> = HashMap::default();
    // The trees as the floor sees them, for what grows under them (D246).
    let mut canopy = Crowns::default();
    // And as the sky's light sees them, for how much of it reaches under them (D270).
    let mut crowns = Vec::new();
    let mut shadows = Vec::new();
    let mut planted = [0, 0];
    // The nearest of each to the start, for scripted runs to walk up to.
    let mut nearest_to_start = [Vec2::splat(f32::MAX); 2];
    for (young, tries, apart) in [(false, 14_000, SPACING_M), (true, 8_000, 4.5)] {
        for _ in 0..tries {
            let angle = dice.range(0.0, TAU);
            let distance = libm::sqrtf(dice.range(CLEARING_M * CLEARING_M, FOREST_M * FOREST_M));
            let at = Vec2::new(libm::cosf(angle), libm::sinf(angle)) * distance;
            // Glades open here and there, where only young trees grow.
            let glade = glade_at(&glades, at);
            let species = species_at(&groves, at, dice.range(-0.12, 0.12));
            // Scots pines are light-hungry and their stands open under high crowns: there a
            // young conifer, which is a spruce, comes up only in one place in four, so a pine
            // stand reads as tall clean pines over an open floor, not a spruce thicket
            // (D289). By where it is, and after the dice are thrown for it, so every other
            // tree keeps its place; its room stays open.
            let scatter = (libm::sinf(at.x * 12.9898 + at.y * 78.233) * 43_758.547)
                .fract()
                .abs();
            let left_out = young && species == Species::Pine && scatter < 0.75;
            // Conifers stand closer than oaks, whose crowns spread. Trees stand in clumps
            // with gaps between, not evenly spaced as a plantation's (D247).
            // Grown pines, self-sown, stand in knots of a few two or three metres apart with
            // open ground between, on a finer field and each a little nearer or farther than
            // its neighbours, so a grove is not evenly spaced poles (D276). Spruces keep their
            // room: their skirts, which stop a walker, would close a knot into a wall.
            let clumped = 0.5 + 0.5 * clumps.fbm(Vec3::new(at.x, 0.0, at.y) / 22.0, 2, 2.0, 0.5);
            let apart = apart
                * match species {
                    Species::Pine if !young => {
                        let knots = smoothstep(
                            -0.25,
                            0.25,
                            clumps.fbm(Vec3::new(at.x, 30.0, at.y) / 9.0, 2, 2.0, 0.5),
                        );
                        PINES_APART * (0.6 + 1.4 * knots) * dice.range(0.8, 1.2)
                    }
                    Species::Oak => 0.65 + 0.8 * clumped.clamp(0.0, 1.0),
                    _ => 0.75 * (0.65 + 0.8 * clumped.clamp(0.0, 1.0)),
                };
            if path.edge_distance(at) < 2.5
                || (!young && glade < -0.25)
                || nearest(&feet, at, apart) < apart
            {
                continue;
            }
            feet.entry(square(at, FILED_M)).or_default().push(at);
            let choices = &pools[species as usize][young as usize];
            let mut kind = choices[dice.below(choices.len() as u32) as usize];
            // At the edge of a glade, where it had room, an oak may be a veteran, its crown
            // spreading over the open ground; never two near.
            if species == Species::Oak
                && !young
                && glade < -0.05
                && dice.unit() < 0.4
                && veterans.iter().all(|v: &Vec2| v.distance(at) > 40.0)
            {
                kind = veterans_kinds.start + dice.below(veterans_kinds.len() as u32) as usize;
                veterans.push(at);
            }
            let model = &trees[kind];
            // A veteran's model is 24 m tall; it stands about 20.
            let scale = dice.range(0.85, 1.15)
                * if veterans_kinds.contains(&kind) {
                    0.82
                } else {
                    1.0
                };
            // In steps of a twentieth, so trees share their distances: Bevy draws at most
            // 65,535 distinct ones, and past that shares a slot and culls wrongly (D230).
            let size =
                ((model.height * scale / DETAIL_FOR_M).clamp(0.45, 1.4) * 20.0).round() / 20.0;
            // Sunk a little, so the root flare meets uneven ground.
            let mut place = place_on(at, &mut dice, 0.3, scale);
            // A conifer of a kind stouter or more slender than the next, and its crown with
            // it, so a stand of one kind is not one trunk over and over (D276).
            let wide = scale
                * if model.species == Species::Oak {
                    1.0
                } else {
                    dice.range(0.85, 1.18)
                };
            place.scale = Vec3::new(wide, scale, wide);
            // Rolled as the rest, then left out on dug ground (a dungeon's digs).
            if left_out || planet.dug(place.translation) {
                continue;
            }
            by_species[model.species as usize] += 1;
            // What grows on it, by where it stands (D309).
            let grows = overgrowth::Overgrowth::of(model.species);
            let ivy =
                (!model.ivy.is_empty() && overgrowth::chance(at, 1.0) < grows.ivy).then(|| {
                    &model.ivy[(overgrowth::chance(at, 2.0) * model.ivy.len() as f32) as usize
                        % model.ivy.len()]
                });
            let moss_tag =
                if !model.young && !model.modelled && overgrowth::chance(at, 3.0) < grows.moss {
                    let (low, high) = grows.moss_m;
                    overgrowth::moss_tag(
                        low + (high - low) * overgrowth::chance(at, 4.0),
                        model.height,
                    )
                } else {
                    0
                };
            let foot = place.translation;
            let crown = place.rotation * (model.crown.0 * place.scale);
            shadows.push(sky_occlusion::CrownShadow {
                foot,
                turn: place.rotation,
                scale: place.scale,
                lumps: model.shadow_lumps.clone(),
            });
            trunks.add(terra_world::flora::Trunk {
                foot,
                radius_m: model.trunk_m * wide,
                height_m: model.height * scale,
                solid: true,
            });
            canopy.add(Stand::of(
                at,
                at + Vec2::new(crown.dot(east), crown.dot(south)),
                model.crown.1 * wide,
                model.species,
                model.young,
                model.trunk_m * wide,
            ));
            crowns.push(crown_seen(
                foot + crown,
                place.rotation * Vec3::Y,
                model.height * scale,
                model.crown.1 * wide,
                model.species,
                model.young,
            ));
            commands
                .spawn((
                    // Named, so a check can find it and walk into it (tools/sandbox/collide.ts).
                    Name::new(format!("{:?}", model.species)),
                    place,
                    Visibility::default(),
                    felled::standing(
                        model,
                        cut_as(felled::cut_kind(model.species), model.height * scale),
                        &place,
                    ),
                ))
                // A young tree is a bush to the camera: it fades as a whole while it is
                // between the camera and the player, its boughs not solid to the camera's
                // boom (D274).
                .insert_if(terra_world::parting::ObstructionFade::default(), || {
                    model.young
                })
                .with_children(|tree| {
                    draw(
                        tree,
                        model,
                        &looks,
                        size,
                        true,
                        moss_tag,
                        ivy.map(|ivy| (ivy, &ivy_material)),
                    );
                });
            planted[young as usize] += 1;
            // How many wear ivy and a band of moss, and the nearest of each to the start, for
            // scripted runs to look at (D309).
            for (worn, tally) in [(ivy.is_some(), &mut ivied), (moss_tag != 0, &mut banded)] {
                let (count, nearest) = &mut tally[model.species as usize];
                if worn {
                    *count += 1;
                    if at.length() < nearest.length() {
                        *nearest = at;
                    }
                }
            }
            let of = &mut nearest_of[model.species as usize];
            if !young && at.length() < of.length() {
                *of = at;
            }
            if at.length() < nearest_to_start[young as usize].length() {
                nearest_to_start[young as usize] = at;
            }
        }
    }
    tally::Tally {
        veterans: &veterans,
        by_species,
        nearest_of,
        ivied,
        banded,
        planted,
        nearest_to_start,
    }
    .tell();

    // The forest carried on past the meadow (D273).
    far::lay(
        far::Lay {
            planet: &planet,
            start,
            east,
            south,
            seed,
            glades: &glades,
            groves: &groves,
            clumps: &clumps,
            leaves: leaf_picture.clone(),
            crowns: [0, 1, 2].map(|k| LEAF_MEANS[k] * CROWN_TINTS[k] * (1.0 - LEAF.y)),
            parting,
        },
        &mut commands,
        &mut meshes,
        &mut far_forest,
    );

    // The floor under the trees, and the map the grass grows by (D246).
    let occluders = SkyOccluders {
        middle: start,
        up,
        east,
        // Past the forest's edge, so where it ends, it sees the open sky as the environment
        // map does, in the meadow beyond where the edge is looked at from.
        half_m: FOREST_M + 70.0,
        below_m: 15.0,
        above_m: 45.0,
        crowns,
        shadows,
    };
    let sun_way = match sky {
        (Some(path), Some(clock)) => Some(path.sun(clock.hour)),
        _ => None,
    };
    let mut map = floor::Floor::new(path, canopy, seed.derive("patches"));
    floor::lay(
        &mut floor::Lay {
            commands: &mut commands,
            planet: &planet,
            start,
            east,
            south,
            assets: &assets,
            meshes: &mut meshes,
            plain: &mut plain,
            mossy: &mut mossy,
            mossy_scans: &mut mossy_scans,
            flora: &flora,
            cards: &cards,
            variants: &mut variants,
            bark: bark.clone(),
            pine_bark,
            spruce_bark,
            atlas: floor_cards,
            thicket: thicket_cards,
            seed,
            foliage: &mut materials,
            shrinking: HashMap::default(),
            shade: shade::NearShadows::new(start, east, south),
            sun: sun_way.map(|way| (&occluders, way)),
        },
        &mut map,
        &floor::Area::YARD,
    );
    commands.insert_resource(occluders);
    let map = Arc::new(map);
    commands.insert_resource(CoverRules {
        source: Arc::new(LushCover(map)),
        seed: seed.derive("grass"),
    });
    commands.insert_resource(Forest {
        dropping: looks.dropping_leaves.clone(),
        dropping_needles: looks.dropping_needles.clone(),
        stand_in: looks.stand_in_leaves.clone(),
        lumps: looks.shadow_lumps.clone(),
    });
}

/// Paths through the forest: a ring round the clearing, and two ways out from it to the
/// forest's edge. Each is a line of points with the path's half-width at each.
struct Path {
    lines: Vec<Vec<(Vec2, f32)>>,
    /// Which pieces of which lines pass near each square.
    near: HashMap<(i32, i32), Vec<(usize, usize)>>,
}

impl Path {
    fn new(seed: Seed) -> Self {
        let mut dice = seed.rng();
        let mut lines: Vec<Vec<(Vec2, f32)>> = Vec::new();
        let (a, b) = (dice.range(0.0, TAU), dice.range(0.0, TAU));
        lines.push(
            (0..=200)
                .map(|i| {
                    let t = i as f32 / 200.0 * TAU;
                    let r = 75.0 + 15.0 * libm::sinf(3.0 * t + a) + 6.0 * libm::sinf(5.0 * t + b);
                    let width = 1.1 + 0.25 * libm::sinf(7.0 * t + b);
                    (Vec2::new(libm::cosf(t), libm::sinf(t)) * r, width)
                })
                .collect(),
        );
        let heading = dice.range(0.0, TAU);
        for way in 0..2 {
            let angle = heading + way as f32 * std::f32::consts::PI + dice.range(-0.6, 0.6);
            let along = Vec2::new(libm::cosf(angle), libm::sinf(angle));
            let across = along.perp();
            let phase = dice.range(0.0, TAU);
            lines.push(
                (0..=150)
                    .map(|i| {
                        let t = 6.0 + i as f32 * 1.5;
                        // Gently, so a walk straight along it stays on it, for benchmarks.
                        let wander = 2.5 * libm::sinf(t / 25.0 + phase) + 0.8 * libm::sinf(t / 9.0);
                        let width = 0.9 + 0.2 * libm::sinf(t / 13.0 + phase);
                        (
                            along * t + across * wander * smoothstep(6.0, 30.0, t),
                            width,
                        )
                    })
                    .collect(),
            );
        }
        let mut near: HashMap<(i32, i32), Vec<(usize, usize)>> = HashMap::default();
        for (l, line) in lines.iter().enumerate() {
            for (p, pair) in line.windows(2).enumerate() {
                let (low, high) = (pair[0].0.min(pair[1].0), pair[0].0.max(pair[1].0));
                let (i0, j0) = square(low - 2.0, PATH_SQUARE_M);
                let (i1, j1) = square(high + 2.0, PATH_SQUARE_M);
                for i in i0..=i1 {
                    for j in j0..=j1 {
                        near.entry((i, j)).or_default().push((l, p));
                    }
                }
            }
        }
        Self { lines, near }
    }
}

impl floor::Ways for Path {
    fn edge_distance(&self, at: Vec2) -> f32 {
        Path::edge_distance(self, at)
    }

    fn beside(&self, dice: &mut Rng, out_m: f32) -> Option<Vec2> {
        Some(Path::beside(self, dice, out_m))
    }
}

impl Path {
    /// How far a place is outside the nearest path's edge, negative on it; a large number
    /// far from every path.
    fn edge_distance(&self, at: Vec2) -> f32 {
        let (i, j) = square(at, PATH_SQUARE_M);
        let mut nearest = PATH_SQUARE_M;
        for di in -1..=1 {
            for dj in -1..=1 {
                for &(l, p) in self.near.get(&(i + di, j + dj)).into_iter().flatten() {
                    let ((a, wa), (b, wb)) = (self.lines[l][p], self.lines[l][p + 1]);
                    let t = ((at - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
                    nearest = nearest.min(at.distance(a + (b - a) * t) - (wa + (wb - wa) * t));
                }
            }
        }
        nearest
    }

    /// A place just outside a path's edge, on one side or the other, up to so far out: where
    /// what lines a path is tried, since the paths are about a hundredth of the forest's floor.
    fn beside(&self, dice: &mut Rng, out_m: f32) -> Vec2 {
        let pieces: usize = self.lines.iter().map(|line| line.len() - 1).sum();
        let mut piece = dice.below(pieces as u32) as usize;
        let line = self
            .lines
            .iter()
            .find(|line| {
                let here = piece < line.len() - 1;
                if !here {
                    piece -= line.len() - 1;
                }
                here
            })
            .expect("a piece of a line");
        let ((a, wa), (b, wb)) = (line[piece], line[piece + 1]);
        let t = dice.unit();
        let side = if dice.unit() < 0.5 { 1.0 } else { -1.0 };
        let across = (b - a).normalize_or_zero().perp() * side;
        a.lerp(b, t) + across * (wa + (wb - wa) * t + dice.range(0.0, out_m))
    }
}
