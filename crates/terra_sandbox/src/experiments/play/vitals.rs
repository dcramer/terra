//! The body's needs (game design 001, survival.md): health, stamina and hunger.
//!
//! Stamina pays for sprinting, jumping and working, and comes back when you stop. Hunger
//! falls all the time, faster when you work hard; low, it slows what comes back; at
//! nothing, it drains health, and starving kills (D101). Health comes back slowly while
//! you are fed. The bars show what a body would feel, nothing more (D93).
//!
//!   H             eat the most filling food you carry

use super::Player;
use super::bag::Notice;
use super::controls::{Action, Controls};
use super::inventory::{Belongings, Raw};
use bevy::prelude::*;

pub const FULL: f32 = 100.0;

/// How hard the body is working, which decides what it spends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effort {
    Resting,
    Moving,
    Sprinting,
    Working,
}

/// Health, stamina and how fed you are, each out of [`FULL`].
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Vitals {
    pub health: f32,
    pub stamina: f32,
    pub fed: f32,
    /// Seconds since stamina was last spent; it waits a moment before coming back.
    since_spent: f32,
}

impl Default for Vitals {
    fn default() -> Self {
        Self {
            health: FULL,
            stamina: FULL,
            fed: 70.0,
            since_spent: 10.0,
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

impl Vitals {
    /// Moves the body on by a moment of doing something. Returns whether it died.
    pub fn tick(&mut self, dt: f32, effort: Effort) -> bool {
        let hard = matches!(effort, Effort::Sprinting | Effort::Working);
        let cost = match effort {
            Effort::Sprinting => SPRINT_COST,
            Effort::Working => WORK_COST,
            _ => 0.0,
        };
        if cost > 0.0 && self.stamina > 0.0 {
            self.spend(cost * dt);
        } else {
            self.since_spent += dt;
        }
        let slowed = if self.fed < HUNGRY { 0.4 } else { 1.0 };
        if self.since_spent > STAMINA_WAIT_S {
            self.stamina = (self.stamina + STAMINA_BACK * slowed * dt).min(FULL);
        }
        self.fed = (self.fed - (HUNGER + if hard { HUNGER_HARD } else { 0.0 }) * dt).max(0.0);
        if self.fed <= 0.0 {
            self.health -= STARVING * dt;
        } else if self.fed > HUNGRY {
            self.health = (self.health + HEALING * dt).min(FULL);
        }
        self.health <= 0.0
    }

    /// Spends stamina, if there is any to spend. Returns whether there was.
    pub fn spend(&mut self, amount: f32) -> bool {
        if self.stamina <= 0.0 {
            return false;
        }
        self.stamina = (self.stamina - amount).max(0.0);
        self.since_spent = 0.0;
        true
    }

    pub fn can_sprint(&self) -> bool {
        self.stamina > 0.0
    }

    /// Takes a blow. Returns whether it killed.
    pub fn hurt(&mut self, amount: f32) -> bool {
        self.health -= amount;
        self.health <= 0.0
    }

    pub fn eat(&mut self, food: Raw) {
        self.fed = (self.fed + nourishment(food)).min(FULL);
    }
}

/// How filling a food is, out of [`FULL`]; nothing for what is not food.
pub fn nourishment(raw: Raw) -> f32 {
    match raw {
        Raw::Berries => 8.0,
        Raw::Meat => 15.0,
        Raw::CookedMeat => 40.0,
        _ => 0.0,
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
        .add_systems(Update, (eat, show));
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
            for (which, colour) in [
                (Which::Health, Color::srgb(0.82, 0.25, 0.22)),
                (Which::Stamina, Color::srgb(0.95, 0.78, 0.3)),
                (Which::Fed, Color::srgb(0.55, 0.7, 0.3)),
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
                ));
            }
        });
}

/// H eats the most filling food carried.
fn eat(
    controls: Controls,
    mut vitals: Single<&mut Vitals, With<Player>>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
) {
    if !controls.went_down(Action::Eat) {
        return;
    }
    if vitals.fed >= FULL - 1.0 {
        notice.say("You are not hungry");
        return;
    }
    match belongings.eat_best() {
        Some(food) => {
            vitals.eat(food);
            notice.say(format!("Ate {}", food.name().to_lowercase()));
        }
        None => notice.say("You have nothing to eat"),
    }
}

fn show(
    vitals: Single<&Vitals, With<Player>>,
    options: Res<crate::shell::Options>,
    mut bars: Query<(&Bar, &mut Node, &ChildOf)>,
    mut frames: Query<&mut Visibility, Without<Bar>>,
) {
    for (bar, mut node, parent) in &mut bars {
        let value = match bar.0 {
            Which::Health => vitals.health,
            Which::Stamina => vitals.stamina,
            Which::Fed => vitals.fed,
        };
        let width = percent((value / FULL * 100.0).clamp(0.0, 100.0));
        if node.width != width {
            node.width = width;
        }
        if let Ok(mut visibility) = frames.get_mut(parent.parent()) {
            let wanted = if options.vantage.is_none() {
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

    #[test]
    fn sprinting_spends_stamina_and_resting_brings_it_back() {
        let mut vitals = Vitals::default();
        for _ in 0..60 {
            vitals.tick(0.1, Effort::Sprinting);
        }
        assert!(vitals.stamina < FULL * 0.3);
        let tired = vitals.stamina;
        for _ in 0..60 {
            vitals.tick(0.1, Effort::Resting);
        }
        assert!(vitals.stamina > tired + 50.0);
    }

    #[test]
    fn hunger_falls_and_starving_kills() {
        let mut vitals = Vitals::default();
        let mut died = false;
        for _ in 0..20_000 {
            died |= vitals.tick(0.1, Effort::Resting);
        }
        assert!(died, "starving kills in the end (D101)");
        assert_eq!(vitals.fed, 0.0);
    }

    #[test]
    fn being_fed_heals_and_better_food_fills_more() {
        let mut vitals = Vitals::default();
        vitals.hurt(50.0);
        for _ in 0..100 {
            vitals.tick(0.1, Effort::Resting);
        }
        assert!(vitals.health > 50.0);
        assert!(nourishment(Raw::CookedMeat) > nourishment(Raw::Meat));
        assert_eq!(nourishment(Raw::Wood), 0.0);
    }

    #[test]
    fn hungry_bodies_recover_stamina_slowly() {
        let mut fed = Vitals::default();
        let mut hungry = Vitals {
            fed: 10.0,
            ..Vitals::default()
        };
        for vitals in [&mut fed, &mut hungry] {
            vitals.spend(80.0);
            for _ in 0..20 {
                vitals.tick(0.1, Effort::Resting);
            }
        }
        assert!(fed.stamina > hungry.stamina);
    }
}
