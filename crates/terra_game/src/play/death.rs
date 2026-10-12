//! Going down and coming back. There is no fail state (D18): when health runs out you
//! fall, everything in your backpack drops where you fell in a bag that lasts a while
//! (D84), and you wake at a bed in your claim, or where you first arrived if there is
//! none. What you wear stays on you. F at the bag takes back what fits.
//!
//! Death also costs a little skill progress (D101); that is in `skills`.

use super::bag::{Notice, Told};
use super::catalogue::Rest;
use super::claim::Claim;
use super::interact::{CanInteract, Focusing, Interactable, Interacted};
use super::inventory::{self, Backpack, Belongings, Items};
use super::props::Prop;
use super::vitals::{Effort, Vitals};
use super::{Downed, LocalPlayer, Motion, Player, Stance, View};
use crate::planet::Planet;
use bevy::prelude::*;
use terra_items::Grid;

/// How long you lie where you fell before waking.
const DOWN_S: f32 = 3.0;
/// How long a dropped bag lasts before it is gone (D84). A stand-in.
const BAG_LASTS_S: f32 = 600.0;

/// Where a player first arrived, to wake at when there is no bed to wake in.
#[derive(Component)]
pub struct Arrival {
    pub at: Vec3,
    pub facing: Vec3,
}

/// Things lying on the ground to be picked up: what someone carried, where they went down,
/// or what did not fit in their backpack (`pile`).
#[derive(Component)]
pub struct DroppedBag {
    pub grid: Grid,
    age: f32,
}

impl DroppedBag {
    pub fn new(grid: Grid) -> Self {
        Self { grid, age: 0.0 }
    }
}

/// Sent when a player goes down, for whatever else has to answer it.
#[derive(Message, Clone, Copy)]
pub struct WentDown(pub Entity);

pub fn plugin(app: &mut App) {
    app.add_message::<WentDown>()
        .add_systems(Update, (live, fall, (recover, decay).after(Focusing)));
}

/// Moves every player's body on by a moment, and lays it down when health runs out.
fn live(
    mut commands: Commands,
    time: Res<Time>,
    mut players: Query<(Entity, &mut Vitals, &Motion, &Stance), (With<Player>, Without<Downed>)>,
    mut went_down: MessageWriter<WentDown>,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut vitals, motion, stance) in &mut players {
        if vitals.tick(dt, effort(motion, stance)) {
            commands.entity(entity).insert(Downed { lying_s: 0.0 });
            went_down.write(WentDown(entity));
        }
    }
}

/// How hard a player is working, which is what tires and starves them.
fn effort(motion: &Motion, stance: &Stance) -> Effort {
    let speed = motion.velocity.length();
    if stance.since_acting < 0.3 {
        Effort::Working
    } else if speed > 5.6 {
        Effort::Sprinting
    } else if speed > 0.5 {
        Effort::Moving
    } else {
        Effort::Resting
    }
}

/// Drops a player's bag as they fall, and wakes them once they have lain a moment.
#[allow(clippy::too_many_arguments)]
fn fall(
    mut commands: Commands,
    time: Res<Time>,
    items: Res<Items>,
    planet: Res<Planet>,
    beds: Query<(&Prop, &Transform), Without<Player>>,
    mut players: Query<
        (
            Entity,
            &mut Transform,
            &mut Motion,
            &mut Downed,
            &mut Vitals,
            &mut Backpack,
            &Arrival,
            &Claim,
            Option<&mut View>,
            Has<LocalPlayer>,
        ),
        With<Player>,
    >,
    mut notice: ResMut<Notice>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (
        entity,
        mut transform,
        mut motion,
        mut downed,
        mut vitals,
        mut backpack,
        arrival,
        claim,
        mut view,
        ours,
    ) in &mut players
    {
        // Only what happens to this machine's own player is said on its screen.
        let mut say = |text: &str| {
            if ours {
                notice.say(text);
            }
        };
        // Just fallen: the bag drops once, however long the clock stands still.
        if downed.is_added() {
            let bag = inventory::empty_into_bag(&items, &mut backpack.0);
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
                    Transform::from_translation(at + up * 0.2).looking_to(motion.facing, up),
                ));
            }
            say("You have fallen. What you carried lies where you fell");
            info!("A player went down at {}.", transform.translation);
        }
        let lying = downed.lying_s + time.delta_secs();
        if lying < DOWN_S {
            downed.lying_s = lying;
            continue;
        }

        // Wake at a bed in the claim, or where you first arrived.
        let bed = beds
            .iter()
            .filter(|(prop, at)| {
                prop.item.rest == Some(Rest::Sleep) && claim.covers(at.translation)
            })
            .map(|(_, at)| at.translation)
            .next();
        let (at, facing) = match bed {
            Some(bed) => {
                let up = bed.normalize();
                let beside = bed + up.any_orthonormal_vector() * 1.2;
                let looking = view.as_ref().map_or(motion.facing, |view| view.forward);
                (
                    beside,
                    looking
                        .reject_from(up)
                        .normalize_or(up.any_orthonormal_vector()),
                )
            }
            None => (arrival.at, arrival.facing),
        };
        let up = at.normalize();
        // On a floor if the bed is indoors, and never below the ground.
        let feet = up * planet.ground_at(at).length().max(at.length());
        *transform = Transform::from_translation(feet).looking_to(facing, up);
        if let Some(view) = view.as_mut() {
            view.forward = facing;
        }
        motion.facing = facing;
        commands.entity(entity).remove::<Downed>();
        motion.velocity = Vec3::ZERO;
        motion.rising = 0.0;
        *vitals = Vitals::default();
        say(if bed.is_some() {
            "You wake in your bed"
        } else {
            "You wake where you first arrived"
        });
    }
}

/// F at a bag takes back what fits from it; an emptied bag is gone.
fn recover(
    mut commands: Commands,
    mut interacted: MessageReader<Interacted>,
    mut bags: Query<&mut DroppedBag>,
    mut belongings: Belongings,
    mut told: MessageWriter<Told>,
) {
    for used in interacted.read() {
        let Some(entity) = used.on(Interactable::Bag) else {
            continue;
        };
        let (Ok(mut bag), Some(mut inventory)) = (bags.get_mut(entity), belongings.of(used.player))
        else {
            continue;
        };
        let taken = inventory.take_from(&mut bag.grid);
        let said = if bag.grid.is_empty() {
            commands.entity(entity).despawn();
            format!("Picked up everything ({taken} things)")
        } else {
            format!("Picked up {taken} things; {} would not fit", bag.grid.len())
        };
        told.write(Told::new(used.player, said));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::test_player;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn a_bag_goes_to_whoever_takes_it_back() {
        use crate::play::inventory::give;
        let mut world = crate::play::test_world();
        let ground = Vec3::Y * 2000.0;
        let watches = test_player(&mut world, ground);
        let takes = test_player(&mut world, ground + Vec3::X);
        let mut grid = Grid::new(4, 4);
        world.resource_scope(|_, mut items: Mut<Items>| {
            let wood = items.kind("Wood");
            give(&mut items, &mut grid, wood, 3);
        });
        let bag = world.spawn(DroppedBag { grid, age: 0.0 }).id();
        world.write_message(Interacted {
            player: takes,
            target: bag,
            what: Interactable::Bag,
        });
        world.run_system_once(recover).unwrap();
        let wood = world.resource::<Items>().kind("Wood");
        let carried = |world: &World, player| world.get::<Backpack>(player).unwrap().0.count(wood);
        assert_eq!(carried(&world, takes), 3);
        assert_eq!(carried(&world, watches), 0);
        let told: Vec<_> = world.resource_mut::<Messages<Told>>().drain().collect();
        assert_eq!(told.len(), 1);
        assert_eq!(told[0].player, takes);
    }

    #[test]
    fn only_the_player_whose_health_runs_out_goes_down() {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Messages<WentDown>>();
        let ground = Vec3::Y * 2000.0;
        let well = test_player(&mut world, ground);
        let dying = test_player(&mut world, ground + Vec3::X);
        world.get_mut::<Vitals>(dying).unwrap().health = 0.0;
        world.run_system_once(live).unwrap();
        assert!(world.get::<Downed>(dying).is_some());
        assert!(world.get::<Downed>(well).is_none());
        let fell: Vec<_> = world
            .resource_mut::<Messages<WentDown>>()
            .drain()
            .map(|WentDown(player)| player)
            .collect();
        assert_eq!(fell, [dying]);
    }
}
