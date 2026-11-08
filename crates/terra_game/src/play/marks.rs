//! Marks (D590): facts on a character, each imprinted by something done (beating the demo's
//! boss), that take no pack space and cannot be dropped, stored, traded or lost on a death.
//! A small general record: which marks there are is data (`data/marks.ron`, D136), and what
//! a mark does is asked of it by whatever reads it (a shut gate asks whether any mark its
//! bearer has opens gates).
//!
//! Kept with the character (`save`), so on this machine until the server keeps characters
//! (D592). A scripted run gives one with `marks.give`, so testing a gate never means beating
//! the boss (v0-goals, Proof).

use super::Player;
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::BTreeSet;
use terra_walker::IsLocalPlayer;

mod shown;

/// The marks a character bears, by key.
#[derive(Component, Default, Clone, Debug, PartialEq)]
pub struct Marks(pub BTreeSet<String>);

/// A kind of mark, as `data/marks.ron` gives it.
#[derive(Deserialize, Clone, Debug)]
pub struct Kind {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub opens_gates: bool,
    /// Its glyph, shown on the bearer's arm (`shown`): a picture among the assets, white on
    /// nothing (`tools/characters/marks.ts`); none, and it is not shown.
    #[serde(default)]
    pub shown: Option<String>,
}

/// Every kind of mark.
#[derive(Resource, Debug)]
pub struct Kinds(Vec<Kind>);

impl Kinds {
    pub const FILE: &str = "marks.ron";

    pub fn read() -> Self {
        Self(terra_world::data::read(Self::FILE))
    }

    pub fn get(&self, key: &str) -> Option<&Kind> {
        self.0.iter().find(|kind| kind.key == key)
    }

    /// The mark of a bearer's that opens gates, if they have one.
    pub fn opens_gates(&self, marks: &Marks) -> Option<&Kind> {
        self.0
            .iter()
            .find(|kind| kind.opens_gates && marks.0.contains(&kind.key))
    }

    /// The glyph a bearer shows on their arm: that of the first mark they bear that has one.
    pub fn shown(&self, marks: &Marks) -> Option<&str> {
        self.0
            .iter()
            .filter(|kind| marks.0.contains(&kind.key))
            .find_map(|kind| kind.shown.as_deref())
    }

    /// Imprints a mark, by key: one of the kinds there are, or an error naming them.
    pub fn imprint(&self, marks: &mut Marks, key: &str) -> Result<&Kind, String> {
        let Some(kind) = self.get(key) else {
            let known: Vec<&str> = self.0.iter().map(|kind| kind.key.as_str()).collect();
            return Err(format!(
                "There is no mark called \"{key}\": data/marks.ron has {}.",
                known.join(", ")
            ));
        };
        marks.0.insert(kind.key.clone());
        Ok(kind)
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Kinds::read())
        .register_required_components::<Player, Marks>();
    shown::plugin(app);
    app.add_systems(
        Update,
        (
            earned,
            earned_with_the_host.run_if(resource_exists::<terra_walker::net::link::Link>),
        )
            .run_if(resource_exists::<Kinds>),
    );
    terra_world::remote::method(app, "marks.give", give);
    terra_world::remote::method(app, "marks.state", state);
}

#[derive(Deserialize)]
struct Give {
    mark: String,
}

/// Those who stand by when a creature falls whose kind marks its slayers bear its mark
/// (D590): everyone within this of it, in metres, as a boss's fall counts for the party in
/// shipped games, not for whoever dealt the last blow.
const BY_M: f32 = 60.0;

/// A creature fell whose kind imprints a mark (`data/creatures.ron`'s `marks`): the players
/// by it bear it from now on, and are told.
fn earned(
    mut struck: MessageReader<terra_creatures::combat::Struck>,
    bestiary: Res<terra_creatures::species::Bestiary>,
    kinds: Res<Kinds>,
    mut players: Query<(Entity, &Transform, &mut Marks), With<Player>>,
    mut told: MessageWriter<terra_survival::told::Told>,
    mut host: Option<ResMut<terra_walker::net::Host>>,
) {
    for fell in struck.read().filter(|blow| blow.killed) {
        let Some(key) = bestiary.get(fell.species).marks.as_deref() else {
            continue;
        };
        // Hosting, the fall is ordered where it happened, so the joined players by it are
        // marked in their own games, where their marks are kept for now (D592).
        if let Some(host) = host.as_mut() {
            let mut order = vec![MARKED];
            order.extend(key.bytes().take(MOST_KEY));
            host.order(fell.at, &order);
        }
        for (player, at, mut marks) in &mut players {
            if at.translation.distance(fell.at) > BY_M || marks.0.contains(key) {
                continue;
            }
            match kinds.imprint(&mut marks, key) {
                Ok(kind) => {
                    info!("A player bears {} now.", kind.name);
                    told.write(terra_survival::told::Told::new(
                        player,
                        format!("You bear {}", kind.name),
                    ));
                }
                Err(problem) => error!("{problem}"),
            }
        }
    }
}

/// The first byte of a mark earned as a host orders it, the mark's key after it, and the
/// longest a key is read.
pub const MARKED: u8 = 20;
const MOST_KEY: usize = 64;

/// On a joined game: a mark the host orders earned where this game's player is near
/// (an operation is told by area) is imprinted on them, and they are told.
fn earned_with_the_host(
    mut link: ResMut<terra_walker::net::link::Link>,
    kinds: Res<Kinds>,
    mut player: Query<(Entity, &mut Marks), IsLocalPlayer>,
    mut told: MessageWriter<terra_survival::told::Told>,
) {
    use terra_net::client::Event as Heard;
    let (orders, rest): (Vec<Heard>, Vec<Heard>) =
        std::mem::take(&mut link.events).into_iter().partition(
            |event| matches!(event, Heard::Op { bytes, .. } if bytes.first() == Some(&MARKED)),
        );
    link.events = rest;
    let Ok((entity, mut marks)) = player.single_mut() else {
        return;
    };
    for order in orders {
        let Heard::Op { bytes, .. } = order else {
            continue;
        };
        let key = String::from_utf8_lossy(&bytes[1..bytes.len().min(1 + MOST_KEY)]).into_owned();
        if marks.0.contains(&key) {
            continue;
        }
        if let Ok(kind) = kinds.imprint(&mut marks, &key) {
            told.write(terra_survival::told::Told::new(
                entity,
                format!("You bear {}", kind.name),
            ));
        }
    }
}

/// `marks.give {"mark": "troll"}`: imprints a mark on this machine's player.
fn give(
    In(params): In<Option<serde_json::Value>>,
    kinds: Res<Kinds>,
    mut marks: Option<Single<&mut Marks, IsLocalPlayer>>,
) -> bevy::remote::BrpResult {
    use terra_world::remote::refused;
    let Give { mark } = bevy::remote::builtin_methods::parse_some(params)?;
    let marks = marks
        .as_mut()
        .ok_or_else(|| refused("There is no player to mark yet."))?;
    let kind = kinds.imprint(marks, &mark).map_err(refused)?;
    Ok(serde_json::json!({ "marked": kind.key }))
}

/// `marks.state`: the marks this machine's player bears.
fn state(
    In(_): In<Option<serde_json::Value>>,
    marks: Option<Single<&Marks, IsLocalPlayer>>,
) -> bevy::remote::BrpResult {
    let marks = marks.ok_or_else(|| terra_world::remote::refused("There is no player yet."))?;
    Ok(serde_json::json!({ "marks": marks.0 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_has_the_trolls_mark_and_it_opens_gates() {
        let kinds = Kinds::read();
        let mut marks = Marks::default();
        assert!(
            kinds.opens_gates(&marks).is_none(),
            "an unmarked player opens gates"
        );
        let kind = kinds.imprint(&mut marks, "troll").unwrap();
        assert!(kind.opens_gates);
        assert_eq!(
            kinds.opens_gates(&marks).map(|kind| kind.key.as_str()),
            Some("troll")
        );
        // Imprinted twice it is borne once; a mark the data does not have is refused by name.
        kinds.imprint(&mut marks, "troll").unwrap();
        assert_eq!(marks.0.len(), 1);
        let refused = kinds.imprint(&mut marks, "crown").unwrap_err();
        assert!(
            refused.contains("crown") && refused.contains("troll"),
            "{refused}"
        );
    }
}
