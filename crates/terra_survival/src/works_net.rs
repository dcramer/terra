//! A station that works on its own, across the network (docs/netcode.md, build step 5): a
//! smelter or a campfire is one station in every game, so what one player sets it to make
//! another may take from it. The host runs it (`crafting::work` leaves it be in a joined
//! game): each one standing on the host is a thing of its own kind ([`STATION`]) whose one
//! part says which prop it is, its queue, how far along it is and what waits made, told on
//! a change (D652). A joined game shows that, and asks the host for what its own player
//! does at it ([`WORKED`]): one more queued, one taken off the queue, so many taken of what
//! was made. What it costs and what is made are in the asker's own pack, at its word, until
//! the pack is the server's (D592, D644).
//!
//! A joined game's ask carries a number; the host's part says the number of the last ask it
//! did. Until a joined game sees its own number (or two seconds pass) it keeps what its
//! player did, so a word the host said before the ask reached it takes nothing back.
//!
//! A station a player works at (a workbench) stays each game's own: what it makes goes to
//! the one working it, as they stand there.

use crate::crafting::{MOST_QUEUED, Recipes, Stations, Works};
use crate::props::{Prop, Theirs};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use terra_net::proto::{EntityId, Kind};
use terra_walker::net::link::Link;
use terra_walker::net::{Host, tell};
use terra_world::tick::Sim;

/// The kind of a lone-working station's entity.
pub const STATION: Kind = Kind(4);
/// The first byte of what a joined game asks of a station.
pub const WORKED: u8 = 25;
/// How often the stations standing are looked over, in ticks: four times a second.
const EVERY_TICKS: u32 = 16;
/// How many lookings a joined game waits for the host to say its ask was done: two seconds.
const MOST_WAITS: u32 = 8;
/// No prop's name is longer, and no station holds more made and waiting than is told.
const MOST_NAME: usize = 48;
const MOST_DONE: usize = 64;

/// Which prop a station is, whoever's game it stands in: its item and where it stands in
/// the base's space, to the centimetre.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key(String, [i32; 3]);

impl Key {
    fn of(prop: &Prop) -> Key {
        let at = prop.at.to_array();
        Key(
            prop.item.name.clone(),
            at.map(|metres| (metres * 100.0).round() as i32),
        )
    }

    fn said(&self, bytes: &mut Vec<u8>) {
        let name = &self.0.as_bytes()[..self.0.len().min(MOST_NAME)];
        bytes.push(name.len() as u8);
        bytes.extend_from_slice(name);
        for metres in self.1 {
            bytes.extend(metres.to_le_bytes());
        }
    }

    /// The key some bytes begin with, and the bytes after it.
    fn heard(bytes: &[u8]) -> Option<(Key, &[u8])> {
        let (&length, rest) = bytes.split_first()?;
        if usize::from(length) > MOST_NAME {
            return None;
        }
        let (name, rest) = rest.split_at_checked(usize::from(length))?;
        let (at, rest) = rest.split_at_checked(12)?;
        let at = [0, 4, 8].map(|i| i32::from_le_bytes([at[i], at[i + 1], at[i + 2], at[i + 3]]));
        Some((Key(std::str::from_utf8(name).ok()?.to_string(), at), rest))
    }
}

/// A station's work as the host tells it: its queue and what waits made, each a recipe by
/// its place among them all, how many whole seconds into the first it is, and the number
/// of the last ask done to it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct Shown {
    last: u32,
    seconds: u16,
    queue: Vec<u16>,
    done: Vec<u16>,
}

impl Shown {
    fn of(works: &Works, last: u32) -> Shown {
        let recipe = |index: &usize| *index as u16;
        Shown {
            last,
            seconds: works.progress.clamp(0.0, 65_535.0) as u16,
            queue: works.queue.iter().take(MOST_QUEUED).map(recipe).collect(),
            done: works.done.iter().take(MOST_DONE).map(recipe).collect(),
        }
    }

    fn said(&self, key: &Key) -> Vec<u8> {
        let mut bytes = Vec::new();
        key.said(&mut bytes);
        bytes.extend(self.last.to_le_bytes());
        bytes.extend(self.seconds.to_le_bytes());
        for list in [&self.queue, &self.done] {
            bytes.push(list.len() as u8);
            for recipe in list {
                bytes.extend(recipe.to_le_bytes());
            }
        }
        bytes
    }

    /// The station and its work some bytes say, if they are one's whole and no more.
    fn heard(bytes: &[u8]) -> Option<(Key, Shown)> {
        let (key, rest) = Key::heard(bytes)?;
        let (head, mut rest) = rest.split_at_checked(6)?;
        let mut list = |most: usize| {
            let (&count, after) = rest.split_first()?;
            let (list, after) = after.split_at_checked(usize::from(count) * 2)?;
            rest = after;
            (usize::from(count) <= most).then(|| {
                list.chunks_exact(2)
                    .map(|recipe| u16::from_le_bytes([recipe[0], recipe[1]]))
                    .collect::<Vec<u16>>()
            })
        };
        let (queue, done) = (list(MOST_QUEUED)?, list(MOST_DONE)?);
        rest.is_empty().then_some((
            key,
            Shown {
                last: u32::from_le_bytes([head[0], head[1], head[2], head[3]]),
                seconds: u16::from_le_bytes([head[4], head[5]]),
                queue,
                done,
            },
        ))
    }

    /// The work this says, as a station holds it: a recipe this game lacks is left out.
    fn works(&self, recipes: &Recipes) -> Works {
        let known = |list: &[u16]| -> Vec<usize> {
            (list.iter().map(|recipe| usize::from(*recipe)))
                .filter(|recipe| *recipe < recipes.iter().count())
                .collect()
        };
        Works::resumed(
            known(&self.queue),
            f32::from(self.seconds),
            known(&self.done),
        )
    }
}

/// What a joined player did at a station, as it is asked of the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    /// One more of a recipe on the queue.
    Queue(u16),
    /// The thing at a place in the queue, of a recipe, taken off it.
    Cancel(u8, u16),
    /// So many of what waited made, taken, first made first.
    Took(u8),
}

impl Act {
    fn said(self, key: &Key, number: u32) -> Vec<u8> {
        let mut bytes = vec![WORKED];
        key.said(&mut bytes);
        bytes.extend(number.to_le_bytes());
        let (which, place, recipe) = match self {
            Act::Queue(recipe) => (0, 0, recipe),
            Act::Cancel(place, recipe) => (1, place, recipe),
            Act::Took(count) => (2, count, 0),
        };
        bytes.extend([which, place]);
        bytes.extend(recipe.to_le_bytes());
        bytes
    }

    /// The station, the ask's number and the act some bytes after [`WORKED`] say.
    fn heard(bytes: &[u8]) -> Option<(Key, u32, Act)> {
        let (key, rest) = Key::heard(bytes)?;
        let &[a, b, c, d, which, place, low, high] = rest else {
            return None;
        };
        let recipe = u16::from_le_bytes([low, high]);
        let act = match which {
            0 => Act::Queue(recipe),
            1 => Act::Cancel(place, recipe),
            2 => Act::Took(place),
            _ => return None,
        };
        Some((key, u32::from_le_bytes([a, b, c, d]), act))
    }

    /// Does it to a station's work, as far as it can be done: a queue takes no more than
    /// it holds, and only a recipe of this station's; only the thing said is taken off.
    fn done_to(self, works: &mut Works, recipes: &Recipes, prop: &Prop) {
        match self {
            Act::Queue(recipe) => {
                let recipe = usize::from(recipe);
                let here = (recipes.iter().nth(recipe))
                    .is_some_and(|found| Some(found.station) == prop.item.station);
                if here && works.queue.len() < MOST_QUEUED {
                    works.queue.push_back(recipe);
                }
            }
            Act::Cancel(place, recipe) => {
                let place = usize::from(place);
                if works.queue.get(place) == Some(&usize::from(recipe)) {
                    works.queue.remove(place);
                    if place == 0 {
                        works.progress = 0.0;
                    }
                }
            }
            Act::Took(count) => {
                let count = usize::from(count).min(works.done.len());
                works.done.drain(..count);
            }
        }
    }
}

/// What a joined player has done to a station since the host's word of it: what was `told`
/// against what is `now`, each a queue and what waits made.
fn acts(told: (&[u16], &[u16]), now: (&[u16], &[u16])) -> Vec<Act> {
    let mut acts = Vec::new();
    if now.1.len() < told.1.len() {
        acts.push(Act::Took((told.1.len() - now.1.len()) as u8));
    }
    if now.0.len() > told.0.len() && now.0.starts_with(told.0) {
        acts.extend(
            now.0[told.0.len()..]
                .iter()
                .map(|recipe| Act::Queue(*recipe)),
        );
    } else if now.0.len() + 1 == told.0.len() {
        let place = (0..now.0.len())
            .find(|place| now.0[*place] != told.0[*place])
            .unwrap_or(now.0.len());
        acts.push(Act::Cancel(place as u8, told.0[place]));
    }
    acts
}

/// On the host: each station's thing on the network, what was last told of it and the
/// number of the last ask done to it. On a joined game: each station's work as the host
/// last said it and this game took it, and the ask it waits to see done.
#[derive(Resource, Default)]
struct Told {
    put: HashMap<Entity, (EntityId, Vec<u8>)>,
    last: HashMap<Key, u32>,
    taken: HashMap<Key, (Vec<u16>, Vec<u16>)>,
    waits: HashMap<Key, (u32, u32)>,
    number: u32,
    since: u32,
}

type Lone<'w, 's, D> = Query<'w, 's, D, Without<Theirs>>;

/// Whether a prop is a station that works on its own.
fn lone(prop: &Prop, stations: &Stations) -> bool {
    (prop.item.station).is_some_and(|station| stations.works_alone(station))
}

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

/// On the host, before the tick's telling: what a guest's player did at a station is done
/// to it, and every lone-working station standing is told as it is, on a change.
fn host(
    mut host: ResMut<Host>,
    mut told: ResMut<Told>,
    (stations, recipes): (Res<Stations>, Res<Recipes>),
    mut works: Lone<(Entity, &Prop, &mut Works, &GlobalTransform)>,
) {
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        let Some((&WORKED, rest)) = bytes.split_first() else {
            // Another layer's.
            host.asked.push((peer, bytes));
            continue;
        };
        let Some((key, number, act)) = Act::heard(rest) else {
            continue;
        };
        for (_, prop, mut works, _) in &mut works {
            if lone(prop, &stations) && Key::of(prop) == key {
                act.done_to(&mut works, &recipes, prop);
                told.last.insert(key.clone(), number);
            }
        }
    }
    told.since += 1;
    if told.since < EVERY_TICKS {
        return;
    }
    told.since = 0;
    let mut standing = Vec::new();
    for (entity, prop, works, at) in &works {
        if !lone(prop, &stations) {
            continue;
        }
        standing.push(entity);
        let key = Key::of(prop);
        let last = told.last.get(&key).copied().unwrap_or(0);
        let bytes = Shown::of(works, last).said(&key);
        let place = at.translation();
        match told.put.get_mut(&entity) {
            Some((_, said)) if *said == bytes => {}
            Some((id, said)) => {
                host.tell_of(*id, place, &bytes);
                *said = bytes;
            }
            None => {
                let id = host.put(STATION, place);
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

/// On a joined game: what its own player has done at a station since the host's word is
/// asked of the host, and each station is at what the host says, once this game's asks of
/// it are done.
fn joined(
    mut link: ResMut<Link>,
    mut told: ResMut<Told>,
    (stations, recipes): (Res<Stations>, Res<Recipes>),
    mut works: Lone<(&Prop, &mut Works)>,
) {
    told.since += 1;
    let Some(tick) = link.tick() else {
        return;
    };
    if told.since < EVERY_TICKS {
        return;
    }
    told.since = 0;
    if told.number == 0 {
        // Numbers of its own, begun where no other game's are likely to be.
        told.number = tick.wrapping_mul(2_654_435_761) | 1;
    }
    let says: HashMap<Key, Shown> = (link.shown_of(STATION, 0).into_iter())
        .filter_map(|(_, _, newest, _)| Shown::heard(newest))
        .collect();
    for (prop, mut works) in &mut works {
        let key = Key::of(prop);
        let (true, Some(word)) = (lone(prop, &stations), says.get(&key)) else {
            continue;
        };
        let now = Shown::of(&works, word.last);
        // Its own player's doings since the host's word was taken, asked of the host.
        if let Some((queue, done)) = told.taken.get(&key) {
            let mine = acts((queue, done), (&now.queue, &now.done));
            if !mine.is_empty() {
                for act in mine {
                    told.number = told.number.wrapping_add(2);
                    link.ask(&act.said(&key, told.number));
                }
                let number = told.number;
                told.waits.insert(key.clone(), (number, 0));
                told.taken.insert(key, (now.queue, now.done));
                continue;
            }
        }
        // Until the host says the ask was done, what the player did here stands.
        if let Some((number, waited)) = told.waits.get_mut(&key) {
            if word.last != *number && *waited < MOST_WAITS {
                *waited += 1;
                continue;
            }
            told.waits.remove(&key);
        }
        if now != *word {
            *works = word.works(&recipes);
        }
        told.taken
            .insert(key, (word.queue.clone(), word.done.clone()));
    }
}

/// With no session, what was told is forgotten, so the next is told anew.
fn alone(mut told: ResMut<Told>) {
    if !(told.put.is_empty() && told.taken.is_empty() && told.last.is_empty()) {
        *told = Told::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> Key {
        Key("Smelter".into(), [320, 0, -1_475])
    }

    #[test]
    fn a_stations_work_and_an_ask_cross_whole_and_nonsense_is_none() {
        let shown = Shown {
            last: 0xDEAD_BEEF,
            seconds: 7,
            queue: vec![3, 3, 12],
            done: vec![3],
        };
        let bytes = shown.said(&key());
        assert_eq!(Shown::heard(&bytes), Some((key(), shown)));
        assert_eq!(Shown::heard(&bytes[..bytes.len() - 1]), None);
        assert_eq!(Shown::heard(&[bytes.clone(), vec![0]].concat()), None);
        assert_eq!(Shown::heard(&[]), None);
        for act in [Act::Queue(12), Act::Cancel(2, 12), Act::Took(3)] {
            let bytes = act.said(&key(), 41);
            assert_eq!(bytes[0], WORKED);
            assert_eq!(Act::heard(&bytes[1..]), Some((key(), 41, act)));
            assert_eq!(Act::heard(&bytes[1..bytes.len() - 1]), None);
        }
        // An act of no kind there is: none.
        let mut odd = Act::Took(1).said(&key(), 41);
        let which = odd.len() - 4;
        odd[which] = 9;
        assert_eq!(Act::heard(&odd[1..]), None);
    }

    /// What a player did is read from the queue and what waits, before and after: knowing
    /// the answer by doing each to a queue by hand.
    #[test]
    fn what_a_player_did_is_read_from_the_queue_before_and_after() {
        let told: (&[u16], &[u16]) = (&[3, 5, 3], &[7, 7]);
        assert_eq!(acts(told, told), Vec::new());
        assert_eq!(acts(told, (&[3, 5, 3, 9], &[7, 7])), vec![Act::Queue(9)]);
        assert_eq!(
            acts(told, (&[3, 5, 3, 9, 9], &[7, 7])),
            vec![Act::Queue(9), Act::Queue(9)]
        );
        assert_eq!(acts(told, (&[3, 3], &[7, 7])), vec![Act::Cancel(1, 5)]);
        assert_eq!(acts(told, (&[3, 5], &[7, 7])), vec![Act::Cancel(2, 3)]);
        assert_eq!(acts(told, (&[5, 3], &[7, 7])), vec![Act::Cancel(0, 3)]);
        assert_eq!(acts(told, (&[3, 5, 3], &[])), vec![Act::Took(2)]);
        // Both at once: what was made taken, and one more queued.
        assert_eq!(
            acts(told, (&[3, 5, 3, 4], &[7])),
            vec![Act::Took(1), Act::Queue(4)]
        );
    }
}
