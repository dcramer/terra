//! A place where magic pools (game design 001; D103): a glow in a hollow, seen from far
//! off, where monsters form. Spiders guard it, and while fewer than three remain another
//! forms every so often. A smith from the village kneels bound at its edge; with the
//! spiders dead, F frees them (`people`).
//!
//! Nothing marks it on a map or a compass: you see the glow, or someone tells you where
//! to look (D92).

use super::creatures::{Creature, Form, Lair, Species};
use super::flora::ClearAround;
use super::interact::Interactable;
use super::people::{Arrive, Doing, Dyed};
use bevy::{light::NotShadowCaster, prelude::*};

/// How many spiders the place keeps, and how often it forms another below that.
const KEEPS: usize = 3;
const FORMS_EVERY_S: f32 = 90.0;
const MOTES: usize = 60;
const GLOW: Color = Color::srgb(0.66, 0.36, 1.0);
const GLOW_LUMENS: f32 = 4_000_000.0;

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
    mut clear: MessageWriter<ClearAround>,
) {
    let middle = lair.0;
    let up = middle.normalize();
    let east = up.any_orthonormal_vector();
    clear.write(ClearAround(middle, 14.0));
    commands.insert_resource(Pool {
        middle,
        until_next: FORMS_EVERY_S,
    });
    commands.spawn((
        Name::new("Pooled magic"),
        PointLight {
            color: GLOW,
            intensity: GLOW_LUMENS,
            range: 60.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(middle + up * 2.0),
    ));
    // The pooled magic itself: a low, glowing pool on the ground.
    commands.spawn((
        Name::new("Pool"),
        Mesh3d(meshes.add(Cylinder::new(3.2, 0.08).mesh().resolution(24))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: GLOW,
            emissive: LinearRgba::rgb(4.0, 1.8, 9.0),
            ..default()
        })),
        NotShadowCaster,
        Transform::from_translation(middle + up * 0.05).looking_to(east, up),
    ));
    let mote = meshes.add(Sphere::new(0.13).mesh().ico(1).expect("a small sphere"));
    let glowing = materials.add(StandardMaterial {
        base_color: GLOW,
        emissive: LinearRgba::rgb(6.0, 3.0, 12.0),
        unlit: true,
        ..default()
    });
    for index in 0..MOTES {
        let turn = index as f32 / MOTES as f32 * std::f32::consts::TAU * 3.7;
        let out = Quat::from_axis_angle(up, turn) * east * (0.4 + (index % 9) as f32 * 0.5);
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
        trade: Some("smith"),
        body: "base-male",
        wears: vec![Dyed(
            "leather_coat",
            Some(("Leather", Color::srgb(0.45, 0.32, 0.22))),
        )],
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
        at.translation = mote.from + up * (mote.through * 7.0) + up.any_orthonormal_vector() * sway;
        at.scale = Vec3::splat(1.0 - mote.through);
    }
}

/// While fewer than three of a kind that forms here remain (`pools` in
/// `assets/data/creatures.ron`), another forms now and then.
fn form(
    time: Res<Time>,
    mut pool: ResMut<Pool>,
    creatures: Query<&Creature>,
    mut forming: MessageWriter<Form>,
) {
    let alive = |species| {
        creatures
            .iter()
            .filter(|creature| creature.species == species && creature.alive())
            .count()
    };
    let short = Species::all().find(|&species| species.def().pools && alive(species) < KEEPS);
    let Some(species) = short else {
        pool.until_next = FORMS_EVERY_S;
        return;
    };
    pool.until_next -= time.delta_secs();
    if pool.until_next <= 0.0 {
        pool.until_next = FORMS_EVERY_S;
        forming.write(Form(species, pool.middle));
        info!(
            "A {} formed where the magic pools.",
            species.name().to_lowercase()
        );
    }
}
