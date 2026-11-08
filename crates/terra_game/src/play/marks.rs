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
    terra_world::remote::method(app, "marks.give", give);
    terra_world::remote::method(app, "marks.state", state);
}

#[derive(Deserialize)]
struct Give {
    mark: String,
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
