//! The body's needs (game design 001, survival.md): health, stamina and hunger.
//!
//! Stamina pays for sprinting, jumping and working, and comes back when you stop. Hunger
//! falls all the time, faster when you work hard; low, it slows what comes back; at
//! nothing, it drains health, and starving kills (D101). Health comes back slowly while
//! you are fed. The bars show what a body would feel, nothing more (D93).
//!
//!   H             eat the most filling food you carry

use crate::inventory::Belongings;
use crate::told::Told;
use bevy::prelude::*;
use terra_walker::controls::Action;
use terra_walker::intent::Intent;
use terra_walker::{IsLocalPlayer, Player, Stamina};
use terra_world::tick::Sim;

/// Every bar is out of the same, the walker's stamina too.
pub const FULL: f32 = Stamina::FULL;

/// How hard the body is working, which decides what it spends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effort {
    Resting,
    Moving,
    Sprinting,
    Working,
}

/// Health and how fed you are, each out of [`FULL`]. Stamina is the walker's ([`Stamina`]),
/// which these fill ([`Vitals::tick`]).
#[derive(Component, Clone, Copy, PartialEq, Debug)]
#[require(Stamina)]
pub struct Vitals {
    pub health: f32,
    pub fed: f32,
    /// Seconds since the body was last in danger from the cold or the heat; it heals only
    /// once out of it (`warmth`).
    since_exposed: f32,
}

impl Default for Vitals {
    fn default() -> Self {
        Self {
            health: FULL,
            fed: 70.0,
            since_exposed: 10.0,
        }
    }
}

/// Stamina spent each second.
const SPRINT_COST: f32 = 14.0;
const WORK_COST: f32 = 4.0;
/// Stamina back each second once rested a moment, and how long that moment is.
const STAMINA_BACK: f32 = 18.0;
const STAMINA_WAIT_S: f32 = 0.8;
/// Hunger: how much is used each second at rest, and how much more hard work uses.
const HUNGER: f32 = 0.12;
const HUNGER_HARD: f32 = 0.2;
/// Below this you are hungry, and less comes back.
const HUNGRY: f32 = 30.0;
/// Health back each second while fed, and lost each second while starving.
const HEALING: f32 = 0.6;
const STARVING: f32 = 1.5;
/// Health lost each second for each degree a body is past the danger of the cold or the
/// heat: at 33 °C, hypothermic, it is down in under two minutes (D153).
const EXPOSED: f32 = 0.5;

/// Stamina coming back once rested a moment, at `slowed` of the full rate.
fn refill(stamina: &mut Stamina, dt: f32, slowed: f32) {
    if stamina.rested_s() > STAMINA_WAIT_S {
        stamina.fill(STAMINA_BACK * slowed * dt);
    }
}

/// A moment passing for a body with stamina but no needs of its own, as the sparring
/// partner: what it spent comes back as a fed body's does.
pub fn rest(stamina: &mut Stamina, dt: f32) {
    stamina.wait(dt);
    refill(stamina, dt, 1.0);
}

impl Vitals {
    /// Moves the body on by a moment of doing something, spending and filling its stamina.
    /// Returns whether it died.
    pub fn tick(&mut self, stamina: &mut Stamina, dt: f32, effort: Effort) -> bool {
        let hard = matches!(effort, Effort::Sprinting | Effort::Working);
        let cost = match effort {
            Effort::Sprinting => SPRINT_COST,
            Effort::Working => WORK_COST,
            _ => 0.0,
        };
        if cost > 0.0 && stamina.left > 0.0 {
            stamina.spend(cost * dt);
        } else {
            stamina.wait(dt);
        }
        let slowed = if self.fed < HUNGRY { 0.4 } else { 1.0 };
        refill(stamina, dt, slowed);
        self.fed = (self.fed - (HUNGER + if hard { HUNGER_HARD } else { 0.0 }) * dt).max(0.0);
        self.since_exposed += dt;
        if self.fed <= 0.0 {
            self.health -= STARVING * dt;
        } else if self.fed > HUNGRY && self.since_exposed > 1.0 {
            self.health = (self.health + HEALING * dt).min(FULL);
        }
        self.health <= 0.0
    }

    /// Takes what a moment so many degrees past the danger of the cold or the heat costs
    /// (`terra_climate::body::danger`): health, and no healing while it lasts.
    pub fn expose(&mut self, dt: f32, danger_c: f32) {
        if danger_c != 0.0 {
            self.health -= EXPOSED * danger_c.abs() * dt;
            self.since_exposed = 0.0;
        }
    }

    /// Takes a blow. Returns whether it killed.
    pub fn hurt(&mut self, amount: f32) -> bool {
        self.health -= amount;
        self.health <= 0.0
    }

    /// Eats something that fills you so much, out of [`FULL`] (its kind's `food`).
    pub fn eat(&mut self, filling: f32) {
        self.fed = (self.fed + filling).min(FULL);
    }
}

#[derive(Component)]
struct Bar(Which);

#[derive(Clone, Copy)]
enum Which {
    Health,
    Stamina,
    Fed,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(FixedUpdate, eat.in_set(Sim::Act))
        .add_systems(Update, show);
}

fn setup(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: px(24),
            bottom: px(24),
            flex_direction: FlexDirection::Column,
            row_gap: px(5),
            ..default()
        })
        .with_children(|bars| {
            // Each says what it is, so food is not taken for mana, which will be blue.
            for (which, colour, name) in [
                (Which::Health, Color::srgb(0.82, 0.25, 0.22), "Health"),
                (Which::Stamina, Color::srgb(0.95, 0.78, 0.3), "Stamina"),
                (Which::Fed, Color::srgb(0.55, 0.7, 0.3), "Food"),
            ] {
                bars.spawn((
                    Node {
                        width: px(200),
                        height: px(9),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
                ))
                .with_child((
                    Bar(which),
                    Node {
                        width: percent(100),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BackgroundColor(colour),
                ))
                .with_child((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(208),
                        top: px(-4),
                        ..default()
                    },
                    Text::new(name),
                    TextFont::from_font_size(12.0),
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
                ));
            }
        });
}

/// H eats the most filling food carried.
/// The eat key eats the most filling food a player carries, if they are hungry.
fn eat(
    mut players: Query<(Entity, &Intent, &mut Vitals), With<Player>>,
    mut belongings: Belongings,
    mut told: MessageWriter<Told>,
) {
    for (player, intent, mut vitals) in &mut players {
        if !intent.pressed(Action::Eat) {
            continue;
        }
        let Some(mut inventory) = belongings.of(player) else {
            continue;
        };
        let said = if vitals.fed >= FULL - 1.0 {
            "You are not hungry".to_string()
        } else if let Some(food) = inventory.eat_best() {
            let items = inventory.items();
            vitals.eat(items.food(food));
            format!("Ate {}", items.name(food).to_lowercase())
        } else {
            "You have nothing to eat".to_string()
        };
        told.write(Told::new(player, said));
    }
}

fn show(
    vitals: Single<(&Vitals, &Stamina), IsLocalPlayer>,
    options: Res<terra_world::shell::Options>,
    mut bars: Query<(&Bar, &mut Node, &ChildOf)>,
    mut frames: Query<&mut Visibility, Without<Bar>>,
) {
    let (vitals, stamina) = *vitals;
    for (bar, mut node, parent) in &mut bars {
        let value = match bar.0 {
            Which::Health => vitals.health,
            Which::Stamina => stamina.left,
            Which::Fed => vitals.fed,
        };
        let width = percent((value / FULL * 100.0).clamp(0.0, 100.0));
        if node.width != width {
            node.width = width;
        }
        if let Ok(mut visibility) = frames.get_mut(parent.parent()) {
            let wanted = if options.shows_interface() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *visibility != wanted {
                *visibility = wanted;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{Backpack, Items, give};
    use crate::{test_player, test_world};
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn only_the_player_who_means_to_eat_eats() {
        let mut world = test_world();
        let ground = Vec3::Y * 2000.0;
        let hungry = |world: &mut World, at| {
            let player = test_player(world, at);
            world.get_mut::<Vitals>(player).unwrap().fed = 20.0;
            world.resource_scope(|world, mut items: Mut<Items>| {
                let berries = items.kind("Berries");
                let mut backpack = world.get_mut::<Backpack>(player).unwrap();
                give(&mut items, &mut backpack.0, berries, 2);
            });
            player
        };
        let eats = hungry(&mut world, ground);
        let waits = hungry(&mut world, ground + Vec3::X * 3.0);
        world.get_mut::<Intent>(eats).unwrap().hold(Action::Eat);
        world.run_system_once(eat).unwrap();
        let fed = |world: &World, player| world.get::<Vitals>(player).unwrap().fed;
        assert!(fed(&world, eats) > 20.0);
        assert_eq!(fed(&world, waits), 20.0);
        let told: Vec<_> = world.resource_mut::<Messages<Told>>().drain().collect();
        assert_eq!(told.len(), 1);
        assert_eq!(told[0].player, eats);
    }

    #[test]
    fn a_body_without_needs_gets_its_stamina_back_after_a_moment() {
        let mut stamina = Stamina::default();
        stamina.spend(60.0);
        rest(&mut stamina, STAMINA_WAIT_S * 0.5);
        assert_eq!(
            stamina.left,
            FULL - 60.0,
            "not before it has rested a moment"
        );
        for _ in 0..100 {
            rest(&mut stamina, 0.1);
        }
        assert_eq!(stamina.left, FULL);
    }

    #[test]
    fn sprinting_spends_stamina_and_resting_brings_it_back() {
        let (mut vitals, mut stamina) = (Vitals::default(), Stamina::default());
        for _ in 0..60 {
            vitals.tick(&mut stamina, 0.1, Effort::Sprinting);
        }
        assert!(stamina.left < FULL * 0.3);
        let tired = stamina.left;
        for _ in 0..60 {
            vitals.tick(&mut stamina, 0.1, Effort::Resting);
        }
        assert!(stamina.left > tired + 50.0);
    }

    #[test]
    fn hunger_falls_and_starving_kills() {
        let mut vitals = Vitals::default();
        let mut died = false;
        for _ in 0..20_000 {
            died |= vitals.tick(&mut Stamina::default(), 0.1, Effort::Resting);
        }
        assert!(died, "starving kills in the end (D101)");
        assert_eq!(vitals.fed, 0.0);
    }

    #[test]
    fn being_fed_heals_and_better_food_fills_more() {
        let mut vitals = Vitals::default();
        vitals.hurt(50.0);
        for _ in 0..100 {
            vitals.tick(&mut Stamina::default(), 0.1, Effort::Resting);
        }
        assert!(vitals.health > 50.0);
        let items = crate::inventory::Items::default();
        let food = |name| items.food(items.kind(name));
        assert!(food("Cooked meat") > food("Meat"));
        assert_eq!(food("Wood"), 0.0);
    }

    #[test]
    fn the_cold_wears_health_down_and_no_one_heals_in_it() {
        let mut vitals = Vitals::default();
        vitals.hurt(20.0);
        let mut died = false;
        for _ in 0..100 {
            vitals.expose(0.1, -2.0);
            died |= vitals.tick(&mut Stamina::default(), 0.1, Effort::Resting);
        }
        assert!((vitals.health - 70.0).abs() < 0.01, "{}", vitals.health);
        assert!(!died);
        for _ in 0..2000 {
            vitals.expose(0.1, -2.0);
            died |= vitals.tick(&mut Stamina::default(), 0.1, Effort::Resting);
        }
        assert!(died, "hypothermia kills in the end");
        // Out of danger, it heals again.
        let mut warm = Vitals::default();
        warm.hurt(20.0);
        warm.expose(0.1, 0.0);
        for _ in 0..30 {
            warm.tick(&mut Stamina::default(), 0.1, Effort::Resting);
        }
        assert!(warm.health > 80.0);
    }

    #[test]
    fn hungry_bodies_recover_stamina_slowly() {
        let mut fed = (Vitals::default(), Stamina::default());
        let mut hungry = (
            Vitals {
                fed: 10.0,
                ..Vitals::default()
            },
            Stamina::default(),
        );
        for (vitals, stamina) in [&mut fed, &mut hungry] {
            stamina.spend(80.0);
            for _ in 0..20 {
                vitals.tick(stamina, 0.1, Effort::Resting);
            }
        }
        assert!(fed.1.left > hungry.1.left);
    }
}
