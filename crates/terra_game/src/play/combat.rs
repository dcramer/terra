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
use super::creatures::{self, Bitten, Creature, Species};
use super::inventory::{self, Belongings, Broke, Gear, InventoryScreen, Items};
use super::kinds::ToolDef;
use super::skills::{Did, ONCE, Skill, Skills};
use super::species::Part;
use super::vitals::Vitals;
use super::zones::Zones;
use super::{Downed, IsLocalPlayer, LocalPlayer, Player, Stance, View};
use crate::planet::MainCamera;
use bevy::prelude::*;

/// What a swing costs in stamina.
const SWING_STAMINA: f32 = 9.0;
/// Seconds between swings.
const SWING_EVERY_S: f32 = 0.65;
/// How much of a blow a guard takes, and what each guarded blow costs in stamina.
const GUARDED: f32 = 0.25;
const GUARD_STAMINA: f32 = 12.0;
/// How much more a blow does where it lands (D119): a head takes twice a body's, legs a
/// little over half. Proposed numbers.
const HEAD: f32 = 2.0;
const LEGS: f32 = 0.6;
/// How much more a blow does that its target did not see coming: a critical by timing,
/// not by dice (D119).
const AMBUSH: f32 = 1.5;

/// How much a blow on a part counts for.
fn counts(part: Part) -> f32 {
    match part {
        Part::Head => HEAD,
        Part::Body => 1.0,
        Part::Legs => LEGS,
    }
}

/// A blow, however it was dealt: how hard it would hit a body, where it landed, on what
/// part, and which skill dealt it.
#[derive(Clone, Copy, Debug)]
pub struct Blow {
    pub damage: f32,
    pub at: Vec3,
    pub part: Part,
    pub skill: Skill,
}

/// When a player last swung, so another waits its turn and gathering waits for it.
#[derive(Component)]
pub struct Swing {
    pub since: f32,
}

impl Default for Swing {
    fn default() -> Self {
        Self { since: f32::MAX }
    }
}

/// A blow landed on a creature: which, where on it, and whether it fell.
#[derive(Message, Clone, Copy, Debug)]
pub struct Struck {
    pub creature: Entity,
    pub species: Species,
    pub at: Vec3,
    /// A head, or a blow it did not see coming: one that counts for more (D119).
    pub critical: bool,
    pub killed: bool,
}

/// A player took a blow, of so much health.
#[derive(Message, Clone, Copy, Debug)]
pub struct Hurt {
    pub player: Entity,
    pub amount: f32,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Struck>()
        .add_message::<Hurt>()
        .register_required_components::<Player, Swing>()
        .add_systems(Update, (swing, guard, take_blows).chain());
}

/// Swings at the creature in front, if there is one within reach.
#[allow(clippy::too_many_arguments)]
fn swing(
    mut commands: Commands,
    time: Res<Time>,
    controls: Controls,
    screen: Res<InventoryScreen>,
    view: Single<&View, IsLocalPlayer>,
    mut belongings: Belongings,
    player: Single<(&Transform, &mut Stance, &mut Vitals, &Skills, Has<Downed>), IsLocalPlayer>,
    mut creatures: Query<(Entity, &mut Creature, &Transform), Without<Player>>,
    mut swung: Single<&mut Swing, IsLocalPlayer>,
    mut did: MessageWriter<Did>,
    mut struck: MessageWriter<Struck>,
    mut notice: ResMut<Notice>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    zones: Zones,
) {
    swung.since += time.delta_secs();
    let (player_at, mut stance, mut vitals, skills, downed) = player.into_inner();
    // Whatever in hand strikes at all: a weapon, or a tool used as one.
    let Some((damage, reach)) = belongings.tool_in_hand().and_then(ToolDef::strikes) else {
        return;
    };
    if screen.open || downed || !controls.went_down(Action::Use) {
        return;
    }
    if swung.since < SWING_EVERY_S {
        return;
    }
    let feet = player_at.translation;
    let up = feet.normalize();
    let target = creatures
        .iter_mut()
        .filter(|(_, creature, _)| creature.alive())
        .filter_map(|(entity, creature, at)| {
            let toward = (at.translation - feet).reject_from(up);
            let distance = toward.length();
            let ahead = toward.normalize_or_zero().dot(view.forward);
            (distance < reach + 0.6 && ahead > 0.4).then_some((
                entity,
                creature,
                distance,
                at.translation,
            ))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let Some((entity, mut creature, _, there)) = target else {
        return;
    };
    // A tired arm hits softly.
    let strength = if vitals.spend(SWING_STAMINA) {
        1.0
    } else {
        0.4
    };
    let blow = damage * strength * skills.speed(Skill::Fighting);
    swung.since = 0.0;
    stance.since_acting = 0.0;
    stance.attacking = 0.6;
    // It lands where the aim is, if that is on it; otherwise on its body, on the side
    // toward the player.
    let eye = camera.translation();
    let ray = eye + *camera.forward() * (eye.distance(there) + reach + 1.0);
    let (at, part) = match zones.along(entity, eye, ray) {
        Some((along, part)) => (eye.lerp(ray, along), part),
        None => (
            there + (feet - there).reject_from(up).normalize_or_zero() * 0.3 + up * 0.5,
            Part::Body,
        ),
    };
    strike(
        &mut commands,
        entity,
        &mut creature,
        Blow {
            damage: blow,
            at,
            part,
            skill: Skill::Fighting,
        },
        &mut did,
        &mut struck,
        &mut notice,
    );
    belongings.wear_in_hand(1);
}

/// Lands a blow on a creature, however it was dealt: a swing or an arrow, each growing its
/// own skill. Where it lands, and whether the creature saw it coming, decide how much it
/// does (D119). It learns it was hurt, and falls if that was enough. Returns how much
/// the blow did.
pub fn strike(
    commands: &mut Commands,
    entity: Entity,
    creature: &mut Creature,
    blow: Blow,
    did: &mut MessageWriter<Did>,
    struck: &mut MessageWriter<Struck>,
    notice: &mut Notice,
) -> f32 {
    let ambush = creature.unaware();
    let dealt = blow.damage * counts(blow.part) * if ambush { AMBUSH } else { 1.0 };
    creature.health -= dealt;
    creature.hurt = true;
    did.write(Did(blow.skill, ONCE));
    struck.write(Struck {
        creature: entity,
        species: creature.species,
        at: blow.at,
        critical: ambush || blow.part == Part::Head,
        killed: creature.health <= 0.0,
    });
    info!(
        "Struck a {} in the {}{} for {dealt:.0}.",
        creature.species,
        blow.part.name(),
        if ambush { ", unaware" } else { "" }
    );
    let skill = blow.skill;
    if creature.health <= 0.0 {
        creatures::fallen(commands, entity, creature);
        did.write(Did(skill, ONCE * 2.0));
        info!("Killed a {}.", creature.species);
        let name = creature.species.name().to_lowercase();
        notice.say(format!("The {name} falls"));
    }
    dealt
}

/// Holding the build menu's button without the hammer in hand guards.
fn guard(
    controls: Controls,
    belongings: Belongings,
    player: Single<(&mut Stance, Has<Downed>), IsLocalPlayer>,
) {
    let (mut stance, downed) = player.into_inner();
    let holding = belongings.tool_in_hand();
    stance.guarding =
        holding.is_some_and(|tool| !tool.builds) && !downed && controls.held(Action::BuildMenu);
}

/// Lands the blows creatures deal on whoever they hit, less what leather, a guard or a
/// roll takes.
fn take_blows(
    mut bitten: MessageReader<Bitten>,
    items: Res<Items>,
    mut players: Query<
        (
            &mut Stance,
            &mut Vitals,
            &mut Gear,
            Has<Downed>,
            Has<LocalPlayer>,
        ),
        With<Player>,
    >,
    mut hurt: MessageWriter<Hurt>,
    mut broke: MessageWriter<Broke>,
) {
    for &Bitten { player, damage } in bitten.read() {
        let Ok((mut stance, mut vitals, mut gear, downed, ours)) = players.get_mut(player) else {
            continue;
        };
        if downed || stance.dodging > 0.0 {
            continue;
        }
        // Leather takes some of it.
        let damage = damage * (1.0 - inventory::protection(&items, &gear.0));
        let taken = if stance.guarding && vitals.spend(GUARD_STAMINA) {
            damage * GUARDED
        } else {
            damage
        };
        vitals.hurt(taken);
        hurt.write(Hurt {
            player,
            amount: taken,
        });
        // What is worn takes the blow too, and wears. Only this machine's player hears of it.
        for kind in inventory::wear_worn(&items, &mut gear.0, 1) {
            if ours {
                broke.write(Broke { kind });
            }
        }
        stance.flinch = 0.35;
        info!("Bitten for {taken:.0}; health {:.0}.", vitals.health);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::test_player;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn a_blow_hurts_only_the_player_it_lands_on() {
        let mut world = World::new();
        world.init_resource::<Messages<Bitten>>();
        world.init_resource::<Messages<Hurt>>();
        world.init_resource::<Messages<Broke>>();
        let ground = Vec3::Y * 2000.0;
        let spared = test_player(&mut world, ground);
        let bitten = test_player(&mut world, ground + Vec3::X * 3.0);
        world.write_message(Bitten {
            player: bitten,
            damage: 9.0,
        });
        world.run_system_once(take_blows).unwrap();
        let health = |world: &World, player| world.get::<Vitals>(player).unwrap().health;
        assert_eq!(health(&world, spared), Vitals::default().health);
        assert!(health(&world, bitten) < Vitals::default().health);
        let hurt: Vec<_> = world.resource_mut::<Messages<Hurt>>().drain().collect();
        assert_eq!(hurt.len(), 1);
        assert_eq!(hurt[0].player, bitten);
    }

    #[test]
    fn the_spear_is_the_weapon_and_the_hammer_is_not() {
        let items = Items::default();
        let strikes = |name: &str| items.tool(items.kind(name)).and_then(ToolDef::strikes);
        let (spear, spear_reach) = strikes("Spear").unwrap();
        for kind in items.kinds() {
            if let Some((damage, reach)) = items.tool(kind).and_then(ToolDef::strikes) {
                assert!(damage <= spear && reach <= spear_reach);
            }
        }
        assert_eq!(strikes("Hammer"), None);
        assert!(strikes("Iron axe").unwrap().0 > strikes("Stone axe").unwrap().0);
    }
}
