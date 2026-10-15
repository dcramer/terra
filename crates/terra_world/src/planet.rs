//! The planet on screen: its sea, the sun, and the camera; and the planet itself, which
//! everything asks about the ground.
//!
//! The ground is voxel terrain from `terra_voxel` (docs/terrain.md): a generated planet
//! that can be dug and filled. It is drawn in chunks by `terrain`. The ground's look is in
//! `assets/shaders/ground.wesl` and the sea's in `assets/shaders/water.wesl`; both reload
//! when saved.

use crate::shell::Options;
use crate::sky::Sky;
use bevy::{
    camera_controller::free_camera::FreeCamera,
    image::{ImageArrayLayout, ImageLoaderSettings},
    input::common_conditions::input_toggle_active,
    light::NotShadowCaster,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use bevy_inspector_egui::{prelude::*, quick::ResourceInspectorPlugin};
use std::sync::Arc;
use terra_core::{Ground, Operation, PlanetSpec, RayHit, Seed, Yield};
use terra_gen::Noise;
use terra_voxel::{GeneratedPlanet, VoxelPlanet};

// The ground as generated is the land's (`terra_land::ground`), so the tools that map its
// rules without the engine see the same ground.
use terra_land::ground::{RELIEF_M, ROLLING_M, SEA_LEVEL_M};
/// The camera starts here. The free camera treats +Y as up, so the pole is the one place
/// on the planet where that matches the ground.
const START: Vec3 = Vec3::Y;

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
    /// Shared, so chunks can be meshed on other threads; an edit makes a new copy when
    /// one is still being read.
    ground: Arc<VoxelPlanet<GeneratedPlanet>>,
    /// How high its hills rise, in metres.
    relief_m: f32,
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
    pub fn new(spec: PlanetSpec) -> Self {
        Planet {
            ground: Arc::new(VoxelPlanet::new(terra_land::ground::world(spec))),
            relief_m: RELIEF_M,
            cover: Noise::new(spec.seed.derive("ground cover")),
        }
    }

    /// A planet with no hills, no sea and no finer detail: ground as flat as a sphere is,
    /// for the yard, the test level.
    pub fn smooth(spec: PlanetSpec) -> Self {
        Planet {
            ground: Arc::new(VoxelPlanet::new(terra_land::ground::smooth(spec))),
            relief_m: 0.0,
            cover: Noise::new(spec.seed.derive("ground cover")),
        }
    }

    /// The yard's ground rolling gently, for a forest to stand on: swells a few metres high
    /// and smaller bumps on them, no sea, as the lush forest stands on.
    pub fn rolling(spec: PlanetSpec) -> Self {
        Planet {
            ground: Arc::new(VoxelPlanet::new(terra_land::ground::rolling(spec))),
            relief_m: ROLLING_M,
            cover: Noise::new(spec.seed.derive("ground cover")),
        }
    }

    /// Where something at a point stands: the nearest ground straight up or down from
    /// it. The point is a place in the world, not a direction, since there can be ground
    /// above as well as below, as in a hole that has been dug.
    pub fn ground_at(&self, point: Vec3) -> Vec3 {
        let up = point.normalize_or(Vec3::Y);
        let inside = self.ground.distance(point) < 0.0;
        // Out of the ground the surface is looked for below; in it, above. Each step is as
        // long as the ground is far, which on undug ground lands on it at once, and at
        // least a few centimetres, so the side of a hole is not crawled along.
        let toward = if inside { up } else { -up };
        let (mut near, mut far) = (point, point);
        for _ in 0..64 {
            let distance = self.ground.distance(far);
            if distance.abs() < 0.002 {
                return far;
            }
            if (distance < 0.0) != inside {
                break;
            }
            near = far;
            far += toward * distance.abs().max(0.05);
        }
        // The surface lies between the last step on this side and the first past it.
        for _ in 0..16 {
            let middle = (near + far) * 0.5;
            if (self.ground.distance(middle) < 0.0) == inside {
                near = middle;
            } else {
                far = middle;
            }
        }
        (near + far) * 0.5
    }

    /// Whether the ground at a point has been dug or filled, rather than lying where it was
    /// generated: nothing that grew there stands on it any more.
    pub fn dug(&self, point: Vec3) -> bool {
        let up = point.normalize_or(Vec3::Y);
        (self.ground_at(point).length() - self.surface(up).length()).abs() > 0.15
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

    /// Digs or fills the ground, and says what came out. Returns nothing taken for an
    /// operation that does not shape the ground.
    pub fn apply(&mut self, operation: &Operation) -> Yield {
        Arc::make_mut(&mut self.ground).apply(operation)
    }

    /// The operations that have shaped the ground, in order: what a save keeps.
    pub fn operations(&self) -> Vec<Operation> {
        self.ground.operations()
    }

    /// The voxel ground itself, for drawing it.
    pub fn voxels(&self) -> &Arc<VoxelPlanet<GeneratedPlanet>> {
        &self.ground
    }

    pub fn radius_m(&self) -> f32 {
        self.ground.field().spec().radius_m
    }

    pub fn seed(&self) -> Seed {
        self.ground.field().spec().seed
    }

    /// How tall the tallest hills are, in metres.
    pub fn relief_m(&self) -> f32 {
        self.relief_m
    }

    /// The point on the ground as it was generated, in a direction from the centre: before
    /// any digging, which is right for laying out a world.
    pub fn surface(&self, direction: Vec3) -> Vec3 {
        direction * (self.radius_m() + self.ground.field().height_m(direction))
    }

    /// Where the camera starts: a point on the ground.
    pub fn start(&self) -> Vec3 {
        self.surface(START)
    }

    /// Distance from the planet's centre to the surface of the sea.
    pub fn sea_radius_m(&self) -> f32 {
        self.radius_m() + SEA_LEVEL_M
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
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

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

/// The camera the world is seen through. Other cameras, such as the one that draws item
/// icons (`icons`), draw into images; queries for the view ask for this one.
#[derive(Component)]
#[require(crate::parting::CameraSubject)]
pub struct MainCamera;

pub type GroundMaterial = ExtendedMaterial<StandardMaterial, GroundExtension>;

/// What the ground is drawn with: every chunk of it shares this.
#[derive(Resource)]
pub struct GroundLook(pub Handle<GroundMaterial>);
type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExtension>;

/// What the ground shader is told. Field order must match the struct in ground.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, PartialEq)]
#[bind_group_data(GroundKey)]
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
    /// How many metres each layer of the ground's textures covers: soil, grass, rock, sand.
    #[uniform(100)]
    layer_metres: Vec4,
    /// Whether the ground is drawn with its textures, or in flat colours.
    #[uniform(100)]
    textured: u32,
    /// How many metres the fifth layer on, leaf litter, needles and roots, each cover.
    #[uniform(100)]
    more_metres: Vec4,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    color: Option<Handle<Image>>,
    #[texture(103, dimension = "2d_array")]
    normal: Option<Handle<Image>>,
    /// What covers the ground near the camera, from its cover: from a point in the world
    /// to its place on the map.
    #[uniform(100)]
    cover_from_world: Mat4,
    /// Whether there is a cover map to read.
    #[uniform(100)]
    cover_on: u32,
    /// Drawn as the low quality preset draws it (D239, D307): the shader built with
    /// `GROUND_LOW`, two layers a pixel, one or two projections, flat colours far off.
    cheap: bool,
    #[texture(104)]
    #[sampler(105)]
    cover_map: Option<Handle<Image>>,
    #[texture(106)]
    #[sampler(107)]
    cover_more: Option<Handle<Image>>,
}

/// What covers the ground near the camera, painted by the ground cover (`cover`) for the
/// ground to draw under it, as Horizon Zero Dawn's placement paints its terrain: grass in
/// red, how lush in green, forest floor (leaf litter) in blue, moss in alpha; and in a
/// second picture fallen needles in red, duff and roots in green, and how much canopy
/// stands over in blue. The ground owns the question; the cover answers it.
#[derive(Resource, Clone, Default)]
pub struct GroundCoverMap {
    pub image: Option<Handle<Image>>,
    pub more: Option<Handle<Image>>,
    /// From a point in the world to the map: its x and y are where on the picture, from 0
    /// to 1.
    pub from_world: Mat4,
}

/// The ground's textures (D14): real soil, grass, rock and sand, each tinted by the
/// planet's colours for it, so the planet still decides what colour its ground is.
#[derive(Resource, Clone)]
pub struct GroundTextures {
    color: Handle<Image>,
    normal: Handle<Image>,
}

/// The layers of `materials/ground`, in order, and how many metres each covers
/// (assets/materials/README.md).
const GROUND_LAYERS: u32 = 8;
const GROUND_LAYER_METRES: Vec4 = Vec4::new(1.3, 2.0, 2.7, 1.5);
/// The fifth layer on, for the forest floor: fallen leaves, fallen needles, duff with roots
/// in it, and moss.
const GROUND_MORE_METRES: Vec4 = Vec4::new(2.0, 1.5, 1.67, 0.7);

impl GroundTextures {
    /// The layers' colours and normal maps, as arrays, for materials drawn in them too, as
    /// the lush forest's moss on its rocks is (D294).
    pub fn color(&self) -> &Handle<Image> {
        &self.color
    }

    pub fn normal(&self) -> &Handle<Image> {
        &self.normal
    }

    fn load(assets: &AssetServer) -> Self {
        let layers = |linear: bool| {
            move |settings: &mut ImageLoaderSettings| {
                settings.array_layout = Some(ImageArrayLayout::RowCount {
                    rows: GROUND_LAYERS,
                });
                settings.is_srgb = !linear;
            }
        };
        GroundTextures {
            color: assets
                .load_builder()
                .with_settings(layers(false))
                .load("materials/ground/color.jpg"),
            normal: assets
                .load_builder()
                .with_settings(layers(true))
                .load("materials/ground/normal.jpg"),
        }
    }
}

/// Which of the ground's shaders a material is drawn with: the low preset's is another
/// build of it, so the high one's work is not even branched past.
#[repr(C)]
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct GroundKey {
    cheap: bool,
}

impl From<&GroundExtension> for GroundKey {
    fn from(ground: &GroundExtension) -> Self {
        GroundKey {
            cheap: ground.cheap,
        }
    }
}

impl MaterialExtension for GroundExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/ground.wesl".into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if key.bind_group_data.cheap
            && let Some(fragment) = descriptor.fragment.as_mut()
        {
            fragment.shader_defs.push("GROUND_LOW".into());
        }
        Ok(())
    }
}

impl GroundExtension {
    fn new(
        planet: &Planet,
        tweaks: &Tweaks,
        textures: &GroundTextures,
        cover: &GroundCoverMap,
        quality: crate::quality::Quality,
    ) -> Self {
        let radius_m = planet.radius_m();
        GroundExtension {
            radius_m,
            relief_m: planet.relief_m(),
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
            layer_metres: GROUND_LAYER_METRES,
            more_metres: GROUND_MORE_METRES,
            // Textures are for ground that is covered; the bare stand-in stays flat.
            textured: (tweaks.ground_cover > 0.0) as u32,
            color: Some(textures.color.clone()),
            normal: Some(textures.normal.clone()),
            cover_from_world: cover.from_world,
            cover_on: cover.image.is_some() as u32,
            cheap: quality == crate::quality::Quality::Low,
            // Bevy's blank picture stands in for a map until there is one.
            cover_map: Some(cover.image.clone().unwrap_or_default()),
            cover_more: Some(cover.more.clone().unwrap_or_default()),
        }
    }
}

/// What the water shader is told. Field order must match the struct in water.wesl.
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
        "shaders/water.wesl".into()
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
    let yard = app.world().resource::<Options>().yard();
    let rolling = app.world().resource::<Options>().rolling;
    let planet = if yard && rolling {
        Planet::rolling(spec)
    } else if yard {
        Planet::smooth(spec)
    } else {
        Planet::new(spec)
    };
    let tweaks = Tweaks {
        sun_direction: Vec3::new(0.5, 0.75, 0.45),
        sun_color: Color::WHITE,
        sun_illuminance: 12_000.0,
        sea: !yard,
        sea_level_m: SEA_LEVEL_M,
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
    let start = crate::start::Start::choose(&planet, yard);
    app.insert_resource(planet)
        .insert_resource(start)
        .insert_resource(tweaks)
        .init_resource::<GroundCoverMap>()
        // The ground is drawn at a quality, chosen by the world layer when it runs; the
        // default otherwise, as on the workbench.
        .init_resource::<crate::quality::Quality>()
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
            apply_tweaks.run_if(
                resource_changed::<Tweaks>
                    .or_eager(resource_exists_and_changed::<Sky>)
                    .or_eager(resource_changed::<GroundCoverMap>)
                    .or_eager(resource_changed::<crate::quality::Quality>),
            ),
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
    assets: Res<AssetServer>,
) {
    let radius_m = planet.radius_m();
    let textures = GroundTextures::load(&assets);
    let ground_material = ground_materials.add(ExtendedMaterial {
        base: StandardMaterial {
            perceptual_roughness: 0.95,
            reflectance: 0.2,
            ..default()
        },
        extension: GroundExtension::new(
            &planet,
            &tweaks,
            &textures,
            &GroundCoverMap::default(),
            crate::quality::Quality::default(),
        ),
    });
    commands.insert_resource(textures);
    // The ground itself is drawn in chunks, by `terrain`, with this look.
    commands.insert_resource(GroundLook(ground_material));
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
        MainCamera,
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
    textures: Res<GroundTextures>,
    cover: Res<GroundCoverMap>,
    quality: Res<crate::quality::Quality>,
) {
    let toward_sun = tweaks.sun_direction.normalize_or(Vec3::Y);
    sun.0.color = tweaks.sun_color;
    sun.0.illuminance = tweaks.sun_illuminance;
    *sun.1 = Transform::from_translation(toward_sun).looking_at(Vec3::ZERO, Vec3::Y);

    sea.0.scale = Vec3::splat(planet.radius_m() + tweaks.sea_level_m);
    *sea.1 = if tweaks.sea {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    // The ground changes only when its tweaks do, not with the sun, so it is left alone
    // otherwise: a changed material is prepared again, and every chunk drawn with it.
    let ground = GroundExtension::new(&planet, &tweaks, &textures, &cover, *quality);
    let stale: Vec<_> = ground_materials
        .iter()
        .filter(|(_, material)| material.extension != ground)
        .map(|(id, _)| id)
        .collect();
    for id in stale {
        if let Some(mut material) = ground_materials.get_mut(id) {
            material.extension = ground.clone();
        }
    }
    for (_, material) in water_materials.iter_mut() {
        material.base.base_color = tweaks.sea_shallow;
        material.extension = WaterExtension::new(sky.as_deref());
    }
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
    fn something_stands_on_the_ground_from_above_or_below() {
        let planet = planet();
        let ground = planet.start();
        let up = ground.normalize();
        assert!(planet.ground_at(ground + up * 7.0).distance(ground) < 0.01);
        assert!(planet.ground_at(ground - up * 3.0).distance(ground) < 0.01);
    }

    #[test]
    fn in_a_hole_it_stands_on_the_bottom() {
        let mut planet = planet();
        let ground = planet.start();
        let up = ground.normalize();
        let taken = planet.apply(&Operation::Dig {
            centre: ground,
            radius: 2.0,
        });
        assert!(!taken.is_empty());
        let floor = planet.ground_at(ground + up * 0.5);
        assert!(
            (floor.distance(ground) - 2.0).abs() < 0.05,
            "{}",
            floor.distance(ground)
        );
        assert!(planet.distance(ground) > 1.9);
        assert!(planet.dug(ground));
        assert!(!planet.dug(planet.surface((ground + Vec3::X * 20.0).normalize())));
    }

    /// Directions spread evenly over the sphere (a Fibonacci lattice).
    fn directions(count: usize) -> impl Iterator<Item = Vec3> {
        (0..count).map(move |i| {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
            let around = i as f32 * 2.399_963;
            let flat = libm::sqrtf(1.0 - y * y);
            Vec3::new(flat * libm::cosf(around), y, flat * libm::sinf(around))
        })
    }

    #[test]
    fn no_grass_grows_under_the_sea_or_on_a_cliff() {
        let planet = planet();
        let sea = planet.sea_radius_m();
        let mut grassy = 0;
        for up in directions(400) {
            // Level ground a few metres above the sea, where grass can grow.
            let shore = up * (sea + 3.0);
            if planet.cover(shore, up).grass <= 0.0 {
                continue;
            }
            grassy += 1;
            // The same place under the sea, and as a cliff, facing sideways.
            assert_eq!(planet.cover(up * (sea - 1.0), up).grass, 0.0);
            assert_eq!(planet.cover(shore, up.any_orthonormal_vector()).grass, 0.0);
        }
        assert!(grassy > 40, "only {grassy} places had grass to lose");
    }

    #[test]
    fn under_the_sea_is_where_the_ground_is_below_the_water_not_the_point() {
        let planet = planet();
        let sea = planet.sea_radius_m();
        let ground = |up: Vec3| planet.surface(up).length();
        let wet = directions(4000).find(|&up| ground(up) < sea - 2.0).unwrap();
        let dry = directions(4000)
            .find(|&up| ground(up) > sea + 10.0)
            .unwrap();
        // Up in the air over the sea floor, and down in the rock under a hill.
        assert!(planet.under_sea(wet * (sea + 50.0)));
        assert!(planet.under_sea(wet * (ground(wet) + 1.0)));
        assert!(!planet.under_sea(dry * (sea - 5.0)));
        assert!(!planet.under_sea(dry * (ground(dry) + 1.0)));
    }

    /// A pit dug on a slant, as a pickaxe aimed ahead and down digs it, and the ground
    /// found beside it is never above where the ground was before.
    #[test]
    fn beside_a_slanting_pit_the_ground_is_where_it_is() {
        let mut planet = planet();
        let start = planet.start();
        let up = start.normalize();
        let ahead = up.any_orthonormal_vector();
        let side = up.cross(ahead);
        for step in 0..5 {
            let centre = start + ahead * (step as f32 * 0.6) - up * (0.2 + step as f32 * 0.7);
            planet.apply(&Operation::Dig {
                centre,
                radius: 0.7,
            });
        }
        for along in 0..20 {
            for across in 0..10 {
                let place =
                    start + ahead * (along as f32 * 0.2 - 1.0) + side * (across as f32 * 0.1);
                for height in [-0.4, 0.0, 0.5] {
                    let found = planet.ground_at(place + up * height);
                    let before = planet.surface(found.normalize()).length();
                    assert!(
                        found.length() <= before + 0.01,
                        "{} above {before}",
                        found.length()
                    );
                    assert!(planet.distance(found).abs() < 0.01);
                }
            }
        }
    }
}
