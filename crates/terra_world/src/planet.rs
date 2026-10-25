//! The planet on screen: its sea, the sun, and the camera; and the planet itself, which
//! everything asks about the ground.
//!
//! The ground is voxel terrain from `terra_voxel` (docs/terrain.md): a generated planet
//! that can be dug and filled. It is drawn in chunks by `terrain`. The ground's look is in
//! `assets/shaders/ground.wesl` and the sea's in `assets/shaders/water.wesl`; both reload
//! when saved.

use crate::shell::Options;
use crate::sky::Sky;
use sea::{WaterExtension, sea_waves};

mod sea;
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
use terra_gen::smoothstep;
use terra_voxel::{GeneratedPlanet, VoxelPlanet};

// The ground as generated is the land's (`terra_land::ground`), so the tools that map its
// rules without the engine see the same ground.
use terra_land::ground::{RELIEF_M, ROLLING_M, SEA_LEVEL_M};
/// The camera starts here. The free camera treats +Y as up, so the pole is the one place
/// on the planet where that matches the ground.
const START: Vec3 = Vec3::Y;

/// How far round the start the land is worked out ahead on a world with mountains (D322):
/// its lakes, the seasons its trees follow and its snow, as far as one sees its woods and
/// peaks clearly.
const ROUND_M: f32 = 16_000.0;

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

/// Where something at a point stands on a ground that says how far a point is from it
/// (`distance`): the nearest surface straight up or down from it, toward or away from the
/// centre. `Planet::ground_at` asks it of the voxel ground; the controller course (D53,
/// `terra_motion::course`) of its fixtures.
pub fn surface_through(point: Vec3, distance: impl Fn(Vec3) -> f32) -> Vec3 {
    let up = point.normalize_or(Vec3::Y);
    let inside = distance(point) < 0.0;
    // Out of the ground the surface is looked for below; in it, above. Each step is as
    // long as the ground is far, which on undug ground lands on it at once, and at
    // least a few centimetres, so the side of a hole is not crawled along.
    let toward = if inside { up } else { -up };
    let (mut near, mut far) = (point, point);
    for _ in 0..64 {
        let away = distance(far);
        if away.abs() < 0.002 {
            return far;
        }
        if (away < 0.0) != inside {
            break;
        }
        near = far;
        far += toward * away.abs().max(0.05);
    }
    // The surface lies between the last step on this side and the first past it.
    for _ in 0..16 {
        let middle = (near + far) * 0.5;
        if (distance(middle) < 0.0) == inside {
            near = middle;
        } else {
            far = middle;
        }
    }
    (near + far) * 0.5
}

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
    /// The land round the start worked out ahead: its lakes filled to their spill points, its
    /// seasons and its snow (D322); none
    /// asked for, the sea alone.
    round: Option<Arc<terra_land::round::Round>>,
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
        // The land lays the ground's patterned ground where its climate keeps it frozen.
        let climate: terra_climate::Climate =
            crate::data::read(crate::daylight::WorldClimate::FILE);
        let ground = terra_land::Land::new(terra_land::ground::world(spec), climate).ground;
        // Its hills, and on a world large enough its mountains (D322).
        let relief_m = RELIEF_M
            + ground
                .mountains()
                .map_or(0.0, |range| range.mountains.highest_m());
        Planet {
            ground: Arc::new(VoxelPlanet::new(ground)),
            relief_m,
            cover: Noise::new(spec.seed.derive("ground cover")),
            round: None,
        }
    }

    /// A planet with no hills, no sea and no finer detail: ground as flat as a sphere is,
    /// for the yard, the test level.
    pub fn smooth(spec: PlanetSpec) -> Self {
        Planet {
            ground: Arc::new(VoxelPlanet::new(terra_land::ground::smooth(spec))),
            relief_m: 0.0,
            cover: Noise::new(spec.seed.derive("ground cover")),
            round: None,
        }
    }

    /// The yard's ground rolling gently, for a forest to stand on: swells a few metres high
    /// and smaller bumps on them, no sea, as the lush forest stands on.
    pub fn rolling(spec: PlanetSpec) -> Self {
        Planet {
            ground: Arc::new(VoxelPlanet::new(terra_land::ground::rolling(spec))),
            relief_m: ROLLING_M,
            cover: Noise::new(spec.seed.derive("ground cover")),
            round: None,
        }
    }

    /// Where something at a point stands: the nearest ground straight up or down from
    /// it. The point is a place in the world, not a direction, since there can be ground
    /// above as well as below, as in a hole that has been dug.
    pub fn ground_at(&self, point: Vec3) -> Vec3 {
        surface_through(point, |at| self.ground.distance(at))
    }

    /// Whether the ground at a point has been dug or filled, rather than lying where it was
    /// generated: nothing that grew there stands on it any more.
    pub fn dug(&self, point: Vec3) -> bool {
        let up = point.normalize_or(Vec3::Y);
        (self.ground_at(point).length() - self.surface(up).length()).abs() > 0.15
    }

    /// Whether the ground at a point is under water: the sea, or a lake (D322).
    pub fn under_sea(&self, point: Vec3) -> bool {
        self.ground_at(point).length() < self.water_radius_m(point)
    }

    /// The distance from the centre to the water's surface over a point: a lake's where a
    /// lake is, the sea's elsewhere (`terra_core::Water`, D45). Where there is no water at all
    /// it is the sea's level all the same, under the ground.
    pub fn water_radius_m(&self, point: Vec3) -> f32 {
        self.round
            .as_ref()
            .and_then(|round| round.water_level(point.normalize_or(Vec3::Y)))
            .unwrap_or(self.sea_radius_m())
    }

    /// The highest water's surface at or beside a point, a lake's cell on (D322): what grows
    /// keeps out of it, since a lake's drawn water reaches a cell past its own.
    pub fn water_near_m(&self, point: Vec3) -> f32 {
        let sea = self.sea_radius_m();
        self.round
            .as_ref()
            .and_then(|round| round.water_near(point.normalize_or(Vec3::Y), 1))
            .map_or(sea, |lake| lake.max(sea))
    }

    /// The lakes found round the start, if the land round it was looked at.
    pub fn waters(&self) -> Option<&terra_land::water::Waters> {
        self.round.as_ref().map(|round| &round.waters)
    }

    /// The land round the start, its rules worked out ahead (lakes, seasons, snow), if it
    /// was looked at.
    pub fn round(&self) -> Option<&Arc<terra_land::round::Round>> {
        self.round.as_ref()
    }

    /// Works out the land round a point, so many metres across, ahead (D322): every hollow of
    /// the ground as generated filled to its spill point, the growing seasons its trees follow
    /// and the snow lying on `day`.
    pub fn look_round(&mut self, around: Vec3, across_m: f32, day: i64) {
        let climate: terra_climate::Climate =
            crate::data::read(crate::daylight::WorldClimate::FILE);
        let land = terra_land::Land::new(self.ground.field().clone(), climate);
        self.round = Some(Arc::new(terra_land::round::Round::new(
            land, around, across_m, day,
        )));
    }

    /// Clears the woods of the land round the place from where settlements have their fields
    /// (`terra_land::town::Clearing`), once it is worked out and before anything stands on it.
    pub fn clear_woods(
        &mut self,
        clearings: impl FnOnce(&terra_land::Land) -> Vec<terra_land::town::Clearing>,
    ) {
        if let Some(round) = self.round.as_mut().and_then(Arc::get_mut) {
            round.clearings = clearings(&round.land);
        }
    }

    /// Which way the ground faces at a point on it.
    pub fn normal(&self, point: Vec3) -> Vec3 {
        self.ground.normal(point)
    }

    /// How far a point is above the ground, below it if negative.
    pub fn distance(&self, point: Vec3) -> f32 {
        self.ground.distance(point)
    }

    /// How much further than its true distance the ground can be from a point, as a factor
    /// (`terra_voxel::Field::steepest`): a sweep by [`Planet::distance`] advances by the
    /// distance over this, so it never steps past a surface (D53).
    pub fn steepest(&self) -> f32 {
        terra_voxel::Field::steepest(&*self.ground)
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

    /// Whether the planet has mountains (D322), whose bare rock, scree and gravel the ground is
    /// drawn with.
    pub fn has_mountains(&self) -> bool {
        self.ground.field().mountains().is_some()
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
        let (water, lowland) = crate::shore::near(self, point);
        let steepness = 1.0 - normal.dot(point.normalize());
        let patches = self.cover.fbm(point / 70.0, 3, 2.0, 0.5);
        // Past the trees the heath holds the ground, not a meadow's grass (`terra_land::heath`).
        let heath = self
            .round
            .as_ref()
            .map_or(0.0, |round| round.heath.at(point).open);
        Cover {
            grass: (1.0 - heath)
                * smoothstep(-0.4, -0.26, patches)
                * crate::shore::grass(point.length() - water, lowland)
                * (1.0 - smoothstep(0.14, 0.27, steepness)),
            lush: smoothstep(
                -0.35,
                0.35,
                self.cover.fbm(point / 45.0 + 31.0, 2, 2.0, 0.5),
            ) * (1.0 - MOUNTAIN_STRAW * self.mountain_straw(point, normal)),
        }
    }

    /// How much of a mountain meadow's grass is last year's straw, 0 to 1: under a short
    /// growing season the old leaves stand in the tussocks among the new into high summer,
    /// so a subalpine meadow reads olive and straw, not a lowland's fresh green; full from
    /// the tree line's season, none from `LOWLAND_MEADOW_C` over it. Off a world with
    /// mountains, none.
    fn mountain_straw(&self, point: Vec3, normal: Vec3) -> f32 {
        let Some(round) = self.round.as_ref() else {
            return 0.0;
        };
        let place = round.land.place(point, normal);
        let season = round
            .seasons
            .at(place.latitude, place.height_m, place.sunward);
        let over = season.growing_mean_c - terra_climate::TREE_LINE_SEASON_C;
        1.0 - smoothstep(0.0, LOWLAND_MEADOW_C, over)
    }
}

/// How far a growing season warmer than the tree line's, in °C, a meadow is all fresh green;
/// and how much of its lushness the straw takes at most.
const LOWLAND_MEADOW_C: f32 = 10.0;
const MOUNTAIN_STRAW: f32 = 0.8;

#[derive(Resource, Reflect, InspectorOptions)]
#[reflect(Resource, InspectorOptions)]
pub struct Tweaks {
    /// Direction from the planet toward the sun.
    pub sun_direction: Vec3,
    pub sun_color: Color,
    #[inspector(min = 0.0, max = 100_000.0)]
    pub sun_illuminance: f32,
    /// Direction toward where the world's light comes from: the sun's, or under a deck turned
    /// up toward the sky's brightest part, overhead (`daylight`). Zero for the sun's own.
    pub light_direction: Vec3,
    /// How wide the world's light is as a source, for its shadows' penumbrae (Bevy's PCSS):
    /// zero for the sun's crisp edges, wide under a deck, whose light comes from the whole sky
    /// (`daylight`).
    pub light_size: f32,
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
pub(crate) type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExtension>;

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
    /// How many metres the ninth layer on, scree, gravel and snow, each cover.
    #[uniform(100)]
    mountain_metres: Vec4,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    color: Option<Handle<Image>>,
    #[texture(103, dimension = "2d_array")]
    normal: Option<Handle<Image>>,
    /// What covers the ground near the camera, from its cover: from a point in the world
    /// to its place on the map.
    #[uniform(100)]
    cover_from_world: Mat4,
    /// Whether there is a cover map to read: 1 its grass the most the land's may grow, 2 the
    /// ground's own (`GroundCoverMap::sows`).
    #[uniform(100)]
    cover_on: u32,
    /// Drawn as the low quality preset draws it (D239, D307): the shader built with
    /// `GROUND_LOW`, two layers a pixel, one or two projections, flat colours far off.
    cheap: bool,
    /// The cover's sampler serves every map from here on (ground.wesl's `map_sampler`):
    /// each is `ImageSampler::linear()`, and a stage may bind only 16 samplers.
    #[texture(104)]
    #[sampler(105)]
    cover_map: Option<Handle<Image>>,
    #[texture(106)]
    cover_more: Option<Handle<Image>>,
    #[texture(114)]
    cover_stood: Option<Handle<Image>>,
    /// What the climate leaves round the camera (`climate::ClimateMaps`, D327): from a point
    /// in the world to its place on the maps, whether there are maps, and the leaves map's
    /// layer for the broadleaves whose fallen leaves lie on it, or -1. The maps are painted
    /// again in place, so the material changes only when these do.
    #[uniform(100)]
    climate_from_world: Mat4,
    #[uniform(100)]
    climate_on: u32,
    #[uniform(100)]
    fallen_layer: i32,
    #[texture(108)]
    climate_ground: Option<Handle<Image>>,
    #[texture(110, dimension = "2d_array")]
    climate_leaves: Option<Handle<Image>>,
    /// The heath past the trees round the land's middle (`terra_land::heath`, `LandMap`):
    /// the middle's direction and the planet's radius; east and the map's width in metres;
    /// north and whether there is a map; the leaves map's layers of the heath's dwarf shrubs
    /// and of grass, for their colour now, and the shrubs' autumn colour.
    #[uniform(100)]
    land_middle: Vec4,
    #[uniform(100)]
    land_east: Vec4,
    #[uniform(100)]
    land_north: Vec4,
    #[uniform(100)]
    heath_layer: i32,
    #[uniform(100)]
    grass_layer: i32,
    #[uniform(100)]
    heath_autumn: Vec4,
    #[texture(112, dimension = "2d_array")]
    land_map: Option<Handle<Image>>,
    /// The land's horizon round the camera (`crate::horizon`, D416), read as the land map is:
    /// what the sun is under past the shadow map's reach is shaded.
    #[uniform(100)]
    horizon_middle: Vec4,
    #[uniform(100)]
    horizon_east: Vec4,
    #[uniform(100)]
    horizon_north: Vec4,
    #[texture(115, dimension = "2d_array")]
    horizon_map: Option<Handle<Image>>,
}

/// What covers the open ground past the trees round the land's middle, as the land's rules
/// make it (`terra_land::heath`), for the ground to draw: a picture of two layers over the
/// lakes' grid, the lichen, dwarf shrubs, sedge and moss in the first, the fellfield's stones,
/// the wetness and how open the ground is in the second.
#[derive(Resource, Clone, Default)]
pub struct LandMap {
    pub image: Option<Handle<Image>>,
    pub middle: Vec4,
    pub east: Vec4,
    pub north: Vec4,
}

impl LandMap {
    /// The picture of a place's heath.
    pub fn of(round: &terra_land::round::Round, images: &mut Assets<Image>) -> Self {
        use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
        let heath = &round.heath;
        let region = heath.region();
        let n = region.cells as u32;
        let mut image = Image::new(
            Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: 2,
            },
            TextureDimension::D2,
            heath.bytes(),
            TextureFormat::Rgba8Unorm,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = bevy::image::ImageSampler::linear();
        let size = region.cell_m * region.cells as f32;
        Self {
            image: Some(images.add(image)),
            middle: region.middle.normalize().extend(region.middle.length()),
            east: region.east.extend(size),
            north: region.north.extend(1.0),
        }
    }
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
    /// And a third: how near a trunk, log or rock stands, in red (`Growing::stands`).
    pub stood: Option<Handle<Image>>,
    /// From a point in the world to the map: its x and y are where on the picture, from 0
    /// to 1.
    pub from_world: Mat4,
    /// Whether its grass is the ground's own, not only a limit on the land's
    /// (`CoverSource::sows`).
    pub sows: bool,
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
const GROUND_LAYERS: u32 = 12;
const GROUND_LAYER_METRES: Vec4 = Vec4::new(1.3, 2.0, 2.7, 1.5);
/// The fifth layer on, for the forest floor: fallen leaves, fallen needles, duff with roots
/// in it, and moss.
const GROUND_MORE_METRES: Vec4 = Vec4::new(2.0, 1.5, 1.67, 0.7);
/// The ninth layer on, for mountains (D322): scree, gravel and snow; and a settlement's
/// cobbles (D350).
const GROUND_MOUNTAIN_METRES: Vec4 = Vec4::new(1.8, 2.0, 2.0, 2.0);

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
        let cook = |kind| terra_core::cooked::Cook {
            kind,
            layers: GROUND_LAYERS,
            mips: true,
        };
        GroundTextures {
            color: crate::cooked::picture(
                assets,
                "materials/ground/color.jpg",
                cook(terra_core::cooked::Kind::Colour),
                layers(false),
            ),
            normal: crate::cooked::picture(
                assets,
                "materials/ground/normal.jpg",
                cook(terra_core::cooked::Kind::Normal),
                layers(true),
            ),
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
        climate: &crate::climate::ClimateMaps,
        quality: crate::quality::Quality,
        land: &LandMap,
        horizon: &crate::horizon::HorizonPicture,
    ) -> Self {
        let radius_m = planet.radius_m();
        let horizon = horizon.uniforms();
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
            mountain_metres: GROUND_MOUNTAIN_METRES,
            // Textures are for ground that is covered; the bare stand-in stays flat.
            textured: (tweaks.ground_cover > 0.0) as u32,
            color: Some(textures.color.clone()),
            normal: Some(textures.normal.clone()),
            cover_from_world: cover.from_world,
            cover_on: match (&cover.image, cover.sows) {
                (None, _) => 0,
                (Some(_), false) => 1,
                (Some(_), true) => 2,
            },
            cheap: quality == crate::quality::Quality::Low,
            // Bevy's blank picture stands in for a map until there is one.
            cover_map: Some(cover.image.clone().unwrap_or_default()),
            cover_more: Some(cover.more.clone().unwrap_or_default()),
            cover_stood: Some(cover.stood.clone().unwrap_or_default()),
            climate_from_world: climate.from_world,
            climate_on: climate.ground.is_some() as u32,
            fallen_layer: climate.fallen_layer,
            // Bevy's fallback pictures, of the right shape, until there are maps.
            climate_ground: climate.ground.clone(),
            climate_leaves: climate.leaves.clone(),
            land_middle: land.middle,
            land_east: land.east,
            land_north: if land.image.is_some() {
                land.north
            } else {
                Vec4::ZERO
            },
            heath_layer: climate.heath_layer,
            grass_layer: climate.grass_layer,
            heath_autumn: climate.autumn_of(climate.heath_layer),
            land_map: land.image.clone(),
            horizon_middle: horizon.0,
            horizon_east: horizon.1,
            horizon_north: horizon.2,
            horizon_map: horizon.3,
        }
    }
}

pub fn plugin(app: &mut App) {
    let options = app.world().resource::<Options>();
    let (radius_m, yard, rolling) = (options.radius_m, options.yard(), options.rolling);
    let spec = PlanetSpec {
        seed: Seed(1),
        radius_m,
    };
    crate::crash::note("seed", spec.seed.0);
    let mut planet = if yard && rolling {
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
        light_direction: Vec3::ZERO,
        light_size: 0.0,
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
    // On a world with mountains, the land round the start is worked out before anything stands
    // on it (D322): its lakes, seasons and snow on the clock's date. A site the land's rules
    // make is found first, and the start stood on its shore once its lakes are known.
    let day = {
        let options = app.world().resource::<Options>();
        let climate: terra_climate::Climate =
            crate::data::read(crate::daylight::WorldClimate::FILE);
        options.date.map_or(0, |date| date.day(&climate.year))
    };
    let start = match app.world().resource::<Options>().site.clone() {
        Some(name) => {
            let found = crate::start::find_site(&planet, &name);
            planet.look_round(found.middle(), ROUND_M, day);
            planet.clear_woods(|land| found.clearings(land));
            app.insert_resource(crate::start::Sited(found));
            found.start(&planet)
        }
        None => {
            let start = crate::start::Start::choose(&planet, yard);
            if planet.has_mountains() {
                planet.look_round(start.translation, ROUND_M, day);
            }
            start
        }
    };
    app.insert_resource(planet)
        .insert_resource(start)
        .insert_resource(tweaks)
        .init_resource::<GroundCoverMap>()
        .init_resource::<crate::climate::ClimateMaps>()
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
            crate::lakes::plugin,
            crate::horizon::plugin,
        ))
        .add_plugins(
            ResourceInspectorPlugin::<Tweaks>::default()
                .run_if(input_toggle_active(false, KeyCode::F2)),
        )
        .add_systems(Startup, setup)
        .add_systems(Update, sea_waves)
        .add_systems(
            Update,
            apply_tweaks.run_if(
                resource_changed::<Tweaks>
                    .or_eager(resource_exists_and_changed::<Sky>)
                    .or_eager(resource_changed::<GroundCoverMap>)
                    .or_eager(resource_changed::<crate::climate::ClimateMaps>)
                    .or_eager(resource_changed::<crate::quality::Quality>)
                    .or_eager(resource_exists_and_changed::<crate::horizon::HorizonPicture>),
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
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
) {
    let radius_m = planet.radius_m();
    let textures = GroundTextures::load(&assets);
    let land = planet
        .round()
        .map_or_else(LandMap::default, |round| LandMap::of(round, &mut images));
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
            &crate::climate::ClimateMaps::default(),
            crate::quality::Quality::default(),
            &land,
            &crate::horizon::HorizonPicture::default(),
        ),
    });
    commands.insert_resource(textures);
    commands.insert_resource(land);
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
            extension: WaterExtension::new(sky.as_deref(), planet.start()),
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
    mut sea: Single<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<WaterMaterial>,
        ),
        With<Sea>,
    >,
    mut ground_materials: ResMut<Assets<GroundMaterial>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    textures: Res<GroundTextures>,
    cover: Res<GroundCoverMap>,
    climate: Res<crate::climate::ClimateMaps>,
    quality: Res<crate::quality::Quality>,
    land: Option<Res<LandMap>>,
    horizon: Option<Res<crate::horizon::HorizonPicture>>,
) {
    let toward_sun = tweaks
        .light_direction
        .try_normalize()
        .unwrap_or(tweaks.sun_direction.normalize_or(Vec3::Y));
    sun.0.color = tweaks.sun_color;
    sun.0.illuminance = tweaks.sun_illuminance;
    sun.0.soft_shadow_size = (tweaks.light_size > 0.0).then_some(tweaks.light_size);
    *sun.1 = Transform::from_translation(toward_sun).looking_at(Vec3::ZERO, Vec3::Y);

    sea.0.scale = Vec3::splat(planet.radius_m() + tweaks.sea_level_m);
    *sea.1 = if tweaks.sea {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    // The ground changes only when its tweaks do, not with the sun, so it is left alone
    // otherwise: a changed material is prepared again, and every chunk drawn with it.
    let no_land = LandMap::default();
    let land = land.as_deref().unwrap_or(&no_land);
    let no_horizon = crate::horizon::HorizonPicture::default();
    let horizon = horizon.as_deref().unwrap_or(&no_horizon);
    let ground = GroundExtension::new(
        &planet, &tweaks, &textures, &cover, &climate, *quality, land, horizon,
    );
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
    // Every water reflects the sky; the sea's colour is the tweaks', a lake's its own.
    let sea_material = sea.2.id();
    for (id, material) in water_materials.iter_mut() {
        if id == sea_material {
            material.base.base_color = tweaks.sea_shallow;
        }
        material.extension.reflect(sky.as_deref());
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
