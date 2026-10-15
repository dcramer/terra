//! Woods by the land's rules (D313, D321, docs/demos.md §1): on a world with mountains, the
//! trees round the start stand where `terra_land::woods` says, closer and taller where the
//! growing season is warm, thinning and shrinking toward the tree line, krummholz past it,
//! none on cliffs, rock, scree or water. Near the camera each is a grown conifer
//! (`grown`, the kind `flora.ron` calls Pine) with its levels of detail; past where those
//! end, a billboard drawn by the far forest's shader (`far_forest.wesl`, D273), merged a
//! square at a time, the same trees on the same feet, so the forest carries up the far slopes
//! to its line.
//!
//! The lush forest's own far forest (`lush/far.rs`) builds the same cards; once its pine
//! stand has landed, both can share this.

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

use terra_land::woods::{Conifer, Tree, Woods};

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
/// Each kind's full height range and how wide its crown is for its height, pine then spruce,
/// as the lush forest grows its Scots pines and Norway spruces (lush/far.rs).
const HEIGHTS: [(f32, f32); 2] = [(23.0, 31.0), (18.0, 26.0)];
const CROWN: [f32; 2] = [0.42, 0.36];
/// The far crowns' colours, pine and spruce: their leaves' average (lush.rs FAR_CROWNS).
const FAR_CROWNS: [Color; 2] = [Color::srgb(0.15, 0.23, 0.12), Color::srgb(0.13, 0.21, 0.12)];

pub type FarWoods = ExtendedMaterial<StandardMaterial, FarExtension>;

/// What the far forest's shader is told. Field order must match far_forest.wesl, whose
/// kinds are oak, pine and spruce.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FarExtension {
    #[uniform(100)]
    crowns: [Vec4; 3],
    #[uniform(100)]
    near: Vec4,
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
        .add_systems(PostStartup, plant);
}

#[allow(clippy::too_many_arguments)]
fn plant(
    mut commands: Commands,
    planet: Res<Planet>,
    start: Res<Start>,
    flora: Res<Flora>,
    cards: Res<Cards>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut foliage: ResMut<Assets<grown::Foliage>>,
    mut plain: ResMut<Assets<StandardMaterial>>,
    mut far: ResMut<Assets<FarWoods>>,
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

    // Near: grown conifers, each its kind's variant at its stature.
    let up = middle.normalize();
    let wind = up.any_orthonormal_vector();
    let looks = grown::Looks::new(
        wind,
        parting.map(|p| p.0.clone()).unwrap_or_default(),
        &cards,
        &assets,
        &mut foliage,
        &mut plain,
    );
    let mut looks = looks;
    let Some(pine) = flora.kinds().iter().find(|k| k.name == "Pine") else {
        error!("woods: flora.ron has no Pine to grow");
        return;
    };
    let Some(how) = &pine.grown else {
        return;
    };
    let size = pine.size.unwrap_or((16.0, 27.0));
    let variants = grown::grow(
        how,
        "woods pine",
        seed.derive("grown"),
        pine.variants,
        size,
        &cards,
        &mut looks,
        &assets,
        &mut meshes,
        &mut foliage,
    );
    if variants.is_empty() {
        return;
    }
    // The start is kept clear, as the game's own start is, so one stands there in the open.
    let near: Vec<Tree> = near
        .into_iter()
        .filter(|tree| tree.at.distance(middle) > CLEAR_M)
        .collect();
    for tree in &near {
        let variant = &variants[(tree.luck * variants.len() as f32) as usize % variants.len()];
        let (low, high) = HEIGHTS[kind_index(tree.kind)];
        let height = (low + (high - low) * tree.luck) * tree.stature;
        let scale = height / variant.height;
        let up = tree.at.normalize();
        let rotation = Quat::from_axis_angle(up, tree.luck * std::f32::consts::TAU * 7.0)
            * Quat::from_rotation_arc(Vec3::Y, up);
        // Toward their limit trees grow squat as well as short, crowns spreading low where the
        // wind and winter's snow cut back what stands above it: krummholz is wider than tall.
        let stunted = (1.0 - tree.stature / 0.7).clamp(0.0, 1.0);
        let squat = 1.0 + 1.6 * stunted * stunted;
        let transform = Transform {
            translation: tree.at - up * 0.15,
            rotation,
            scale: Vec3::new(scale * squat, scale, scale * squat),
        };
        grown::draw(
            &mut commands.spawn((transform, Visibility::default())),
            variant,
            height,
            &looks,
        );
    }

    // Far: billboards, the near crowns' colours, merged a square at a time.
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
            crowns: [
                FAR_CROWNS[0].to_linear().to_vec4(),
                FAR_CROWNS[0].to_linear().to_vec4(),
                FAR_CROWNS[1].to_linear().to_vec4(),
            ],
            // Within the grown trees' last level of detail they stand; the cards come in
            // behind them.
            near: Vec4::new(NEAR_M * 0.75, NEAR_M * 0.9, 0.0, 0.0),
        },
    });
    let (east, north) = (land.frame.east(middle), land.frame.north(middle));
    let mut squares: HashMap<(i32, i32), Vec<(Tree, f32)>> = HashMap::default();
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
            .push((*tree, widen));
    }
    let square_count = squares.len();
    for (_, trees) in squares {
        let (mesh, middle, aabb) = merged(&trees);
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
    info!(
        "Woods: {} grown trees within {NEAR_M:.0} m, {} far ones to {FAR_M:.0} m in {square_count} squares, in {:.1} s",
        near.len(),
        mid.len() + coarse.len(),
        began.elapsed().as_secs_f32()
    );
}

fn kind_index(kind: Conifer) -> usize {
    match kind {
        Conifer::Pine => 0,
        Conifer::Spruce => 1,
    }
}

/// A square's trees as one mesh of cards about its middle, in the far forest's form
/// (far_forest.wesl): each card's four corners at its foot, the shader standing them up and
/// turning them to the eye; the first texture coordinate the corner, the second the card's
/// width and height, the colour its kind (pine 0.5, spruce 1) and its luck. Its bounds hold
/// the cards turned any way.
fn merged(trees: &[(Tree, f32)]) -> (Mesh, Vec3, Aabb) {
    let middle = trees.iter().map(|(t, _)| t.at).sum::<Vec3>() / trees.len() as f32;
    let up = middle.normalize();
    let count = trees.len() * 4;
    let (mut positions, mut normals, mut corners, mut sizes, mut colours) = (
        Vec::with_capacity(count),
        Vec::with_capacity(count),
        Vec::with_capacity(count),
        Vec::with_capacity(count),
        Vec::with_capacity(count),
    );
    let mut indices = Vec::with_capacity(trees.len() * 6);
    let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for (tree, widen) in trees {
        let kind = kind_index(tree.kind);
        let (short, tall) = HEIGHTS[kind];
        let height = (short + (tall - short) * tree.luck) * tree.stature;
        let width = height * CROWN[kind] * widen * (0.85 + 0.3 * tree.luck);
        let at = tree.at - middle;
        let reach = Vec3::splat(width * 0.5);
        let top = up * height;
        for p in [at - reach, at + reach, at - reach + top, at + reach + top] {
            low = low.min(p);
            high = high.max(p);
        }
        let first = positions.len() as u32;
        let species = match tree.kind {
            Conifer::Pine => 0.5,
            Conifer::Spruce => 1.0,
        };
        for corner in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            positions.push(at.to_array());
            normals.push(up.to_array());
            corners.push(corner);
            sizes.push([width, height]);
            colours.push([species, tree.luck, 0.0, 1.0]);
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
