//! A guest's character is the host's to keep (D592): in a joined game the player's record
//! (where they stand, how they fare, what they wear, carry and bear) is sent to the host
//! every few seconds and as the game closes; the host keeps it with its world's save, by who
//! the player is, and gives it back in its greeting when they come again
//! (`terra_walker::net::Host::keep`), so a friend finds their character as they left it
//! whatever became of their own machine's files.
//!
//! The record is taken at the guest's word, as their marks are at a gate (`portal::shared`),
//! until the pack is the server's (docs/netcode.md, build step 5): the host checks only that
//! it reads as a record and is no longer than one could be.

use super::{IsLocalPlayer, Kept, PlayerParts, SavedPlayer, load, put_player, saved_player};
use crate::play::inventory::{Backpack, Bar, Gear, Items};
use crate::play::marks::Marks;
use crate::play::skills::Skills;
use crate::play::vitals::Vitals;
use crate::play::{Stamina, View};
use crate::shell::Options;
use bevy::prelude::*;
use bevy::remote::BrpResult;
use terra_walker::net::link::Link;
use terra_walker::net::{Host, farewell, tell};
use terra_world::tick::Sim;

/// The first byte of a guest's record as it is asked of the host.
pub const CHARACTER: u8 = 17;
/// No record is longer: a full pack's is a few thousand bytes.
const MOST_BYTES: usize = 64 * 1024;
/// How often a joined game sends its record, in seconds.
const EVERY_S: f32 = 10.0;

pub(super) fn plugin(app: &mut App) {
    let joined = resource_exists::<Link>;
    app.add_systems(
        FixedUpdate,
        hear.before(tell)
            .run_if(resource_exists::<Host>)
            .in_set(Sim::End),
    )
    .add_systems(
        Update,
        (
            // As hosting begins, and whenever the save is read or kept after: a world
            // hosted as it loads has no save in hand yet.
            restore
                .after(load)
                .run_if(|host: Option<Res<Host>>, kept: Res<Kept>| {
                    host.is_some_and(|host| host.is_added() || kept.is_changed())
                }),
            (take, send.run_if(due)).chain().after(load).run_if(joined),
        )
            .run_if(terra_world::instance::in_world),
    )
    .add_systems(
        Last,
        send.before(farewell)
            .run_if(super::closing)
            .run_if(joined)
            .run_if(terra_world::instance::in_world),
    );
    terra_world::remote::method(app, "character.keep", keep_now);
}

/// A player's public key as a save's key.
fn hex(identity: &[u8; 32]) -> String {
    identity.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The public key a save's key names, if it is one.
fn unhex(key: &str) -> Option<[u8; 32]> {
    let mut identity = [0; 32];
    if key.len() != 64 || !key.is_ascii() {
        return None;
    }
    for (byte, pair) in identity.iter_mut().zip(key.as_bytes().chunks(2)) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(identity)
}

/// A record as it crosses, and as the host keeps it for the greeting.
fn written(player: &SavedPlayer) -> Option<Vec<u8>> {
    let bytes = ron::to_string(player).ok()?.into_bytes();
    (bytes.len() <= MOST_BYTES).then_some(bytes)
}

/// The record some bytes are, if they are one.
fn read(bytes: &[u8]) -> Option<SavedPlayer> {
    (bytes.len() <= MOST_BYTES)
        .then(|| ron::de::from_bytes(bytes).ok())
        .flatten()
}

/// On the host, before the tick's telling: a guest's record is kept, for its greeting when
/// it comes again and in the world's save.
fn hear(mut host: ResMut<Host>, mut kept: ResMut<Kept>) {
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        let Some((&CHARACTER, record)) = bytes.split_first() else {
            // Another layer's.
            host.asked.push((peer, bytes));
            continue;
        };
        let (Some(identity), Some(player)) = (host.identity(peer), read(record)) else {
            warn!(
                "A guest's character came as {} bytes that are no record: not kept (D592).",
                record.len()
            );
            continue;
        };
        host.keep(peer, record.to_vec());
        if let Some(file) = kept.file.as_mut() {
            file.guests.insert(hex(&identity), player);
        }
    }
}

/// As hosting begins: the characters the world's save holds are the host's to give back,
/// each put where it stood.
fn restore(mut host: ResMut<Host>, kept: Res<Kept>) {
    let guests = kept.file.iter().flat_map(|file| &file.guests);
    let records: Vec<_> = guests
        .filter_map(|(key, player)| Some((unhex(key)?, written(player)?, Vec3::from(player.at))))
        .collect();
    host.restore(records);
}

type Parts<'a> = (
    &'a mut Transform,
    (&'a mut Vitals, &'a mut Stamina),
    &'a mut Skills,
    &'a mut Backpack,
    &'a mut Gear,
    &'a mut Bar,
    &'a mut Marks,
);

/// In a joined game, once: the character the host kept is this player's, in place of
/// whatever this machine's own save put there.
fn take(
    mut link: ResMut<Link>,
    options: Res<Options>,
    mut items: ResMut<Items>,
    mut view: Single<&mut View, IsLocalPlayer>,
    player: Single<Parts, IsLocalPlayer>,
) {
    let Some(record) = link.take_kept() else {
        return;
    };
    let Some(kept) = read(&record) else {
        warn!("The host's record of this character could not be read: begun afresh.");
        return;
    };
    let (transform, (vitals, stamina), skills, backpack, gear, bar, marks) = player.into_inner();
    put_player(
        &kept,
        &options,
        &mut items,
        &mut view,
        (
            transform, vitals, stamina, skills, backpack, gear, bar, marks,
        ),
    );
    info!("The host kept this character: taken up as it was left.");
}

/// Whether a record is due to the host.
fn due(time: Res<Time>, mut since: Local<f32>) -> bool {
    *since += time.delta_secs();
    if *since < EVERY_S {
        return false;
    }
    *since = 0.0;
    true
}

/// In a joined game: the player's record, asked of the host to keep.
fn send(
    mut link: ResMut<Link>,
    items: Res<Items>,
    view: Single<&View, IsLocalPlayer>,
    player: Single<PlayerParts, IsLocalPlayer>,
) {
    let Some(record) = written(&saved_player(&items, &view, &player.into_inner())) else {
        return;
    };
    let mut bytes = vec![CHARACTER];
    bytes.extend(record);
    link.ask(&bytes);
}

/// `character.keep`: in a joined game, sends the player's record to the host now, as it is
/// sent every few seconds, so a scripted run need not wait.
fn keep_now(
    In(_): In<Option<serde_json::Value>>,
    link: Option<ResMut<Link>>,
    items: Res<Items>,
    view: Option<Single<&View, IsLocalPlayer>>,
    player: Option<Single<PlayerParts, IsLocalPlayer>>,
) -> BrpResult {
    let (Some(link), Some(view), Some(player)) = (link, view, player) else {
        return Err(terra_world::remote::refused(
            "This game is not joined to a host with a player in its world: join one first (net.join).",
        ));
    };
    send(link, items, view, player);
    Ok(serde_json::json!("sent"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_players_key_is_a_saves_key_and_back() {
        let identity: [u8; 32] = std::array::from_fn(|at| (at * 7 + 3) as u8);
        assert_eq!(unhex(&hex(&identity)), Some(identity));
        assert_eq!(unhex("not a key"), None);
        assert_eq!(unhex(&"zz".repeat(32)), None);
    }

    /// What a guest sends is kept only if it reads as a record no longer than one could be
    /// (D51: a stranger's bytes cost only their sender).
    #[test]
    fn only_a_record_is_kept() {
        assert!(read(b"nonsense").is_none());
        assert!(read(&vec![b' '; MOST_BYTES + 1]).is_none());
        let player = SavedPlayer {
            at: [1.0, 2.0, 3.0],
            facing: [0.0, 0.0, 1.0],
            health: 80.0,
            stamina: 50.0,
            fed: 60.0,
            skills: vec![("Woodcutting".into(), 0.5)],
            worn: vec![None],
            worn_held: vec![0],
            worn_wear: vec![0],
            backpack: Vec::new(),
            bar: vec![None],
            picked: 0,
            marks: vec!["troll".into()],
        };
        let bytes = written(&player).expect("a record is short enough");
        assert_eq!(read(&bytes), Some(player));
    }
}
