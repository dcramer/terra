//! What storage holds, across the network (docs/netcode.md, build step 5): a chest's
//! contents are the same in every game, so what one player leaves in it another takes out.
//! It is state, sent as the most recent (D652): each storage standing on the host is a thing
//! of its own kind ([`STORE`]) whose one part says which prop it is and what its grid holds;
//! a joined game fills its own copy of that prop to match, and asks the host to hold what
//! its player left or took ([`STORED`]), which the host takes at its word until the pack is
//! the server's (D592, D644).
//!
//! Not yet: two players in one chest at once each write the whole of it, and the later word
//! stands (Valheim gives a chest one user at a time).

use crate::inventory::Items;
use crate::props::{Prop, Theirs};
use crate::storage::Storage;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use terra_items::{Cell, Grid, ItemKind};
use terra_net::proto::{EntityId, Kind};
use terra_walker::net::link::Link;
use terra_walker::net::{Host, tell};
use terra_world::tick::Sim;

/// The kind of a storage's entity.
pub const STORE: Kind = Kind(3);
/// The first byte of what a joined game asks a storage to hold.
pub const STORED: u8 = 24;
/// How often the storage standing is looked over, in ticks: four times a second.
const EVERY_TICKS: u32 = 16;
/// No prop's name is longer, and no storage holds more items: a part stays well inside a
/// packet (a chest of 48 single cells is 510 bytes).
const MOST_NAME: usize = 48;
const MOST_ITEMS: usize = 96;

/// One item as it crosses: its kind, its cell, how many its stack holds and how worn it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Kept {
    y: u8,
    x: u8,
    kind: u16,
    held: u32,
    wear: u16,
}

/// A storage as it crosses: which prop it is (its item and where it stands in the base's
/// space, to the centimetre) and what it holds, in the order of its cells.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Held {
    item: String,
    at: [i32; 3],
    items: Vec<Kept>,
}

/// Which prop a storage is, whoever's game it stands in.
type Key = (String, [i32; 3]);

impl Held {
    fn of(prop: &Prop, grid: &Grid) -> Held {
        let mut items: Vec<Kept> = grid
            .iter()
            .map(|placed| Kept {
                y: placed.at.y,
                x: placed.at.x,
                kind: placed.item.kind.0,
                held: placed.stack.map_or(0, |stack| stack.held),
                wear: placed.item.wear,
            })
            .collect();
        items.sort();
        Held {
            item: prop.item.name.clone(),
            at: prop
                .at
                .to_array()
                .map(|metres| (metres * 100.0).round() as i32),
            items,
        }
    }

    fn key(&self) -> Key {
        (self.item.clone(), self.at)
    }

    fn said(&self) -> Vec<u8> {
        let name = &self.item.as_bytes()[..self.item.len().min(MOST_NAME)];
        let mut bytes = vec![name.len() as u8];
        bytes.extend_from_slice(name);
        for metres in self.at {
            bytes.extend(metres.to_le_bytes());
        }
        let items = &self.items[..self.items.len().min(MOST_ITEMS)];
        bytes.push(items.len() as u8);
        for item in items {
            bytes.extend([item.x, item.y]);
            bytes.extend(item.kind.to_le_bytes());
            bytes.extend(item.held.to_le_bytes());
            bytes.extend(item.wear.to_le_bytes());
        }
        bytes
    }

    /// The storage some bytes say, if they are one's whole and no more.
    fn heard(bytes: &[u8]) -> Option<Held> {
        let (&length, rest) = bytes.split_first()?;
        let (name, rest) = rest.split_at_checked(usize::from(length).min(MOST_NAME + 1))?;
        let (at, rest) = rest.split_at_checked(12)?;
        let at = [0, 4, 8].map(|i| i32::from_le_bytes([at[i], at[i + 1], at[i + 2], at[i + 3]]));
        let (&count, rest) = rest.split_first()?;
        if usize::from(length) > MOST_NAME
            || usize::from(count) > MOST_ITEMS
            || rest.len() != usize::from(count) * 10
        {
            return None;
        }
        let items = rest
            .chunks_exact(10)
            .map(|item| Kept {
                x: item[0],
                y: item[1],
                kind: u16::from_le_bytes([item[2], item[3]]),
                held: u32::from_le_bytes([item[4], item[5], item[6], item[7]]),
                wear: u16::from_le_bytes([item[8], item[9]]),
            })
            .collect();
        Some(Held {
            item: std::str::from_utf8(name).ok()?.to_string(),
            at,
            items,
        })
    }

    /// A grid of a storage's size holding what this says: an item of a kind this game's
    /// catalogue lacks, or that does not fit where it is said to lie, is left out.
    fn filled(&self, items: &mut Items, like: &Grid) -> Grid {
        let size = like.size();
        let mut grid = Grid::new(size.w, size.h);
        for kept in &self.items {
            if items.catalogue.get(ItemKind(kept.kind)).is_err() {
                continue;
            }
            let item = items.make(ItemKind(kept.kind));
            let cell = Cell {
                x: kept.x,
                y: kept.y,
            };
            if grid.insert(&items.catalogue, item, Some(cell)).is_err() {
                continue;
            }
            if kept.held > 0 {
                let _ = grid.set_held(item.id, kept.held);
            }
            if kept.wear > 0 {
                let _ = grid.set_wear(item.id, kept.wear);
            }
        }
        grid
    }
}

/// On the host: each storage's thing on the network and what was last told of it. On a
/// joined game: what the host last said each holds, and what this game last said of it.
#[derive(Resource, Default)]
struct Told {
    put: HashMap<Entity, (EntityId, Vec<u8>)>,
    heard: HashMap<Key, Vec<u8>>,
    said: HashMap<Key, Vec<u8>>,
    since: u32,
}

type Stores<'w, 's, D> = Query<'w, 's, D, (With<Prop>, Without<Theirs>)>;

pub fn plugin(app: &mut App) {
    app.init_resource::<Told>().add_systems(
        FixedUpdate,
        (
            host.before(tell).run_if(resource_exists::<Host>),
            joined
                .after(terra_walker::net::link::show)
                .run_if(resource_exists::<Link>),
            alone
                .run_if(not(resource_exists::<Link>))
                .run_if(not(resource_exists::<Host>)),
        )
            .in_set(Sim::End),
    );
}

/// On the host, before the tick's telling: a storage holds what a guest's game says its
/// player left it holding, and every storage standing is told as it is, on a change.
fn host(
    mut host: ResMut<Host>,
    mut told: ResMut<Told>,
    mut items: ResMut<Items>,
    mut stores: Stores<(Entity, &Prop, &mut Storage, &GlobalTransform)>,
) {
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        let Some((&STORED, rest)) = bytes.split_first() else {
            // Another layer's.
            host.asked.push((peer, bytes));
            continue;
        };
        let Some(held) = Held::heard(rest) else {
            continue;
        };
        for (_, prop, mut storage, _) in &mut stores {
            let now = Held::of(prop, &storage.0);
            if now.key() == held.key() && now != held {
                storage.0 = held.filled(&mut items, &storage.0);
            }
        }
    }
    told.since += 1;
    if told.since < EVERY_TICKS {
        return;
    }
    told.since = 0;
    let mut standing = Vec::new();
    for (entity, prop, storage, at) in &stores {
        standing.push(entity);
        let bytes = Held::of(prop, &storage.0).said();
        let place = at.translation();
        match told.put.get_mut(&entity) {
            Some((_, last)) if *last == bytes => {}
            Some((id, last)) => {
                host.tell_of(*id, place, &bytes);
                *last = bytes;
            }
            None => {
                let id = host.put(STORE, place);
                host.tell_of(id, place, &bytes);
                told.put.insert(entity, (id, bytes));
            }
        }
    }
    told.put.retain(|entity, (id, _)| {
        let stands = standing.contains(entity);
        if !stands {
            host.gone(*id);
        }
        stands
    });
}

/// On a joined game: each storage holds what the host newly says it does, and what its own
/// player has since left in one or taken out is asked of the host.
fn joined(
    mut link: ResMut<Link>,
    mut told: ResMut<Told>,
    mut items: ResMut<Items>,
    mut stores: Stores<(&Prop, &mut Storage)>,
) {
    told.since += 1;
    if told.since < EVERY_TICKS || link.tick().is_none() {
        return;
    }
    told.since = 0;
    let says: HashMap<Key, Vec<u8>> = (link.shown_of(STORE, 0).into_iter())
        .filter_map(|(_, _, newest, _)| Some((Held::heard(newest)?.key(), newest.to_vec())))
        .collect();
    for (prop, mut storage) in &mut stores {
        let now = Held::of(prop, &storage.0);
        let (key, mut bytes) = (now.key(), now.said());
        // The host's word first, once for each change of it: a storage whose prop does not
        // stand here yet is filled when it does.
        if let Some(word) = says.get(&key)
            && told.heard.get(&key) != Some(word)
        {
            if *word != bytes
                && let Some(held) = Held::heard(word)
            {
                storage.0 = held.filled(&mut items, &storage.0);
                bytes = Held::of(prop, &storage.0).said();
            }
            told.heard.insert(key.clone(), word.clone());
            told.said.insert(key.clone(), bytes.clone());
        }
        // Then this game's own: only a storage the host has told of, so what stood here
        // before the session is never pressed on the host.
        if told.heard.contains_key(&key) && told.said.get(&key) != Some(&bytes) {
            let mut ask = vec![STORED];
            ask.extend(&bytes);
            link.ask(&ask);
            told.said.insert(key, bytes);
        }
    }
}

/// With no session, what was told is forgotten, so the next is told anew.
fn alone(mut told: ResMut<Told>) {
    if !(told.put.is_empty() && told.heard.is_empty() && told.said.is_empty()) {
        *told = Told::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held() -> Held {
        let item = |x, y, kind, held, wear| Kept {
            y,
            x,
            kind,
            held,
            wear,
        };
        Held {
            item: "Chest".into(),
            at: [-1250, 0, 4_075],
            items: vec![
                item(0, 0, 7, 0, 0),
                item(3, 0, 12, 24, 0),
                item(1, 2, 40, 0, 310),
            ],
        }
    }

    #[test]
    fn what_a_storage_holds_crosses_whole_and_nonsense_is_none() {
        let held = held();
        let bytes = held.said();
        assert_eq!(Held::heard(&bytes), Some(held));
        // Cut short, run long, or saying more items than it carries: none.
        assert_eq!(Held::heard(&bytes[..bytes.len() - 1]), None);
        assert_eq!(Held::heard(&[bytes.clone(), vec![0]].concat()), None);
        let mut more = bytes.clone();
        let count = 1 + "Chest".len() + 12;
        more[count] += 1;
        assert_eq!(Held::heard(&more), None);
        assert_eq!(Held::heard(&[]), None);
        assert_eq!(Held::heard(&[200, 1, 2]), None);
    }

    /// A full chest of single cells, the most a part carries today, stays well inside a
    /// packet's 1,200 bytes.
    #[test]
    fn a_full_chest_is_a_part_that_fits_a_packet() {
        let mut full = held();
        full.items = (0..48u8)
            .map(|cell| Kept {
                y: cell / 8,
                x: cell % 8,
                kind: 9,
                held: 0,
                wear: 0,
            })
            .collect();
        assert!(full.said().len() < 600, "{} bytes", full.said().len());
    }
}
