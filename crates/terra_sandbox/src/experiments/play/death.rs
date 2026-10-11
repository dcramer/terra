//! Going down and coming back. There is no fail state (D18): when health runs out you
//! fall, everything in your backpack drops where you fell in a bag that lasts a while
//! (D84), and you wake at a bed in your claim, or where you first arrived if there is
//! none. What you wear stays on you. F at the bag takes back what fits.
//!
//! Death also costs a little skill progress (D101); that is in `skills`.

use super::bag::Notice;
use super::claim::Claim;
use super::interact::{CanInteract, Focus, Focusing, Interactable};
use super::inventory::Belongings;
use super::props::Prop;
use super::vitals::{Effort, Vitals};
use super::{Player, View};
use crate::planet::Planet;
use bevy::prelude::*;
use terra_items::Grid;

/// How long you lie where you fell before waking.
const DOWN_S: f32 = 3.0;
/// How long a dropped bag lasts before it is gone (D84). A stand-in.
const BAG_LASTS_S: f32 = 600.0;

/// Where a player first arrived, to wake at when there is no bed to wake in.
#[derive(Resource)]
pub struct Arrival {
    pub at: Vec3,
    pub facing: Vec3,
}

/// What someone carried, where they went down.
#[derive(Component)]
pub struct DroppedBag {
    pub grid: Grid,
    age: f32,
}

/// Sent when the player goes down, for whatever else has to answer it.
#[derive(Message, Clone, Copy)]
pub struct WentDown;

pub fn plugin(app: &mut App) {
    app.add_message::<WentDown>()
        .add_systems(Update, (live, fall, (recover, decay).after(Focusing)));
}

/// Moves the body on by a moment, and lays it down when health runs out.
fn live(
    time: Res<Time>,
    player: Single<(&mut Vitals, &mut Player)>,
    mut went_down: MessageWriter<WentDown>,
) {
    let (mut vitals, mut player) = player.into_inner();
    if player.down.is_some() {
        return;
    }
    let dt = time.delta_secs().min(0.1);
    let speed = player.velocity.length();
    let effort = if player.since_acting < 0.3 {
        Effort::Working
    } else if speed > 5.6 {
        Effort::Sprinting
    } else if speed > 0.5 {
        Effort::Moving
    } else {
        Effort::Resting
    };
    if vitals.tick(dt, effort) {
        player.down = Some(0.0);
        went_down.write(WentDown);
    }
}

/// Drops the bag as the player falls, and wakes them once they have lain a moment.
#[allow(clippy::too_many_arguments)]
fn fall(
    mut commands: Commands,
    time: Res<Time>,
    arrival: Res<Arrival>,
    claim: Res<Claim>,
    planet: Res<Planet>,
    beds: Query<(&Prop, &Transform), Without<Player>>,
    player: Single<(&mut Transform, &mut Player, &mut Vitals)>,
    mut view: ResMut<View>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (mut transform, mut player, mut vitals) = player.into_inner();
    let Some(lying) = player.down else { return };
    if lying == 0.0 {
        let bag = belongings.empty_into_bag();
        if !bag.is_empty() {
            let at = transform.translation;
            let up = at.normalize();
            commands.spawn((
                Name::new("Dropped bag"),
                DroppedBag {
                    grid: bag,
                    age: 0.0,
                },
                CanInteract(Interactable::Bag),
                Mesh3d(meshes.add(Cuboid::new(0.55, 0.4, 0.45))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.55, 0.38, 0.24),
                    perceptual_roughness: 0.9,
                    ..default()
                })),
                Transform::from_translation(at + up * 0.2).looking_to(player.facing, up),
            ));
        }
        notice.say("You have fallen. What you carried lies where you fell");
        info!("The player went down at {}.", transform.translation);
    }
    let lying = lying + time.delta_secs();
    if lying < DOWN_S {
        player.down = Some(lying);
        return;
    }

    // Wake at a bed in the claim, or where you first arrived.
    let bed = beds
        .iter()
        .filter(|(prop, at)| prop.item.file == "Bed_Single" && claim.covers(at.translation))
        .map(|(_, at)| at.translation)
        .next();
    let (at, facing) = match bed {
        Some(bed) => {
            let up = bed.normalize();
            let beside = bed + up.any_orthonormal_vector() * 1.2;
            (
                beside,
                view.forward
                    .reject_from(up)
                    .normalize_or(up.any_orthonormal_vector()),
            )
        }
        None => (arrival.at, arrival.facing),
    };
    let up = at.normalize();
    // On a floor if the bed is indoors, and never below the ground.
    let feet = up * planet.drawn(up).length().max(at.length());
    *transform = Transform::from_translation(feet).looking_to(facing, up);
    view.forward = facing;
    player.down = None;
    player.velocity = Vec3::ZERO;
    player.rising = 0.0;
    *vitals = Vitals::default();
    notice.say(if bed.is_some() {
        "You wake in your bed"
    } else {
        "You wake where you first arrived"
    });
}

/// F at a bag takes back what fits from it; an emptied bag is gone.
fn recover(
    mut commands: Commands,
    focus: Res<Focus>,
    mut bags: Query<&mut DroppedBag>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
) {
    if !focus.used(Interactable::Bag) {
        return;
    }
    let Some(entity) = focus.entity else { return };
    let Ok(mut bag) = bags.get_mut(entity) else {
        return;
    };
    let taken = belongings.take_from(&mut bag.grid);
    if bag.grid.is_empty() {
        commands.entity(entity).despawn();
        notice.say(format!("Took back everything ({taken} things)"));
    } else {
        notice.say(format!(
            "Took back {taken} things; {} would not fit",
            bag.grid.len()
        ));
    }
}

/// Bags left too long are gone, so the world does not fill with them.
fn decay(mut commands: Commands, time: Res<Time>, mut bags: Query<(Entity, &mut DroppedBag)>) {
    for (entity, mut bag) in &mut bags {
        bag.age += time.delta_secs();
        if bag.age > BAG_LASTS_S {
            commands.entity(entity).despawn();
        }
    }
}
