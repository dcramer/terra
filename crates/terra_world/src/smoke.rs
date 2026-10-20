//! Smoke (D325): a column of soft cards rising from where something burns, a hearth's
//! through a chimney or the thatch, a forge's, drifting with the climate's wind and thinning
//! as it rises. An emitter is an entity with [`Smoke`]; what burns is the layer above's to
//! say. Each is drawn as one mesh of cards, moved on the GPU from each card's phase, the
//! time and the wind (`shaders/smoke.wesl`), on our own material, with no particle library;
//! how many cards, by the quality preset.

use crate::climate::ClimateFields;
use crate::daylight::Clock;
use crate::quality::Quality;
use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

/// What smoke is: a hearth's, thin and blue-grey, or a forge's, thicker and darker; or, with
/// nothing burning, snow the wind lifts off a patch and carries low along the ground (D325),
/// only once it blows hard enough.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Burning {
    Hearth,
    Forge,
    Snow,
}

/// Snow lifts off a patch from this wind, in metres a second, and drifts thickest from that:
/// loose snow starts moving at about 5 m/s at a person's height, and blows in sheets by 10.
const SNOW_LIFTS_M_S: (f32, f32) = (4.5, 9.0);

/// Smoke rising from here: what is burning, and how thick it is now, 0 for none to 1. The
/// layer that owns the fire sets the thickness; the smoke follows it over a few seconds.
#[derive(Component, Clone, Copy, Debug)]
pub struct Smoke {
    pub burning: Burning,
    pub thickness: f32,
}

/// What the smoke shader is told. Field order must match `smoke.wesl`.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SmokeExtension {
    /// Where the wind carries it, metres a second in the world; and how thick it is.
    #[uniform(100)]
    wind: Vec4,
    /// How long a card lives in seconds, how fast it rises, and how wide it is born and ends.
    #[uniform(100)]
    shape: Vec4,
    /// Its colour, linear, and how opaque a card is at its thickest.
    #[uniform(100)]
    colour: Vec4,
}

impl MaterialExtension for SmokeExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/smoke.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/smoke.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/smoke.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/smoke.wesl".into()
    }
}

pub type SmokeMaterial = ExtendedMaterial<StandardMaterial, SmokeExtension>;

/// An emitter's drawn column: its material, and how thick it is shown.
#[derive(Component)]
struct Drawn {
    material: Handle<SmokeMaterial>,
    shown: f32,
    /// The wind where it stands, when it is next looked up, and what the material was last
    /// told.
    wind: Vec3,
    due: f32,
    told: Vec4,
}

/// How often an emitter's wind is looked up again, in seconds.
const WIND_EVERY_S: f32 = 3.0;

/// How many cards a column is, by preset.
fn cards(quality: Quality) -> usize {
    match quality {
        Quality::Low => 10,
        Quality::Medium => 16,
        Quality::High | Quality::Ultra => 24,
    }
}

/// Each kind's life, rise, sizes, colour and opacity.
fn look(burning: Burning) -> (Vec4, Vec4) {
    match burning {
        // Wood smoke from a hearth: blue-grey, thin, rising slowly and spreading wide. Its
        // albedo is a grey's, as smoke's is (a white card glows brighter than the land under a
        // dusk sky), and a card is a wisp, so only where many overlap low over the roof does
        // it read as a column.
        Burning::Hearth => (
            Vec4::new(14.0, 0.9, 0.7, 5.0),
            Vec4::new(0.3, 0.31, 0.33, 0.2),
        ),
        // A forge's coal and charcoal: darker, thicker, quicker.
        Burning::Forge => (
            Vec4::new(10.0, 1.4, 0.8, 4.0),
            Vec4::new(0.2, 0.2, 0.2, 0.32),
        ),
        // Blown snow: short-lived veils hugging the ground, barely rising, white and thin.
        Burning::Snow => (
            Vec4::new(2.6, 0.18, 0.5, 2.4),
            Vec4::new(0.8, 0.82, 0.87, 0.24),
        ),
    }
}

/// How quickly the shown smoke follows its thickness, a share a second.
const FOLLOW: f32 = 0.35;

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<SmokeMaterial>::default())
        .add_systems(Update, (draw, blow).chain());
}

/// The cards of a column, every corner at its foot: the first texture coordinate the
/// corner, the second each card's phase through its life and its luck.
fn column(count: usize) -> Mesh {
    let mut positions = Vec::with_capacity(count * 4);
    let mut corners = Vec::with_capacity(count * 4);
    let mut seeds = Vec::with_capacity(count * 4);
    let mut normals = Vec::with_capacity(count * 4);
    let mut indices = Vec::with_capacity(count * 6);
    for card in 0..count {
        let phase = card as f32 / count as f32;
        // The golden ratio's spread, so neighbours in phase differ in luck.
        let luck = (card as f32 * 0.618_034) % 1.0;
        let first = positions.len() as u32;
        for corner in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            positions.push([0.0f32, 0.0, 0.0]);
            normals.push([0.0f32, 1.0, 0.0]);
            corners.push(corner);
            seeds.push([phase, luck]);
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

/// A column for each new emitter: its cards, its own material, and room enough round it
/// that it is not culled while any of it is in view.
fn draw(
    mut commands: Commands,
    quality: Res<Quality>,
    emitters: Query<(Entity, &Smoke), Added<Smoke>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SmokeMaterial>>,
    mut shared: Local<Option<(Quality, Handle<Mesh>)>>,
) {
    for (entity, smoke) in &emitters {
        let mesh = match &*shared {
            Some((made, mesh)) if *made == *quality => mesh.clone(),
            _ => {
                let mesh = meshes.add(column(cards(*quality)));
                *shared = Some((*quality, mesh.clone()));
                mesh
            }
        };
        let (shape, colour) = look(smoke.burning);
        let material = materials.add(SmokeMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                diffuse_transmission: 0.08,
                ..default()
            },
            extension: SmokeExtension {
                wind: Vec4::ZERO,
                shape,
                colour,
            },
        });
        // A column stands up to twenty metres high and leans as far.
        let reach = shape.x * shape.y + 8.0;
        commands.entity(entity).insert(Drawn {
            material: material.clone(),
            shown: 0.0,
            wind: Vec3::ZERO,
            // Not all looked up on the same frame.
            due: (entity.index_u32() % 32) as f32 / 32.0 * WIND_EVERY_S,
            told: Vec4::splat(-1.0),
        });
        commands.spawn((
            Name::new("Smoke"),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            Aabb::from_min_max(Vec3::new(-reach, -1.0, -reach), Vec3::splat(reach)),
            NotShadowCaster,
            ChildOf(entity),
        ));
    }
}

/// Each column thickening or thinning toward what its fire gives, and carried by the wind
/// where it stands, as the climate has it now.
fn blow(
    time: Res<Time>,
    clock: Res<Clock>,
    climate: Option<Res<ClimateFields>>,
    mut emitters: Query<(&Smoke, &mut Drawn, &GlobalTransform)>,
    mut materials: ResMut<Assets<SmokeMaterial>>,
) {
    let step = time.delta_secs();
    let follow = 1.0 - libm::expf(-step * FOLLOW * 4.0);
    for (smoke, mut drawn, at) in &mut emitters {
        // Snow lifts only in a wind hard enough to carry it.
        let lifted = if smoke.burning == Burning::Snow {
            let speed = drawn.wind.length();
            ((speed - SNOW_LIFTS_M_S.0) / (SNOW_LIFTS_M_S.1 - SNOW_LIFTS_M_S.0)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        drawn.shown += (smoke.thickness.clamp(0.0, 1.0) * lifted - drawn.shown) * follow;
        // The wind looked up again only now and then: it changes over minutes.
        drawn.due -= step;
        if drawn.due <= 0.0 {
            drawn.due = WIND_EVERY_S;
            let here = at.translation();
            let up = here.normalize_or(Vec3::Y);
            // The wind from the climate where it stands; a light air where there is none.
            let (speed, from) = match &climate {
                Some(climate) => {
                    let wind = climate.fields.at(climate.place(here), clock.time()).wind;
                    (wind.speed_m_s, wind.from)
                }
                None => (1.5, 0.8),
            };
            // North as the world's frame has it, where there is one.
            let north = climate.as_ref().map_or_else(
                || {
                    Vec3::Y
                        .reject_from(up)
                        .normalize_or(up.any_orthonormal_vector())
                },
                |climate| climate.frame.north(here),
            );
            let east = north.cross(up);
            let blowing_to = -(north * libm::cosf(from) + east * libm::sinf(from));
            // Smoke drifts slower than the wind at the eaves blows; snow goes with it.
            drawn.wind = match smoke.burning {
                Burning::Snow => blowing_to * speed.min(16.0),
                _ => blowing_to * speed.min(8.0) * 0.6,
            };
        }
        // The material touched only when what it is told has moved, so it is not sent to
        // the GPU again every frame.
        let told = drawn.wind.extend(drawn.shown);
        if told.distance(drawn.told) > 0.01
            && let Some(mut material) = materials.get_mut(&drawn.material)
        {
            material.extension.wind = told;
            drawn.told = told;
        }
    }
}
