//! Lakes on screen (D322): each lake the planet found round the start (`Planet::waters`,
//! filled to its spill point by `terra_land::lakes`) drawn as its own surface at its own
//! level, its water a medium by depth (`assets/shaders/lake.wesl`, ported from
//! boujie_water_shader, MIT), rippled by the climate's wind and reflecting the sky.

use crate::air::AirPicture;
use crate::daylight::Clock;
use crate::instance::{Building, InWorld, InstanceApp};
use crate::planet::Planet;
use crate::rises::{RISES, Rises};
use crate::sky::Sky;
use crate::wakes::{WAKES, Wakes};
use crate::waves::{self, TRAINS};
use bevy::color::Mix;
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use terra_gen::smoothstep;
use terra_land::water::Waters;

/// Pools smaller than this, about 30 m across, are the ground's wet hollows and the pits where
/// the mountains' couloirs cross, not drawn as water: drawn from 300 m², the walls over the
/// mountain lake stood dotted with a hundred puddles.
const SMALLEST_M2: f32 = 1000.0;
/// A clear mountain lake over rock (docs/demos.md, open question 5): green over the pale
/// bed where shallow, letting much of it show, and a dark bottle green where deep, as
/// Morskie Oko's, hiding the bed. Linear colours.
const SHALLOW: LinearRgba = LinearRgba::new(0.06, 0.13, 0.10, 0.35);
const DEEP: LinearRgba = LinearRgba::new(0.008, 0.03, 0.025, 0.93);
/// Beer's law's strength a metre, and the depth offset before it, as the port's own
/// defaults are tuned for clear water (beers_law 2, depth_offset -0.75 there, for a sea whose
/// depth is in tenths of its units; ours is metres of clear mountain water, which lets the
/// eye a few metres down). At 0.35 the bed showed pale cyan through the shallows tens of
/// metres out, where a mountain lake at dusk mirrors its sky past the first metre or two
/// (lake film r6).
const BEERS_LAW: f32 = 0.6;
const DEPTH_OFFSET: f32 = -0.2;
/// Foam against the shore from this depth to that, and from this distance on the water is all
/// its deep colour, the depth buffer too coarse for its shallows (the port's DEPTH_FOG_FADE).
const FOAM_M: (f32, f32) = (0.02, 0.18);
/// A lowland lake in the forest, as a settlement's on the valley floor (D381): not a clear
/// tarn over pale granite but a lake over silt and drowned leaves, its water stained by the
/// woods' tannins and its plankton, olive brown where shallow and near black a metre or two
/// down, as the lakes of Mazury and Småland are; its bed shows only at the very edge. A lake
/// is a mountain one from the first height over the sea, fully by the second, as its shore
/// is (flora's `MOUNTAIN_SHORE_M`).
const LOWLAND_SHALLOW: LinearRgba = LinearRgba::new(0.03, 0.036, 0.018, 0.62);
const LOWLAND_DEEP: LinearRgba = LinearRgba::new(0.004, 0.007, 0.005, 0.97);
const LOWLAND_BEERS_LAW: f32 = 1.1;
const MOUNTAIN_LAKE_M: (f32, f32) = (250.0, 500.0);
const FADE_M: (f32, f32) = (350.0, 1400.0);

pub type LakeMaterial = ExtendedMaterial<StandardMaterial, LakeExtension>;

/// What the lake shader is told. Field order must match the struct in lake.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct LakeExtension {
    #[uniform(100)]
    horizon: LinearRgba,
    #[uniform(100)]
    zenith: LinearRgba,
    #[uniform(100)]
    sky_brightness: f32,
    #[uniform(100)]
    beers_law: f32,
    #[uniform(100)]
    depth_offset: f32,
    #[uniform(100)]
    shallow: LinearRgba,
    #[uniform(100)]
    deep: LinearRgba,
    #[uniform(100)]
    foam_from: f32,
    #[uniform(100)]
    foam_to: f32,
    #[uniform(100)]
    fade_from: f32,
    #[uniform(100)]
    fade_to: f32,
    /// Which way the wind blows, level in the world's frame, and how hard, 0 to 1.
    #[uniform(100)]
    wind: Vec4,
    /// A point near the lakes their noise is laid out from, so it reads small numbers.
    #[uniform(100)]
    origin: Vec4,
    /// East and north where the skyline was taken, and the skyline round it
    /// (`terra_land::round::Skyline`), what the water mirrors.
    #[uniform(100)]
    east: Vec4,
    #[uniform(100)]
    north: Vec4,
    #[uniform(100)]
    skyline: [Vec4; SKYLINE_WAYS],
    /// The waves the surface is made of (`waves::trains`).
    #[uniform(100)]
    trains: [Vec4; TRAINS],
    /// The rings spreading where fish rise (`rises`): each where it is from `origin`, and
    /// when it began by the shader's clock, none when 0.
    #[uniform(100)]
    rises: [Vec4; RISES],
    /// The wakes behind what swims (`wakes`): each swimmer where it is from `origin` and how
    /// fast it goes, and which way it heads and how wide it is.
    #[uniform(100)]
    wakes_at: [Vec4; WAKES],
    #[uniform(100)]
    wakes_way: [Vec4; WAKES],
    /// Where each was a wake's length back, from `origin`, w 1; the wake bends through it.
    #[uniform(100)]
    wakes_was: [Vec4; WAKES],
    /// Whether the air's picture is bound, 1 or 0: then the water mirrors the sky the air
    /// draws, exposed as it is, rather than the painted sky's two colours.
    #[uniform(100)]
    over_air: u32,
    /// How much of the sky's own dusk colours the sky draws over the air (`Sky::twilight`),
    /// and the glow round the sun's way, so the water mirrors the dusk the sky shows: mirroring
    /// the air alone, a lake under a pink dusk sky was teal (lake film r6).
    #[uniform(100)]
    twilight: f32,
    #[uniform(100)]
    glow: LinearRgba,
    /// Toward the sun, below the horizon too (`Sky::sun`), which the dusk's warm side faces:
    /// the world's light is the moon's by then.
    #[uniform(100)]
    sun: Vec4,
    /// 1 while the water is drawn as a flat mask for the looks check (`WaterMask`), else 0.
    #[uniform(100)]
    mask: f32,
    /// The land's horizon round the camera (`crate::horizon`, D416): the water in a range's
    /// shadow past the shadow map's reach, and the slopes it mirrors off screen lit as they
    /// stand.
    #[uniform(100)]
    horizon_middle: Vec4,
    #[uniform(100)]
    horizon_east: Vec4,
    #[uniform(100)]
    horizon_north: Vec4,
    /// The air all round the camera (`air::AirPicture`), unexposed.
    #[texture(101, dimension = "cube")]
    #[sampler(102)]
    air: Option<Handle<Image>>,
    #[texture(103, dimension = "2d_array")]
    horizon_map: Option<Handle<Image>>,
}

impl LakeExtension {
    /// Told the land's horizon.
    fn see(&mut self, horizon: Option<&crate::horizon::HorizonPicture>) {
        let (middle, east, north, map) = horizon.map_or_else(
            || crate::horizon::HorizonPicture::default().uniforms(),
            crate::horizon::HorizonPicture::uniforms,
        );
        self.horizon_middle = middle;
        self.horizon_east = east;
        self.horizon_north = north;
        self.horizon_map = map;
    }
}

/// How many ways out round the lake its skyline is taken in.
const SKYLINE_WAYS: usize = 64;

/// What the lakes mirror, taken once: the skyline round the start's lake.
#[derive(Clone)]
struct Mirror {
    origin: Vec3,
    east: Vec3,
    north: Vec3,
    skyline: [Vec4; SKYLINE_WAYS],
    /// How much the start's lake is a lowland one, 0 a mountain lake to 1.
    lowland: f32,
}

impl MaterialExtension for LakeExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/lake.wesl".into()
    }
}

impl LakeExtension {
    fn new(sky: Option<&Sky>, air: Option<&AirPicture>, wind: Vec4, mirror: &Mirror) -> Self {
        let air = air.map(|picture| picture.0.clone());
        let (horizon, zenith, sky_brightness) = sky
            .map_or((LinearRgba::BLACK, LinearRgba::BLACK, 0.0), |sky| {
                (sky.horizon.into(), sky.zenith.into(), sky.brightness)
            });
        let (twilight, glow) = sky.map_or((0.0, LinearRgba::BLACK), |sky| {
            (sky.twilight, sky.glow.into())
        });
        let sun = sky
            .map_or(Vec3::ZERO, |sky| sky.sun.normalize_or_zero())
            .extend(0.0);
        let lowland = mirror.lowland;
        let mix = |a: LinearRgba, b: LinearRgba| a.mix(&b, lowland);
        Self {
            horizon,
            zenith,
            sky_brightness,
            beers_law: BEERS_LAW + (LOWLAND_BEERS_LAW - BEERS_LAW) * lowland,
            depth_offset: DEPTH_OFFSET,
            shallow: mix(SHALLOW, LOWLAND_SHALLOW),
            deep: mix(DEEP, LOWLAND_DEEP),
            foam_from: FOAM_M.0,
            foam_to: FOAM_M.1,
            fade_from: FADE_M.0,
            fade_to: FADE_M.1,
            wind,
            origin: mirror.origin.extend(0.0),
            east: mirror.east.extend(0.0),
            north: mirror.north.extend(0.0),
            skyline: mirror.skyline,
            trains: waves::trains(),
            rises: [Vec4::ZERO; RISES],
            wakes_at: [Vec4::ZERO; WAKES],
            wakes_way: [Vec4::ZERO; WAKES],
            wakes_was: [Vec4::ZERO; WAKES],
            over_air: air.is_some() as u32,
            twilight,
            glow,
            sun,
            mask: 0.0,
            horizon_middle: Vec4::ZERO,
            horizon_east: Vec4::ZERO,
            horizon_north: Vec4::ZERO,
            air,
            horizon_map: None,
        }
    }
}

/// While set, every lake draws itself as a flat magenta (`MASK` in lake.wesl) rather than
/// water: a shot then shows what the water covers, by what draws it, so the looks check can
/// leave the water's smooth colour out of its debug-tint rule (`--water-mask`, D429).
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct WaterMask(pub bool);

/// The lakes' material, the patterned ground's pools' (peaty), and what they mirror.
#[derive(Resource)]
struct LakeLook(Handle<LakeMaterial>, Mirror, Option<Handle<LakeMaterial>>);

/// A polygon's or a kettle's pool lies over peat, its water the brown of the peat's tannins
/// and dark where it is a little deep, as the tundra's photographs' pools are, not a mountain
/// lake's green over rock (D322: the water a medium by what lies under it).
const PEAT_SHALLOW: LinearRgba = LinearRgba::new(0.05, 0.034, 0.015, 0.55);
const PEAT_DEEP: LinearRgba = LinearRgba::new(0.008, 0.006, 0.003, 0.95);

impl LakeExtension {
    fn peaty(self) -> Self {
        Self {
            shallow: PEAT_SHALLOW,
            deep: PEAT_DEEP,
            ..self
        }
    }
}

/// The lakes of the world the run is in, their water's look and mist the world's (D469).
pub fn plugin(app: &mut App) {
    entering(app);
    app.add_plugins(MaterialPlugin::<LakeMaterial>::default())
        .add_plugins(MaterialPlugin::<MistMaterial>::default())
        .add_systems(
            Update,
            (
                follow_sky_and_wind,
                follow_rises,
                follow_wakes,
                mist_follows_the_morning,
            ),
        );
}

/// The mist a lake steams on a calm morning (D371): the night's cold air over water still
/// warm from the day before, banks a few metres deep drifting over it, thickest at dawn and
/// burnt off by mid-morning, as Morskie Oko's and every mountain lake's morning mist is. Drawn
/// as games lay mist banks over water: soft cards standing on it turned to the eye, fading
/// where they meet what stands behind them (`mist.wesl`), lit by the sun and the sky.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct MistExtension {
    /// Which way and how fast its drifts move, metres a second; and how thick it is, 0 to 1.
    #[uniform(100)]
    wind: Vec4,
    /// A point near the lakes its noise is laid out from.
    #[uniform(100)]
    origin: Vec4,
}

impl MaterialExtension for MistExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/mist.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/mist.wesl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/mist.wesl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/mist.wesl".into()
    }
}

pub type MistMaterial = ExtendedMaterial<StandardMaterial, MistExtension>;

/// The lakes' one mist material.
#[derive(Resource)]
struct LakeMist(Handle<MistMaterial>);

/// Lakes this big and bigger steam: a pond's mist is too small to see from its shore.
const MISTY_M2: f32 = 50_000.0;
/// A bank's card every this many metres over the water, this wide and this tall at most,
/// standing this far under the surface so its foot never shows.
const MIST_EVERY_M: f32 = 40.0;
const MIST_WIDE_M: (f32, f32) = (40.0, 75.0);
const MIST_TALL_M: (f32, f32) = (3.0, 13.0);
const MIST_FOOT_M: f32 = 1.5;

/// A lake's mist banks as one mesh: a card standing on its water every `MIST_EVERY_M`, by
/// the lake's own cells, its place and size jittered.
fn mist_mesh(waters: &Waters, lake: usize) -> Option<Mesh> {
    let region = waters.region();
    let level = waters.lake_radius_m(lake);
    let every = (MIST_EVERY_M / region.cell_m).round().max(1.0) as usize;
    let (mut positions, mut normals, mut corners, mut sizes, mut indices) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut seed = terra_gen::Seed(0x5EED_0000 + lake as u64).rng();
    for (c, r) in waters.cells_of(lake) {
        if c % every != 0 || r % every != 0 {
            continue;
        }
        let jitter = Vec2::new(seed.unit() - 0.5, seed.unit() - 0.5) * MIST_EVERY_M;
        let at = region.offset(c, r) + jitter;
        let foot = (region.middle + region.east * at.x + region.north * at.y).normalize()
            * (level - MIST_FOOT_M);
        let size = Vec2::new(
            MIST_WIDE_M.0 + (MIST_WIDE_M.1 - MIST_WIDE_M.0) * seed.unit(),
            MIST_TALL_M.0 + (MIST_TALL_M.1 - MIST_TALL_M.0) * seed.unit() + MIST_FOOT_M,
        );
        let base = positions.len() as u32;
        for corner in [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]] {
            positions.push(foot.to_array());
            normals.push(foot.normalize().to_array());
            corners.push(corner);
            sizes.push(size.to_array());
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
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
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, corners)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, sizes)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

/// How thick a lake's mist is, 0 to 1, at an hour of the place's own sun and in a wind of so
/// much (0 still to 1 a stiff breeze): from before dawn to two hours after sunrise, thinning
/// to nothing by mid-morning, and torn away by a breeze.
fn mist_thickness(local_hour: f32, wind: f32) -> f32 {
    let dawn = smoothstep(4.0, 5.5, local_hour) * (1.0 - smoothstep(7.5, 9.5, local_hour));
    let calm = 1.0 - smoothstep(0.6, 1.1, wind);
    dawn * calm
}

/// How much a lake so high over the sea is a lowland one, 1, rather than a mountain one, 0.
pub(crate) fn lowland(over_sea_m: f32) -> f32 {
    1.0 - smoothstep(MOUNTAIN_LAKE_M.0, MOUNTAIN_LAKE_M.1, over_sea_m)
}

/// The lakes' mist as the morning goes, by the sun where the camera is (D363): thickest from
/// dawn to an hour or two after, gone by mid-morning, thinned and torn by a wind, and drifting
/// with it.
fn mist_follows_the_morning(
    clock: Option<Res<Clock>>,
    path: Option<Res<crate::daylight::SkyPath>>,
    look: Option<Res<LakeLook>>,
    mist: Option<Res<LakeMist>>,
    lakes: Res<Assets<LakeMaterial>>,
    mut mists: ResMut<Assets<MistMaterial>>,
) {
    let (Some(clock), Some(path), Some(mist)) = (clock, path, mist) else {
        return;
    };
    let wind = look
        .and_then(|look| lakes.get(&look.0).map(|m| m.extension.wind))
        .unwrap_or(Vec4::ZERO);
    // Drifting a metre every few seconds in a light air, faster in a breeze.
    let wanted = (wind.truncate() * (0.3 + 1.5 * wind.w))
        .extend(mist_thickness(path.local_hour(clock.hour), wind.w));
    if mists
        .get(&mist.0)
        .is_some_and(|m| m.extension.wind != wanted)
        && let Some(mut m) = mists.get_mut(&mist.0)
    {
        m.extension.wind = wanted;
    }
}

/// What of the lakes is the world's (D469).
pub(crate) fn entering(app: &mut App) {
    app.world_resource::<LakeLook>()
        .world_resource::<LakeMist>()
        .add_systems(OnEnter(InWorld), spawn.in_set(Building::Place));
}

fn spawn(
    mut commands: Commands,
    planet: Res<Planet>,
    start: Res<crate::start::Start>,
    sky: Option<Res<Sky>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LakeMaterial>>,
    mut mists: ResMut<Assets<MistMaterial>>,
) {
    let (Some(waters), Some(round)) = (planet.waters(), planet.round()) else {
        return;
    };
    // The skyline round the start's lake, seen from its water by the shore.
    let at = start.translation;
    let up = at.normalize();
    let level = waters.lake_near(up, 6).unwrap_or(at.length());
    let skyline = round.skyline(up * level, level, SKYLINE_WAYS);
    let mut ways = [Vec4::ZERO; SKYLINE_WAYS];
    for (way, seen) in ways.iter_mut().zip(&skyline) {
        *way = Vec4::new(
            seen.elevation,
            seen.top_m,
            seen.tree_line_m,
            seen.snow_line_m,
        );
    }
    let mirror = Mirror {
        origin: waters.region().middle,
        east: round.land.frame.east(up),
        north: round.land.frame.north(up),
        skyline: ways,
        lowland: lowland(level - planet.sea_radius_m()),
    };
    info!(
        "Lakes: the start's lake {:.0} m over the sea, {:.0}% a lowland lake",
        level - planet.sea_radius_m(),
        mirror.lowland * 100.0
    );
    let look = materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::from(SHALLOW),
            perceptual_roughness: 0.06,
            reflectance: 0.5,
            alpha_mode: AlphaMode::Blend,
            // Drawn after the opaque scene is copied for transmissive materials, which its
            // shader reads for its reflection of what is on screen (lake.wesl `on_screen`);
            // the shader lets no light through itself.
            specular_transmission: 1.0,
            ..default()
        },
        extension: LakeExtension::new(sky.as_deref(), None, Vec4::ZERO, &mirror),
    });
    let mut drawn = 0;
    let mist = mists.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            double_sided: true,
            cull_mode: None,
            ..default()
        },
        extension: MistExtension {
            wind: Vec4::ZERO,
            origin: waters.region().middle.extend(0.0),
        },
    });
    let mut misty = 0;
    for (index, lake) in waters.lakes().iter().enumerate() {
        if lake.area_m2 < SMALLEST_M2 {
            continue;
        }
        if lake.area_m2 >= MISTY_M2
            && let Some(banks) = mist_mesh(waters, index)
        {
            commands.spawn((
                Name::new(format!("Lake {index} mist")),
                Mesh3d(meshes.add(banks)),
                MeshMaterial3d(mist.clone()),
                NotShadowCaster,
                bevy::light::NotShadowReceiver,
                // Turned on the GPU, the cards reach past their feet' bounds.
                bevy::camera::visibility::NoFrustumCulling,
            ));
            misty += 1;
        }
        commands.spawn((
            Name::new(format!("Lake {index}")),
            Mesh3d(meshes.add(mesh(waters, index))),
            MeshMaterial3d(look.clone()),
            NotShadowCaster,
            // Light in the water scatters: the shore's trees' shadows fall on its bed, seen
            // through it, not drawn hard on its surface as black cracks.
            bevy::light::NotShadowReceiver,
        ));
        drawn += 1;
    }
    info!(
        "Lakes: {drawn} drawn of {} found round the start, {misty} big enough to steam on a calm morning",
        waters.lakes().len()
    );
    // The patterned ground's pools, many and small, drawn as one surface, each at its level.
    let mut peat = None;
    if let Some(pools) = &round.pools {
        let mut pooled = materials.get(&look).cloned().expect("the lakes' material");
        pooled.extension = pooled.extension.peaty();
        let pooled = materials.add(pooled);
        commands.spawn((
            Name::new("Pools"),
            Mesh3d(meshes.add(pools_mesh(pools))),
            MeshMaterial3d(pooled.clone()),
            NotShadowCaster,
        ));
        peat = Some(pooled);
        info!(
            "Lakes: {} pools in the patterned ground round the start",
            pools.lakes().len()
        );
    }
    commands.insert_resource(LakeLook(look, mirror, peat));
    commands.insert_resource(LakeMist(mist));
}

/// The sky the lakes reflect, as it changes, and the wind over them where the camera is
/// (`wind::WorldWind`).
fn follow_sky_and_wind(
    look: Option<Res<LakeLook>>,
    sky: Option<Res<Sky>>,
    air: Option<Res<AirPicture>>,
    wind: Option<Res<crate::wind::WorldWind>>,
    masked: Option<Res<WaterMask>>,
    horizon: Option<Res<crate::horizon::HorizonPicture>>,
    mut materials: ResMut<Assets<LakeMaterial>>,
) {
    let Some(look) = look else {
        return;
    };
    let mask = if masked.is_some_and(|masked| masked.0) {
        1.0
    } else {
        0.0
    };
    let wind = waves::over_water(wind.as_deref());
    let sky_changed = sky.as_ref().is_some_and(|sky| sky.is_changed())
        || air.as_ref().is_some_and(|air| air.is_changed())
        || horizon.as_ref().is_some_and(|horizon| horizon.is_changed());
    let Some(material) = materials.get(&look.0) else {
        return;
    };
    if !sky_changed && material.extension.wind == wind && material.extension.mask == mask {
        return;
    }
    let mut extension = LakeExtension::new(sky.as_deref(), air.as_deref(), wind, &look.1);
    extension.mask = mask;
    extension.see(horizon.as_deref());
    // The rings spreading now stay.
    extension.rises = material.extension.rises;
    extension.wakes_at = material.extension.wakes_at;
    extension.wakes_way = material.extension.wakes_way;
    extension.wakes_was = material.extension.wakes_was;
    if let Some(peat) = &look.2
        && let Some(mut material) = materials.get_mut(peat)
    {
        material.extension = extension.clone().peaty();
    }
    if let Some(mut material) = materials.get_mut(&look.0) {
        material.extension = extension;
    }
}

/// The rings where fish rise, told the lakes' material as each begins, from the point the
/// lakes are laid out from.
fn follow_rises(
    look: Option<Res<LakeLook>>,
    rises: Option<Res<Rises>>,
    mut materials: ResMut<Assets<LakeMaterial>>,
) {
    let (Some(look), Some(rises)) = (look, rises) else {
        return;
    };
    if !rises.is_changed() {
        return;
    }
    let origin = look.1.origin;
    let rings = rises.rings.map(|ring| {
        if ring.w > 0.0 {
            (ring.truncate() - origin).extend(ring.w)
        } else {
            Vec4::ZERO
        }
    });
    if let Some(mut material) = materials.get_mut(&look.0)
        && material.extension.rises != rings
    {
        material.extension.rises = rings;
    }
}

/// The wakes behind what swims, told the lakes' material as they move, from the point the
/// lakes are laid out from.
fn follow_wakes(
    look: Option<Res<LakeLook>>,
    wakes: Option<Res<Wakes>>,
    mut materials: ResMut<Assets<LakeMaterial>>,
) {
    let (Some(look), Some(wakes)) = (look, wakes) else {
        return;
    };
    if !wakes.is_changed() {
        return;
    }
    let origin = look.1.origin;
    let at = wakes.at.map(|at| {
        if at.w > 0.0 {
            (at.truncate() - origin).extend(at.w)
        } else {
            Vec4::ZERO
        }
    });
    let was = wakes.was.map(|was| {
        if was.w > 0.0 {
            (was.truncate() - origin).extend(was.w)
        } else {
            Vec4::ZERO
        }
    });
    if let Some(mut material) = materials.get_mut(&look.0)
        && (material.extension.wakes_at != at
            || material.extension.wakes_way != wakes.way
            || material.extension.wakes_was != was)
    {
        material.extension.wakes_at = at;
        material.extension.wakes_way = wakes.way;
        material.extension.wakes_was = was;
    }
}

/// Every lake of a grid as one surface, as many small pools are best drawn: each cell under a
/// lake at its level.
fn pools_mesh(waters: &Waters) -> Mesh {
    let region = waters.region();
    let n = region.cells;
    let corner = |c: usize, r: usize, radius: f32| {
        let at = region.offset(c, r) + Vec2::new(-0.5, 0.5) * region.cell_m;
        (region.middle + region.east * at.x + region.north * at.y).normalize() * radius
    };
    // Each pool's cells and a cell's margin round them, so its edge runs under the ground it
    // rises onto, where the water fades out by its depth (lake.wesl): on its own cells alone
    // the water ended in their straight edges wherever the ground past them was lower than
    // the cell said. A margin cell is the first pool's that reaches it, so no two layers of
    // water lie in one place.
    let mut level: Vec<Option<(f32, usize)>> = vec![None; n * n];
    for r in 0..n {
        for c in 0..n {
            if let Some(lake) = waters.lake_at(c, r) {
                level[r * n + c] = Some((waters.lake_radius_m(lake), lake));
            }
        }
    }
    let mut margin = level.clone();
    for r in 0..n {
        for c in 0..n {
            let Some(pool) = level[r * n + c] else {
                continue;
            };
            for (dc, dr) in [
                (-1i64, 0i64),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (1, 1),
                (-1, 1),
                (1, -1),
            ] {
                let (cc, rr) = (c as i64 + dc, r as i64 + dr);
                if cc < 0 || rr < 0 || cc as usize >= n || rr as usize >= n {
                    continue;
                }
                let at = rr as usize * n + cc as usize;
                if margin[at].is_none() {
                    margin[at] = Some(pool);
                }
            }
        }
    }
    let mut middles = vec![None; waters.lakes().len()];
    let (mut positions, mut normals, mut uvs, mut indices) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for r in 0..n {
        for c in 0..n {
            let Some((radius, pool)) = margin[r * n + c] else {
                continue;
            };
            let middle = *middles[pool].get_or_insert_with(|| region.offset(c, r));
            let base = positions.len() as u32;
            for (dc, dr) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                let p = corner(c + dc, r + dr, radius);
                positions.push(p.to_array());
                normals.push(p.normalize().to_array());
                uvs.push(on_water(region, c + dc, r + dr, middle));
            }
            indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// Where a cell's north-west corner lies on its water, metres east and north of a point of
/// that water, as its uv: what its waves are laid out on (lake.wesl), in numbers small enough
/// that a ripple of a few centimetres keeps its phase, which the world's position, tens of
/// kilometres from the planet's centre, rounds to millimetres.
fn on_water(region: &terra_land::region::Region, c: usize, r: usize, middle: Vec2) -> [f32; 2] {
    (region.offset(c, r) + Vec2::new(-0.5, 0.5) * region.cell_m - middle).to_array()
}

/// A lake's water as Bevy draws it: its cells and a cell's margin round them, so its edge runs
/// under the shore, at its level on the sphere.
fn mesh(waters: &Waters, lake: usize) -> Mesh {
    let region = waters.region();
    let radius = waters.lake_radius_m(lake);
    let n = region.cells;
    let mut under = vec![false; n * n];
    for (c, r) in waters.cells_of(lake) {
        for dr in -1i64..=1 {
            for dc in -1i64..=1 {
                let (cc, rr) = (c as i64 + dc, r as i64 + dr);
                // A cell past the shore, so the water meets the ground under it, but never
                // another lake's: two layers of water there blend twice, a bright square.
                if cc >= 0 && rr >= 0 && (cc as usize) < n && (rr as usize) < n {
                    let (cc, rr) = (cc as usize, rr as usize);
                    if waters.lake_at(cc, rr).is_none_or(|other| other == lake) {
                        under[rr * n + cc] = true;
                    }
                }
            }
        }
    }
    // A cell's north-west corner, on the lake's sphere.
    let corner = |c: usize, r: usize| {
        let at = region.offset(c, r) + Vec2::new(-0.5, 0.5) * region.cell_m;
        (region.middle + region.east * at.x + region.north * at.y).normalize() * radius
    };
    let middle = waters
        .cells_of(lake)
        .next()
        .map_or(Vec2::ZERO, |(c, r)| region.offset(c, r));
    let (mut positions, mut normals, mut uvs, mut indices) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for r in 0..n {
        for c in 0..n {
            if !under[r * n + c] {
                continue;
            }
            let base = positions.len() as u32;
            for (dc, dr) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                let p = corner(c + dc, r + dr);
                positions.push(p.to_array());
                normals.push(p.normalize().to_array());
                uvs.push(on_water(region, c + dc, r + dr, middle));
            }
            // North-west, south-east, north-east, then north-west, south-west, south-east:
            // counter-clockwise seen from above, so they face the sky.
            indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lake_steams_in_the_calm_of_the_morning_and_not_after() {
        let still = 0.1;
        // Thick in the first hours of the morning, gone by mid-morning and all the day.
        assert!(mist_thickness(6.5, still) > 0.9);
        assert_eq!(mist_thickness(10.0, still), 0.0);
        assert_eq!(mist_thickness(15.0, still), 0.0);
        // Thinner as it burns off.
        assert!(mist_thickness(8.5, still) < mist_thickness(7.0, still));
        // A breeze tears it away.
        assert!(mist_thickness(6.5, 0.9) < 0.6 * mist_thickness(6.5, still));
        assert_eq!(mist_thickness(6.5, 1.2), 0.0);
    }
}
