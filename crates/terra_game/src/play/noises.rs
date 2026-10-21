//! Noises heard from the state of things, so the modules that own that state need not
//! know about sound: feet on the ground, swings and blows, creatures startled or on the
//! hunt, cutting, building and stations at work, and panels opening and closing.
//!
//! Anything that walks gets `Footsteps` and is heard stepping, whatever it is.

use super::build_menu::BuildMenu;
use super::button::left_click;
use super::crafting::{Stations, Works};
use super::creatures::{Bitten, Creature, State};
use super::death::WentDown;
use super::flora::Growth;
use super::harvest::{Cutting, Felled};
use super::inventory::InventoryScreen;
use super::noise::{Cue, Feet, Noise};
use super::options::OptionsMenu;
use super::people::Person;
use super::props::Prop;
use super::species::{Bestiary, Voice};
use super::tool::Builder;
use super::{Downed, IsLocalPlayer, Motion, Player, Stance, View};
use crate::planet::Planet;
use bevy::{platform::collections::HashMap, prelude::*, ui::Pressed, ui_widgets};

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

#[derive(Clone, Copy, PartialEq, Debug)]
enum Walker {
    Player,
    Person,
    /// A creature, stepping as its kind says (`assets/data/creatures.ron`): how far a step
    /// carries, as so far and so much more for each metre a second, and on what feet.
    Creature {
        stride: (f32, f32),
        feet: Feet,
    },
}

impl Walker {
    /// How far a step carries at a speed in metres a second: longer strides when
    /// running, so steps quicken less than speed does.
    fn stride_m(self, speed: f32) -> f32 {
        match self {
            Walker::Player | Walker::Person => 0.55 + 0.25 * speed,
            Walker::Creature {
                stride: (least, per_mps),
                ..
            } => least + per_mps * speed,
        }
    }
}

fn give_feet(
    mut commands: Commands,
    players: Query<Entity, Added<Player>>,
    people: Query<Entity, Added<Person>>,
    creatures: Query<(Entity, &Creature), Added<Creature>>,
    bestiary: Res<Bestiary>,
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
        let about = bestiary.get(creature.species);
        let (stride, feet) = (about.stride, about.feet);
        give(entity, Walker::Creature { stride, feet });
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
            (Walker::Creature { feet, .. }, _) => feet,
        };
        noise.write(Noise::at(Cue::Step(surface), here));
    }
}

/// What a creature was last frame, to hear what changed.
#[derive(Component)]
struct Was {
    health: f32,
    alarmed: bool,
    flying: bool,
    /// Until it next calls of its own accord, seconds, for a kind that does.
    calls_in: f32,
}

/// Blows landing, creatures dying, a deer startled, a spider turning to attack.
fn creatures_heard(
    mut commands: Commands,
    time: Res<Time>,
    mut creatures: Query<(Entity, &Creature, &GlobalTransform, Option<&mut Was>)>,
    bestiary: Res<Bestiary>,
    mut noise: MessageWriter<Noise>,
    mut dice: Local<Option<terra_gen::Rng>>,
) {
    let dice = dice.get_or_insert_with(|| terra_gen::Seed(23).derive("calls").rng());
    for (entity, creature, at, was) in &mut creatures {
        // Running from someone, or going for them, or up a tree out of reach.
        let alarmed = matches!(
            creature.state,
            State::Flee(_) | State::Chase | State::Attack(_) | State::Climb { .. }
        );
        let flying = matches!(creature.state, State::Fly { .. });
        let Some(mut was) = was else {
            commands.entity(entity).try_insert(Was {
                health: creature.health,
                alarmed,
                flying,
                calls_in: dice.range(0.0, 20.0),
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
        let voice = match bestiary.get(creature.species).voice {
            Voice::Snort => Some(Cue::Snort),
            Voice::Hiss => Some(Cue::Hiss),
            Voice::Chatter => Some(Cue::Chatter),
            Voice::Chirp => Some(Cue::Chirp),
            Voice::Quack => Some(Cue::Quack),
            Voice::Croak => Some(Cue::Croak),
            Voice::Bark => Some(Cue::Bark),
            Voice::Silent => None,
        };
        // A call as it takes fright, and wings beating as it takes off.
        let fled = (alarmed && !was.alarmed) || (flying && !was.flying);
        if fled
            && creature.alive()
            && let Some(cue) = voice
        {
            noise.write(Noise::at(cue, here));
        }
        if flying && !was.flying {
            noise.write(Noise::at(Cue::Wings, here));
        }
        // Now and then a call of its own at rest: a duck on the water, a heron, a grouse.
        if let Some((least, most)) = bestiary.get(creature.species).calls_s
            && creature.alive()
            && !alarmed
        {
            was.calls_in -= time.delta_secs();
            if was.calls_in <= 0.0 {
                was.calls_in = dice.range(least, most);
                if let Some(cue) = voice {
                    noise.write(Noise::at(cue, here));
                }
            }
        }
        was.health = creature.health;
        was.alarmed = alarmed;
        was.flying = flying;
    }
}

/// Players' swings, hurts and falls, each heard from where it happened.
fn player_heard(
    players: Query<(Entity, &Stance, &GlobalTransform), With<Player>>,
    mut bitten: MessageReader<Bitten>,
    mut went_down: MessageReader<WentDown>,
    mut swinging: Local<HashMap<Entity, f32>>,
    mut noise: MessageWriter<Noise>,
) {
    let chest =
        |at: &GlobalTransform| at.translation() + at.translation().normalize_or(Vec3::Y) * 1.2;
    swinging.retain(|player, _| players.contains(*player));
    for (player, stance, at) in &players {
        // A swing sets the figure's attack going again.
        let swung = swinging.entry(player).or_default();
        if stance.attacking > *swung + 0.2 {
            noise.write(Noise::at(Cue::Swing, chest(at)));
        }
        *swung = stance.attacking;
    }
    let mut heard = |player: Entity, cue: Cue| {
        if let Ok((_, _, at)) = players.get(player) {
            noise.write(Noise::at(cue, chest(at)));
        }
    };
    for &Bitten { player, .. } in bitten.read() {
        heard(player, Cue::Hurt);
    }
    for &WentDown(player) in went_down.read() {
        heard(player, Cue::Down);
    }
}

/// An axe, pick or knife at work, stroke by stroke, and what it cut coming down.
fn cutting_heard(
    time: Res<Time>,
    cutting: Single<&Cutting, IsLocalPlayer>,
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
    builder: Single<&Builder, IsLocalPlayer>,
    view: Single<&View, IsLocalPlayer>,
    player: Single<&GlobalTransform, IsLocalPlayer>,
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
    kinds: Res<Stations>,
    mut was: Local<HashMap<Entity, (f32, usize, f32)>>,
    mut noise: MessageWriter<Noise>,
) {
    // Each stroke of work, as often as the station says, or of no use to hear.
    let every = |station| kinds.strokes(station).map_or(0.6, |(_, every)| every);
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
    mouse: Res<ButtonInput<MouseButton>>,
    pressed: Query<(), (Added<Pressed>, With<ui_widgets::Button>)>,
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
    if left_click(&mouse) && !pressed.is_empty() {
        noise.write(Noise::near(Cue::Click));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_walker::movement::{JOG_SPEED, SPRINT_SPEED};

    #[test]
    fn running_quickens_steps_less_than_it_quickens_the_feet() {
        let per_second = |speed: f32| speed / Walker::Player.stride_m(speed);
        let (jog, sprint) = (per_second(JOG_SPEED), per_second(SPRINT_SPEED));
        // A runner takes between two and three and a half steps a second.
        assert!(jog > 2.0 && jog < 3.2, "{jog} steps a second jogging");
        assert!(
            sprint > jog && sprint < 3.6,
            "{sprint} steps a second sprinting"
        );
        assert!(sprint / jog < SPRINT_SPEED / JOG_SPEED);
        // A spider's legs go far quicker than a person's.
        let bestiary = Bestiary::read(&Default::default());
        let spider = bestiary.get(bestiary.named("Spider").unwrap());
        let spider = Walker::Creature {
            stride: spider.stride,
            feet: spider.feet,
        };
        assert!(2.0 / spider.stride_m(2.0) > 2.0 * per_second(2.0));
    }
}
