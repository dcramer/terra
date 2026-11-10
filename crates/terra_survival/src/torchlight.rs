//! A torch in hand burns: the world's own torch fire at its head (`terra_world::fire`, the
//! flames, smoke and wavering pitch light a standing torch has, D377), which is what lets
//! whoever holds it see at night (D134). The torch's look is shown in hand as any held thing's
//! is (`held`); this only sets it alight.

use crate::held::InHand;
use crate::inventory::{self, Backpack, Bar, Gear, Items, OFF_HAND};
use bevy::light::NotShadowCaster;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use terra_walker::Player;
use terra_walker::character::Held;
use terra_walker::figure::{Grip, Hand};
use terra_world::fire::{Body, Carried, Fill, Kindled, Logs, Rig, TORCH_LIGHT};
use terra_world::footprint::Footprint;

/// Where its flames stand from along its look, from where it is held, in metres: its head, as
/// the dungeon's wall torch has it (0.47 m over the foot, the grip 0.1 m up, art/held/torch.json).
const HEAD_M: f32 = 0.37;
/// A hand torch's flames: a hand's width and a forearm tall, two thirds a wall torch's
/// (the dungeon's `TORCH_FIRE`, 0.45 m), drawn and lit as it is: the marched torch field
/// (D466), pitch's light, a torch's rig as one carried reaches (`Rig::Held`).
const FIRE: Kindled = Kindled {
    tall: 0.3,
    wide: 0.1,
    heat: 0.6,
    logs: Logs::NONE,
    body: Body::Marched,
    light: TORCH_LIGHT,
    rig: Rig::Held,
};

/// Where a carried torch's shadowed light hangs on its holder's body (`fire::Carried::key`:
/// to the torch's side, up, ahead, metres from their feet). None: over its flames, on the
/// hand. A place on the body is tried against that in one game run by
/// tools/sandbox/torchlight.ts (`fire.carried`).
const KEY_M: Option<[f32; 3]> = None;
/// Its fill on its holder (`fire::Carried::fill`): none.
const FILL: Option<Fill> = None;

/// The light of the torch in `grip`'s hand on its holder's body: on the right for the main
/// hand, the left for the off hand.
fn carried(grip: &Grip) -> Carried {
    let side = match grip.hand {
        Hand::Main => 1.0,
        Hand::Off => -1.0,
    };
    Carried {
        by: grip.player,
        side,
        key: KEY_M,
        fill: FILL,
    }
}

/// A torch's flame, with its light.
#[derive(Component)]
struct Flame;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (light, unshadow).chain());
}

/// Sets a torch in either of a player's hands alight, and puts it out when it is put away or
/// burnt out. The flame goes on the look in hand, so it moves with it.
fn light(
    mut commands: Commands,
    items: Res<Items>,
    players: Query<(&Backpack, &Bar, &Gear), With<Player>>,
    grips: Query<(Entity, &Grip, &InHand)>,
    children: Query<&Children>,
    held: Query<(), With<Held>>,
    flames: Query<(Entity, &ChildOf), With<Flame>>,
    meshes: Query<(), With<Mesh3d>>,
) {
    // What is alight now: the look in each hand holding a torch.
    let mut alight = HashMap::new();
    for (hand, grip, shown) in &grips {
        let lights = shown
            .kind()
            .and_then(|kind| items.tool(kind))
            .is_some_and(|tool| tool.lights);
        // Not burnt out: the bar's item in the main hand, the off hand's slot's in the other.
        let burning = lights
            && players
                .get(grip.player)
                .is_ok_and(|(backpack, bar, gear)| match grip.hand {
                    Hand::Main => inventory::holding(&items, &backpack.0, &bar.0) == shown.kind(),
                    Hand::Off => gear.0.worn(OFF_HAND).is_some_and(|item| {
                        let def = items.catalogue.get(item.kind);
                        def.is_ok_and(|def| !item.broken(def))
                    }),
                });
        let in_hand = children
            .get(hand)
            .ok()
            .and_then(|looks| looks.iter().find(|&each| held.contains(each)));
        if let (true, Some(torch)) = (burning, in_hand) {
            alight.insert(torch, carried(grip));
        }
    }
    // Anything alight that is not a torch in hand goes out.
    let mut lit = HashSet::new();
    for (flame, parent) in &flames {
        if alight.contains_key(&parent.parent()) {
            lit.insert(parent.parent());
        } else {
            commands.entity(flame).despawn();
        }
    }
    let unlit = alight.into_iter().filter(|(torch, _)| !lit.contains(torch));
    for (torch, carried) in unlit {
        // The fire burns up from the head however the torch is tipped: a kept fire is drawn
        // from where it stands, up. An empty footprint, so it clears no ground as a campfire
        // does (`fire::scorch`).
        commands.entity(torch).with_child((
            Name::new("Flame"),
            Flame,
            FIRE,
            carried,
            Footprint::default(),
            Transform::from_xyz(0.0, HEAD_M, 0.0),
        ));
        for part in children.iter_descendants(torch) {
            if meshes.contains(part) {
                commands.entity(part).insert(NotShadowCaster);
            }
        }
    }
}

/// A burning torch casts no shadow, as a game's torch is left out of its own light's shadows:
/// its light hangs a hand's breadth over its head, so the head would shadow the ground round
/// whoever holds it (as the staged torch's did, `terra_creatures::staged::quench::unshadow`).
/// Its parts already there are told when it is lit (`light`); this tells those loaded after.
fn unshadow(
    parts: Query<Entity, Added<Mesh3d>>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    flames: Query<(), With<Flame>>,
    mut commands: Commands,
) {
    for part in &parts {
        let burning = parents.iter_ancestors(part).any(|up| {
            children
                .get(up)
                .is_ok_and(|under| under.iter().any(|each| flames.contains(each)))
        });
        if burning {
            commands.entity(part).insert(NotShadowCaster);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A torch's light is its holder's, on the side of the hand it is in. A key on the body
    /// hangs over the head (a 1.8 m body) and ahead of the spine: never behind, where it
    /// would throw the holder's shadow forward over the ground they walk into.
    #[test]
    fn a_torchs_light_is_on_its_holders_body_on_the_torchs_side() {
        let player = Entity::PLACEHOLDER;
        let of = |hand| carried(&Grip { player, hand });
        let (main, off) = (of(Hand::Main), of(Hand::Off));
        assert_eq!(main.by, player);
        assert!(main.side == 1.0 && off.side == -1.0);
        assert_eq!((main.key, main.fill), (off.key, off.fill));
        if let Some([side, up, ahead]) = main.key {
            assert!(side >= 0.0 && up > 1.8 && up < 2.5 && ahead >= 0.0);
        }
    }
}
