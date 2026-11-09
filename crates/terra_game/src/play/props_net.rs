//! What stands in a base, across the network (docs/netcode.md, build step 5): a prop a
//! player sets down (a claim stone, a campfire, a chest) stands in every game, and one taken
//! apart is gone from every game. The host orders each of its props where it stands, those
//! standing as hosting begins too, and each one's going; a joined game sets down the ones
//! it is ordered and asks the host to set down, or take away, its own.
//!
//! What storage holds and a lone station's work cross as state of their own
//! (`terra_survival::storage_net`, `works_net`). Not yet: a fire's burning is each game's
//! own; a claim stone told to a joined game marks no claim there.

use super::catalogue::Builds;
use super::props::{self, Models, Prop, Theirs};
use crate::build::Site;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use terra_walker::net::link::Link;
use terra_walker::net::{Host, tell};
use terra_world::planet::Planet;
use terra_world::tick::Sim;

/// The first byte of a word of a prop, asked or ordered.
pub const PROP: u8 = 23;
/// How often the props standing are looked over, in ticks: four times a second.
const EVERY_TICKS: u32 = 16;
/// No word of a prop is longer.
const MOST_BYTES: usize = 512;

/// A prop as it crosses: its item, where in the base's own space, how raised, turned, and
/// whether indoors.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Stood {
    item: String,
    at: [f32; 3],
    lift: f32,
    turn: f32,
    indoors: bool,
}

/// What is said of a prop.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum Word {
    Placed(Stood),
    Removed(Stood),
}

/// Which prop one is, whoever's game it stands in: its item and where, to the centimetre.
type Key = (String, [i32; 3]);

impl Stood {
    fn of(prop: &Prop) -> Stood {
        Stood {
            item: prop.item.name.clone(),
            at: prop.at.to_array(),
            lift: prop.lift,
            turn: prop.turn,
            indoors: prop.indoors,
        }
    }

    fn key(&self) -> Key {
        (
            self.item.clone(),
            self.at.map(|metres| (metres * 100.0).round() as i32),
        )
    }
}

impl Word {
    fn said(&self) -> Option<Vec<u8>> {
        let mut bytes = vec![PROP];
        bytes.extend(ron::to_string(self).ok()?.into_bytes());
        (bytes.len() <= MOST_BYTES).then_some(bytes)
    }

    fn heard(bytes: &[u8]) -> Option<Word> {
        let (&PROP, rest) = bytes.split_first()? else {
            return None;
        };
        (rest.len() <= MOST_BYTES)
            .then(|| ron::de::from_bytes(rest).ok())
            .flatten()
    }
}

/// The props this game has told of or been told of, each as it stood, so each crosses
/// once; and whether the session has begun.
#[derive(Resource, Default)]
struct Told {
    stood: HashMap<Key, Stood>,
    /// The props a joined game has seen standing in it: only those can be taken apart there.
    seen: bevy::platform::collections::HashSet<Key>,
    begun: bool,
    since: u32,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Told>().add_systems(
        FixedUpdate,
        (
            host.before(tell).run_if(resource_exists::<Host>),
            // After the base's own words: a prop is set down in the base's space, which a
            // word of the base may have just moved (a claim stone set down moves it).
            joined
                .after(terra_walker::net::link::show)
                .after(super::base_net::joined)
                .run_if(resource_exists::<Link>),
            alone.run_if(not(resource_exists::<Link>)),
        )
            .in_set(Sim::End)
            .run_if(terra_world::instance::in_world),
    );
}

/// What setting a prop down needs.
#[derive(bevy::ecs::system::SystemParam)]
struct Setting<'w, 's> {
    commands: Commands<'w, 's>,
    models: Res<'w, Models>,
    builds: Res<'w, Builds>,
    site: Res<'w, Site>,
    planet: Res<'w, Planet>,
    standing: Query<'w, 's, (Entity, &'static Prop), Without<Theirs>>,
}

impl Setting<'_, '_> {
    /// The props standing, each by which it is.
    fn standing(&self) -> HashMap<Key, (Entity, Stood)> {
        self.standing
            .iter()
            .map(|(entity, prop)| {
                let stood = Stood::of(prop);
                (stood.key(), (entity, stood))
            })
            .collect()
    }

    /// Does what a word says: sets a prop down unless it stands already, or takes away the
    /// one that stands there.
    fn make(&mut self, word: &Word) {
        let standing = self.standing();
        match word {
            Word::Placed(stood) if !standing.contains_key(&stood.key()) => {
                let Some(item) = self.builds.item_named(&stood.item) else {
                    warn!(
                        "A prop told of is a {}, which this game's catalogue lacks: not set down.",
                        stood.item
                    );
                    return;
                };
                props::place(
                    &mut self.commands,
                    &self.models,
                    &self.site,
                    &self.planet,
                    &item,
                    Vec3::from(stood.at),
                    stood.lift,
                    stood.turn,
                    stood.indoors,
                );
            }
            Word::Removed(stood) => {
                if let Some((entity, _)) = standing.get(&stood.key()) {
                    self.commands.entity(*entity).try_despawn();
                }
            }
            Word::Placed(_) => {}
        }
    }
}

/// What changed between the props told of and those standing: those newly standing, and
/// those gone.
fn changes(told: &HashMap<Key, Stood>, standing: &HashMap<Key, (Entity, Stood)>) -> Vec<Word> {
    let come = standing
        .iter()
        .filter(|(key, _)| !told.contains_key(*key))
        .map(|(_, (_, stood))| Word::Placed(stood.clone()));
    let gone = told
        .iter()
        .filter(|(key, _)| !standing.contains_key(*key))
        .map(|(_, stood)| Word::Removed(stood.clone()));
    come.chain(gone).collect()
}

/// On the host, before the tick's telling: what a guest asks of a prop is done, and every
/// prop newly standing or gone, whoever's doing, is ordered where it stands.
fn host(
    mut host: ResMut<Host>,
    mut told: ResMut<Told>,
    mut setting: Setting,
    base: Res<super::base_net::Shared>,
) {
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        if bytes.first() != Some(&PROP) {
            // Another layer's.
            host.asked.push((peer, bytes));
            continue;
        }
        if let Some(word) = Word::heard(&bytes) {
            setting.make(&word);
        }
    }
    told.since += 1;
    // After the base's frame: a prop's place is in it (`base_net::Shared::told`).
    if told.since < EVERY_TICKS || !base.told(&setting.site.frame) {
        return;
    }
    told.since = 0;
    for word in changes(&told.stood, &setting.standing()) {
        let (Word::Placed(stood) | Word::Removed(stood)) = &word;
        let at = setting.site.frame.transform_point(Vec3::from(stood.at));
        if let Some(bytes) = word.said() {
            host.order(at, &bytes);
        }
        match &word {
            Word::Placed(stood) => told.stood.insert(stood.key(), stood.clone()),
            Word::Removed(stood) => told.stood.remove(&stood.key()),
        };
    }
}

/// On a joined game, at the tick's end: its own props newly set down or taken apart are
/// asked of the host, and the props the host orders are set down or taken away.
fn joined(mut link: ResMut<Link>, mut told: ResMut<Told>, mut setting: Setting) {
    // Its own doings first, looked for among what stood before this tick's orders are made:
    // an order's prop stands only once the tick is over, and would read as gone.
    told.since += 1;
    if told.since >= EVERY_TICKS {
        told.since = 0;
        let standing = setting.standing();
        // What stood here before the session began is this game's own past, not asked.
        let begun = std::mem::replace(&mut told.begun, true);
        for word in changes(&told.stood, &standing) {
            match &word {
                Word::Placed(stood) => {
                    told.stood.insert(stood.key(), stood.clone());
                }
                // Gone only if it was seen standing here: one told of that this game could
                // not set down (its catalogue lacks it) was never here to take apart.
                Word::Removed(stood) if told.seen.contains(&stood.key()) => {
                    told.stood.remove(&stood.key());
                }
                Word::Removed(_) => continue,
            }
            if begun && let Some(bytes) = word.said() {
                link.ask(&bytes);
            }
        }
        told.seen = standing.into_keys().collect();
    }
    // In the host's order with the base's words (`base_net::in_order`): a prop said before
    // the base moved is set down before it moves.
    let words = super::base_net::in_order(&mut link.events, PROP, super::base_net::BASE);
    for bytes in words {
        let Some(word) = Word::heard(&bytes) else {
            continue;
        };
        setting.make(&word);
        match &word {
            Word::Placed(stood) => {
                told.stood.insert(stood.key(), stood.clone());
            }
            Word::Removed(stood) => {
                told.stood.remove(&stood.key());
                told.seen.remove(&stood.key());
            }
        }
    }
}

/// With neither a link nor a host, what was told is forgotten: the next session begins
/// anew. (Not by the link being new: it is taken out and put back every tick.)
fn alone(mut told: ResMut<Told>, host: Option<Res<Host>>) {
    if host.is_none() && (told.begun || !told.stood.is_empty()) {
        *told = Told::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stood(item: &str, east: f32) -> Stood {
        Stood {
            item: item.into(),
            at: [east, 0.0, 2.0],
            lift: 0.0,
            turn: 90.0,
            indoors: false,
        }
    }

    #[test]
    fn a_word_of_a_prop_crosses_whole_and_nonsense_is_no_word() {
        for word in [
            Word::Placed(stood("Campfire", 1.0)),
            Word::Removed(stood("Chest", -3.5)),
        ] {
            let said = word.said().expect("short enough");
            assert_eq!(said[0], PROP);
            assert_eq!(Word::heard(&said), Some(word));
        }
        assert_eq!(Word::heard(&[PROP, b'?']), None);
        assert_eq!(Word::heard(b"no tag"), None);
    }

    /// Which prop one is does not turn on a millimetre: the same item a hair off is the
    /// same prop, another item there or the same a pace off is another.
    #[test]
    fn a_prop_is_told_by_its_item_and_where_to_the_centimetre() {
        let fire = stood("Campfire", 1.0);
        assert_eq!(stood("Campfire", 1.004).key(), fire.key());
        assert_ne!(stood("Campfire", 2.0).key(), fire.key());
        assert_ne!(stood("Chest", 1.0).key(), fire.key());
    }

    #[test]
    fn what_changed_is_what_newly_stands_and_what_is_gone() {
        let (fire, chest) = (stood("Campfire", 1.0), stood("Chest", 4.0));
        let told: HashMap<Key, Stood> = [(fire.key(), fire.clone())].into_iter().collect();
        let standing: HashMap<Key, (Entity, Stood)> =
            [(chest.key(), (Entity::PLACEHOLDER, chest.clone()))]
                .into_iter()
                .collect();
        let mut words = changes(&told, &standing);
        words.sort_by_key(|word| matches!(word, Word::Removed(_)));
        assert_eq!(words, [Word::Placed(chest), Word::Removed(fire)]);
        assert!(
            changes(&told, &HashMap::default()).len() == 1
                && changes(&HashMap::default(), &standing).len() == 1
        );
    }
}
