//! Wear and repair (D99): tools, weapons and armour wear with use, and a broken one does
//! nothing until it is mended, but is never lost. Mending is free, at the station that
//! makes the thing, in a claim: the workbench mends stone tools and leather, the forge
//! iron, a campfire torches. Opening the station mends everything it can.
//!
//! The wearing itself is done where the use is (`Belongings::wear_in_hand` and
//! `wear_worn`), and kept on the item (`terra_items`). This says when something breaks,
//! burns a torch down, and mends.

use super::IsLocalPlayer;
use super::bag::Notice;
use super::claim::Claim;
use super::crafting::{Crafts, Station};
use super::inventory::{Belongings, Broke, InventoryScreen, Items};
use super::props::Prop;
use bevy::prelude::*;
use terra_items::ItemKind;

pub fn plugin(app: &mut App) {
    app.add_message::<Broke>()
        .add_systems(Update, (burn, tell, mend).chain());
}

/// A torch in hand burns down, a use for each second.
fn burn(time: Res<Time>, mut belongings: Belongings, mut burnt: Local<f32>) {
    let Some(mut belongings) = belongings.mine() else {
        return;
    };
    if !belongings.tool_in_hand().is_some_and(|tool| tool.lights) {
        return;
    }
    *burnt += time.delta_secs();
    if *burnt >= 1.0 {
        let seconds = *burnt as u16;
        *burnt -= seconds as f32;
        belongings.wear_in_hand(seconds);
    }
}

/// Says when something has broken, and where to mend it.
fn tell(
    items: Res<Items>,
    crafts: Crafts,
    mut broke: MessageReader<Broke>,
    ours: Option<Single<Entity, IsLocalPlayer>>,
    mut notice: ResMut<Notice>,
) {
    let ours = ours.map(|player| *player);
    for &Broke { player, kind } in broke.read() {
        if Some(player) != ours {
            continue;
        }
        let items = &*items;
        let name = items.name(kind).to_lowercase();
        let verb = if items.tool(kind).is_some_and(|tool| tool.lights) {
            "burnt out"
        } else {
            "broke"
        };
        notice.say(match crafts.mended_at(items, kind) {
            Some(station) => format!(
                "Your {name} {verb}: mend it at a {}",
                crafts.stations.name(station).to_lowercase()
            ),
            None => format!("Your {name} {verb}"),
        });
    }
}

/// Opening a station in a claim mends everything carried or worn that it makes.
fn mend(
    screen: Res<InventoryScreen>,
    claim: Single<&Claim, IsLocalPlayer>,
    stations: Query<&Transform, With<Prop>>,
    crafts: Crafts,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut was_at: Local<Option<Entity>>,
) {
    let Some(mut belongings) = belongings.mine() else {
        return;
    };
    let at = screen.at.map(|(entity, _)| entity);
    if at == *was_at {
        return;
    }
    *was_at = at;
    let Some((entity, station)) = screen.at else {
        return;
    };
    if !stations
        .get(entity)
        .is_ok_and(|there| claim.covers(there.translation))
    {
        return;
    }
    let menders: Vec<(ItemKind, Option<Station>)> = belongings
        .items()
        .catalogue
        .iter()
        .map(|(kind, _)| (kind, crafts.mended_at(belongings.items(), kind)))
        .collect();
    let mended = belongings.mend(|kind| {
        menders
            .iter()
            .any(|&(each, at)| each == kind && at == Some(station))
    });
    if mended.is_empty() {
        return;
    }
    let names: Vec<String> = mended
        .iter()
        .map(|&kind| belongings.items().name(kind).to_lowercase())
        .collect();
    let name = crafts.stations.name(station);
    info!("Mended {} at the {name}.", names.join(", "));
    notice.say(format!(
        "Mended your {} at the {}",
        names.join(" and "),
        name.to_lowercase()
    ));
}

#[cfg(test)]
mod tests {
    use crate::play::crafting::{Recipes, Stations, mended_at};
    use crate::play::inventory::Items;

    #[test]
    fn each_thing_is_mended_where_it_is_made() {
        let items = Items::default();
        let stations = Stations::read();
        let recipes = Recipes::read(&stations, &items);
        let at = |name| mended_at(&items, &stations, &recipes, items.kind(name));
        let station = |name| stations.named(name);
        assert_eq!(at("Stone axe"), station("Workbench"));
        assert_eq!(at("Iron axe"), station("Forge"));
        assert_eq!(at("Torch"), station("Campfire"));
        assert_eq!(at("Hammer"), station("Workbench"));
        assert_eq!(at("Leather coat"), station("Workbench"));
        // Every thing that wears can be mended somewhere.
        for kind in items
            .kinds()
            .filter(|&kind| items.def(kind).lasts.is_some())
        {
            assert!(
                mended_at(&items, &stations, &recipes, kind).is_some(),
                "{}",
                items.name(kind)
            );
        }
    }
}
