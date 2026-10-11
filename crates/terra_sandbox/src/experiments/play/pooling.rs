//! A place where magic pools (game design 001; D103): a glow in a hollow, seen from far
//! off, where monsters form. Spiders guard it, and while fewer than three remain another
//! forms every so often. A smith from the village kneels bound at its edge; with the
//! spiders dead, F frees them (`people`).
//!
//! Nothing marks it on a map or a compass: you see the glow, or someone tells you where
//! to look (D92).

use super::creatures::{Creature, Form, Lair, Species};
use super::interact::Interactable;
use super::people::{Arrive, Doing};
use bevy::{light::NotShadowCaster, prelude::*};

/// How many spiders the place keeps, and how often it forms another below that.
const KEEPS: usize = 3;
const FORMS_EVERY_S: f32 = 90.0;
const MOTES: usize = 28;
const GLOW: Color = Color::srgb(0.66, 0.36, 1.0);
const GLOW_LUMENS: f32 = 900_000.0;

#[derive(Component)]
struct Mote {
    /// Where it rises from, around the middle, and how far through its rise it is.
    from: Vec3,
    through: f32,
    rate: f32,
}

#[derive(Resource)]
struct Pool {
    middle: Vec3,
    until_next: f32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            mark.run_if(resource_added::<Lair>),
            (rise, form).run_if(resource_exists::<Pool>),
        ),
    );
}

/// Once the lair is chosen, lights it, fills it with motes and puts the prisoner there.
fn mark(
    mut commands: Commands,
    lair: Res<Lair>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut arrive: MessageWriter<Arrive>,
) {
    let middle = lair.0;
    let up = middle.normalize();
    let east = up.any_orthonormal_vector();
    commands.insert_resource(Pool {
        middle,
        until_next: FORMS_EVERY_S,
    });
    commands.spawn((
        Name::new("Pooled magic"),
        PointLight {
            color: GLOW,
            intensity: GLOW_LUMENS,
            range: 40.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(middle + up * 2.0),
    ));
    let mote = meshes.add(Sphere::new(0.07).mesh().ico(1).expect("a small sphere"));
    let glowing = materials.add(StandardMaterial {
        base_color: GLOW,
        emissive: LinearRgba::rgb(6.0, 3.0, 12.0),
        unlit: true,
        ..default()
    });
    for index in 0..MOTES {
        let turn = index as f32 / MOTES as f32 * std::f32::consts::TAU * 3.7;
        let out = Quat::from_axis_angle(up, turn) * east * (0.6 + (index % 7) as f32 * 0.9);
        commands.spawn((
            Name::new("Mote"),
            Mote {
                from: middle + out,
                through: index as f32 / MOTES as f32,
                rate: 0.12 + (index % 5) as f32 * 0.03,
            },
            Mesh3d(mote.clone()),
            MeshMaterial3d(glowing.clone()),
            NotShadowCaster,
            Transform::from_translation(middle),
        ));
    }
    // The smith the village lost, bound at the edge of the hollow.
    let edge = middle + Quat::from_axis_angle(up, 2.2) * east * 6.0;
    arrive.write(Arrive {
        name: "Bren",
        at: edge,
        facing: (middle - edge).reject_from(up),
        doing: Doing::Bound,
        smith: true,
        colour: Color::srgb(0.45, 0.32, 0.22),
        interact: Some(Interactable::Prisoner),
        says: None,
    });
}

/// Motes drift up from the hollow and fade, over and over.
fn rise(time: Res<Time>, pool: Res<Pool>, mut motes: Query<(&mut Mote, &mut Transform)>) {
    let up = pool.middle.normalize();
    for (mut mote, mut at) in &mut motes {
        mote.through = (mote.through + mote.rate * time.delta_secs()).fract();
        let sway = libm::sinf((time.elapsed_secs() + mote.through * 9.0) * 1.3) * 0.25;
        at.translation = mote.from + up * (mote.through * 4.5) + up.any_orthonormal_vector() * sway;
        at.scale = Vec3::splat(1.0 - mote.through);
    }
}

/// While fewer than three spiders remain, another forms now and then.
fn form(
    time: Res<Time>,
    mut pool: ResMut<Pool>,
    creatures: Query<&Creature>,
    mut forming: MessageWriter<Form>,
) {
    let spiders = creatures
        .iter()
        .filter(|creature| creature.species == Species::Spider && creature.alive())
        .count();
    if spiders >= KEEPS {
        pool.until_next = FORMS_EVERY_S;
        return;
    }
    pool.until_next -= time.delta_secs();
    if pool.until_next <= 0.0 {
        pool.until_next = FORMS_EVERY_S;
        forming.write(Form(Species::Spider, pool.middle));
        info!("A spider formed where the magic pools.");
    }
}
