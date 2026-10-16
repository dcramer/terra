//! Lakes on screen (D322): each lake the planet found round the start (`Planet::waters`,
//! filled to its spill point by `terra_land::lakes`) drawn as its own surface at its own
//! level, its water a medium by depth (`assets/shaders/lake.wesl`, ported from
//! boujie_water_shader, MIT), rippled by the climate's wind and reflecting the sky.

use crate::air::AirPicture;
use crate::climate::ClimateFields;
use crate::daylight::Clock;
use crate::planet::{MainCamera, Planet};
use crate::sky::Sky;
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
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
/// eye a few metres down).
const BEERS_LAW: f32 = 0.35;
const DEPTH_OFFSET: f32 = -0.2;
/// Foam against the shore from this depth to that, and from this distance on the water is all
/// its deep colour, the depth buffer too coarse for its shallows (the port's DEPTH_FOG_FADE).
const FOAM_M: (f32, f32) = (0.02, 0.18);
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
    /// The waves the surface is made of (`trains`).
    #[uniform(100)]
    trains: [Vec4; TRAINS],
    /// Whether the air's picture is bound, 1 or 0: then the water mirrors the sky the air
    /// draws, exposed as it is, rather than the painted sky's two colours.
    #[uniform(100)]
    over_air: u32,
    /// The air all round the camera (`air::AirPicture`), unexposed.
    #[texture(101, dimension = "cube")]
    #[sampler(102)]
    air: Option<Handle<Image>>,
}

/// How many ways out round the lake its skyline is taken in.
const SKYLINE_WAYS: usize = 64;

/// How many trains of waves make a lake's surface, the first `SWELL` of them long ones.
const TRAINS: usize = 28;
/// The longest train's length and the shortest's, in metres: from the swell a breeze leaves
/// on a mountain lake down to the capillary ripples a gust raises.
const LONGEST_M: f32 = 3.2;
const SHORTEST_M: f32 = 0.035;

/// The trains of waves a lake's surface is summed from, as Finch sums them ("Effective Water
/// Simulation from Physical Models", GPU Gems, 2004): each a plane wave, its length from the
/// longest to the shortest in even ratios, a little jittered so no two beat in step; how far
/// it turns from the wind, the long ones more; and how fast it runs, by water's dispersion
/// (gravity and, for ripples of a few centimetres, surface tension), so each length moves at
/// its own speed and the pattern changes as it goes rather than sliding as one picture.
/// Each is its wavenumber (radians a metre), angular frequency (radians a second), its turn
/// from the wind (radians) and its phase.
fn trains() -> [Vec4; TRAINS] {
    let ratio = libm::powf(SHORTEST_M / LONGEST_M, 1.0 / (TRAINS - 1) as f32);
    std::array::from_fn(|i| {
        // A fixed scatter, the same every run, and uncorrelated from train to train: an even
        // one (the golden ratio's multiples) turned the trains in step, and their crests
        // crossed in a lattice.
        let scatter = |salt: u32| scatter(i as u32 * 3 + salt);
        let length = LONGEST_M * ratio.powi(i as i32) * (0.95 + 0.1 * scatter(0));
        let k = std::f32::consts::TAU / length;
        let spread = if i < SWELL { 1.4 } else { 1.7 };
        Vec4::new(
            k,
            angular_frequency(k),
            spread * (2.0 * scatter(1) - 1.0),
            std::f32::consts::TAU * scatter(2),
        )
    })
}

/// A number from 0 to 1 for each whole number, by Wellons's lowbias32 hash.
fn scatter(n: u32) -> f32 {
    let mut x = n.wrapping_add(0x9e37_79b9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// How many of the trains are the swell, longer than 55 cm: always running, even on glassy
/// water, where the shorter ones come with the wind's gusts.
const SWELL: usize = 11;

/// How fast a wave of so many radians a metre turns, radians a second, on deep water:
/// ω² = gk + (σ/ρ)k³, gravity's term and surface tension's (water's 0.0728 N/m over 1000
/// kg/m³), which takes over below about 1.7 cm.
fn angular_frequency(k: f32) -> f32 {
    (9.81 * k + 7.28e-5 * k * k * k).sqrt()
}

/// What the lakes mirror, taken once: the skyline round the start's lake.
#[derive(Clone)]
struct Mirror {
    origin: Vec3,
    east: Vec3,
    north: Vec3,
    skyline: [Vec4; SKYLINE_WAYS],
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
        Self {
            horizon,
            zenith,
            sky_brightness,
            beers_law: BEERS_LAW,
            depth_offset: DEPTH_OFFSET,
            shallow: SHALLOW,
            deep: DEEP,
            foam_from: FOAM_M.0,
            foam_to: FOAM_M.1,
            fade_from: FADE_M.0,
            fade_to: FADE_M.1,
            wind,
            origin: mirror.origin.extend(0.0),
            east: mirror.east.extend(0.0),
            north: mirror.north.extend(0.0),
            skyline: mirror.skyline,
            trains: trains(),
            over_air: air.is_some() as u32,
            air,
        }
    }
}

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

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<LakeMaterial>::default())
        .add_systems(Startup, spawn)
        .add_plugins(MaterialPlugin::<MistMaterial>::default())
        .add_systems(Update, (follow_sky_and_wind, mist_follows_the_morning));
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

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
    };
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

/// The sky the lakes reflect, as it changes, and the wind over them, from the climate where
/// the camera is, taken again every few seconds of the clock.
fn follow_sky_and_wind(
    look: Option<Res<LakeLook>>,
    sky: Option<Res<Sky>>,
    air: Option<Res<AirPicture>>,
    fields: Option<Res<ClimateFields>>,
    clock: Option<Res<Clock>>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut materials: ResMut<Assets<LakeMaterial>>,
    mut last: Local<Option<(f64, Vec4)>>,
) {
    // Without the world's clock, as in the icon baker and the workbench, there is no lake to follow.
    let (Some(look), Some(clock)) = (look, clock) else {
        return;
    };
    let now = clock.time().0;
    let wind = match (&fields, &camera) {
        (Some(fields), Some(camera)) if last.is_none_or(|(when, _)| (now - when).abs() > 0.002) => {
            let at = camera.translation();
            let weather = fields.fields.at(fields.place(at), clock.time());
            let up = at.normalize_or(Vec3::Y);
            // The wind blows from its bearing, clockwise from north.
            let (north, east) = (fields.frame.north(at), fields.frame.east(at));
            let from = north * libm::cosf(weather.wind.from) + east * libm::sinf(weather.wind.from);
            let toward = (-from).reject_from(up).normalize_or_zero();
            let strength = (weather.wind.speed_m_s / 8.0).clamp(0.0, 1.0);
            let wind = toward.extend(strength);
            *last = Some((now, wind));
            wind
        }
        _ => last.map_or(Vec4::ZERO, |(_, wind)| wind),
    };
    let sky_changed = sky.as_ref().is_some_and(|sky| sky.is_changed())
        || air.as_ref().is_some_and(|air| air.is_changed());
    let Some(material) = materials.get(&look.0) else {
        return;
    };
    if !sky_changed && material.extension.wind == wind {
        return;
    }
    let extension = LakeExtension::new(sky.as_deref(), air.as_deref(), wind, &look.1);
    if let Some(peat) = &look.2
        && let Some(mut material) = materials.get_mut(peat)
    {
        material.extension = extension.clone().peaty();
    }
    if let Some(mut material) = materials.get_mut(&look.0) {
        material.extension = extension;
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
    fn waves_run_at_water_speeds() {
        // A deep-water wave 1.56 m long comes round once a second (c = √(gλ/2π)).
        let period = std::f32::consts::TAU / angular_frequency(std::f32::consts::TAU / 1.56);
        assert!((period - 1.0).abs() < 0.01, "{period}");
        // Surface tension makes the slowest wave on water a ripple about 1.7 cm long, running
        // at about 23 cm/s; longer and shorter ones both run faster.
        let (length, speed) = (5..60)
            .map(|mm| {
                let k = std::f32::consts::TAU / (mm as f32 * 0.001);
                (mm as f32 * 0.001, angular_frequency(k) / k)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert!((0.016..0.019).contains(&length), "{length}");
        assert!((speed - 0.231).abs() < 0.005, "{speed}");
    }

    #[test]
    fn the_swell_is_the_long_trains_and_the_ripples_the_short() {
        let trains = trains();
        let length = |t: &Vec4| std::f32::consts::TAU / t.x;
        for (i, train) in trains.iter().enumerate() {
            assert_eq!(
                i < SWELL,
                length(train) > 0.55,
                "train {i}, {} m",
                length(train)
            );
            // Each runs by its own length's speed, so the pattern changes as it goes.
            assert_eq!(train.y, angular_frequency(train.x));
        }
        assert!(trains.windows(2).all(|w| w[0].x < w[1].x), "longest first");
        assert!(
            length(&trains[TRAINS - 1]) < 0.05,
            "down to a gust's ripples"
        );
    }

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
