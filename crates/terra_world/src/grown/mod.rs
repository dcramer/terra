//! Plants grown by rules rather than modelled (D222): trees, bushes, herbs and flowers made
//! of bark and cards of real leaves, as SpeedTree makes them for shipped games. A kind in
//! `flora.ron` says how it grows (`Grown`): a tree's habit, its bark, its leaf card and a
//! few numbers; a bush's, herb's or flower's card and shape. Each kind is grown as a few
//! variants from the world's seed (D182), each a set of meshes, and every plant of the kind
//! draws one of them.
//!
//! A tree is drawn at levels of detail that change at distances growing with its height,
//! dithering from one to the next, its crown's shadow cast from a sparse stand-in only the
//! sun sees, and past the third level its crown as solid lumps in the leaves' colour
//! (D203). A bush is drawn once, its shadow from a copy only the sun sees; herbs and flowers
//! fade out where the grass does.
//!
//! The leaf, frond and flower cards are one atlas (`materials/foliage`), its cards' cells
//! and outlines in `assets/data/foliage.ron`, made by `tools/textures` (`cards`); bark is
//! a material from the same tool.

pub mod build;
mod plants;
pub mod tree;

use crate::data;
use bevy::{
    camera::visibility::{RenderLayers, VisibilityRange},
    image::ImageLoaderSettings,
    light::NotShadowCaster,
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use serde::Deserialize;
use terra_gen::Seed;

/// Where a 20 m tree's detail drops, in metres from the camera: full, then fewer branches
/// and cards, then fewer again, then far off its crown as solid lumps; past the last, it is
/// not drawn. A tree's distances grow with its height, so detail follows how big it is on
/// screen rather than how far it is (D203, D221).
pub const DETAIL_M: [f32; 4] = [45.0, 100.0, 200.0, 420.0];
/// The height the distances are for.
pub const DETAIL_FOR_M: f32 = 20.0;
/// One level of detail dithers into the next over this share of the distance where they
/// meet, so a change is spread over many metres of walking.
pub const FADE: f32 = 0.2;
/// A crown casts a dappled shadow from a sparse stand-in of its own cards to the first
/// distance, a solid one from rounded lumps, which the GPU draws far more cheaply, to the
/// second; beyond, none. For a 20 m tree; they grow with its height as its levels do. Card
/// shadows farther out cost 4 ms a frame on the RTX 3080 (D221).
const SHADOW_M: [f32; 2] = [30.0, 100.0];
/// The solid shadow lumps are this much of the crown's size, so a low sun, which looks
/// through many crowns, still comes through in places (D204).
const SHADOW_LUMP: f32 = 0.52;
/// The render layer only the sun sees, which the shadow stand-ins are on.
pub const SUN_ONLY: usize = 1;
/// Bushes cast shadows this near, and fade out at the second distance.
const BUSH_M: [f32; 2] = [20.0, 70.0];
/// Herbs and flowers fade out by this distance, as the grass does.
const SMALL_M: f32 = 45.0;

/// How a kind grows, as `flora.ron` says.
#[derive(Deserialize, Debug, Clone)]
pub enum Grown {
    Tree(TreeDef),
    Bush(BushDef),
    Rosette(RosetteDef),
    Tuft(TuftDef),
}

/// How a tree branches.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Habit {
    /// A short trunk splitting into great limbs, wider than they rise, as an oak's.
    Spreading,
    /// A trunk to the top, with whorls of branches longest at the bottom, as a pine's.
    Conical,
    /// A slender trunk nearly to the top, with branches climbing steeply and twigs hanging,
    /// as a birch's.
    Upright,
}

fn white() -> (f32, f32, f32) {
    (1.0, 1.0, 1.0)
}

/// A tree: its habit, its bark (a material in `assets/materials` and how many metres its
/// picture covers), its leaves (a card) and their tint, how big a spray of them is, and
/// how thick its trunk is against its height.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TreeDef {
    pub habit: Habit,
    pub bark: String,
    pub bark_m: f32,
    #[serde(default = "white")]
    pub bark_tint: (f32, f32, f32),
    pub leaves: String,
    #[serde(default = "white")]
    pub tint: (f32, f32, f32),
    pub clump_m: f32,
    pub trunk: f32,
}

/// A bush: its leaf card and their tint, how many clumps of them, and how wide against
/// its height.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct BushDef {
    pub leaves: String,
    #[serde(default = "white")]
    pub tint: (f32, f32, f32),
    pub clumps: (u32, u32),
    pub spread: f32,
}

/// A herb or fern: its leaf card and tint, how many leaves, how steeply they rise from the
/// ground (degrees, from and to), and where in the card's picture the leaf's stalk and tip
/// are (0 to 1 from its top left).
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct RosetteDef {
    pub card: String,
    #[serde(default = "white")]
    pub tint: (f32, f32, f32),
    pub leaves: (u32, u32),
    pub tilt: (f32, f32),
    pub stalk: (f32, f32),
    pub tip: (f32, f32),
}

/// A flower: its cards, a variant to each in turn, their tint, and how many cross.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TuftDef {
    pub cards: Vec<String>,
    #[serde(default = "white")]
    pub tint: (f32, f32, f32),
    pub planes: u32,
}

impl Grown {
    /// Every card it names.
    pub fn cards(&self) -> Vec<&str> {
        match self {
            Grown::Tree(def) => vec![def.leaves.as_str()],
            Grown::Bush(def) => vec![def.leaves.as_str()],
            Grown::Rosette(def) => vec![def.card.as_str()],
            Grown::Tuft(def) => def.cards.iter().map(String::as_str).collect(),
        }
    }
}

/// One card in the atlas: the top-left of its cell and how far across the cell is, the
/// outline round its leaves in the atlas's coordinates, and their average colour, linear.
#[derive(Clone, Debug)]
pub struct Card {
    pub cell: Vec2,
    pub across: f32,
    pub outline: Vec<(f32, f32)>,
    pub colour: Vec3,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CardFile {
    atlas: String,
    cut: f32,
    across: f32,
    cards: HashMap<String, CardEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CardEntry {
    cell: (f32, f32),
    outline: Vec<(f32, f32)>,
    colour: (f32, f32, f32),
}

/// The cards there are (`assets/data/foliage.ron`), and the atlas they are in.
#[derive(Resource)]
pub struct Cards {
    pub atlas: String,
    pub cut: f32,
    cards: HashMap<String, Card>,
}

impl Cards {
    pub const FILE: &str = "foliage.ron";

    pub fn read() -> Self {
        let file: CardFile = data::read(Self::FILE);
        let cards = file
            .cards
            .into_iter()
            .map(|(name, entry)| {
                let colour = Color::srgb(entry.colour.0, entry.colour.1, entry.colour.2)
                    .to_linear()
                    .to_vec3();
                (
                    name,
                    Card {
                        cell: Vec2::new(entry.cell.0, entry.cell.1),
                        across: file.across,
                        outline: entry.outline,
                        colour,
                    },
                )
            })
            .collect();
        Self {
            atlas: file.atlas,
            cut: file.cut,
            cards,
        }
    }

    /// A card by name. A name the file lacks is a mistake in the data, caught by a test.
    pub fn get(&self, name: &str) -> &Card {
        self.cards
            .get(name)
            .unwrap_or_else(|| panic!("no card {name} in {}", Self::FILE))
    }

    pub fn has(&self, name: &str) -> bool {
        self.cards.contains_key(name)
    }
}

pub type Foliage = ExtendedMaterial<StandardMaterial, FoliageExtension>;

/// What the foliage shader is told: which way the wind blows, level with the ground, and
/// how hard. Field order must match the struct in foliage.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FoliageExtension {
    #[uniform(100)]
    pub direction: Vec3,
    #[uniform(100)]
    pub strength: f32,
    /// 1 for bark: each tree's bark is shifted by where it stands, and weathered in patches
    /// with moss and a wet foot (D218).
    #[uniform(100)]
    pub bark: f32,
    /// For a crown whose cards drop out with distance: from where, to where, and 1 (D223).
    /// Off for grown plants until they draw their crowns that way.
    #[uniform(100)]
    pub cards: Vec3,
}

impl MaterialExtension for FoliageExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/foliage.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/foliage.wesl".into()
    }

    // Crossfades between levels of detail dither in a pattern that moves every frame, for
    // TAA to blend, leaves fade at the camera, and the sky a point sees lights it (D214,
    // D216).
    fn fragment_shader() -> ShaderRef {
        "shaders/foliage_fragment.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/foliage_prepass.wesl".into()
    }
}

/// The materials grown plants are drawn in: the leaf cards, each bark by name and tint,
/// far crowns in their vertex colours, and what only the sun sees.
#[derive(Resource)]
pub struct Looks {
    wind: FoliageExtension,
    pub leaves: Handle<Foliage>,
    far: Handle<StandardMaterial>,
    solid: Handle<StandardMaterial>,
    barks: HashMap<String, Handle<Foliage>>,
}

impl Looks {
    /// The materials, with the wind blowing along `wind` (level with the ground).
    pub fn new(
        wind: Vec3,
        cards: &Cards,
        assets: &AssetServer,
        foliage: &mut Assets<Foliage>,
        plain: &mut Assets<StandardMaterial>,
    ) -> Self {
        let wind = FoliageExtension {
            direction: wind,
            strength: 1.0,
            bark: 0.0,
            cards: Vec3::ZERO,
        };
        let leaves = foliage.add(Foliage {
            base: StandardMaterial {
                base_color_texture: Some(assets.load(cards.atlas.clone())),
                alpha_mode: AlphaMode::Mask(cards.cut),
                // Seen from both sides, but with the same normal on both, which leans out of
                // the crown, so a card does not flip from lit to dark as it turns.
                cull_mode: None,
                double_sided: false,
                diffuse_transmission: 0.55,
                perceptual_roughness: 0.85,
                reflectance: 0.2,
                ..default()
            },
            extension: wind.clone(),
        });
        // A far crown: rough, lit through from behind a little, in its vertex colours.
        let far = plain.add(StandardMaterial {
            perceptual_roughness: 0.9,
            reflectance: 0.2,
            diffuse_transmission: 0.2,
            ..default()
        });
        // Only the sun sees it, so how it would look does not matter.
        let solid = plain.add(StandardMaterial::default());
        Self {
            wind,
            leaves,
            far,
            solid,
            barks: HashMap::default(),
        }
    }

    /// A bark, `materials/<name>`, tinted: its colour, relief, roughness and crevices.
    fn bark(
        &mut self,
        name: &str,
        tint: Color,
        assets: &AssetServer,
        foliage: &mut Assets<Foliage>,
    ) -> Handle<Foliage> {
        let key = format!("{name} {:?}", tint.to_srgba().to_f32_array());
        if let Some(handle) = self.barks.get(&key) {
            return handle.clone();
        }
        let linear = |settings: &mut ImageLoaderSettings| settings.is_srgb = false;
        let map = |file: &str| format!("materials/{name}/{file}");
        let arm: Handle<Image> = assets
            .load_builder()
            .with_settings(linear)
            .load(map("arm.jpg"));
        let handle = foliage.add(Foliage {
            base: StandardMaterial {
                base_color: tint,
                base_color_texture: Some(assets.load(map("color.jpg"))),
                normal_map_texture: Some(
                    assets
                        .load_builder()
                        .with_settings(linear)
                        .load(map("normal.jpg")),
                ),
                occlusion_texture: Some(arm.clone()),
                metallic_roughness_texture: Some(arm),
                perceptual_roughness: 1.0,
                metallic: 0.0,
                reflectance: 0.15,
                ..default()
            },
            extension: FoliageExtension {
                bark: 1.0,
                ..self.wind.clone()
            },
        });
        self.barks.insert(key, handle.clone());
        handle
    }
}

/// One variant of a kind, ready to draw many times.
#[derive(Clone)]
pub enum Grew {
    Tree(Box<TreeModel>),
    Bush(Handle<Mesh>),
    Small(Handle<Mesh>),
}

/// A tree at each level of detail, and the stand-ins its shadow is cast from.
#[derive(Clone)]
pub struct TreeModel {
    pub wood: [Handle<Mesh>; 3],
    pub crown: [Handle<Mesh>; 3],
    pub bark: Handle<Foliage>,
    /// The crown far off, as solid lumps.
    pub far: Handle<Mesh>,
    pub shadow_cards: Handle<Mesh>,
    pub shadow_lumps: Handle<Mesh>,
    pub trunk_m: f32,
}

/// A variant and its size as grown: how tall it is, and how far round its foot.
#[derive(Clone)]
pub struct Variant {
    pub grew: Grew,
    pub height: f32,
    pub spread: f32,
}

fn vec3(rgb: (f32, f32, f32)) -> Vec3 {
    Vec3::new(rgb.0, rgb.1, rgb.2)
}

/// Grows `count` variants of a kind from a seed, each at a height in `size` (a tree's;
/// smaller plants are grown at about a metre and scaled where they stand).
#[allow(clippy::too_many_arguments)]
pub fn grow(
    def: &Grown,
    name: &str,
    seed: Seed,
    count: u32,
    size: (f32, f32),
    cards: &Cards,
    looks: &mut Looks,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    foliage: &mut Assets<Foliage>,
) -> Vec<Variant> {
    (0..count)
        .map(|index| {
            let mut dice = seed.index(u64::from(index)).rng();
            match def {
                Grown::Tree(def) => {
                    let height = size.0 + (size.1 - size.0) * (index as f32 + 0.5) / count as f32;
                    let shape = tree::grow(&mut dice, def, height);
                    let leaf = cards.get(&def.leaves);
                    let tint = vec3(def.tint);
                    let canopy = tree::Canopy::of(&shape, tint);
                    let mut wood = [0, 1, 2].map(|detail| tree::wood(&shape, detail, def.bark_m));
                    let cards = tree::crown_cards(&mut dice, &shape, &canopy);
                    let mut crown = [0, 1, 2].map(|detail| tree::crown(&cards, &shape, leaf, detail));
                    for build in wood.iter_mut().chain(crown.iter_mut()) {
                        build.see_sky(|at| canopy.sky(at));
                    }
                    let shadow_cards = tree::shadow_cards(&cards, &shape, leaf);
                    let far = tree::lumps(&shape, 1.0, leaf.colour * tint * 0.85);
                    let shadow_lumps = tree::lumps(&shape, SHADOW_LUMP, Vec3::ONE);
                    debug!(
                        "{name} {index}, {height:.0} m: wood {} {} {}, crown {} {} {}, shadow {} {}, far {} triangles",
                        wood[0].triangles(),
                        wood[1].triangles(),
                        wood[2].triangles(),
                        crown[0].triangles(),
                        crown[1].triangles(),
                        crown[2].triangles(),
                        shadow_cards.triangles(),
                        shadow_lumps.triangles(),
                        far.triangles()
                    );
                    let bark = looks.bark(
                        &def.bark,
                        Color::srgb(def.bark_tint.0, def.bark_tint.1, def.bark_tint.2),
                        assets,
                        foliage,
                    );
                    Variant {
                        grew: Grew::Tree(Box::new(TreeModel {
                            wood: wood.map(|build| meshes.add(build.mesh(true))),
                            crown: crown.map(|build| meshes.add(build.mesh(false))),
                            bark,
                            far: meshes.add(far.mesh(false)),
                            shadow_cards: meshes.add(shadow_cards.mesh(false)),
                            shadow_lumps: meshes.add(shadow_lumps.mesh(false)),
                            trunk_m: shape.trunk_m,
                        })),
                        height,
                        spread: shape.trunk_m * 1.3,
                    }
                }
                Grown::Bush(def) => {
                    let build = plants::bush(&mut dice, def, cards.get(&def.leaves), vec3(def.tint));
                    Variant {
                        height: build.height(),
                        spread: 0.3,
                        grew: Grew::Bush(meshes.add(build.mesh(false))),
                    }
                }
                Grown::Rosette(def) => {
                    let build = plants::rosette(&mut dice, def, cards.get(&def.card), vec3(def.tint));
                    Variant {
                        height: build.height(),
                        spread: 0.2,
                        grew: Grew::Small(meshes.add(build.mesh(false))),
                    }
                }
                Grown::Tuft(def) => {
                    let card = &def.cards[index as usize % def.cards.len()];
                    let build = plants::tuft(&mut dice, def.planes, cards.get(card), vec3(def.tint));
                    Variant {
                        height: build.height(),
                        spread: 0.1,
                        grew: Grew::Small(meshes.add(build.mesh(false))),
                    }
                }
            }
        })
        .collect()
}

/// Levels `first` to `last` of a tree `size` times the height the distances are for.
pub fn detail(first: usize, last: usize, size: f32) -> VisibilityRange {
    let at = |level: usize| DETAIL_M[level] * size;
    let from = if first == 0 { 0.0 } else { at(first - 1) };
    VisibilityRange {
        start_margin: from * (1.0 - FADE)..from,
        end_margin: at(last) * (1.0 - FADE)..at(last),
        use_aabb: false,
    }
}

/// Drawn from `from` to `to` metres from the camera, fading in and out over a fifth of each.
fn between(from: f32, to: f32) -> VisibilityRange {
    VisibilityRange {
        start_margin: from * (1.0 - FADE)..from,
        end_margin: to * (1.0 - FADE)..to,
        use_aabb: false,
    }
}

/// Drawn from the camera out to `m`, fading over its last stretch.
fn within(m: f32) -> VisibilityRange {
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: m * (1.0 - FADE)..m,
        use_aabb: false,
    }
}

/// Draws a variant on a plant's entity, which stands it where it grows, `height` metres
/// tall: a herb or flower on the entity itself, a tree or bush as its children.
pub fn draw(entity: &mut EntityCommands, variant: &Variant, height: f32, looks: &Looks) {
    if let Grew::Small(mesh) = &variant.grew {
        entity.insert((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(looks.leaves.clone()),
            within(SMALL_M),
            NotShadowCaster,
        ));
        return;
    }
    entity.with_children(|plant| draw_children(plant, variant, height, looks));
}

fn draw_children(plant: &mut ChildSpawnerCommands, variant: &Variant, height: f32, looks: &Looks) {
    match &variant.grew {
        Grew::Tree(model) => {
            let size = (height / DETAIL_FOR_M).clamp(0.45, 1.4);
            for level in 0..3 {
                // The simplest trunk stands on under the far crown too.
                let last = if level == 2 { 3 } else { level };
                let mut wood = plant.spawn((
                    Mesh3d(model.wood[level].clone()),
                    MeshMaterial3d(model.bark.clone()),
                    detail(level, last, size),
                ));
                if level == 2 {
                    wood.insert(NotShadowCaster);
                }
                plant.spawn((
                    Mesh3d(model.crown[level].clone()),
                    MeshMaterial3d(looks.leaves.clone()),
                    detail(level, level, size),
                    NotShadowCaster,
                ));
            }
            // Far off, the crown as its solid lumps in the leaves' colour, so the forest
            // carries on past where its cards end.
            plant.spawn((
                Mesh3d(model.far.clone()),
                MeshMaterial3d(looks.far.clone()),
                detail(3, 3, size),
                NotShadowCaster,
            ));
            plant.spawn((
                Mesh3d(model.shadow_cards.clone()),
                MeshMaterial3d(looks.leaves.clone()),
                between(0.0, SHADOW_M[0] * size),
                RenderLayers::layer(SUN_ONLY),
            ));
            plant.spawn((
                Mesh3d(model.shadow_lumps.clone()),
                MeshMaterial3d(looks.solid.clone()),
                between(SHADOW_M[0] * size, SHADOW_M[1] * size),
                RenderLayers::layer(SUN_ONLY),
            ));
        }
        // Drawn once, out to its distance; its shadow from a copy only the sun sees, which
        // fades out nearer, so the bush itself never dithers between two of itself.
        Grew::Bush(mesh) => {
            plant.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(looks.leaves.clone()),
                within(BUSH_M[1]),
                NotShadowCaster,
            ));
            plant.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(looks.leaves.clone()),
                within(BUSH_M[0]),
                RenderLayers::layer(SUN_ONLY),
            ));
        }
        Grew::Small(_) => {}
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<Foliage>::default())
        .insert_resource(Cards::read());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flora_kinds::Flora;
    use crate::mipmaps::CUT_OUT;

    #[test]
    fn every_card_a_kind_names_is_in_the_atlas_and_cut_where_its_mipmaps_are() {
        let cards = Cards::read();
        assert_eq!(cards.cut, CUT_OUT);
        for kind in Flora::read().kinds() {
            for card in kind.grown.iter().flat_map(Grown::cards) {
                assert!(
                    cards.has(card),
                    "{} names card {card}, which is not in the atlas",
                    kind.name
                );
            }
        }
        // An outline is an octagon inside its cell.
        for card in cards.cards.values() {
            assert_eq!(card.outline.len(), 8);
            for &(u, v) in &card.outline {
                let own = (Vec2::new(u, v) - card.cell) / card.across;
                assert!(own.min_element() >= -1e-3 && own.max_element() <= 1.0 + 1e-3);
            }
        }
    }

    #[test]
    fn every_bark_a_tree_names_is_a_material_with_its_maps() {
        let folder =
            bevy::asset::io::file::FileAssetReader::get_base_path().join("assets/materials");
        for kind in Flora::read().kinds() {
            if let Some(Grown::Tree(def)) = &kind.grown {
                for map in ["color.jpg", "normal.jpg", "arm.jpg"] {
                    assert!(
                        folder.join(&def.bark).join(map).is_file(),
                        "{}'s bark {} has no {map}",
                        kind.name,
                        def.bark
                    );
                }
            }
        }
    }

    #[test]
    fn a_tree_grows_the_same_from_the_same_seed_and_keeps_to_its_budget() {
        let cards = Cards::read();
        for kind in Flora::read().kinds() {
            let Some(Grown::Tree(def)) = &kind.grown else {
                continue;
            };
            let (low, high) = kind.size.unwrap_or((10.0, 10.0));
            for height in [low, high] {
                let grown = |seed: u64| {
                    let mut dice = Seed(seed).rng();
                    let shape = tree::grow(&mut dice, def, height);
                    let leaf = cards.get(&def.leaves);
                    let canopy = tree::Canopy::of(&shape, Vec3::ONE);
                    let wood = tree::wood(&shape, 0, def.bark_m);
                    let cards = tree::crown_cards(&mut dice, &shape, &canopy);
                    let crown = tree::crown(&cards, &shape, leaf, 0);
                    (wood.triangles(), crown.triangles(), crown.positions)
                };
                let (wood, crown, positions) = grown(3);
                assert_eq!(grown(3).2, positions, "{} grew differently", kind.name);
                // The style guide's near tree, leaves included (docs/art-style.md).
                assert!(
                    wood + crown <= 8_000,
                    "{} at {height} m: {wood} triangles of wood and {crown} of leaves",
                    kind.name
                );
                assert!(
                    crown > 300,
                    "{} at {height} m has only {crown} triangles of leaves",
                    kind.name
                );
            }
        }
    }
}
