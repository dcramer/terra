//! What a heavy blow raises where it lands, as action games throw it up for a big enemy's
//! slam (God of War's troll, Elden Ring's giants): on ground a burst of dust rolling out low
//! and a spray of small stones thrown up and falling, in the ground's own colour; over water a
//! splash, a crown of droplets thrown up and falling back, a white spray puff and rings
//! spreading on the water (`terra_world::rises`). A few dozen soft sprites facing the view,
//! lit by the sun and the sky as anything else is, drawn as the build's dust is
//! (`terra_building::loss`), and gone in under three seconds.

use super::placed;
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use serde::Deserialize;
use serde_json::{Value, json};
use terra_core::Ground as _;
use terra_gen::hash::prospector32_round;
use terra_voxel::material;
use terra_world::planet::{GroundTextures, MainCamera, Planet};
use terra_world::remote::method;

/// Water at least this deep over the ground splashes, metres.
const SPLASHES_M: f32 = 0.04;
const GRAVITY: f32 = 9.81;
/// The rows of the ground's textures (assets/materials/README.md, `ground`), and which row
/// each of the ground's materials is drawn with: soil's mud, rock, sand, scree, gravel.
const GROUND_LAYERS: usize = 12;
const LAYER_OF: [(terra_core::MaterialId, usize); 6] = [
    (material::SOIL, 0),
    (material::STONE, 2),
    (material::ORE, 2),
    (material::SAND, 3),
    (material::SCREE, 8),
    (material::GRAVEL, 9),
];
/// How many puffs a burst's dust is: one in the middle, the rest a ring rolling out.
const PUFFS: u32 = 9;
/// The colour of ground whose textures are not in, as dry gravel is.
const DRY_GROUND: Color = Color::srgb(0.46, 0.43, 0.39);
/// Spray, lit white as foam and spray are.
const SPRAY: Color = Color::srgb(0.86, 0.89, 0.9);
/// How long the longest of it lasts, seconds: nothing outlives it.
pub const LONGEST_S: f32 = 2.6;

/// A blow landing at a point, as big as `size` says (1 a troll's two-handed smash).
#[derive(Message, Clone, Copy, Debug)]
pub struct Impact {
    pub at: Vec3,
    pub size: f32,
}

/// Where a blow lands: on ground, at its point, or on water, at its surface over the point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lands {
    Ground(Vec3),
    Water(Vec3),
}

/// Where a blow at a point on the ground lands, given the water's surface over it (its
/// distance from the planet's middle): on water deep enough to splash, else on the ground.
pub fn lands(ground: Vec3, water_radius_m: f32) -> Lands {
    if water_radius_m > ground.length() + SPLASHES_M {
        Lands::Water(ground.normalize() * water_radius_m)
    } else {
        Lands::Ground(ground)
    }
}

/// What every burst is drawn with: a quad, a soft round, a pebble, and the ground's colours
/// by row once worked out; rings still to come on the water, and when.
#[derive(Resource, Default)]
struct Spray {
    quad: Option<Handle<Mesh>>,
    soft: Option<Handle<Image>>,
    pebble: Option<Handle<Mesh>>,
    layers: Option<Vec<Color>>,
    rings: Vec<(Vec3, f32)>,
}

/// A sprite flying: how long it has been up and lasts, its velocity, how hard it falls and
/// how much the air slows it, how wide it is at first and at last, and the water's surface
/// it goes into, if it is a drop.
#[derive(Component)]
struct Mote {
    age: f32,
    life: f32,
    velocity: Vec3,
    gravity: f32,
    drag: f32,
    wide: [f32; 2],
    into_water: Option<f32>,
}

/// A material shared by a group of sprites, faded over their life: thick at once and thinning
/// away as a puff does, or holding and going late as drops do.
#[derive(Component)]
struct Fading {
    material: Handle<StandardMaterial>,
    alpha: f32,
    age: f32,
    life: f32,
    late: bool,
}

/// A stone thrown up: its velocity and spin, how long it has been up and lasts, and whether it
/// lies still.
#[derive(Component)]
struct Stone {
    velocity: Vec3,
    spin: Vec3,
    age: f32,
    life: f32,
    size: f32,
    lying: bool,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Spray>()
        .add_message::<Impact>()
        .add_systems(Update, (burst, rings, fly, fade, tumble));
    method(app, "staged.impact", impact);
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Asked {
    at: [f32; 3],
    size: Option<f32>,
    #[serde(default)]
    base: bool,
}

/// A heavy blow lands at a point: dust and stones on ground, a splash on water.
fn impact(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Asked = parse_some(params)?;
    let at = placed(world, asked.at, asked.base)?;
    let planet = world.resource::<Planet>();
    let ground = planet.ground_at(at);
    let on = lands(ground, planet.water_radius_m(ground));
    world.write_message(Impact {
        at,
        size: asked.size.unwrap_or(1.0).clamp(0.1, 3.0),
    });
    Ok(
        json!({ "done": true, "on": if matches!(on, Lands::Water(_)) { "water" } else { "ground" } }),
    )
}

/// A number from 0 to 1 for a burst, a sprite and a use.
fn random(seed: u32, i: u32, salt: u32) -> f32 {
    let h = prospector32_round(seed ^ i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA77));
    ((h >> 8) & 0xFFFF) as f32 / 65535.0
}

/// A soft round, its alpha falling off from its middle, a little uneven, as the build's dust
/// is drawn with.
pub(crate) fn soft_image() -> Image {
    const SIDE: u32 = 64;
    let mut data = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (u, v) = (
                (x as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0,
                (y as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0,
            );
            let r = (u * u + v * v).sqrt();
            let lump = 0.8 + 0.2 * libm::sinf(u * 11.0 + libm::cosf(v * 8.0) * 2.5);
            // Steeply soft, so a puff's edge, and where the ground cuts it, never shows as a
            // line (the Cetri's round-9 reviews: hard-edged quads at 1.6).
            let alpha = libm::powf((1.0 - r).max(0.0), 2.6) * lump;
            data.extend([255, 255, 255, (alpha.clamp(0.0, 1.0) * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// The mean colour of each row of the ground's textures, from a sparse grid of its pixels:
/// what its dust and stones are coloured by. Each row's own picture comes first in its share
/// of the data, mipmapped or not.
fn layer_colours(image: &Image) -> Option<Vec<Color>> {
    let data = image.data.as_ref()?;
    if !matches!(
        image.texture_descriptor.format,
        TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm
    ) {
        return None;
    }
    let size = image.texture_descriptor.size;
    let layers = size.depth_or_array_layers as usize;
    if layers < GROUND_LAYERS || data.is_empty() {
        return None;
    }
    let (w, h) = (size.width as usize, size.height as usize);
    let stride = data.len() / layers;
    if stride < w * h * 4 {
        return None;
    }
    let srgb = image.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb;
    Some(
        (0..layers)
            .map(|layer| {
                let mut sum = Vec3::ZERO;
                let mut count = 0.0;
                for gy in 0..24 {
                    for gx in 0..24 {
                        let (x, y) = ((gx * w + w / 2) / 24, (gy * h + h / 2) / 24);
                        let i = layer * stride + (y * w + x) * 4;
                        let px = Vec3::new(data[i] as f32, data[i + 1] as f32, data[i + 2] as f32)
                            / 255.0;
                        sum += px;
                        count += 1.0;
                    }
                }
                let mean = sum / count;
                if srgb {
                    Color::srgb(mean.x, mean.y, mean.z)
                } else {
                    Color::linear_rgb(mean.x, mean.y, mean.z)
                }
            })
            .collect(),
    )
}

/// The colour of the ground at a point on it: its material's texture's mean.
fn ground_colour(
    planet: &Planet,
    spray: &mut Spray,
    textures: Option<&GroundTextures>,
    images: &Assets<Image>,
    at: Vec3,
) -> Color {
    if spray.layers.is_none() {
        spray.layers = textures
            .and_then(|textures| images.get(textures.color()))
            .and_then(layer_colours);
    }
    let made_of = planet.voxels().material(at - at.normalize() * 0.15);
    let row = LAYER_OF
        .iter()
        .find(|(id, _)| *id == made_of)
        .map_or(9, |(_, row)| *row);
    spray
        .layers
        .as_ref()
        .and_then(|layers| layers.get(row).copied())
        .unwrap_or(DRY_GROUND)
}

/// A lit soft material, of a colour and thickness, for a group of sprites, faded by its own
/// entity over their life.
fn soft_material(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    soft: &Handle<Image>,
    colour: Color,
    alpha: f32,
    life: f32,
    late: bool,
) -> Handle<StandardMaterial> {
    let material = materials.add(StandardMaterial {
        base_color: colour.with_alpha(if late { alpha } else { 0.0 }),
        base_color_texture: Some(soft.clone()),
        perceptual_roughness: 1.0,
        reflectance: 0.2,
        // Lit from behind as well, as a cloud of dust or spray is lit through.
        diffuse_transmission: 0.6,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    commands.spawn((
        Name::new("Impact fading"),
        Fading {
            material: material.clone(),
            alpha,
            age: 0.0,
            life,
            late,
        },
    ));
    material
}

/// Each blow's burst, raised where it lands.
#[allow(clippy::too_many_arguments)]
fn burst(
    mut commands: Commands,
    mut impacts: MessageReader<Impact>,
    planet: Res<Planet>,
    textures: Option<Res<GroundTextures>>,
    mut spray: ResMut<Spray>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    for (n, impact) in impacts.read().enumerate() {
        let quad = spray
            .quad
            .get_or_insert_with(|| meshes.add(Rectangle::new(1.0, 1.0)))
            .clone();
        let soft = spray
            .soft
            .get_or_insert_with(|| images.add(soft_image()))
            .clone();
        let ground = planet.ground_at(impact.at);
        let seed = impact.at.x.to_bits()
            ^ (impact.at.z.to_bits() >> 3)
            ^ (n as u32).wrapping_mul(0x27D4_EB2D);
        let mut sprites = Sprites {
            commands: &mut commands,
            quad,
            seed,
            up: ground.normalize(),
            size: impact.size,
        };
        match lands(ground, planet.water_radius_m(ground)) {
            Lands::Water(surface) => {
                splash(&mut sprites, &mut materials, &soft, surface);
                spray.rings.push((surface, 0.0));
                spray.rings.push((surface, 0.3));
            }
            Lands::Ground(at) => {
                let colour = ground_colour(&planet, &mut spray, textures.as_deref(), &images, at);
                dust(&mut sprites, &mut materials, &soft, at, colour);
                let pebble = spray
                    .pebble
                    .get_or_insert_with(|| meshes.add(Sphere::new(0.5).mesh().ico(1).unwrap()))
                    .clone();
                stones(&mut sprites, &mut materials, pebble, at, colour);
            }
        }
    }
}

/// What a burst's sprites are made with.
struct Sprites<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    quad: Handle<Mesh>,
    seed: u32,
    up: Vec3,
    size: f32,
}

impl Sprites<'_, '_, '_> {
    fn r(&self, i: u32, salt: u32) -> f32 {
        random(self.seed, i, salt)
    }

    /// A level way round the up, so many turns round.
    fn round(&self, turns: f32) -> Vec3 {
        let east = self.up.any_orthonormal_vector();
        let north = self.up.cross(east);
        let a = turns * std::f32::consts::TAU;
        east * libm::cosf(a) + north * libm::sinf(a)
    }

    fn mote(&mut self, material: &Handle<StandardMaterial>, at: Vec3, mote: Mote) {
        self.commands.spawn((
            Name::new("Impact mote"),
            Mesh3d(self.quad.clone()),
            MeshMaterial3d(material.clone()),
            NotShadowCaster,
            Transform::from_translation(at).with_scale(Vec3::splat(mote.wide[0])),
            mote,
        ));
    }
}

/// Dust: a thick puff where it struck and low ones rolling out from it along the ground,
/// rising a little and thinning away.
fn dust(
    s: &mut Sprites,
    materials: &mut Assets<StandardMaterial>,
    soft: &Handle<Image>,
    at: Vec3,
    colour: Color,
) {
    // Dust is greyer than the stones it comes off: the fine stuff, dry. The material under the
    // surface can be the soil under a gravel shore, whose mud read as a brown tint over the
    // troll, so it is half a dry earth grey; a dark one, as a pale grey lit through by a low
    // sun read as white paper or snow over the forest's litter (the Cetri's round-7 reviews),
    // and half that, lit through, as beige or tan quads (round 9): a dark, dry earth. Thin, it
    // read as nothing at a giant's footfall (the Cetri's round-12 reviews): denser and darker,
    // nine puffs low along the ground.
    let colour = colour.mix(&Color::srgb(0.18, 0.15, 0.12), 0.72);
    let size = s.size;
    for puff in 0..PUFFS {
        let middle = puff == 0;
        let life = if middle {
            2.4
        } else {
            1.8 + 0.5 * s.r(puff, 1)
        };
        let material = soft_material(
            s.commands,
            materials,
            soft,
            colour,
            if middle { 0.56 } else { 0.46 },
            life,
            false,
        );
        let out = s.round(puff as f32 / (PUFFS - 1) as f32 + 0.1 * s.r(puff, 2));
        // A ring thrown out low along the ground, as a heavy blow's dust rolls out from it,
        // not a cloud standing over it (which hung in front of the troll as a tint).
        for sprite in 0..3u32 {
            let i = puff * 3 + sprite;
            let speed = if middle { 0.4 } else { 1.8 + 1.4 * s.r(i, 3) } * size;
            let velocity =
                out * speed + s.up * (0.06 + 0.16 * s.r(i, 4)) * size + s.round(s.r(i, 5)) * 0.3;
            let wide = if middle { 1.3 } else { 0.8 + 0.5 * s.r(i, 6) } * size;
            s.mote(
                &material,
                at + s.up * 0.12 * size + out * 0.3 * size,
                Mote {
                    age: 0.0,
                    life,
                    velocity,
                    gravity: 0.0,
                    drag: 1.4,
                    wide: [wide * 0.25, wide],
                    into_water: None,
                },
            );
        }
    }
}

/// Small stones thrown up and out, falling under gravity, bouncing once and lying.
fn stones(
    s: &mut Sprites,
    materials: &mut Assets<StandardMaterial>,
    pebble: Handle<Mesh>,
    at: Vec3,
    colour: Color,
) {
    let count = (16.0 * s.size).round() as u32;
    let shades: Vec<Handle<StandardMaterial>> = (0..3)
        .map(|k| {
            materials.add(StandardMaterial {
                base_color: colour.darker(0.05 + 0.08 * k as f32),
                perceptual_roughness: 0.85,
                ..default()
            })
        })
        .collect();
    for i in 0..count {
        let out = s.round(s.r(i, 11));
        let velocity = out * (1.2 + 3.0 * s.r(i, 12)) * s.size.sqrt()
            + s.up * (2.5 + 3.5 * s.r(i, 13)) * s.size.sqrt();
        let size = 0.03 + 0.06 * s.r(i, 14) * s.r(i, 15).max(0.3);
        let shape = Vec3::new(1.0, 0.6 + 0.4 * s.r(i, 16), 0.7 + 0.3 * s.r(i, 17)) * size;
        s.commands.spawn((
            Name::new("Impact stone"),
            Mesh3d(pebble.clone()),
            MeshMaterial3d(shades[(i % 3) as usize].clone()),
            NotShadowCaster,
            Transform::from_translation(at + s.up * 0.1 + out * 0.2).with_scale(shape),
            Stone {
                velocity,
                spin: Vec3::new(s.r(i, 18) - 0.5, s.r(i, 19) - 0.5, s.r(i, 20) - 0.5) * 30.0,
                age: 0.0,
                life: LONGEST_S - 0.3 * s.r(i, 21),
                size,
                lying: false,
            },
        ));
    }
}

/// A splash: a crown of drops thrown up and out, a few straight up from its middle, falling
/// back into the water; a white puff of spray over it.
fn splash(
    s: &mut Sprites,
    materials: &mut Assets<StandardMaterial>,
    soft: &Handle<Image>,
    surface: Vec3,
) {
    let size = s.size;
    let water = surface.length();
    let drops = soft_material(s.commands, materials, soft, SPRAY, 0.9, 1.6, true);
    // A crown: a sheet of many small drops thrown up and out from round where the blow went
    // in, falling back in an arc, the shutter drawing each into a streak; and a short jet from
    // the middle. Fewer, bigger drops and a tall column read as balls of light over a white
    // pillar.
    let crown = (70.0 * size).round() as u32;
    for i in 0..crown + 10 {
        let column = i >= crown;
        let out = s.round(i as f32 / crown as f32 + 0.05 * s.r(i, 1));
        let velocity = if column {
            s.round(s.r(i, 2)) * 0.5 + s.up * (3.2 + 1.6 * s.r(i, 3))
        } else {
            out * (1.4 + 2.0 * s.r(i, 4)) + s.up * (2.4 + 2.2 * s.r(i, 5))
        } * size.sqrt();
        let wide = (0.03 + 0.04 * s.r(i, 6)) * size.sqrt();
        s.mote(
            &drops,
            surface + out * 0.35 * size + s.up * 0.05,
            Mote {
                age: 0.0,
                life: 1.6,
                velocity,
                gravity: GRAVITY,
                drag: 0.3,
                wide: [wide, wide * 0.7],
                into_water: Some(water),
            },
        );
    }
    // A thin mist low over it, drifting out: spray, not a white cloud standing over the water.
    for puff in 0..3u32 {
        let life = 1.2 + 0.4 * s.r(puff, 7);
        let material = soft_material(s.commands, materials, soft, SPRAY, 0.3, life, false);
        for sprite in 0..2u32 {
            let i = puff * 2 + sprite;
            let out = s.round(s.r(i, 8));
            s.mote(
                &material,
                surface + s.up * (0.15 + 0.2 * puff as f32) * size,
                Mote {
                    age: 0.0,
                    life,
                    velocity: out * 0.8 * size + s.up * (0.3 + 0.3 * s.r(i, 9)) * size,
                    gravity: 0.0,
                    drag: 1.2,
                    wide: [0.3 * size, (0.8 + 0.4 * s.r(i, 10)) * size],
                    into_water: None,
                },
            );
        }
    }
}

/// The splashes' rings begun on the water, each when its time comes.
fn rings(
    time: Res<Time>,
    mut spray: ResMut<Spray>,
    rises: Option<ResMut<terra_world::rises::Rises>>,
) {
    if spray.rings.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let now = time.elapsed_secs_wrapped();
    let mut rises = rises;
    spray.rings.retain_mut(|(at, wait)| {
        *wait -= dt;
        if *wait > 0.0 {
            return true;
        }
        if let Some(rises) = rises.as_mut() {
            rises.ring(*at, now);
        }
        false
    });
}

/// Each sprite on its way, growing or shrinking, facing the view; gone at its end, or a drop
/// as it goes back into the water.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut motes: Query<(Entity, &mut Mote, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let eye = camera.map(|camera| camera.translation());
    for (entity, mut mote, mut transform) in &mut motes {
        mote.age += dt;
        let through = mote.age / mote.life;
        let up = transform.translation.normalize_or(Vec3::Y);
        mote.velocity = mote.velocity * (1.0 - mote.drag * dt).max(0.0) - up * mote.gravity * dt;
        transform.translation += mote.velocity * dt;
        let sunk = mote.into_water.is_some_and(|water| {
            transform.translation.length() < water && mote.velocity.dot(up) < 0.0
        });
        if through >= 1.0 || sunk {
            commands.entity(entity).despawn();
            continue;
        }
        let grow = 1.0 - (1.0 - through) * (1.0 - through);
        transform.scale = Vec3::splat(mote.wide[0] + (mote.wide[1] - mote.wide[0]) * grow);
        if let Some(eye) = eye {
            let at = transform.translation;
            transform.look_at(at * 2.0 - eye, up);
        }
    }
}

/// Each group's material faded over its life, and let go at its end.
fn fade(
    mut commands: Commands,
    time: Res<Time>,
    mut fading: Query<(Entity, &mut Fading)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut group) in &mut fading {
        group.age += time.delta_secs();
        let through = group.age / group.life;
        if through >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let alpha = if group.late {
            group.alpha * (1.0 - ((through - 0.6) / 0.4).clamp(0.0, 1.0))
        } else {
            // Thick at once, then thinning away.
            group.alpha * (through / 0.08).min(1.0) * (1.0 - through) * (1.0 - through)
        };
        if let Some(mut material) = materials.get_mut(&group.material) {
            material.base_color = material.base_color.with_alpha(alpha);
        }
    }
}

/// Each stone falling, bouncing once off the ground and lying, shrinking away at its end.
fn tumble(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<Planet>,
    mut stones: Query<(Entity, &mut Stone, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut stone, mut transform) in &mut stones {
        stone.age += dt;
        if stone.age >= stone.life {
            commands.entity(entity).despawn();
            continue;
        }
        let left = stone.life - stone.age;
        if left < 0.4 {
            transform.scale *= 1.0 - (dt / left).min(1.0);
        }
        if stone.lying {
            continue;
        }
        let up = transform.translation.normalize_or(Vec3::Y);
        stone.velocity -= up * GRAVITY * dt;
        transform.translation += stone.velocity * dt;
        let spin = stone.spin * dt;
        transform.rotate(Quat::from_scaled_axis(spin));
        if planet.distance(transform.translation) < stone.size * 0.4 && stone.velocity.dot(up) < 0.0
        {
            let down = stone.velocity.dot(up);
            let along = stone.velocity - up * down;
            if -down < 1.5 {
                stone.lying = true;
                let ground = planet.ground_at(transform.translation);
                transform.translation = ground + up * stone.size * 0.3;
            } else {
                stone.velocity = along * 0.45 - up * down * 0.3;
                stone.spin *= 0.5;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use terra_core::{PlanetSpec, Seed};

    #[test]
    fn a_blow_splashes_on_water_and_raises_dust_on_ground() {
        let ground = Vec3::new(0.0, 1000.0, 0.0);
        // Water half a metre over the ground: a splash at its surface.
        assert_eq!(
            lands(ground, 1000.5),
            Lands::Water(Vec3::new(0.0, 1000.5, 0.0))
        );
        // The water's surface under the ground, as it is where there is none: dust.
        assert_eq!(lands(ground, 990.0), Lands::Ground(ground));
        // A wet shore's film of water is not a splash.
        assert_eq!(lands(ground, 1000.01), Lands::Ground(ground));
    }

    /// A world on a smooth planet with what a burst needs, the clock stepping a sixtieth.
    fn world() -> (World, Vec3) {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(2),
            radius_m: 2000.0,
        });
        let ground = planet.ground_at(Vec3::Y * 2100.0);
        let mut world = World::new();
        world.insert_resource(planet);
        world.init_resource::<Spray>();
        world.init_resource::<Messages<Impact>>();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Assets<Image>>();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
        world.insert_resource(time);
        (world, ground)
    }

    fn count<C: Component>(world: &mut World) -> usize {
        world.query_filtered::<(), With<C>>().iter(world).count()
    }

    #[test]
    fn a_burst_on_ground_throws_stones_that_land_and_is_gone_within_three_seconds() {
        let (mut world, ground) = world();
        world.write_message(Impact {
            at: ground,
            size: 1.0,
        });
        world.run_system_once(burst).unwrap();
        let (motes, stones) = (count::<Mote>(&mut world), count::<Stone>(&mut world));
        // A few dozen sprites, not hundreds.
        assert!((10..=60).contains(&motes), "{motes} sprites");
        assert!(stones >= 8, "{stones} stones");
        let mut highest: f32 = 0.0;
        for frame in 0..60 * 3 {
            world.run_system_once(fly).unwrap();
            world.run_system_once(fade).unwrap();
            world.run_system_once(tumble).unwrap();
            if frame == 110 {
                // By nearly two seconds every stone is down on the ground, none under it.
                let planet = world.resource::<Planet>().clone();
                let mut lying = world.query::<(&Stone, &Transform)>();
                for (stone, at) in lying.iter(&world) {
                    assert!(stone.lying, "a stone still in the air");
                    assert!(planet.distance(at.translation).abs() < 0.1);
                }
            }
            let planet = world.resource::<Planet>().clone();
            let mut flying = world.query::<&Transform>();
            for at in flying.iter(&world) {
                highest = highest.max(planet.distance(at.translation));
            }
        }
        assert!(highest > 0.5, "nothing was thrown up");
        assert_eq!(
            count::<Mote>(&mut world) + count::<Stone>(&mut world) + count::<Fading>(&mut world),
            0
        );
    }
}
