//! Woods by the land's rules (D313, D321, docs/demos.md §1): on a world with mountains, the
//! trees round the start stand where `terra_land::woods` says, closer and taller where the
//! growing season is warm, thinning and shrinking toward the tree line, krummholz past it,
//! none on cliffs, rock, scree or water. Near the camera each is a grown conifer with its
//! levels of detail, drawn by what `Conifers` holds (D366): the lush forest's Scots pines and
//! Norway spruces where the layer above gives them, the flora's generic pine where nothing
//! does. Past where its levels end, a billboard drawn by the far forest's shader
//! (`far_forest.wesl`, D273), merged a square at a time, the same trees on the same feet, so
//! the forest carries up the far slopes to its line; each card stands for its grown tree and
//! comes in where that tree's last level goes.

use crate::flora_kinds::Flora;
use crate::grown::{self, Cards};
use crate::planet::Planet;
use crate::start::Start;
use bevy::{
    asset::RenderAssetUsages,
    camera::{primitives::Aabb, visibility::NoAutoAabb},
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use std::sync::Arc;

pub use terra_land::woods::TreeKind;
use terra_land::woods::{Tree, Woods};

/// Grown trees out to here from the start, and billboards for every tree out to
/// the far one; the far trees on a grid twice as coarse, each card wider, past `COARSE_M`.
const NEAR_M: f32 = 600.0;
const FAR_M: f32 = 4_500.0;
const COARSE_M: f32 = 1_600.0;
/// No tree stands this near the start.
const CLEAR_M: f32 = 14.0;
/// A tree to a cell this wide, at most, near and far.
const SPACING_M: f32 = 6.5;
/// The far cards are merged in squares this wide.
const SQUARE_M: f32 = 250.0;
/// Each kind's full height range and how wide its crown is for its height, oak, pine and
/// spruce, as the lush forest grows its oaks, Scots pines and Norway spruces (lush/far.rs).
const HEIGHTS: [(f32, f32); 3] = [(18.0, 26.0), (23.0, 31.0), (18.0, 26.0)];
const CROWN: [f32; 3] = [0.62, 0.42, 0.36];
/// The far crowns' colours, oak, pine and spruce, for the flora's pine: its leaves' average.
const FAR_CROWNS: [Color; 3] = [
    Color::srgb(0.15, 0.23, 0.12),
    Color::srgb(0.15, 0.23, 0.12),
    Color::srgb(0.13, 0.21, 0.12),
];
/// A card standing for a grown tree is told where that tree's last level ends in its colour's
/// blue, in metres over this (`far_forest.wesl`).
const HANDOVER_SCALE_M: f32 = 1_000.0;

/// What draws the woods' conifers near the camera (D366). The woods say where each stands,
/// which kind it is and how tall; how it is drawn is the layer above's to fill in when it has
/// better trees than the flora's (the sandbox gives the lush forest's), as a placement system
/// is handed its trees' models rather than knowing how they were made. Without one, they are
/// the flora's generic grown pine (`FloraPines`).
pub trait ConiferModels: Send + Sync + 'static {
    /// Draws a conifer on its entity: its transform, standing it at its foot, and its levels
    /// of detail as its children.
    fn draw(&self, entity: &mut EntityCommands, tree: &Planted) -> Drawn;

    /// The far cards' crown colours, oak, pine and spruce, linear: what the near crowns'
    /// leaves average to, as the far forest's are coloured (D273).
    fn far_crowns(&self) -> [Vec3; 3];
}

/// The conifers the woods are drawn with, if the layer above has given any; it inserts them
/// before `Planting`.
#[derive(Resource, Clone)]
pub struct Conifers(pub Arc<dyn ConiferModels>);

/// Where the woods are planted, at `PostStartup`: whatever fills in `Conifers` runs before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Planting;

/// A conifer the woods stand: where its foot is on the ground, how it is turned, what kind it
/// is, how tall, how much wider than its kind its crown spreads (krummholz's), and a number of
/// its own from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Planted {
    pub foot: Vec3,
    pub rotation: Quat,
    pub kind: TreeKind,
    pub height_m: f32,
    pub squat: f32,
    pub luck: f32,
}

/// How a conifer was drawn: out to how far from the eye its last level reaches, where its far
/// card takes over, and as which kind, which its card is drawn as.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub to_m: f32,
    pub kind: TreeKind,
}

pub type FarWoods = ExtendedMaterial<StandardMaterial, FarExtension>;

/// What the far forest's shader is told. Field order must match far_forest.wesl, whose
/// kinds are oak, pine and spruce.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct FarExtension {
    #[uniform(100)]
    crowns: [Vec4; 3],
    #[uniform(100)]
    near: Vec4,
    /// The land's horizon round the camera (`crate::horizon`, D416): a valley's trees in a
    /// range's shadow past the shadow map's reach.
    #[uniform(100)]
    horizon_middle: Vec4,
    #[uniform(100)]
    horizon_east: Vec4,
    #[uniform(100)]
    horizon_north: Vec4,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    horizon_map: Option<Handle<Image>>,
}

/// The far forest told the land's horizon each time it is baked.
fn see_horizon(
    horizon: Res<crate::horizon::HorizonPicture>,
    mut materials: ResMut<Assets<FarWoods>>,
) {
    let (middle, east, north, map) = horizon.uniforms();
    for (_, material) in materials.iter_mut() {
        if material.extension.horizon_map != map {
            material.extension.horizon_middle = middle;
            material.extension.horizon_east = east;
            material.extension.horizon_north = north;
            material.extension.horizon_map = map.clone();
        }
    }
}

impl MaterialExtension for FarExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/far_forest.wesl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/far_forest.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/far_forest.wesl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/far_forest.wesl".into()
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<FarWoods>::default())
        .add_systems(PostStartup, plant.in_set(Planting))
        .add_systems(
            Update,
            see_horizon.run_if(resource_exists_and_changed::<crate::horizon::HorizonPicture>),
        );
}

/// The flora's generic grown pine (`flora.ron`'s Pine), in its variants, for both kinds.
struct FloraPines {
    variants: Vec<grown::Variant>,
    looks: grown::Looks,
}

impl ConiferModels for FloraPines {
    fn draw(&self, entity: &mut EntityCommands, tree: &Planted) -> Drawn {
        let variants = &self.variants;
        let variant = &variants[(tree.luck * variants.len() as f32) as usize % variants.len()];
        let scale = tree.height_m / variant.height;
        let up = tree.foot.normalize();
        entity.insert(Transform {
            translation: tree.foot - up * 0.15,
            rotation: tree.rotation,
            scale: Vec3::new(scale * tree.squat, scale, scale * tree.squat),
        });
        grown::draw(entity, variant, tree.height_m, &self.looks);
        let size = (tree.height_m / grown::DETAIL_FOR_M).clamp(0.45, 1.4);
        Drawn {
            to_m: grown::detail(3, 3, size).end_margin.end,
            kind: tree.kind,
        }
    }

    fn far_crowns(&self) -> [Vec3; 3] {
        FAR_CROWNS.map(|colour| colour.to_linear().to_vec3())
    }
}

#[allow(clippy::too_many_arguments)]
fn flora_pines(
    flora: &Flora,
    cards: &Cards,
    assets: &AssetServer,
    wind: Vec3,
    parting: Option<&crate::parting::Parting>,
    seed: terra_gen::Seed,
    meshes: &mut Assets<Mesh>,
    foliage: &mut Assets<grown::Foliage>,
    plain: &mut Assets<StandardMaterial>,
) -> Option<FloraPines> {
    let mut looks = grown::Looks::new(
        wind,
        parting.map(|p| p.0.clone()).unwrap_or_default(),
        cards,
        assets,
        foliage,
        plain,
    );
    let Some(pine) = flora.kinds().iter().find(|k| k.name == "Pine") else {
        error!("woods: flora.ron has no Pine to grow");
        return None;
    };
    let how = pine.grown.as_ref()?;
    let size = pine.size.unwrap_or((16.0, 27.0));
    let variants = grown::grow(
        how,
        "woods pine",
        seed,
        pine.variants,
        size,
        cards,
        &mut looks,
        assets,
        meshes,
        foliage,
    );
    (!variants.is_empty()).then_some(FloraPines { variants, looks })
}

/// The conifer the land's rules stand: its height for its kind and stature, how much wider
/// than tall-for-its-kind it spreads, and how it is turned.
fn planted(tree: &Tree) -> Planted {
    let (low, high) = HEIGHTS[kind_index(tree.kind)];
    let up = tree.at.normalize();
    // Toward their limit trees grow squat as well as short, crowns spreading low where the
    // wind and winter's snow cut back what stands above it: krummholz is wider than tall.
    let stunted = (1.0 - tree.stature / 0.7).clamp(0.0, 1.0);
    Planted {
        foot: tree.at,
        rotation: Quat::from_axis_angle(up, tree.luck * std::f32::consts::TAU * 7.0)
            * Quat::from_rotation_arc(Vec3::Y, up),
        kind: tree.kind,
        height_m: (low + (high - low) * tree.luck) * tree.stature * tree.age,
        squat: 1.0 + 1.6 * stunted * stunted,
        luck: tree.luck,
    }
}

/// Where a tree stands, exactly, to find the grown tree a far card stands for: the same rules
/// give the same tree the same foot to the bit.
fn foot_key(at: Vec3) -> [u32; 3] {
    at.to_array().map(f32::to_bits)
}

#[allow(clippy::too_many_arguments)]
fn plant(
    mut commands: Commands,
    planet: Res<Planet>,
    start: Res<Start>,
    flora: Res<Flora>,
    cards: Res<Cards>,
    conifers: Option<Res<Conifers>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut foliage: ResMut<Assets<grown::Foliage>>,
    mut plain: ResMut<Assets<StandardMaterial>>,
    mut far: ResMut<Assets<FarWoods>>,
    mut trunks: ResMut<crate::flora::Trunks>,
    parting: Option<Res<crate::parting::Parting>>,
    sited: Option<Res<crate::start::Sited>>,
) {
    let Some(round) = planet.round() else {
        return;
    };
    let began = std::time::Instant::now();
    let land = &round.land;
    let middle = sited
        .and_then(|sited| sited.woods_middle(&planet))
        .unwrap_or(start.translation);
    let woods = Woods::of(round);
    let seed = planet.seed().derive("woods");
    let near = woods.trees(middle, 0.0, NEAR_M, SPACING_M, seed);
    // The cards stand for every tree, the grown ones too, as an impostor stands for its
    // model: seen from away from the start, past the grown trees' last level of detail, they
    // are the trees. Near the camera they give way to the grown ones (`near`).
    let mid: Vec<Tree> = woods
        .trees(middle, 0.0, COARSE_M, SPACING_M, seed)
        .into_iter()
        .filter(|tree| tree.at.distance(middle) > CLEAR_M)
        .collect();
    let coarse = woods.trees(
        middle,
        COARSE_M,
        FAR_M,
        SPACING_M * 2.0,
        seed.derive("coarse"),
    );

    // Near: grown conifers, each its kind's at its stature, drawn by what the layer above
    // gave, or the flora's pine.
    let conifers: Arc<dyn ConiferModels> = match conifers {
        Some(conifers) => conifers.0.clone(),
        None => {
            let wind = middle.normalize().any_orthonormal_vector();
            let Some(pines) = flora_pines(
                &flora,
                &cards,
                &assets,
                wind,
                parting.as_deref(),
                seed.derive("grown"),
                &mut meshes,
                &mut foliage,
                &mut plain,
            ) else {
                return;
            };
            Arc::new(pines)
        }
    };
    // The start is kept clear, as the game's own start is, so one stands there in the open.
    let near: Vec<Tree> = near
        .into_iter()
        .filter(|tree| tree.at.distance(middle) > CLEAR_M)
        .collect();
    let mut grown: HashMap<[u32; 3], Drawn> = HashMap::default();
    let mut drawn_as = [[0usize; 3]; 3];
    for tree in &near {
        let planted = planted(tree);
        trunks.add(crate::flora::Trunk {
            foot: tree.at,
            radius_m: (planted.height_m * 0.012).max(0.08),
            height_m: planted.height_m,
            solid: false,
        });
        let mut entity = commands.spawn((Name::new("Woods tree"), Visibility::default()));
        let drawn = conifers.draw(&mut entity, &planted);
        drawn_as[kind_index(tree.kind)][kind_index(drawn.kind)] += 1;
        grown.insert(foot_key(tree.at), drawn);
    }

    // Far: billboards, the near crowns' colours, merged a square at a time.
    let [oak, pine, spruce] = conifers.far_crowns().map(|c| c.extend(1.0));
    let material = far.add(FarWoods {
        base: StandardMaterial {
            base_color_texture: Some(assets.load("textures/forest_spike/leaves.png")),
            alpha_mode: AlphaMode::Mask(0.5),
            perceptual_roughness: 0.85,
            reflectance: 0.2,
            diffuse_transmission: 0.3,
            ..default()
        },
        extension: FarExtension {
            crowns: [oak, pine, spruce],
            // A card standing for a grown tree comes in as that tree's last level goes, over
            // the same last fifth (its colour's blue says where, `HANDOVER_SCALE_M`); one past
            // the grown trees, nothing else standing for its tree, is gone only when walked
            // into, as the far forest's are (D273).
            near: Vec4::new(25.0, 45.0, HANDOVER_SCALE_M, 1.0 - grown::FADE),
            // The horizon's, until `see_horizon` has one.
            ..Default::default()
        },
    });
    let (east, north) = (land.frame.east(middle), land.frame.north(middle));
    let mut squares: HashMap<(i32, i32), Vec<Card>> = HashMap::default();
    for (tree, widen) in mid
        .iter()
        .map(|t| (t, 1.0))
        .chain(coarse.iter().map(|t| (t, 1.35)))
    {
        let offset = tree.at - middle;
        let at = Vec2::new(offset.dot(east), offset.dot(north));
        squares
            .entry((
                (at.x / SQUARE_M).floor() as i32,
                (at.y / SQUARE_M).floor() as i32,
            ))
            .or_default()
            .push(Card {
                tree: *tree,
                widen,
                grown: grown.get(&foot_key(tree.at)).copied(),
            });
    }
    let square_count = squares.len();
    for (_, cards) in squares {
        let (mesh, middle, aabb) = merged(&cards);
        commands.spawn((
            Name::new("Far woods"),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(middle),
            aabb,
            NoAutoAabb,
            NotShadowCaster,
        ));
    }
    say_planted(
        near.len(),
        mid.len() + coarse.len(),
        square_count,
        &drawn_as,
        began,
    );
}

/// What the woods planted, and how each kind's grown trees were drawn.
fn say_planted(
    near: usize,
    far: usize,
    squares: usize,
    drawn_as: &[[usize; 3]; 3],
    began: std::time::Instant,
) {
    let mine = |k: usize| drawn_as[k][k];
    let other: usize = (0..3)
        .map(|k| drawn_as[k].iter().sum::<usize>() - drawn_as[k][k])
        .sum();
    info!(
        "Woods: {near} grown trees within {NEAR_M:.0} m ({} oaks, {} pines and {} spruces drawn as their kind, {other} as another), {far} far ones to {FAR_M:.0} m in {squares} squares, in {:.1} s",
        mine(0),
        mine(1),
        mine(2),
        began.elapsed().as_secs_f32()
    );
}

fn kind_index(kind: TreeKind) -> usize {
    match kind {
        TreeKind::Oak => 0,
        TreeKind::Pine => 1,
        TreeKind::Spruce => 2,
    }
}

/// A far card: the tree it stands for, how much wider than its tree it is drawn, and, if its
/// tree is grown near the camera, how that was drawn.
#[derive(Clone, Copy, Debug)]
struct Card {
    tree: Tree,
    widen: f32,
    grown: Option<Drawn>,
}

/// A square's trees as one mesh of cards about its middle, in the far forest's form
/// (far_forest.wesl): each card's four corners at its foot, the shader standing them up and
/// turning them to the eye; the first texture coordinate the corner, the second the card's
/// width and height, the colour its kind (oak 0, pine 0.5, spruce 1), its luck and, for a grown
/// tree's card, where it comes in. Its bounds hold the cards turned any way.
fn merged(cards: &[Card]) -> (Mesh, Vec3, Aabb) {
    let middle = cards.iter().map(|c| c.tree.at).sum::<Vec3>() / cards.len() as f32;
    let up = middle.normalize();
    let count = cards.len() * 4;
    let (mut positions, mut normals, mut corners, mut sizes, mut colours) = (
        Vec::with_capacity(count),
        Vec::with_capacity(count),
        Vec::with_capacity(count),
        Vec::with_capacity(count),
        Vec::with_capacity(count),
    );
    let mut indices = Vec::with_capacity(cards.len() * 6);
    let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for card in cards {
        let tree = &card.tree;
        let planted = planted(tree);
        // As its grown tree was drawn, if it was.
        let kind = card.grown.map_or(tree.kind, |drawn| drawn.kind);
        let height = planted.height_m;
        let width = height
            * CROWN[kind_index(kind)]
            * planted.squat
            * card.widen
            * (0.85 + 0.3 * tree.luck);
        let at = tree.at - middle;
        let reach = Vec3::splat(width * 0.5);
        let top = up * height;
        for p in [at - reach, at + reach, at - reach + top, at + reach + top] {
            low = low.min(p);
            high = high.max(p);
        }
        let first = positions.len() as u32;
        let species = match kind {
            TreeKind::Oak => 0.0,
            TreeKind::Pine => 0.5,
            TreeKind::Spruce => 1.0,
        };
        let comes_in = card
            .grown
            .map_or(0.0, |drawn| drawn.to_m / HANDOVER_SCALE_M);
        for corner in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            positions.push(at.to_array());
            normals.push(up.to_array());
            corners.push(corner);
            sizes.push([width, height]);
            colours.push([species, tree.luck, comes_in, 1.0]);
        }
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, corners);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, sizes);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_indices(Indices::U32(indices));
    (mesh, middle, Aabb::from_min_max(low, high))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn tree(at: Vec3, kind: TreeKind, stature: f32) -> Tree {
        Tree {
            at,
            kind,
            stature,
            luck: 0.5,
            age: 1.0,
        }
    }

    #[test]
    fn a_card_standing_for_a_grown_tree_comes_in_where_it_goes_and_looks_as_it_was_drawn() {
        let ground = Vec3::new(0.0, 30_000.0, 0.0);
        let cards = [
            // A tall pine grown near, drawn as a pine out to 588 m.
            Card {
                tree: tree(ground, TreeKind::Pine, 1.0),
                widen: 1.0,
                grown: Some(Drawn {
                    to_m: 588.0,
                    kind: TreeKind::Pine,
                }),
            },
            // A stunted pine grown near, drawn as a young spruce out to 189 m.
            Card {
                tree: tree(ground + Vec3::X * 20.0, TreeKind::Pine, 0.25),
                widen: 1.0,
                grown: Some(Drawn {
                    to_m: 189.0,
                    kind: TreeKind::Spruce,
                }),
            },
            // A pine past the grown trees.
            Card {
                tree: tree(ground + Vec3::X * 900.0, TreeKind::Pine, 1.0),
                widen: 1.0,
                grown: None,
            },
        ];
        let (mesh, _, _) = merged(&cards);
        let Some(VertexAttributeValues::Float32x4(colours)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("no colours");
        };
        let Some(VertexAttributeValues::Float32x2(sizes)) = mesh.attribute(Mesh::ATTRIBUTE_UV_1)
        else {
            panic!("no sizes");
        };
        // Four corners a card, each telling the shader the same.
        let card = |k: usize| (colours[k * 4], sizes[k * 4]);
        let (tall, tall_size) = card(0);
        assert_eq!(tall[0], 0.5, "a pine's card");
        assert!((tall[2] * HANDOVER_SCALE_M - 588.0).abs() < 1e-3);
        let (stunted, stunted_size) = card(1);
        assert_eq!(stunted[0], 1.0, "drawn as a spruce, its card a spruce's");
        assert!((stunted[2] * HANDOVER_SCALE_M - 189.0).abs() < 1e-3);
        // Krummholz's card is squat as its tree: wider for its height than a tall one's.
        assert!(stunted_size[0] / stunted_size[1] > tall_size[0] / tall_size[1]);
        let (beyond, _) = card(2);
        assert_eq!(beyond[2], 0.0, "no grown tree to come in after");
    }

    #[test]
    fn a_tree_toward_its_limit_is_shorter_and_squatter() {
        let ground = Vec3::new(0.0, 30_000.0, 0.0);
        let (tall, low) = (
            planted(&tree(ground, TreeKind::Spruce, 1.0)),
            planted(&tree(ground, TreeKind::Spruce, 0.3)),
        );
        assert!(low.height_m < tall.height_m * 0.4);
        assert_eq!(tall.squat, 1.0);
        assert!(low.squat > 1.5);
        // Standing up off the ground, whatever its turn.
        assert!((tall.rotation * Vec3::Y).dot(ground.normalize()) > 0.999);
    }
}
