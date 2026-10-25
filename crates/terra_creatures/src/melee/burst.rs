//! What a blow that lands shows where its blade met the body (D487, in place of a flash of the
//! whole struck body): a white-hot glow at the contact for a few frames, lighting what is
//! around it, and a spray of sparks flung on the way the blade was going, falling as they
//! fade. Enshrouded's landed hit is this (its "flash" is at the contact, the struck body keeps
//! its colour; research/2026-10-25-what-a-landed-hit-shows.md), as are Monster Hunter's and
//! Souls' hit sparks. Spawned at the hit's place and direction, as Unreal spawns a Niagara
//! impact system (`SpawnSystemAtLocation`). Drawn unlit and added to what is behind, so it
//! reads alike by day and by night; soft sprites facing the view, as the staged impact's
//! (`staged::impact`). The set's `landed.burst` says how big and how long. Presentation only:
//! nothing the fight's rules read.

use crate::staged::impact::soft_image;
use bevy::{light::NotShadowCaster, prelude::*};
use terra_gen::hash;
use terra_motion::moves::Burst as Shape;
use terra_world::planet::MainCamera;

/// What the glow at the contact gives off, unlit: a white-hot core a share of its width
/// across, in a warm halo.
const CORE: LinearRgba = LinearRgba::rgb(8.0, 7.6, 6.8);
const CORE_WIDE: f32 = 0.4;
const HALO: LinearRgba = LinearRgba::rgb(4.0, 2.6, 1.2);
/// How far the burst is drawn toward the view from the contact, metres, so the body it is
/// on does not hide half of it (Niagara's Camera Offset).
const TOWARD_VIEW_M: f32 = 0.3;
/// A spark's colour: orange-yellow, as hot steel's.
const SPARK: LinearRgba = LinearRgba::rgb(3.2, 1.9, 0.7);
/// The light the glow sheds on what is near, lumens at its brightest, and how far it reaches.
const LIGHT_LM: f32 = 120_000.0;
const LIGHT_M: f32 = 3.0;
/// How far a spark's way strays from the blade's, as a share of it.
const SPREAD: f32 = 0.9;
/// How long a spark streak is for each metre a second it flies (a motion streak), and how
/// thick it starts, metres.
const STREAK_S: f32 = 0.04;
const SPARK_THICK_M: f32 = 0.02;
const GRAVITY: f32 = 9.81;

/// A landed blow's contact: where the blade met the body, and the way it was going.
#[derive(Message, Clone, Copy, Debug)]
pub struct Met {
    pub at: Vec3,
    pub along: Vec3,
}

/// The glow at a contact: how long it has shown, for how long, and how wide.
#[derive(Component)]
pub struct Glow {
    age: f32,
    life: f32,
    wide: f32,
}

/// A spark on its way.
#[derive(Component)]
pub struct Spark {
    age: f32,
    life: f32,
    velocity: Vec3,
}

/// The burst's one quad and its materials, made the first time one shows.
struct Looks {
    quad: Handle<Mesh>,
    core: Handle<StandardMaterial>,
    halo: Handle<StandardMaterial>,
    spark: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Met>()
        .add_systems(Update, (spawn, (glow, spark)).chain());
}

/// An unlit material added to what is behind, its alpha the soft round's.
fn additive(colour: LinearRgba, image: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color: colour.into(),
        base_color_texture: Some(image),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    }
}

/// Each contact bursts: its glow and light at the contact, and its sparks flung from it.
fn spawn(
    mut met: MessageReader<Met>,
    mut commands: Commands,
    set: Res<super::Set>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut looks: Local<Option<Looks>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let shape: Shape = set.0.landed.burst;
    let eye = camera.map(|camera| camera.translation());
    for &Met { at: met_at, along } in met.read() {
        let looks = looks.get_or_insert_with(|| {
            let image = images.add(soft_image());
            Looks {
                quad: meshes.add(Rectangle::new(1.0, 1.0)),
                core: materials.add(additive(CORE, image.clone())),
                halo: materials.add(additive(HALO, image.clone())),
                spark: materials.add(additive(SPARK, image)),
            }
        });
        let at = eye.map_or(met_at, |eye| {
            met_at + (eye - met_at).normalize_or_zero() * TOWARD_VIEW_M
        });
        if shape.glow_m > 0.0 && shape.glow_s > 0.0 {
            let glow = |wide: f32| Glow {
                age: 0.0,
                life: shape.glow_s,
                wide,
            };
            commands.spawn((
                Mesh3d(looks.quad.clone()),
                MeshMaterial3d(looks.halo.clone()),
                Transform::from_translation(at).with_scale(Vec3::splat(shape.glow_m)),
                NotShadowCaster,
                glow(shape.glow_m),
                PointLight {
                    intensity: LIGHT_LM,
                    color: Color::srgb(1.0, 0.85, 0.6),
                    range: LIGHT_M,
                    ..default()
                },
            ));
            let core = shape.glow_m * CORE_WIDE;
            commands.spawn((
                Mesh3d(looks.quad.clone()),
                MeshMaterial3d(looks.core.clone()),
                Transform::from_translation(at).with_scale(Vec3::splat(core)),
                NotShadowCaster,
                glow(core),
            ));
        }
        let way = along.normalize_or(Vec3::Y);
        let seed = hash::of(&[at.x.to_bits(), at.y.to_bits(), at.z.to_bits()]);
        for i in 0..shape.sparks {
            let r = |salt: u32| hash::unit(hash::of(&[seed, i, salt]));
            // A way within the sphere's surface, picked evenly (z and turn), to stray by.
            let (z, turn) = (r(0) * 2.0 - 1.0, r(1) * core::f32::consts::TAU);
            let flat = (1.0 - z * z).max(0.0).sqrt();
            let stray = Vec3::new(flat * libm::cosf(turn), flat * libm::sinf(turn), z);
            let speed = shape.spark_mps * (0.45 + 0.55 * r(2));
            commands.spawn((
                Mesh3d(looks.quad.clone()),
                MeshMaterial3d(looks.spark.clone()),
                Transform::from_translation(at).with_scale(Vec3::ZERO),
                NotShadowCaster,
                Spark {
                    age: 0.0,
                    life: shape.spark_s * (0.6 + 0.4 * r(3)),
                    velocity: (way + stray * SPREAD).normalize_or(way) * speed,
                },
            ));
        }
    }
}

/// A glow facing the view, dimming and shrinking as its light dies; gone at its end.
fn glow(
    mut commands: Commands,
    time: Res<Time>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut glows: Query<(Entity, &mut Glow, &mut Transform, Option<&mut PointLight>)>,
) {
    let eye = camera.map(|camera| camera.translation());
    for (entity, mut glow, mut transform, light) in &mut glows {
        glow.age += time.delta_secs();
        let left = 1.0 - glow.age / glow.life;
        if left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.scale = Vec3::splat(glow.wide * (0.6 + 0.4 * left));
        if let Some(mut light) = light {
            light.intensity = LIGHT_LM * left * left;
        }
        if let Some(eye) = eye {
            let at = transform.translation;
            transform.look_at(at * 2.0 - eye, at.normalize_or(Vec3::Y));
        }
    }
}

/// A spark flies and falls, drawn as a streak along its way facing the view, thinning; gone
/// at its end.
fn spark(
    mut commands: Commands,
    time: Res<Time>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut sparks: Query<(Entity, &mut Spark, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let eye = camera.map(|camera| camera.translation());
    for (entity, mut spark, mut transform) in &mut sparks {
        spark.age += dt;
        let left = 1.0 - spark.age / spark.life;
        if left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let up = transform.translation.normalize_or(Vec3::Y);
        spark.velocity -= up * GRAVITY * dt;
        transform.translation += spark.velocity * dt;
        let ahead = spark.velocity.normalize_or(up);
        // The quad's height along the spark's way, its face turned to the view about it.
        let toward = eye.map_or(up.any_orthonormal_vector(), |eye| {
            (eye - transform.translation).normalize_or(up)
        });
        let side = ahead
            .cross(toward)
            .normalize_or(ahead.any_orthonormal_vector());
        transform.rotation = Quat::from_mat3(&Mat3::from_cols(side, ahead, side.cross(ahead)));
        transform.scale = Vec3::new(
            SPARK_THICK_M * left,
            spark.velocity.length() * STREAK_S + SPARK_THICK_M,
            1.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use terra_motion::blade::Edge;
    use terra_motion::moves::{MoveSet, Phase};

    /// A frame at 60.
    const FRAME_S: f32 = 1.0 / 60.0;
    /// A place on the ground of a world 1 km round: up is +y.
    const GROUND: Vec3 = Vec3::new(0.0, 1000.0, 0.0);

    fn set() -> MoveSet {
        let path = terra_world::data::path(super::super::SET);
        MoveSet::read(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Messages<Met>>();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Assets<Image>>();
        world.insert_resource(super::super::Set(set()));
        world
    }

    fn count<T: Component>(world: &mut World) -> usize {
        world.query::<&T>().iter(world).count()
    }

    /// A contact bursts there: a glow at the contact for a few frames at 60 (Enshrouded's 3
    /// to 4, the combat scorecard's `hit.burst.frames`), sparks from it flying on the way the
    /// blade went, and nothing left once the longest spark's life is over.
    #[test]
    fn a_contact_glows_and_sparks_where_the_blade_met_and_is_gone_after_its_life() {
        let shape = set().landed.burst;
        assert!(
            shape.glow_m > 0.0 && shape.sparks > 0,
            "the set shows a burst"
        );
        let mut world = world();
        let at = GROUND + Vec3::new(0.1, 1.2, -1.0);
        let along = Vec3::NEG_X;
        world.write_message(Met { at, along });
        let systems = [
            world.register_system(spawn),
            world.register_system(glow),
            world.register_system(spark),
        ];
        let mut frames = 0;
        let mut flew = None;
        for frame in 0..60 {
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(FRAME_S));
            for system in systems {
                world.run_system(system).unwrap();
            }
            world.resource_mut::<Messages<Met>>().update();
            let glows: Vec<Vec3> = world
                .query_filtered::<&Transform, With<Glow>>()
                .iter(&world)
                .map(|place| place.translation)
                .collect();
            assert!(glows.iter().all(|place| place.distance(at) < 1e-4));
            frames += !glows.is_empty() as u32;
            if frame == 0 {
                let sparks: Vec<Vec3> = world
                    .query_filtered::<&Transform, With<Spark>>()
                    .iter(&world)
                    .map(|place| place.translation - at)
                    .collect();
                assert_eq!(sparks.len(), shape.sparks as usize);
                // One frame on: near the contact, and gone on, on the whole, the blade's way.
                assert!(sparks.iter().all(|off| off.length() < 0.2), "{sparks:?}");
                flew = Some(sparks.iter().sum::<Vec3>() / sparks.len() as f32);
            }
            if frame as f32 * FRAME_S > shape.spark_s.max(shape.glow_s) {
                assert_eq!(count::<Glow>(&mut world) + count::<Spark>(&mut world), 0);
            }
        }
        let flew = flew.unwrap();
        assert!(flew.normalize().dot(along) > 0.6, "sparks went {flew}");
        assert!(
            (1..=6).contains(&frames),
            "a few frames, not a lamp: {frames}"
        );
        println!("measured hit.burst.frames {frames}");
    }

    /// A fighter's blow swept across the other's body in its active frames: a hit and a
    /// guard broken burst at the contact, the way the blade went; a blow on the guard and a
    /// blow that misses show none. The struck body keeps its own colours, frame by frame at
    /// 60 through the burst's life (D487: no whole-body flash); the frames it shows lit are
    /// the combat scorecard's `hit.flash.frames` (bar 0).
    #[test]
    fn only_a_landed_blow_bursts_and_at_the_contact_and_the_body_keeps_its_colours() {
        use super::super::Fighter;
        use terra_survival::noise::Noise;
        use terra_survival::told::Notice;
        use terra_walker::Player;

        // The attacker at the ground, the struck 1.3 m ahead along -z, facing it (+z).
        let struck_at = GROUND + Vec3::new(0.0, 0.0, -1.3);
        let cases = [
            ("slash", None, 0.0, Some("Hit")),
            ("heavy", Some(1.0), 0.0, Some("GuardBroken")),
            ("slash", Some(1.0), 0.0, None),
            ("slash", None, 1.5, None),
        ];
        for (blow, guard, wide_m, bursts) in cases {
            let mut world = world();
            world.init_resource::<Messages<Noise>>();
            world.init_resource::<Notice>();
            world.init_resource::<super::super::contact::Glare>();
            let set = world.resource::<super::super::Set>().0.clone();
            // The sword level at the chest, pointing at the struck: hilt 0.15 m and tip
            // 0.83 m out of the hand, swept from its right (+x) to across its middle.
            let hand_at = |x: f32| {
                GlobalTransform::from(
                    Transform::from_translation(GROUND + Vec3::new(x + wide_m, 1.2, -0.5))
                        .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2)),
                )
            };
            let was = hand_at(0.8);
            let hand = world.spawn(hand_at(0.0)).id();
            let mut attacker = Fighter::new(Some(hand));
            attacker.was = Some(Edge {
                hilt: was.transform_point(Vec3::Y * terra_motion::blade::HILT_M),
                tip: was.transform_point(Vec3::Y * terra_motion::blade::TIP_M),
            });
            attacker.runner.start(blow);
            while attacker
                .runner
                .now(&set)
                .is_none_or(|each| each.phase(attacker.runner.t()) != Phase::Active)
            {
                attacker.runner.tick(&set, FRAME_S);
            }
            world.spawn((Transform::from_translation(GROUND), attacker));
            let mut struck = Fighter::new(None);
            struck.guard_s = guard;
            // The struck body's skin, a mesh of its own with a material of its own.
            let own = world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(Color::srgb(0.6, 0.4, 0.3));
            let skin = world.spawn(MeshMaterial3d(own.clone())).id();
            world
                .spawn((Transform::from_translation(struck_at), struck, Player))
                .add_children(&[skin]);
            world
                .run_system_cached(super::super::contact::strike)
                .unwrap();
            let met: Vec<Met> = world.resource_mut::<Messages<Met>>().drain().collect();
            // The burst plays out over its life; the skin shows its own material, unlit.
            world.write_message_batch(met.iter().copied());
            let systems = [
                world.register_system(spawn),
                world.register_system(glow),
                world.register_system(spark),
            ];
            let mut lit = 0;
            for _ in 0..30 {
                world
                    .resource_mut::<Time>()
                    .advance_by(Duration::from_secs_f32(FRAME_S));
                for system in systems {
                    world.run_system(system).unwrap();
                }
                world.resource_mut::<Messages<Met>>().update();
                let shows = world.get::<MeshMaterial3d<StandardMaterial>>(skin);
                let materials = world.resource::<Assets<StandardMaterial>>();
                let kept = shows.is_some_and(|m| m.0 == own)
                    && materials
                        .get(&own)
                        .is_some_and(|m| m.emissive == LinearRgba::BLACK);
                lit += !kept as u32;
            }
            assert_eq!(
                lit, 0,
                "{blow}: the struck body keeps its own colours (D487)"
            );
            if bursts == Some("Hit") {
                println!("measured hit.flash.frames {lit}");
            }
            match bursts {
                Some(taken) => {
                    assert_eq!(met.len(), 1, "{blow} {taken}");
                    let at = met[0].at;
                    assert!(
                        (at - struck_at).reject_from(Vec3::Y).length() < 0.3 + 1e-3,
                        "{taken} at the struck body's capsule: {at}"
                    );
                    assert!(met[0].along.dot(Vec3::NEG_X) > 0.9, "{:?}", met[0].along);
                }
                None => assert!(met.is_empty(), "{blow} guard {guard:?} off {wide_m}"),
            }
        }
    }
}
