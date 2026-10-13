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
    asset::RenderAssetUsages,
    camera::visibility::{RenderLayers, VisibilityRange},
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
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
/// Where a tree's detail drops, in metres from the camera: full, then fewer branches and
/// cards, then fewer again; past the last, it is not drawn.
const DETAIL_M: [f32; 3] = [30.0, 75.0, 180.0];
/// Over how many metres one level of detail dithers into the next.
const FADE_M: f32 = 6.0;
/// Crowns this near the camera cast a dappled shadow from a sparse stand-in of leaf cards;
/// out to the second distance, a solid one from rounded clumps, which the GPU draws far
/// more cheaply; beyond, none.
const SHADOW_M: [f32; 2] = [25.0, 80.0];
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
}

impl MaterialExtension for FoliageExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/foliage.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/foliage.wesl".into()
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<Foliage>::default())
        .init_resource::<Unmipped>()
        .add_systems(
            Update,
            (plant.run_if(not(resource_exists::<Forest>)), add_mipmaps),
        );
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
    };
    // Read at a slant, as bark up a trunk and leaves edge-on are, a picture blurs unless
    // sampled along the slant too (anisotropic filtering).
    let repeat = |settings: &mut ImageLoaderSettings| {
        settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            anisotropy_clamp: 8,
            ..ImageSamplerDescriptor::linear()
        });
    };
    let slanted = |settings: &mut ImageLoaderSettings| {
        settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            anisotropy_clamp: 8,
            ..ImageSamplerDescriptor::linear()
        });
    };
    let bark_picture: Handle<Image> = assets
        .load_builder()
        .with_settings(repeat)
        .load("textures/forest_spike/bark.png");
    let leaf_picture: Handle<Image> = assets
        .load_builder()
        .with_settings(slanted)
        .load("textures/forest_spike/leaves.png");
    unmipped.0.push((bark_picture.clone(), None));
    unmipped.0.push((leaf_picture.clone(), Some(LEAF_CUT)));
    let bark = materials.add(Foliage {
        base: StandardMaterial {
            base_color_texture: Some(bark_picture),
            perceptual_roughness: 0.95,
            reflectance: 0.2,
            ..default()
        },
        extension: wind.clone(),
    });
    let leaves = materials.add(Foliage {
        base: StandardMaterial {
            base_color_texture: Some(leaf_picture),
            alpha_mode: AlphaMode::Mask(LEAF_CUT),
            // Seen from both sides, but with the same normal on both, which leans out of
            // the crown, so a card does not flip from lit to dark as it turns.
            cull_mode: None,
            double_sided: false,
            diffuse_transmission: 0.35,
            perceptual_roughness: 0.75,
            reflectance: 0.3,
            ..default()
        },
        extension: wind.clone(),
    });
    // Only the sun sees it, so how it would look does not matter.
    let solid = plain.add(StandardMaterial::default());

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
            let crown = [0, 1, 2].map(|detail| crown(&mut dice, &tree, detail, height));
            let shadow = crown_stand_in(&mut dice, &tree, height);
            let clumps = crown_clumps(&tree);
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
                wood: wood.map(|build| meshes.add(build.mesh())),
                crown: crown.map(|build| meshes.add(build.mesh())),
                shadow: meshes.add(shadow.mesh()),
                clumps: meshes.add(clumps.mesh()),
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
    let detail = |level: usize| {
        let from = if level == 0 { 0.0 } else { DETAIL_M[level - 1] };
        VisibilityRange {
            start_margin: (from - FADE_M).max(0.0)..from,
            end_margin: DETAIL_M[level] - FADE_M..DETAIL_M[level],
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
                        let mut wood = tree.spawn((
                            Mesh3d(model.wood[level].clone()),
                            MeshMaterial3d(bark.clone()),
                            detail(level),
                        ));
                        if level == 2 {
                            wood.insert(NotShadowCaster);
                        }
                        tree.spawn((
                            Mesh3d(model.crown[level].clone()),
                            MeshMaterial3d(leaves.clone()),
                            detail(level),
                            NotShadowCaster,
                        ));
                    }
                    tree.spawn((
                        Mesh3d(model.shadow.clone()),
                        MeshMaterial3d(leaves.clone()),
                        VisibilityRange {
                            start_margin: 0.0..0.0,
                            end_margin: SHADOW_M[0] - FADE_M..SHADOW_M[0],
                            use_aabb: false,
                        },
                        RenderLayers::layer(SUN_ONLY),
                    ));
                    tree.spawn((
                        Mesh3d(model.clumps.clone()),
                        MeshMaterial3d(solid.clone()),
                        VisibilityRange {
                            start_margin: SHADOW_M[0] - FADE_M..SHADOW_M[0],
                            end_margin: SHADOW_M[1] - FADE_M..SHADOW_M[1],
                            use_aabb: false,
                        },
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
                bush.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(leaves.clone()),
                    VisibilityRange {
                        start_margin: 0.0..0.0,
                        end_margin: BUSH_M[0] - 2.0..BUSH_M[0],
                        use_aabb: false,
                    },
                ));
                bush.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(leaves.clone()),
                    VisibilityRange {
                        start_margin: BUSH_M[0] - 2.0..BUSH_M[0],
                        end_margin: BUSH_M[1] - FADE_M..BUSH_M[1],
                        use_aabb: false,
                    },
                    NotShadowCaster,
                ));
            });
        bushed += 1;
    }
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
    let sides = [[12, 8, 6, 4], [7, 5, 4, 3], [5, 4, 3, 3]][detail];
    let mut build = Build::default();
    for limb in tree.limbs.iter().filter(|limb| limb.depth <= deepest) {
        tube(&mut build, limb, sides[limb.depth.min(3) as usize], height);
    }
    build
}

/// A limb as a tube with bark round it.
fn tube(build: &mut Build, limb: &Limb, sides: u32, height: f32) {
    let start = build.positions.len() as u32;
    let mut along = 0.0;
    let mut side = (limb.points[1] - limb.points[0]).any_orthonormal_vector();
    for (i, (&point, &radius)) in limb.points.iter().zip(&limb.radii).enumerate() {
        if i > 0 {
            along += point.distance(limb.points[i - 1]);
        }
        let forward = if i + 1 < limb.points.len() {
            limb.points[i + 1] - point
        } else {
            point - limb.points[i - 1]
        }
        .normalize();
        // Carry the ring's orientation along the limb, so it does not twist.
        side = (side - forward * side.dot(forward)).normalize_or(forward.any_orthonormal_vector());
        let other = forward.cross(side);
        let wraps = (TAU * radius / 1.6).round().max(1.0);
        let bend = (point.y / height).clamp(0.0, 1.0);
        let flutter = if limb.depth >= 2 { 0.15 } else { 0.0 };
        for j in 0..=sides {
            let angle = j as f32 / sides as f32 * TAU;
            let normal = side * libm::cosf(angle) + other * libm::sinf(angle);
            let uv = Vec2::new(j as f32 / sides as f32 * wraps, along / 1.6);
            build.vertex(
                point + normal * radius,
                normal,
                uv,
                Vec2::new(bend * bend, flutter),
                Vec3::ONE,
            );
        }
    }
    let ring = sides + 1;
    for i in 0..(limb.points.len() as u32 - 1) {
        for j in 0..sides {
            let a = start + i * ring + j;
            let b = a + ring;
            build.indices.extend([a, b, a + 1, a + 1, b, b + 1]);
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

/// A tree's crown at a level of detail: a clump of many small cards at every tip, then a
/// few larger ones, then one large one.
fn crown(dice: &mut Rng, tree: &Tree, detail: usize, height: f32) -> Build {
    let canopy = Canopy::of(tree, height);
    let mut build = Build::default();
    for &(tip, size) in &tree.tips {
        let (cards, grow) = [(9 + dice.below(6), 1.0), (3, 1.35), (1, 1.8)][detail];
        clump(&mut build, dice, &canopy, tip, size * grow, cards);
    }
    build
}

/// What a crown's shadow is cast from: two cards at every tip, so the light still comes
/// through in flecks, with a fraction of the full crown's layers for the sun to draw.
fn crown_stand_in(dice: &mut Rng, tree: &Tree, height: f32) -> Build {
    let canopy = Canopy::of(tree, height);
    let mut build = Build::default();
    for &(tip, size) in &tree.tips {
        clump(&mut build, dice, &canopy, tip, size * 1.3, 2);
    }
    build
}

/// What a crown's shadow is cast from farther off: a rounded lump for each branch, as
/// big as most of its leaves, solid so the GPU can skip what is hidden. The light no
/// longer comes through in flecks, which nobody sees at that distance.
fn crown_clumps(tree: &Tree) -> Build {
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
        let radii = (spread * 0.8 + Vec3::splat(size * 0.6)) * Vec3::new(1.0, 0.75, 1.0);
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

/// A clump of leaf cards round a point.
fn clump(build: &mut Build, dice: &mut Rng, canopy: &Canopy, at: Vec3, size: f32, cards: u32) {
    for _ in 0..cards {
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
        let base = build.positions.len() as u32;
        for (u, v) in LEAF_OUTLINE {
            let corner =
                middle + right * (u * 2.0 - 1.0) * half * flip + up * (1.0 - v * 2.0) * half;
            build.vertex(corner, normal, Vec2::new(u, v), sway, tint);
        }
        for i in 1..LEAF_OUTLINE.len() as u32 - 1 {
            build.indices.extend([base, base + i, base + i + 1]);
        }
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
