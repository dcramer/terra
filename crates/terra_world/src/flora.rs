//! What grows and lies around the scene: trees, rocks, bushes and flowers. The grass is the
//! ground cover's (`cover`), on every world.
//!
//! Trees, bushes, herbs and flowers are grown by rules from bark and leaf cards
//! (`grown`, D222), a few variants a kind. Rocks and the like are models (scans and free
//! ones, see THIRD_PARTY.md), read for their shapes only and drawn in real materials,
//! rock and bark, tinted per kind (D212, flora.wesl). Everything moves in one wind. Where
//! each thing stands comes from the seed and the place, so it is the same on every run.
//! Each plant or rock is one entity, sharing its meshes with the rest of its variant; a
//! tree's levels of detail are its children.

use super::flora_kinds::Cuttable;
use super::flora_kinds::{Cut, Flora, Grows, MATERIALS, Season};
use super::footprint::Footprint;
use super::grown;
use model::{Drawn, Model};

mod kept;
pub use kept::KeptClear;
pub(crate) use kept::clear_grown;
mod entering;
mod fell;
mod model;
mod piece;
mod stock;
pub mod trees;
pub use stock::Stock;
mod under;
use super::scene::SUN_DIRECTION;
use crate::planet::Planet;
use crate::spatial::SpatialHash;
use crate::start::Start;
use bevy::{
    ecs::{change_detection::Tick, system::SystemChangeTick},
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
pub use piece::FloraPiece;
use terra_core::{Piece, Solid, Walker};
use terra_gen::smoothstep;
use terra_gen::{Noise, Rng, Seed};

pub(crate) type WindMaterial = ExtendedMaterial<StandardMaterial, WindExtension>;

/// What the wind shader is told, and the real materials parts are drawn in (bark,
/// leaves, rock; `flora_kinds::MATERIALS`). Field order must match the structs in
/// wind.wesl and flora.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WindExtension {
    #[uniform(100)]
    direction: Vec3,
    #[uniform(100)]
    strength: f32,
    /// How many metres each layer of the materials covers, in order.
    #[uniform(100)]
    layer_metres: [Vec4; 2],
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    color: Option<Handle<Image>>,
    #[texture(103, dimension = "2d_array")]
    normal: Option<Handle<Image>>,
}

impl MaterialExtension for WindExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/wind.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/flora.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/wind.wesl".into()
    }

    // Shaded only where in front, after the prepass (`grown::test_depth_early`).
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        crate::grown::test_depth_early(descriptor, key.mesh_key);
        Ok(())
    }
}

/// Loads `materials/flora`, the real materials parts of plants and rocks are drawn in,
/// as texture arrays.
fn flora_textures(assets: &AssetServer) -> (Handle<Image>, Handle<Image>) {
    use bevy::image::{ImageArrayLayout, ImageLoaderSettings};
    let layers = |linear: bool| {
        move |settings: &mut ImageLoaderSettings| {
            settings.array_layout = Some(ImageArrayLayout::RowCount {
                rows: MATERIALS.len() as u32,
            });
            settings.is_srgb = !linear;
        }
    };
    let cook = |kind| terra_core::cooked::Cook {
        kind,
        layers: MATERIALS.len() as u32,
        mips: true,
    };
    (
        crate::cooked::picture(
            assets,
            "materials/flora/color.jpg",
            cook(terra_core::cooked::Kind::Colour),
            layers(false),
        ),
        crate::cooked::picture(
            assets,
            "materials/flora/normal.jpg",
            cook(terra_core::cooked::Kind::Normal),
            layers(true),
        ),
    )
}

/// The flora material's extension, blowing towards `toward`, with the real materials.
fn wind_extension(assets: &AssetServer, toward: Vec3) -> WindExtension {
    let (color, normal) = flora_textures(assets);
    WindExtension {
        direction: toward,
        strength: 1.0,
        layer_metres: layer_metres(),
        color: Some(color),
        normal: Some(normal),
    }
}

/// The flora material's base: matte, as bark, leaves and rock are.
fn matte(cull_mode: Option<bevy::render::render_resource::Face>) -> StandardMaterial {
    StandardMaterial {
        perceptual_roughness: 0.9,
        reflectance: 0.18,
        cull_mode,
        ..default()
    }
}

/// How many metres each of the materials covers, eight to a uniform.
fn layer_metres() -> [Vec4; 2] {
    let mut metres = [1.0; 8];
    for (slot, (_, m)) in metres.iter_mut().zip(MATERIALS) {
        *slot = m;
    }
    [
        Vec4::from_slice(&metres[..4]),
        Vec4::from_slice(&metres[4..]),
    ]
}

/// How far below a swept sphere the foot of what it may touch is looked for.
const SWEPT_BELOW_M: f32 = 6.0;

/// Trees, rocks, stumps and logs that stop a walker or hold one up, each fitted to its model
/// (`terra_core::Solid`, D267), filed by place so a walker asks only about those near it.
#[derive(Resource)]
pub struct Obstacles {
    solid: SpatialHash<Solid>,
    widest_m: f32,
}

impl Default for Obstacles {
    fn default() -> Self {
        Self {
            solid: SpatialHash::default(),
            widest_m: 0.0,
        }
    }
}

impl Obstacles {
    /// A round thing standing at a point on the ground, taller than anyone steps, such as a
    /// trunk.
    pub fn add(&mut self, at: Vec3, radius: f32) {
        self.add_solid(
            at,
            Solid::of_pieces(at, vec![Piece::round(Vec2::ZERO, radius, f32::INFINITY)]),
        );
    }

    /// A thing fitted to its model, filed at a point near it that `remove` takes it away by,
    /// such as the foot of the tree it is.
    pub fn add_solid(&mut self, at: Vec3, solid: Solid) {
        self.widest_m = self.widest_m.max(solid.reach + solid.at.distance(at));
        self.solid.insert(at, solid);
    }

    /// Takes away what stands at a point, as when a tree is felled.
    pub fn remove(&mut self, at: Vec3) {
        self.solid.remove(at, |_| true);
    }

    /// The pieces too tall to step onto that a body reaching so far from a point might
    /// touch: the middle of each and how far round it reaches.
    pub fn near(&self, at: Vec3, reach: f32) -> impl Iterator<Item = (Vec3, f32)> + '_ {
        let step = Walker::default().step_up;
        self.solids_near(at, reach).flat_map(move |solid| {
            solid
                .pieces
                .iter()
                .filter(move |piece| piece.top > step)
                .map(|piece| {
                    let (middle, radius) = piece.bounds();
                    (solid.at + solid.world(middle), radius)
                })
        })
    }

    /// Whether something is filed within `within` of a point, as a solid trunk is at its foot
    /// until it is felled. Asks only the cubes round the point, not `solids_near`'s reach
    /// grown by the widest thing filed (a town's fence line is one thing tens of metres
    /// long), which every solid trunk near a creature asked once a step.
    pub fn stands_at(&self, at: Vec3, within: f32) -> bool {
        self.solid
            .near(at, within)
            .any(|(filed, _)| filed.distance(at) < within)
    }

    /// The things a body reaching so far from a point might touch.
    pub fn solids_near(&self, at: Vec3, reach: f32) -> impl Iterator<Item = &Solid> + '_ {
        self.solid
            .near(at, reach + self.widest_m)
            .map(|(_, solid)| solid)
    }

    /// Moves a round body standing at a point out of every piece it cannot step onto, along
    /// the ground. Only what stands within three metres of its height counts.
    pub fn push_out(&self, at: Vec3, radius: f32, step: f32) -> Vec3 {
        let mut at = at;
        for solid in self.solids_near(at, radius) {
            if solid.plan(at).1.abs() < 3.0 {
                at = solid.push_out(at, radius, step);
            }
        }
        at
    }

    /// How far a sphere of `radius` moves from `from` along `direction` (of length one),
    /// within `length`, before it touches a trunk, rock, stump or log: a camera boom's sweep.
    /// None if nothing is in the way.
    pub fn sweep(&self, from: Vec3, direction: Vec3, length: f32, radius: f32) -> Option<f32> {
        self.sweep_where(from, direction, length, radius, |_| true)
    }

    /// `sweep`, against only the things `keep` says, as a camera's boom ignores what is
    /// thin (Unreal's collision channels).
    pub fn sweep_where(
        &self,
        from: Vec3,
        direction: Vec3,
        length: f32,
        radius: f32,
        keep: impl Fn(&Solid) -> bool,
    ) -> Option<f32> {
        let middle = from + direction * (length * 0.5);
        // Each is filed at its foot, which may be some way below a boom high over it.
        self.solids_near(middle, length * 0.5 + radius + SWEPT_BELOW_M)
            .filter(|solid| keep(solid))
            .filter_map(|solid| solid.sweep(from, direction, length, radius))
            .reduce(f32::min)
    }

    /// How far a point is from the nearest trunk, rock, stump or log, at least, looking no
    /// further than `reach`: `reach` when nothing is nearer.
    pub fn clearance(&self, at: Vec3, reach: f32) -> f32 {
        self.clearance_where(at, reach, |_| true)
    }

    /// `clearance`, from only the things `keep` says.
    pub fn clearance_where(&self, at: Vec3, reach: f32, keep: impl Fn(&Solid) -> bool) -> f32 {
        self.solids_near(at, reach + SWEPT_BELOW_M)
            .filter(|solid| keep(solid))
            .map(|solid| solid.clearance(at))
            .fold(reach, f32::min)
    }

    pub fn len(&self) -> usize {
        self.solid.len()
    }

    pub fn is_empty(&self) -> bool {
        self.solid.is_empty()
    }
}

/// A standing tree's trunk: where its foot is, how thick and how tall it is, and whether it
/// stands among the [`Obstacles`] too, where a felled one is no longer found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trunk {
    pub foot: Vec3,
    pub radius_m: f32,
    pub height_m: f32,
    pub solid: bool,
}

/// Every standing tree near the start, filed by place, whatever planted it: the world's
/// flora, the land's woods, a forest of the sandbox's. What lives in trees asks it: how wooded
/// a place is, and a trunk to climb (creatures).
#[derive(Resource)]
pub struct Trunks(SpatialHash<Trunk>);

impl Default for Trunks {
    fn default() -> Self {
        Self(SpatialHash::new(16.0))
    }
}

impl Trunks {
    pub fn add(&mut self, trunk: Trunk) {
        self.0.insert(trunk.foot, trunk);
    }

    /// Takes away the tree whose foot is exactly here, as when the woods' block it stands in
    /// streams out (D622).
    pub fn remove(&mut self, foot: Vec3) {
        self.0.remove(foot, |_| true);
    }

    /// The trees standing within `reach` of a point, across the ground, the felled left out.
    pub fn near<'a>(
        &'a self,
        at: Vec3,
        reach: f32,
        obstacles: &'a Obstacles,
    ) -> impl Iterator<Item = Trunk> + 'a {
        let up = at.normalize_or(Vec3::Y);
        self.0
            .near(at, reach)
            .map(|(_, trunk)| *trunk)
            .filter(move |trunk| (trunk.foot - at).reject_from(up).length() <= reach)
            .filter(move |trunk| !trunk.solid || obstacles.stands_at(trunk.foot, 0.5))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A plant or rock, and how close a base may come before it is cleared away.
#[derive(Component)]
pub struct Growth {
    clearance_m: f32,
    /// What cutting it takes and gives, if it can be cut.
    pub cut: Option<Cuttable>,
    /// Where it stops a walker, if it does.
    pub obstacle: Option<Vec3>,
    /// How far up from its foot to aim for it, in metres.
    pub middle_m: f32,
    /// Whether it grows low and soft, a flower, herb or fern, in no one's way: what a meadow
    /// mown out of the forest keeps ([`MowAround`]).
    low: bool,
}

impl Growth {
    /// Something grown outside this module, such as by a sandbox experiment: cut as `cut`
    /// says, in a walker's way at `obstacle` if anywhere, aimed at `middle_m` up from its
    /// foot. A base clears it as it would a tree.
    pub fn new(cut: Option<Cuttable>, obstacle: Option<Vec3>, middle_m: f32) -> Self {
        Self {
            clearance_m: 5.0,
            cut,
            obstacle,
            middle_m,
            low: false,
        }
    }
}

/// What the plants were last cleared for what is built: there once they are planted.
#[derive(Resource)]
struct Cleared {
    /// When they were last cleared.
    cleared_at: Tick,
    /// Each footprint as it was last cleared for, so a change clears for only what it
    /// changed.
    seen: HashMap<Entity, Footprint>,
}

/// A flat map laid over the ground around the scene: east and north of its middle, in
/// metres.
#[derive(Clone, Copy)]
struct Patch {
    middle: Vec3,
    east: Vec3,
    north: Vec3,
}

impl Patch {
    /// The point on the ground, as drawn, at a place on the map.
    fn ground(&self, planet: &Planet, east_m: f32, north_m: f32) -> Vec3 {
        planet.ground_at(self.middle + self.east * east_m + self.north * north_m)
    }
}

/// A mountain lake's shore stones lie this far from the start, as far as its grown trees.
const SHORE_STONES_M: f32 = 650.0;
/// A lake's shore is a mountain lake's, strewn with stones, from the first height over the
/// sea, fully by the second, in metres.
const MOUNTAIN_SHORE_M: (f32, f32) = (250.0, 500.0);
/// A stone stands so much of its size over where its foot is put, sunk by a third and leaning.
const STONE_STANDS: f32 = 0.7;
/// A stone's top is never left between so far under the water and so far over it: just under,
/// the water's edge fades it away over the stone (lake.wesl, as at a shore) and foams round
/// it, a glassy lens on the lake (the review of 2026-10-16). It is raised to break the surface
/// as a stone awash does, or let down to lie clear under it.
const AWASH_M: (f32, f32) = (-0.18, 0.06);
const BREAKS_M: f32 = 0.1;
const UNDER_M: f32 = -0.25;

/// A shore stone's size, standing on ground so far over the water (under it, below nothing),
/// that keeps its top out of the band just round the surface; none if it would be too small.
fn clear_of_the_surface(over: f32, size: f32) -> Option<f32> {
    let top = over + size * STONE_STANDS;
    if !(AWASH_M.0..AWASH_M.1).contains(&top) {
        return Some(size);
    }
    let raised = (BREAKS_M - over) / STONE_STANDS;
    let lowered = (UNDER_M - over) / STONE_STANDS;
    if top > -0.1 && raised <= size * 1.8 {
        Some(raised)
    } else {
        (lowered >= 0.15).then_some(lowered)
    }
}

/// Asks for the plants within so many metres of a place to be cleared, for a village, a gate
/// or a hollow set down among them: those growing there now, and whatever grows there later
/// ([`KeptClear`]), as a forest's trees stream in round a player after a thing is set down.
#[derive(Message, Clone, Copy)]
pub struct ClearAround(pub Vec3, pub f32);

/// Asks for the plants within so many metres of a place to be cleared but what grows low and
/// soft, flowers, herbs and ferns: a meadow cut out of the forest for a settlement, its trees,
/// bushes, logs and stones taken away and its flowers left (D381).
#[derive(Message, Clone, Copy)]
pub struct MowAround(pub Vec3, pub f32);

/// Says a growth was cleared away, hidden for good with its obstacle taken out, by what was
/// built, set down, mown or dug where it stood; what was drawn for it elsewhere goes with it
/// (a woods tree's far card, `woods::felled`).
#[derive(Message, Clone, Copy, Debug)]
pub struct ClearedAway(pub Entity);

/// Clears a growth away: hidden, its obstacle out of the walkers' way, and said.
fn clear_away(
    entity: Entity,
    growth: &Growth,
    mut visibility: Mut<Visibility>,
    obstacles: &mut Obstacles,
    gone: &mut MessageWriter<ClearedAway>,
) {
    visibility.set_if_neq(Visibility::Hidden);
    if let Some(stood) = growth.obstacle {
        obstacles.remove(stood);
    }
    gone.write(ClearedAway(entity));
}

/// What grows lower than this, in no one's way, a mown meadow keeps.
const LOW_M: f32 = 0.9;

/// When what grows is planted, and when it is cleared away for what is built or dug. Whatever
/// keeps a [`Footprint`] up to date does it before these.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FloraSystems {
    Plant,
    Clear,
}

/// Where the layer above lays the woods' own undergrowth on their floor (D619): among the
/// trees (`Crowns::among`) within `reach_m` of `middle`, metres east and south of the start.
/// The flora sows none of its bushes, plants, flowers and understory there, which the floor's
/// hazel, bracken, ferns, herbs and bilberry stand for. Inserted before `FloraSystems::Plant`.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Undergrown {
    pub middle: Vec2,
    pub reach_m: f32,
}

impl Undergrown {
    /// Whether the woods' floor lays its undergrowth at a place (start-local, east and south).
    pub fn holds(&self, canopy: &crate::cover::forest::Crowns, at: Vec2) -> bool {
        at.distance(self.middle) <= self.reach_m && canopy.among(at)
    }
}

pub fn plugin(app: &mut App) {
    fell::plugin(app);
    entering::plugin(app);
    app.add_plugins(MaterialPlugin::<WindMaterial>::default())
        // What grows is the world's: none drawn or cleared while it is generated (D623).
        .configure_sets(
            Update,
            FloraSystems::Clear.run_if(crate::instance::in_world),
        )
        .add_systems(
            Update,
            (follow_wind, piece::draw.run_if(crate::instance::in_world)),
        )
        .add_message::<ClearAround>()
        .add_message::<MowAround>()
        .add_message::<ClearedAway>()
        .add_systems(
            Update,
            (
                make_way,
                clear_new,
                clear_around,
                clear_grown,
                mow_around,
                follow_digs,
            )
                .chain()
                .in_set(FloraSystems::Clear),
        );
}

/// The flora sways with the world's wind (D255), from the climate.
fn follow_wind(
    wind: Option<Res<crate::wind::WorldWind>>,
    mut materials: ResMut<Assets<WindMaterial>>,
    mut own: Local<crate::wind::OwnSway<WindMaterial>>,
) {
    terra_gen::stat_scope!("flora");
    let Some(wind) = wind.filter(|wind| wind.is_changed()) else {
        return;
    };
    for (id, material) in materials.iter_mut() {
        let sway = own.of(id, material.extension.strength);
        material.extension.direction = wind.toward;
        material.extension.strength = sway * wind.strength;
    }
}

/// How likely a tree stands at a point so far from the start on a world without mountains:
/// woods with clearings, thinning to nothing round the start.
fn wooded(woods: &Noise, point: Vec3, from_start_m: f32) -> f32 {
    let wooded = smoothstep(-0.12, 0.2, woods.fbm(point / 120.0, 3, 2.0, 0.5));
    (0.12 + 0.8 * wooded) * smoothstep(16.0, 42.0, from_start_m)
}

/// What a scatter's square puts: a kind's model by its place in the kind's models, where,
/// how much room it keeps, what cutting it gives and what it stops (`plant`'s `put`).
pub(super) struct Placing {
    kind: usize,
    model: usize,
    transform: Transform,
    room: f32,
    cut: Option<Cuttable>,
    stops: Option<(Vec3, f32)>,
}

/// Asks `decide` once for each square of a grid laid over the map, with a random point in
/// that square, what it puts there. The same seed always gives the same points. Every square
/// is decided at once over every core (`terra_gen::par::map`, a row of squares a task), as
/// each has its own random numbers; what they put comes back row by row, square by square,
/// so it is put, and what stops what is filed, as one square after the next had it. Decided
/// one after the next, the mountain lake's shore stones, shore and forest floor held the
/// frame the world is entered in for 2.5 s (crossing-freeze j3).
fn scatter(
    seed: Seed,
    reach_m: f32,
    spacing_m: f32,
    decide: impl Fn(&mut Rng, f32, f32, &mut Vec<Placing>) + Sync,
) -> Vec<Placing> {
    let cells = (reach_m / spacing_m).ceil() as i32;
    let rows: Vec<i32> = (-cells..cells).collect();
    terra_gen::par::map(&rows, |&i| {
        let mut row = Vec::new();
        for j in -cells..cells {
            let mut rng = seed.at(i, 0, j).rng();
            let east = (i as f32 + rng.unit()) * spacing_m;
            let north = (j as f32 + rng.unit()) * spacing_m;
            decide(&mut rng, east, north, &mut row);
        }
        row
    })
    .into_iter()
    .flatten()
    .collect()
}

fn plant(
    mut commands: Commands,
    planet: Res<Planet>,
    footprints: Query<(Entity, &Footprint)>,
    ticks: SystemChangeTick,
    (start, sited, fields, clock): (
        Res<Start>,
        Option<Res<crate::start::Sited>>,
        Option<Res<crate::climate::ClimateFields>>,
        Option<Res<crate::daylight::Clock>>,
    ),
    flora: Res<Flora>,
    (mut obstacles, mut trunks): (ResMut<Obstacles>, ResMut<Trunks>),
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindMaterial>>,
    mut foliage: ResMut<Assets<grown::Foliage>>,
    mut plain: ResMut<Assets<StandardMaterial>>,
    cards: Res<grown::Cards>,
    assets: Res<AssetServer>,
    parting: Option<Res<crate::parting::Parting>>,
    (undergrown, woods_floor): (
        Option<Res<Undergrown>>,
        Option<Res<crate::cover::forest::WoodsFloor>>,
    ),
) {
    let middle = start.translation;
    let up = middle.normalize();
    let east = up.cross(Vec3::X).normalize();
    let ground = Patch {
        middle,
        east,
        north: east.cross(up),
    };
    let seed = planet.seed().derive("flora");
    // What dies back is not there in its season (D419), as the start's climate has it.
    let season = match (fields, clock) {
        (Some(fields), Some(clock)) => Season::at(&fields, middle, clock.time()),
        _ => Season::Unknown,
    };

    // The wind blows in off the water, which is where the sun is.
    let wind = wind_extension(&assets, -SUN_DIRECTION.reject_from(up).normalize());
    let solid = materials.add(ExtendedMaterial {
        base: matte(Some(bevy::render::render_resource::Face::Back)),
        extension: wind,
    });

    // Every kind's models, or its grown variants, by its place in `assets/data/flora.ron`.
    let mut looks = grown::Looks::new(
        -SUN_DIRECTION.reject_from(up).normalize(),
        parting.map(|parting| parting.0.clone()).unwrap_or_default(),
        &cards,
        &assets,
        &mut foliage,
        &mut plain,
    );
    let models = model::of_kinds(
        &flora,
        seed,
        &cards,
        &mut looks,
        &assets,
        (&mut meshes, &mut foliage),
    );
    // A kind another layer lays and draws (`Grows::Laid`) has none of its own.
    let drawn_here = |kind: usize| !matches!(flora.kinds()[kind].grows, Grows::Laid);
    if (0..models.len()).any(|kind| drawn_here(kind) && models[kind].is_empty()) {
        error!("Some models are missing, so nothing is planted. Is assets/models/nature there?");
        return;
    }
    let size = |rng: &mut Rng, kind: usize| {
        let (least, most) = flora.kinds()[kind].size.unwrap_or((1.0, 1.0));
        rng.range(least, most)
    };

    let to_base = start.compute_affine().inverse();
    // Where the woods' floor lays its own undergrowth, none of the old is sown.
    let floored = |point: Vec3| match (&undergrown, &woods_floor) {
        (Some(undergrown), Some(floor)) => floor.0.as_ref().is_some_and(|floor| {
            let local = to_base.transform_point3(point);
            undergrown.holds(&floor.canopy, Vec2::new(local.x, local.z))
        }),
        _ => false,
    };
    let woods = Noise::new(seed.derive("woods"));
    let blooms = Noise::new(seed.derive("blooms"));
    let pick = |rng: &mut Rng, kind: usize| {
        let model = rng.below(models[kind].len() as u32) as usize;
        (model, &models[kind][model])
    };
    let standing = trees::standing;
    let mut planted = 0;
    let mut solid_things: Vec<(Vec3, f32)> = Vec::new();
    let mut put = |model: &Model,
                   transform: Transform,
                   clearance_m: f32,
                   cut: Option<Cuttable>,
                   stops: Option<(Vec3, f32)>| {
        // Nothing in a footprint, nor a plant out of a standing stone (lake round 31).
        let at = transform.translation;
        if footprints.iter().any(|(_, f)| f.covers(at, clearance_m))
            || stops.is_none() && under::in_a_stone(&solid_things, at)
        {
            return;
        }
        planted += 1;
        solid_things.extend(stops);
        // A herb, fern or flower: cut as a plant, low and in no one's way.
        let low = stops.is_none()
            && model.height * transform.scale.y < LOW_M
            && cut
                .as_ref()
                .is_none_or(|cut| matches!(cut.what, Cut::Plant));
        let growth = Growth {
            clearance_m,
            cut,
            obstacle: stops.map(|(at, _)| at),
            middle_m: (model.height * transform.scale.y * 0.4).min(1.4),
            low,
        };
        match &model.look {
            Drawn::Mesh(mesh) => {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(solid.clone()),
                    transform,
                    growth,
                ));
            }
            Drawn::Grown(variant) => {
                let tall = model.height * transform.scale.y;
                grown::draw(
                    &mut commands.spawn((transform, Visibility::default(), growth)),
                    variant,
                    tall,
                    &looks,
                );
            }
        }
    };
    let mut place = |placings: Vec<Placing>| {
        for p in placings {
            put(
                &models[p.kind][p.model],
                p.transform,
                p.room,
                p.cut,
                p.stops,
            );
        }
    };

    // Trees: woods with clearings, thinning to nothing round the base and at the shore, by
    // the world's own rule (`trees`), which a world that draws nothing asks too (D640).
    let sizes: trees::Sizes = (models.iter())
        .map(|kind| {
            kind.iter()
                .map(|model| (model.height, model.spread))
                .collect()
        })
        .collect();
    let trees = trees::trees((&planet, &start.0), seed, &flora, &sizes, &season);
    for tree in &trees {
        let (foot, radius_m) = tree.stops.unwrap_or_default();
        trunks.add(Trunk {
            foot,
            radius_m,
            height_m: models[tree.kind][tree.model].height * tree.transform.scale.y,
            solid: true,
        });
    }
    place(trees);

    // Rocks: anywhere, more of them on bare and steep ground, mossy in the grass.
    let rocks = trees::rocks((&planet, &start.0), seed, &flora, &sizes, &season);
    place(rocks);

    // A mountain lake's shore: the bare band its high water and ice keep scoured
    // (`terra_land::woods::SHORE_BARE_M`) strewn with its granite's stones and blocks, from
    // the shallows to where the trees start, as Morskie Oko's and every glacial lake's is,
    // thickest at the waterline. Grey rock only: ore's rust is for a find, not a shoreline.
    if let (true, Some(round), Some(stone)) = (
        planet.has_mountains(),
        planet.round(),
        flora.kinds().iter().position(|kind| kind.name == "Rock"),
    ) {
        let bare = terra_land::woods::SHORE_BARE_M;
        place(scatter(
            seed.derive("shore stones"),
            SHORE_STONES_M,
            2.6,
            |rng, x, z, out| {
                // Asked of the lakes' grid first, which is cheap, so only the shore is looked at.
                let across = ground.middle + ground.east * x + ground.north * z;
                if round.water_near(across.normalize(), 1).is_none() {
                    return;
                }
                let point = ground.ground(&planet, x, z);
                let over = point.length() - planet.water_near_m(point);
                if !(-0.7..bare + 0.4).contains(&over) {
                    return;
                }
                // A mountain lake's, scoured by its ice; a lowland lake in the forest, as a
                // settlement's by the valley floor, has soft shores, not a field of boulders
                // through its shallows (D381).
                let mountain = smoothstep(
                    MOUNTAIN_SHORE_M.0,
                    MOUNTAIN_SHORE_M.1,
                    point.length() - planet.sea_radius_m(),
                );
                let chance = 0.6 * mountain * (1.0 - smoothstep(bare - 0.3, bare + 0.4, over));
                if rng.unit() > chance {
                    return;
                }
                // Stones over the band, the blocks the ice shoved at the waterline and in the
                // shallows, so the band itself stays walkable.
                let at_the_edge = 1.0 - smoothstep(0.0, 0.3, over);
                let size = 0.2
                    + 0.45 * rng.unit().powi(2)
                    + at_the_edge
                        * (1.3 * rng.unit().powi(3) + if rng.unit() < 0.08 { 1.2 } else { 0.0 });
                let Some(size) = clear_of_the_surface(over, size) else {
                    return;
                };
                let (index, model) = pick(rng, stone);
                let scale = size / model.height;
                out.push(Placing {
                    kind: stone,
                    model: index,
                    stops: (size > 0.9).then_some((point, model.spread * scale * 0.8)),
                    transform: standing(rng, point, 0.5, scale, size * 0.3),
                    room: 1.0 + size,
                    cut: flora.kinds()[stone].cuttable(size),
                });
            },
        ));
    }

    // Past the trees the heath's own cover grows (below), not the woods' ferns and herbs,
    // even where a pool's edge leaves its map a little meadow.
    let on_heath = |point: Vec3| {
        planet.round().is_some_and(|round| {
            let up = point.normalize();
            let (east, north) = (
                up.any_orthonormal_vector(),
                up.any_orthonormal_vector().cross(up),
            );
            [
                Vec3::ZERO,
                east * 8.0,
                -east * 8.0,
                north * 8.0,
                -north * 8.0,
            ]
            .iter()
            .any(|&off| round.heath.at(point + off).open > 0.5)
        })
    };
    // Round the start, and round a settlement the world starts beside, whose meadow lies
    // far from the start (D381): its flowers and herbs, and the bushes at its forest's edge.
    let mut patches = vec![(ground, seed, 1.0)];
    if let Some(middle) = sited.and_then(|sited| sited.flora_middle(&planet)) {
        let up = middle.normalize();
        let east = up.cross(Vec3::X).normalize();
        let patch = Patch {
            middle,
            east,
            north: east.cross(up),
        };
        patches.push((patch, seed.derive("settlement"), 1.5));
    }
    for &(ground, seed, reach) in &patches {
        let sow = under::Sow {
            planet: &planet,
            flora: &flora,
            models: &models,
            season: &season,
            woods: &woods,
            standing: &standing,
            floored: &floored,
        };
        place(sow.in_the_grass(&ground, seed, reach, &on_heath));
        // Flowers, in drifts.
        place(scatter(
            seed.derive("flowers"),
            130.0 * reach,
            1.5,
            |rng, x, z, out| {
                let point = ground.ground(&planet, x, z);
                let drift = smoothstep(0.05, 0.22, blooms.fbm(point / 22.0, 2, 2.0, 0.5));
                if rng.unit() > 0.75 * drift {
                    return;
                }
                let cover = planet.cover(point, planet.normal(point));
                if cover.grass < 0.5 || on_heath(point) || floored(point) {
                    return;
                }
                let Some(kind) = flora.choose(Grows::Flowers, 0.0, cover.grass, 0.0, &season, rng)
                else {
                    return;
                };
                let (index, model) = pick(rng, kind);
                let tall = size(rng, kind);
                let scale = tall / model.height;
                out.push(Placing {
                    kind,
                    model: index,
                    transform: standing(rng, point, 0.15, scale, 0.02),
                    room: 0.5,
                    cut: flora.kinds()[kind].cuttable(tall),
                    stops: None,
                });
            },
        ));
    }

    // Round the start, rushes and reeds on the lakes' wet margins and ferns and shrubs on the
    // shaded forest floor, as their kinds' sites say.
    let sow = under::Sow {
        planet: &planet,
        flora: &flora,
        models: &models,
        season: &season,
        woods: &woods,
        standing: &standing,
        floored: &floored,
    };
    place(sow.shore(&ground, seed));
    place(sow.understory(&ground, seed, &on_heath));

    // The heath's dwarf shrubs past the trees, where its dwarf-shrub community holds the ground
    // (`terra_land::heath`), in clumps, a few among the lichen and sedge.
    if let Some(round) = planet.round() {
        place(scatter(
            seed.derive("heath shrubs"),
            110.0,
            1.7,
            |rng, x, z, out| {
                let point = ground.ground(&planet, x, z);
                let heath = round.heath.at(point);
                let clumped = smoothstep(-0.1, 0.25, blooms.fbm(point / 9.0 + 41.0, 2, 2.0, 0.5));
                // Scrub in the wet hollows among the sedge, as willow and birch stand there.
                let hollow = heath.wet * (heath.sedge + heath.moss) / heath.open.max(1e-3);
                let chance = (1.3 * heath.shrub + 0.08 * heath.lichen + 0.3 * hollow)
                    * (0.4 + 0.6 * clumped);
                if rng.unit() > chance || point.length() < planet.water_near_m(point) + 0.1 {
                    return;
                }
                let Some(kind) = flora.choose(Grows::Heath, hollow, 0.0, 0.0, &season, rng) else {
                    return;
                };
                let (index, model) = pick(rng, kind);
                let tall = size(rng, kind);
                let scale = tall / model.height;
                out.push(Placing {
                    kind,
                    model: index,
                    transform: standing(rng, point, 0.2, scale, 0.02),
                    room: 0.5,
                    cut: flora.kinds()[kind].cuttable(tall),
                    stops: None,
                });
            },
        ));
    }
    for (at, radius) in solid_things {
        obstacles.add(at, radius);
    }

    commands.insert_resource(stock::Stock::of(&flora, &models, looks));
    // The grass is the cover's, every world's (`cover`), grown round the camera.
    info!(
        "Planted {planted} trees, rocks and plants ({} of them solid).",
        obstacles.len()
    );
    commands.insert_resource(Cleared {
        cleared_at: ticks.this_run(),
        seen: footprints
            .iter()
            .map(|(entity, footprint)| (entity, footprint.clone()))
            .collect(),
    });
}

/// When something built changes shape, clears away whatever it now stands on, with what
/// that blocked. Only what changed is looked at, the plants within reach of the places it
/// took or gave up, so a change to a big base costs a frame no more than a change to a small
/// one.
fn make_way(
    footprints: Query<(Entity, Ref<Footprint>)>,
    cleared: Option<ResMut<Cleared>>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(Entity, &Growth, &Transform, &mut Visibility)>,
    mut gone: MessageWriter<ClearedAway>,
    ticks: SystemChangeTick,
) {
    terra_gen::stat_scope!("flora");
    let Some(mut cleared) = cleared else { return };
    let mut changes: Vec<(&Footprint, Vec<Footprint>)> = Vec::new();
    for (entity, footprint) in &footprints {
        if !footprint
            .last_changed()
            .is_newer_than(cleared.cleared_at, ticks.this_run())
        {
            continue;
        }
        let footprint = footprint.into_inner();
        let changed = footprint.changes_from(cleared.seen.get(&entity));
        cleared.seen.insert(entity, footprint.clone());
        if !changed.is_empty() {
            changes.push((footprint, changed));
        }
    }
    cleared.cleared_at = ticks.this_run();
    if changes.is_empty() {
        return;
    }
    // Plants are cleared by the footprint as it is now, but only those within reach of a
    // place it changed: each change's box, in its own space, as wide as a plant's
    // clearance can reach.
    for (footprint, changed) in &changes {
        let local = footprint.to_local();
        let boxes: Vec<(Vec2, Vec2)> = changed
            .iter()
            .filter(|change| change.frame == footprint.frame)
            .filter_map(Footprint::bounds)
            .collect();
        for (entity, growth, transform, visibility) in &mut growth {
            if *visibility == Visibility::Hidden {
                continue;
            }
            let at = local.transform_point3(transform.translation);
            let reach = growth.clearance_m + footprint.cell_m;
            let near = boxes.iter().any(|(low, high)| {
                at.x > low.x - reach
                    && at.x < high.x + reach
                    && at.z > low.y - reach
                    && at.z < high.y + reach
            });
            if near && footprint.covers_local(at, growth.clearance_m) {
                clear_away(entity, growth, visibility, &mut obstacles, &mut gone);
            }
        }
    }
}

/// What grows after something was built, as a forest grown later than a village is,
/// keeps out of what is built as what grew before it does: a plant set down on a footprint,
/// or within its clearance of one, is cleared at once.
fn clear_new(
    footprints: Query<&Footprint>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(Entity, &Growth, &Transform, &mut Visibility), Added<Growth>>,
    mut gone: MessageWriter<ClearedAway>,
) {
    terra_gen::stat_scope!("flora");
    if footprints.is_empty() {
        return;
    }
    for (entity, growth, transform, visibility) in &mut growth {
        if footprints
            .iter()
            .any(|footprint| footprint.covers(transform.translation, growth.clearance_m))
        {
            clear_away(entity, growth, visibility, &mut obstacles, &mut gone);
        }
    }
}

/// Where the ground has been dug or filled, what grew on it goes: the plants, rocks and
/// trees whose foot is no longer on the ground, with what they blocked (the grass over it
/// is the cover's, `cover::follow_digs`). The digs of a loaded game are followed the same
/// way, once the plants have grown.
fn follow_digs(
    planet: Res<Planet>,
    planted: Option<Res<Cleared>>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(Entity, &Growth, &Transform, &mut Visibility)>,
    mut gone: MessageWriter<ClearedAway>,
    mut seen: Local<usize>,
) {
    terra_gen::stat_scope!("flora");
    if planted.is_none() {
        return;
    }
    let count = planet.voxels().edit_count();
    if count == *seen {
        return;
    }
    let edits: Vec<(Vec3, f32)> = planet.voxels().edits_since(*seen).collect();
    *seen = count;
    let near = |point: Vec3, margin: f32| {
        let up = point.normalize();
        edits
            .iter()
            .any(|&(centre, reach)| (point - centre).reject_from(up).length() < reach + margin)
    };
    for (entity, growth, transform, visibility) in &mut growth {
        let foot = transform.translation;
        if *visibility != Visibility::Hidden && near(foot, 0.5) && planet.dug(foot) {
            clear_away(entity, growth, visibility, &mut obstacles, &mut gone);
        }
    }
}

/// Mows a meadow: clears what grows but the low plants.
fn mow_around(
    mut asked: MessageReader<MowAround>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(Entity, &Growth, &Transform, &mut Visibility)>,
    mut gone: MessageWriter<ClearedAway>,
) {
    terra_gen::stat_scope!("flora");
    for &MowAround(at, radius) in asked.read() {
        let up = at.normalize();
        for (entity, growth, transform, visibility) in &mut growth {
            if !growth.low && (transform.translation - at).reject_from(up).length() < radius {
                clear_away(entity, growth, visibility, &mut obstacles, &mut gone);
            }
        }
    }
}

/// Clears plants where something has been set down since they grew.
pub(crate) fn clear_around(
    mut asked: MessageReader<ClearAround>,
    mut kept: Option<ResMut<KeptClear>>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(Entity, &Growth, &Transform, &mut Visibility)>,
    mut gone: MessageWriter<ClearedAway>,
) {
    terra_gen::stat_scope!("flora");
    for &ClearAround(at, radius) in asked.read() {
        if let Some(kept) = kept.as_mut() {
            kept.keep(at, radius);
        }
        let up = at.normalize();
        for (entity, growth, transform, visibility) in &mut growth {
            if (transform.translation - at).reject_from(up).length() < radius {
                // Only the plant's own obstacle goes; whatever was set down stays solid.
                clear_away(entity, growth, visibility, &mut obstacles, &mut gone);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{Operation, PlanetSpec};

    /// A placing of `kind` that says only where.
    fn mark(at: Vec3, kind: usize) -> Placing {
        Placing {
            kind,
            model: 0,
            transform: Transform::from_translation(at),
            room: 0.0,
            cut: None,
            stops: None,
        }
    }

    /// Decided over every core, a scatter puts the same things in the same order as its grid
    /// walked one square after the next, each square drawing on its own random numbers, some
    /// putting none and some several.
    #[test]
    fn a_scatter_puts_what_one_square_after_the_next_would_in_its_order() {
        let seed = Seed(7).derive("test scatter");
        let decide = |rng: &mut Rng, east: f32, north: f32, out: &mut Vec<Placing>| {
            for _ in 0..rng.below(3) {
                let kind = rng.below(5) as usize;
                out.push(mark(Vec3::new(east, rng.unit(), north), kind));
            }
        };
        let mut one_by_one = Vec::new();
        let cells = (40.0_f32 / 1.5).ceil() as i32;
        for i in -cells..cells {
            for j in -cells..cells {
                let mut rng = seed.at(i, 0, j).rng();
                let east = (i as f32 + rng.unit()) * 1.5;
                let north = (j as f32 + rng.unit()) * 1.5;
                decide(&mut rng, east, north, &mut one_by_one);
            }
        }
        let at_once = scatter(seed, 40.0, 1.5, decide);
        let key = |p: &Placing| (p.kind, p.transform.translation.to_array());
        assert!(one_by_one.len() > 1000, "{}", one_by_one.len());
        assert_eq!(
            at_once.iter().map(key).collect::<Vec<_>>(),
            one_by_one.iter().map(key).collect::<Vec<_>>()
        );
    }

    /// A solid trunk is found while what stops a walker stands at its foot, and not once that
    /// is taken away as it is felled; a long thing filed beside it, a fence line running past
    /// its foot, is not taken for it. One not solid is found whatever stands.
    #[test]
    fn a_solid_trunk_is_found_until_felled_whatever_else_is_filed() {
        let foot = Vec3::new(0.0, 100.0, 0.0);
        let fence = foot + Vec3::new(20.0, 0.0, 0.0);
        let posts = (0..41)
            .map(|post| Piece::round(Vec2::new(post as f32 - 20.0, 0.0), 0.1, 1.2))
            .collect();
        let mut obstacles = Obstacles::default();
        obstacles.add_solid(fence, Solid::of_pieces(fence, posts));
        let mut trunks = Trunks::default();
        let trunk = |foot, solid| Trunk {
            foot,
            radius_m: 0.3,
            height_m: 12.0,
            solid,
        };
        trunks.add(trunk(foot, true));
        let thin = foot + Vec3::new(0.0, 0.0, 3.0);
        trunks.add(trunk(thin, false));
        let found = |obstacles: &Obstacles| -> Vec<Vec3> {
            trunks.near(foot, 5.0, obstacles).map(|t| t.foot).collect()
        };
        assert_eq!(found(&obstacles), [thin], "felled: only the one not solid");
        obstacles.add(foot, 0.3);
        let mut standing = found(&obstacles);
        standing.sort_by(|a, b| a.z.total_cmp(&b.z));
        assert_eq!(standing, [foot, thin], "standing");
        obstacles.remove(foot);
        assert_eq!(found(&obstacles), [thin], "felled again");
    }

    #[test]
    fn no_shore_stone_ends_just_under_the_water() {
        for over in [-0.7, -0.5, -0.35, -0.2, -0.1, 0.0, 0.2] {
            for size in [0.2, 0.3, 0.45, 0.6, 0.9, 1.5] {
                let Some(kept) = clear_of_the_surface(over, size) else {
                    continue;
                };
                let top = over + kept * STONE_STANDS;
                assert!(
                    !(AWASH_M.0..AWASH_M.1).contains(&top),
                    "a stone of {size} m at {over} m left its top at {top} m"
                );
                assert!(kept >= 0.15);
            }
        }
        // Clear of the band, a stone is as it was.
        assert_eq!(clear_of_the_surface(0.3, 0.4), Some(0.4));
        assert_eq!(clear_of_the_surface(-0.7, 0.3), Some(0.3));
    }

    #[test]
    fn scatter_puts_the_same_things_in_the_same_places_on_every_machine() {
        let placed: Vec<_> = scatter(
            Seed(1).derive("trees"),
            13.0,
            6.5,
            |rng, east, north, out| {
                out.push(mark(Vec3::new(east, north, rng.unit()), 0));
            },
        )
        .iter()
        .map(|p| p.transform.translation.into())
        .collect::<Vec<(f32, f32, f32)>>();
        // Two squares either way of the middle, so four by four.
        assert_eq!(placed.len(), 16);
        let pinned = [
            (-12.742245, -12.017293, 0.5474843),
            (-8.730848, -0.9315399, 0.7379886),
            (-12.104529, 4.3883123, 0.40877444),
        ];
        for (got, want) in placed.iter().zip(pinned) {
            assert!(
                (got.0 - want.0).abs() < 1e-4
                    && (got.1 - want.1).abs() < 1e-4
                    && (got.2 - want.2).abs() < 1e-6,
                "{got:?}, not {want:?}"
            );
        }
        // Each in its own square of the grid.
        for &(east, north, _) in &placed {
            assert!((-13.0..13.0).contains(&east) && (-13.0..13.0).contains(&north));
        }
    }

    #[test]
    fn the_old_undergrowth_keeps_off_only_where_the_woods_floor_lays_its_own() {
        use crate::cover::forest::{Crowns, Stand};
        let mut canopy = Crowns::default();
        // Two trunks half a metre thick: one inside the floor's reach, one beyond it.
        for at in [Vec2::new(40.0, 0.0), Vec2::new(0.0, 260.0)] {
            canopy.add(Stand {
                at,
                trunk_m: 0.5,
                crown_at: at,
                crown_m: 3.0,
                closes: 0.8,
                conifer: true,
                spruce: false,
            });
        }
        let floor = Undergrown {
            middle: Vec2::new(0.0, 20.0),
            reach_m: 220.0,
        };
        // Beside the near trunk, within the reach: the floor's own undergrowth.
        assert!(floor.holds(&canopy, Vec2::new(43.0, 2.0)));
        // Open ground in the reach, 9.5 m from the trunk's bark: the old flora's.
        assert!(!floor.holds(&canopy, Vec2::new(50.0, 0.0)));
        // Beside the far trunk, 240 m from the middle: the old flora's still.
        assert!(!floor.holds(&canopy, Vec2::new(0.0, 262.0)));
    }

    #[test]
    fn a_wide_trunk_is_found_from_beyond_the_cubes_a_body_reaches() {
        let ground = Vec3::Y * 2000.0;
        let mut obstacles = Obstacles::default();
        // A great trunk six metres round, its edge a metre and a half from the body.
        let trunk = ground + Vec3::X * 7.5;
        obstacles.add(trunk, 6.0);
        obstacles.add(ground - Vec3::X * 40.0, 0.5);
        let reach = 2.0;
        let touching: Vec<Vec3> = obstacles
            .near(ground, reach)
            .filter(|&(at, radius)| at.distance(ground) < reach + radius)
            .map(|(at, _)| at)
            .collect();
        assert_eq!(touching, [trunk]);
        obstacles.remove(trunk);
        assert_eq!(obstacles.near(ground, reach).count(), 0);
    }

    /// A tree standing on the ground, in the way, and another far off.
    fn two_trees(world: &mut World, near: Vec3, far: Vec3) -> (Entity, Entity) {
        let mut obstacles = Obstacles::default();
        let mut tree = |at: Vec3| {
            obstacles.add(at, 0.4);
            world
                .spawn((
                    Growth::new(None, Some(at), 1.0),
                    Transform::from_translation(at),
                    Visibility::Inherited,
                ))
                .id()
        };
        let trees = (tree(near), tree(far));
        world.insert_resource(obstacles);
        world.init_resource::<Messages<ClearedAway>>();
        trees
    }

    /// Whether a plant is hidden; one cleared away is said to be, so what was drawn for it
    /// elsewhere goes too (a woods tree's far card), and one left standing is not.
    fn hidden(world: &World, tree: Entity) -> bool {
        let said = world
            .resource::<Messages<ClearedAway>>()
            .iter_current_update_messages()
            .any(|&ClearedAway(gone)| gone == tree);
        let hidden = world.get::<Visibility>(tree) == Some(&Visibility::Hidden);
        assert_eq!(said, hidden, "a plant hidden is said to be cleared away");
        hidden
    }

    /// Whether a walker at a point would find a trunk there.
    fn blocked(world: &World, at: Vec3) -> bool {
        world
            .resource::<Obstacles>()
            .near(at, 0.1)
            .any(|(trunk, _)| trunk == at)
    }

    fn planet() -> Planet {
        Planet::new(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        })
    }

    /// What the plants were cleared for, nothing yet, as `plant` leaves it on open ground.
    fn cleared() -> Cleared {
        Cleared {
            cleared_at: Tick::new(0),
            seen: HashMap::default(),
        }
    }

    #[test]
    fn what_is_set_down_clears_the_plants_around_it_and_their_trunks() {
        let mut world = World::new();
        let ground = Vec3::Y * 2000.0;
        let (near, far) = two_trees(&mut world, ground + Vec3::X * 3.0, ground + Vec3::X * 30.0);
        world.init_resource::<Messages<ClearAround>>();
        world.write_message(ClearAround(ground, 5.0));
        world.run_system_cached(clear_around).unwrap();
        assert!(hidden(&world, near) && !blocked(&world, ground + Vec3::X * 3.0));
        assert!(!hidden(&world, far) && blocked(&world, ground + Vec3::X * 30.0));
    }

    /// A place asked to be cleared stays clear: a plant that grows there after (a tree
    /// streamed in round the player once a gate stands) is cleared as it comes, and one
    /// beyond it is not. Without the place kept, the late tree stands where it was cleared.
    #[test]
    fn a_place_cleared_stays_clear_of_what_grows_there_after() {
        let mut world = World::new();
        let ground = Vec3::Y * 2000.0;
        world.init_resource::<Obstacles>();
        world.init_resource::<KeptClear>();
        world.init_resource::<Messages<ClearAround>>();
        world.init_resource::<Messages<ClearedAway>>();
        world.write_message(ClearAround(ground, 5.0));
        world.run_system_cached(clear_around).unwrap();
        // Grown after the asking.
        let (near, far) = two_trees(&mut world, ground + Vec3::X * 3.0, ground + Vec3::X * 30.0);
        world.run_system_cached(clear_grown).unwrap();
        assert!(hidden(&world, near) && !blocked(&world, ground + Vec3::X * 3.0));
        assert!(!hidden(&world, far) && blocked(&world, ground + Vec3::X * 30.0));
    }

    #[test]
    fn digging_under_a_plant_takes_it_and_its_trunk_away() {
        let mut world = World::new();
        let mut planet = planet();
        let start = planet.start();
        let up = start.normalize();
        let away = planet.surface((start + up.any_orthonormal_vector() * 30.0).normalize());
        let (near, far) = two_trees(&mut world, start, away);
        world.insert_resource(cleared());
        planet.apply(&Operation::Dig {
            centre: start,
            radius: 2.0,
        });
        world.insert_resource(planet);
        world.run_system_cached(follow_digs).unwrap();
        assert!(hidden(&world, near) && !blocked(&world, start));
        assert!(!hidden(&world, far) && blocked(&world, away));
    }

    #[test]
    fn building_over_a_plant_takes_it_and_its_trunk_away() {
        let mut world = World::new();
        let planet = planet();
        let start = planet.start();
        let up = start.normalize();
        let frame =
            Transform::from_translation(start).with_rotation(Quat::from_rotation_arc(Vec3::Y, up));
        let on = frame.transform_point(Vec3::new(1.5, 0.0, 1.5));
        let off = frame.transform_point(Vec3::new(40.0, 0.0, 0.0));
        let (near, far) = two_trees(&mut world, on, off);
        world.insert_resource(cleared());
        world.insert_resource(planet);
        world.spawn(Footprint {
            frame,
            cell_m: 1.0,
            ground: (0..4)
                .flat_map(|x| (0..4).map(move |z| IVec2::new(x, z)))
                .collect(),
            ..default()
        });
        world.run_system_cached(make_way).unwrap();
        assert!(hidden(&world, near) && !blocked(&world, on));
        assert!(!hidden(&world, far) && blocked(&world, off));
    }

    #[test]
    fn a_change_to_a_footprint_clears_only_where_it_changed() {
        let mut world = World::new();
        let planet = planet();
        let start = planet.start();
        let up = start.normalize();
        let frame =
            Transform::from_translation(start).with_rotation(Quat::from_rotation_arc(Vec3::Y, up));
        let at = |x: f32, z: f32| frame.transform_point(Vec3::new(x, 0.0, z));
        // A tree where the base will grow to, 30 m east, and one well beyond.
        let (grown_over, beyond) = two_trees(&mut world, at(30.5, 0.5), at(60.0, 0.0));
        world.insert_resource(cleared());
        world.insert_resource(planet);
        let room = |x: i32| (x..x + 4).flat_map(move |x| (0..4).map(move |z| IVec2::new(x, z)));
        let base = world
            .spawn(Footprint {
                frame,
                cell_m: 1.0,
                ground: room(0).collect(),
                ..default()
            })
            .id();
        world.run_system_cached(make_way).unwrap();
        assert!(!hidden(&world, grown_over));
        // The base grows a room 30 m east: the tree under it goes, not the one beyond.
        world
            .get_mut::<Footprint>(base)
            .unwrap()
            .ground
            .extend(room(30));
        world.run_system_cached(make_way).unwrap();
        assert!(hidden(&world, grown_over) && !hidden(&world, beyond));
        // What it was cleared for is the footprint as it now is.
        assert_eq!(world.resource::<Cleared>().seen[&base].ground.len(), 32);
    }

    #[test]
    fn what_grows_after_something_is_built_keeps_out_of_it() {
        let mut world = World::new();
        let frame = Transform::from_translation(Vec3::Y * 2000.0);
        world.spawn(Footprint {
            frame,
            cell_m: 1.0,
            standing: (0..4)
                .flat_map(|x| (0..4).map(move |z| IVec2::new(x, z)))
                .collect(),
            ..default()
        });
        // Planted after it, one in it and one far off.
        let on = frame.transform_point(Vec3::new(1.5, 0.0, 1.5));
        let off = frame.transform_point(Vec3::new(40.0, 0.0, 0.0));
        let (near, far) = two_trees(&mut world, on, off);
        world.run_system_cached(clear_new).unwrap();
        assert!(hidden(&world, near) && !blocked(&world, on));
        assert!(!hidden(&world, far) && blocked(&world, off));
    }
}
