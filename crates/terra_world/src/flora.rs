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
use super::flora_kinds::{Flora, FloraDef, Grows, MATERIALS, Sway};
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
};
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
    (
        assets
            .load_builder()
            .with_settings(layers(false))
            .load("materials/flora/color.jpg"),
        assets
            .load_builder()
            .with_settings(layers(true))
            .load("materials/flora/normal.jpg"),
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

/// Trees and rocks that stop a walker: where each stands and how far round it, filed by
/// place so a walker asks only about those near it.
#[derive(Resource)]
pub struct Obstacles {
    solid: SpatialHash<f32>,
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
    pub fn add(&mut self, at: Vec3, radius: f32) {
        self.widest_m = self.widest_m.max(radius);
        self.solid.insert(at, radius);
    }

    /// Takes away what stands at a point, as when a tree is felled.
    pub fn remove(&mut self, at: Vec3) {
        self.solid.remove(at, |_| true);
    }

    /// Those that a body reaching so far from a point might touch, and how far round each is.
    pub fn near(&self, at: Vec3, reach: f32) -> impl Iterator<Item = (Vec3, f32)> + '_ {
        self.solid
            .near(at, reach + self.widest_m)
            .map(|&(point, radius)| (point, radius))
    }

    pub(crate) fn len(&self) -> usize {
        self.solid.len()
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
        }
    }
}

/// The squares of grass, and what is needed to regrow one.
#[derive(Resource)]
struct Meadow {
    ground: Patch,
    seed: Seed,
    material: Handle<WindMaterial>,
    dry: LinearRgba,
    lush: LinearRgba,
    squares: HashMap<(i32, i32), Entity>,
    /// When the grass was last cleared for what is built.
    cleared_at: Tick,
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

/// Asks for the plants within so many metres of a place to be cleared, for a village or
/// a hollow set down after the plants grew.
#[derive(Message, Clone, Copy)]
pub struct ClearAround(pub Vec3, pub f32);

/// When what grows is planted, and when it is cleared away for what is built or dug. Whatever
/// keeps a [`Footprint`] up to date does it before these.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FloraSystems {
    Plant,
    Clear,
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<WindMaterial>::default())
        .init_resource::<Obstacles>()
        // The yard, the flat test level, has nothing growing on it.
        .add_systems(
            PostStartup,
            plant
                .run_if(crate::shell::in_the_world)
                .in_set(FloraSystems::Plant),
        )
        .add_message::<ClearAround>()
        .add_systems(
            Update,
            (make_way, clear_new, clear_around, follow_digs).in_set(FloraSystems::Clear),
        );
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
        // `materials/flora` in eighths, or whole for plain colour.
        let alpha = look.material.map_or(1.0, |layer| layer as f32 / 8.0);
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

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn plant(
    mut commands: Commands,
    planet: Res<Planet>,
    tweaks: Res<Tweaks>,
    footprints: Query<&Footprint>,
    ticks: SystemChangeTick,
    start: Res<Start>,
    flora: Res<Flora>,
    mut obstacles: ResMut<Obstacles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindMaterial>>,
    mut foliage: ResMut<Assets<grown::Foliage>>,
    mut plain: ResMut<Assets<StandardMaterial>>,
    cards: Res<grown::Cards>,
    assets: Res<AssetServer>,
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

    let sea = planet.sea_radius_m();
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
            .any(|footprint| footprint.covers(transform.translation, clearance_m))
        {
            return;
        }
        planted += 1;
        solid_things.extend(stops);
        let growth = Growth {
            clearance_m,
            cut,
            obstacle: stops.map(|(at, _)| at),
            middle_m: (model.height * transform.scale.y * 0.4).min(1.4),
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

    // Trees: woods with clearings, thinning to nothing around the base and at the shore.
    scatter(seed.derive("trees"), 340.0, 6.5, |rng, x, z| {
        let point = ground.ground(&planet, x, z);
        let normal = planet.normal(point);
        let from_base = Vec2::new(x, z).length();
        if point.length() - sea < 1.4 || normal.dot(point.normalize()) < 0.86 {
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
        let Some(kind) = flora.choose(Grows::Trees, which, 0.0, 0.0, rng) else {
            return;
        };
        let tall = size(rng, kind);
        let model = pick(rng, &models[kind]);
        let scale = tall / model.height;
        let trunk = (point, (model.spread * scale).clamp(0.25, 0.6));
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
        if point.length() - sea < -0.4 || rng.unit() > 0.4 - 0.22 * cover.grass {
            return;
        }
        // Mostly small, now and then a boulder; a few of the bigger ones hold ore.
        let size = 0.35 + 2.4 * rng.unit().powi(3) + if rng.unit() < 0.04 { 2.5 } else { 0.0 };
        // A few of the bigger ones hold ore; in the grass, most are mossy.
        let Some(kind) = flora.choose(Grows::Rocks, 0.0, cover.grass, size, rng) else {
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

    // Bushes and leafy plants, in the grass.
    scatter(seed.derive("bushes"), 230.0, 5.5, |rng, x, z| {
        let point = ground.ground(&planet, x, z);
        let cover = planet.cover(point, planet.normal(point));
        if rng.unit() > 0.3 * cover.grass {
            return;
        }
        // Some bear berries, the first food to be found.
        let Some(kind) = flora.choose(Grows::Bushes, 0.0, cover.grass, 0.0, rng) else {
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
    scatter(seed.derive("plants"), 150.0, 3.2, |rng, x, z| {
        let point = ground.ground(&planet, x, z);
        let cover = planet.cover(point, planet.normal(point));
        if rng.unit() > 0.28 * cover.grass {
            return;
        }
        let Some(kind) = flora.choose(Grows::Plants, 0.0, cover.grass, 0.0, rng) else {
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
    scatter(seed.derive("flowers"), 130.0, 1.5, |rng, x, z| {
        let point = ground.ground(&planet, x, z);
        let drift = smoothstep(0.05, 0.22, blooms.fbm(point / 22.0, 2, 2.0, 0.5));
        if rng.unit() > 0.75 * drift {
            return;
        }
        let cover = planet.cover(point, planet.normal(point));
        if cover.grass < 0.5 {
            return;
        }
        let Some(kind) = flora.choose(Grows::Flowers, 0.0, cover.grass, 0.0, rng) else {
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
        ground,
        seed: seed.derive("grass"),
        material: blades,
        dry: tweaks.grass_dry.to_linear(),
        lush: tweaks.grass_lush.to_linear(),
        squares: HashMap::default(),
        cleared_at: ticks.this_run(),
    };
    let squares = (GRASS_REACH_M / GRASS_SQUARE_M).ceil() as i32;
    // One thread per row of squares; working out where the ground is, is the slow part.
    let footprints: Vec<&Footprint> = footprints.iter().collect();
    let grown: Vec<((i32, i32), Option<Mesh>)> = std::thread::scope(|scope| {
        let (meadow, planet, footprints) = (&meadow, &*planet, footprints.as_slice());
        let rows: Vec<_> = (-squares..squares)
            .map(|i| {
                scope.spawn(move || {
                    (-squares..squares)
                        .map(|j| ((i, j), grass_square(meadow, planet, footprints, i, j)))
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
    meadow: &Meadow,
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
            let thinning = 1.0 - smoothstep(GRASS_REACH_M * 0.55, GRASS_REACH_M, at.length());
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
/// that blocked.
fn make_way(
    mut commands: Commands,
    footprints: Query<Ref<Footprint>>,
    planet: Res<Planet>,
    meadow: Option<ResMut<Meadow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(&Growth, &Transform, &mut Visibility)>,
    ticks: SystemChangeTick,
) {
    let Some(mut meadow) = meadow else { return };
    let changed: Vec<&Footprint> = footprints
        .iter()
        .filter(|footprint| {
            footprint
                .last_changed()
                .is_newer_than(meadow.cleared_at, ticks.this_run())
        })
        .map(Ref::into_inner)
        .collect();
    if changed.is_empty() {
        return;
    }
    meadow.cleared_at = ticks.this_run();
    for (growth, transform, mut visibility) in &mut growth {
        if changed
            .iter()
            .any(|footprint| footprint.covers(transform.translation, growth.clearance_m))
        {
            *visibility = Visibility::Hidden;
            if let Some(stood) = growth.obstacle {
                obstacles.remove(stood);
            }
        }
    }
    // Regrow the squares of grass at or beside what changed.
    let all: Vec<&Footprint> = footprints.iter().map(Ref::into_inner).collect();
    for (&(i, j), &entity) in &meadow.squares {
        let centre = (Vec2::new(i as f32, j as f32) + 0.5) * GRASS_SQUARE_M;
        let point = meadow.ground.ground(&planet, centre.x, centre.y);
        if !changed
            .iter()
            .any(|footprint| footprint.covers(point, GRASS_SQUARE_M))
        {
            continue;
        }
        match grass_square(&meadow, &planet, &all, i, j) {
            Some(mesh) => commands.entity(entity).insert(Mesh3d(meshes.add(mesh))),
            None => commands.entity(entity).remove::<Mesh3d>(),
        };
    }
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
    mut commands: Commands,
    planet: Res<Planet>,
    footprints: Query<&Footprint>,
    meadow: Option<Res<Meadow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut obstacles: ResMut<Obstacles>,
    mut growth: Query<(&Growth, &Transform, &mut Visibility)>,
    mut seen: Local<usize>,
) {
    let Some(meadow) = meadow else { return };
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
    for (&(i, j), &entity) in &meadow.squares {
        let centre = (Vec2::new(i as f32, j as f32) + 0.5) * GRASS_SQUARE_M;
        let point = meadow.ground.ground(&planet, centre.x, centre.y);
        if !near(point, GRASS_SQUARE_M * 0.71) {
            continue;
        }
        match grass_square(
            &meadow,
            &planet,
            &footprints.iter().collect::<Vec<_>>(),
            i,
            j,
        ) {
            Some(mesh) => commands.entity(entity).insert(Mesh3d(meshes.add(mesh))),
            None => commands.entity(entity).remove::<Mesh3d>(),
        };
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
            ground: Patch {
                middle,
                east: Vec3::X,
                north: Vec3::Z,
            },
            seed: Seed(1),
            material: Handle::default(),
            dry: LinearRgba::BLACK,
            lush: LinearRgba::WHITE,
            squares: HashMap::default(),
            cleared_at: Tick::new(0),
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
