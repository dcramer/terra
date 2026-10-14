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

use bevy::gltf::GltfMaterial;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::{RenderLayers, VisibilityRange},
    image::{ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension, ParallaxMappingMethod},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::{AsBindGroup, TextureFormat},
    shader::ShaderRef,
};
use std::f32::consts::{PI, TAU};
use std::sync::Arc;
use terra_gen::{Noise, Rng, Seed};
use terra_world::cover::{CoverRules, CoverSource, Growing};
use terra_world::detail::DetailScale;
use terra_world::flora::Growth;
use terra_world::flora_kinds::Flora;
use terra_world::grown::Cards;
use terra_world::planet::Planet;
use terra_world::sky_occlusion::{self, SkyOccluders};

mod floor;
mod footing;
mod shade;
mod species;
mod thicket;

use floor::{Crowns, Stand};
use footing::{Footing, Shapes, SolidShape, Trunk};
use species::{Form, Species};

type Foliage = ExtendedMaterial<StandardMaterial, FoliageExtension>;

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
/// Crowns' shadows hand over from their cards to solid lumps between these distances, for
/// every tree alike (D234).
const SHADOW_HANDOVER_M: [f32; 2] = [14.0, 26.0];
/// How much of a trunk one repeat of the bark's picture covers: round it, and along it.
const BARK_M: Vec2 = Vec2::new(2.15, 2.15);
/// The trees' barks (D247): the scan in `materials`, how many metres across its picture is,
/// its tint, how deep its furrows are relief-mapped, and how much moss it grows. Three for
/// oaks, two for pines, two for spruces, in that order. Each tint takes its scan to bark's
/// real albedo, about 0.08 to 0.1 linear on average (sRGB 80 to 90) whatever the scan's own
/// (D264): tinted as the scans came, under Khronos PBR Neutral (D254) trunks read near black.
#[allow(clippy::type_complexity)]
const BARKS: [(&str, f32, (f32, f32, f32), f32, f32); 7] = [
    ("oak_bark", 2.15, (0.68, 0.66, 0.63), 0.05, 1.0),
    ("oak_bark", 1.5, (0.62, 0.61, 0.6), 0.06, 1.0),
    ("bark_brown_02", 1.0, (0.86, 0.82, 0.8), 0.05, 1.0),
    ("pine_bark", 2.0, (0.84, 0.76, 0.7), 0.05, 0.35),
    ("pine_bark", 2.0, (0.9, 0.8, 0.72), 0.05, 0.3),
    ("bark_brown_02", 1.0, (0.78, 0.77, 0.76), 0.04, 0.6),
    ("pine_bark", 2.0, (0.74, 0.72, 0.7), 0.05, 0.6),
];
/// The solid shadow lumps are this much of the crown's size, so a low sun, which looks
/// through many crowns, still comes through in places.
const SHADOW_LUMP: f32 = 0.6;
/// The render layer only the sun sees, which the stand-ins are on.
const SUN_ONLY: usize = 1;
/// How many kinds of bush there are; each is drawn many times. The trees' are `KINDS`.
const BUSH_KINDS: u64 = 4;
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
/// The bushes' leaves in their picture, leaves.png, as an octagon in texture coordinates,
/// so a card is cut to it rather than square: two thirds of the square, so the GPU shades a
/// third fewer empty pixels. Fitted to its opaque pixels by hand-run script.
const SPRAY: Leaf = Leaf {
    cell: Vec2::ZERO,
    across: 1.0,
    outline: [
        (0.310, 0.062),
        (0.662, 0.062),
        (0.946, 0.346),
        (0.946, 0.705),
        (0.713, 0.938),
        (0.317, 0.938),
        (0.054, 0.674),
        (0.054, 0.317),
    ],
};

/// A card's picture: its cell in its atlas, how far across the cell is, and the octagon
/// round its leaves in the atlas's coordinates.
#[derive(Clone, Copy)]
struct Leaf {
    cell: Vec2,
    across: f32,
    outline: [(f32, f32); 8],
}

impl Leaf {
    /// A card of the shared atlas grown plants use (`assets/data/foliage.ron`, D222).
    fn of(cards: &Cards, name: &str) -> Self {
        let card = cards.get(name);
        let mut outline = [(0.0, 0.0); 8];
        for (to, from) in outline.iter_mut().zip(&card.outline) {
            *to = *from;
        }
        Self {
            cell: card.cell,
            across: card.across,
            outline,
        }
    }
}
/// How the crowns' leaves are shaded (D247): their pictures greyed by 15% and darkened by
/// 15%, toward a forest's leaves in the shade; how brightly a leaf glows lit through toward
/// the sun; and how dark the inside of a crown goes in the sun's light, at least.
const LEAF: Vec4 = Vec4::new(0.15, 0.15, 1.6, 0.3);
/// Paths are filed in squares this wide, to find the nearest quickly.
const PATH_SQUARE_M: f32 = 12.0;

/// What the foliage shader is told. Field order must match the struct in foliage.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FoliageExtension {
    #[uniform(100)]
    direction: Vec3,
    #[uniform(100)]
    strength: f32,
    /// 1 for bark: each tree's bark is shifted by where it stands, and weathered in patches
    /// with moss and a wet foot (D218); up to 2 for bark that grows less moss (D235).
    #[uniform(100)]
    bark: f32,
    /// For a crown whose cards drop out: from where, to where, and 1 (D223).
    #[uniform(100)]
    cards: Vec3,
    /// For a shadow stand-in: each card or lump shrinks to its middle between x and y, and
    /// grows from it between z and w; zero where it does not (D234).
    #[uniform(100)]
    shrink: Vec4,
    /// For leaves: how much their colour is greyed, and darkened, how brightly they glow lit
    /// through from behind, and how dark the inside of a crown goes, at least (D247).
    #[uniform(100)]
    leaf: Vec4,
}

impl MaterialExtension for FoliageExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/foliage.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/foliage.wesl".into()
    }

    // Crossfades between levels of detail dither in a pattern that moves every frame, for
    // TAA to blend (D214).
    fn fragment_shader() -> ShaderRef {
        "shaders/foliage_fragment.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/foliage_prepass.wesl".into()
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins((
        MaterialPlugin::<Foliage>::default(),
        floor::plugin,
        footing::plugin,
    ))
    .init_resource::<Unmipped>()
    .init_resource::<Scanned>()
    .add_systems(
        Update,
        (
            plant.run_if(not(resource_exists::<Forest>)),
            add_mipmaps,
            dress_scans,
            follow_detail,
        ),
    )
    // The bark's relief at the quality preset's depth.
    .add_systems(PostUpdate, terra_world::quality::follow_relief::<Foliage>);
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
    if let Some(mut material) = materials.get_mut(&forest.dropping) {
        material.extension.cards = (Vec2::from(CARDS_DROP_M) * detail.0).extend(1.0);
    }
    let [from, to] = SHADOW_HANDOVER_M;
    let end = SHADOW_M[1];
    if let Some(mut material) = materials.get_mut(&forest.stand_in) {
        material.extension.shrink = Vec4::new(from, to, 0.0, 0.0) * detail.0;
    }
    if let Some(mut material) = materials.get_mut(&forest.lumps) {
        material.extension.shrink = Vec4::new(end * (1.0 - SHADOW_END), end, from, to) * detail.0;
    }
}

/// Scans' glTF materials still to be copied into the materials their meshes are drawn with.
#[derive(Resource, Default)]
struct Scanned(Vec<(Handle<GltfMaterial>, Handle<StandardMaterial>, Color)>);

/// Copies each scan's glTF material, once loaded, into the material it is drawn with: its
/// scanned colour, normal, roughness and occlusion maps, its colour times a tint.
fn dress_scans(
    mut scanned: ResMut<Scanned>,
    gltf: Res<Assets<GltfMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    scanned.0.retain(|(from, to, tint)| {
        let (Some(from), Some(mut to)) = (gltf.get(from), materials.get_mut(to)) else {
            return true;
        };
        *to = StandardMaterial {
            base_color: {
                let (a, b) = (from.base_color.to_linear(), tint.to_linear());
                Color::linear_rgba(a.red * b.red, a.green * b.green, a.blue * b.blue, a.alpha)
            },
            base_color_texture: from.base_color_texture.clone(),
            normal_map_texture: from.normal_map_texture.clone(),
            metallic_roughness_texture: from.metallic_roughness_texture.clone(),
            occlusion_texture: from.occlusion_texture.clone(),
            perceptual_roughness: from.perceptual_roughness,
            metallic: from.metallic,
            reflectance: 0.3,
            alpha_mode: from.alpha_mode,
            double_sided: from.double_sided,
            cull_mode: from.cull_mode,
            ..default()
        };
        false
    });
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
    /// The shadow stand-ins, whose handover follows how far detail reaches too.
    stand_in: Handle<Foliage>,
    lumps: Handle<Foliage>,
}

/// The forest's grass, as terra_world's ground cover asks it, from the floor's map (D246).
struct LushCover(Arc<floor::Map>);

impl CoverSource for LushCover {
    fn growing(&self, _planet: &Planet, _ground: Vec3, at: Vec2) -> Growing {
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

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// One kind of tree at each level of detail, and the stand-in its shadow is cast from.
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
}

/// The kinds of tree the forest grows, each grown once and drawn many times: what it is,
/// its form if an oak, whether young, and the heights it is grown between (D182, D235).
/// Grown oaks are tall forest oaks, broad ones, and ones forked low; Scots pines stand in
/// groves among them, Norway spruces in the groves' damper hollows, young spruces under
/// them.
const KINDS: [(Species, Form, bool, (f32, f32)); 21] = [
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
    (Species::Pine, Form::Tall, false, (24.0, 28.0)),
    (Species::Pine, Form::Tall, false, (26.0, 31.0)),
    (Species::Pine, Form::Tall, false, (23.0, 27.0)),
    (Species::Pine, Form::Tall, false, (27.0, 32.0)),
    (Species::Spruce, Form::Tall, false, (19.0, 24.0)),
    (Species::Spruce, Form::Tall, false, (22.0, 27.0)),
    (Species::Spruce, Form::Tall, false, (17.0, 22.0)),
    (Species::Spruce, Form::Tall, true, (3.5, 5.0)),
    (Species::Spruce, Form::Tall, true, (4.5, 6.5)),
    (Species::Spruce, Form::Tall, true, (5.5, 8.0)),
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
            depth_map: Some(map("depth.jpg", false)),
            parallax_depth_scale: relief,
            parallax_mapping_method: ParallaxMappingMethod::Relief { max_steps: 4 },
            max_parallax_layer_count: 16.0,
            ..default()
        },
        extension: FoliageExtension {
            bark: 2.0 - moss,
            ..wind.clone()
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn plant(
    mut commands: Commands,
    planet: Option<Res<Planet>>,
    flora: Option<Res<Flora>>,
    sun: Option<Single<Entity, With<DirectionalLight>>>,
    assets: Res<AssetServer>,
    mut shapes: ResMut<Shapes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Foliage>>,
    mut plain: ResMut<Assets<StandardMaterial>>,
    mut unmipped: ResMut<Unmipped>,
    mut scanned: ResMut<Scanned>,
    mut mossy: ResMut<Assets<floor::Mossy>>,
    mut mossy_scans: ResMut<floor::MossyScans>,
) {
    let (Some(planet), Some(flora), Some(sun)) = (planet, flora, sun) else {
        return;
    };
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
    let seed = Seed(77).derive("lush");

    let wind = FoliageExtension {
        direction: east,
        strength: 1.0,
        bark: 0.0,
        cards: Vec3::ZERO,
        shrink: Vec4::ZERO,
        leaf: Vec4::ZERO,
    };
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
    let bark = bark_material(
        &assets,
        &mut materials,
        &wind,
        "oak_bark",
        Color::srgb(0.68, 0.66, 0.63),
        0.03,
        1.0,
    );
    // The crowns' leaves, oak and conifer alike, are cards of the atlas grown plants use, so
    // all of them are drawn in one material (D222).
    let cards = Cards::read();
    let atlas: Handle<Image> = assets.load(cards.atlas.clone());
    // A crown's leaves: a little of the light through them from behind is Bevy's, the same
    // from every angle; the glow toward the sun is the foliage shader's own (`LEAF`, D247),
    // so a leaf lit from the front keeps most of its colour. The forest floor's plants keep
    // Bevy's alone, as they had it.
    let crown_material = |cards_drop: Vec3, leaf: Vec4, transmission: f32| Foliage {
        base: StandardMaterial {
            base_color_texture: Some(atlas.clone()),
            alpha_mode: AlphaMode::Mask(cards.cut),
            // Seen from both sides, but with the same normal on both, which leans out of
            // the crown, so a card does not flip from lit to dark as it turns.
            cull_mode: None,
            double_sided: false,
            diffuse_transmission: transmission,
            perceptual_roughness: 0.8,
            reflectance: 0.25,
            ..default()
        },
        extension: FoliageExtension {
            cards: cards_drop,
            leaf,
            ..wind.clone()
        },
    };
    let crown_leaves = materials.add(crown_material(Vec3::ZERO, LEAF, 0.25));
    // The floor's and the middle layer's: greyed and darkened as the crowns' are, if less,
    // so their pictures' spring green reads as leaves in a wood's shade, not lime, and lit through
    // less, as backlit they glowed lime too (D268).
    let floor_cards = materials.add(crown_material(
        Vec3::ZERO,
        Vec4::new(0.12, 0.1, 0.0, 0.0),
        0.35,
    ));
    // The near crown's leaves, whose cards drop out with distance (D223).
    let dropping_leaves = materials.add(crown_material(
        Vec2::from(CARDS_DROP_M).extend(1.0),
        LEAF,
        0.25,
    ));
    let leaves = materials.add(Foliage {
        base: StandardMaterial {
            base_color_texture: Some(leaf_picture.clone()),
            alpha_mode: AlphaMode::Mask(LEAF_CUT),
            // Seen from both sides, but with the same normal on both, which leans out of
            // the crown, so a card does not flip from lit to dark as it turns.
            cull_mode: None,
            double_sided: false,
            diffuse_transmission: 0.5,
            perceptual_roughness: 0.75,
            reflectance: 0.3,
            ..default()
        },
        extension: wind.clone(),
    });
    // The shadow stand-ins: their cards shrink away over the handover, and the lumps grow in
    // then shrink away at the end of the shadows (D234).
    let shrinking = |out: [f32; 2], grow: [f32; 2]| FoliageExtension {
        shrink: Vec4::new(out[0], out[1], grow[0], grow[1]),
        ..wind.clone()
    };
    let leaves_base = materials
        .get(&crown_leaves)
        .expect("just added")
        .base
        .clone();
    let stand_in_leaves = materials.add(Foliage {
        base: leaves_base,
        extension: shrinking(SHADOW_HANDOVER_M, [0.0, 0.0]),
    });
    let shadow_lumps = materials.add(Foliage {
        base: StandardMaterial::default(),
        extension: shrinking(
            [SHADOW_M[1] * (1.0 - SHADOW_END), SHADOW_M[1]],
            SHADOW_HANDOVER_M,
        ),
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
    let far_crowns = [
        far_crown(Color::srgb(0.22, 0.32, 0.12)),
        far_crown(Color::srgb(0.15, 0.23, 0.12)),
        far_crown(Color::srgb(0.13, 0.21, 0.12)),
    ];
    // Each kind's bark in several scans or shades, so a stand of them is not one bark (D247):
    // old oaks' deep-furrowed willow, it again larger and greyer (Bark Willow, dark mud
    // without plates, read as fur, D264) and a grey-brown with knots; pines' pine
    // bark, darker and lighter; spruces' the grey-brown scan and a grey pine bark. Each is
    // `BARKS`' entry: what scan, how many metres its picture covers, tint, relief and moss.
    let barks: Vec<(Handle<Foliage>, Vec2)> = BARKS
        .iter()
        .map(|&(name, metres, (r, g, b), relief, moss)| {
            (
                bark_material(
                    &assets,
                    &mut materials,
                    &wind,
                    name,
                    Color::srgb(r, g, b),
                    relief,
                    moss,
                ),
                Vec2::splat(metres),
            )
        })
        .collect();
    let leaf_of = [
        Leaf::of(&cards, "oakbranch"),
        Leaf::of(&cards, "pine"),
        Leaf::of(&cards, "spruce"),
    ];

    let trees: Vec<TreeModel> = KINDS
        .iter()
        .enumerate()
        .map(|(kind, &(species, form, young, (low, high)))| {
            let mut dice = seed.derive("tree").index(kind as u64).rng();
            let height = dice.range(low, high);
            let tree = match species {
                Species::Oak => species::oak(&mut dice, height, form, young),
                _ => species::conifer(&mut dice, height, species, young),
            };
            let (which, leaf, tint, far_material) = match species {
                Species::Oak => (kind % 3, leaf_of[0], Vec3::new(0.92, 0.95, 0.85), &far_crowns[0]),
                Species::Pine => (3 + kind % 2, leaf_of[1], Vec3::new(1.15, 1.15, 1.0), &far_crowns[1]),
                // Dark green, about sRGB 0.15. The needle picture is the deep green of Norway
                // spruce, and the tint pushes it further from blue: needles this dark are lit
                // mostly by the blue sky, which turned them teal (D247, D248).
                Species::Spruce => (5 + kind % 2, leaf_of[2], Vec3::new(1.5, 1.65, 0.45), &far_crowns[2]),
            };
            let (bark, bark_m) = barks[which].clone();
            let wood = [0, 1, 2].map(|detail| wood(&tree, detail, height, bark_m));
            // Every level of the crown, and its shadow, from one set of cards (D221).
            let mut cards = species::crown_cards(&mut dice, &tree, height, leaf);
            for card in cards.iter_mut().flatten() {
                card.tint *= tint;
            }
            let near = crown_near(&cards);
            let far = crown_far(&cards);
            let shadow = crown_stand_in(&cards);
            let clumps = crown_clumps(&tree, 1.0, false);
            let lumps = crown_clumps(&tree, SHADOW_LUMP, true);
            info!(
                "lush: tree {kind}, {species:?} {form:?}{}, {height:.0} m: wood {} {} {} triangles, crown {} near and {} far, shadows {} {}",
                if young { " young" } else { "" },
                wood[0].triangles(),
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
                mesh.generate_tangents().expect("the tubes have normals and texture places");
                meshes.add(mesh)
            });
            let crown_near = meshes.add(near.mesh());
            shapes.remember(&wood[0], trunk);
            shapes.remember(&crown_near, seen);
            TreeModel {
                wood,
                crown_near,
                crown_far: meshes.add(far.mesh()),
                shadow: meshes.add(shadow.mesh()),
                clumps: meshes.add(clumps.mesh()),
                lumps: meshes.add(lumps.mesh()),
                height,
                trunk_m: tree.trunk_m,
                crown: floor::crown_of(&tree.tips),
                skirt_m: skirt(&tree),
                species,
                young,
                bark,
                far: far_material.clone(),
                modelled: false,
            }
        })
        .collect();
    // Veteran oaks: a trunk and limbs modelled (Tripo, from a concept drawn to the style
    // guide, art/props/oak_bough), far more gnarled and burred than a grown one can be, its
    // crown grown on it as cards at the clumps tools/blender/tips.py found along its outer
    // branches (D235). The model's own texture read as blue-black clay beside the grown
    // trunks, its baked shadows black: it is drawn in the grown oaks' bark instead, laid on
    // by where each point is (D264).
    let mut trees = trees;
    let first_veteran = trees.len();
    for (n, (model, tips, which)) in VETERANS.into_iter().enumerate() {
        let (name, metres, (r, g, b), relief, moss) = BARKS[which];
        let bark = bark_material(
            &assets,
            &mut materials,
            &wind,
            name,
            Color::srgb(r, g, b),
            relief,
            moss,
        );
        if let Some(mut material) = materials.get_mut(&bark) {
            // From metres to the picture's repeats, as the shader projects it.
            material.base.uv_transform = bevy::math::Affine2::from_scale(Vec2::splat(1.0 / metres));
        }
        let trunk = |file: String| -> Handle<Mesh> {
            assets.load(
                GltfAssetLabel::Primitive {
                    mesh: 0,
                    primitive: 0,
                }
                .from_asset(file),
            )
        };
        let near_trunk = trunk(format!("models/nature/forest/{model}.glb"));
        let far_trunk = trunk(format!("models/nature/forest/{model}_far.glb"));
        let tips: Vec<species::Tip> = terra_world::data::read(&format!("trees/{tips}.ron"));
        let tree = species::modelled(&tips, VETERAN_TRUNK_M);
        let mut dice = seed.derive("veteran").index(n as u64).rng();
        let mut cards = species::crown_cards(&mut dice, &tree, VETERAN_M, leaf_of[0]);
        for card in cards.iter_mut().flatten() {
            card.tint *= Vec3::new(0.9, 0.95, 0.85);
            // Its trunk does not sway, so nor do its leaves much with it.
            card.sway.x *= 0.25;
        }
        let near = crown_near(&cards);
        info!(
            "lush: veteran oak {model}, {VETERAN_M:.0} m: {} clumps, crown {} near",
            tree.tips.len(),
            near.triangles()
        );
        trees.push(TreeModel {
            wood: [near_trunk, far_trunk.clone(), far_trunk],
            crown_near: {
                let seen = SolidShape::crown(&near.positions, &near.indices);
                let mesh = meshes.add(near.mesh());
                shapes.remember(&mesh, seen);
                mesh
            },
            crown_far: meshes.add(crown_far(&cards).mesh()),
            shadow: meshes.add(crown_stand_in(&cards).mesh()),
            clumps: meshes.add(crown_clumps(&tree, 1.0, false).mesh()),
            lumps: meshes.add(crown_clumps(&tree, SHADOW_LUMP, true).mesh()),
            height: VETERAN_M,
            trunk_m: tree.trunk_m,
            crown: floor::crown_of(&tree.tips),
            skirt_m: 0.0,
            species: Species::Oak,
            young: false,
            bark,
            far: far_crowns[0].clone(),
            modelled: true,
        });
    }
    let veterans_kinds = first_veteran..trees.len();

    let path = Path::new(seed.derive("path"));
    // For scripted runs: --face this to look, or walk, down a path from the start.
    let way = path.lines[1][path.lines[1].len() - 1].0;
    info!(
        "lush: a path leads out to {:.0},{:.0} (east, south).",
        way.x, way.y
    );

    let cut_as = |name: &str, size: f32| {
        flora
            .kinds()
            .iter()
            .find(|kind| kind.name == name)
            .and_then(|kind| kind.cuttable(size))
    };
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
    // Levels `first` to `last` of a tree this many times the height the distances are for.
    // From one distance to another, crossfading at each end as the levels do.
    let between = |from: f32, to: f32| VisibilityRange {
        start_margin: from * (1.0 - FADE)..from,
        end_margin: to * (1.0 - FADE)..to,
        use_aabb: false,
    };
    // A crown's shadow handing over from cards to lumps (D234): the cards shrink away and the
    // lumps grow in over SHADOW_HANDOVER_M, the same for every tree as Horizon Zero Dawn keeps
    // one shadow detail a cascade, then the lumps shrink away toward SHADOW_M. Bevy's shadow
    // passes do not dither, so a caster that simply ended would vanish at once.
    let handover = |cards: bool, _size: f32| {
        let [from, to] = SHADOW_HANDOVER_M;
        if cards {
            VisibilityRange::abrupt(0.0, to)
        } else {
            VisibilityRange::abrupt(from, SHADOW_M[1])
        }
    };
    // Where a crown of two cards a clump gives way to lumps.
    let far_from = |size: f32| (DETAIL_M[2] * size).max(CARDS_DROP_M[1] * 1.4);
    let detail = |first: usize, last: usize, size: f32| {
        let at = |level: usize| DETAIL_M[level] * size;
        let from = if first == 0 { 0.0 } else { at(first - 1) };
        VisibilityRange {
            start_margin: from * (1.0 - FADE)..from,
            end_margin: at(last) * (1.0 - FADE)..at(last),
            use_aabb: false,
        }
    };

    // Grown trees, tried at random over the patch and kept if far enough from the others
    // and off the paths; then young ones in the gaps between them. Pines grow in groves,
    // where a broad field of noise is high, spruces in the groves' hollows, where another
    // is; oaks everywhere else, and the edges of groves mixed.
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
    let mut feet: HashMap<(i32, i32), Vec<Vec2>> = HashMap::default();
    // The trees as the floor sees them, for what grows under them (D246).
    let mut canopy = Crowns::default();
    // And as the sky's light sees them, for how much of it reaches under them (D270).
    let mut crowns = Vec::new();
    let mut planted = [0, 0];
    // The nearest of each to the start, for scripted runs to walk up to.
    let mut nearest_to_start = [Vec2::splat(f32::MAX); 2];
    for (young, tries, apart) in [(false, 14_000, SPACING_M), (true, 8_000, 4.5)] {
        for _ in 0..tries {
            let angle = dice.range(0.0, TAU);
            let distance = libm::sqrtf(dice.range(CLEARING_M * CLEARING_M, FOREST_M * FOREST_M));
            let at = Vec2::new(libm::cosf(angle), libm::sinf(angle)) * distance;
            // Glades open here and there, where only young trees grow.
            let glade = glades.fbm(Vec3::new(at.x, 0.0, at.y) / 45.0, 2, 2.0, 0.5);
            let grove = groves.fbm(Vec3::new(at.x, 0.0, at.y) / 70.0, 2, 2.0, 0.5);
            let species = if grove < 0.1 + dice.range(-0.12, 0.12) {
                Species::Oak
            } else if groves.sample(Vec3::new(at.x, 40.0, at.y) / 35.0) > 0.15 {
                Species::Spruce
            } else {
                Species::Pine
            };
            // Conifers stand closer than oaks, whose crowns spread. Trees stand in clumps
            // with gaps between, not evenly spaced as a plantation's (D247).
            let clumped = 0.5 + 0.5 * clumps.fbm(Vec3::new(at.x, 0.0, at.y) / 22.0, 2, 2.0, 0.5);
            let apart = apart
                * match species {
                    Species::Oak => 1.0,
                    _ => 0.75,
                }
                * (0.65 + 0.8 * clumped.clamp(0.0, 1.0));
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
            by_species[model.species as usize] += 1;
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
            let place = place_on(at, &mut dice, 0.3, scale);
            let foot = place.translation;
            let crown = place.rotation * model.crown.0 * scale;
            canopy.add(Stand::of(
                at,
                at + Vec2::new(crown.dot(east), crown.dot(south)),
                model.crown.1 * scale,
                model.species,
                model.young,
                model.trunk_m * scale,
            ));
            crowns.push(crown_seen(
                foot + crown,
                place.rotation * Vec3::Y,
                model.height * scale,
                model.crown.1 * scale,
                model.species,
                model.young,
            ));
            commands
                .spawn((
                    // Named, so a check can find it and walk into it (tools/sandbox/collide.ts).
                    Name::new(format!("{:?}", model.species)),
                    place,
                    Visibility::default(),
                    // It stops a walker at its trunk and roots as drawn, or at its skirt where
                    // its branches reach the ground, so the camera is not walked in among a
                    // spruce's boughs (D248, D267). Filed at its foot, where it is felled from.
                    Trunk {
                        wood: model.wood[0].clone(),
                        crown: model.crown_near.clone(),
                        reach_m: footing::trunk_reach(model.trunk_m),
                        skirt_m: model.skirt_m,
                    },
                    Growth::new(
                        cut_as(
                            if model.species == Species::Oak {
                                "Leafy tree"
                            } else {
                                "Pine"
                            },
                            model.height * scale,
                        ),
                        Some(foot),
                        1.4,
                    ),
                ))
                .with_children(|tree| {
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
                        let mut wood = tree.spawn((
                            Mesh3d(model.wood[level].clone()),
                            MeshMaterial3d(model.bark.clone()),
                            detail(level, last, size),
                        ));
                        if level == 2 {
                            wood.insert(NotShadowCaster);
                        }
                    }
                    // The crown thins card by card out to CARDS_DROP_M[1], then is two cards
                    // a clump, the same cards at the same size, then lumps (D223).
                    tree.spawn((
                        Mesh3d(model.crown_near.clone()),
                        MeshMaterial3d(dropping_leaves.clone()),
                        between(0.0, CARDS_DROP_M[1]),
                        NotShadowCaster,
                    ));
                    tree.spawn((
                        Mesh3d(model.crown_far.clone()),
                        MeshMaterial3d(crown_leaves.clone()),
                        between(CARDS_DROP_M[1], far_from(size)),
                        NotShadowCaster,
                    ));
                    // Far off, the crown as its solid lumps in the leaves' colour, so the
                    // forest carries on past where its cards end.
                    tree.spawn((
                        Mesh3d(model.clumps.clone()),
                        MeshMaterial3d(model.far.clone()),
                        between(far_from(size), DETAIL_M[3] * size),
                        NotShadowCaster,
                    ));
                    tree.spawn((
                        Mesh3d(model.shadow.clone()),
                        MeshMaterial3d(stand_in_leaves.clone()),
                        handover(true, size),
                        RenderLayers::layer(SUN_ONLY),
                    ));
                    tree.spawn((
                        Mesh3d(model.lumps.clone()),
                        MeshMaterial3d(shadow_lumps.clone()),
                        handover(false, size),
                        RenderLayers::layer(SUN_ONLY),
                    ));
                });
            planted[young as usize] += 1;
            let of = &mut nearest_of[model.species as usize];
            if !young && at.length() < of.length() {
                *of = at;
            }
            if at.length() < nearest_to_start[young as usize].length() {
                nearest_to_start[young as usize] = at;
            }
        }
    }
    if let Some(near) = veterans
        .iter()
        .min_by(|a, b| a.length().total_cmp(&b.length()))
    {
        info!(
            "lush: {} veteran oaks, the nearest at {:.1},{:.1} (east, south).",
            veterans.len(),
            near.x,
            near.y
        );
    }

    info!(
        "lush: {} oaks, {} pines and {} spruces; the nearest grown of each at {:.1},{:.1}, {:.1},{:.1} and {:.1},{:.1} (east, south).",
        by_species[0],
        by_species[1],
        by_species[2],
        nearest_of[0].x,
        nearest_of[0].y,
        nearest_of[1].x,
        nearest_of[1].y,
        nearest_of[2].x,
        nearest_of[2].y
    );
    info!(
        "lush: {} grown trees and {} young ones planted. The nearest stand at {:.1},{:.1} and \
         {:.1},{:.1} (east, south).",
        planted[0],
        planted[1],
        nearest_to_start[0].x,
        nearest_to_start[0].y,
        nearest_to_start[1].x,
        nearest_to_start[1].y
    );

    // The floor under the trees, and the map the grass grows by (D246).
    commands.insert_resource(SkyOccluders {
        middle: start,
        up,
        east,
        // Past the forest's edge, so where it ends, it sees the open sky as the environment
        // map does, in the meadow beyond where the edge is looked at from.
        half_m: FOREST_M + 70.0,
        below_m: 15.0,
        above_m: 45.0,
        crowns,
    });
    let map = floor::lay(
        &mut floor::Lay {
            commands: &mut commands,
            planet: &planet,
            start,
            east,
            south,
            assets: &assets,
            meshes: &mut meshes,
            plain: &mut plain,
            scanned: &mut scanned,
            mossy: &mut mossy,
            mossy_scans: &mut mossy_scans,
            flora: &flora,
            cards: &cards,
            bark: bark.clone(),
            pine_bark: barks[3].0.clone(),
            atlas: floor_cards,
            leaves: leaves.clone(),
            seed,
            foliage: &mut materials,
            shrinking: HashMap::default(),
            shade: shade::NearShadows::new(start, east, south),
        },
        path,
        canopy,
    );
    let map = Arc::new(map);
    commands.insert_resource(CoverRules {
        source: Arc::new(LushCover(map)),
        seed: seed.derive("grass"),
    });
    commands.insert_resource(Forest {
        dropping: dropping_leaves.clone(),
        stand_in: stand_in_leaves.clone(),
        lumps: shadow_lumps.clone(),
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
}

/// A mesh being built.
#[derive(Default)]
struct Build {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    sway: Vec<[f32; 2]>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
    /// For a crown whose cards drop out with distance: each vertex's offset from its
    /// card's middle, and in w the card's rank in its clump plus a hundredth of how many
    /// cards the clump has (D223). Carried as the tangent, which leaves have no other use for.
    dropping: Vec<[f32; 4]>,
    /// For a shadow stand-in, which only the sun sees: each vertex's offset from its card's
    /// or lump's middle in its colour, since Bevy's shadow passes are given no tangents
    /// (D234).
    offset_in_colour: bool,
}

impl Build {
    fn vertex(&mut self, at: Vec3, normal: Vec3, uv: Vec2, sway: Vec2, colour: Vec3) -> u32 {
        self.positions.push(at.into());
        self.normals.push(normal.into());
        self.uvs.push(uv.into());
        self.sway.push(sway.into());
        self.colours.push([colour.x, colour.y, colour.z, 1.0]);
        self.positions.len() as u32 - 1
    }

    fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    fn mesh(self) -> Mesh {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.sway)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colours)
        .with_inserted_indices(Indices::U32(self.indices));
        if self.dropping.is_empty() {
            mesh
        } else {
            mesh.with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, self.dropping)
        }
    }
}

/// A limb of a tree: a line of points, with how thick it is at each.
struct Limb {
    points: Vec<Vec3>,
    radii: Vec<f32>,
    depth: u32,
    /// Whether it runs out to a point, as a branch does; a cut log does not.
    pointed: bool,
}

/// How far a tree's limbs and leaves reach from its trunk within 1.8 m of the ground.
fn skirt(tree: &Tree) -> f32 {
    let low = |p: &Vec3| p.y < 1.8;
    let limbs = tree
        .limbs
        .iter()
        .filter(|limb| limb.depth > 0)
        .flat_map(|limb| {
            limb.points
                .iter()
                .filter(|p| low(p))
                .map(|p| Vec2::new(p.x, p.z).length())
        });
    let tips = tree
        .tips
        .iter()
        .filter(|(p, _)| low(p))
        .map(|(p, size)| Vec2::new(p.x, p.z).length() + size * 0.5);
    limbs.chain(tips).fold(0.0, f32::max)
}

/// A tree's shape, before it is made into meshes.
struct Tree {
    limbs: Vec<Limb>,
    /// Where each clump of leaves is, and how big.
    tips: Vec<(Vec3, f32)>,
    /// Which branch each tip grows on, counted from the great limbs out.
    branch_of: Vec<usize>,
    branches: usize,
    trunk_m: f32,
    /// Which way each clump's twig runs, outward: its cards' stalks are toward it.
    along: Vec<Vec3>,
    /// Which mass of the crown each clump is in, and each mass's middle, which its cards'
    /// normals lean out of, so the crown shades as masses with gaps, not as one ball.
    mass_of: Vec<usize>,
    masses: Vec<Vec3>,
    species: Species,
}

/// A tree's wood at a level of detail: all of it, then without the twigs, then only the
/// trunk and great limbs; rounder near the camera. A conifer's many thin branches are few
/// sided, and gone at the last level, where its needles hide them. `bark_m` is how much of
/// a limb one repeat of its bark's picture covers, round it and along it.
fn wood(tree: &Tree, detail: usize, height: f32, bark_m: Vec2) -> Build {
    // Near the camera, round enough that no limb shows facets and a trunk's roots each have
    // sides to swell on (D264).
    let (deepest, sides) = match tree.species {
        Species::Oak => (
            [3, 2, 1][detail],
            [[36, 12, 7, 4], [14, 6, 4, 3], [7, 4, 3, 3]][detail],
        ),
        Species::Pine | Species::Spruce => (
            [2, 1, 0][detail],
            [[28, 6, 4, 3], [12, 4, 3, 3], [7, 3, 3, 3]][detail],
        ),
    };
    let mut build = Build::default();
    for limb in tree.limbs.iter().filter(|limb| limb.depth <= deepest) {
        tube(
            &mut build,
            limb,
            sides[limb.depth.min(3) as usize],
            height,
            bark_m,
        );
    }
    // A Scots pine's bark is thick, grey-brown and fissured low on its bole and thin, flaking
    // and orange up in its crown, which tells a pine at a glance; one shade all the way up
    // read as one orange tube (D264). Carried in its vertices' colour, which tints the scan.
    if tree.species == Species::Pine {
        for (at, colour) in build.positions.iter().zip(&mut build.colours) {
            let up = smoothstep(0.3, 0.65, at[1] / height);
            let shade = Vec3::new(0.82, 0.84, 0.9).lerp(Vec3::new(1.12, 1.0, 0.86), up);
            colour[0] = shade.x;
            colour[1] = shade.y;
            colour[2] = shade.z;
        }
    }
    build
}

/// A limb's rings: as grown, and for a trunk also between, every 0.35 m near the ground
/// and 1 m above, so its roots and lumps have rings to show on.
fn rings(limb: &Limb) -> (Vec<Vec3>, Vec<f32>) {
    if limb.depth > 0 {
        // A branch runs out to a point, as a real one does, rather than ending in an open,
        // flat-cut tube (D247).
        let (mut points, mut radii) = (limb.points.clone(), limb.radii.clone());
        // It narrows over a few of its own widths, as a twig does, not to a pin at once.
        if let (true, &[.., before, last], Some(&r)) =
            (limb.pointed, points.as_slice(), radii.last())
        {
            let ahead = (last - before).normalize_or(Vec3::Y);
            points.push(last + ahead * r * 2.0);
            radii.push(r * 0.55);
            points.push(last + ahead * r * 5.0);
            radii.push(r * 0.05);
        }
        return (points, radii);
    }
    let (mut points, mut radii) = (vec![limb.points[0]], vec![limb.radii[0]]);
    for k in 1..limb.points.len() {
        let (a, b) = (limb.points[k - 1], limb.points[k]);
        // Close at the foot, where the roots swell out of it.
        let spacing = if a.y < 1.2 {
            0.12
        } else if a.y < 3.0 {
            0.35
        } else {
            1.0
        };
        let count = (a.distance(b) / spacing).ceil().max(1.0) as usize;
        for n in 1..=count {
            let t = n as f32 / count as f32;
            points.push(a.lerp(b, t));
            radii.push(limb.radii[k - 1] + (limb.radii[k] - limb.radii[k - 1]) * t);
        }
    }
    (points, radii)
}

/// How a limb's radius varies round it and along it, as a share of its radius: shallow
/// flutes running up the bark and broad lumps, and on a trunk six to eight roots swelling
/// out of its foot, each of its own size, width and height and curling a little round the
/// trunk as it climbs, so no foot is a star of equal points (D264); each tree its own by
/// where it grew.
struct Shape {
    phase: f32,
    trunk: bool,
    /// Each root: where round the trunk, how far it stands out (a share of the radius), how
    /// wide (radians), and how high up the trunk it reaches (metres).
    roots: Vec<(f32, f32, f32, f32)>,
}

impl Shape {
    fn of(limb: &Limb) -> Self {
        let p = limb.points[0];
        let phase =
            (libm::sinf(p.x * 12.9898 + p.z * 78.233 + p.y * 37.719) * 43_758.547).fract() * TAU;
        let trunk = limb.depth == 0;
        let roots = if trunk {
            let mut dice = Seed((phase * 1.0e6) as u64).derive("roots").rng();
            let count = 6 + dice.below(3);
            let r = limb.radii[0];
            (0..count)
                .map(|k| {
                    (
                        (k as f32 + dice.range(-0.3, 0.3)) / count as f32 * TAU + phase,
                        dice.range(0.3, 0.85),
                        dice.range(0.16, 0.32),
                        r * dice.range(0.5, 1.3) + 0.25,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        Self {
            phase,
            trunk,
            roots,
        }
    }

    fn at(&self, angle: f32, along: f32, up: f32) -> f32 {
        let deep = if self.trunk { 1.0 } else { 0.5 };
        let flutes = 0.035 * libm::cosf(7.0 * angle + self.phase + 0.25 * along)
            + 0.02 * libm::cosf(13.0 * angle + 2.1 * self.phase - 0.4 * along);
        let lumps = 0.05
            * libm::sinf(3.0 * angle + 1.7 * along + self.phase)
            * libm::sinf(0.9 * along + 2.0 * angle);
        let mut shape = 1.0 + deep * (flutes + lumps);
        if self.trunk {
            // Each root a ridge, highest at the ground and dying into the bole over its own
            // height; the trunk drawn in a little between them.
            let mut swell = 0.0f32;
            for &(at, out, wide, high) in &self.roots {
                let off = (angle - at - 0.12 * up + PI).rem_euclid(TAU) - PI;
                let fall = libm::expf(-(up - 0.15).max(0.0) / high * 2.2);
                let ridge = libm::expf(-(off / wide) * (off / wide));
                swell = swell.max(out * ridge * fall);
            }
            let between = 0.1 * (1.0 - smoothstep(0.0, 1.2, up));
            shape *= 1.0 + swell - between;
        }
        shape
    }
}

/// A limb as a tube with bark round it.
fn tube(build: &mut Build, limb: &Limb, sides: u32, height: f32, bark_m: Vec2) {
    let start = build.positions.len() as u32;
    let (points, radii) = rings(limb);
    let shape = Shape::of(limb);
    let mut along = 0.0;
    // One repeat of the bark's picture is about `bark_m` round the limb a third of the way
    // up, as many times all the way up: a count that changed from ring to ring cut the bark
    // into bands. Along the limb it repeats as often as round it at each ring, so the bark
    // keeps its own proportions as the limb thins, rather than stretching up it (D247).
    let wraps = (TAU * radii[radii.len() / 3] / bark_m.x).round().max(1.0);
    let repeat_along =
        |r: f32| (TAU * r.max(0.004) / wraps * bark_m.y / bark_m.x).min(bark_m.y * 2.0);
    let mut v = 0.0;
    let mut side = (points[1] - points[0]).any_orthonormal_vector();
    // How far along the limb each ring is, and its radius at each side, so a normal can
    // follow how the radius changes along it too: a root's back faces up out of the ground,
    // not level, and a twig's tip leans forward as it narrows.
    let mut alongs = vec![0.0f32; points.len()];
    for i in 1..points.len() {
        alongs[i] = alongs[i - 1] + points[i].distance(points[i - 1]);
    }
    let radius_at = |i: usize, angle: f32| radii[i] * shape.at(angle, alongs[i], points[i].y);
    for (i, (&point, &radius)) in points.iter().zip(&radii).enumerate() {
        if i > 0 {
            let step = point.distance(points[i - 1]);
            along += step;
            // Along its surface, which slopes out over a flared foot, not its middle line, or
            // the bark is drawn out in streaks down the roots.
            let surface = libm::sqrtf(step * step + (radius - radii[i - 1]).powi(2));
            v += surface / repeat_along(0.5 * (radius + radii[i - 1]));
        }
        let (before, after) = (i.saturating_sub(1), (i + 1).min(points.len() - 1));
        let forward = if i + 1 < points.len() {
            points[i + 1] - point
        } else {
            point - points[i - 1]
        }
        .normalize();
        // Carry the ring's orientation along the limb, so it does not twist.
        side = (side - forward * side.dot(forward)).normalize_or(forward.any_orthonormal_vector());
        let other = forward.cross(side);
        let bend = (point.y / height).clamp(0.0, 1.0);
        let flutter = if limb.depth >= 2 { 0.15 } else { 0.0 };
        for j in 0..=sides {
            let angle = j as f32 / sides as f32 * TAU;
            let out = side * libm::cosf(angle) + other * libm::sinf(angle);
            let round = side * -libm::sinf(angle) + other * libm::cosf(angle);
            // The trunk's own shape: flutes, lumps and roots, and the normal that follows it
            // round, from how fast the radius changes with the angle.
            let r = radius * shape.at(angle, along, point.y);
            let step = 0.01;
            let slope = radius
                * (shape.at(angle + step, along, point.y) - shape.at(angle - step, along, point.y))
                / (2.0 * step);
            let rise = (radius_at(after, angle) - radius_at(before, angle))
                / (alongs[after] - alongs[before]).max(1.0e-4);
            let normal = (out - round * slope / r.max(1.0e-4) - forward * rise).normalize_or(out);
            let uv = Vec2::new(j as f32 / sides as f32 * wraps, v);
            let index = build.vertex(
                point + out * r,
                normal,
                uv,
                Vec2::new(bend * bend, flutter),
                Vec3::ONE,
            );
            // How much of the sky it sees, under its own crown and its neighbours': least at
            // the foot, which stays damp, dark and mossy for a metre or so, more up the bole,
            // most up in the crown (D216). The foot's darkening reached eye height, and the
            // trunks there read black (D264).
            build.colours[index as usize][3] =
                0.3 + 0.2 * smoothstep(0.0, 1.4, point.y) + 0.5 * smoothstep(0.0, 0.8, bend);
        }
    }
    let ring = sides + 1;
    for i in 0..(points.len() as u32 - 1) {
        for j in 0..sides {
            let a = start + i * ring + j;
            let b = a + ring;
            // Wound so each triangle faces out of the tube, as its normals do: the other way the
            // outside was culled and the inside of the far wall drawn, lit from behind.
            build.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
}

/// How the cards of one crown or bush are lit and moved.
struct Canopy {
    /// The middle of the crown, which the cards' normals lean away from.
    centre: Vec3,
    /// How far its outermost clump is from the middle.
    reach: f32,
    height: f32,
    /// How far its top bends in the wind, against a tree's.
    bend: f32,
    /// Its leaves' colour, as a multiple of the picture's.
    tint: Vec3,
}

/// A crown near the camera: every card, each knowing its rank, which the foliage shader
/// shrinks to its middle one by one with distance while the rest grow to cover as much, as
/// SpeedTree's leaves drop out: the crown thins without ever being swapped (D223).
fn crown_near(cards: &[Vec<Card>]) -> Build {
    let mut build = Build::default();
    for tip in cards {
        for (k, card) in tip.iter().enumerate() {
            card.draw_ranked(&mut build, 1.0, Some((k, tip.len())));
        }
    }
    build
}

/// A crown far off: two cards a clump, the first two of its near crown's, grown as they
/// are there when the rest have dropped out (D223).
fn crown_far(cards: &[Vec<Card>]) -> Build {
    let mut build = Build::default();
    for tip in cards {
        let keep = 2.min(tip.len());
        let grow = libm::powf(tip.len() as f32 / keep as f32, 0.55);
        for card in &tip[..keep] {
            card.draw(&mut build, grow);
        }
    }
    build
}

/// What a crown's shadow is cast from near the camera: the first two cards of every tip,
/// the same as the crown's own, enlarged as the simpler levels are, so the light still
/// comes through in flecks where it does through the crown.
fn crown_stand_in(cards: &[Vec<Card>]) -> Build {
    let mut build = Build {
        offset_in_colour: true,
        ..default()
    };
    for tip in cards {
        let keep = 2.min(tip.len());
        let grow = libm::sqrtf(tip.len() as f32 / keep as f32).min(2.6) * 0.8;
        for card in &tip[..keep] {
            card.draw(&mut build, grow);
        }
    }
    build
}

/// What a crown's shadow is cast from farther off: a rounded lump for each branch, as
/// big as most of its leaves, solid so the GPU can skip what is hidden. The light no
/// longer comes through in flecks, which nobody sees at that distance.
fn crown_clumps(tree: &Tree, scale: f32, shadow: bool) -> Build {
    let mut build = Build {
        offset_in_colour: shadow,
        ..default()
    };
    for branch in 0..tree.branches {
        let tips: Vec<(Vec3, f32)> = tree
            .tips
            .iter()
            .zip(&tree.branch_of)
            .filter(|(_, of)| **of == branch)
            .map(|(tip, _)| *tip)
            .collect();
        if tips.is_empty() {
            continue;
        }
        let centre = tips.iter().fold(Vec3::ZERO, |sum, (tip, _)| sum + *tip) / tips.len() as f32;
        let size = tips.iter().map(|(_, size)| *size).sum::<f32>() / tips.len() as f32;
        let spread = tips
            .iter()
            .map(|(tip, _)| (*tip - centre).abs())
            .fold(Vec3::ZERO, Vec3::max);
        let radii = (spread * 0.8 + Vec3::splat(size * 0.6)) * Vec3::new(1.0, 0.75, 1.0) * scale;
        lump(&mut build, centre, radii);
    }
    build
}

/// A rough ball: rings of six round, four rings high.
fn lump(build: &mut Build, centre: Vec3, radii: Vec3) {
    let base = build.positions.len() as u32;
    let (around, rings) = (6, 4);
    for ring in 0..=rings {
        let tilt = ring as f32 / rings as f32 * std::f32::consts::PI;
        for step in 0..around {
            let turn = step as f32 / around as f32 * TAU;
            let normal = Vec3::new(
                libm::sinf(tilt) * libm::cosf(turn),
                libm::cosf(tilt),
                libm::sinf(tilt) * libm::sinf(turn),
            );
            let index = build.vertex(
                centre + normal * radii,
                normal,
                Vec2::ZERO,
                Vec2::ZERO,
                Vec3::ONE,
            );
            // Its offset from the lump's middle, for the shader to grow it from (D234).
            if build.offset_in_colour {
                let offset = normal * radii;
                build.colours[index as usize] = [offset.x, offset.y, offset.z, 1.0];
            }
        }
    }
    for ring in 0..rings {
        for step in 0..around {
            let a = base + ring * around + step;
            let b = base + ring * around + (step + 1) % around;
            build
                .indices
                .extend([a, a + around, b, b, a + around, b + around]);
        }
    }
}

/// One leaf card in a crown: where it is, which way it faces, how big, its picture, and
/// how it is lit and moves.
#[derive(Clone, Copy)]
struct Card {
    middle: Vec3,
    right: Vec3,
    up: Vec3,
    half: f32,
    normal: Vec3,
    tint: Vec3,
    sway: Vec2,
    flip: f32,
    /// How much of the sky it sees, least deep in the crown (D216).
    sky: f32,
    leaf: Leaf,
}

/// A bush's card near `at` in a clump this size.
fn card(dice: &mut Rng, canopy: &Canopy, at: Vec3, size: f32) -> Card {
    let middle = at + dice.unit_vector() * size * dice.range(0.2, 0.75);
    let out = (middle - canopy.centre).normalize_or(Vec3::Y);
    let facing = (dice.unit_vector() + out * 0.7 + Vec3::Y * 0.3).normalize();
    let spin = Quat::from_axis_angle(facing, dice.range(0.0, TAU));
    let right = spin * facing.any_orthonormal_vector();
    let up = facing.cross(right);
    let half = size * dice.range(0.45, 0.65);
    // The crown shades as one: normals lean out of it.
    let normal = (out * 0.75 + facing * 0.25).normalize();
    // Leaves deep in the crown, and low in it, are shaded by the rest.
    let depth = (middle.distance(canopy.centre) / canopy.reach).clamp(0.0, 1.0);
    let rise = ((middle.y - canopy.centre.y) / canopy.reach * 0.5 + 0.5).clamp(0.0, 1.0);
    let light = 0.5 + 0.35 * depth + 0.25 * rise + dice.range(-0.06, 0.06);
    let tint = Vec3::new(
        light * dice.range(0.95, 1.05),
        light,
        light * dice.range(0.9, 1.0),
    ) * canopy.tint;
    let bend = (middle.y / canopy.height).clamp(0.0, 1.0);
    let sway = Vec2::new(bend * bend * canopy.bend, dice.range(0.6, 1.0));
    let flip = if dice.unit() < 0.5 { -1.0 } else { 1.0 };
    Card {
        middle,
        right,
        up,
        half,
        normal,
        tint,
        sway,
        flip,
        sky: 1.0,
        leaf: SPRAY,
    }
}

impl Card {
    /// Draws it, `grow` times its size; `rank`, its place and its clump's count, for a crown
    /// whose cards drop out with distance.
    fn draw(&self, build: &mut Build, grow: f32) {
        self.draw_ranked(build, grow, None);
    }

    fn draw_ranked(&self, build: &mut Build, grow: f32, rank: Option<(usize, usize)>) {
        let half = self.half * grow;
        let base = build.positions.len() as u32;
        for (u, v) in self.leaf.outline {
            let atlas = Vec2::new(u, v);
            let own = (atlas - self.leaf.cell) / self.leaf.across;
            let corner = self.middle
                + self.right * (own.x * 2.0 - 1.0) * half * self.flip
                + self.up * (1.0 - own.y * 2.0) * half;
            let index = build.vertex(corner, self.normal, atlas, self.sway, self.tint);
            build.colours[index as usize][3] = self.sky;
            if build.offset_in_colour {
                let offset = corner - self.middle;
                build.colours[index as usize] = [offset.x, offset.y, offset.z, 1.0];
            }
            if let Some((k, n)) = rank {
                let offset = corner - self.middle;
                build
                    .dropping
                    .push([offset.x, offset.y, offset.z, k as f32 + n as f32 / 100.0]);
            }
        }
        for i in 1..self.leaf.outline.len() as u32 - 1 {
            build.indices.extend([base, base + i, base + i + 1]);
        }
    }
}

/// A clump of cards round `at`, for a bush.
fn clump(build: &mut Build, dice: &mut Rng, canopy: &Canopy, at: Vec3, size: f32, cards: u32) {
    for _ in 0..cards {
        card(dice, canopy, at, size).draw(build, 1.0);
    }
}

/// A bush: clumps of leaf cards in a rough dome, lighter and yellower than an oak.
fn grow_bush(dice: &mut Rng) -> Build {
    let height = dice.range(0.9, 1.6);
    let width = height * dice.range(0.8, 1.3);
    let canopy = Canopy {
        centre: Vec3::Y * height * 0.45,
        reach: width * 0.6,
        height,
        bend: 0.25,
        tint: Vec3::new(1.05, 1.1, 0.8),
    };
    let mut build = Build::default();
    for _ in 0..7 + dice.below(5) {
        let around = dice.range(0.0, TAU);
        let out = dice.range(0.0, 0.45) * width;
        let at = Vec3::new(
            libm::cosf(around) * out,
            dice.range(0.35, 0.8) * height,
            libm::sinf(around) * out,
        );
        let (cards, size) = (6 + dice.below(4), dice.range(0.35, 0.5) * height);
        clump(&mut build, dice, &canopy, at, size, cards);
    }
    build
}

/// A tree's crown as the sky's light sees it (D270): an ellipsoid round its leaves, from
/// where it stands, how tall it is and how far its leaves reach; an oak's crown fills its
/// upper two thirds, a Scots pine's its top fifth, a spruce's nearly all of it. How much of
/// the sky it hides straight up through it is the floor's own measure (`floor::Stand`).
fn crown_seen(
    middle_at_foot: Vec3,
    up: Vec3,
    height: f32,
    reach: f32,
    species: Species,
    young: bool,
) -> sky_occlusion::Crown {
    let (mid, half, wide, closes) = match species {
        Species::Oak => (0.68, 0.32, 1.0, 0.85),
        Species::Pine => (0.82, 0.16, 1.0, 0.62),
        Species::Spruce => (0.55, 0.45, 0.75, 0.92),
    };
    sky_occlusion::Crown {
        middle: middle_at_foot + up * height * mid,
        half: Vec3::new(reach * wide, height * half, reach * wide),
        closes: closes * if young { 0.7 } else { 1.0 },
    }
}
