//! What grows and lies around the scene: trees, rocks, bushes, flowers and grass.
//!
//! Trees, bushes, herbs and flowers are grown by rules from bark and leaf cards
//! (`grown`, D222), a few variants a kind. Rocks and the like are models (scans and free
//! ones, see THIRD_PARTY.md), read for their shapes only and drawn in real materials,
//! rock and bark, tinted per kind (D212, flora.wesl). Everything moves in one wind. Where
//! each thing stands comes from the seed and the place, so it is the same on every run.
//! Each plant or rock is one entity, sharing its meshes with the rest of its variant; a
//! tree's levels of detail are its children.
//! Grass is far too many blades for that, so it is built in squares, each one mesh.

use super::flora_kinds::Cuttable;
use super::flora_kinds::{Cut, Flora, FloraDef, Grows, MATERIALS, Season, Sway};
use super::footprint::Footprint;
use super::grown;
use super::models::{Part, read_model};
use super::scene::SUN_DIRECTION;
use crate::planet::{Planet, Tweaks};
use crate::spatial::SpatialHash;
use crate::start::Start;
use bevy::{
    asset::RenderAssetUsages,
    ecs::{change_detection::Tick, system::SystemChangeTick},
    light::NotShadowCaster,
    mesh::Indices,
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::{AsBindGroup, PrimitiveTopology},
    shader::ShaderRef,
    tasks::{AsyncComputeTaskPool, Task, TaskPool, block_on, poll_once},
};
use terra_core::{Piece, Solid, Walker};
use terra_gen::smoothstep;
use terra_gen::{Noise, Rng, Seed};

/// Grass is planted this far from the base, thinning out over the last stretch.
const GRASS_REACH_M: f32 = 95.0;
/// The side of one square of grass, in metres.
const GRASS_SQUARE_M: f32 = 10.0;
/// Tufts are tried on a grid this far apart, in metres.
const GRASS_SPACING_M: f32 = 0.34;

type WindMaterial = ExtendedMaterial<StandardMaterial, WindExtension>;

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
            .filter(move |trunk| {
                !trunk.solid
                    || obstacles
                        .solids_near(trunk.foot, 0.5)
                        .any(|solid| solid.at.distance(trunk.foot) < 0.5)
            })
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

/// The squares of grass, and what is needed to regrow one.
#[derive(Resource)]
struct Meadow {
    grass: Grass,
    material: Handle<WindMaterial>,
    squares: HashMap<(i32, i32), Entity>,
    /// When the grass was last cleared for what is built.
    cleared_at: Tick,
    /// Each footprint as it was last cleared for, so a change clears for only what it
    /// changed.
    seen: HashMap<Entity, Footprint>,
    /// Squares being grown again on the workers, each put in place of the one drawn when
    /// it is done.
    growing: HashMap<(i32, i32), Task<Option<Mesh>>>,
}

/// What a square of grass is grown from, apart from the planet and what is built.
#[derive(Clone)]
struct Grass {
    ground: Patch,
    seed: Seed,
    dry: LinearRgba,
    lush: LinearRgba,
    /// The share of its tufts grown, at the quality preset (D239).
    density: f32,
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

/// Asks for the plants within so many metres of a place to be cleared, for a village or
/// a hollow set down after the plants grew.
#[derive(Message, Clone, Copy)]
pub struct ClearAround(pub Vec3, pub f32);

/// Asks for the plants within so many metres of a place to be cleared but what grows low and
/// soft, flowers, herbs and ferns: a meadow cut out of the forest for a settlement, its trees,
/// bushes, logs and stones taken away and its flowers left (D381).
#[derive(Message, Clone, Copy)]
pub struct MowAround(pub Vec3, pub f32);

/// What grows lower than this, in no one's way, a mown meadow keeps.
const LOW_M: f32 = 0.9;

/// When what grows is planted, and when it is cleared away for what is built or dug. Whatever
/// keeps a [`Footprint`] up to date does it before these.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FloraSystems {
    Plant,
    Clear,
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<WindMaterial>::default())
        .add_systems(Update, follow_wind)
        .init_resource::<Obstacles>()
        .init_resource::<Trunks>()
        // The yard, the flat test level, has nothing growing on it.
        .add_systems(
            PostStartup,
            plant
                .run_if(crate::shell::in_the_world)
                .in_set(FloraSystems::Plant),
        )
        .add_message::<ClearAround>()
        .add_message::<MowAround>()
        .add_systems(
            Update,
            (
                make_way,
                clear_new,
                clear_around,
                mow_around,
                follow_digs,
                regrown,
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
    let Some(wind) = wind.filter(|wind| wind.is_changed()) else {
        return;
    };
    for (id, material) in materials.iter_mut() {
        let sway = own.of(id, material.extension.strength);
        material.extension.direction = wind.toward;
        material.extension.strength = sway * wind.strength;
    }
}

/// A model ready to place: its mesh, or what a grown one is drawn from, and its size as
/// modelled or grown.
#[derive(Clone)]
struct Model {
    look: Drawn,
    height: f32,
    /// Half its width at the ground, more or less.
    spread: f32,
}

/// How a model is drawn: one mesh in the flora material, or a grown plant's levels of
/// detail and shadow stand-ins (`grown`).
#[derive(Clone)]
enum Drawn {
    Mesh(Handle<Mesh>),
    Grown(Box<grown::Variant>),
}

/// Turns a model's parts into one mesh in our colours, carrying how each vertex moves in
/// the wind.
fn build_model(kind: &FloraDef, parts: &[Part], meshes: &mut Assets<Mesh>) -> Model {
    let all = || parts.iter().flat_map(|part| part.positions.iter());
    let height = all().map(|p| p.y).fold(0.01, f32::max);
    let spread = all()
        .filter(|p| p.y < height * 0.15)
        .map(|p| p.xz().length())
        .fold(0.05, f32::max);
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colours = Vec::new();
    let mut sways = Vec::new();
    let mut indices = Vec::new();
    for part in parts {
        let look = kind.look(&part.material);
        let foliage = look.foliage;
        let colour = look.colour.to_linear();
        let low = part.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let high = part.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let first = positions.len() as u32;
        // The free models are flat-shaded, every triangle its own plane. Drawn in a real
        // material they read as cut facets, so their normals are made smooth across gentle
        // curves and kept split at creases (docs/art-style.md): a crown or a bush as soft
        // lobes, a trunk round, a rock still angular at its edges.
        let crease = if foliage { 75.0 } else { 50.0 };
        let part_normals = smooth_normals(&part.positions, &part.indices, &part.normals, crease);
        // The material is told to the shader in the colour's alpha: a layer of
        // `materials/flora` in eighths, a sixteenth more for one with ore's rust in it, or
        // whole for plain colour.
        let rust = if kind.rust { 0.5 } else { 0.0 };
        let alpha = look
            .material
            .map_or(1.0, |layer| (layer as f32 + rust) / 8.0);
        for (position, normal) in part.positions.iter().zip(&part_normals) {
            let up_plant = (position.y / height).clamp(0.0, 1.0);
            // Foliage is darker underneath and inside, which gives a crown its depth.
            let up_part = ((position.y - low) / (high - low).max(1e-3)).clamp(0.0, 1.0);
            let shade = if foliage {
                0.66 + 0.42 * up_part
            } else {
                0.85 + 0.15 * up_plant
            };
            let (bend, flutter) = match kind.sway {
                Sway::Tree => (1.7 * up_plant * up_plant, if foliage { 0.7 } else { 0.0 }),
                Sway::Bush => (0.5 * up_plant, 0.5),
                Sway::Plant => (0.9 * up_plant, 0.4 * up_plant),
                Sway::Still => (0.0, 0.0),
            };
            positions.push(*position);
            normals.push(*normal);
            colours.push([
                colour.red * shade,
                colour.green * shade,
                colour.blue * shade,
                alpha,
            ]);
            sways.push([bend, flutter]);
        }
        indices.extend(part.indices.iter().map(|index| first + index));
    }
    let count = positions.len();
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, sways)
    // Room for the shader to carry where each point is on the plant, unmoved by the wind,
    // which is where its material is projected from (wind.wesl).
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, vec![[0.0f32; 2]; count])
    .with_inserted_indices(Indices::U32(indices));
    Model {
        look: Drawn::Mesh(meshes.add(mesh)),
        height,
        spread,
    }
}

/// Normals smooth across a surface where its triangles meet at less than `crease_deg`,
/// and split where they meet at more, as a modeller's auto-smooth does: each corner takes
/// the area-weighted normals of the triangles at the same point that face within the
/// crease of its own.
fn smooth_normals(
    positions: &[Vec3],
    indices: &[u32],
    normals: &[Vec3],
    crease_deg: f32,
) -> Vec<Vec3> {
    let limit = libm::cosf(crease_deg.to_radians());
    // Corners at the same point, found by position rounded to a tenth of a millimetre.
    let key = |p: Vec3| (p * 1.0e4).round().as_ivec3();
    let mut faces_at: HashMap<IVec3, Vec<usize>> = HashMap::default();
    let faces: Vec<Vec3> = indices
        .chunks_exact(3)
        .map(|t| {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| positions[i as usize]);
            (b - a).cross(c - a)
        })
        .collect();
    for (face, t) in indices.chunks_exact(3).enumerate() {
        for &i in t {
            faces_at
                .entry(key(positions[i as usize]))
                .or_default()
                .push(face);
        }
    }
    let mut out = normals.to_vec();
    let mut done = vec![false; positions.len()];
    for (face, t) in indices.chunks_exact(3).enumerate() {
        let own = faces[face].normalize_or_zero();
        if own == Vec3::ZERO {
            continue;
        }
        for &i in t {
            let i = i as usize;
            if done[i] {
                continue;
            }
            done[i] = true;
            let sum: Vec3 = faces_at[&key(positions[i])]
                .iter()
                .map(|&other| faces[other])
                .filter(|n| n.normalize_or_zero().dot(own) >= limit)
                .sum();
            out[i] = sum.normalize_or(own);
        }
    }
    out
}

/// Calls `place` once for each square of a grid laid over the map, with a random point
/// in that square. The same seed always gives the same points.
fn scatter(seed: Seed, reach_m: f32, spacing_m: f32, mut place: impl FnMut(&mut Rng, f32, f32)) {
    let cells = (reach_m / spacing_m).ceil() as i32;
    for i in -cells..cells {
        for j in -cells..cells {
            let mut rng = seed.at(i, 0, j).rng();
            let east = (i as f32 + rng.unit()) * spacing_m;
            let north = (j as f32 + rng.unit()) * spacing_m;
            place(&mut rng, east, north);
        }
    }
}

fn plant(
    mut commands: Commands,
    planet: Res<Planet>,
    tweaks: Res<Tweaks>,
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
    quality: Res<crate::quality::Quality>,
    parting: Option<Res<crate::parting::Parting>>,
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
    let (color, normal) = flora_textures(&assets);
    let wind = WindExtension {
        direction: -SUN_DIRECTION.reject_from(up).normalize(),
        strength: 1.0,
        layer_metres: layer_metres(),
        color: Some(color),
        normal: Some(normal),
    };
    let matte = |cull_mode| StandardMaterial {
        perceptual_roughness: 0.9,
        reflectance: 0.18,
        cull_mode,
        ..default()
    };
    let solid = materials.add(ExtendedMaterial {
        base: matte(Some(bevy::render::render_resource::Face::Back)),
        extension: wind.clone(),
    });
    // Blades of grass are single triangles, seen from both sides.
    let blades = materials.add(ExtendedMaterial {
        base: matte(None),
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
    let models: Vec<Vec<Model>> = flora
        .kinds()
        .iter()
        .map(|kind| {
            if let Some(how) = &kind.grown {
                let grew = grown::grow(
                    how,
                    &kind.name,
                    seed.derive("grown").derive(&kind.name),
                    kind.variants,
                    kind.size.unwrap_or((1.0, 1.0)),
                    &cards,
                    &mut looks,
                    &assets,
                    &mut meshes,
                    &mut foliage,
                );
                return grew
                    .into_iter()
                    .map(|variant| Model {
                        height: variant.height,
                        spread: variant.spread,
                        look: Drawn::Grown(Box::new(variant)),
                    })
                    .collect();
            }
            kind.models
                .iter()
                .filter_map(|name| match read_model(&format!("nature/{name}.glb")) {
                    Ok(parts) => Some(build_model(kind, &parts, &mut meshes)),
                    Err(problem) => {
                        error!("Could not read a model of {}. {problem}", kind.name);
                        None
                    }
                })
                .collect()
        })
        .collect();
    if models.iter().any(Vec::is_empty) {
        error!("Some models are missing, so nothing is planted. Is assets/models/nature there?");
        return;
    }
    let size = |rng: &mut Rng, kind: usize| {
        let (least, most) = flora.kinds()[kind].size.unwrap_or((1.0, 1.0));
        rng.range(least, most)
    };

    let to_base = start.compute_affine().inverse();
    let woods = Noise::new(seed.derive("woods"));
    let species = Noise::new(seed.derive("species"));
    let blooms = Noise::new(seed.derive("blooms"));
    let pick =
        |rng: &mut Rng, models: &[Model]| models[rng.below(models.len() as u32) as usize].clone();
    // Standing on the ground at a point, turned at random, leaning at most a little.
    let standing = |rng: &mut Rng, point: Vec3, lean: f32, scale: f32, sunk: f32| {
        let up = point.normalize();
        let lean_axis = up.any_orthonormal_vector();
        let rotation = Quat::from_axis_angle(up, rng.range(0.0, std::f32::consts::TAU))
            * Quat::from_axis_angle(lean_axis, rng.range(0.0, lean))
            * Quat::from_rotation_arc(Vec3::Y, up);
        Transform {
            translation: point - up * sunk,
            rotation,
            scale: Vec3::splat(scale),
        }
    };
    let mut planted = 0;
    let mut solid_things: Vec<(Vec3, f32)> = Vec::new();
    let mut put = |model: &Model,
                   transform: Transform,
                   clearance_m: f32,
                   cut: Option<Cuttable>,
                   stops: Option<(Vec3, f32)>| {
        if footprints
            .iter()
            .any(|(_, footprint)| footprint.covers(transform.translation, clearance_m))
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

    // Trees: woods with clearings, thinning to nothing around the base and at the shore. On a
    // world with mountains the woods grow them, by the land's rules (`woods`).
    let trees_m = if planet.has_mountains() { 0.0 } else { 340.0 };
    scatter(seed.derive("trees"), trees_m, 6.5, |rng, x, z| {
        let point = ground.ground(&planet, x, z);
        let normal = planet.normal(point);
        let from_base = Vec2::new(x, z).length();
        if point.length() - planet.water_near_m(point) < 1.4 || normal.dot(point.normalize()) < 0.86
        {
            return;
        }
        // Keep the view from the door to the water open.
        let from_door = to_base.transform_point3(point) - Vec3::new(7.5, 0.0, 0.0);
        if (0.0..120.0).contains(&from_door.z) && from_door.x.abs() < 10.0 + 0.3 * from_door.z {
            return;
        }
        let wooded = smoothstep(-0.12, 0.2, woods.fbm(point / 120.0, 3, 2.0, 0.5));
        let chance = (0.12 + 0.8 * wooded) * smoothstep(16.0, 42.0, from_base);
        if rng.unit() > chance {
            return;
        }
        let which = species.fbm(point / 150.0 + 9.0, 2, 2.0, 0.5) + rng.range(-0.12, 0.12);
        let Some(kind) = flora.choose(Grows::Trees, which, 0.0, 0.0, &season, rng) else {
            return;
        };
        let tall = size(rng, kind);
        let model = pick(rng, &models[kind]);
        let scale = tall / model.height;
        let trunk = (point, (model.spread * scale).clamp(0.25, 0.6));
        trunks.add(Trunk {
            foot: point,
            radius_m: trunk.1,
            height_m: tall,
            solid: true,
        });
        // The tallest are great trees, which only an iron axe will fell (D96).
        let wood = flora.kinds()[kind].cuttable(tall);
        put(
            &model,
            standing(rng, point, 0.06, scale, 0.1),
            5.0,
            wood,
            Some(trunk),
        );
    });

    // Rocks: anywhere, more of them on bare and steep ground, mossy in the grass.
    scatter(seed.derive("rocks"), 300.0, 7.0, |rng, x, z| {
        let point = ground.ground(&planet, x, z);
        let cover = planet.cover(point, planet.normal(point));
        // On the heath past the trees, boulders lie in fields on the scoured rises and the
        // fellfield, few elsewhere, and lichen grows over them as moss does in the grass.
        let heath = planet.round().map(|round| round.heath.at(point));
        let (chance, grown_over) = match heath {
            Some(h) if h.open > 0.3 => {
                let field = smoothstep(0.0, 0.3, blooms.fbm(point / 60.0 + 13.0, 2, 2.0, 0.5));
                (
                    0.06 + 0.5 * (h.fellfield / h.open).min(1.0) + 0.25 * field,
                    h.open * 0.8,
                )
            }
            _ => (0.4 - 0.22 * cover.grass, cover.grass),
        };
        if point.length() - planet.water_near_m(point) < -0.4 || rng.unit() > chance {
            return;
        }
        // Mostly small, now and then a boulder; a few of the bigger ones hold ore.
        let size = 0.35 + 2.4 * rng.unit().powi(3) + if rng.unit() < 0.04 { 2.5 } else { 0.0 };
        // A few of the bigger ones hold ore; in the grass, most are mossy.
        let Some(kind) = flora.choose(Grows::Rocks, 0.0, grown_over, size, &season, rng) else {
            return;
        };
        let model = pick(rng, &models[kind]);
        let scale = size / model.height;
        let stops = (size > 0.9).then_some((point, model.spread * scale * 0.8));
        let metal = flora.kinds()[kind].cuttable(size);
        put(
            &model,
            standing(rng, point, 0.5, scale, size * 0.22),
            1.0 + size,
            metal,
            stops,
        );
    });

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
        scatter(
            seed.derive("shore stones"),
            SHORE_STONES_M,
            2.6,
            |rng, x, z| {
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
                let model = pick(rng, &models[stone]);
                let scale = size / model.height;
                let stops = (size > 0.9).then_some((point, model.spread * scale * 0.8));
                let metal = flora.kinds()[stone].cuttable(size);
                put(
                    &model,
                    standing(rng, point, 0.5, scale, size * 0.3),
                    1.0 + size,
                    metal,
                    stops,
                );
            },
        );
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
        // Bushes and leafy plants, in the grass.
        scatter(seed.derive("bushes"), 230.0 * reach, 5.5, |rng, x, z| {
            let point = ground.ground(&planet, x, z);
            let cover = planet.cover(point, planet.normal(point));
            if rng.unit() > 0.3 * cover.grass {
                return;
            }
            // Some bear berries, the first food to be found.
            let Some(kind) = flora.choose(Grows::Bushes, 0.0, cover.grass, 0.0, &season, rng)
            else {
                return;
            };
            let model = pick(rng, &models[kind]);
            let tall = size(rng, kind);
            let scale = tall / model.height;
            let fibre = flora.kinds()[kind].cuttable(tall);
            put(
                &model,
                standing(rng, point, 0.1, scale, 0.05),
                1.2,
                fibre,
                None,
            );
        });
        scatter(seed.derive("plants"), 150.0 * reach, 3.2, |rng, x, z| {
            let point = ground.ground(&planet, x, z);
            let cover = planet.cover(point, planet.normal(point));
            if rng.unit() > 0.28 * cover.grass || on_heath(point) {
                return;
            }
            let Some(kind) = flora.choose(Grows::Plants, 0.0, cover.grass, 0.0, &season, rng)
            else {
                return;
            };
            let model = pick(rng, &models[kind]);
            let tall = size(rng, kind);
            let scale = tall / model.height;
            let fibre = flora.kinds()[kind].cuttable(tall);
            put(
                &model,
                standing(rng, point, 0.15, scale, 0.02),
                0.6,
                fibre,
                None,
            );
        });
        // Flowers, in drifts.
        scatter(seed.derive("flowers"), 130.0 * reach, 1.5, |rng, x, z| {
            let point = ground.ground(&planet, x, z);
            let drift = smoothstep(0.05, 0.22, blooms.fbm(point / 22.0, 2, 2.0, 0.5));
            if rng.unit() > 0.75 * drift {
                return;
            }
            let cover = planet.cover(point, planet.normal(point));
            if cover.grass < 0.5 || on_heath(point) {
                return;
            }
            let Some(kind) = flora.choose(Grows::Flowers, 0.0, cover.grass, 0.0, &season, rng)
            else {
                return;
            };
            let model = pick(rng, &models[kind]);
            let tall = size(rng, kind);
            let scale = tall / model.height;
            put(
                &model,
                standing(rng, point, 0.15, scale, 0.02),
                0.5,
                flora.kinds()[kind].cuttable(tall),
                None,
            );
        });
    }

    // The heath's dwarf shrubs past the trees, where its dwarf-shrub community holds the ground
    // (`terra_land::heath`), in clumps, a few among the lichen and sedge.
    if let Some(round) = planet.round() {
        scatter(seed.derive("heath shrubs"), 110.0, 1.7, |rng, x, z| {
            let point = ground.ground(&planet, x, z);
            let heath = round.heath.at(point);
            let clumped = smoothstep(-0.1, 0.25, blooms.fbm(point / 9.0 + 41.0, 2, 2.0, 0.5));
            // Scrub in the wet hollows among the sedge, as willow and birch stand there.
            let hollow = heath.wet * (heath.sedge + heath.moss) / heath.open.max(1e-3);
            let chance =
                (1.3 * heath.shrub + 0.08 * heath.lichen + 0.3 * hollow) * (0.4 + 0.6 * clumped);
            if rng.unit() > chance || point.length() < planet.water_near_m(point) + 0.1 {
                return;
            }
            let Some(kind) = flora.choose(Grows::Heath, hollow, 0.0, 0.0, &season, rng) else {
                return;
            };
            let model = pick(rng, &models[kind]);
            let tall = size(rng, kind);
            let scale = tall / model.height;
            put(
                &model,
                standing(rng, point, 0.2, scale, 0.02),
                0.5,
                flora.kinds()[kind].cuttable(tall),
                None,
            );
        });
    }

    // What lies by the door, such as a stump and a log, as if the clearing had been cut.
    let by_the_door = flora
        .kinds()
        .iter()
        .enumerate()
        .filter_map(|(kind, def)| match def.grows {
            Grows::ByTheDoor(east, south, size) => Some((kind, Vec3::new(east, 0.0, south), size)),
            _ => None,
        });
    for (kind, beside, size) in by_the_door {
        let model = &models[kind][0];
        let point = planet.ground_at(start.transform_point(beside));
        // No clearing was cut where no trees grew.
        if planet.round().is_some_and(|round| {
            round
                .stand(
                    point,
                    planet.normal(point),
                    terra_land::woods::Footing::Soil,
                )
                .density
                < 0.2
        }) {
            continue;
        }
        let mut rng = seed.derive("props").rng();
        put(
            model,
            standing(&mut rng, point, 0.0, size / model.height, 0.03),
            1.0,
            flora.kinds()[kind].cuttable(size),
            Some((point, 0.5)),
        );
    }
    for (at, radius) in solid_things {
        obstacles.add(at, radius);
    }

    let mut meadow = Meadow {
        grass: Grass {
            ground,
            seed: seed.derive("grass"),
            dry: tweaks.grass_dry.to_linear(),
            lush: tweaks.grass_lush.to_linear(),
            density: quality.grass(),
        },
        material: blades,
        squares: HashMap::default(),
        cleared_at: ticks.this_run(),
        seen: footprints
            .iter()
            .map(|(entity, footprint)| (entity, footprint.clone()))
            .collect(),
        growing: HashMap::default(),
    };
    let squares = (GRASS_REACH_M / GRASS_SQUARE_M).ceil() as i32;
    // One thread per row of squares; working out where the ground is, is the slow part.
    let footprints: Vec<&Footprint> = footprints.iter().map(|(_, f)| f).collect();
    let grown: Vec<((i32, i32), Option<Mesh>)> = std::thread::scope(|scope| {
        let (grass, planet, footprints) = (&meadow.grass, &*planet, footprints.as_slice());
        let rows: Vec<_> = (-squares..squares)
            .map(|i| {
                scope.spawn(move || {
                    (-squares..squares)
                        .map(|j| ((i, j), grass_square(grass, planet, footprints, i, j)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        rows.into_iter()
            .flat_map(|row| row.join().unwrap())
            .collect()
    });
    let blades: usize = grown
        .iter()
        .filter_map(|(_, mesh)| mesh.as_ref())
        .map(|mesh| mesh.count_vertices() / 3)
        .sum();
    info!(
        "Planted {planted} trees, rocks and plants ({} of them solid), and {blades} blades of grass.",
        obstacles.len()
    );
    for (square, mesh) in grown {
        let mut entity = commands.spawn((
            MeshMaterial3d(meadow.material.clone()),
            Transform::default(),
            Visibility::default(),
            NotShadowCaster,
        ));
        if let Some(mesh) = mesh {
            entity.insert(Mesh3d(meshes.add(mesh)));
        }
        meadow.squares.insert(square, entity.id());
    }
    commands.insert_resource(meadow);
}

/// One square of grass as a mesh, or nothing if no grass grows there.
fn grass_square(
    meadow: &Grass,
    planet: &Planet,
    footprints: &[&Footprint],
    i: i32,
    j: i32,
) -> Option<Mesh> {
    let corner = Vec2::new(i as f32, j as f32) * GRASS_SQUARE_M;
    let nearest = (corner + GRASS_SQUARE_M * 0.5).length() - GRASS_SQUARE_M * 0.71;
    if nearest > GRASS_REACH_M {
        return None;
    }
    let mut positions: Vec<Vec3> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut colours: Vec<[f32; 4]> = Vec::new();
    let mut sways: Vec<[f32; 2]> = Vec::new();
    // Tips a little lighter and drier than the blade, not yellow.
    let tip_tint = LinearRgba::rgb(0.42, 0.46, 0.22);

    let per_side = (GRASS_SQUARE_M / GRASS_SPACING_M).round() as i32;
    let mut rng = meadow.seed.at(i, 0, j).rng();
    for a in 0..per_side {
        for b in 0..per_side {
            let at =
                corner + Vec2::new(a as f32 + rng.unit(), b as f32 + rng.unit()) * GRASS_SPACING_M;
            // Thinner toward the edge of what is planted, so it has no edge.
            let thinning = (1.0 - smoothstep(GRASS_REACH_M * 0.55, GRASS_REACH_M, at.length()))
                * meadow.density;
            let (luck, turn, vigour) = (rng.unit(), rng.unit(), rng.unit());
            if luck > thinning {
                continue;
            }
            let root = meadow.ground.ground(planet, at.x, at.y);
            let ground_normal = planet.normal(root);
            let cover = planet.cover(root, ground_normal);
            // Nothing grows on ground that has been dug or filled.
            if luck > smoothstep(0.3, 0.7, cover.grass) * thinning
                || footprints
                    .iter()
                    .any(|footprint| footprint.covers(root, 0.25))
                || planet.dug(root)
            {
                continue;
            }
            let up = root.normalize();
            let level = up.any_orthonormal_vector();
            let colour = meadow.dry.mix(&meadow.lush, cover.lush);
            let shade = 0.82 + 0.3 * vigour;
            let tall = (0.22 + 0.24 * vigour) * (0.75 + 0.5 * cover.lush);
            // A tuft: a few blades fanning out from one root, each a single triangle.
            for blade in 0..5 {
                let angle = (turn + blade as f32 * 0.2) * std::f32::consts::TAU + vigour;
                let out = Quat::from_axis_angle(up, angle) * level;
                let side = out.cross(up);
                let reach = 0.03 + 0.05 * ((blade * 3 % 5) as f32 / 4.0);
                let foot = root + out * reach;
                let width = 0.016 + 0.014 * vigour;
                let height = tall * (0.6 + 0.4 * ((blade * 7 % 5) as f32 / 4.0));
                let tip = foot + up * height + out * height * 0.4;
                positions.extend([foot - side * width, foot + side * width, tip]);
                // Lit as the ground is, so a field of it reads as one soft surface.
                normals.extend([ground_normal.lerp(up, 0.5).normalize(); 3]);
                let root_colour = colour * (0.7 * shade);
                let tip_colour = colour.mix(&tip_tint, 0.3) * (1.1 * shade);
                colours.extend([
                    root_colour.with_alpha(1.0).to_f32_array(),
                    root_colour.with_alpha(1.0).to_f32_array(),
                    tip_colour.with_alpha(1.0).to_f32_array(),
                ]);
                sways.extend([[0.0, 0.0], [0.0, 0.0], [1.0, 0.5]]);
            }
        }
    }
    if positions.is_empty() {
        return None;
    }
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, sways),
    )
}

/// When something built changes shape, clears away whatever it now stands on, with what
/// that blocked, and grows again the grass where it changed. Only what changed is looked
/// at: the plants within reach of the places it took or gave up, and the squares of grass
/// under those, which are grown on the workers and put in place when done (`regrown`), so
/// a change to a big base costs a frame no more than a change to a small one.
fn make_way(
    footprints: Query<(Entity, Ref<Footprint>)>,
    planet: Res<Planet>,
    meadow: Option<ResMut<Meadow>>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(&Growth, &Transform, &mut Visibility)>,
    ticks: SystemChangeTick,
) {
    let Some(mut meadow) = meadow else { return };
    let mut changes: Vec<(&Footprint, Vec<Footprint>)> = Vec::new();
    for (entity, footprint) in &footprints {
        if !footprint
            .last_changed()
            .is_newer_than(meadow.cleared_at, ticks.this_run())
        {
            continue;
        }
        let footprint = footprint.into_inner();
        let changed = footprint.changes_from(meadow.seen.get(&entity));
        meadow.seen.insert(entity, footprint.clone());
        if !changed.is_empty() {
            changes.push((footprint, changed));
        }
    }
    meadow.cleared_at = ticks.this_run();
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
        for (growth, transform, mut visibility) in &mut growth {
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
                *visibility = Visibility::Hidden;
                if let Some(stood) = growth.obstacle {
                    obstacles.remove(stood);
                }
            }
        }
    }
    // The squares of grass the changed places fall in, or come within a tuft's clearance
    // of, grown again.
    let mut squares: Vec<(i32, i32)> = Vec::new();
    for (_, changed) in &changes {
        for change in changed {
            for place in change.places() {
                let middle = change.frame.transform_point(Vec3::new(
                    (place.x as f32 + 0.5) * change.cell_m,
                    0.0,
                    (place.y as f32 + 0.5) * change.cell_m,
                ));
                let ground = &meadow.grass.ground;
                let from = middle - ground.middle;
                let (east, north) = (from.dot(ground.east), from.dot(ground.north));
                let reach = change.cell_m * 0.71 + 0.5;
                let key = |m: f32| (m / GRASS_SQUARE_M).floor() as i32;
                for i in key(east - reach)..=key(east + reach) {
                    for j in key(north - reach)..=key(north + reach) {
                        if !squares.contains(&(i, j)) {
                            squares.push((i, j));
                        }
                    }
                }
            }
        }
    }
    let all: Vec<Footprint> = footprints
        .iter()
        .map(|(_, f)| f.into_inner().clone())
        .collect();
    regrow(&mut meadow, &planet, all, squares);
}

/// Grows squares of grass again on the workers, without what is built on `footprints`,
/// each to be put in place of the one drawn when done (`regrown`); one already growing
/// from what was there before is dropped.
fn regrow(
    meadow: &mut Meadow,
    planet: &Planet,
    footprints: Vec<Footprint>,
    squares: Vec<(i32, i32)>,
) {
    let squares: Vec<(i32, i32)> = squares
        .into_iter()
        .filter(|key| meadow.squares.contains_key(key))
        .collect();
    if squares.is_empty() {
        return;
    }
    let pool = AsyncComputeTaskPool::get_or_init(TaskPool::default);
    let footprints = std::sync::Arc::new(footprints);
    for (i, j) in squares {
        let (grass, planet, footprints) =
            (meadow.grass.clone(), planet.clone(), footprints.clone());
        let task = pool.spawn(async move {
            let footprints: Vec<&Footprint> = footprints.iter().collect();
            grass_square(&grass, &planet, &footprints, i, j)
        });
        meadow.growing.insert((i, j), task);
    }
}

/// Puts the squares of grass grown again in place, as each is done.
fn regrown(
    mut commands: Commands,
    meadow: Option<ResMut<Meadow>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(mut meadow) = meadow else { return };
    if meadow.growing.is_empty() {
        return;
    }
    let Meadow {
        squares, growing, ..
    } = &mut *meadow;
    growing.retain(|key, task| {
        let Some(grown) = block_on(poll_once(task)) else {
            return true;
        };
        if let Some(&entity) = squares.get(key) {
            match grown {
                Some(mesh) => commands.entity(entity).insert(Mesh3d(meshes.add(mesh))),
                None => commands.entity(entity).remove::<Mesh3d>(),
            };
        }
        false
    });
}

/// What grows after something was built, as a forest grown later than a village is,
/// keeps out of what is built as what grew before it does: a plant set down on a footprint,
/// or within its clearance of one, is cleared at once.
fn clear_new(
    footprints: Query<&Footprint>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(&Growth, &Transform, &mut Visibility), Added<Growth>>,
) {
    if footprints.is_empty() {
        return;
    }
    for (growth, transform, mut visibility) in &mut growth {
        if footprints
            .iter()
            .any(|footprint| footprint.covers(transform.translation, growth.clearance_m))
        {
            *visibility = Visibility::Hidden;
            if let Some(stood) = growth.obstacle {
                obstacles.remove(stood);
            }
        }
    }
}

/// Where the ground has been dug or filled, what grew on it goes: the plants, rocks and
/// trees whose foot is no longer on the ground, with what they blocked, and the grass
/// over it, whose squares are grown again. The digs of a loaded game are followed the
/// same way, once the plants have grown.
fn follow_digs(
    planet: Res<Planet>,
    footprints: Query<&Footprint>,
    meadow: Option<ResMut<Meadow>>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(&Growth, &Transform, &mut Visibility)>,
    mut seen: Local<usize>,
) {
    let Some(mut meadow) = meadow else { return };
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
    for (growth, transform, mut visibility) in &mut growth {
        let foot = transform.translation;
        if *visibility != Visibility::Hidden && near(foot, 0.5) && planet.dug(foot) {
            *visibility = Visibility::Hidden;
            if let Some(stood) = growth.obstacle {
                obstacles.remove(stood);
            }
        }
    }
    let squares: Vec<(i32, i32)> = meadow
        .squares
        .keys()
        .copied()
        .filter(|&(i, j)| {
            let centre = (Vec2::new(i as f32, j as f32) + 0.5) * GRASS_SQUARE_M;
            let point = meadow.grass.ground.ground(&planet, centre.x, centre.y);
            near(point, GRASS_SQUARE_M * 0.71)
        })
        .collect();
    let footprints: Vec<Footprint> = footprints.iter().cloned().collect();
    regrow(&mut meadow, &planet, footprints, squares);
}

/// Mows a meadow: clears what grows but the low plants.
fn mow_around(
    mut asked: MessageReader<MowAround>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(&Growth, &Transform, &mut Visibility)>,
) {
    for &MowAround(at, radius) in asked.read() {
        let up = at.normalize();
        for (growth, transform, mut visibility) in &mut growth {
            if !growth.low && (transform.translation - at).reject_from(up).length() < radius {
                visibility.set_if_neq(Visibility::Hidden);
                if let Some(stood) = growth.obstacle {
                    obstacles.remove(stood);
                }
            }
        }
    }
}

/// Clears plants where something has been set down since they grew.
fn clear_around(
    mut asked: MessageReader<ClearAround>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(&Growth, &Transform, &mut Visibility)>,
) {
    for &ClearAround(at, radius) in asked.read() {
        let up = at.normalize();
        for (growth, transform, mut visibility) in &mut growth {
            if (transform.translation - at).reject_from(up).length() < radius {
                *visibility = Visibility::Hidden;
                // Only the plant's own obstacle goes; whatever was set down stays solid.
                if let Some(stood) = growth.obstacle {
                    obstacles.remove(stood);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{Operation, PlanetSpec};

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

    /// Two triangles, each its own copy of the shared edge, as flat-shaded models are,
    /// folded along it by `fold` degrees.
    fn hinge(fold: f32) -> (Vec<Vec3>, Vec<u32>, Vec<Vec3>) {
        let (s, c) = (libm::sinf(fold.to_radians()), libm::cosf(fold.to_radians()));
        let far = Vec3::new(0.0, s, c);
        let positions = vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::ZERO,
            far,
            Vec3::X,
        ];
        let indices = vec![0, 1, 2, 3, 4, 5];
        let normals = vec![Vec3::Y; 6];
        (positions, indices, normals)
    }

    #[test]
    fn normals_are_smooth_across_a_gentle_bend_and_split_at_a_crease() {
        let (positions, indices, normals) = hinge(20.0);
        let smooth = smooth_normals(&positions, &indices, &normals, 50.0);
        // Where the two meet, both sides take the same normal, between their planes.
        assert!(smooth[0].distance(smooth[3]) < 1e-4, "{smooth:?}");
        let (positions, indices, normals) = hinge(70.0);
        let split = smooth_normals(&positions, &indices, &normals, 50.0);
        assert!(split[0].distance(split[3]) > 0.5, "{split:?}");
        // Away from the shared edge, each keeps its own plane.
        assert!(split[2].distance(Vec3::Y) < 1e-4, "{split:?}");
    }

    #[test]
    fn scatter_puts_the_same_things_in_the_same_places_on_every_machine() {
        let mut placed = Vec::new();
        scatter(Seed(1).derive("trees"), 13.0, 6.5, |rng, east, north| {
            placed.push((east, north, rng.unit()));
        });
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
        trees
    }

    fn hidden(world: &World, tree: Entity) -> bool {
        world.get::<Visibility>(tree) == Some(&Visibility::Hidden)
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

    /// The meadow, with no squares of grass, as `plant` leaves it.
    fn meadow(planet: &Planet) -> Meadow {
        let middle = planet.start();
        Meadow {
            grass: Grass {
                ground: Patch {
                    middle,
                    east: Vec3::X,
                    north: Vec3::Z,
                },
                seed: Seed(1),
                dry: LinearRgba::BLACK,
                lush: LinearRgba::WHITE,
                density: 1.0,
            },
            material: Handle::default(),
            squares: HashMap::default(),
            cleared_at: Tick::new(0),
            seen: HashMap::default(),
            growing: HashMap::default(),
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

    #[test]
    fn digging_under_a_plant_takes_it_and_its_trunk_away() {
        let mut world = World::new();
        let mut planet = planet();
        let start = planet.start();
        let up = start.normalize();
        let away = planet.surface((start + up.any_orthonormal_vector() * 30.0).normalize());
        let (near, far) = two_trees(&mut world, start, away);
        world.insert_resource(meadow(&planet));
        world.init_resource::<Assets<Mesh>>();
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
        world.insert_resource(meadow(&planet));
        world.insert_resource(planet);
        world.init_resource::<Assets<Mesh>>();
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
    fn a_change_to_a_footprint_clears_and_regrows_only_where_it_changed() {
        let mut world = World::new();
        let planet = planet();
        let start = planet.start();
        let up = start.normalize();
        let frame =
            Transform::from_translation(start).with_rotation(Quat::from_rotation_arc(Vec3::Y, up));
        let at = |x: f32, z: f32| frame.transform_point(Vec3::new(x, 0.0, z));
        // A tree where the base will grow to, 30 m east, and one well beyond.
        let (grown_over, beyond) = two_trees(&mut world, at(30.5, 0.5), at(60.0, 0.0));
        // The meadow's squares round the start, its map along the base's grid.
        let mut meadow = meadow(&planet);
        meadow.grass.ground.east = frame.rotation * Vec3::X;
        meadow.grass.ground.north = frame.rotation * Vec3::Z;
        for i in -8..8 {
            for j in -8..8 {
                let square = world.spawn_empty().id();
                meadow.squares.insert((i, j), square);
            }
        }
        world.insert_resource(meadow);
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
        let growing = |world: &World| {
            let mut keys: Vec<(i32, i32)> =
                world.resource::<Meadow>().growing.keys().copied().collect();
            keys.sort();
            keys
        };
        assert!(growing(&world).contains(&(0, 0)));
        assert!(!hidden(&world, grown_over));
        world.resource_mut::<Meadow>().growing.clear();
        // The base grows a room 30 m east: the grass there is grown again, and not where
        // the first room stands, and the tree under it goes.
        world
            .get_mut::<Footprint>(base)
            .unwrap()
            .ground
            .extend(room(30));
        world.run_system_cached(make_way).unwrap();
        assert_eq!(growing(&world), vec![(2, -1), (2, 0), (3, -1), (3, 0)]);
        assert!(hidden(&world, grown_over) && !hidden(&world, beyond));
        // Nothing changed, nothing done.
        world.resource_mut::<Meadow>().growing.clear();
        world.get_mut::<Footprint>(base).unwrap().set_changed();
        world.run_system_cached(make_way).unwrap();
        assert!(growing(&world).is_empty());
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
