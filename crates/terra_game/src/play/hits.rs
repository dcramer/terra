//! Showing a blow land (the developers' list for the MVP, prototypes.md).
//!
//! A creature struck gives a spray of what is inside it, blood from a deer and pale
//! ichor from a spider, and its body jerks; a killing or critical blow (D119) sprays more
//! and kicks the view harder. The player
//! struck sees red at the edges of the view and the view shakes. The sounds are
//! `noises`' job. No numbers float up: what a blow did shows in how the creature acts.
//!
//! It reads [`Struck`] and [`Hurt`], which `combat` writes, so how a blow was dealt,
//! swung or shot, is no concern here.

use super::IsLocalPlayer;
use super::combat::{Hurt, Struck};
use super::shake::{self, Shake};
use super::species::{Bestiary, Blood};
use bevy::prelude::*;

/// How long a drop of the spray lasts, and how many a blow throws, or a killing one.
const SPARK_S: f32 = 0.55;
const SPARKS: usize = 9;
const KILL_SPARKS: usize = 18;
/// How long a struck body jerks, and by how much it swells at most.
const JOLT_S: f32 = 0.18;
const JOLT: f32 = 0.12;
/// How long the view shakes, and how far, in metres: when hurt, and when landing a blow.
const SHAKE_S: f32 = 0.3;
const HURT_SHAKE_M: f32 = 0.09;
const HIT_SHAKE_M: f32 = 0.025;
/// How long the red at the edges takes to go, and how red it is at most.
const PAIN_S: f32 = 0.7;
const PAIN: f32 = 0.55;

/// One drop of the spray.
#[derive(Component)]
struct Spark {
    velocity: Vec3,
    life: f32,
    size: f32,
}

/// A struck creature's body, jerking: how much longer, and its size at rest.
#[derive(Component)]
struct Jolt {
    left: f32,
    rest: Vec3,
}

/// The shape and colours of the spray.
#[derive(Resource)]
struct Spray {
    drop: Handle<Mesh>,
    blood: Handle<StandardMaterial>,
    ichor: Handle<StandardMaterial>,
}

/// The red at the edges of the view, and how much of it is left, out of one.
#[derive(Component)]
struct Pain;

#[derive(Resource, Default)]
struct Hurting(f32);

pub fn plugin(app: &mut App) {
    // The view's shake is the walker's (`shake`); blows kick it before it runs down.
    let kicks = hurt.before(shake::run_down);
    app.init_resource::<Hurting>()
        .add_systems(Startup, setup)
        .add_systems(Update, (spray, jolt, kicks, (fly, settle, redden)).chain());
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let wet = |colour: Color| StandardMaterial {
        base_color: colour,
        perceptual_roughness: 0.35,
        ..default()
    };
    commands.insert_resource(Spray {
        drop: meshes.add(Sphere::new(1.0).mesh().uv(6, 4)),
        blood: materials.add(wet(Color::srgb(0.45, 0.03, 0.03))),
        ichor: materials.add(wet(Color::srgb(0.72, 0.78, 0.45))),
    });
    commands.spawn((
        Pain,
        Name::new("Pain"),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundGradient::from(edges(0.0)),
        Pickable::IGNORE,
    ));
}

/// Red at the edges of the view, clear in the middle, so strong.
fn edges(strength: f32) -> RadialGradient {
    RadialGradient::new(
        UiPosition::CENTER,
        RadialGradientShape::FarthestCorner,
        vec![
            ColorStop::new(Color::srgba(0.6, 0.0, 0.0, 0.0), percent(45)),
            ColorStop::new(Color::srgba(0.6, 0.0, 0.0, strength), percent(100)),
        ],
    )
}

/// A cheap scatter, the same on every run, for which way each drop goes.
fn scatter(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Throws a spray where a blow landed, and sets the struck body jerking.
fn spray(
    mut commands: Commands,
    look: Res<Spray>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    children: Query<&Children>,
    bodies: Query<&Transform, Without<Jolt>>,
    mut struck: MessageReader<Struck>,
    bestiary: Res<Bestiary>,
    mut shake: ResMut<Shake>,
    mut state: Local<u32>,
) {
    if *state == 0 {
        *state = 0x9e37_79b9;
    }
    for blow in struck.read() {
        let up = blow.at.normalize_or(Vec3::Y);
        // Out of the side that was struck, toward whoever struck it, and up a little.
        let out = (camera.translation() - blow.at)
            .reject_from(up)
            .normalize_or(up);
        let material = match bestiary.get(blow.species).blood {
            Blood::Red => look.blood.clone(),
            Blood::Ichor => look.ichor.clone(),
        };
        // A killing or critical blow throws more, and kicks harder.
        let count = if blow.killed || blow.critical {
            KILL_SPARKS
        } else {
            SPARKS
        };
        for _ in 0..count {
            let wild = Vec3::new(
                scatter(&mut state),
                scatter(&mut state),
                scatter(&mut state),
            );
            let velocity = (out * 1.6 + up * 1.4 + wild * 1.3) * (1.0 + 0.4 * scatter(&mut state));
            let size = 0.055 + 0.02 * scatter(&mut state);
            commands.spawn((
                Name::new("Spray"),
                Spark {
                    velocity,
                    life: SPARK_S,
                    size,
                },
                Mesh3d(look.drop.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(blow.at).with_scale(Vec3::splat(size)),
            ));
        }
        // The creature's model is its first child; it swells and settles.
        if let Some(&body) = children.get(blow.creature).ok().and_then(|c| c.first())
            && let Ok(rest) = bodies.get(body)
        {
            commands.entity(body).try_insert(Jolt {
                left: JOLT_S,
                rest: rest.scale,
            });
        }
        let kick = if blow.critical {
            HIT_SHAKE_M * 2.0
        } else {
            HIT_SHAKE_M
        };
        shake.kick(kick, SHAKE_S * 0.5);
    }
}

/// A struck body swells for a moment, then is its own size again.
fn jolt(
    mut commands: Commands,
    time: Res<Time>,
    mut bodies: Query<(Entity, &mut Jolt, &mut Transform)>,
) {
    for (entity, mut jolt, mut transform) in &mut bodies {
        jolt.left -= time.delta_secs();
        if jolt.left <= 0.0 {
            transform.scale = jolt.rest;
            commands.entity(entity).remove::<Jolt>();
            continue;
        }
        let through = 1.0 - jolt.left / JOLT_S;
        let swell = libm::sinf(through * std::f32::consts::PI) * JOLT;
        transform.scale = jolt.rest * (1.0 + swell);
    }
}

/// This machine's player hurt: red at the edges, and the view shakes, both by how bad it was.
fn hurt(
    mut blows: MessageReader<Hurt>,
    ours: Single<Entity, IsLocalPlayer>,
    mut hurting: ResMut<Hurting>,
    mut shake: ResMut<Shake>,
) {
    for &Hurt { player, amount } in blows.read() {
        if player != *ours {
            continue;
        }
        let how_bad = (0.4 + amount / 25.0).min(1.0);
        hurting.0 = hurting.0.max(how_bad);
        shake.kick(HURT_SHAKE_M * how_bad, SHAKE_S);
    }
}

/// Drops fly, fall and shrink away.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut sparks: Query<(Entity, &mut Spark, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut spark, mut transform) in &mut sparks {
        spark.life -= dt;
        if spark.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let up = transform.translation.normalize_or(Vec3::Y);
        spark.velocity -= up * 9.8 * dt;
        transform.translation += spark.velocity * dt;
        transform.scale = Vec3::splat(spark.size * (spark.life / SPARK_S).sqrt());
    }
}

/// The pain runs down. It is not touched once it has, so what draws it is not woken for
/// nothing.
fn settle(time: Res<Time>, mut hurting: ResMut<Hurting>) {
    if hurting.0 > 0.0 {
        hurting.0 = (hurting.0 - time.delta_secs() / PAIN_S).max(0.0);
    }
}

fn redden(hurting: Res<Hurting>, mut pain: Single<&mut BackgroundGradient, With<Pain>>) {
    if !hurting.is_changed() {
        return;
    }
    pain.0 = vec![edges(hurting.0 * PAIN).into()];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scatter_throws_both_ways_within_its_bounds() {
        let mut state = 0x9e37_79b9;
        let scattered: Vec<f32> = (0..64).map(|_| scatter(&mut state)).collect();
        assert!(scattered.iter().all(|x| (-1.0..=1.0).contains(x)));
        assert!(scattered.iter().any(|&x| x > 0.5) && scattered.iter().any(|&x| x < -0.5));
    }
}
