//! Combat (game design 001, combat.md, D97): real time, third person, on stamina.
//!
//! Click with anything but the hammer in hand to swing at a creature in front of you. A
//! spear hits hardest and reaches furthest; tools hit too, less well. Holding the right
//! mouse button guards, taking most of a blow on stamina; Ctrl rolls clear of one. Each
//! blow landed teaches fighting (D88), which makes the next one harder.
//!
//!   Left click    swing          Hold right mouse   guard          Ctrl   roll

use super::bag::Notice;
use super::controls::{Action, Controls};
use super::creatures::{self, Bitten, Creature};
use super::inventory::{Belongings, InventoryScreen, Tool};
use super::skills::{Did, ONCE, Skill, Skills};
use super::vitals::Vitals;
use super::{Player, View};
use bevy::prelude::*;

/// What a swing costs in stamina.
const SWING_STAMINA: f32 = 9.0;
/// Seconds between swings.
const SWING_EVERY_S: f32 = 0.65;
/// How much of a blow a guard takes, and what each guarded blow costs in stamina.
const GUARDED: f32 = 0.25;
const GUARD_STAMINA: f32 = 12.0;

/// How hard a thing in hand hits, and how far it reaches.
pub fn weapon(tool: Tool) -> (f32, f32) {
    match tool {
        Tool::Spear => (18.0, 3.2),
        Tool::IronAxe => (14.0, 2.4),
        Tool::StoneAxe => (9.0, 2.3),
        Tool::IronPickaxe => (11.0, 2.3),
        Tool::StonePickaxe => (7.0, 2.3),
        Tool::Knife => (6.0, 1.9),
        Tool::Hammer => (0.0, 0.0),
    }
}

/// When the last swing was, so another waits its turn and gathering waits for it.
#[derive(Resource)]
pub struct Swing {
    pub since: f32,
}

impl Default for Swing {
    fn default() -> Self {
        Self { since: f32::MAX }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Swing>()
        .add_systems(Update, (swing, guard, take_blows).chain());
}

/// Swings at the creature in front, if there is one within reach.
#[allow(clippy::too_many_arguments)]
fn swing(
    mut commands: Commands,
    time: Res<Time>,
    controls: Controls,
    screen: Res<InventoryScreen>,
    view: Res<View>,
    belongings: Belongings,
    player: Single<(&Transform, &mut Player, &mut Vitals, &Skills)>,
    mut creatures: Query<(Entity, &mut Creature, &Transform), Without<Player>>,
    mut swung: ResMut<Swing>,
    mut did: MessageWriter<Did>,
    mut notice: ResMut<Notice>,
) {
    swung.since += time.delta_secs();
    let (player_at, mut player, mut vitals, skills) = player.into_inner();
    let Some(tool) = belongings.holding().filter(|tool| *tool != Tool::Hammer) else {
        return;
    };
    if screen.open || player.down.is_some() || !controls.went_down(Action::Use) {
        return;
    }
    if swung.since < SWING_EVERY_S {
        return;
    }
    let feet = player_at.translation;
    let up = feet.normalize();
    let (damage, reach) = weapon(tool);
    let target = creatures
        .iter_mut()
        .filter(|(_, creature, _)| creature.alive())
        .filter_map(|(entity, creature, at)| {
            let toward = (at.translation - feet).reject_from(up);
            let distance = toward.length();
            let ahead = toward.normalize_or_zero().dot(view.forward);
            (distance < reach + 0.6 && ahead > 0.4).then_some((entity, creature, distance))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let Some((entity, mut creature, _)) = target else {
        return;
    };
    // A tired arm hits softly.
    let strength = if vitals.spend(SWING_STAMINA) {
        1.0
    } else {
        0.4
    };
    let blow = damage * strength * skills.speed(Skill::Fighting);
    creature.health -= blow;
    creature.hurt = true;
    swung.since = 0.0;
    player.since_acting = 0.0;
    player.attacking = 0.6;
    did.write(Did(Skill::Fighting, ONCE));
    if creature.health <= 0.0 {
        creatures::fallen(&mut commands, entity, &mut creature);
        did.write(Did(Skill::Fighting, ONCE * 2.0));
        info!("Killed a {:?}.", creature.species);
        let name = format!("{:?}", creature.species).to_lowercase();
        notice.say(format!("The {name} falls"));
    }
}

/// Holding the build menu's button without the hammer in hand guards.
fn guard(controls: Controls, belongings: Belongings, mut player: Single<&mut Player>) {
    let holding = belongings.holding();
    player.guarding = holding.is_some_and(|tool| tool != Tool::Hammer)
        && player.down.is_none()
        && controls.held(Action::BuildMenu);
}

/// Lands the blows creatures deal, less what a guard or a roll takes.
fn take_blows(mut bitten: MessageReader<Bitten>, player: Single<(&mut Player, &mut Vitals)>) {
    let (mut player, mut vitals) = player.into_inner();
    for &Bitten(damage) in bitten.read() {
        if player.down.is_some() || player.dodging > 0.0 {
            continue;
        }
        let taken = if player.guarding && vitals.spend(GUARD_STAMINA) {
            damage * GUARDED
        } else {
            damage
        };
        vitals.hurt(taken);
        player.flinch = 0.35;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spear_is_the_weapon_and_the_hammer_is_not() {
        let (spear, spear_reach) = weapon(Tool::Spear);
        for tool in Tool::ALL {
            let (damage, reach) = weapon(tool);
            assert!(damage <= spear && reach <= spear_reach);
        }
        assert_eq!(weapon(Tool::Hammer).0, 0.0);
        assert!(weapon(Tool::IronAxe).0 > weapon(Tool::StoneAxe).0);
    }
}
