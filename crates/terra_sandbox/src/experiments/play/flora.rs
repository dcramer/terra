//! What grows and lies around the scene: trees, rocks, bushes, flowers and grass.
//!
//! The models are free ones (see THIRD_PARTY.md), read for their shapes only. Their
//! colours are ours, chosen per kind to sit with the light, and everything moves in one
//! wind. Where each thing stands comes from the seed and the place, so it is the same on
//! every run. Trees, rocks and plants are one entity each, sharing a mesh per model.
//! Grass is far too many blades for that, so it is built in squares, each one mesh.

use super::models::{Part, read_model};
use super::scene::SUN_DIRECTION;
use crate::experiments::build::Site;
use crate::planet::{Planet, Tweaks};
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::Indices,
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::{AsBindGroup, PrimitiveTopology},
    shader::ShaderRef,
};
use terra_core::Ground;
use terra_gen::{Noise, Rng, Seed};

/// Grass is planted this far from the base, thinning out over the last stretch.
const GRASS_REACH_M: f32 = 95.0;
/// The side of one square of grass, in metres.
const GRASS_SQUARE_M: f32 = 10.0;
/// Tufts are tried on a grid this far apart, in metres.
const GRASS_SPACING_M: f32 = 0.34;

type WindMaterial = ExtendedMaterial<StandardMaterial, WindExtension>;

/// What the wind shader is told. Field order must match the struct in wind.wgsl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WindExtension {
    #[uniform(100)]
    direction: Vec3,
    #[uniform(100)]
    strength: f32,
}

impl MaterialExtension for WindExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/wind.wgsl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/wind.wgsl".into()
    }
}

/// Things solid enough to walk into: a place and a radius around it.
#[derive(Resource, Default)]
pub struct Obstacles(pub Vec<(Vec3, f32)>);

/// A plant or rock, and how close a base may come before it is cleared away.
#[derive(Component)]
struct Growth {
    clearance_m: f32,
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
    /// The base's shape the grass was last cleared for.
    cleared_for: u64,
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
        planet.drawn((self.middle + self.east * east_m + self.north * north_m).normalize())
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<WindMaterial>::default())
        .init_resource::<Obstacles>()
        .add_systems(PostStartup, plant)
        .add_systems(Update, make_way);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Pine,
    Leafy,
    Birch,
    Rock,
    Bush,
    Plant,
    Flower,
    Prop,
}

/// A model ready to place: its mesh, and its size as modelled.
#[derive(Clone)]
struct Model {
    mesh: Handle<Mesh>,
    height: f32,
    /// Half its width at the ground, more or less.
    spread: f32,
}

/// Our colour for a part of a model, as it should look on screen, and whether it is
/// foliage, which catches the wind.
fn colour_of(kind: Kind, material: &str) -> (Color, bool) {
    let material = material.split('.').next().unwrap_or(material);
    match (kind, material) {
        (Kind::Birch, "White") => (Color::srgb(0.93, 0.9, 0.84), false),
        (Kind::Birch, "Black") => (Color::srgb(0.24, 0.2, 0.2), false),
        (Kind::Birch, "Green") => (Color::srgb(0.96, 0.68, 0.2), true),
        (Kind::Birch, "DarkGreen") => (Color::srgb(0.88, 0.5, 0.16), true),
        (Kind::Pine, "Green") => (Color::srgb(0.2, 0.4, 0.3), true),
        (Kind::Leafy, "Green") => (Color::srgb(0.47, 0.6, 0.22), true),
        (Kind::Leafy, "DarkGreen") => (Color::srgb(0.33, 0.5, 0.22), true),
        (Kind::Rock, "Rock") => (Color::srgb(0.66, 0.58, 0.54), false),
        (Kind::Rock, "Green") => (Color::srgb(0.5, 0.58, 0.26), false),
        (Kind::Bush, "Green") => (Color::srgb(0.36, 0.53, 0.24), true),
        (Kind::Bush, "Berry") => (Color::srgb(0.85, 0.22, 0.26), true),
        (Kind::Flower, "Cyan") => (Color::srgb(0.98, 0.95, 0.88), true),
        (Kind::Flower, "Yellow") => (Color::srgb(1.0, 0.8, 0.2), true),
        (Kind::Flower, "Pink") => (Color::srgb(0.96, 0.5, 0.6), true),
        (_, "Wood") => (Color::srgb(0.43, 0.29, 0.22), false),
        (_, "LightWood") => (Color::srgb(0.78, 0.62, 0.44), false),
        (_, "Mushroom_Top") => (Color::srgb(0.84, 0.32, 0.22), false),
        (_, "Mushroom_Bottom") => (Color::srgb(0.93, 0.88, 0.78), false),
        (_, "DarkGreen") => (Color::srgb(0.27, 0.45, 0.22), true),
        (_, "Leaves") => (Color::srgb(0.42, 0.58, 0.25), true),
        _ => (Color::srgb(0.4, 0.56, 0.24), true),
    }
}

/// Turns a model's parts into one mesh in our colours, carrying how each vertex moves in
/// the wind.
fn build_model(kind: Kind, parts: &[Part], meshes: &mut Assets<Mesh>) -> Model {
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
        let (colour, foliage) = colour_of(kind, &part.material);
        let colour = colour.to_linear();
        let low = part.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let high = part.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let first = positions.len() as u32;
        for (position, normal) in part.positions.iter().zip(&part.normals) {
            let up_plant = (position.y / height).clamp(0.0, 1.0);
            // Foliage is darker underneath and inside, which gives a crown its depth.
            let up_part = ((position.y - low) / (high - low).max(1e-3)).clamp(0.0, 1.0);
            let shade = if foliage {
                0.66 + 0.42 * up_part
            } else {
                0.85 + 0.15 * up_plant
            };
            let (bend, flutter) = match kind {
                Kind::Pine | Kind::Leafy | Kind::Birch => {
                    (1.7 * up_plant * up_plant, if foliage { 0.7 } else { 0.0 })
                }
                Kind::Bush => (0.5 * up_plant, 0.5),
                Kind::Plant | Kind::Flower => (0.9 * up_plant, 0.4 * up_plant),
                Kind::Rock | Kind::Prop => (0.0, 0.0),
            };
            positions.push(*position);
            normals.push(*normal);
            colours.push([
                colour.red * shade,
                colour.green * shade,
                colour.blue * shade,
                1.0,
            ]);
            sways.push([bend, flutter]);
        }
        indices.extend(part.indices.iter().map(|index| first + index));
    }
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, sways)
    .with_inserted_indices(Indices::U32(indices));
    Model {
        mesh: meshes.add(mesh),
        height,
        spread,
    }
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
    site: Res<Site>,
    mut obstacles: ResMut<Obstacles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindMaterial>>,
) {
    let middle = site.frame.translation;
    let up = middle.normalize();
    let east = up.cross(Vec3::X).normalize();
    let ground = Patch {
        middle,
        east,
        north: east.cross(up),
    };
    let seed = planet.ground.spec().seed.derive("flora");

    // The wind blows in off the water, which is where the sun is.
    let wind = WindExtension {
        direction: -SUN_DIRECTION.reject_from(up).normalize(),
        strength: 1.0,
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

    let mut load = |kind: Kind, names: &[&str]| -> Vec<Model> {
        names
            .iter()
            .filter_map(|name| match read_model(&format!("nature/{name}.glb")) {
                Ok(parts) => Some(build_model(kind, &parts, &mut meshes)),
                Err(problem) => {
                    error!("Could not read a model. {problem}");
                    None
                }
            })
            .collect()
    };
    let pines = load(
        Kind::Pine,
        &[
            "PineTree_1",
            "PineTree_2",
            "PineTree_3",
            "PineTree_4",
            "PineTree_5",
        ],
    );
    let leafy = load(
        Kind::Leafy,
        &[
            "CommonTree_1",
            "CommonTree_2",
            "CommonTree_3",
            "CommonTree_4",
            "CommonTree_5",
        ],
    );
    let birches = load(Kind::Birch, &["BirchTree_1", "BirchTree_2", "BirchTree_3"]);
    let rocks = load(
        Kind::Rock,
        &[
            "Rock_1", "Rock_2", "Rock_3", "Rock_4", "Rock_5", "Rock_6", "Rock_7",
        ],
    );
    let mossy_rocks = load(Kind::Rock, &["Rock_Moss_1", "Rock_Moss_2", "Rock_Moss_3"]);
    let bushes = load(Kind::Bush, &["Bush_1", "Bush_2", "BushBerries_1"]);
    let plants = load(Kind::Plant, &["Plant_1", "Plant_3", "Plant_4"]);
    let flowers = load(Kind::Flower, &["Flowers", "Plant_5"]);
    let props = load(Kind::Prop, &["TreeStump", "WoodLog_Moss"]);
    if [
        &pines,
        &leafy,
        &birches,
        &rocks,
        &mossy_rocks,
        &bushes,
        &plants,
        &flowers,
        &props,
    ]
    .iter()
    .any(|models| models.is_empty())
    {
        error!("Some models are missing, so nothing is planted. Is assets/models/nature there?");
        return;
    }

    let sea = planet.sea_radius_m();
    let to_base = site.frame.compute_affine().inverse();
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
    let mut put = |model: &Model, transform: Transform, clearance_m: f32| {
        if site.covers(transform.translation, clearance_m) {
            return;
        }
        planted += 1;
        commands.spawn((
            Mesh3d(model.mesh.clone()),
            MeshMaterial3d(solid.clone()),
            transform,
            Growth { clearance_m },
        ));
    };

    // Trees: woods with clearings, thinning to nothing around the base and at the shore.
    scatter(seed.derive("trees"), 340.0, 6.5, |rng, x, z| {
        let point = ground.ground(&planet, x, z);
        let normal = planet.ground.normal(point);
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
        let (models, tall) = if which < -0.06 {
            (&pines, rng.range(9.0, 16.0))
        } else if which > 0.1 {
            (&birches, rng.range(7.0, 11.0))
        } else {
            (&leafy, rng.range(6.0, 10.5))
        };
        let model = pick(rng, models);
        let scale = tall / model.height;
        obstacles
            .0
            .push((point, (model.spread * scale).clamp(0.25, 0.6)));
        put(&model, standing(rng, point, 0.06, scale, 0.1), 5.0);
    });

    // Rocks: anywhere, more of them on bare and steep ground, mossy in the grass.
    scatter(seed.derive("rocks"), 300.0, 7.0, |rng, x, z| {
        let point = ground.ground(&planet, x, z);
        let cover = planet.cover(point, planet.ground.normal(point));
        if point.length() - sea < -0.4 || rng.unit() > 0.4 - 0.22 * cover.grass {
            return;
        }
        let model = if cover.grass > 0.6 && rng.unit() < 0.6 {
            pick(rng, &mossy_rocks)
        } else {
            pick(rng, &rocks)
        };
        // Mostly small, now and then a boulder.
        let size = 0.35 + 2.4 * rng.unit().powi(3) + if rng.unit() < 0.04 { 2.5 } else { 0.0 };
        let scale = size / model.height;
        if size > 0.9 {
            obstacles.0.push((point, model.spread * scale * 0.8));
        }
        put(
            &model,
            standing(rng, point, 0.5, scale, size * 0.22),
            1.0 + size,
        );
    });

    // Bushes and leafy plants, in the grass.
    scatter(seed.derive("bushes"), 230.0, 5.5, |rng, x, z| {
        let point = ground.ground(&planet, x, z);
        let cover = planet.cover(point, planet.ground.normal(point));
        if rng.unit() > 0.3 * cover.grass {
            return;
        }
        let model = pick(rng, &bushes);
        let scale = rng.range(0.7, 1.5) / model.height;
        put(&model, standing(rng, point, 0.1, scale, 0.05), 1.2);
    });
    scatter(seed.derive("plants"), 150.0, 3.2, |rng, x, z| {
        let point = ground.ground(&planet, x, z);
        let cover = planet.cover(point, planet.ground.normal(point));
        if rng.unit() > 0.28 * cover.grass {
            return;
        }
        let model = pick(rng, &plants);
        let scale = rng.range(0.35, 0.75) / model.height;
        put(&model, standing(rng, point, 0.15, scale, 0.02), 0.6);
    });
    // Flowers, in drifts.
    scatter(seed.derive("flowers"), 130.0, 1.5, |rng, x, z| {
        let point = ground.ground(&planet, x, z);
        let drift = smoothstep(0.05, 0.22, blooms.fbm(point / 22.0, 2, 2.0, 0.5));
        if rng.unit() > 0.75 * drift {
            return;
        }
        let cover = planet.cover(point, planet.ground.normal(point));
        if cover.grass < 0.5 {
            return;
        }
        let model = pick(rng, &flowers);
        let scale = rng.range(0.4, 0.62) / model.height;
        put(&model, standing(rng, point, 0.15, scale, 0.02), 0.5);
    });

    // A stump and a mossy log by the door, as if the clearing had been cut.
    for (model, beside, size) in [
        (&props[0], Vec3::new(1.0, 0.0, 11.0), 0.7),
        (&props[1], Vec3::new(-2.5, 0.0, 12.5), 0.9),
    ] {
        let point = planet.drawn(site.frame.transform_point(beside).normalize());
        let mut rng = seed.derive("props").rng();
        put(
            model,
            standing(&mut rng, point, 0.0, size / model.height, 0.03),
            1.0,
        );
        obstacles.0.push((point, 0.5));
    }

    let mut meadow = Meadow {
        ground,
        seed: seed.derive("grass"),
        material: blades,
        dry: tweaks.grass_dry.to_linear(),
        lush: tweaks.grass_lush.to_linear(),
        squares: HashMap::default(),
        cleared_for: site.revision,
    };
    let squares = (GRASS_REACH_M / GRASS_SQUARE_M).ceil() as i32;
    // One thread per row of squares; working out where the ground is, is the slow part.
    let grown: Vec<((i32, i32), Option<Mesh>)> = std::thread::scope(|scope| {
        let (meadow, planet, site) = (&meadow, &*planet, &*site);
        let rows: Vec<_> = (-squares..squares)
            .map(|i| {
                scope.spawn(move || {
                    (-squares..squares)
                        .map(|j| ((i, j), grass_square(meadow, planet, site, i, j)))
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
        obstacles.0.len()
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
fn grass_square(meadow: &Meadow, planet: &Planet, site: &Site, i: i32, j: i32) -> Option<Mesh> {
    let corner = Vec2::new(i as f32, j as f32) * GRASS_SQUARE_M;
    let nearest = (corner + GRASS_SQUARE_M * 0.5).length() - GRASS_SQUARE_M * 0.71;
    if nearest > GRASS_REACH_M {
        return None;
    }
    let mut positions: Vec<Vec3> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut colours: Vec<[f32; 4]> = Vec::new();
    let mut sways: Vec<[f32; 2]> = Vec::new();
    let tip_tint = LinearRgba::rgb(0.6, 0.62, 0.22);

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
            let ground_normal = planet.ground.normal(root);
            let cover = planet.cover(root, ground_normal);
            if luck > smoothstep(0.3, 0.7, cover.grass) * thinning || site.covers(root, 0.25) {
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

/// When the base changes shape, clears away whatever it now stands on.
fn make_way(
    mut commands: Commands,
    site: Res<Site>,
    planet: Res<Planet>,
    meadow: Option<ResMut<Meadow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut growth: Query<(&Growth, &Transform, &mut Visibility)>,
) {
    let Some(mut meadow) = meadow else { return };
    if meadow.cleared_for == site.revision {
        return;
    }
    meadow.cleared_for = site.revision;
    for (growth, transform, mut visibility) in &mut growth {
        if site.covers(transform.translation, growth.clearance_m) {
            *visibility = Visibility::Hidden;
        }
    }
    // Regrow the squares of grass at or beside the base.
    for (&(i, j), &entity) in &meadow.squares {
        let centre = (Vec2::new(i as f32, j as f32) + 0.5) * GRASS_SQUARE_M;
        let point = meadow.ground.ground(&planet, centre.x, centre.y);
        if !site.covers(point, GRASS_SQUARE_M) {
            continue;
        }
        match grass_square(&meadow, &planet, &site, i, j) {
            Some(mesh) => commands.entity(entity).insert(Mesh3d(meshes.add(mesh))),
            None => commands.entity(entity).remove::<Mesh3d>(),
        };
    }
}
