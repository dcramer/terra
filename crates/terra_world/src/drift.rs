//! What drifts in the air round the eye: dust motes and midges that only show where the sun
//! reaches through, a few leaves falling under the crowns, down blowing over the heath. A
//! world's life at a few metres, as shipped open worlds add it: cheap particles round the
//! camera, many of them, each kind faded in where its place and weather are (Red Dead
//! Redemption 2's and The Witcher 3's ambient insects and motes, Unreal's Niagara fields
//! that follow the camera). Which kinds there are, and where each is seen, is data
//! (`assets/data/drift.ron`, D136); what the place is like is read from the world as it is:
//! how many trunks stand near, water near, the season's and the air's warmth, the hour.
//!
//! Each kind is one mesh of cards moved on the GPU (`shaders/drift.wesl`) from each card's
//! luck, the time and the wind, wrapped in a box that follows the eye, so the field is
//! endless and fixed in the world, never carried along with the camera. The box's frame is
//! moved on by whole boxes, so nothing in it jumps. Its material is touched only when how
//! much of the kind is seen or the wind has changed, a few times a minute.

use crate::climate::ClimateFields;
use crate::daylight::Clock;
use crate::flora::{Obstacles, Trunks};
use crate::planet::{MainCamera, Planet};
use crate::quality::Quality;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use serde::Deserialize;

/// The kinds of drift, as the data file lists them.
#[derive(Resource, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Drifts {
    pub kinds: Vec<DriftKind>,
}

impl Drifts {
    pub const FILE: &str = "drift.ron";
}

/// One kind of thing drifting in the air, and where it is seen.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DriftKind {
    pub name: String,
    /// How many at the high preset; fewer at lower ones.
    pub count: u32,
    pub look: Look,
    /// How big one is, metres across.
    pub size_m: f32,
    /// Its colour, as sRGB, and how much brighter it is seen toward the sun than away
    /// from it (light scattered forward, as dust in a sunbeam).
    pub colour: (f32, f32, f32),
    pub glow: f32,
    pub opacity: f32,
    pub motion: Motion,
    /// The box round the eye it fills: half its width and half its height, metres.
    pub box_m: (f32, f32),
    /// How much of it is seen among trees, in the open and by water, 0 to 1 each; the
    /// most of those the place is.
    pub woods: f32,
    pub open: f32,
    pub water: f32,
    /// The growing season's mean warmth, °C, of the places it is found (D313).
    pub season_c: (f32, f32),
    /// The air's warmth it needs now, °C, at least.
    #[serde(default = "any_air")]
    pub air_c: f32,
    /// The hours it is about, of the clock: one or more spans.
    pub hours: Vec<(f32, f32)>,
    /// What must be falling for it to be seen, for rain and snow (D418); none for a kind the
    /// weather does not bring.
    #[serde(default)]
    pub falls: Option<Falls>,
}

/// A kind the weather brings: rain or snow, all of its cards seen at so many millimetres an hour
/// of water and fewer below, so a light fall is fewer flakes, not fainter ones.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Falls {
    pub snow: bool,
    pub full_mm_h: f32,
}

fn any_air() -> f32 {
    -100.0
}

/// How one is drawn.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// A soft round speck: a mote, a midge, a seed's down.
    Speck,
    /// A leaf, tumbling as it falls.
    Leaf,
    /// A bird far off on the wing: a dark shape beating its wings.
    Bird,
}

/// How one moves.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Motion {
    /// Hanging in the air, wandering so far and sinking so fast (m/s), carried by so much
    /// of the wind.
    Hover {
        wander_m: f32,
        sink_mps: f32,
        carried: f32,
    },
    /// Dancing in so many swarms in the box, each so wide, at so fast a beat.
    Swarm {
        swarms: u32,
        radius_m: f32,
        buzz: f32,
    },
    /// Falling at so many metres a second, swaying so far, turning over so many times a
    /// second, carried by so much of the wind.
    Fall {
        fall_mps: f32,
        sway_m: f32,
        spin_hz: f32,
        carried: f32,
    },
    /// Crossing the sky in so many flocks at so many metres a second, each flock so wide
    /// and so high over the eye (metres, lowest and highest), beating its wings so many
    /// times a second, and about on so much of its crossings, 0 to 1.
    Flock {
        flocks: u32,
        speed_mps: f32,
        spread_m: f32,
        height_m: (f32, f32),
        flap_hz: f32,
        about: f32,
    },
}

/// What the drift shader is told. Field order must match `drift.wesl`.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct DriftExtension {
    /// The box's half width and half height, a card's size in metres, and how much of the
    /// kind is seen now, 0 to 1.
    #[uniform(100)]
    field: Vec4,
    /// How it moves (0 hover, 1 swarm, 2 fall) and its three numbers.
    #[uniform(100)]
    motion: Vec4,
    /// The wind carrying it, metres a second in the box's frame, and how much of it carries
    /// this kind.
    #[uniform(100)]
    wind: Vec4,
    /// Its colour, linear, and its glow toward the sun.
    #[uniform(100)]
    colour: Vec4,
    /// How it is drawn (0 speck, 1 leaf, 2 bird) and how opaque.
    #[uniform(100)]
    look: Vec4,
    /// A flock's: how high over the eye it flies, lowest and highest, its wings' beat, and
    /// the share of its crossings it is about on.
    #[uniform(100)]
    flock: Vec4,
}

impl MaterialExtension for DriftExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/drift.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/drift.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/drift.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/drift.wesl".into()
    }
}

pub type DriftMaterial = ExtendedMaterial<StandardMaterial, DriftExtension>;

/// A kind's field: which kind, its material, how much of it is seen and what its material
/// was last told.
#[derive(Component)]
struct Field {
    kind: usize,
    material: Handle<DriftMaterial>,
    shown: f32,
    wanted: f32,
    told: Vec4,
}

/// The fields' frame: its up, so the boxes stand level, kept until the eye has gone far
/// enough round the planet that it leans.
#[derive(Resource, Default)]
struct Frame {
    rotation: Option<Quat>,
}

/// What the place round the eye is, as a kind of drift asks it, 0 to 1 each.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Round {
    pub woods: f32,
    pub open: f32,
    pub water: f32,
    pub season_c: f32,
    pub air_c: f32,
    pub hour: f32,
    /// How high the eye is over the ground, metres.
    pub above_m: f32,
    /// Rain and snow falling, as water, millimetres an hour (`weather::Weather`).
    pub rain_mm_h: f32,
    pub snow_mm_h: f32,
}

impl DriftKind {
    /// How much of this kind is about at the eye's height over the ground, 0 to 1: what
    /// drifts near the ground (motes, midges, leaves, down) lives in the few metres over it,
    /// so an eye high over a meadow, a crane or a bird's-eye shot, has none of it round it;
    /// flocks fly over whatever height the eye is at.
    pub fn aloft(&self, above_m: f32) -> f32 {
        // Flocks fly over whatever height the eye is at, and rain and snow fall through it.
        if matches!(self.motion, Motion::Flock { .. }) || self.falls.is_some() {
            return 1.0;
        }
        let low = self.box_m.1 + 1.0;
        let high = self.box_m.1 * 2.0 + 4.0;
        1.0 - ((above_m - low) / (high - low)).clamp(0.0, 1.0)
    }

    /// How much of this kind is seen at a place, 0 to 1.
    pub fn amount(&self, round: &Round) -> f32 {
        let (low, high) = self.season_c;
        let season = if (low..=high).contains(&round.season_c) {
            1.0
        } else {
            0.0
        };
        let air = ((round.air_c - self.air_c) / 3.0).clamp(0.0, 1.0);
        // Faded in and out over half an hour at each end of its hours.
        let hours = self
            .hours
            .iter()
            .map(|&(from, to)| {
                ((round.hour - from) / 0.5)
                    .min((to - round.hour) / 0.5)
                    .clamp(0.0, 1.0)
            })
            .fold(0.0, f32::max);
        let place = (self.woods * round.woods)
            .max(self.open * round.open)
            .max(self.water * round.water);
        let falling = self.falls.map_or(1.0, |falls| {
            let rate = if falls.snow {
                round.snow_mm_h
            } else {
                round.rain_mm_h
            };
            (rate / falls.full_mm_h.max(1e-3)).clamp(0.0, 1.0)
        });
        season * air * hours * place * falling * self.aloft(round.above_m)
    }
}

/// How often the place round the eye is looked at again, and how fast a field follows it.
const LOOK_EVERY_S: f32 = 0.5;
const FOLLOW: f32 = 0.4;
/// A wood is so many trunks within so far of the eye's ground.
const WOODED: (f32, f32) = (5.0, 12.0);
/// Water within so far counts as by the water.
const WATER_M: f32 = 12.0;
/// The frame is set level again once its up leans so far from the eye's, in radians.
const LEANS: f32 = 0.03;

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<DriftMaterial>::default())
        .insert_resource(crate::data::read::<Drifts>(Drifts::FILE))
        .init_resource::<Frame>()
        .add_systems(Update, (sow, (follow, sense).chain()));
}

/// A field's cards, every corner at the frame's origin: the first texture coordinate the
/// corner, the second each card's luck and its place in the kind's swarms.
fn cards(count: usize) -> Mesh {
    let mut positions = Vec::with_capacity(count * 4);
    let mut corners = Vec::with_capacity(count * 4);
    let mut seeds = Vec::with_capacity(count * 4);
    let mut normals = Vec::with_capacity(count * 4);
    let mut indices = Vec::with_capacity(count * 6);
    for card in 0..count {
        // The golden ratio's spread, so neighbours differ.
        let luck = (card as f32 * 0.618_034 + 0.31) % 1.0;
        let order = (card as f32 + 0.5) / count as f32;
        let first = positions.len() as u32;
        for corner in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            positions.push([0.0f32, 0.0, 0.0]);
            normals.push([0.0f32, 1.0, 0.0]);
            corners.push(corner);
            seeds.push([luck, order]);
        }
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, corners)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, seeds)
    .with_inserted_indices(Indices::U32(indices))
}

fn share(quality: Quality) -> f32 {
    match quality {
        Quality::Low => 0.35,
        Quality::Medium => 0.6,
        Quality::High => 1.0,
    }
}

/// A field for each kind, once there is a camera to follow.
fn sow(
    mut commands: Commands,
    drifts: Res<Drifts>,
    quality: Res<Quality>,
    camera: Query<(), With<MainCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DriftMaterial>>,
    mut sown: Local<bool>,
) {
    if *sown || camera.is_empty() {
        return;
    }
    *sown = true;
    for (index, kind) in drifts.kinds.iter().enumerate() {
        let count = ((kind.count as f32 * share(*quality)) as usize).max(1);
        let (code, a, b, c) = match kind.motion {
            Motion::Hover {
                wander_m, sink_mps, ..
            } => (0.0, wander_m, sink_mps, 0.0),
            Motion::Swarm {
                swarms,
                radius_m,
                buzz,
            } => (1.0, swarms as f32, radius_m, buzz),
            Motion::Fall {
                fall_mps,
                sway_m,
                spin_hz,
                ..
            } => (2.0, fall_mps, sway_m, spin_hz),
            Motion::Flock {
                flocks,
                speed_mps,
                spread_m,
                ..
            } => (3.0, flocks as f32, speed_mps, spread_m),
        };
        let carried = match kind.motion {
            Motion::Hover { carried, .. } | Motion::Fall { carried, .. } => carried,
            Motion::Swarm { .. } | Motion::Flock { .. } => 0.0,
        };
        let flock = match kind.motion {
            Motion::Flock {
                height_m,
                flap_hz,
                about,
                ..
            } => Vec4::new(height_m.0, height_m.1, flap_hz, about),
            _ => Vec4::ZERO,
        };
        let colour = Color::srgb(kind.colour.0, kind.colour.1, kind.colour.2).to_linear();
        let material = materials.add(DriftMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.9,
                reflectance: 0.0,
                diffuse_transmission: 0.6,
                ..default()
            },
            extension: DriftExtension {
                field: Vec4::new(kind.box_m.0, kind.box_m.1, kind.size_m, 0.0),
                motion: Vec4::new(code, a, b, c),
                wind: Vec4::new(0.0, 0.0, 0.0, carried),
                colour: Vec4::new(colour.red, colour.green, colour.blue, kind.glow),
                look: Vec4::new(
                    match kind.look {
                        Look::Speck => 0.0,
                        Look::Leaf => 1.0,
                        Look::Bird => 2.0,
                    },
                    kind.opacity,
                    // A fall thins by its cards, not by fading them.
                    if kind.falls.is_some() { 1.0 } else { 0.0 },
                    0.0,
                ),
                flock,
            },
        });
        commands.spawn((
            Name::new(format!("Drift: {}", kind.name)),
            Mesh3d(meshes.add(cards(count))),
            MeshMaterial3d(material.clone()),
            Transform::IDENTITY,
            // Its cards are wherever the eye is, so it is never culled.
            NoFrustumCulling,
            NotShadowCaster,
            Visibility::Hidden,
            Field {
                kind: index,
                material,
                shown: 0.0,
                wanted: 0.0,
                told: Vec4::splat(-1.0),
            },
        ));
    }
}

/// Keeps each field's frame round the eye: level under it, and moved on by whole boxes, so
/// the cards stay where they are in the world.
fn follow(
    drifts: Res<Drifts>,
    mut frame: ResMut<Frame>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut fields: Query<(&Field, &mut Transform)>,
) {
    let Some(eye) = camera.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let up = eye.normalize_or(Vec3::Y);
    let rotation = match frame.rotation {
        Some(rotation) if (rotation * Vec3::Y).angle_between(up) < LEANS => rotation,
        _ => {
            let rotation = Quat::from_rotation_arc(Vec3::Y, up);
            frame.rotation = Some(rotation);
            rotation
        }
    };
    for (field, mut at) in &mut fields {
        let kind = &drifts.kinds[field.kind];
        let size = Vec3::new(kind.box_m.0, kind.box_m.1, kind.box_m.0) * 2.0;
        // The origin is a whole number of boxes from the world's middle in the frame, so a
        // field put round the eye anew lies just where it did.
        let local = rotation.inverse() * eye;
        let cell = (local / size).round() * size;
        let origin = rotation * cell;
        if at.rotation != rotation || at.translation.distance(origin) > 0.01 {
            *at = Transform::from_translation(origin).with_rotation(rotation);
        }
    }
}

/// Looks at the place round the eye now and then and fades each kind toward how much of it
/// is seen there, carried by the wind there.
#[allow(clippy::too_many_arguments)]
fn sense(
    time: Res<Time>,
    drifts: Res<Drifts>,
    frame: Res<Frame>,
    clock: Res<Clock>,
    planet: Option<Res<Planet>>,
    climate: Option<Res<ClimateFields>>,
    trunks: Res<Trunks>,
    obstacles: Res<Obstacles>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut fields: Query<(&mut Field, &mut Visibility)>,
    mut materials: ResMut<Assets<DriftMaterial>>,
    mut since: Local<f32>,
    mut wind: Local<Vec3>,
    weather: Option<Res<crate::weather::Weather>>,
) {
    let (Some(planet), Some(eye)) = (
        planet,
        camera.iter().next().map(GlobalTransform::translation),
    ) else {
        return;
    };
    let step = time.delta_secs();
    *since -= step;
    if *since <= 0.0 {
        *since = LOOK_EVERY_S;
        let ground = planet.ground_at(eye);
        let mut round = round_at(
            &planet,
            climate.as_deref(),
            &clock,
            &trunks,
            &obstacles,
            ground,
        );
        round.above_m = (eye.length() - ground.length()).max(0.0);
        if let Some(weather) = weather.as_deref() {
            round.rain_mm_h = weather.rain_mm_h();
            round.snow_mm_h = weather.snow_mm_h();
        }
        for (mut field, _) in &mut fields {
            field.wanted = drifts.kinds[field.kind].amount(&round);
        }
        *wind = match &climate {
            Some(climate) => {
                let now = climate.fields.at(climate.place(ground), clock.time()).wind;
                let up = ground.normalize_or(Vec3::Y);
                let north = climate.frame.north(ground);
                let east = north.cross(up);
                let to = -(north * libm::cosf(now.from) + east * libm::sinf(now.from));
                to * now.speed_m_s.min(12.0)
            }
            None => Vec3::new(0.6, 0.0, 0.3),
        };
    }
    let rotation = frame.rotation.unwrap_or_default();
    let follow = 1.0 - libm::expf(-step * FOLLOW * 4.0);
    for (mut field, mut visibility) in &mut fields {
        field.shown += (field.wanted - field.shown) * follow;
        let seen = field.shown > 0.01;
        let wanted = if seen {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if !seen {
            continue;
        }
        let told = (rotation.inverse() * *wind).extend(field.shown);
        if told.distance(field.told) > 0.02
            && let Some(mut material) = materials.get_mut(&field.material)
        {
            material.extension.wind = told.truncate().extend(material.extension.wind.w);
            material.extension.field.w = field.shown;
            field.told = told;
        }
    }
}

/// What the place at a point on the ground is like, as drift asks it.
fn round_at(
    planet: &Planet,
    climate: Option<&ClimateFields>,
    clock: &Clock,
    trunks: &Trunks,
    obstacles: &Obstacles,
    ground: Vec3,
) -> Round {
    let up = ground.normalize_or(Vec3::Y);
    let depth = |point: Vec3| planet.water_radius_m(point) - planet.ground_at(point).length();
    let side = up.any_orthonormal_vector();
    let wet = depth(ground) > 0.0
        || (0..8).any(|i| {
            let turn = Quat::from_axis_angle(up, i as f32 * std::f32::consts::TAU / 8.0);
            depth(ground + turn * side * WATER_M) > 0.05
        });
    let standing = trunks.near(ground, WOODED.1, obstacles).count() as f32;
    let woods = (standing / WOODED.0).clamp(0.0, 1.0);
    let (season_c, air_c) = match climate {
        Some(climate) => {
            let place = climate.place(ground);
            (
                climate.fields.season(place).growing_mean_c,
                climate.fields.at(place, clock.time()).temperature_c,
            )
        }
        None => (12.0, 15.0),
    };
    Round {
        woods,
        open: 1.0 - woods,
        water: if wet { 1.0 } else { 0.0 },
        season_c,
        air_c,
        hour: clock.hour,
        above_m: 0.0,
        rain_mm_h: 0.0,
        snow_mm_h: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drifts() -> Drifts {
        crate::data::read(Drifts::FILE)
    }

    fn kind(name: &str) -> DriftKind {
        drifts()
            .kinds
            .into_iter()
            .find(|kind| kind.name == name)
            .unwrap_or_else(|| panic!("no {name} in the drift file"))
    }

    fn forest_morning() -> Round {
        Round {
            woods: 1.0,
            open: 0.0,
            water: 0.0,
            season_c: 12.0,
            air_c: 14.0,
            hour: 7.5,
            above_m: 1.7,
            rain_mm_h: 0.0,
            snow_mm_h: 0.0,
        }
    }

    #[test]
    fn the_drift_file_reads_and_every_kind_is_somewhere() {
        let drifts = drifts();
        assert!(!drifts.kinds.is_empty());
        for kind in &drifts.kinds {
            assert!(kind.count > 0 && kind.size_m > 0.0, "{}", kind.name);
            assert!(
                kind.woods.max(kind.open).max(kind.water) > 0.0,
                "{}",
                kind.name
            );
            assert!(!kind.hours.is_empty(), "{}", kind.name);
        }
    }

    #[test]
    fn motes_show_in_a_wood_by_day_and_not_at_midnight() {
        let motes = kind("Dust motes");
        assert!(motes.amount(&forest_morning()) > 0.9);
        let night = Round {
            hour: 0.5,
            ..forest_morning()
        };
        assert_eq!(motes.amount(&night), 0.0);
    }

    #[test]
    fn insects_need_warm_air_and_leaves_need_trees() {
        let midges = kind("Midges");
        let cold = Round {
            air_c: 2.0,
            water: 1.0,
            ..forest_morning()
        };
        assert_eq!(midges.amount(&cold), 0.0);
        let leaves = kind("Falling leaves");
        let open = Round {
            woods: 0.0,
            open: 1.0,
            ..forest_morning()
        };
        assert!(leaves.amount(&forest_morning()) > 0.5);
        assert_eq!(leaves.amount(&open), 0.0);
    }

    #[test]
    fn a_kind_fades_in_at_the_start_of_its_hours() {
        let motes = kind("Dust motes");
        let (from, _) = motes.hours[0];
        let at = |hour| {
            motes.amount(&Round {
                hour,
                ..forest_morning()
            })
        };
        assert!(at(from) < 0.05);
        assert!(at(from + 0.25) > 0.3 && at(from + 0.25) < 0.7);
        assert!(at(from + 1.0) > 0.95);
    }

    /// What drifts by the ground is round an eye at a walker's height and gone from one high
    /// over the ground, as a crane's or a bird's-eye shot's; flocks fly over either.
    #[test]
    fn ground_drift_stays_by_the_ground_and_flocks_fly_over_any_eye() {
        let high = Round {
            above_m: 40.0,
            woods: 0.0,
            open: 1.0,
            ..forest_morning()
        };
        let low = Round {
            above_m: 1.7,
            ..high
        };
        for kind in drifts().kinds {
            if matches!(kind.motion, Motion::Flock { .. }) {
                assert_eq!(kind.aloft(40.0), 1.0, "{}", kind.name);
            } else {
                assert_eq!(kind.amount(&high), 0.0, "{}", kind.name);
                assert_eq!(kind.aloft(1.7), 1.0, "{}", kind.name);
            }
        }
        assert!(kind("Dust motes").amount(&low) > 0.1);
    }

    #[test]
    fn snow_falls_only_when_snow_does_and_more_of_it_the_harder() {
        let flakes = kind("Snowfall");
        let mut winter = Round {
            season_c: 12.0,
            air_c: -4.0,
            hour: 11.0,
            ..forest_morning()
        };
        assert_eq!(flakes.amount(&winter), 0.0, "nothing falling");
        winter.rain_mm_h = 2.0;
        assert_eq!(flakes.amount(&winter), 0.0, "rain is not snow");
        winter.rain_mm_h = 0.0;
        winter.snow_mm_h = 0.3;
        let light = flakes.amount(&winter);
        winter.snow_mm_h = 3.0;
        let heavy = flakes.amount(&winter);
        assert!(
            light > 0.0 && heavy > light && heavy <= 1.0,
            "{light} {heavy}"
        );
        // Open ground and the wood alike.
        winter.woods = 0.0;
        winter.open = 1.0;
        assert_eq!(flakes.amount(&winter), heavy);
    }
}
