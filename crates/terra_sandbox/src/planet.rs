//! The stand-in planet on screen: its ground, its sea, the sun, and the camera.
//!
//! The planet itself comes from `terra_core`; this only draws it. The ground's look is in
//! `assets/shaders/ground.wgsl`, which reloads when saved.

use crate::shell::Options;
use bevy::{
    asset::RenderAssetUsages,
    camera_controller::free_camera::FreeCamera,
    input::common_conditions::input_toggle_active,
    mesh::Indices,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, PrimitiveTopology},
    shader::ShaderRef,
};
use bevy_inspector_egui::{prelude::*, quick::ResourceInspectorPlugin};
use std::sync::Arc;
use terra_core::{Ground, PlanetSpec, Seed, StandInPlanet, Water};

/// Height of the tallest hills, in metres.
const RELIEF_M: f32 = 150.0;
/// Grid squares along each edge of the six faces the planet's mesh is built from.
const CELLS_PER_FACE: usize = 256;
/// The camera starts here. The free camera treats +Y as up, so the pole is the one place
/// on the planet where that matches the ground.
const START: Vec3 = Vec3::Y;

/// The planet every experiment shares. Ask it where the ground is.
#[derive(Resource, Clone)]
pub struct Planet {
    pub ground: Arc<StandInPlanet>,
}

impl Planet {
    /// The point on the ground in a direction from the centre.
    pub fn surface(&self, direction: Vec3) -> Vec3 {
        direction * (self.ground.spec().radius_m + self.ground.height_m(direction))
    }

    /// Where the camera starts: a point on the ground.
    pub fn start(&self) -> Vec3 {
        self.surface(START)
    }
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
    pub ground_low: Color,
    pub ground_high: Color,
    pub ground_steep: Color,
    /// Light each triangle as a flat facet instead of smoothing across them.
    pub ground_faceted: bool,
}

#[derive(Component)]
struct Sea;

#[derive(Component)]
struct Sun;

type GroundMaterial = ExtendedMaterial<StandardMaterial, GroundExtension>;

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
}

impl MaterialExtension for GroundExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/ground.wgsl".into()
    }
}

pub fn plugin(app: &mut App) {
    let radius_m = app.world().resource::<Options>().radius_m;
    let spec = PlanetSpec {
        seed: Seed(1),
        radius_m,
    };
    let ground = Arc::new(StandInPlanet::new(spec, RELIEF_M, Some(-20.0)));
    let tweaks = Tweaks {
        sun_direction: Vec3::new(0.5, 0.75, 0.45),
        sun_color: Color::WHITE,
        sun_illuminance: 12_000.0,
        sea: true,
        sea_level_m: ground.level(START).map_or(0.0, |level| level - radius_m),
        ground_low: Color::srgb(0.42, 0.33, 0.26),
        ground_high: Color::srgb(0.72, 0.66, 0.58),
        ground_steep: Color::srgb(0.3, 0.27, 0.25),
        ground_faceted: false,
    };
    app.insert_resource(Planet { ground })
        .insert_resource(tweaks)
        .insert_resource(GlobalAmbientLight {
            brightness: 120.0,
            ..default()
        })
        .register_type::<Tweaks>()
        .add_plugins(MaterialPlugin::<GroundMaterial>::default())
        .add_plugins(
            ResourceInspectorPlugin::<Tweaks>::default()
                .run_if(input_toggle_active(false, KeyCode::F2)),
        )
        .add_systems(Startup, setup)
        .add_systems(Update, apply_tweaks.run_if(resource_changed::<Tweaks>));
}

fn setup(
    mut commands: Commands,
    planet: Res<Planet>,
    tweaks: Res<Tweaks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ground_materials: ResMut<Assets<GroundMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let radius_m = planet.ground.spec().radius_m;
    commands.spawn((
        Name::new("Ground"),
        Mesh3d(meshes.add(ground_mesh(&planet.ground, CELLS_PER_FACE))),
        MeshMaterial3d(ground_materials.add(ExtendedMaterial {
            base: StandardMaterial {
                perceptual_roughness: 0.95,
                ..default()
            },
            extension: GroundExtension {
                radius_m,
                relief_m: RELIEF_M,
                low: tweaks.ground_low.into(),
                high: tweaks.ground_high.into(),
                steep: tweaks.ground_steep.into(),
                faceted: tweaks.ground_faceted as u32,
            },
        })),
    ));
    commands.spawn((
        Name::new("Sea"),
        Sea,
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(48).unwrap())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.03, 0.14, 0.32),
            perceptual_roughness: 0.12,
            metallic: 0.2,
            ..default()
        })),
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
    mut sun: Single<(&mut DirectionalLight, &mut Transform), (With<Sun>, Without<Sea>)>,
    mut sea: Single<(&mut Transform, &mut Visibility), With<Sea>>,
    mut ground_materials: ResMut<Assets<GroundMaterial>>,
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
        material.extension.low = tweaks.ground_low.into();
        material.extension.high = tweaks.ground_high.into();
        material.extension.steep = tweaks.ground_steep.into();
        material.extension.faceted = tweaks.ground_faceted as u32;
    }
}

/// The planet as one mesh: a cube with each face divided into a grid, pushed out into a
/// sphere, with every vertex moved to the ground's height.
fn ground_mesh(planet: &StandInPlanet, cells: usize) -> Mesh {
    // Each face is (outward, across, up), ordered so triangles face outward.
    const FACES: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::NEG_X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::Y, Vec3::X),
    ];
    let side = cells + 1;
    let radius_m = planet.spec().radius_m;
    let face = |(outward, across, up): (Vec3, Vec3, Vec3)| {
        let mut positions = Vec::with_capacity(side * side);
        let mut normals = Vec::with_capacity(side * side);
        for j in 0..side {
            for i in 0..side {
                let a = i as f32 / cells as f32 * 2.0 - 1.0;
                let b = j as f32 / cells as f32 * 2.0 - 1.0;
                let direction = (outward + across * a + up * b).normalize();
                let point = direction * (radius_m + planet.height_m(direction));
                positions.push(point);
                normals.push(planet.normal(point));
            }
        }
        (positions, normals)
    };
    // One thread per face; the heights are the slow part.
    let faces: Vec<(Vec<Vec3>, Vec<Vec3>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = FACES
            .iter()
            .map(|&f| scope.spawn(move || face(f)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });

    let mut positions = Vec::with_capacity(6 * side * side);
    let mut normals = Vec::with_capacity(6 * side * side);
    let mut indices = Vec::with_capacity(6 * cells * cells * 6);
    for (face_index, (face_positions, face_normals)) in faces.into_iter().enumerate() {
        let base = (face_index * side * side) as u32;
        positions.extend(face_positions);
        normals.extend(face_normals);
        for j in 0..cells {
            for i in 0..cells {
                let corner = base + (j * side + i) as u32;
                let (right, above) = (corner + 1, corner + side as u32);
                indices.extend([corner, right, above, right, above + 1, above]);
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}
