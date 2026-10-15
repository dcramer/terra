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

/// Pools smaller than this are the ground's wet hollows, not drawn as water.
const SMALLEST_M2: f32 = 300.0;
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
            over_air: air.is_some() as u32,
            air,
        }
    }
}

/// The lakes' one material, and what it mirrors.
#[derive(Resource)]
struct LakeLook(Handle<LakeMaterial>, Mirror);

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<LakeMaterial>::default())
        .add_systems(Startup, spawn)
        .add_systems(Update, follow_sky_and_wind);
}

fn spawn(
    mut commands: Commands,
    planet: Res<Planet>,
    start: Res<crate::start::Start>,
    sky: Option<Res<Sky>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LakeMaterial>>,
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
            ..default()
        },
        extension: LakeExtension::new(sky.as_deref(), None, Vec4::ZERO, &mirror),
    });
    let mut drawn = 0;
    for (index, lake) in waters.lakes().iter().enumerate() {
        if lake.area_m2 < SMALLEST_M2 {
            continue;
        }
        commands.spawn((
            Name::new(format!("Lake {index}")),
            Mesh3d(meshes.add(mesh(waters, index))),
            MeshMaterial3d(look.clone()),
            NotShadowCaster,
        ));
        drawn += 1;
    }
    info!(
        "Lakes: {drawn} drawn of {} found round the start",
        waters.lakes().len()
    );
    // The patterned ground's pools, many and small, drawn as one surface, each at its level.
    if let Some(pools) = &round.pools {
        commands.spawn((
            Name::new("Pools"),
            Mesh3d(meshes.add(pools_mesh(pools))),
            MeshMaterial3d(look.clone()),
            NotShadowCaster,
        ));
        info!(
            "Lakes: {} pools in the patterned ground round the start",
            pools.lakes().len()
        );
    }
    commands.insert_resource(LakeLook(look, mirror));
}

/// The sky the lakes reflect, as it changes, and the wind over them, from the climate where
/// the camera is, taken again every few seconds of the clock.
fn follow_sky_and_wind(
    look: Option<Res<LakeLook>>,
    sky: Option<Res<Sky>>,
    air: Option<Res<AirPicture>>,
    fields: Option<Res<ClimateFields>>,
    clock: Res<Clock>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut materials: ResMut<Assets<LakeMaterial>>,
    mut last: Local<Option<(f64, Vec4)>>,
) {
    let Some(look) = look else { return };
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
    if let Some(mut material) = materials.get_mut(&look.0) {
        material.extension = LakeExtension::new(sky.as_deref(), air.as_deref(), wind, &look.1);
    }
}

/// Every lake of a grid as one surface, as many small pools are best drawn: each cell under a
/// lake at its level, and each cell beside one at the highest level beside it, so the water's
/// edge runs under the shore.
fn pools_mesh(waters: &Waters) -> Mesh {
    let region = waters.region();
    let n = region.cells;
    let corner = |c: usize, r: usize, radius: f32| {
        let at = region.offset(c, r) + Vec2::new(-0.5, 0.5) * region.cell_m;
        (region.middle + region.east * at.x + region.north * at.y).normalize() * radius
    };
    let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for r in 0..n {
        for c in 0..n {
            let level = waters
                .lake_at(c, r)
                .map(|l| waters.lake_radius_m(l))
                .or_else(|| {
                    let mut best: Option<f32> = None;
                    for (dc, dr) in [
                        (-1i64, -1i64),
                        (0, -1),
                        (1, -1),
                        (-1, 0),
                        (1, 0),
                        (-1, 1),
                        (0, 1),
                        (1, 1),
                    ] {
                        let (cc, rr) = (c as i64 + dc, r as i64 + dr);
                        if cc < 0 || rr < 0 || cc >= n as i64 || rr >= n as i64 {
                            continue;
                        }
                        if let Some(l) = waters.lake_at(cc as usize, rr as usize) {
                            let here = waters.lake_radius_m(l);
                            best = Some(best.map_or(here, |b: f32| b.max(here)));
                        }
                    }
                    best
                });
            let Some(radius) = level else { continue };
            let base = positions.len() as u32;
            for (dc, dr) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                let p = corner(c + dc, r + dr, radius);
                positions.push(p.to_array());
                normals.push(p.normalize().to_array());
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
    .with_inserted_indices(Indices::U32(indices))
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
    let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::new());
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
    .with_inserted_indices(Indices::U32(indices))
}
