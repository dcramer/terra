//! The wardrobe catalogue (D611): the bodies, pieces and looks characters on Quaternius's
//! bodies are composed from, as data (`models/character/wardrobe.json`, its schema in its
//! `about`), and the rule that composes one (`Wardrobe::compose`). A figure is a piece a slot,
//! each its own file bound to the body's skeleton as any worn look is (`character::wear`):
//! Synty Sidekick's shape, with WoW's rule of one geoset a region. Nothing names a piece in
//! code, so a new piece or look is a row and a file (D136).
//!
//! The rule: each slot shows the look's piece when its item is worn, else another of the
//! body's pieces for a worn item in that slot, the first that the pairing matrix does not
//! refuse with a piece already shown, else the body's bare piece (D600); the head
//! shows unless a shown piece holds the head's region (their UpperBody, the bare arms, carries
//! the head too). So every region of the body is drawn exactly once, which the tests hold for
//! every look in every state of undress.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

/// The catalogue the game ships with.
pub fn wardrobe() -> &'static Wardrobe {
    static WARDROBE: OnceLock<Wardrobe> = OnceLock::new();
    WARDROBE.get_or_init(|| {
        Wardrobe::parse(include_str!(
            "../../../assets/models/character/wardrobe.json"
        ))
        .unwrap_or_else(|error| {
            bevy::log::error!("models/character/wardrobe.json: {error}");
            Wardrobe::default()
        })
    })
}

/// The catalogue: its slots, the regions of a body, and its bodies, pieces and looks by name.
#[derive(Debug, Default, Deserialize)]
pub struct Wardrobe {
    pub slots: Vec<String>,
    pub regions: Vec<String>,
    pub bodies: BTreeMap<String, Body>,
    pub pieces: BTreeMap<String, Piece>,
    pub looks: BTreeMap<String, Look>,
    /// Two pieces of different slots and whether they may be worn together, measured through
    /// the clips (`tools/characters/pairings.ts`), never written by hand.
    #[serde(default)]
    pub pairings: Vec<Pairing>,
}

/// A row of the pairing matrix; its numbers stay in the file for whoever reads it.
#[derive(Debug, Deserialize)]
pub struct Pairing {
    pub pieces: [String; 2],
    /// `apart`, `theirs`, `pass` or `refused`.
    pub verdict: String,
}

/// A body of the wardrobe.
#[derive(Debug, Deserialize)]
pub struct Body {
    /// Its file under `assets/`: the skeleton, sockets and clips, no mesh.
    pub model: String,
    /// The folder its pieces' files are in, each `<piece>.gltf`.
    pub pieces: String,
    /// Its head piece.
    pub head: String,
    /// Its bare piece, by slot.
    pub bare: HashMap<String, String>,
    /// The look it wears when nothing says otherwise.
    pub look: String,
}

/// A piece: one mesh file worn in a slot, made on one body.
#[derive(Debug, Deserialize)]
pub struct Piece {
    pub body: String,
    /// Its slot; none for the head.
    #[serde(default)]
    pub slot: Option<String>,
    /// The item it is the look of (`character::item_id`); none for bare skin.
    #[serde(default)]
    pub item: Option<String>,
    /// The body's regions it stands for, so hides.
    pub regions: Vec<String>,
}

/// A look: a piece a slot.
#[derive(Debug, Deserialize)]
pub struct Look {
    pub body: String,
    pub pieces: HashMap<String, String>,
}

impl Wardrobe {
    /// Reads a catalogue and checks that every name in it is one of its own.
    pub fn parse(text: &str) -> Result<Wardrobe, String> {
        let w: Wardrobe = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let piece = |name: &str, body: &str, slot: Option<&str>| -> Result<(), String> {
            let p = w
                .pieces
                .get(name)
                .ok_or_else(|| format!("no piece {name}"))?;
            if p.body != body || p.slot.as_deref() != slot {
                return Err(format!(
                    "{name} is made on {} for slot {:?}, not on {body} for {slot:?}",
                    p.body, p.slot
                ));
            }
            Ok(())
        };
        for (name, body) in &w.bodies {
            piece(&body.head, name, None)?;
            for slot in &w.slots {
                let bare = body
                    .bare
                    .get(slot)
                    .ok_or_else(|| format!("body {name} has no bare {slot}"))?;
                piece(bare, name, Some(slot))?;
            }
            if !w.looks.contains_key(&body.look) {
                return Err(format!("body {name}'s look {} is not a look", body.look));
            }
        }
        for (name, look) in &w.looks {
            if !w.bodies.contains_key(&look.body) {
                return Err(format!("look {name}'s body {} is not a body", look.body));
            }
            for (slot, p) in &look.pieces {
                piece(p, &look.body, Some(slot))?;
            }
        }
        for (name, p) in &w.pieces {
            if !w.bodies.contains_key(&p.body) {
                return Err(format!("piece {name}'s body {} is not a body", p.body));
            }
            if let Some(region) = p.regions.iter().find(|r| !w.regions.contains(r)) {
                return Err(format!("piece {name}'s region {region} is not a region"));
            }
            if let Some(slot) = p.slot.as_ref().filter(|s| !w.slots.contains(s)) {
                return Err(format!("piece {name}'s slot {slot} is not a slot"));
            }
        }
        for pairing in &w.pairings {
            if let Some(p) = pairing.pieces.iter().find(|p| !w.pieces.contains_key(*p)) {
                return Err(format!("a pairing names {p}, which is not a piece"));
            }
        }
        for (name, look) in &w.looks {
            let pieces: Vec<&String> = look.pieces.values().collect();
            for (i, a) in pieces.iter().enumerate() {
                if let Some(b) = pieces[i + 1..].iter().find(|b| w.refused(a, b)) {
                    return Err(format!("look {name} wears {a} with {b}, a refused pairing"));
                }
            }
        }
        Ok(w)
    }

    /// Whether the pairing matrix refuses `a` worn with `b`.
    pub fn refused(&self, a: &str, b: &str) -> bool {
        self.pairings.iter().any(|p| {
            p.verdict == "refused"
                && ((p.pieces[0] == a && p.pieces[1] == b)
                    || (p.pieces[0] == b && p.pieces[1] == a))
        })
    }

    /// The wardrobe body whose file is `model` (a path under `assets/`, with or without a `#`
    /// label), and its name.
    pub fn body_for_model(&self, model: &str) -> Option<(&str, &Body)> {
        let model = model.split('#').next().unwrap_or(model);
        self.bodies
            .iter()
            .find(|(_, body)| body.model == model)
            .map(|(name, body)| (name.as_str(), body))
    }

    /// The pieces `body` shows in `look`, wearing the items `worn` (`character::item_id`s), or
    /// the look whole when `worn` is none: a piece a slot, in the catalogue's order of slots,
    /// then the head if no piece shown holds its region.
    pub fn compose<'a>(&'a self, body: &str, look: &str, worn: Option<&[String]>) -> Vec<&'a str> {
        let Some(b) = self.bodies.get(body) else {
            return Vec::new();
        };
        let look = self.looks.get(look).filter(|l| l.body == body);
        let wears = |p: &Piece| match (worn, &p.item) {
            (None, _) => true,
            (Some(worn), Some(item)) => worn.contains(item),
            (Some(_), None) => false,
        };
        let mut shown: Vec<&str> = Vec::new();
        for slot in &self.slots {
            let of_look = look
                .and_then(|l| l.pieces.get(slot))
                .and_then(|name| self.pieces.get_key_value(name.as_str()))
                .filter(|(_, p)| wears(p));
            let others = self.pieces.iter().filter(|(_, p)| {
                worn.is_some()
                    && p.body == body
                    && p.slot.as_ref() == Some(slot)
                    && p.item.is_some()
                    && wears(p)
            });
            // The first that pairs with every piece shown so far: a refused pairing is never
            // composed, so a worn item with no piece that pairs shows bare.
            let chosen = of_look
                .into_iter()
                .chain(others)
                .map(|(name, _)| name.as_str())
                .find(|name| !shown.iter().any(|s| self.refused(s, name)))
                .or_else(|| b.bare.get(slot).map(String::as_str));
            shown.extend(chosen);
        }
        let head = self.pieces.get(&b.head);
        let held: BTreeSet<&str> = shown
            .iter()
            .filter_map(|name| self.pieces.get(*name))
            .flat_map(|p| p.regions.iter().map(String::as_str))
            .collect();
        if head.is_some_and(|h| !h.regions.iter().any(|r| held.contains(r.as_str()))) {
            shown.push(&b.head);
        }
        shown
    }

    /// A piece's file under `assets/`.
    pub fn file(&self, piece: &str) -> Option<String> {
        let p = self.pieces.get(piece)?;
        Some(format!("{}/{piece}.gltf", self.bodies.get(&p.body)?.pieces))
    }
}

#[cfg(test)]
mod tests;
