//! Woods by the land's rules (D313, D321, docs/demos.md §1): on a world with mountains, the
//! trees round the start stand where `terra_land::woods` says, closer and taller where the
//! growing season is warm, thinning and shrinking toward the tree line, krummholz past it,
//! none on cliffs, rock, scree or water. Near the camera each is a grown conifer with its
//! levels of detail, streamed in round the player a block at a time (`near`, D622), drawn by
//! what `Conifers` holds (D366): the lush forest's Scots pines and
//! Norway spruces where the layer above gives them, the flora's generic pine where nothing
//! does. Past where its levels end, a billboard drawn by the far forest's shader
//! (`far_forest.wesl`, D273), merged a square at a time, the same trees on the same feet, so
//! the forest carries up the far slopes to its line; each card stands for its grown tree and
//! comes in where that tree's last level goes.

use crate::cover::forest::{Crowns, Floor, LayingFloor, Stand, Wild, WoodsFloor};
use crate::flora_kinds::Flora;
use crate::grown::{self, Cards};
use crate::instance::{Building, InWorld, InstanceApp};
use crate::planet::Planet;
use crate::start::Start;
use crate::streaming::{Class, Place, Streaming, Ticket};
use crate::trees::Species;
use bevy::platform::time::Instant;
use bevy::{
    asset::RenderAssetUsages,
    camera::{primitives::Aabb, visibility::NoAutoAabb},
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::{render_resource::AsBindGroup, storage::ShaderBuffer},
    shader::ShaderRef,
};
use std::sync::Arc;

mod felled;
mod near;
mod outline;

pub use terra_land::woods::TreeKind;
use terra_land::woods::{Tree, Woods};

/// Billboards for every tree out to the far reach, each standing for its grown tree, which is
/// streamed in round the player (`near`, D622); the far trees on a grid twice as coarse, each
/// card wider, past `COARSE_M`, with no grown tree, so none is grown past it.
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

    /// How it draws a conifer, without drawing it: what `draw` gives. Every far card is laid
    /// at the start, its grown tree streamed in round the player later (D622), so a card comes
    /// in where its grown tree's last level will end, drawn or not yet.
    fn drawn(&self, tree: &Planted) -> Drawn;

    /// The far cards' crown colours, oak, pine and spruce, linear: what the near crowns'
    /// leaves average to, as the far forest's are coloured (D273).
    fn far_crowns(&self) -> [Vec3; 3];
}

/// The conifers the woods are drawn with, if the layer above has given any; it inserts them
/// before `Planting`.
#[derive(Resource, Clone)]
pub struct Conifers(pub Arc<dyn ConiferModels>);

/// Where the woods are planted, on entering a world (`Building::View`): whatever fills in
/// `Conifers` runs before it.
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
/// card takes over, and as which kind, which its card is drawn as; and its crown and trunk as
/// drawn, which the floor under it is laid by (`cover::forest`, D619).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub to_m: f32,
    pub kind: TreeKind,
    /// Where its crown's middle is from its foot, in the world's directions.
    pub crown_off: Vec3,
    /// How far its leaves reach round that middle, and how thick its trunk is at the foot,
    /// from its middle.
    pub crown_m: f32,
    pub trunk_m: f32,
    /// A young tree's crown lets more light through.
    pub young: bool,
    /// Whether it stops a walker, filed among the `flora::Obstacles` at its transform's
    /// place, where felling it takes it away (the layer above's trees are felled as the
    /// flora's are, D619); the flora's generic pine stops nobody.
    pub solid: bool,
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
    /// The plants' shared buffer (`crate::parting`), for the world's look in its last entry:
    /// the far crowns turn round the hue circle with the near ones (`hue.wesl`).
    #[storage(103, read_only)]
    parting: Handle<ShaderBuffer>,
}

impl FarExtension {
    /// The crowns' colours, the near fade and the plants' shared buffer; the horizon's come
    /// from `see_horizon` once the land's is baked (until then `horizon_lit` reads lit, D416).
    pub fn new(crowns: [Vec4; 3], near: Vec4, parting: Handle<ShaderBuffer>) -> Self {
        Self {
            crowns,
            near,
            parting,
            ..Default::default()
        }
    }
}

/// The far forest told the land's horizon each time it is baked.
fn see_horizon(
    horizon: Res<crate::horizon::HorizonPicture>,
    mut materials: ResMut<Assets<FarWoods>>,
) {
    terra_gen::stat_scope!("flora");
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
    // Last of the cut-outs in the depth prepass, so the near crowns' depth rejects the far
    // cards they hide (D618).
    fn prepass_draw_priority(&self) -> Option<i32> {
        Some(1)
    }
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // The main pass shades only the card's fragments the depth prepass kept, as the leaf
        // cards' does (`grown::test_depth_early`), and tests for equal depth too: a card's empty
        // part over the land behind it is nearer than the land's depth and passed the usual
        // greater-or-equal test, running the whole tree shape only to be discarded; tested
        // equal, the hardware rejects it before the shader runs. The same vertex shader writes
        // both passes' depth. 0.35 ms off the lake's route view on a 4090, 0.48 under its trees,
        // no pixel changed (play-lake-fps j10).
        grown::test_depth_early(descriptor, key.mesh_key);
        let prepass = descriptor
            .label
            .as_ref()
            .is_some_and(|l| l.contains("prepass") || l.contains("shadow"));
        if !prepass
            && key
                .mesh_key
                .contains(bevy::pbr::MeshPipelineKey::DEPTH_PREPASS)
            && let Some(depth) = descriptor.depth_stencil.as_mut()
        {
            depth.depth_compare = Some(bevy::render::render_resource::CompareFunction::Equal);
        }
        Ok(())
    }
}

pub fn plugin(app: &mut App) {
    crate::streaming::plugin(app);
    // The woods and the conifers they are drawn from are the world's (D469).
    app.add_plugins(MaterialPlugin::<FarWoods>::default())
        .world_resource::<Conifers>()
        .init_world_resource::<FarCards>()
        .world_resource::<felled::FarSquares>()
        .world_resource::<near::NearWoods>()
        .add_observer(felled::file_the_felled)
        .configure_sets(
            OnEnter(InWorld),
            Planting.in_set(Building::View).before(LayingFloor),
        )
        .add_systems(OnEnter(InWorld), plant.in_set(Planting))
        .add_systems(
            Update,
            (
                see_horizon.run_if(resource_exists_and_changed::<crate::horizon::HorizonPicture>),
                (stream_cards.run_if(resource_exists::<FarCards>)).run_if(crate::streaming::lifted),
                near::stream.run_if(crate::streaming::lifted),
                felled::unfar_the_felled,
            ),
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
        self.drawn(tree)
    }

    fn drawn(&self, tree: &Planted) -> Drawn {
        let size = (tree.height_m / grown::DETAIL_FOR_M).clamp(0.45, 1.4);
        Drawn {
            to_m: grown::detail(3, 3, size).end_margin.end,
            kind: tree.kind,
            // Its crown as wide for its height as its kind's, round its stem.
            crown_off: Vec3::ZERO,
            crown_m: 0.5 * CROWN[kind_index(tree.kind)] * tree.height_m * tree.squat,
            trunk_m: (tree.height_m * 0.012).max(0.08),
            young: false,
            solid: false,
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

/// The trees the woods' cards stand for round `middle`, laid before the loading screen lifts:
/// every tree out to `COARSE_M` but the start's clearing, none in a snowbed. The coarser
/// cards past it come after ([`coarse_squares`]).
fn stood(
    planet: &Planet,
    round: &terra_land::round::Round,
    middle: Vec3,
    seed: terra_gen::Seed,
) -> Vec<Tree> {
    let began = std::time::Instant::now();
    // The cards stand for every tree, the grown ones too, as an impostor stands for its
    // model: past the grown trees' last level of detail they are the trees. Near the camera
    // they give way to the grown ones, the same trees from the same grid (`near::lay`).
    let mid = cleared(
        middle,
        Woods::of(round).trees(middle, 0.0, COARSE_M, SPACING_M, seed),
    );
    let laid = began.elapsed().as_secs_f32();
    let mid = off_snowbeds(planet, mid);
    info!(
        "Woods: laid out in {laid:.1} s, {:.1} s more off the walls' snowbeds",
        began.elapsed().as_secs_f32() - laid
    );
    mid
}

/// The trees but those in the start's clearing, kept clear as the game's own start is, so
/// one stands there in the open.
fn cleared(middle: Vec3, trees: Vec<Tree>) -> Vec<Tree> {
    trees
        .into_iter()
        .filter(|tree| tree.at.distance(middle) > CLEAR_M)
        .collect()
}

/// The coarser cards' trees from `inner_m` to `outer_m` of `middle`, none in a snowbed: one
/// ring of [`rings`], each a task of the streaming's scheduler, as one task on one thread took
/// 150 s at the lake (perf-regress j2). A tree is laid by its own cell of the ground's grid,
/// so the rings together lay the trees one pass from `COARSE_M` to `FAR_M` would.
fn coarse_trees(
    planet: &Planet,
    middle: Vec3,
    (inner_m, outer_m): (f32, f32),
    seed: terra_gen::Seed,
) -> Vec<Tree> {
    let Some(round) = planet.round() else {
        return Vec::new();
    };
    let coarse = Woods::of(round).trees(
        middle,
        inner_m,
        outer_m,
        SPACING_M * 2.0,
        seed.derive("coarse"),
    );
    off_snowbeds(planet, coarse)
}

/// The coarser cards past `COARSE_M` to `FAR_M` merged a square at a time: worked out after
/// the loading screen lifts, on the streaming pool, as they were 60% of the woods' laying.
/// With how many trees they stand for.
fn coarse_squares(planet: &Planet, middle: Vec3, coarse: Vec<Tree>) -> Squares {
    let Some(round) = planet.round() else {
        return default();
    };
    let land = &round.land;
    let (east, north) = (land.frame.east(middle), land.frame.north(middle));
    let cards = coarse.iter().map(|tree| Card {
        tree: *tree,
        widen: 1.35,
        grown: None,
    });
    let squares = squares(middle, east, north, cards);
    let merged = squares.into_values().map(|cards| merged(&cards)).collect();
    (merged, coarse.len())
}

/// Cards in the squares of `SQUARE_M` about `middle` that they are merged in.
fn squares(
    middle: Vec3,
    east: Vec3,
    north: Vec3,
    cards: impl Iterator<Item = Card>,
) -> HashMap<(i32, i32), Vec<Card>> {
    let mut squares: HashMap<(i32, i32), Vec<Card>> = HashMap::default();
    for card in cards {
        let square = felled::square_of(middle, east, north, card.tree.at);
        squares.entry(square).or_default().push(card);
    }
    squares
}

/// The woods' coarse cards, asked for as the woods are planted and streamed in once the
/// loading screen lifts ([`stream_cards`]); a scripted run is held for them (`loading::hold`).
#[derive(Resource, Default)]
pub(crate) struct FarCards {
    asked: Option<(Vec3, terra_gen::Seed, Handle<FarWoods>)>,
    making: Option<Making>,
}

/// The far cards on their way: their rings' trees being laid, then merged into squares, then
/// the squares entering the world, the nearest first, under the frame's budget (D608).
struct Making {
    middle: Vec3,
    material: Handle<FarWoods>,
    since: Instant,
    rings: Vec<Ticket<Vec<Tree>>>,
    trees: Vec<Tree>,
    merging: Option<Ticket<Squares>>,
    /// The squares made, the farthest first, and how many trees and squares in all.
    entering: Vec<(Mesh, Vec3, Aabb)>,
    made: Option<(usize, usize)>,
}

/// The far cards' squares, each its mesh, middle and bounds, and how many trees they hold.
type Squares = (Vec<(Mesh, Vec3, Aabb)>, usize);

impl FarCards {
    /// Whether they are still to come.
    pub(crate) fn coming(&self) -> usize {
        usize::from(self.asked.is_some() || self.making.is_some())
    }
}

/// Lays and merges the coarse cards through the streaming's scheduler once the loading screen
/// has lifted, a ring a task so the pool's threads share them, and lets the squares into the
/// world under the frame's budget, the nearest first.
fn stream_cards(
    commands: Commands,
    planet: Res<Planet>,
    mut streaming: ResMut<Streaming>,
    mut cards: ResMut<FarCards>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    terra_gen::stat_scope!("flora");
    if let Some((middle, seed, material)) = cards.asked.take() {
        let rings = crate::streaming::order::rings(COARSE_M, FAR_M, streaming.in_flight())
            .into_iter()
            .map(|ring| {
                let planet = planet.clone();
                let place = Place::Ring {
                    middle,
                    inner_m: ring.0,
                };
                streaming.ask(Class::FarWoods, place, move || {
                    coarse_trees(&planet, middle, ring, seed)
                })
            })
            .collect();
        cards.making = Some(Making {
            middle,
            material,
            since: Instant::now(),
            rings,
            trees: Vec::new(),
            merging: None,
            entering: Vec::new(),
            made: None,
        });
        return;
    }
    let Some(making) = cards.making.as_mut() else {
        return;
    };
    making.rings.retain(|ring| match ring.take() {
        Some(trees) => {
            making.trees.extend(trees);
            false
        }
        None => true,
    });
    if making.rings.is_empty() && making.merging.is_none() && making.made.is_none() {
        let (planet, middle) = (planet.clone(), making.middle);
        let trees = std::mem::take(&mut making.trees);
        let place = Place::Ring {
            middle,
            inner_m: COARSE_M,
        };
        let merge = move || coarse_squares(&planet, middle, trees);
        making.merging = Some(streaming.ask(Class::FarWoods, place, merge));
    }
    if let Some((mut squares, trees)) = making.merging.as_ref().and_then(Ticket::take) {
        let middle = making.middle;
        squares.sort_by(|a, b| b.1.distance(middle).total_cmp(&a.1.distance(middle)));
        making.made = Some((trees, squares.len()));
        making.entering = squares;
        making.merging = None;
    }
    while streaming.may_enter(Class::FarWoods)
        && let Some((mesh, middle, aabb)) = making.entering.pop()
    {
        let began = std::time::Instant::now();
        let bytes = crate::streaming::mesh_bytes(&mesh);
        let material = making.material.clone();
        (streaming.commands(Class::FarWoods, &commands)).spawn(far_square(
            meshes.add(mesh),
            material,
            middle,
            aabb,
        ));
        streaming.entered(Class::FarWoods, began, bytes);
    }
    let Some((trees, count)) = making.made.filter(|_| making.entering.is_empty()) else {
        return;
    };
    info!(
        "Woods: {trees} coarse cards from {COARSE_M:.0} to {FAR_M:.0} m in {count} squares, streamed in {:.1} s after the loading screen lifted",
        making.since.elapsed().as_secs_f32()
    );
    cards.making = None;
}

/// A square of far cards, as one entity.
fn far_square(
    mesh: Handle<Mesh>,
    material: Handle<FarWoods>,
    middle: Vec3,
    aabb: Aabb,
) -> impl Bundle {
    (
        Name::new("Far woods"),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(middle),
        aabb,
        NoAutoAabb,
        NotShadowCaster,
    )
}

/// The trees off the snowbeds a range's walls feed their benches and scree
/// (`terrain::snowbed_m`), each over a snowbed's edge kept by its own luck, so the same tree
/// goes from the grown ones and the cards alike.
fn off_snowbeds(planet: &Planet, trees: Vec<Tree>) -> Vec<Tree> {
    let Some(round) = planet.round() else {
        return trees;
    };
    let kept = terra_gen::par::map(&trees, |tree| {
        // On soil the snow is the climate's alone, which the tree's stand already took
        // (`Woods::stand_at`); only rock and scree take their walls' feed, and asking the ground
        // what it is made of again was most of this pass's 18 s at the lake (mountains round 32).
        if tree.footing == terra_land::woods::Footing::Soil {
            return true;
        }
        let normal = terra_land::woods::ground_normal(&round.land, tree.at);
        let snowbed = crate::terrain::snowbed_m(planet, round, tree.at, normal);
        let open = terra_land::woods::Stand {
            density: 1.0,
            stature: 1.0,
        };
        let density = open.under_snow(snowbed).density;
        density > 0.0 && tree.luck <= density
    });
    trees
        .into_iter()
        .zip(kept)
        .filter_map(|(tree, kept)| kept.then_some(tree))
        .collect()
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
    mut far_cards: ResMut<FarCards>,
) {
    let Some(round) = planet.round() else {
        return;
    };
    let began = std::time::Instant::now();
    let land = &round.land;
    let middle = sited
        .and_then(|sited| sited.woods_middle(&planet))
        .unwrap_or(start.translation);
    let seed = planet.seed().derive("woods");
    let mid = stood(&planet, round, middle, seed);

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
    // Near: the first ring of the grown trees' blocks, round the start and the woods' middle
    // both, before the loading screen lifts; the rest stream in round the player (D622).
    let (east, north) = (land.frame.east(middle), land.frame.north(middle));
    let map = near::Map::new(middle, east, north, land.radius_m());
    let mut blocks: Vec<near::Block> = near::within(map.at(start.translation), near::IN_M)
        .chain(near::within(Vec2::ZERO, near::IN_M))
        .collect();
    blocks.sort_unstable();
    blocks.dedup();
    let mut streamed = near::NearWoods::new(map, seed, *start, conifers.clone());
    let mut first: Vec<Tree> = Vec::new();
    let mut drawn_as = [[0usize; 3]; 3];
    // Their floor, by each tree as drawn (D619).
    let mut canopy = Crowns::default();
    for &block in &blocks {
        let trees = near::lay(&planet, &map, block, seed);
        let mut standing = Vec::with_capacity(trees.len());
        let mut feet = Vec::with_capacity(trees.len());
        for tree in &trees {
            let (stood, drawn) = near::grow(&mut commands, &*conifers, &mut trunks, tree);
            standing.push(stood);
            drawn_as[kind_index(tree.kind)][kind_index(drawn.kind)] += 1;
            let stand = stand(&start, tree.at, &drawn);
            feet.push(stand.at);
            canopy.add(stand);
        }
        streamed.laid(block, standing, feet);
        first.extend(trees);
    }
    commands.insert_resource(streamed);
    // For a scripted run to walk up to (as the forest walk says its nearest, D309).
    let from_start = |tree: &&Tree| tree.at.distance(start.translation);
    if let Some(tree) = first
        .iter()
        .min_by(|a, b| from_start(a).total_cmp(&from_start(b)))
    {
        let local = start.local(tree.at);
        info!(
            "Woods: the nearest grown tree to the start stands {:.1} m east and {:.1} m south, {:.1} m tall",
            local.x,
            local.z,
            planted(tree).height_m
        );
    }
    commands.insert_resource(WoodsFloor(Some(Floor::new(
        Wild,
        canopy,
        seed.derive("floor"),
    ))));

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
        // A card standing for a grown tree comes in as that tree's last level goes, over the
        // same last fifth (its colour's blue says where, `HANDOVER_SCALE_M`); one past the
        // grown trees, nothing else standing for its tree, is gone only when walked into, as
        // the far forest's are (D273). The horizon's, until `see_horizon` has one.
        extension: FarExtension::new(
            [oak, pine, spruce],
            Vec4::new(25.0, 45.0, HANDOVER_SCALE_M, 1.0 - grown::FADE),
            parting.map(|parting| parting.0.clone()).unwrap_or_default(),
        ),
    });
    // Every card here stands for a grown tree, streamed in or still to be (D622), and comes
    // in where that tree's last level ends.
    let cards = mid.iter().map(|tree| Card {
        tree: *tree,
        widen: 1.0,
        grown: Some(conifers.drawn(&planted(tree))),
    });
    let squares = squares(middle, east, north, cards);
    let square_count = squares.len();
    // A square holding a grown tree's card keeps its cards, to be merged again without it
    // when the tree is felled (`felled`).
    let mut holding_grown = felled::FarSquares::new(middle, east, north);
    for (square, cards) in squares {
        let (mesh, middle, aabb) = merged(&cards);
        let mut entity =
            commands.spawn(far_square(meshes.add(mesh), material.clone(), middle, aabb));
        if cards.iter().any(|card| card.grown.is_some()) {
            entity.insert(felled::FarSquare { cards });
            holding_grown.by_square.insert(square, entity.id());
        }
    }
    commands.insert_resource(holding_grown);
    far_cards.asked = Some((middle, seed, material));
    let first = (first.len(), blocks.len());
    say_planted(first, mid.len(), square_count, &drawn_as, began);
}

/// What the woods planted, and how each kind's grown trees were drawn.
fn say_planted(
    (near, blocks): (usize, usize),
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
        "Woods: {near} grown trees in the {blocks} blocks within {:.0} m of the start ({} oaks, {} pines and {} spruces drawn as their kind, {other} as another), {far} far ones to {COARSE_M:.0} m in {squares} squares, in {:.1} s; the coarser to {FAR_M:.0} m after the loading screen",
        near::IN_M,
        mine(0),
        mine(1),
        mine(2),
        began.elapsed().as_secs_f32()
    );
}

/// A tree as its floor sees it, on the start's map (east and south of it, as the ground cover
/// asks): its foot, and its crown and trunk as drawn.
pub(super) fn stand(start: &Start, foot: Vec3, drawn: &Drawn) -> Stand {
    let on_map = |point: Vec3| {
        let local = start.local(point);
        Vec2::new(local.x, local.z)
    };
    let species = match drawn.kind {
        TreeKind::Oak => Species::Oak,
        TreeKind::Pine => Species::Pine,
        TreeKind::Spruce => Species::Spruce,
    };
    Stand::of(
        on_map(foot),
        on_map(foot + drawn.crown_off),
        drawn.crown_m,
        species,
        drawn.young,
        drawn.trunk_m,
    )
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

/// A square's trees as one mesh of far cards ([`far_mesh`]).
fn merged(cards: &[Card]) -> (Mesh, Vec3, Aabb) {
    let cards: Vec<FarCard> = cards
        .iter()
        .map(|card| {
            let tree = &card.tree;
            let planted = planted(tree);
            // As its grown tree was drawn, if it was.
            let kind = kind_index(card.grown.map_or(tree.kind, |drawn| drawn.kind));
            let height = planted.height_m;
            FarCard {
                foot: tree.at,
                width: height * CROWN[kind] * planted.squat * card.widen * (0.85 + 0.3 * tree.luck),
                height,
                kind,
                luck: tree.luck,
                comes_in: card
                    .grown
                    .map_or(0.0, |drawn| drawn.to_m / HANDOVER_SCALE_M),
            }
        })
        .collect();
    far_mesh(&cards)
}

/// One far tree's card: where its foot stands, how wide and high it is drawn, its kind (oak
/// 0, pine 1, spruce 2), its luck (0 to 1, the shader's own variation), and for a grown
/// tree's card where it comes in (`FarExtension`'s handover scale, 0 for none).
#[derive(Clone, Copy, Debug)]
pub struct FarCard {
    pub foot: Vec3,
    pub width: f32,
    pub height: f32,
    pub kind: usize,
    pub luck: f32,
    pub comes_in: f32,
}

/// Cards as one mesh about their middle, in the far forest's form (far_forest.wesl): each
/// card a quad for each band of its tree's outline ([`outline`]), the corners at its foot,
/// the shader standing them up and turning them to the eye; the first texture coordinate the
/// corner on the whole card, the second the card's width and height, the colour its kind
/// (oak 0, pine 0.5, spruce 1), its luck, where it comes in, and which way the corner is
/// widened ([`outward`]). Its bounds hold the cards turned any way. The world's woods and the
/// lush experiment's far forest both draw by it (D396).
pub fn far_mesh(cards: &[FarCard]) -> (Mesh, Vec3, Aabb) {
    let middle = cards.iter().map(|c| c.foot).sum::<Vec3>() / cards.len() as f32;
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
        let (width, height) = (card.width, card.height);
        let at = card.foot - middle;
        let reach = Vec3::splat(width * 0.5);
        let top = up * height;
        for p in [at - reach, at + reach, at - reach + top, at + reach + top] {
            low = low.min(p);
            high = high.max(p);
        }
        let species = card.kind as f32 * 0.5;
        // The card cut to its tree's outline, a quad a band; each corner says in its colour's
        // alpha which way the shader widens it (`outward`).
        let bands = outline::outline(card.kind, width, height);
        for (i, band) in bands.iter().enumerate() {
            let first = positions.len() as u32;
            let below = (i == 0 || bands[i - 1].half < band.half) as i32;
            let above = (i + 1 == bands.len() || bands[i + 1].half < band.half) as i32;
            let (left, right) = (0.5 - band.half / width, 0.5 + band.half / width);
            let (low, high) = (band.low / height, band.high / height);
            for (corner, out) in [
                ([left, low], [-1, -below]),
                ([right, low], [1, -below]),
                ([right, high], [1, above]),
                ([left, high], [-1, above]),
            ] {
                positions.push(at.to_array());
                normals.push(up.to_array());
                corners.push(corner);
                sizes.push([width, height]);
                colours.push([species, card.luck, card.comes_in, outward(out)]);
            }
            indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
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

/// Which way a band's corner is widened, across and up, each -1, 0 or 1, as one number for
/// the colour's alpha, 2 to 10: far_forest.wesl's vertex shader reads it back (`outward`).
/// An alpha of 1 is a whole card, not widened.
fn outward([across, up]: [i32; 2]) -> f32 {
    (2 + (across + 1) * 3 + up + 1) as f32
}

#[cfg(test)]
mod tests;
