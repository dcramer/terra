//! How a player moves over the planet: walking, sprinting, rolling and jumping, held up
//! by the ground and their base, and stopped by walls, bodies and the sea.

use super::controls::Action;
use super::intent::Intent;
use super::tick::Sim;
use super::{Downed, Motion, Player, Stance, View, bodies, flora, props, vitals};
use crate::build::{self, Site};
use crate::planet::Planet;
use bevy::prelude::*;
use terra_build::Walker;

const JOG_SPEED: f32 = 4.3;
const SPRINT_SPEED: f32 = 7.4;
const JUMP_SPEED: f32 = 6.0;
const GRAVITY: f32 = 16.0;
/// What a jump costs in stamina, out of 100.
const JUMP_STAMINA: f32 = 8.0;
/// A roll: what it costs, how long it lasts and how fast it goes.
const DODGE_STAMINA: f32 = 18.0;
const DODGE_S: f32 = 0.45;
const DODGE_SPEED: f32 = 8.5;
/// The player can wade this deep and no deeper.
const WADE_DEPTH_M: f32 = 0.7;
/// The steepest ground the player walks up, as rise over run (about 50 degrees).
const STEEPEST: f32 = 1.2;
/// The highest ledge the player steps onto, and how far on it is looked for.
const STEP_UP_M: f32 = 0.7;
const STRIDE_M: f32 = 0.4;

pub fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, walk.in_set(Sim::Act));
}

/// Whether walking from the feet to a place is stopped by a wall: ground rising there
/// more steeply than the player walks up, and still too high to step onto a stride on.
/// `ground` says how far from the centre the ground is at a place.
fn walled(feet: Vec3, target: Vec3, ground: impl Fn(Vec3) -> f32) -> bool {
    let up = feet.normalize();
    let across = (target - feet).reject_from(up);
    let rise = ground(target) - feet.length();
    let steep = rise > 0.05 && rise > across.length() * STEEPEST;
    let beyond = feet + across.normalize_or_zero() * STRIDE_M;
    steep && ground(beyond) - feet.length() > STEP_UP_M
}

/// Whether a body standing at a place has ground at its chest or head. `room` says how
/// far a point is from the ground.
fn cramped(stand: Vec3, walker: &Walker, room: impl Fn(Vec3) -> f32) -> bool {
    let up = stand.normalize();
    [0.9, walker.height - walker.radius]
        .into_iter()
        .any(|height| room(stand + up * height) < walker.radius * 0.8)
}

/// Moves every player by what they mean to do: walking, sprinting, rolling and jumping,
/// held up by the ground and their base, and stopped by walls, bodies and the sea.
fn walk(
    time: Res<Time>,
    planet: Res<Planet>,
    site: Res<Site>,
    builds: Query<&build::Built>,
    obstacles: Res<flora::Obstacles>,
    items: Query<(&props::Prop, &Transform, &Visibility), Without<Player>>,
    others: Query<(&bodies::Body, &Transform), Without<Player>>,
    mut players: Query<
        (
            &mut Transform,
            &mut Motion,
            &mut Stance,
            &mut vitals::Vitals,
            &View,
            &Intent,
            Has<Downed>,
        ),
        With<Player>,
    >,
) {
    for (mut transform, mut motion, mut stance, mut vitals, view, intent, downed) in &mut players {
        // Lying where you fell, you do not move.
        if downed {
            motion.velocity = Vec3::ZERO;
            continue;
        }
        let walker = Walker::default();
        let dt = time.delta_secs().min(0.05);
        let feet = transform.translation;
        let up = feet.normalize();
        let right = view.forward.cross(up);

        let held = |action| f32::from(intent.held(action));
        let wish = (view.forward * (held(Action::Forward) - held(Action::Back))
            + right * (held(Action::Right) - held(Action::Left)))
        .normalize_or_zero();
        // A roll carries you a short way fast, clear of blows; a guard slows you.
        stance.attacking = (stance.attacking - dt).max(0.0);
        stance.flinch = (stance.flinch - dt).max(0.0);
        stance.dodging = (stance.dodging - dt).max(0.0);
        if stance.dodging == 0.0
            && motion.on_ground
            && intent.pressed(Action::Dodge)
            && vitals.spend(DODGE_STAMINA)
        {
            stance.dodging = DODGE_S;
            stance.dodge_direction = if wish == Vec3::ZERO {
                -view.forward
            } else {
                wish
            };
        }
        let top_speed = if stance.dodging > 0.0 {
            DODGE_SPEED
        } else if stance.guarding {
            JOG_SPEED * 0.5
        } else if intent.held(Action::Sprint) && vitals.can_sprint() {
            SPRINT_SPEED
        } else {
            JOG_SPEED
        };
        // Ease into and out of a run rather than starting and stopping dead.
        let ease = 1.0 - libm::expf(-dt * if motion.on_ground { 11.0 } else { 2.5 });
        let wish = if stance.dodging > 0.0 {
            stance.dodge_direction
        } else {
            wish
        };
        motion.velocity = motion.velocity.lerp(wish * top_speed, ease).reject_from(up);
        let mut target = feet + motion.velocity * dt;
        // Furniture and kit are solid. Walls come after, so they have the last word.
        target = props::push_out(target, walker.radius, items.iter());
        // Creatures and people take up room.
        target = bodies::push_out(target, walker.radius, others.iter());

        // The build being walked at decides what blocks the way: the player's own, or someone
        // else's, such as a house in the village. Builds stand well apart, so it is the one
        // the player is at or nearest. It works in its own space.
        let site: &build::Build = builds
            .iter()
            .map(|built| &built.0)
            .find(|built| built.covers(feet, 3.0))
            .unwrap_or(&site);
        let to_base = site.frame.compute_affine().inverse();
        let from_base = site.frame.compute_affine();
        let walked = site.walkable().walk(
            to_base.transform_point3(feet),
            to_base.transform_point3(target),
            &walker,
        );
        target = from_base.transform_point3(walked);
        // Trees and boulders push the player round them.
        for (at, radius) in obstacles.near(target, walker.radius) {
            let apart = (target - at).reject_from(up);
            let reach = radius + walker.radius;
            if apart.length_squared() < reach * reach && (target - at).dot(up).abs() < 3.0 {
                target += apart.normalize_or(view.forward) * (reach - apart.length());
            }
        }
        // The sea is for looking at: wade in a little way and no further.
        let ground_ahead = planet.ground_at(target).length();
        if ground_ahead < planet.sea_radius_m() - WADE_DEPTH_M
            && ground_ahead < planet.ground_at(feet).length()
        {
            target = feet;
        }
        // Ground rising too steeply is a wall, as the side of a dug hole is: it stops the
        // player rather than lifting them up it, and they slide along it as far as it lets
        // them. A jump can clear a low one; hills are walked up as before.
        let ground = |at: Vec3| planet.ground_at(at).length();
        if walled(feet, target, ground) {
            let facing = planet
                .normal(planet.ground_at(target))
                .reject_from(up)
                .normalize_or_zero();
            let step = target - feet;
            let along = feet + step - facing * step.dot(facing).min(0.0);
            target = if walled(feet, along, ground) {
                feet
            } else {
                along
            };
        }
        // The body must fit where it goes, as it does not under the overhang of a slanting
        // pit. Someone already caught in the ground may move, to get out.
        let room = |at: Vec3| planet.distance(at);
        let stands = |at: Vec3| planet.ground_at(at);
        if cramped(stands(target), &walker, room) && !cramped(stands(feet), &walker, room) {
            target = feet;
        }

        motion.rising -= GRAVITY * dt;
        if motion.on_ground && intent.pressed(Action::Jump) && vitals.spend(JUMP_STAMINA) {
            motion.rising = JUMP_SPEED;
        }
        target += up * motion.rising * dt;

        // What holds the player up: indoors, the base's floor; outdoors, whichever is higher of
        // the ground and anything the base offers there, such as its steps or a roof.
        let local = to_base.transform_point3(target);
        let before = to_base.transform_point3(feet);
        let offered = site.support(Vec3::new(local.x, before.y.max(local.y), local.z), &walker);
        let offered = offered.map(|height| {
            from_base
                .transform_point3(Vec3::new(local.x, height, local.z))
                .length()
        });
        // The ground as it is drawn, so the player stands on what they see. Near the start
        // that is the true ground; further off the planet's mesh is coarser, and walking on
        // the true ground there sinks the player into the drawn one or lifts them off it.
        let ground = planet.ground_at(target).length();
        let held_at = match offered {
            Some(floor) if site.walkable().ceiling(before).is_some() => floor,
            Some(surface) => surface.max(ground),
            None => ground,
        };
        motion.on_ground = target.length() <= held_at + 0.02 && motion.rising <= 0.0;
        if motion.on_ground {
            target = target.normalize() * held_at;
            motion.rising = 0.0;
            motion.airborne = 0.0;
        } else {
            motion.airborne += dt;
        }
        // A ceiling stops a jump.
        if let Some(ceiling) = site.walkable().ceiling(before)
            && motion.rising > 0.0
            && to_base.transform_point3(target).y + walker.height > ceiling
        {
            target =
                from_base.transform_point3(Vec3::new(local.x, ceiling - walker.height, local.z));
            motion.rising = 0.0;
        }

        let indoors = site
            .walkable()
            .ceiling(to_base.transform_point3(target))
            .is_some();
        if indoors != motion.indoors {
            motion.indoors = indoors;
            info!(
                "Player is now {}.",
                if indoors {
                    "inside the base"
                } else {
                    "outside"
                }
            );
        }

        // What was actually covered, which is less than what was asked when something is in
        // the way. The figure's stride follows this.
        let up = target.normalize();
        motion.velocity = ((target - feet) / dt.max(1e-4)).reject_from(up);
        // Turn to face the way the figure is going, or the way it is working.
        stance.since_acting += dt;
        let toward = if stance.since_acting < 0.8 {
            view.forward
        } else {
            wish
        };
        let turn = 1.0 - libm::expf(-dt * 12.0);
        // Turning right round has no shorter way to go, so it is given one.
        let about_face = toward != Vec3::ZERO && motion.facing.dot(toward) < -0.98;
        let from = if about_face {
            motion.facing + motion.facing.cross(up) * 0.3
        } else {
            motion.facing
        };
        motion.facing = from
            .lerp(toward, if toward == Vec3::ZERO { 0.0 } else { turn })
            .reject_from(up)
            .normalize_or(view.forward);
        *transform = Transform::from_translation(target).looking_to(motion.facing, up);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hill_and_a_ledge_are_walked_up_and_a_wall_is_not() {
        let feet = Vec3::Y * 2000.0;
        let stride = feet + Vec3::X * 0.07;
        // Ground rising one in three.
        let hill = |at: Vec3| 2000.0 + at.x / 3.0;
        assert!(!walled(feet, stride, hill));
        // A ledge a little ahead, low and then high.
        let ledge = |height: f32| move |at: Vec3| 2000.0 + if at.x > 0.05 { height } else { 0.0 };
        assert!(!walled(feet, stride, ledge(0.3)));
        assert!(walled(feet, stride, ledge(1.5)));
        // Going down is never stopped.
        assert!(!walled(feet, stride, ledge(-2.0)));
    }

    #[test]
    fn a_body_does_not_fit_under_a_low_roof() {
        let walker = Walker::default();
        let stand = Vec3::Y * 2000.0;
        // Open sky, a roof two metres up, and one a metre and a half up.
        let open = |at: Vec3| at.length() - 2000.0;
        let roof = |height: f32| move |at: Vec3| (2000.0 + height - at.length()).min(open(at));
        assert!(!cramped(stand, &walker, open));
        assert!(!cramped(stand, &walker, roof(2.1)));
        assert!(cramped(stand, &walker, roof(1.5)));
    }
}
