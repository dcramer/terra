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
use super::crafting::{RECIPES, Station};
use super::inventory::{Belongings, Broke, InventoryScreen, Items, Made, Tool};
use super::props::Prop;
use bevy::prelude::*;
use terra_items::ItemKind;

pub fn plugin(app: &mut App) {
    app.add_message::<Broke>()
        .add_systems(Update, (burn, tell, mend).chain());
}

/// The station that mends a kind of thing: the one that makes it. The hammer, which no
/// station makes, is mended at the workbench.
pub fn mended_at(items: &Items, kind: ItemKind) -> Option<Station> {
    let made = if let Some(tool) = items.tool(kind) {
        if tool == Tool::Hammer {
            return Some(Station::Workbench);
        }
        Made::Tool(tool)
    } else {
        Made::Apparel(items.apparel(kind)?)
    };
    RECIPES
        .iter()
        .find(|recipe| recipe.makes == made)
        .map(|recipe| recipe.station)
}

/// A torch in hand burns down, a use for each second.
fn burn(time: Res<Time>, mut belongings: Belongings, mut burnt: Local<f32>) {
    if belongings.holding() != Some(Tool::Torch) {
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
fn tell(items: Res<Items>, mut broke: MessageReader<Broke>, mut notice: ResMut<Notice>) {
    for &Broke { kind } in broke.read() {
        let items = &*items;
        let name = items.name(kind).to_lowercase();
        let verb = if items.tool(kind) == Some(Tool::Torch) {
            "burnt out"
        } else {
            "broke"
        };
        notice.say(match mended_at(items, kind) {
            Some(station) => format!(
                "Your {name} {verb}: mend it at a {}",
                station.name().to_lowercase()
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
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut was_at: Local<Option<Entity>>,
) {
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
        .map(|(kind, _)| (kind, mended_at(belongings.items(), kind)))
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
    info!("Mended {} at the {}.", names.join(", "), station.name());
    notice.say(format!(
        "Mended your {} at the {}",
        names.join(" and "),
        station.name().to_lowercase()
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::inventory::Apparel;

    #[test]
    fn each_thing_is_mended_where_it_is_made() {
        let items = Items::default();
        let at = |tool| mended_at(&items, items.tool_kind(tool));
        assert_eq!(at(Tool::StoneAxe), Some(Station::Workbench));
        assert_eq!(at(Tool::IronAxe), Some(Station::Forge));
        assert_eq!(at(Tool::Torch), Some(Station::Campfire));
        assert_eq!(at(Tool::Hammer), Some(Station::Workbench));
        let coat = items.apparel_kind(Apparel::LeatherCoat);
        assert_eq!(mended_at(&items, coat), Some(Station::Workbench));
        // Every tool can be mended somewhere.
        for tool in Tool::ALL {
            assert!(at(tool).is_some(), "{tool:?}");
        }
    }
}
