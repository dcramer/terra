//! What a heavy blow does to what grows where it lands: on the forest's soil a giant's step
//! kicks up the leaf litter, dead leaves thrown up and fluttering down (as Shadow of the
//! Colossus and God of War throw debris and leaves round a giant's foot), and any blow on
//! ground shoves the plants round it, bushes, ferns and grass shuddering and settling, as
//! games shake foliage round a blast with a radial force. The shove is a body that parts
//! plants (`terra_world::parting`, D271) whose reach swells and falls back, so every plant
//! shader that yields to a walker yields to it, with no shader of its own.

use super::{GRAVITY, LONGEST_S, Sprites};
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use terra_world::parting::PartsPlants;
use terra_world::planet::Planet;

/// Dead leaves of the litter, as an oak wood's floor is: browns, an ochre and an olive.
const LITTER: [Color; 4] = [
    Color::srgb(0.36, 0.22, 0.1),
    Color::srgb(0.5, 0.34, 0.13),
    Color::srgb(0.31, 0.27, 0.12),
    Color::srgb(0.23, 0.15, 0.08),
];
/// How much the air slows a leaf, a share of its speed a second.
const LEAF_DRAG: f32 = 2.0;
/// How long a blow's shove through the plants lasts, seconds; its swells come so often, and
/// die away so fast.
const SHOVE_S: f32 = 1.0;
const SHOVE_EVERY_S: f32 = 0.22;
const SHOVE_DIES_S: f32 = 0.35;

/// A dead leaf thrown up: its velocity, the axis it turns about and how fast, the way it
/// rocks as it falls and when, how fast the air lets it fall, how long it has been up and
/// lasts, its size, and whether it lies on the ground.
#[derive(Component)]
pub(super) struct Leaf {
    velocity: Vec3,
    axis: Vec3,
    spin: f32,
    sway: Vec3,
    phase: f32,
    falls: f32,
    age: f32,
    life: f32,
    scale: Vec3,
    lying: bool,
}

/// A blow's shove through the plants round it: how long it has gone on and how far it
/// reaches at its first swell.
#[derive(Component)]
pub(super) struct Shove {
    age: f32,
    reach: f32,
}

/// A dead leaf's outline, cut out: pointed, widest below its middle, its edge waved as an
/// oak's lobes are, on a short stalk, its midrib a shade darker. Its tip is at the top.
pub(super) fn leaf_image() -> Image {
    const WIDE: u32 = 32;
    const LONG: u32 = 64;
    let mut data = Vec::with_capacity((WIDE * LONG * 4) as usize);
    for y in 0..LONG {
        for x in 0..WIDE {
            let u = ((x as f32 + 0.5) / WIDE as f32 * 2.0 - 1.0).abs();
            let v = (y as f32 + 0.5) / LONG as f32;
            let blade = v / 0.86;
            let half = if blade < 1.0 {
                libm::powf(libm::sinf(blade * std::f32::consts::PI), 0.8)
                    * (0.86 + 0.14 * blade)
                    * (0.9 + 0.1 * libm::sinf(blade * 34.0))
            } else {
                0.0
            };
            let stalk = (0.86..0.98).contains(&v) && u < 0.08;
            let inside = u < half || stalk;
            let shade = if u < 0.06 { 190 } else { 255 };
            data.extend([shade, shade, shade, if inside { 255 } else { 0 }]);
        }
    }
    Image::new(
        Extent3d {
            width: WIDE,
            height: LONG,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Dead leaves of the litter kicked up from round where it struck, thrown up a metre or
/// three by a giant's step, turning over and over, then coming down slowly, rocking as they
/// go, to lie. Cut-out cards turning freely, lit through as leaves are.
pub(super) fn leaves(
    s: &mut Sprites,
    materials: &mut Assets<StandardMaterial>,
    leaf: Handle<Image>,
    at: Vec3,
    colour: Color,
) {
    let count = (24.0 * s.size).round() as u32;
    let shades: Vec<Handle<StandardMaterial>> = LITTER
        .iter()
        .map(|litter| {
            materials.add(StandardMaterial {
                base_color: litter.mix(&colour, 0.2),
                base_color_texture: Some(leaf.clone()),
                alpha_mode: AlphaMode::Mask(0.5),
                double_sided: true,
                cull_mode: None,
                perceptual_roughness: 0.8,
                diffuse_transmission: 0.35,
                ..default()
            })
        })
        .collect();
    let reach = s.size.sqrt();
    let big = libm::powf(s.size.max(1.0), 0.25);
    for i in 0..count {
        let out = s.round(s.r(i, 31));
        let velocity = out * (0.8 + 2.2 * s.r(i, 32)) * reach
            + s.up * (3.0 + 3.0 * s.r(i, 33)) * reach.min(2.0);
        let long = (0.07 + 0.05 * s.r(i, 34)) * big;
        let scale = Vec3::new(long * 0.6, long, 1.0);
        let axis =
            Vec3::new(s.r(i, 35) - 0.5, s.r(i, 36) - 0.5, s.r(i, 37) - 0.5).normalize_or(Vec3::X);
        let turns = if s.r(i, 39) < 0.5 { -1.0 } else { 1.0 };
        s.commands.spawn((
            Name::new("Impact leaf"),
            Mesh3d(s.quad.clone()),
            MeshMaterial3d(shades[i as usize % shades.len()].clone()),
            NotShadowCaster,
            Transform {
                translation: at + s.up * 0.05 + out * 0.3 * s.size * s.r(i, 40).max(0.3),
                rotation: Quat::from_axis_angle(axis, s.r(i, 38) * std::f32::consts::TAU),
                scale,
            },
            Leaf {
                velocity,
                axis,
                spin: (6.0 + 10.0 * s.r(i, 41)) * turns,
                sway: s.round(s.r(i, 42)) * (0.5 + 0.7 * s.r(i, 43)),
                phase: s.r(i, 44) * std::f32::consts::TAU,
                falls: 1.2 + 0.8 * s.r(i, 45),
                age: 0.0,
                life: LONGEST_S - 0.2 * s.r(i, 46),
                scale,
                lying: false,
            },
        ));
    }
}

/// The plants round where it struck shoved: a body that parts them, standing where it
/// struck, its reach swelling and falling back a few times, ever less.
pub(super) fn shove(s: &mut Sprites, at: Vec3) {
    s.commands.spawn((
        Name::new("Impact shove"),
        Transform::from_translation(at),
        Shove {
            age: 0.0,
            reach: (0.7 * s.size).clamp(0.5, 3.5),
        },
        PartsPlants {
            radius: 0.0,
            height: 2.5,
        },
    ));
}

/// How far a shove reaches so long after the blow: swelling out and back each
/// `SHOVE_EVERY_S`, each swell smaller, so what it reaches shudders and settles.
fn shove_reach(reach: f32, age: f32) -> f32 {
    if age >= SHOVE_S {
        return 0.0;
    }
    reach
        * libm::expf(-age / SHOVE_DIES_S)
        * libm::sinf(std::f32::consts::PI * age / SHOVE_EVERY_S).abs()
}

/// Each leaf turning over as it flies, slowed by the air, falling no faster than the air lets
/// it and rocking as it falls; lying where it lands, and shrinking away at its end.
pub(super) fn flutter(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<Planet>,
    mut leaves: Query<(Entity, &mut Leaf, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut leaf, mut transform) in &mut leaves {
        leaf.age += dt;
        if leaf.age >= leaf.life {
            commands.entity(entity).despawn();
            continue;
        }
        transform.scale = leaf.scale * ((leaf.life - leaf.age) / 0.4).min(1.0);
        if leaf.lying {
            continue;
        }
        let up = transform.translation.normalize_or(Vec3::Y);
        leaf.velocity = leaf.velocity * (1.0 - LEAF_DRAG * dt).max(0.0) - up * GRAVITY * dt;
        let down = -leaf.velocity.dot(up);
        if down > leaf.falls {
            let falls = leaf.falls;
            leaf.velocity += up * (down - falls);
        }
        let rocking = if down > 0.0 {
            leaf.sway * libm::sinf(leaf.phase + leaf.age * 7.0)
        } else {
            Vec3::ZERO
        };
        transform.translation += (leaf.velocity + rocking) * dt;
        let turn = Quat::from_axis_angle(leaf.axis, leaf.spin * dt);
        transform.rotation = turn * transform.rotation;
        if down > 0.0 && planet.distance(transform.translation) < 0.02 {
            leaf.lying = true;
            transform.translation = planet.ground_at(transform.translation) + up * 0.02;
            transform.rotation =
                Quat::from_rotation_arc(Vec3::Z, up) * Quat::from_rotation_z(leaf.phase);
        }
    }
}

/// Each shove's reach through the plants this frame; gone at its end.
pub(super) fn shudder(
    mut commands: Commands,
    time: Res<Time>,
    mut shoves: Query<(Entity, &mut Shove, &mut PartsPlants)>,
) {
    for (entity, mut shove, mut parts) in &mut shoves {
        shove.age += time.delta_secs();
        if shove.age >= SHOVE_S {
            commands.entity(entity).despawn();
            continue;
        }
        parts.radius = shove_reach(shove.reach, shove.age);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{count, world};
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn a_giants_step_throws_leaves_up_that_come_down_slowly_and_are_gone_within_three_seconds() {
        let (mut world, ground) = world();
        world
            .run_system_once(
                move |mut commands: Commands,
                      mut meshes: ResMut<Assets<Mesh>>,
                      mut materials: ResMut<Assets<StandardMaterial>>,
                      mut images: ResMut<Assets<Image>>| {
                    let mut sprites = Sprites {
                        commands: &mut commands,
                        quad: meshes.add(Rectangle::new(1.0, 1.0)),
                        seed: 7,
                        up: ground.normalize(),
                        size: 4.5,
                    };
                    let leaf = images.add(leaf_image());
                    leaves(&mut sprites, &mut materials, leaf, ground, LITTER[0]);
                },
            )
            .unwrap();
        let thrown = count::<Leaf>(&mut world);
        assert!(thrown > 50, "{thrown} leaves");
        let planet = world.resource::<Planet>().clone();
        let mut was: std::collections::HashMap<Entity, f32> = std::collections::HashMap::new();
        let (mut highest, mut fastest_down): (f32, f32) = (0.0, 0.0);
        for _ in 0..60 * 3 {
            world.run_system_once(flutter).unwrap();
            let mut flying = world.query::<(Entity, &Leaf, &Transform)>();
            for (entity, leaf, at) in flying.iter(&world) {
                let height = planet.distance(at.translation);
                highest = highest.max(height);
                if let Some(before) = was.insert(entity, height)
                    && !leaf.lying
                {
                    fastest_down = fastest_down.max((before - height) * 60.0);
                }
            }
        }
        // Thrown up over a metre, falling no faster than a dead leaf does, not as a stone.
        assert!(highest > 1.0, "the leaves rose {highest} m");
        assert!(fastest_down < 2.2, "a leaf fell at {fastest_down} m/s");
        assert_eq!(count::<Leaf>(&mut world), 0);
    }

    #[test]
    fn a_shove_swells_a_few_times_ever_less_and_ends() {
        let reach = 3.0;
        let steps: Vec<f32> = (0..=120)
            .map(|i| shove_reach(reach, i as f32 * SHOVE_S / 100.0))
            .collect();
        let swells: Vec<f32> = steps
            .windows(3)
            .filter(|w| w[1] > w[0] && w[1] >= w[2])
            .map(|w| w[1])
            .collect();
        assert!(swells.len() >= 3, "{swells:?}");
        assert!(swells[0] > 0.6 * reach, "{swells:?}");
        assert!(swells.windows(2).all(|w| w[1] < w[0]), "{swells:?}");
        assert_eq!(shove_reach(reach, SHOVE_S), 0.0);
    }
}
