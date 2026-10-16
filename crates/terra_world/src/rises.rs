//! Fish rising on a lake: a ring spreading on the water where a trout took a fly from its
//! surface, now and then one leaping clear and falling back. A still lake's surest sign of
//! life, and what shipped open worlds add to their water cheaply (The Witcher 3's and Red Dead
//! Redemption 2's rings on still water): a few at a time near the eye, a ring a normal laid
//! on the water's own waves in the lake's shader (`lake.wesl`, `rise_slopes`), so it takes the
//! water's reflection as a real ring does, and a leap a small fish drawn for half a second.
//!
//! Fish rise most at dawn and dusk, when the flies hatch, and little in the day's middle; on
//! deep enough water only, never on a pool in the heath (D379).

use crate::audio::{
    self, Bus, Mix, Speaker, Voice,
    synth::{Sound, Synth},
};
use crate::daylight::Clock;
use crate::planet::{MainCamera, Planet};
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use terra_gen::{Rng, Seed};

/// How many rings can be spreading at once, as the lake's shader holds them.
pub const RISES: usize = 8;
/// A ring is gone after so long, seconds.
pub const RING_S: f32 = 6.0;
/// Rises come at random, about this often near the eye at the best of the morning or
/// evening, seconds between, and so far from the eye, metres.
const EVERY_S: f32 = 1.6;
const WITHIN_M: (f32, f32) = (6.0, 48.0);
/// Water at least this deep, metres.
const DEEP_M: f32 = 0.6;
/// One rise in so many is a leap.
const LEAPS: f32 = 0.18;
/// A leap: how long, how high and how far, and the fish's length, metres.
const LEAP_S: f32 = 0.55;
const LEAP_HIGH_M: f32 = 0.38;
const LEAP_FAR_M: f32 = 0.7;
const FISH_M: f32 = 0.32;

/// The rings spreading now: where each is in the world, and when it began by the shader's
/// clock (`Time::elapsed_secs_wrapped`), none when it is 0.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Rises {
    pub rings: [Vec4; RISES],
    next: usize,
}

impl Default for Rises {
    fn default() -> Self {
        Self {
            rings: [Vec4::ZERO; RISES],
            next: 0,
        }
    }
}

impl Rises {
    /// A ring begins at a point now.
    pub fn ring(&mut self, at: Vec3, now: f32) {
        // Never 0, which is none.
        self.rings[self.next] = at.extend(now.max(1e-3));
        self.next = (self.next + 1) % RISES;
    }
}

/// How often fish rise at an hour, as a share of their most: at dawn and in the evening, when
/// flies hatch over the water, and little in the day's middle or the night.
pub fn rising_at(hour: f32) -> f32 {
    let bell = |middle: f32, width: f32| libm::expf(-((hour - middle) / width).powi(2));
    (bell(7.5, 2.8) + bell(19.5, 2.2))
        .min(1.0)
        .max(0.25 * bell(13.0, 5.0))
}

/// A fish in the air, so far through its leap.
#[derive(Component)]
struct Leap {
    from: Vec3,
    to: Vec3,
    up: Vec3,
    through: f32,
}

#[derive(Resource)]
struct Rising {
    dice: Rng,
    wait_s: f32,
    fish: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Rises>()
        .insert_resource(Rising {
            dice: Seed(41).derive("rises").rng(),
            wait_s: 3.0,
            fish: None,
        })
        .add_systems(Update, (rise, leap));
}

/// Now and then a fish rises somewhere on deep water near the eye, mostly where it looks.
fn rise(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<Clock>,
    planet: Option<Res<Planet>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut rises: ResMut<Rises>,
    mut rising: ResMut<Rising>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    (speaker, mix, synths): (Res<Speaker>, Res<Mix>, Option<ResMut<Assets<Synth>>>),
) {
    let (Some(planet), Some(eye)) = (planet, camera.iter().next()) else {
        return;
    };
    if planet.waters().is_none() {
        return;
    }
    rising.wait_s -= time.delta_secs();
    if rising.wait_s > 0.0 {
        return;
    }
    let rate = rising_at(clock.hour);
    let wait = rising.dice.range(0.3, 1.7) * EVERY_S / rate.max(0.05);
    rising.wait_s = wait;
    let here = eye.translation();
    let up = here.normalize_or(Vec3::Y);
    let forward = eye
        .forward()
        .as_vec3()
        .reject_from(up)
        .normalize_or(up.any_orthonormal_vector());
    // Mostly where it looks, now and then anywhere round it.
    let spread = if rising.dice.unit() < 0.75 {
        1.0
    } else {
        std::f32::consts::PI
    };
    let turn = Quat::from_axis_angle(up, rising.dice.range(-spread, spread));
    let at = here + turn * forward * rising.dice.range(WITHIN_M.0, WITHIN_M.1);
    let ground = planet.ground_at(at);
    let surface = planet.water_radius_m(at);
    if surface - ground.length() < DEEP_M {
        return;
    }
    let on_water = at.normalize() * surface;
    let now = time.elapsed_secs_wrapped();
    rises.ring(on_water, now);
    let leaps = rising.dice.unit() < LEAPS;
    if speaker.0
        && let Some(mut synths) = synths
    {
        // Heard where it is: a soft plop, a leap's splash louder.
        let (sound, level, reach_m) = if leaps {
            (Sound::Splash, 0.5, 8.0)
        } else {
            (Sound::Plop, 0.35, 5.0)
        };
        audio::play_synth(
            &mut commands,
            &mut synths,
            &mix,
            Synth {
                sound,
                seed: rising.dice.next_u64(),
            },
            Voice {
                bus: Bus::Ambience,
                level,
            },
            Some(on_water),
            reach_m,
        );
    }
    if leaps {
        let way = Quat::from_axis_angle(up, rising.dice.range(0.0, std::f32::consts::TAU))
            * up.any_orthonormal_vector();
        let to = on_water + way * LEAP_FAR_M;
        let (mesh, material) = rising
            .fish
            .get_or_insert_with(|| {
                (
                    meshes.add(fish(FISH_M)),
                    materials.add(StandardMaterial {
                        base_color: Color::WHITE,
                        perceptual_roughness: 0.35,
                        reflectance: 0.6,
                        metallic: 0.2,
                        ..default()
                    }),
                )
            })
            .clone();
        commands.spawn((
            Name::new("Leaping fish"),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(on_water),
            NotShadowCaster,
            Leap {
                from: on_water,
                to,
                up: on_water.normalize(),
                through: 0.0,
            },
        ));
    }
}

/// Each leaping fish along its arc, head first, and a second ring where it falls back.
fn leap(
    mut commands: Commands,
    time: Res<Time>,
    mut rises: ResMut<Rises>,
    mut fish: Query<(Entity, &mut Leap, &mut Transform)>,
) {
    for (entity, mut leap, mut at) in &mut fish {
        leap.through += time.delta_secs() / LEAP_S;
        if leap.through >= 1.0 {
            rises.ring(leap.to, time.elapsed_secs_wrapped());
            commands.entity(entity).despawn();
            continue;
        }
        let s = leap.through;
        let height = LEAP_HIGH_M * 4.0 * s * (1.0 - s);
        let point = leap.from.lerp(leap.to, s) + leap.up * height;
        // Its heading along the arc: rising, level at the top, diving.
        let rising = LEAP_HIGH_M * 4.0 * (1.0 - 2.0 * s);
        let way = (leap.to - leap.from) + leap.up * rising;
        *at = Transform::from_translation(point).looking_to(way, leap.up);
    }
}

/// A small fish, head along -Z: a spindle of a body, dark olive on its back and silver on
/// its flanks and belly, with a forked tail.
fn fish(length: f32) -> Mesh {
    const RINGS: usize = 9;
    const AROUND: usize = 8;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colours = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let back = [0.05, 0.07, 0.05, 1.0];
    let flank = [0.55, 0.57, 0.55, 1.0];
    let belly = [0.85, 0.85, 0.8, 1.0];
    for ring in 0..RINGS {
        let s = ring as f32 / (RINGS - 1) as f32;
        // Thickest a third of the way back from the head.
        let girth = libm::powf(libm::sinf(std::f32::consts::PI * libm::powf(s, 0.75)), 0.8);
        let z = -length * 0.5 + length * 0.85 * s;
        for k in 0..AROUND {
            let angle = k as f32 / AROUND as f32 * std::f32::consts::TAU;
            let (sin, cos) = (libm::sinf(angle), libm::cosf(angle));
            let (x, y) = (cos * 0.07 * length * girth, sin * 0.11 * length * girth);
            positions.push([x, y, z]);
            normals.push(Vec3::new(cos, sin * 0.7, 0.0).normalize().to_array());
            colours.push(if sin > 0.35 {
                back
            } else if sin > -0.4 {
                flank
            } else {
                belly
            });
        }
    }
    for ring in 0..RINGS - 1 {
        for k in 0..AROUND {
            let a = (ring * AROUND + k) as u32;
            let b = (ring * AROUND + (k + 1) % AROUND) as u32;
            let (c, d) = (a + AROUND as u32, b + AROUND as u32);
            indices.extend([a, c, b, b, c, d]);
        }
    }
    // The tail: two blades out from the wrist, seen from either side.
    let wrist = positions.len() as u32;
    let tail_z = length * 0.35;
    for p in [
        [0.0, 0.0, tail_z],
        [0.0, 0.1 * length, length * 0.5],
        [0.0, 0.0, length * 0.44],
        [0.0, -0.1 * length, length * 0.5],
    ] {
        positions.push(p);
        normals.push([1.0, 0.0, 0.0]);
        colours.push(back);
    }
    indices.extend([wrist, wrist + 1, wrist + 2, wrist, wrist + 2, wrist + 3]);
    indices.extend([wrist, wrist + 2, wrist + 1, wrist, wrist + 3, wrist + 2]);
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fish_rise_most_at_dawn_and_dusk_and_least_in_the_night() {
        assert!(rising_at(7.5) > 0.95 && rising_at(19.5) > 0.95);
        assert!(rising_at(13.0) < 0.3 && rising_at(13.0) > 0.05);
        assert!(rising_at(1.0) < 0.05);
    }

    #[test]
    fn rings_take_turns_and_none_is_ever_marked_unborn() {
        let mut rises = Rises::default();
        for i in 0..RISES + 2 {
            rises.ring(Vec3::X * i as f32, 0.0);
        }
        assert!(rises.rings.iter().all(|ring| ring.w > 0.0));
        // The oldest two were written over by the last.
        assert_eq!(rises.rings[0].x, RISES as f32);
        assert_eq!(rises.rings[1].x, (RISES + 1) as f32);
    }
}
