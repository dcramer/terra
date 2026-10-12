//! Noises heard from the state of things, so the modules that own that state need not
//! know about sound: feet on the ground, swings and blows, creatures startled or on the
//! hunt, cutting, building and stations at work, and panels opening and closing.
//!
//! Anything that walks gets `Footsteps` and is heard stepping, whatever it is.

use super::build_menu::BuildMenu;
use super::crafting::{Station, Works};
use super::creatures::{Bitten, Creature, Species, State};
use super::death::WentDown;
use super::flora::Growth;
use super::harvest::{Cutting, Felled};
use super::inventory::InventoryScreen;
use super::noise::{Cue, Feet, Noise};
use super::options::OptionsMenu;
use super::people::Person;
use super::props::Prop;
use super::tool::Builder;
use super::{Downed, Motion, Player, Stance, View};
use crate::planet::Planet;
use bevy::{platform::collections::HashMap, prelude::*};

/// Seconds between strokes of an axe or pick, and of a hammer building.
const STRIKE_EVERY_S: f32 = 0.5;
const HAMMER_EVERY_S: f32 = 0.42;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            give_feet,
            step,
            creatures_heard,
            player_heard,
            cutting_heard,
            building_heard,
            stations_heard,
            panels_heard,
        ),
    );
}

/// Who is walking, and how far since their last step.
#[derive(Component)]
pub struct Footsteps {
    walker: Walker,
    walked: f32,
    last: Option<Vec3>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Walker {
    Player,
    Person,
    Deer,
    Spider,
}

impl Walker {
    /// How far a step carries at a speed in metres a second: longer strides when
    /// running, so steps quicken less than speed does.
    fn stride_m(self, speed: f32) -> f32 {
        match self {
            Walker::Player | Walker::Person => 0.55 + 0.25 * speed,
            Walker::Deer => 0.5 + 0.2 * speed,
            Walker::Spider => 0.3 + 0.05 * speed,
        }
    }
}

fn give_feet(
    mut commands: Commands,
    players: Query<Entity, Added<Player>>,
    people: Query<Entity, Added<Person>>,
    creatures: Query<(Entity, &Creature), Added<Creature>>,
) {
    let mut give = |entity, walker| {
        commands.entity(entity).try_insert(Footsteps {
            walker,
            walked: 0.0,
            last: None,
        });
    };
    players
        .iter()
        .for_each(|entity| give(entity, Walker::Player));
    people
        .iter()
        .for_each(|entity| give(entity, Walker::Person));
    for (entity, creature) in &creatures {
        let walker = match creature.species {
            Species::Deer => Walker::Deer,
            Species::Spider => Walker::Spider,
        };
        give(entity, walker);
    }
}

/// Counts off each walker's steps by how far they move over the ground.
fn step(
    time: Res<Time>,
    planet: Res<Planet>,
    mut walkers: Query<(
        &GlobalTransform,
        &mut Footsteps,
        Option<(&Motion, Has<Downed>)>,
        Option<&Creature>,
    )>,
    mut noise: MessageWriter<Noise>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (at, mut feet, player, creature) in &mut walkers {
        let here = at.translation();
        let up = here.normalize_or(Vec3::Y);
        let moved = feet
            .last
            .map_or(0.0, |last| (here - last).reject_from(up).length());
        feet.last = Some(here);
        // Off the ground, or carried somewhere at once (loading, waking at a bed).
        let grounded = player.is_none_or(|(motion, downed)| motion.on_ground && !downed)
            && creature.is_none_or(Creature::alive);
        if !grounded || moved > 4.0 {
            feet.walked = 0.0;
            continue;
        }
        feet.walked += moved;
        let stride = feet.walker.stride_m(moved / dt);
        if feet.walked < stride {
            continue;
        }
        feet.walked -= stride;
        let surface = match (feet.walker, player) {
            (Walker::Player, Some((motion, _))) if motion.indoors => Feet::Wood,
            (Walker::Player | Walker::Person, _) if planet.under_sea(here) => Feet::Water,
            (Walker::Player | Walker::Person, _) => Feet::Grass,
            (Walker::Deer, _) => Feet::Hoof,
            (Walker::Spider, _) => Feet::Skitter,
        };
        noise.write(Noise::at(Cue::Step(surface), here));
    }
}

/// What a creature was last frame, to hear what changed.
#[derive(Component)]
struct Was {
    health: f32,
    alarmed: bool,
}

/// Blows landing, creatures dying, a deer startled, a spider turning to attack.
fn creatures_heard(
    mut commands: Commands,
    mut creatures: Query<(Entity, &Creature, &GlobalTransform, Option<&mut Was>)>,
    mut noise: MessageWriter<Noise>,
) {
    for (entity, creature, at, was) in &mut creatures {
        let alarmed = matches!(
            (creature.species, creature.state),
            (Species::Deer, State::Flee(_)) | (Species::Spider, State::Chase | State::Attack(_))
        );
        let Some(mut was) = was else {
            commands.entity(entity).try_insert(Was {
                health: creature.health,
                alarmed,
            });
            continue;
        };
        let here = at.translation() + at.translation().normalize_or(Vec3::Y) * 0.5;
        if creature.health < was.health - 0.01 {
            let cue = if creature.alive() {
                Cue::Hit
            } else {
                Cue::Kill
            };
            noise.write(Noise::at(cue, here));
        }
        if alarmed && !was.alarmed && creature.alive() {
            let cue = match creature.species {
                Species::Deer => Cue::Snort,
                Species::Spider => Cue::Hiss,
            };
            noise.write(Noise::at(cue, here));
        }
        was.health = creature.health;
        was.alarmed = alarmed;
    }
}

/// The player's swings, hurts and falls.
fn player_heard(
    player: Single<(&Stance, &GlobalTransform), With<Player>>,
    mut bitten: MessageReader<Bitten>,
    mut went_down: MessageReader<WentDown>,
    mut swinging: Local<f32>,
    mut noise: MessageWriter<Noise>,
) {
    let (stance, at) = *player;
    let chest = at.translation() + at.translation().normalize_or(Vec3::Y) * 1.2;
    // A swing sets the figure's attack going again.
    if stance.attacking > *swinging + 0.2 {
        noise.write(Noise::at(Cue::Swing, chest));
    }
    *swinging = stance.attacking;
    for _ in bitten.read() {
        noise.write(Noise::at(Cue::Hurt, chest));
    }
    for _ in went_down.read() {
        noise.write(Noise::at(Cue::Down, chest));
    }
}

/// An axe, pick or knife at work, stroke by stroke, and what it cut coming down.
fn cutting_heard(
    time: Res<Time>,
    cutting: Res<Cutting>,
    growth: Query<(&Growth, &GlobalTransform)>,
    felled: Query<(&Felled, &GlobalTransform), Added<Felled>>,
    mut beat: Local<f32>,
    mut noise: MessageWriter<Noise>,
) {
    match cutting.held().and_then(|entity| growth.get(entity).ok()) {
        Some((growth, at)) => {
            *beat += time.delta_secs();
            if *beat >= STRIKE_EVERY_S
                && let Some(cut) = growth.cut
            {
                *beat = 0.0;
                let up = at.translation().normalize_or(Vec3::Y);
                let middle = at.translation() + up * growth.middle_m;
                noise.write(Noise::at(Cue::Strike(cut.what), middle));
            }
        }
        // The first stroke lands a moment after the button goes down.
        None => *beat = STRIKE_EVERY_S - 0.15,
    }
    for (felled, at) in &felled {
        noise.write(Noise::at(
            Cue::Felled {
                tall: felled.tall(),
            },
            at.translation(),
        ));
    }
}

/// The hammer at work while something is built or taken apart.
fn building_heard(
    time: Res<Time>,
    builder: Res<Builder>,
    view: Res<View>,
    player: Single<&GlobalTransform, With<Player>>,
    mut beat: Local<f32>,
    mut noise: MessageWriter<Noise>,
) {
    if builder.progress().is_none() {
        *beat = HAMMER_EVERY_S - 0.1;
        return;
    }
    *beat += time.delta_secs();
    if *beat >= HAMMER_EVERY_S {
        *beat = 0.0;
        let feet = player.translation();
        let at = feet + view.forward * 1.5 + feet.normalize_or(Vec3::Y);
        noise.write(Noise::at(Cue::Hammer, at));
    }
}

/// Stations at work, and things made at them.
fn stations_heard(
    time: Res<Time>,
    stations: Query<(Entity, &Prop, &Works, &GlobalTransform)>,
    mut was: Local<HashMap<Entity, (f32, usize, f32)>>,
    mut noise: MessageWriter<Noise>,
) {
    let every = |station: Station| match station {
        Station::Forge => 0.8,
        _ => 0.6,
    };
    for (entity, prop, works, at) in &stations {
        let Some(station) = prop.item.station else {
            continue;
        };
        let (progress, made, beat) =
            was.entry(entity)
                .or_insert((works.progress, works.done.len(), 0.0));
        let working = !works.queue.is_empty() && works.progress != *progress;
        if working {
            *beat += time.delta_secs();
            if *beat >= every(station) {
                *beat = 0.0;
                noise.write(Noise::at(Cue::Working(station), at.translation()));
            }
        }
        if works.done.len() > *made {
            noise.write(Noise::at(Cue::Made, at.translation()));
        }
        *progress = works.progress;
        *made = works.done.len();
    }
    was.retain(|entity, _| stations.contains(*entity));
}

/// Panels opening and closing, and clicks on anything that can be clicked.
fn panels_heard(
    inventory: Res<InventoryScreen>,
    build: Res<BuildMenu>,
    options: Res<OptionsMenu>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
    mut was: Local<Option<[bool; 3]>>,
    mut noise: MessageWriter<Noise>,
) {
    let open = [inventory.open, build.open, options.open];
    let before = was.replace(open).unwrap_or(open);
    for (now, then) in open.into_iter().zip(before) {
        if now != then {
            noise.write(Noise::near(if now { Cue::Open } else { Cue::Close }));
        }
    }
    if buttons
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        noise.write(Noise::near(Cue::Click));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_quickens_steps_less_than_it_quickens_the_feet() {
        let per_second = |speed: f32| speed / Walker::Player.stride_m(speed);
        let (jog, sprint) = (per_second(4.3), per_second(7.4));
        assert!(jog > 2.0 && jog < 3.2, "{jog} steps a second jogging");
        assert!(
            sprint > jog && sprint < 3.6,
            "{sprint} steps a second sprinting"
        );
        // A spider's legs go far quicker than a person's.
        assert!(2.0 / Walker::Spider.stride_m(2.0) > 2.0 * per_second(2.0));
    }
}
