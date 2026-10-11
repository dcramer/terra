//! The stand-in planet on screen: its ground, its sea, the sun, and the camera.
//!
//! The planet itself comes from `terra_core`; this only draws it. The whole planet is one
//! coarse mesh, with a square of much finer ground set into it around the start, because
//! that is where anything is looked at closely. The ground's look is in
//! `assets/shaders/ground.wgsl` and the sea's in `assets/shaders/water.wgsl`; both reload
//! when saved.

use crate::shell::Options;
use crate::sky::Sky;
use bevy::{
    asset::RenderAssetUsages,
    camera_controller::free_camera::FreeCamera,
    input::common_conditions::input_toggle_active,
    light::NotShadowCaster,
    mesh::Indices,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, PrimitiveTopology},
    shader::ShaderRef,
};
use bevy_inspector_egui::{prelude::*, quick::ResourceInspectorPlugin};
use std::sync::Arc;
use terra_core::{Ground, PlanetSpec, RayHit, Seed, StandInPlanet, Water};
use terra_gen::Noise;

/// Height of the tallest hills, in metres.
const RELIEF_M: f32 = 150.0;
/// Sea level relative to the planet's nominal radius, in metres.
const SEA_LEVEL_M: f32 = -20.0;
/// Grid squares along each edge of the six faces the planet's mesh is built from.
const CELLS_PER_FACE: usize = 256;
/// The camera starts here. The free camera treats +Y as up, so the pole is the one place
/// on the planet where that matches the ground.
const START: Vec3 = Vec3::Y;

/// The face of the cube the finer ground is set into: the one the start is in the middle of.
const DETAIL_FACE: usize = 2;
/// How many of the coarse mesh's squares the finer ground replaces, along each edge.
const DETAIL_CELLS: usize = 32;
/// How many squares each of those is divided into, along each edge. About a metre each.
const DETAIL_DIVISIONS: usize = 16;
/// Over this many coarse squares in from its edge, the finer ground eases from the coarse
/// mesh's shape to the true one, so the two meet without a step.
const DETAIL_BLEND_CELLS: f32 = 3.0;

/// The six faces of the cube the planet is divided along, each as (outward, across, up),
/// ordered so that triangles built across-then-up face outward. Anything that divides the
/// planet's surface into a grid uses these.
pub const FACES: [(Vec3, Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::Y, Vec3::Z),
    (Vec3::NEG_X, Vec3::Z, Vec3::Y),
    (Vec3::Y, Vec3::Z, Vec3::X),
    (Vec3::NEG_Y, Vec3::X, Vec3::Z),
    (Vec3::Z, Vec3::X, Vec3::Y),
    (Vec3::NEG_Z, Vec3::Y, Vec3::X),
];

/// The planet every experiment shares. Ask it where the ground is.
/// The planet the game stands on. Everything asks it about the ground through the
/// questions below, and nothing reaches into what answers them, so the stand-in's
/// heights can give way to voxel terrain (docs/terrain.md) behind them.
#[derive(Resource, Clone)]
pub struct Planet {
    ground: Arc<StandInPlanet>,
    cover: Noise,
}

/// What grows at a place, each from 0 to 1.
#[derive(Clone, Copy, Debug)]
pub struct Cover {
    /// How much of the ground is grass rather than bare soil or sand.
    pub grass: f32,
    /// Dry and sparse at 0, green and thick at 1.
    pub lush: f32,
}

impl Planet {
    /// The stand-in planet for a spec, with the sandbox's hills and sea.
    pub fn new(spec: PlanetSpec) -> Self {
        Planet {
            ground: Arc::new(StandInPlanet::new(spec, RELIEF_M, Some(SEA_LEVEL_M))),
            cover: Noise::new(spec.seed.derive("ground cover")),
        }
    }

    /// Where something at a point stands: the ground under it, as it is drawn. The point
    /// is a place in the world, not a direction, since with voxels there may be ground
    /// above as well as below, as in a cave or under an overhang.
    pub fn ground_at(&self, point: Vec3) -> Vec3 {
        self.drawn(point.normalize())
    }

    /// Whether the ground at a point is under the sea.
    pub fn under_sea(&self, point: Vec3) -> bool {
        self.ground_at(point).length() < self.sea_radius_m()
    }

    /// Which way the ground faces at a point on it.
    pub fn normal(&self, point: Vec3) -> Vec3 {
        self.ground.normal(point)
    }

    /// How far a point is above the ground, below it if negative.
    pub fn distance(&self, point: Vec3) -> f32 {
        self.ground.distance(point)
    }

    /// Where a ray first meets the ground, within so far. `direction` is unit length.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<RayHit> {
        self.ground.raycast(origin, direction, max_distance)
    }

    pub fn radius_m(&self) -> f32 {
        self.ground.spec().radius_m
    }

    pub fn seed(&self) -> Seed {
        self.ground.spec().seed
    }

    /// The point on the ground in a direction from the centre.
    pub fn surface(&self, direction: Vec3) -> Vec3 {
        direction * (self.ground.spec().radius_m + self.ground.height_m(direction))
    }

    /// Where the camera starts: a point on the ground.
    pub fn start(&self) -> Vec3 {
        self.surface(START)
    }

    /// Distance from the planet's centre to the surface of the sea.
    pub fn sea_radius_m(&self) -> f32 {
        self.ground.spec().radius_m + SEA_LEVEL_M
    }

    /// What grows on the ground at a point, given which way the ground faces there.
    pub fn cover(&self, point: Vec3, normal: Vec3) -> Cover {
        let above_sea = point.length() - self.sea_radius_m();
        let steepness = 1.0 - normal.dot(point.normalize());
        let patches = self.cover.fbm(point / 70.0, 3, 2.0, 0.5);
        Cover {
            grass: smoothstep(-0.4, -0.26, patches)
                * smoothstep(0.5, 1.6, above_sea)
                * (1.0 - smoothstep(0.14, 0.27, steepness)),
            lush: smoothstep(
                -0.35,
                0.35,
                self.cover.fbm(point / 45.0 + 31.0, 2, 2.0, 0.5),
            ),
        }
    }

    /// The point on the ground as it is drawn in a direction from the centre. Near the
    /// start that is the true ground. Far from it the mesh is coarse and cuts across
    /// hollows and humps by metres, so anything that should sit on what is seen there asks
    /// for this.
    pub fn drawn(&self, direction: Vec3) -> Vec3 {
        let (face, a, b) = face_coordinates(direction);
        let trueness = if face == DETAIL_FACE {
            detail_trueness(cell_coordinate(a), cell_coordinate(b))
        } else {
            0.0
        };
        if trueness >= 1.0 {
            return self.surface(direction);
        }
        let coarse = self.coarse(face, a, b, direction);
        if trueness <= 0.0 {
            coarse
        } else {
            coarse.lerp(self.surface(direction), trueness)
        }
    }

    /// Where a line from the centre meets the coarse mesh's triangle in that direction.
    fn coarse(&self, face: usize, a: f32, b: f32, direction: Vec3) -> Vec3 {
        let cells = CELLS_PER_FACE as f32;
        let (x, y) = (cell_coordinate(a), cell_coordinate(b));
        let (i, j) = (
            x.floor().clamp(0.0, cells - 1.0),
            y.floor().clamp(0.0, cells - 1.0),
        );
        let corner = |di: f32, dj: f32| {
            self.surface(face_direction(
                face,
                (i + di) / cells * 2.0 - 1.0,
                (j + dj) / cells * 2.0 - 1.0,
            ))
        };
        // Each square is two triangles, split along the diagonal from its right corner
        // to the one above.
        let (right, above) = (corner(1.0, 0.0), corner(0.0, 1.0));
        let third = if (x - i) + (y - j) <= 1.0 {
            corner(0.0, 0.0)
        } else {
            corner(1.0, 1.0)
        };
        let plane = (right - third).cross(above - third);
        direction * (plane.dot(third) / plane.dot(direction))
    }
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The direction of a place on a face, from its position across (a) and up (b) the face,
/// each from -1 to 1.
fn face_direction(face: usize, a: f32, b: f32) -> Vec3 {
    let (outward, across, up) = FACES[face];
    (outward + across * a + up * b).normalize()
}

/// The face a direction falls on, and its position across and up that face.
fn face_coordinates(direction: Vec3) -> (usize, f32, f32) {
    let (face, facing) = FACES
        .iter()
        .map(|(outward, _, _)| outward.dot(direction))
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    let (_, across, up) = FACES[face];
    (
        face,
        across.dot(direction) / facing,
        up.dot(direction) / facing,
    )
}

/// A position across a face, counted in the coarse mesh's squares.
fn cell_coordinate(a: f32) -> f32 {
    (a + 1.0) * 0.5 * CELLS_PER_FACE as f32
}

/// The coarse squares the finer ground replaces, along either edge of its face.
fn detail_cells() -> std::ops::Range<usize> {
    let first = (CELLS_PER_FACE - DETAIL_CELLS) / 2;
    first..first + DETAIL_CELLS
}

/// How far the finer ground has eased from the coarse mesh's shape (0) to the true ground
/// (1) at a place on its face, given in coarse squares.
fn detail_trueness(x: f32, y: f32) -> f32 {
    let cells = detail_cells();
    let (first, last) = (cells.start as f32, cells.end as f32);
    let from_edge = (x - first).min(last - x).min(y - first).min(last - y);
    smoothstep(0.0, DETAIL_BLEND_CELLS, from_edge)
}

/// Values worth changing while the sandbox runs. Press F2 to edit them.
#[derive(Resource, Reflect, InspectorOptions)]
#[reflect(Resource, InspectorOptions)]
pub struct Tweaks {
    /// Direction from the planet toward the sun.
    pub sun_direction: Vec3,
    pub sun_color: Color,
    #[inspector(min = 0.0, max = 100_000.0)]
    pub sun_illuminance: f32,
    pub sea: bool,
    /// Sea level relative to the planet's nominal radius, in metres.
    #[inspector(min = -150.0, max = 150.0)]
    pub sea_level_m: f32,
    pub sea_shallow: Color,
    pub sea_deep: Color,
    pub ground_low: Color,
    pub ground_high: Color,
    pub ground_steep: Color,
    /// Light each triangle as a flat facet instead of smoothing across them.
    pub ground_faceted: bool,
    /// How much of the grass and sand the ground could have is shown, from none to all.
    #[inspector(min = 0.0, max = 1.0)]
    pub ground_cover: f32,
    pub grass_dry: Color,
    pub grass_lush: Color,
    pub sand: Color,
}

#[derive(Component)]
struct Sea;

#[derive(Component)]
struct Sun;

type GroundMaterial = ExtendedMaterial<StandardMaterial, GroundExtension>;
type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExtension>;

/// What the ground shader is told. Field order must match the struct in ground.wgsl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GroundExtension {
    #[uniform(100)]
    radius_m: f32,
    #[uniform(100)]
    relief_m: f32,
    #[uniform(100)]
    low: LinearRgba,
    #[uniform(100)]
    high: LinearRgba,
    #[uniform(100)]
    steep: LinearRgba,
    #[uniform(100)]
    faceted: u32,
    #[uniform(100)]
    cover: f32,
    #[uniform(100)]
    sea_radius_m: f32,
    #[uniform(100)]
    grass_dry: LinearRgba,
    #[uniform(100)]
    grass_lush: LinearRgba,
    #[uniform(100)]
    sand: LinearRgba,
    #[uniform(100)]
    sea_deep: LinearRgba,
}

impl MaterialExtension for GroundExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/ground.wgsl".into()
    }
}

impl GroundExtension {
    fn new(planet: &Planet, tweaks: &Tweaks) -> Self {
        let radius_m = planet.ground.spec().radius_m;
        GroundExtension {
            radius_m,
            relief_m: RELIEF_M,
            low: tweaks.ground_low.into(),
            high: tweaks.ground_high.into(),
            steep: tweaks.ground_steep.into(),
            faceted: tweaks.ground_faceted as u32,
            cover: tweaks.ground_cover,
            // Where there is no sea, put its level out of reach below the ground.
            sea_radius_m: if tweaks.sea {
                radius_m + tweaks.sea_level_m
            } else {
                0.0
            },
            grass_dry: tweaks.grass_dry.into(),
            grass_lush: tweaks.grass_lush.into(),
            sand: tweaks.sand.into(),
            sea_deep: tweaks.sea_deep.into(),
        }
    }
}

/// What the water shader is told. Field order must match the struct in water.wgsl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WaterExtension {
    /// The sky at the horizon and overhead, which is what the water reflects.
    #[uniform(100)]
    horizon: LinearRgba,
    #[uniform(100)]
    zenith: LinearRgba,
    #[uniform(100)]
    sky_brightness: f32,
}

impl MaterialExtension for WaterExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
}

impl WaterExtension {
    fn new(sky: Option<&Sky>) -> Self {
        // With no sky to reflect, the water reflects nothing.
        let (horizon, zenith, sky_brightness) = sky
            .map_or((LinearRgba::BLACK, LinearRgba::BLACK, 0.0), |sky| {
                (sky.horizon.into(), sky.zenith.into(), sky.brightness)
            });
        WaterExtension {
            horizon,
            zenith,
            sky_brightness,
        }
    }
}

pub fn plugin(app: &mut App) {
    let radius_m = app.world().resource::<Options>().radius_m;
    let spec = PlanetSpec {
        seed: Seed(1),
        radius_m,
    };
    let planet = Planet::new(spec);
    let tweaks = Tweaks {
        sun_direction: Vec3::new(0.5, 0.75, 0.45),
        sun_color: Color::WHITE,
        sun_illuminance: 12_000.0,
        sea: true,
        sea_level_m: planet
            .ground
            .level(START)
            .map_or(0.0, |level| level - radius_m),
        sea_shallow: Color::srgb(0.1, 0.38, 0.5),
        sea_deep: Color::srgb(0.02, 0.1, 0.24),
        ground_low: Color::srgb(0.42, 0.33, 0.26),
        ground_high: Color::srgb(0.72, 0.66, 0.58),
        ground_steep: Color::srgb(0.3, 0.27, 0.25),
        ground_faceted: false,
        ground_cover: 0.0,
        grass_dry: Color::srgb(0.6, 0.58, 0.26),
        grass_lush: Color::srgb(0.3, 0.5, 0.2),
        sand: Color::srgb(0.84, 0.74, 0.55),
    };
    app.insert_resource(planet)
        .insert_resource(tweaks)
        .insert_resource(GlobalAmbientLight {
            brightness: 120.0,
            ..default()
        })
        .register_type::<Tweaks>()
        .add_plugins((
            MaterialPlugin::<GroundMaterial>::default(),
            MaterialPlugin::<WaterMaterial>::default(),
        ))
        .add_plugins(
            ResourceInspectorPlugin::<Tweaks>::default()
                .run_if(input_toggle_active(false, KeyCode::F2)),
        )
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            apply_tweaks
                .run_if(resource_changed::<Tweaks>.or_eager(resource_exists_and_changed::<Sky>)),
        );
}

fn setup(
    mut commands: Commands,
    planet: Res<Planet>,
    tweaks: Res<Tweaks>,
    sky: Option<Res<Sky>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ground_materials: ResMut<Assets<GroundMaterial>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
) {
    let radius_m = planet.ground.spec().radius_m;
    let ground_material = ground_materials.add(ExtendedMaterial {
        base: StandardMaterial {
            perceptual_roughness: 0.95,
            reflectance: 0.2,
            ..default()
        },
        extension: GroundExtension::new(&planet, &tweaks),
    });
    commands.spawn((
        Name::new("Ground"),
        Mesh3d(meshes.add(ground_mesh(&planet, CELLS_PER_FACE))),
        MeshMaterial3d(ground_material.clone()),
    ));
    commands.spawn((
        Name::new("Ground near the start"),
        Mesh3d(meshes.add(detail_mesh(&planet))),
        MeshMaterial3d(ground_material),
    ));
    commands.spawn((
        Name::new("Sea"),
        Sea,
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(64).unwrap())),
        MeshMaterial3d(water_materials.add(ExtendedMaterial {
            base: StandardMaterial {
                base_color: tweaks.sea_shallow,
                perceptual_roughness: 0.1,
                reflectance: 0.6,
                alpha_mode: AlphaMode::Blend,
                ..default()
            },
            extension: WaterExtension::new(sky.as_deref()),
        })),
        NotShadowCaster,
    ));
    commands.spawn((
        Name::new("Sun"),
        Sun,
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
    ));

    // Stand a little above the ground, looking out along it.
    let start = planet.start();
    let up = start.normalize();
    let along = up.cross(Vec3::X).normalize();
    commands.spawn((
        Name::new("Camera"),
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            near: 0.2,
            far: radius_m * 40.0,
            ..default()
        }),
        Transform::from_translation(start + up * 25.0).looking_to(along - up * 0.25, up),
        // Speeds suited to a planet rather than a room. The scroll wheel changes them.
        FreeCamera {
            walk_speed: 30.0,
            run_speed: 300.0,
            ..default()
        },
    ));
}

fn apply_tweaks(
    tweaks: Res<Tweaks>,
    planet: Res<Planet>,
    sky: Option<Res<Sky>>,
    mut sun: Single<(&mut DirectionalLight, &mut Transform), (With<Sun>, Without<Sea>)>,
    mut sea: Single<(&mut Transform, &mut Visibility), With<Sea>>,
    mut ground_materials: ResMut<Assets<GroundMaterial>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
) {
    let toward_sun = tweaks.sun_direction.normalize_or(Vec3::Y);
    sun.0.color = tweaks.sun_color;
    sun.0.illuminance = tweaks.sun_illuminance;
    *sun.1 = Transform::from_translation(toward_sun).looking_at(Vec3::ZERO, Vec3::Y);

    sea.0.scale = Vec3::splat(planet.ground.spec().radius_m + tweaks.sea_level_m);
    *sea.1 = if tweaks.sea {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    for (_, material) in ground_materials.iter_mut() {
        material.extension = GroundExtension::new(&planet, &tweaks);
    }
    for (_, material) in water_materials.iter_mut() {
        material.base.base_color = tweaks.sea_shallow;
        material.extension = WaterExtension::new(sky.as_deref());
    }
}

/// What grows at a point, packed the way the ground shader reads it from a vertex.
fn cover_colour(planet: &Planet, point: Vec3, normal: Vec3) -> [f32; 4] {
    let cover = planet.cover(point, normal);
    [cover.grass, cover.lush, 0.0, 1.0]
}

struct Surface {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Surface {
    fn into_mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colours)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// The planet as one mesh: a cube with each face divided into a grid, pushed out into a
/// sphere, with every vertex moved to the ground's height. The squares the finer ground
/// replaces are left out.
fn ground_mesh(planet: &Planet, cells: usize) -> Mesh {
    let side = cells + 1;
    let face = |face: usize| {
        let mut positions = Vec::with_capacity(side * side);
        let mut normals = Vec::with_capacity(side * side);
        let mut colours = Vec::with_capacity(side * side);
        for j in 0..side {
            for i in 0..side {
                let a = i as f32 / cells as f32 * 2.0 - 1.0;
                let b = j as f32 / cells as f32 * 2.0 - 1.0;
                let point = planet.surface(face_direction(face, a, b));
                let normal = planet.ground.normal(point);
                positions.push(point);
                normals.push(normal);
                colours.push(cover_colour(planet, point, normal));
            }
        }
        (positions, normals, colours)
    };
    // One thread per face; the heights are the slow part.
    let faces: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..FACES.len())
            .map(|f| scope.spawn(move || face(f)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });

    let mut surface = Surface {
        positions: Vec::with_capacity(6 * side * side),
        normals: Vec::with_capacity(6 * side * side),
        colours: Vec::with_capacity(6 * side * side),
        indices: Vec::with_capacity(6 * cells * cells * 6),
    };
    let detail = detail_cells();
    for (face_index, (positions, normals, colours)) in faces.into_iter().enumerate() {
        let base = (face_index * side * side) as u32;
        surface.positions.extend(positions);
        surface.normals.extend(normals);
        surface.colours.extend(colours);
        for j in 0..cells {
            for i in 0..cells {
                if face_index == DETAIL_FACE && detail.contains(&i) && detail.contains(&j) {
                    continue;
                }
                let corner = base + (j * side + i) as u32;
                let (right, above) = (corner + 1, corner + side as u32);
                surface
                    .indices
                    .extend([corner, right, above, right, above + 1, above]);
            }
        }
    }
    surface.into_mesh()
}

/// The finer ground around the start. Its edge follows the coarse mesh exactly, so the
/// two join, and a strip hanging down from the edge covers any hairline gap between them.
fn detail_mesh(planet: &Planet) -> Mesh {
    let cells = DETAIL_CELLS * DETAIL_DIVISIONS;
    let side = cells + 1;
    let first = detail_cells().start as f32;
    let row = |j: usize| -> Vec<Vec3> {
        (0..side)
            .map(|i| {
                let x = first + i as f32 / DETAIL_DIVISIONS as f32;
                let y = first + j as f32 / DETAIL_DIVISIONS as f32;
                let (a, b) = (
                    x / CELLS_PER_FACE as f32 * 2.0 - 1.0,
                    y / CELLS_PER_FACE as f32 * 2.0 - 1.0,
                );
                let direction = face_direction(DETAIL_FACE, a, b);
                let trueness = detail_trueness(x, y);
                if trueness >= 1.0 {
                    planet.surface(direction)
                } else {
                    planet
                        .coarse(DETAIL_FACE, a, b, direction)
                        .lerp(planet.surface(direction), trueness)
                }
            })
            .collect()
    };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let rows: Vec<usize> = (0..side).collect();
    let positions: Vec<Vec3> = std::thread::scope(|scope| {
        let handles: Vec<_> = rows
            .chunks(side.div_ceil(threads))
            .map(|rows| scope.spawn(move || rows.iter().flat_map(|&j| row(j)).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect()
    });

    // Which way the ground faces comes from its neighbours in the grid.
    let at = |i: usize, j: usize| positions[j * side + i];
    let mut normals = Vec::with_capacity(side * side + 4 * side);
    let mut colours = Vec::with_capacity(side * side + 4 * side);
    for j in 0..side {
        for i in 0..side {
            let across = at((i + 1).min(cells), j) - at(i.saturating_sub(1), j);
            let up = at(i, (j + 1).min(cells)) - at(i, j.saturating_sub(1));
            let point = at(i, j);
            let mut normal = across.cross(up).normalize();
            if normal.dot(point) < 0.0 {
                normal = -normal;
            }
            normals.push(normal);
            colours.push(cover_colour(planet, point, normal));
        }
    }
    let mut indices = Vec::with_capacity(cells * cells * 6 + 4 * side * 12);
    for j in 0..cells {
        for i in 0..cells {
            let corner = (j * side + i) as u32;
            let (right, above) = (corner + 1, corner + side as u32);
            indices.extend([corner, right, above, right, above + 1, above]);
        }
    }

    // The strip hanging from the edge: a copy of each edge vertex, a little lower.
    let mut positions = positions;
    let edge: Vec<usize> = (0..side)
        .chain((0..side).map(|j| j * side + cells))
        .chain((0..side).rev().map(|i| cells * side + i))
        .chain((0..side).rev().map(|j| j * side))
        .collect();
    let lowered_from = positions.len() as u32;
    for &vertex in &edge {
        let point = positions[vertex];
        positions.push(point - point.normalize() * 1.5);
        normals.push(normals[vertex]);
        colours.push(colours[vertex]);
    }
    for (n, pair) in edge.windows(2).enumerate() {
        let (a, b) = (pair[0] as u32, pair[1] as u32);
        let (below_a, below_b) = (lowered_from + n as u32, lowered_from + n as u32 + 1);
        // Both ways round, so it is seen from either side.
        indices.extend([a, below_a, b, b, below_a, below_b]);
        indices.extend([a, b, below_a, b, below_b, below_a]);
    }

    Surface {
        positions,
        normals,
        colours,
        indices,
    }
    .into_mesh()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planet() -> Planet {
        Planet::new(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        })
    }

    #[test]
    fn a_direction_maps_to_a_face_and_back() {
        let mut rng = Seed(3).rng();
        for _ in 0..500 {
            let direction = rng.unit_vector();
            let (face, a, b) = face_coordinates(direction);
            assert!(a.abs() <= 1.0001 && b.abs() <= 1.0001);
            assert!(face_direction(face, a, b).distance(direction) < 1e-5);
        }
    }

    #[test]
    fn the_drawn_ground_is_true_near_the_start_and_coarse_far_away() {
        let planet = planet();
        let near = Vec3::new(0.02, 1.0, -0.03).normalize();
        assert_eq!(planet.drawn(near), planet.surface(near));

        // Far away, the drawn ground passes through the coarse mesh's corners and lies
        // between them elsewhere, which is not where the true ground is.
        let corner = face_direction(0, 0.25, -0.5);
        assert!(planet.drawn(corner).distance(planet.surface(corner)) < 0.01);
        let mut rng = Seed(4).rng();
        let mut worst: f32 = 0.0;
        for _ in 0..200 {
            let direction = (Vec3::X + rng.unit_vector() * 0.4).normalize();
            let drawn = planet.drawn(direction);
            assert!(drawn.normalize().distance(direction) < 1e-5);
            worst = worst.max(drawn.distance(planet.surface(direction)));
        }
        assert!(worst > 0.2 && worst < 15.0, "worst difference {worst}");
    }

    #[test]
    fn the_finer_ground_meets_the_coarse_mesh_at_its_edge() {
        let planet = planet();
        let first = detail_cells().start as f32;
        // Along the edge, the drawn ground is the coarse one, from either side.
        for step in 0..=64 {
            let y = first + step as f32 * 0.37;
            let direction = |x: f32| {
                face_direction(
                    DETAIL_FACE,
                    x / CELLS_PER_FACE as f32 * 2.0 - 1.0,
                    y / CELLS_PER_FACE as f32 * 2.0 - 1.0,
                )
            };
            let inside = planet.drawn(direction(first + 1e-3));
            let outside = planet.drawn(direction(first - 1e-3));
            assert!(inside.distance(outside) < 0.05);
        }
    }
}
