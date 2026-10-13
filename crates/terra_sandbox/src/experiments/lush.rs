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
use std::f32::consts::TAU;
use std::sync::Arc;
use terra_gen::{Noise, Rng, Seed};
use terra_world::cover::{CoverRules, CoverSource, Growing};
use terra_world::flora::{Growth, Obstacles};
use terra_world::flora_kinds::Flora;
use terra_world::planet::Planet;

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
/// How many kinds of fallen log there are; each is drawn many times.
const LOG_KINDS: u64 = 5;
/// How much of a trunk one repeat of the bark's picture covers: round it, and along it.
const BARK_M: Vec2 = Vec2::new(2.15, 2.15);
/// The solid shadow lumps are this much of the crown's size, so a low sun, which looks
/// through many crowns, still comes through in places.
const SHADOW_LUMP: f32 = 0.6;
/// The render layer only the sun sees, which the stand-ins are on.
const SUN_ONLY: usize = 1;
/// How many kinds of grown tree, young tree and bush there are; each is drawn many times.
const OAK_KINDS: u64 = 6;
const YOUNG_KINDS: u64 = 3;
const BUSH_KINDS: u64 = 4;
/// Bushes cast shadows this near, and fade out at the second distance.
const BUSH_M: [f32; 2] = [20.0, 70.0];
/// Tree feet are filed in squares this wide, for the grass to thin under.
const FILED_M: f32 = 8.0;
/// Where the leaves' picture is cut: more opaque than this is leaf.
const LEAF_CUT: f32 = 0.45;
/// The outline of the leaves in their picture, as an octagon in texture coordinates, so a
/// card is cut to it rather than square: two thirds of the square, so the GPU shades a
/// third fewer empty pixels. Fitted to leaves.png's opaque pixels by hand-run script.
const LEAF_OUTLINE: [(f32, f32); 8] = [
    (0.310, 0.062),
    (0.662, 0.062),
    (0.946, 0.346),
    (0.946, 0.705),
    (0.713, 0.938),
    (0.317, 0.938),
    (0.054, 0.674),
    (0.054, 0.317),
];
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
    /// with moss and a wet foot (D218).
    #[uniform(100)]
    bark: f32,
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
    app.add_plugins(MaterialPlugin::<Foliage>::default())
        .init_resource::<Unmipped>()
        .init_resource::<Scanned>()
        .add_systems(
            Update,
            (
                plant.run_if(not(resource_exists::<Forest>)),
                add_mipmaps,
                dress_scans,
            ),
        );
}

/// Scans' glTF materials still to be copied into the materials their meshes are drawn with.
#[derive(Resource, Default)]
struct Scanned(Vec<(Handle<GltfMaterial>, Handle<StandardMaterial>)>);

/// Copies each scan's glTF material, once loaded, into the material it is drawn with: its
/// scanned colour, normal, roughness and occlusion maps.
fn dress_scans(
    mut scanned: ResMut<Scanned>,
    gltf: Res<Assets<GltfMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    scanned.0.retain(|(from, to)| {
        let (Some(from), Some(mut to)) = (gltf.get(from), materials.get_mut(to)) else {
            return true;
        };
        *to = StandardMaterial {
            base_color: from.base_color,
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
struct Forest;

/// The forest's map, on a flat patch laid over the ground: east and south of the start, in
/// metres.
struct Map {
    path: Path,
    /// Every tree's foot, filed by square, for the grass to thin under.
    trunks: HashMap<(i32, i32), Vec<Vec2>>,
    /// Where the grass grows thick or thin, and green or dry.
    patches: Noise,
}

/// The forest's grass, as terra_world's ground cover asks it: thick in the open, thin
/// under the trees, none on the paths.
struct LushCover(Arc<Map>);

impl CoverSource for LushCover {
    fn growing(&self, _planet: &Planet, _ground: Vec3, at: Vec2) -> Growing {
        self.0.growing(at)
    }

    fn reaches(&self, middle: Vec2, half: f32) -> bool {
        middle.length() < FOREST_M + 30.0 + half
    }
}

impl Map {
    /// What grows at a place: grass thick in the open and thin in the shade under the
    /// crowns, none on the paths; there, fallen leaves, and moss round the trees' feet and
    /// in patches on the leaves.
    fn growing(&self, at: Vec2) -> Growing {
        let off_path = smoothstep(-0.1, 0.6, self.path.edge_distance(at));
        let trunk = nearest(&self.trunks, at, FILED_M);
        let patch = 0.55
            + 0.45
                * self
                    .patches
                    .fbm(Vec3::new(at.x, 0.0, at.y) / 9.0, 2, 2.0, 0.5);
        let open = smoothstep(2.5, 7.0, trunk);
        let shaded = 1.0 - smoothstep(3.0, 8.0, trunk);
        let noise = |height: f32, across: f32| {
            0.5 + 0.5 * self.patches.sample(Vec3::new(at.x, height, at.y) / across)
        };
        // In the shade grass grows only where some light gets through, in patches.
        let shade_grass = smoothstep(0.6, 0.8, noise(19.0, 4.0));
        // Moss round a tree's foot, its edge wandering, and in patches on the leaves.
        let foot = 1.0 - smoothstep(0.5, 2.4, trunk + (noise(53.0, 1.3) - 0.5) * 2.2);
        let mossy = noise(31.0, 3.5);
        Growing {
            grass: off_path * patch * (0.9 * open + 0.6 * (1.0 - open) * shade_grass),
            lush: noise(7.0, 20.0),
            path: self.path.edge_distance(at) < 0.0,
            litter: off_path * shaded,
            moss: off_path
                * (foot * (0.35 + 0.65 * mossy)).max(shaded * smoothstep(0.6, 0.85, mossy) * 0.7),
        }
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
    crown: [Handle<Mesh>; 3],
    shadow: Handle<Mesh>,
    clumps: Handle<Mesh>,
    /// The solid lumps the crown's shadow is cast from beyond the cards: smaller than the
    /// crown, so a low sun still finds gaps between its branches.
    lumps: Handle<Mesh>,
    height: f32,
    trunk_m: f32,
}

#[allow(clippy::too_many_arguments)]
fn plant(
    mut commands: Commands,
    planet: Option<Res<Planet>>,
    flora: Option<Res<Flora>>,
    sun: Option<Single<Entity, With<DirectionalLight>>>,
    assets: Res<AssetServer>,
    mut obstacles: ResMut<Obstacles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Foliage>>,
    mut plain: ResMut<Assets<StandardMaterial>>,
    mut unmipped: ResMut<Unmipped>,
    mut scanned: ResMut<Scanned>,
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
    };
    // Read at a slant, as leaves edge-on are, a picture blurs unless sampled along the
    // slant too (anisotropic filtering).
    let slanted = |settings: &mut ImageLoaderSettings| {
        settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            anisotropy_clamp: 8,
            ..ImageSamplerDescriptor::linear()
        });
    };
    // Scanned furrowed bark, as an old oak's is, its colour in the range real bark has (D214), with the
    // relief and roughness that let it catch the sky's light in the shade. Under materials/,
    // so terra_world gives it its mipmaps.
    let bark_map = |file: &str, colour: bool| -> Handle<Image> {
        assets
            .load_builder()
            .with_settings(move |settings: &mut ImageLoaderSettings| settings.is_srgb = colour)
            .load(format!("materials/oak_bark/{file}"))
    };
    let leaf_picture: Handle<Image> = assets
        .load_builder()
        .with_settings(slanted)
        .load("textures/forest_spike/leaves.png");
    unmipped.0.push((leaf_picture.clone(), Some(LEAF_CUT)));
    let arm = bark_map("arm.jpg", false);
    let bark = materials.add(Foliage {
        base: StandardMaterial {
            // The scan is at the top of bark's range; this takes it to the darker grey-brown
            // of an old oak, about sRGB 78, as the forests of the Witcher 3 and Sons of the
            // Forest have it (D221).
            base_color: Color::srgb(0.58, 0.56, 0.54),
            base_color_texture: Some(bark_map("color.jpg", true)),
            normal_map_texture: Some(bark_map("normal.jpg", false)),
            // Occlusion in red, roughness in green, metal in blue.
            occlusion_texture: Some(arm.clone()),
            metallic_roughness_texture: Some(arm),
            perceptual_roughness: 1.0,
            metallic: 0.0,
            reflectance: 0.3,
            // The furrows read as cut into the trunk, not painted on it: relief mapping from
            // the bark's own height (D218).
            depth_map: Some(bark_map("depth.jpg", false)),
            parallax_depth_scale: 0.03,
            parallax_mapping_method: ParallaxMappingMethod::Relief { max_steps: 4 },
            max_parallax_layer_count: 16.0,
            ..default()
        },
        extension: FoliageExtension {
            bark: 1.0,
            ..wind.clone()
        },
    });
    let leaves = materials.add(Foliage {
        base: StandardMaterial {
            base_color_texture: Some(leaf_picture),
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
    // Only the sun sees it, so how it would look does not matter.
    let solid = plain.add(StandardMaterial::default());
    // A far crown: the leaves' average colour, rough, lit through from behind a little.
    let far_crown = plain.add(StandardMaterial {
        base_color: Color::srgb(0.26, 0.36, 0.13),
        perceptual_roughness: 0.9,
        reflectance: 0.2,
        diffuse_transmission: 0.2,
        ..default()
    });

    let trees: Vec<TreeModel> = (0..OAK_KINDS + YOUNG_KINDS)
        .map(|kind| {
            let mut dice = seed.derive("oak").index(kind).rng();
            let height = if kind < OAK_KINDS {
                dice.range(16.0, 25.0)
            } else {
                dice.range(7.0, 11.0)
            };
            let tree = grow_tree(&mut dice, height);
            let wood = [0, 1, 2].map(|detail| wood(&tree, detail, height));
            // Every level of the crown, and its shadow, from one set of cards (D221).
            let cards = crown_cards(&mut dice, &tree, height);
            let crown = [0, 1, 2].map(|detail| crown(&cards, detail));
            let shadow = crown_stand_in(&cards);
            let clumps = crown_clumps(&tree, 1.0);
            let lumps = crown_clumps(&tree, SHADOW_LUMP);
            info!(
                "lush: tree {kind}, {height:.0} m: wood {} {} {} triangles, crown {} {} {}, shadows {} {}",
                wood[0].triangles(),
                wood[1].triangles(),
                wood[2].triangles(),
                crown[0].triangles(),
                crown[1].triangles(),
                crown[2].triangles(),
                shadow.triangles(),
                clumps.triangles()
            );
            TreeModel {
                // Bark has a normal map, which needs the tubes' tangents.
                wood: wood.map(|build| {
                    let mut mesh = build.mesh();
                    mesh.generate_tangents().expect("the tubes have normals and texture places");
                    meshes.add(mesh)
                }),
                crown: crown.map(|build| meshes.add(build.mesh())),
                shadow: meshes.add(shadow.mesh()),
                clumps: meshes.add(clumps.mesh()),
                lumps: meshes.add(lumps.mesh()),
                height,
                trunk_m: tree.trunk_m,
            }
        })
        .collect();
    let bushes: Vec<Handle<Mesh>> = (0..BUSH_KINDS)
        .map(|kind| meshes.add(grow_bush(&mut seed.derive("bush").index(kind).rng()).mesh()))
        .collect();

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
    // and off the paths; then young ones in the gaps between them.
    let mut dice = seed.derive("places").rng();
    let glades = Noise::new(seed.derive("glades"));
    let mut feet: HashMap<(i32, i32), Vec<Vec2>> = HashMap::default();
    let mut planted = [0, 0];
    // The nearest of each to the start, for scripted runs to walk up to.
    let mut nearest_to_start = [Vec2::splat(f32::MAX); 2];
    for (young, tries, apart) in [(false, 14_000, SPACING_M), (true, 5_000, 4.5)] {
        for _ in 0..tries {
            let angle = dice.range(0.0, TAU);
            let distance = libm::sqrtf(dice.range(CLEARING_M * CLEARING_M, FOREST_M * FOREST_M));
            let at = Vec2::new(libm::cosf(angle), libm::sinf(angle)) * distance;
            // Glades open here and there, where only young trees grow.
            let glade = glades.fbm(Vec3::new(at.x, 0.0, at.y) / 45.0, 2, 2.0, 0.5);
            if path.edge_distance(at) < 2.5
                || (!young && glade < -0.25)
                || nearest(&feet, at, apart) < apart
            {
                continue;
            }
            feet.entry(square(at, FILED_M)).or_default().push(at);
            let kind = if young {
                OAK_KINDS + u64::from(dice.below(YOUNG_KINDS as u32))
            } else {
                u64::from(dice.below(OAK_KINDS as u32))
            } as usize;
            let model = &trees[kind];
            let scale = dice.range(0.85, 1.15);
            let size = (model.height * scale / DETAIL_FOR_M).clamp(0.45, 1.4);
            // Sunk a little, so the root flare meets uneven ground.
            let place = place_on(at, &mut dice, 0.3, scale);
            let foot = place.translation;
            obstacles.add(foot, model.trunk_m * scale * 1.2);
            commands
                .spawn((
                    place,
                    Visibility::default(),
                    Growth::new(cut_as("Leafy tree", model.height * scale), Some(foot), 1.4),
                ))
                .with_children(|tree| {
                    for level in 0..3 {
                        // The simplest trunk stands on under the far crown too.
                        let last = if level == 2 { 3 } else { level };
                        let mut wood = tree.spawn((
                            Mesh3d(model.wood[level].clone()),
                            MeshMaterial3d(bark.clone()),
                            detail(level, last, size),
                        ));
                        if level == 2 {
                            wood.insert(NotShadowCaster);
                        }
                        tree.spawn((
                            Mesh3d(model.crown[level].clone()),
                            MeshMaterial3d(leaves.clone()),
                            detail(level, level, size),
                            NotShadowCaster,
                        ));
                    }
                    // Far off, the crown as its solid lumps in the leaves' colour, so the
                    // forest carries on past where its cards end.
                    tree.spawn((
                        Mesh3d(model.clumps.clone()),
                        MeshMaterial3d(far_crown.clone()),
                        detail(3, 3, size),
                        NotShadowCaster,
                    ));
                    tree.spawn((
                        Mesh3d(model.shadow.clone()),
                        MeshMaterial3d(leaves.clone()),
                        between(0.0, SHADOW_M[0] * size),
                        RenderLayers::layer(SUN_ONLY),
                    ));
                    tree.spawn((
                        Mesh3d(model.lumps.clone()),
                        MeshMaterial3d(solid.clone()),
                        between(SHADOW_M[0] * size, SHADOW_M[1] * size),
                        RenderLayers::layer(SUN_ONLY),
                    ));
                });
            planted[young as usize] += 1;
            if at.length() < nearest_to_start[young as usize].length() {
                nearest_to_start[young as usize] = at;
            }
        }
    }

    // Bushes: along the paths and in the glades more than under the trees.
    let mut bushed = 0;
    for _ in 0..9_000 {
        let angle = dice.range(0.0, TAU);
        let distance = libm::sqrtf(dice.range(9.0, (FOREST_M + 10.0) * (FOREST_M + 10.0)));
        let at = Vec2::new(libm::cosf(angle), libm::sinf(angle)) * distance;
        let path = path.edge_distance(at);
        let chance = if path < 6.0 { 0.55 } else { 0.18 };
        if path < 0.6 || nearest(&feet, at, 2.0) < 1.6 || dice.unit() > chance {
            continue;
        }
        let scale = dice.range(0.7, 1.3);
        let place = place_on(at, &mut dice, 0.05, scale);
        let mesh = bushes[dice.below(BUSH_KINDS as u32) as usize].clone();
        commands
            .spawn((
                place,
                Visibility::default(),
                Growth::new(cut_as("Bush", place.scale.y), None, 0.5),
            ))
            .with_children(|bush| {
                // Drawn once, out to its distance; its shadow from a copy only the sun sees,
                // which fades out nearer, so the bush itself never dithers between two of
                // itself.
                let within = |m: f32| VisibilityRange {
                    start_margin: 0.0..0.0,
                    end_margin: m * (1.0 - FADE)..m,
                    use_aabb: false,
                };
                bush.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(leaves.clone()),
                    within(BUSH_M[1]),
                    NotShadowCaster,
                ));
                bush.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(leaves.clone()),
                    within(BUSH_M[0]),
                    RenderLayers::layer(SUN_ONLY),
                ));
            });
        bushed += 1;
    }

    // The forest floor (D219): scanned rocks and boulders, ferns in the shade, stumps, and
    // logs fallen from trees of the forest's own kind, each drawn out to its own distance.
    // A scan's mesh, and a material of our own that its glTF material is copied into once
    // loaded (`dress_scans`), as Bevy 0.20 loads a glTF's material as a `GltfMaterial`.
    let mut scan = |file: &str| -> (Handle<Mesh>, Handle<StandardMaterial>) {
        let path = format!("models/nature/forest/{file}.glb");
        let material = plain.add(StandardMaterial::default());
        scanned.0.push((
            assets.load(
                GltfAssetLabel::Material {
                    index: 0,
                    is_scale_inverted: false,
                }
                .from_asset(path.clone()),
            ),
            material.clone(),
        ));
        (
            assets.load(
                GltfAssetLabel::Primitive {
                    mesh: 0,
                    primitive: 0,
                }
                .from_asset(path),
            ),
            material,
        )
    };
    let rocks: Vec<_> = (1..=7)
        .map(|n| scan(&format!("mossy_rock_b_{n}")))
        .collect();
    let mut boulders: Vec<_> = (1..=6)
        .map(|n| scan(&format!("mossy_rock_a_{n}")))
        .collect();
    boulders.push(scan("boulder_1"));
    let fern = scan("fern");
    let stump = scan("stump");
    let logs: Vec<Handle<Mesh>> = (0..LOG_KINDS)
        .map(|kind| {
            let mut dice = seed.derive("log").index(kind).rng();
            let mut mesh = fallen_log(&mut dice).mesh();
            mesh.generate_tangents()
                .expect("a log has normals and texture places");
            meshes.add(mesh)
        })
        .collect();
    let within = |m: f32| VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: m * (1.0 - FADE)..m,
        use_aabb: false,
    };
    let mut floor_dice = seed.derive("floor").rng();
    let mut laid = [0; 5];
    // What, how many tries, how far it is drawn, its size, how far it sinks, whether it stops a
    // walker, and how it likes the shade: 1 under trees only, 0 anywhere, -1 the open.
    for (what, tries, seen_m, size, sink, solid, shade) in [
        (0, 1100, 80.0, (0.7, 1.6), 0.15, false, -0.3),
        (1, 60, 160.0, (0.6, 1.3), 0.3, true, 0.0),
        (2, 2600, 35.0, (0.9, 1.8), 0.02, false, 1.0),
        (3, 70, 90.0, (0.8, 1.3), 0.08, true, 0.6),
        (4, 160, 110.0, (0.8, 1.2), 0.0, false, 0.8),
    ] {
        for _ in 0..tries {
            let angle = floor_dice.range(0.0, TAU);
            let distance =
                libm::sqrtf(floor_dice.range(CLEARING_M * CLEARING_M, FOREST_M * FOREST_M));
            let at = Vec2::new(libm::cosf(angle), libm::sinf(angle)) * distance;
            let trunk = nearest(&feet, at, 8.0);
            let under = 1.0 - smoothstep(3.0, 8.0, trunk);
            let likes = if shade >= 0.0 {
                1.0 - shade + shade * under
            } else {
                1.0 + shade * under
            };
            if path.edge_distance(at) < 1.2 || trunk < 1.6 || floor_dice.unit() > likes {
                continue;
            }
            let scale = floor_dice.range(size.0, size.1);
            let mut place = place_on(at, &mut floor_dice, sink * scale, scale);
            let (mesh, material) = match what {
                0 => rocks[floor_dice.below(rocks.len() as u32) as usize].clone(),
                1 => boulders[floor_dice.below(boulders.len() as u32) as usize].clone(),
                2 => fern.clone(),
                3 => stump.clone(),
                _ => {
                    // A log lies along the ground, half sunk at its ends.
                    place.translation -= place.rotation * Vec3::Y * 0.1;
                    (
                        logs[floor_dice.below(LOG_KINDS as u32) as usize].clone(),
                        Handle::default(),
                    )
                }
            };
            let mut thing = if what == 4 {
                commands.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(bark.clone()),
                    place,
                    within(seen_m),
                ))
            } else {
                commands.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    place,
                    within(seen_m),
                ))
            };
            if what == 2 {
                thing.insert(NotShadowCaster);
            }
            if solid {
                obstacles.add(place.translation, 0.6 * scale);
            }
            laid[what] += 1;
        }
    }
    info!(
        "lush: the floor has {} rocks, {} boulders, {} ferns, {} stumps and {} fallen logs.",
        laid[0], laid[1], laid[2], laid[3], laid[4]
    );
    info!(
        "lush: {} grown trees, {} young ones and {bushed} bushes planted. The nearest stand at \
         {:.1},{:.1} and {:.1},{:.1} (east, south).",
        planted[0],
        planted[1],
        nearest_to_start[0].x,
        nearest_to_start[0].y,
        nearest_to_start[1].x,
        nearest_to_start[1].y
    );

    let map = Arc::new(Map {
        path,
        trunks: feet,
        patches: Noise::new(seed.derive("patches")),
    });
    commands.insert_resource(CoverRules {
        source: Arc::new(LushCover(map)),
        seed: seed.derive("grass"),
    });
    commands.insert_resource(Forest);
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
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.sway)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colours)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// A limb of a tree: a line of points, with how thick it is at each.
struct Limb {
    points: Vec<Vec3>,
    radii: Vec<f32>,
    depth: u32,
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
}

/// Grows an oak: a stout trunk that splits into a few great limbs spreading wider than
/// they rise, branching three times more, with leaf clumps at the ends.
fn grow_tree(dice: &mut Rng, height: f32) -> Tree {
    let mut tree = Tree {
        limbs: Vec::new(),
        tips: Vec::new(),
        branch_of: Vec::new(),
        branches: 0,
        trunk_m: height * 0.034,
    };
    let trunk = tree.trunk_m;
    grow(
        dice,
        Vec3::ZERO,
        Vec3::Y,
        height * 0.34,
        trunk,
        0,
        height,
        0,
        &mut tree,
    );
    tree
}

/// Grows one limb and everything that branches from it.
#[allow(clippy::too_many_arguments)]
fn grow(
    dice: &mut Rng,
    from: Vec3,
    heading: Vec3,
    length: f32,
    radius: f32,
    depth: u32,
    height: f32,
    branch: usize,
    tree: &mut Tree,
) {
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
    // The foot of the trunk flares into roots.
    if depth == 0 {
        for (i, r) in radii.iter_mut().enumerate() {
            *r *= 1.0 + 1.1 * libm::expf(-points[i].y / 0.9);
        }
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
                grow(dice, at, heading, length, radius * 0.62, 1, height, 0, tree);
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
                let base = points[index];
                let side = dice.unit_vector();
                let heading = (direction * 0.5 + side + Vec3::Y * 0.25).normalize();
                let next_length = length * dice.range(0.4, 0.6);
                // Each branch off a great limb, with all it carries, is one clump of leaves.
                let branch = if depth == 1 {
                    tree.branches += 1;
                    tree.branches - 1
                } else {
                    branch
                };
                grow(
                    dice,
                    base,
                    heading,
                    next_length,
                    radii[index] * 0.6,
                    depth + 1,
                    height,
                    branch,
                    tree,
                );
            }
        }
        // Twigs end in a clump of leaves.
        _ => {
            tree.tips.push((at, dice.range(1.4, 2.1)));
            tree.branch_of.push(branch);
        }
    }
    if depth >= 2 {
        tree.tips
            .push((points[points.len() / 2], dice.range(1.1, 1.7)));
        tree.branch_of.push(branch);
    }
}

/// A tree's wood at a level of detail: all of it, then without the twigs, then only the
/// trunk and great limbs; rounder near the camera.
fn wood(tree: &Tree, detail: usize, height: f32) -> Build {
    let deepest = [3, 2, 1][detail];
    let sides = [[20, 8, 6, 4], [10, 5, 4, 3], [6, 4, 3, 3]][detail];
    let mut build = Build::default();
    for limb in tree.limbs.iter().filter(|limb| limb.depth <= deepest) {
        tube(&mut build, limb, sides[limb.depth.min(3) as usize], height);
    }
    build
}

/// A log fallen and lying along the ground, from the forest's own kind of tree: a trunk's
/// length, tapering and a little bent, its ends cut (D219).
fn fallen_log(dice: &mut Rng) -> Build {
    let length = dice.range(4.0, 9.0);
    let radius = dice.range(0.22, 0.42);
    let bend = dice.range(-0.4, 0.4);
    let steps = 8;
    let points: Vec<Vec3> = (0..=steps)
        .map(|k| {
            let t = k as f32 / steps as f32;
            Vec3::new(
                (t - 0.5) * length,
                radius * (1.0 - 0.25 * t),
                bend * libm::sinf(t * std::f32::consts::PI),
            )
        })
        .collect();
    let radii: Vec<f32> = (0..=steps)
        .map(|k| radius * (1.0 - 0.3 * k as f32 / steps as f32))
        .collect();
    let limb = Limb {
        points: points.clone(),
        radii: radii.clone(),
        depth: 1,
    };
    let mut build = Build::default();
    // Lying still: a height so tall that nothing of it sways.
    tube(&mut build, &limb, 14, 1.0e4);
    // Each end cut flat: a disc of the bark's picture, facing out along the log.
    for (end, inward) in [
        (0, points[1] - points[0]),
        (steps, points[steps - 1] - points[steps]),
    ] {
        let normal = -inward.normalize();
        let side = normal.any_orthonormal_vector();
        let other = normal.cross(side);
        let centre = build.vertex(
            points[end],
            normal,
            Vec2::splat(0.5),
            Vec2::ZERO,
            Vec3::splat(0.7),
        );
        let ring: Vec<u32> = (0..12)
            .map(|j| {
                let angle = j as f32 / 12.0 * TAU;
                let out = side * libm::cosf(angle) + other * libm::sinf(angle);
                build.vertex(
                    points[end] + out * radii[end],
                    normal,
                    Vec2::new(0.5 + 0.2 * libm::cosf(angle), 0.5 + 0.2 * libm::sinf(angle)),
                    Vec2::ZERO,
                    Vec3::splat(0.7),
                )
            })
            .collect();
        for j in 0..12 {
            build.indices.extend([centre, ring[j], ring[(j + 1) % 12]]);
        }
    }
    build
}

/// A limb's rings: as grown, and for a trunk also between, every 0.35 m near the ground
/// and 1 m above, so its roots and lumps have rings to show on.
fn rings(limb: &Limb) -> (Vec<Vec3>, Vec<f32>) {
    if limb.depth > 0 {
        return (limb.points.clone(), limb.radii.clone());
    }
    let (mut points, mut radii) = (vec![limb.points[0]], vec![limb.radii[0]]);
    for k in 1..limb.points.len() {
        let (a, b) = (limb.points[k - 1], limb.points[k]);
        let spacing = if a.y < 3.0 { 0.35 } else { 1.0 };
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
/// flutes running up the bark and broad lumps, and on a trunk five roots spreading at its
/// foot, each tree its own by where it grew.
struct Shape {
    phase: f32,
    trunk: bool,
}

impl Shape {
    fn of(limb: &Limb) -> Self {
        let p = limb.points[0];
        let phase =
            (libm::sinf(p.x * 12.9898 + p.z * 78.233 + p.y * 37.719) * 43_758.547).fract() * TAU;
        Self {
            phase,
            trunk: limb.depth == 0,
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
            let foot = 1.0 - smoothstep(0.0, 1.6, up);
            let root = 0.5 + 0.5 * libm::cosf(5.0 * angle + self.phase);
            shape *= 1.0 + foot * foot * (0.3 * root * root - 0.1);
        }
        shape
    }
}

/// A limb as a tube with bark round it.
fn tube(build: &mut Build, limb: &Limb, sides: u32, height: f32) {
    let start = build.positions.len() as u32;
    let (points, radii) = rings(limb);
    let shape = Shape::of(limb);
    let mut along = 0.0;
    let mut side = (points[1] - points[0]).any_orthonormal_vector();
    for (i, (&point, &radius)) in points.iter().zip(&radii).enumerate() {
        if i > 0 {
            along += point.distance(points[i - 1]);
        }
        let forward = if i + 1 < points.len() {
            points[i + 1] - point
        } else {
            point - points[i - 1]
        }
        .normalize();
        // Carry the ring's orientation along the limb, so it does not twist.
        side = (side - forward * side.dot(forward)).normalize_or(forward.any_orthonormal_vector());
        let other = forward.cross(side);
        // One repeat of the bark's picture is about BARK_M round the limb, as many times
        // all the way up as at its foot: a count that changed from ring to ring cut the bark
        // into bands.
        let wraps = (TAU * radii[0] / BARK_M.x).round().max(1.0);
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
            let normal = (out * r - round * slope).normalize_or(out);
            let uv = Vec2::new(j as f32 / sides as f32 * wraps, along / BARK_M.y);
            let index = build.vertex(
                point + out * r,
                normal,
                uv,
                Vec2::new(bend * bend, flutter),
                Vec3::ONE,
            );
            // How much of the sky it sees, under its own crown and its neighbours': little
            // low on the trunk, most up in the crown (D216).
            build.colours[index as usize][3] = 0.35 + 0.65 * smoothstep(0.2, 0.85, bend);
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

impl Canopy {
    fn of(tree: &Tree, height: f32) -> Self {
        let tips = &tree.tips;
        let centre =
            tips.iter().fold(Vec3::ZERO, |sum, (tip, _)| sum + *tip) / tips.len().max(1) as f32;
        let reach = tips
            .iter()
            .map(|(tip, _)| tip.distance(centre))
            .fold(0.0, f32::max)
            .max(1.0);
        Self {
            centre,
            reach,
            height,
            bend: 1.0,
            tint: Vec3::ONE,
        }
    }
}

/// A crown's cards, grown once: for each tip, its cards in the order the levels of detail
/// keep them. A simpler level keeps the first few in the same places, each enlarged so they
/// cover as much as all of them did, as SpeedTree's levels do: crossfading between levels
/// then shows one crown thinning, not two different crowns (D221).
fn crown_cards(dice: &mut Rng, tree: &Tree, height: f32) -> Vec<Vec<Card>> {
    let canopy = Canopy::of(tree, height);
    tree.tips
        .iter()
        .map(|&(tip, size)| {
            let count = 9 + dice.below(6);
            (0..count).map(|_| card(dice, &canopy, tip, size)).collect()
        })
        .collect()
}

/// A crown at a level of detail: all its cards, then three a tip, then one.
fn crown(cards: &[Vec<Card>], detail: usize) -> Build {
    let mut build = Build::default();
    for tip in cards {
        let keep = [tip.len(), 3, 1][detail].min(tip.len());
        // Area kept: each card grows by the root of how many it stands for, a little less,
        // since the kept cards overlap less than all of them did.
        let grow = if keep == tip.len() {
            1.0
        } else {
            0.85 * libm::sqrtf(tip.len() as f32 / keep as f32).min(2.6)
        };
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
    let mut build = Build::default();
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
fn crown_clumps(tree: &Tree, scale: f32) -> Build {
    let mut build = Build::default();
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
            build.vertex(
                centre + normal * radii,
                normal,
                Vec2::ZERO,
                Vec2::ZERO,
                Vec3::ONE,
            );
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

/// One leaf card in a crown: where it is, which way it faces, how big, and how it is lit
/// and moves.
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
}

/// A card near `at` in a clump this size.
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
    }
}

impl Card {
    /// Draws it, `grow` times its size.
    fn draw(&self, build: &mut Build, grow: f32) {
        let half = self.half * grow;
        let base = build.positions.len() as u32;
        for (u, v) in LEAF_OUTLINE {
            let corner = self.middle
                + self.right * (u * 2.0 - 1.0) * half * self.flip
                + self.up * (1.0 - v * 2.0) * half;
            build.vertex(corner, self.normal, Vec2::new(u, v), self.sway, self.tint);
        }
        for i in 1..LEAF_OUTLINE.len() as u32 - 1 {
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
