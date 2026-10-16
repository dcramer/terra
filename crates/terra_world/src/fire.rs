//! Fire as it is drawn (D377): flames, embers and smoke wherever something burns, a jet of
//! flame where something breathes fire, and the light they give. What burns, and where, is
//! the layer above's to say ([`Blazes`]); this draws it, as smoke (D325) is drawn: instanced
//! soft cards, moved on the GPU from each card's seed, the time and the wind, on our own
//! materials, with no particle library.
//!
//! As shipped games draw fire (research/2026-10-15-dragons-and-fire.md,
//! research/2026-10-16-real-time-fire-rendering.md): a body sheet to a blaze where it burns
//! and a few flamelets breaking off its top, eroded by noise scrolled up through them, their
//! colour glowing soot's at its temperature (1050 to 1650 K: deep red, orange, a yellow
//! heart, never white or pink by day), faded in where they meet what is behind; embers as
//! sparks stretched along their flight, rising with the plume and carried downwind; smoke
//! above as near-black lit cards paling as they climb, glowing from under where the fire is;
//! heat haze over the flames; and a few point lights where the fire is greatest, flickering
//! as a fire that size puffs, not one a flame.
//!
//! Every blaze is a slot of one buffer the GPU reads, so all flames are one draw, all
//! embers another, all smoke a third, however many burn; how many slots, by the preset.

use crate::climate::ClimateFields;
use crate::daylight::Clock;
use crate::quality::Quality;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{render_resource::AsBindGroup, storage::ShaderBuffer},
    shader::ShaderRef,
};

/// Something burning, as the layer above says it: where its flames stand from (their foot,
/// in the world), how tall they are in metres, how hot (0 to 1, a dying fire's embers to a
/// blaze's heart), and a number of its own so no two flicker alike.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blaze {
    pub at: Vec3,
    pub tall: f32,
    pub heat: f32,
    pub seed: f32,
}

/// A jet of flame breathed out: from where, which way, how far it reaches, how wide it
/// spreads (the tangent of its half angle), and whether it is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jet {
    pub from: Vec3,
    pub way: Vec3,
    pub reach: f32,
    pub spread: f32,
    pub on: bool,
}

/// Everything burning now, set by whatever owns the fires each frame: the blazes, the
/// largest first (those past the preset's slots are not drawn), and the jets.
#[derive(Resource, Default, Debug)]
pub struct Blazes {
    pub blazes: Vec<Blaze>,
    pub jets: Vec<Jet>,
}

/// How a fire's light wavers at a time, for a light of its own number and so bright: as a
/// fire that size puffs (1.5 / sqrt(D) Hz, D its width, taken from its light), up to 4 Hz,
/// by a fifth either way for a small fire and less for a big one, whose many flames even
/// out; and how far it bobs up and down, in metres.
fn flicker(t: f32, k: f32, lumens: f32) -> (f32, f32) {
    let wide = (lumens / LUMENS_A_METRE).sqrt().clamp(0.5, 8.0);
    let hz = (1.5 / wide.sqrt()).min(4.0);
    let reach = 0.22 - 0.14 * ((wide - 0.5) / 7.5);
    let a = core::f32::consts::TAU * hz * t + k;
    let wave = 0.55 * libm::sinf(a)
        + 0.3 * libm::sinf(a * 2.31 + 1.3 * k)
        + 0.15 * libm::sinf(a * 4.7 + 0.4);
    (1.0 + reach * wave, 0.2 * libm::sinf(a * 0.7 + 2.0 * k))
}

/// How many blazes are drawn at most, by preset.
pub fn slots(quality: Quality) -> usize {
    match quality {
        Quality::Low => 384,
        Quality::Medium => 1024,
        Quality::High => 2048,
    }
}

/// How many cards a blaze has of each: flames, embers, smoke and heat haze.
const FLAME_CARDS: usize = 3;
const EMBER_CARDS: usize = 5;
const SMOKE_CARDS: usize = 4;
const HAZE_CARDS: usize = 1;
/// How many cards a jet is, and how many jets are drawn.
const JET_CARDS: usize = 360;
const JETS: usize = 2;
/// How many point lights the fires are lit by, and how far round each its fire is gathered,
/// in metres.
const LIGHTS: usize = 8;
const LIGHT_CELL_M: f32 = 9.0;
/// A blaze's light, in lumens for a blaze a metre tall at full heat, how far it reaches, and
/// the most one light gives.
const LUMENS_A_METRE: f32 = 700_000.0;
const MOST_LUMENS: f32 = 3.0e7;
const LIGHT_RANGE_M: f32 = 55.0;
/// A jet's light at its mouth and where it reaches.
const JET_LUMENS: f32 = 4.0e6;
/// The least a fire's smoke drifts, metres a second.
const LEAST_DRAUGHT_MPS: f32 = 1.6;

/// What a fire's material is told. Field order must match `fire.wesl`.
#[derive(Clone, Copy, Debug, Default, Reflect, bevy::render::render_resource::ShaderType)]
pub struct FireUniform {
    /// x which cards (0 flames, 1 embers, 2 jets), y how many slots are filled, z the time
    /// the cards move by, w unused.
    kind: Vec4,
    /// The wind where the fire is, metres a second in the world, and its strength.
    wind: Vec4,
    /// Each jet: from, and its reach; its way, and its spread; whether it is on (1) or since
    /// when off, and when it came on.
    jets: [Vec4; JETS * 3],
}

/// Flames, embers and jets: drawn unlit, their light added to what is behind, on Bevy's own
/// material as smoke is, its shaders ours.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FireExtension {
    #[uniform(100)]
    uniform: FireUniform,
    #[storage(101, read_only)]
    slots: Handle<ShaderBuffer>,
    /// The flames' flipbook (`tools/textures/flames.ts`): three simulated fires' heat frame by
    /// frame, one a channel, linear.
    #[texture(102)]
    #[sampler(103)]
    flipbook: Handle<Image>,
}

/// The flames' flipbook.
const FLIPBOOK: &str = "textures/fire/flames.png";

impl MaterialExtension for FireExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }
}

pub type FireMaterial = ExtendedMaterial<StandardMaterial, FireExtension>;

/// What the smoke over a fire is told. Field order must match `fire_smoke.wesl`.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FireSmokeExtension {
    /// The wind, and how many slots are filled.
    #[uniform(100)]
    wind: Vec4,
    #[storage(101, read_only)]
    slots: Handle<ShaderBuffer>,
}

impl MaterialExtension for FireSmokeExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }
}

pub type FireSmokeMaterial = ExtendedMaterial<StandardMaterial, FireSmokeExtension>;

/// The fires' drawing: their buffer, their materials, and their lights.
#[derive(Resource)]
struct Drawn {
    slots: Handle<ShaderBuffer>,
    fire: [Handle<FireMaterial>; 4],
    smoke: Handle<FireSmokeMaterial>,
    lights: Vec<Entity>,
    /// When each jet came on or went off, by the cards' clock.
    jets: [(bool, f32); JETS],
    wind: Vec3,
    wind_due: f32,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Blazes>()
        .add_plugins((
            MaterialPlugin::<FireMaterial>::default(),
            MaterialPlugin::<FireSmokeMaterial>::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, (blow, show, light).chain());
}

/// A mesh of cards, `count` of them, every corner at the origin: the first texture
/// coordinate the corner, the second which card and of which slot.
fn cards(slots: usize, each: usize) -> Mesh {
    let count = slots * each;
    let mut positions = Vec::with_capacity(count * 4);
    let mut normals = Vec::with_capacity(count * 4);
    let mut corners = Vec::with_capacity(count * 4);
    let mut ids = Vec::with_capacity(count * 4);
    let mut indices = Vec::with_capacity(count * 6);
    for slot in 0..slots {
        for card in 0..each {
            let first = positions.len() as u32;
            for corner in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
                positions.push([0.0f32, 0.0, 0.0]);
                normals.push([0.0f32, 1.0, 0.0]);
                corners.push(corner);
                ids.push([slot as f32, card as f32 / each as f32]);
            }
            indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, corners)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, ids)
    .with_inserted_indices(Indices::U32(indices))
}

fn setup(
    mut commands: Commands,
    quality: Res<Quality>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    assets: Res<AssetServer>,
    mut fires: ResMut<Assets<FireMaterial>>,
    mut smokes: ResMut<Assets<FireSmokeMaterial>>,
) {
    let count = slots(*quality);
    let slots = buffers.add(ShaderBuffer::from(vec![[0.0f32; 4]; count * 2]));
    // Heat, not colour: read as it is stored.
    let flipbook: Handle<Image> = assets
        .load_builder()
        .with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
            settings.is_srgb = false;
        })
        .load(FLIPBOOK);
    let fire = [0.0, 1.0, 2.0, 3.0].map(|kind| {
        // The haze reads the opaque scene as Bevy copies it for transmissive materials, so it
        // is drawn among them, blended over it.
        let haze = kind == 3.0;
        fires.add(FireMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                alpha_mode: if haze {
                    AlphaMode::Blend
                } else {
                    AlphaMode::Add
                },
                specular_transmission: if haze { 1.0 } else { 0.0 },
                unlit: true,
                ..default()
            },
            extension: FireExtension {
                uniform: FireUniform {
                    kind: Vec4::new(kind, 0.0, 0.0, 0.0),
                    ..default()
                },
                slots: slots.clone(),
                flipbook: flipbook.clone(),
            },
        })
    });
    let smoke = smokes.add(FireSmokeMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            diffuse_transmission: 0.1,
            ..default()
        },
        extension: FireSmokeExtension {
            wind: Vec4::ZERO,
            slots: slots.clone(),
        },
    });
    // Drawn wherever the fires are: the cards are placed on the GPU, so no bounds would hold.
    let mut spawn = |name: &str, mesh: Mesh, material: Handle<FireMaterial>| {
        commands.spawn((
            Name::new(name.to_string()),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            NotShadowCaster,
        ));
    };
    spawn("Fire: flames", cards(count, FLAME_CARDS), fire[0].clone());
    spawn("Fire: embers", cards(count, EMBER_CARDS), fire[1].clone());
    spawn("Fire: jets", cards(JETS, JET_CARDS), fire[2].clone());
    spawn("Fire: haze", cards(count, HAZE_CARDS), fire[3].clone());
    commands.spawn((
        Name::new("Fire: smoke"),
        Mesh3d(meshes.add(cards(count, SMOKE_CARDS))),
        MeshMaterial3d(smoke.clone()),
        Transform::IDENTITY,
        NoFrustumCulling,
        NotShadowCaster,
    ));
    let lights = (0..LIGHTS)
        .map(|i| {
            commands
                .spawn((
                    Name::new(format!("Fire: light {i}")),
                    PointLight {
                        color: Color::srgb(1.0, 0.55, 0.22),
                        intensity: 0.0,
                        range: LIGHT_RANGE_M,
                        radius: 1.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::IDENTITY,
                    Visibility::Hidden,
                ))
                .id()
        })
        .collect();
    commands.insert_resource(Drawn {
        slots,
        fire,
        smoke,
        lights,
        jets: [(false, -100.0); JETS],
        wind: Vec3::ZERO,
        wind_due: 0.0,
    });
}

/// The wind where the fires are, as the climate has it, looked up now and then.
fn blow(
    time: Res<Time>,
    clock: Res<Clock>,
    climate: Option<Res<ClimateFields>>,
    blazes: Res<Blazes>,
    mut drawn: ResMut<Drawn>,
) {
    drawn.wind_due -= time.delta_secs();
    let Some(here) = blazes.blazes.first().map(|b| b.at) else {
        return;
    };
    if drawn.wind_due > 0.0 {
        return;
    }
    drawn.wind_due = 3.0;
    let up = here.normalize_or(Vec3::Y);
    let (speed, from, north) = match &climate {
        Some(climate) => {
            let wind = climate.fields.at(climate.place(here), clock.time()).wind;
            (wind.speed_m_s, wind.from, climate.frame.north(here))
        }
        None => (
            1.5,
            0.8,
            Vec3::Y
                .reject_from(up)
                .normalize_or(up.any_orthonormal_vector()),
        ),
    };
    let east = north.cross(up);
    let blowing_to = -(north * libm::cosf(from) + east * libm::sinf(from));
    // A fire makes its own draught and the air is never quite still over one: its smoke
    // leans at least as a light air carries it.
    drawn.wind = blowing_to * speed.clamp(LEAST_DRAUGHT_MPS, 10.0);
}

/// The blazes into the buffer, the hottest and tallest first, and the materials told how
/// many there are, the wind and the jets.
fn show(
    time: Res<Time>,
    quality: Res<Quality>,
    blazes: Res<Blazes>,
    mut drawn: ResMut<Drawn>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut fires: ResMut<Assets<FireMaterial>>,
    mut smokes: ResMut<Assets<FireSmokeMaterial>>,
) {
    let now = time.elapsed_secs_wrapped();
    let count = blazes.blazes.len().min(slots(*quality));
    if let Some(mut buffer) = buffers.get_mut(&drawn.slots) {
        let mut data: Vec<[f32; 4]> = Vec::with_capacity(slots(*quality) * 2);
        for blaze in blazes.blazes.iter().take(count) {
            data.push([blaze.at.x, blaze.at.y, blaze.at.z, blaze.tall]);
            data.push([blaze.heat, blaze.seed, 0.0, 0.0]);
        }
        data.resize(slots(*quality) * 2, [0.0; 4]);
        *buffer = ShaderBuffer::from(data);
    }
    let mut jets = [Vec4::ZERO; JETS * 3];
    for (i, state) in drawn.jets.iter_mut().enumerate() {
        let jet = blazes.jets.get(i).copied();
        let on = jet.is_some_and(|jet| jet.on);
        if on != state.0 {
            *state = (on, now);
        }
        let jet = jet.unwrap_or(Jet {
            from: Vec3::ZERO,
            way: Vec3::Z,
            reach: 0.0,
            spread: 0.0,
            on: false,
        });
        jets[i * 3] = jet.from.extend(jet.reach);
        jets[i * 3 + 1] = jet.way.normalize_or(Vec3::Z).extend(jet.spread);
        jets[i * 3 + 2] = Vec4::new(f32::from(u8::from(state.0)), state.1, 0.0, 0.0);
    }
    let wind = drawn.wind.extend(drawn.wind.length());
    for (kind, handle) in drawn.fire.iter().enumerate() {
        if let Some(mut material) = fires.get_mut(handle) {
            material.extension.uniform.kind = Vec4::new(kind as f32, count as f32, now, 0.0);
            material.extension.uniform.wind = wind;
            material.extension.uniform.jets = jets;
        }
    }
    if let Some(mut material) = smokes.get_mut(&drawn.smoke) {
        material.extension.wind = drawn.wind.extend(count as f32);
    }
}

/// The fires' light: each of a few lights where the most fire is, gathered over cells of a
/// few metres, at the cell's fire's middle and as bright as it burns, wavering; and a light
/// at each jet's mouth and where it reaches.
fn light(
    time: Res<Time>,
    blazes: Res<Blazes>,
    drawn: Res<Drawn>,
    mut lights: Query<(&mut PointLight, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_secs_wrapped();
    let mut cells: std::collections::BTreeMap<[i64; 3], (Vec3, f32)> =
        std::collections::BTreeMap::new();
    for blaze in &blazes.blazes {
        let weight = blaze.tall * blaze.heat;
        let key = (blaze.at / LIGHT_CELL_M).floor().as_i64vec3().to_array();
        let cell = cells.entry(key).or_insert((Vec3::ZERO, 0.0));
        cell.0 += blaze.at * weight;
        cell.1 += weight;
    }
    let mut lit: Vec<(Vec3, f32)> = blazes
        .jets
        .iter()
        .filter(|jet| jet.on)
        .flat_map(|jet| {
            let way = jet.way.normalize_or(Vec3::Z);
            [
                (jet.from + way * jet.reach * 0.25, JET_LUMENS),
                (jet.from + way * jet.reach * 0.85, JET_LUMENS),
            ]
        })
        .collect();
    let mut gathered: Vec<(Vec3, f32)> = cells
        .into_values()
        .filter(|(_, weight)| *weight > 0.0)
        .map(|(sum, weight)| (sum / weight, (weight * LUMENS_A_METRE).min(MOST_LUMENS)))
        .collect();
    gathered.sort_by(|a, b| b.1.total_cmp(&a.1));
    lit.extend(gathered);
    for (i, &entity) in drawn.lights.iter().enumerate() {
        let Ok((mut light, mut transform, mut visibility)) = lights.get_mut(entity) else {
            continue;
        };
        match lit.get(i) {
            Some(&(at, lumens)) => {
                let up = at.normalize_or(Vec3::Y);
                let (waver, bob) = flicker(t, i as f32 * 1.7, lumens);
                light.intensity = lumens * waver;
                transform.translation = at + up * (1.5 + bob);
                *visibility = Visibility::Inherited;
            }
            None => {
                light.intensity = 0.0;
                *visibility = Visibility::Hidden;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How far a light wavers over ten seconds, and how many times a second it turns.
    fn wavers(lumens: f32) -> (f32, f32) {
        let samples: Vec<f32> = (0..10_000)
            .map(|i| flicker(i as f32 * 0.001, 0.0, lumens).0)
            .collect();
        let (low, high) = samples
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), &w| (l.min(w), h.max(w)));
        let turns = samples
            .windows(3)
            .filter(|w| (w[1] - w[0]) * (w[2] - w[1]) < 0.0)
            .count();
        (high - low, turns as f32 / 10.0)
    }

    #[test]
    fn a_small_fires_light_flickers_quicker_and_further_than_a_big_ones() {
        let (small_reach, small_turns) = wavers(LUMENS_A_METRE * 0.5);
        let (big_reach, big_turns) = wavers(LUMENS_A_METRE * 40.0);
        assert!(small_reach > big_reach * 1.5, "{small_reach} {big_reach}");
        assert!(small_turns > big_turns * 1.5, "{small_turns} {big_turns}");
        // Never out, never doubled.
        for lumens in [LUMENS_A_METRE * 0.1, LUMENS_A_METRE * 100.0] {
            for i in 0..1000 {
                let (waver, _) = flicker(i as f32 * 0.01, 1.0, lumens);
                assert!((0.6..=1.4).contains(&waver), "{waver}");
            }
        }
    }
}
