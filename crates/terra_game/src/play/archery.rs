//! The bow (D97; the MVP's ranged weapon): hold the use key to draw, let go to loose an
//! arrow from the backpack toward where you aim. The longer the draw, up to a full one,
//! the faster and harder it flies. Arrows fall toward the planet's middle as they go, so
//! a far shot is aimed high, and they stick where they land.
//!
//! Archery is a skill of its own (D88): the more you shoot, the harder your arrows hit.
//! An arrow is used up when shot. A deer hit runs, as it does from anything that hurts
//! it, so a hunter shoots from further than it can hear them.

use super::bag::Notice;
use super::bodies::Body;
use super::combat::{Struck, strike};
use super::controls::{Action, Controls};
use super::creatures::Creature;
use super::inventory::{Belongings, InventoryScreen, Raw, Tool};
use super::models::bake;
use super::models::read_model;
use super::skills::{Did, Skill, Skills};
use super::{Player, View};
use crate::build::Site;
use crate::planet::MainCamera;
use crate::planet::Planet;
use bevy::prelude::*;

/// Seconds to draw the bow fully.
const FULL_DRAW_S: f32 = 0.9;
/// A draw shorter than this is let go without a shot.
const LEAST_DRAW_S: f32 = 0.15;
/// How fast an arrow leaves a full draw, in metres a second, and how hard it hits.
const FULL_SPEED: f32 = 42.0;
const FULL_DAMAGE: f32 = 16.0;
const GRAVITY: f32 = 9.8;
/// How long an arrow flies before it is given up, and lies where it stuck.
const FLIES_S: f32 = 6.0;
const STUCK_S: f32 = 20.0;
/// How far ahead the aiming mark is looked along for something to shoot at.
const AIM_M: f32 = 80.0;
/// How near an arrow has to pass a body to hit it, beyond the body's own size.
const GRAZE_M: f32 = 0.12;

/// How hard a bow hits and how fast it shoots at full draw, for a station's screen.
pub fn bow() -> (f32, f32) {
    (FULL_DAMAGE, FULL_SPEED)
}

#[derive(Resource, Default)]
struct Drawing {
    /// Seconds the bow has been held drawn, if it is.
    held: Option<f32>,
}

/// An arrow in flight, or stuck where it landed.
#[derive(Component)]
struct Arrow {
    velocity: Vec3,
    /// How hard it hits at full speed, by the draw it was shot with.
    damage: f32,
    age: f32,
    stuck: bool,
}

#[derive(Resource)]
struct ArrowLook {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Drawing>()
        .add_systems(Startup, load)
        .add_systems(Update, (draw, fly).chain());
}

fn load(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = match read_model("tools/Arrow.glb") {
        Ok(mut parts) => {
            // The file's arrow points along -Z; baking wants it along +Y, tip first.
            let turn = Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::Y);
            for part in &mut parts {
                for point in part.positions.iter_mut().chain(part.normals.iter_mut()) {
                    *point = turn * *point;
                }
            }
            bake(&parts, 0.75, 0.0, None)
        }
        Err(problem) => {
            error!("Could not read the arrow. {problem}");
            Cuboid::new(0.02, 0.75, 0.02).into()
        }
    };
    commands.insert_resource(ArrowLook {
        mesh: meshes.add(mesh),
        material: materials.add(StandardMaterial {
            perceptual_roughness: 0.7,
            ..default()
        }),
    });
}

/// Holding the use key with the bow draws it; letting go looses an arrow, if there is one.
#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    time: Res<Time>,
    controls: Controls,
    screen: Res<InventoryScreen>,
    view: Res<View>,
    look: Res<ArrowLook>,
    planet: Res<Planet>,
    creatures: Query<(&Creature, &Transform, &Body), Without<Player>>,
    camera: Single<&Transform, (With<MainCamera>, Without<Player>)>,
    player: Single<(&Transform, &mut Player)>,
    mut belongings: Belongings,
    mut drawing: ResMut<Drawing>,
    mut notice: ResMut<Notice>,
) {
    let (at, mut player) = player.into_inner();
    let ready = belongings.holding() == Some(Tool::Bow) && !screen.open && player.down.is_none();
    if !ready {
        drawing.held = None;
        return;
    }
    if controls.held(Action::Use) {
        let held = drawing.held.unwrap_or(0.0) + time.delta_secs();
        drawing.held = Some(held.min(FULL_DRAW_S));
        player.since_acting = 0.0;
        return;
    }
    let Some(held) = drawing.held.take() else {
        return;
    };
    if held < LEAST_DRAW_S {
        return;
    }
    if !belongings.pay(&[(Raw::Arrow, 1)], 1) {
        notice.say(format!("You have no arrows: {}", Raw::Arrow.where_found()));
        return;
    }
    // From the bow toward what the middle of the view is on, some way off.
    let up = at.translation.normalize();
    let right = view.forward.cross(up);
    let from = at.translation + up * 1.45 + right * 0.25 + view.forward * 0.4;
    // Whatever the middle of the view is on: a creature, the ground, or far off.
    let eye = camera.translation;
    let ahead = *camera.forward();
    let on_creature = creatures
        .iter()
        .filter(|(creature, ..)| creature.alive())
        .filter_map(|(_, at, body)| {
            let middle = at.translation + at.translation.normalize() * body.radius;
            passes_within(eye, eye + ahead * AIM_M, middle, body.radius)
        })
        .min_by(|a, b| a.total_cmp(b))
        .map(|along| along * AIM_M);
    let on_ground = planet.raycast(eye, ahead, AIM_M).map(|hit| hit.distance);
    let distance = [on_creature, on_ground]
        .into_iter()
        .flatten()
        .fold(AIM_M, f32::min);
    let aimed = eye + ahead * distance;
    let direction = (aimed - from).normalize_or(view.forward);
    let pull = held / FULL_DRAW_S;
    let velocity = direction * FULL_SPEED * pull;
    commands.spawn((
        Name::new("Arrow"),
        Arrow {
            velocity,
            damage: FULL_DAMAGE * pull,
            age: 0.0,
            stuck: false,
        },
        Mesh3d(look.mesh.clone()),
        MeshMaterial3d(look.material.clone()),
        Transform::from_translation(from)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, direction)),
    ));
    player.attacking = 0.4;
    info!("Loosed an arrow at {:.0}% draw.", pull * 100.0);
    belongings.wear_in_hand(1);
}

/// Arrows fly, fall, hit what they pass through, and stick where they land.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<Planet>,
    site: Res<Site>,
    skills: Single<&Skills, With<Player>>,
    mut arrows: Query<(Entity, &mut Arrow, &mut Transform), Without<Creature>>,
    mut creatures: Query<(Entity, &mut Creature, &Transform, &Body), Without<Arrow>>,
    mut did: MessageWriter<Did>,
    mut struck: MessageWriter<Struck>,
    mut notice: ResMut<Notice>,
) {
    let dt = time.delta_secs().min(0.05);
    for (entity, mut arrow, mut transform) in &mut arrows {
        arrow.age += dt;
        if arrow.stuck {
            if arrow.age > STUCK_S {
                commands.entity(entity).despawn();
            }
            continue;
        }
        if arrow.age > FLIES_S {
            commands.entity(entity).despawn();
            continue;
        }
        let from = transform.translation;
        let up = from.normalize();
        arrow.velocity -= up * GRAVITY * dt;
        let to = from + arrow.velocity * dt;

        // The first living body along the way, if any.
        let hit = creatures
            .iter_mut()
            .filter(|(_, creature, ..)| creature.alive())
            .filter_map(|(entity, creature, at, body)| {
                let middle = at.translation + at.translation.normalize() * body.radius;
                let along = passes_within(from, to, middle, body.radius + GRAZE_M)?;
                Some((along, entity, creature))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, hit_entity, mut creature)) = hit {
            let speed = arrow.velocity.length() / FULL_SPEED;
            let blow = arrow.damage * speed.min(1.0) * skills.speed(Skill::Archery);
            strike(
                &mut commands,
                hit_entity,
                &mut creature,
                blow,
                to,
                Skill::Archery,
                &mut did,
                &mut struck,
                &mut notice,
            );
            info!("An arrow hit a {:?} for {blow:.0}.", creature.species);
            if !creature.alive()
                && let Ok((.., at, _)) = creatures.get(hit_entity)
            {
                // Where, in the terms --at takes, so a scripted hunt can go to it.
                let local = site
                    .frame
                    .compute_affine()
                    .inverse()
                    .transform_point3(at.translation);
                info!(
                    "It fell {:.1} m east and {:.1} m south of the start.",
                    local.x, local.z
                );
            }
            commands.entity(entity).despawn();
            continue;
        }

        let ground = planet.ground_at(to);
        if to.length() <= ground.length() {
            transform.translation = ground;
            arrow.stuck = true;
            arrow.age = 0.0;
            continue;
        }
        transform.translation = to;
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, arrow.velocity.normalize_or(Vec3::Y));
    }
}

/// How far along the segment from `from` to `to` it first comes within `radius` of
/// `point`, out of one; `None` if it never does.
fn passes_within(from: Vec3, to: Vec3, point: Vec3, radius: f32) -> Option<f32> {
    let step = to - from;
    let length_squared = step.length_squared();
    let along = if length_squared > 1e-8 {
        ((point - from).dot(step) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let nearest = from + step * along;
    (nearest.distance_squared(point) <= radius * radius).then_some(along)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arrow_hits_what_it_passes_close_to() {
        let (from, to) = (Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0));
        let along = passes_within(from, to, Vec3::new(4.0, 0.3, 0.0), 0.5).unwrap();
        assert!((along - 0.4).abs() < 1e-5);
        assert!(passes_within(from, to, Vec3::new(4.0, 0.8, 0.0), 0.5).is_none());
        assert!(passes_within(from, to, Vec3::new(12.0, 0.0, 0.0), 0.5).is_none());
    }

    #[test]
    fn a_full_draw_hits_about_as_hard_as_a_spear() {
        let (spear, _) = super::super::combat::weapon(Tool::Spear);
        let (bow, _) = bow();
        assert!(
            bow < spear && bow > spear * 0.6,
            "a bow is a different way to fight, not a better one"
        );
    }
}
