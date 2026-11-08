//! The save's two kinds of record (docs/instances.md "Saving and networking", D471): the
//! character's (their body and what they carry, which world they are in, and the way back
//! through the gates they came by) and each world's own (`Instance`: its base, props, people,
//! ground, felled trees and clock), by the world's key. One file holds them all, so a
//! crossing, a save and a load are one path: a world left is kept here in memory as it is
//! left, written on the save's schedule, and read back when it is entered again.

use super::{Instance, SavedPlayer, VERSION};
use crate::play::portal::{Return, Returns};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use terra_world::instance::{InstanceKey, WorldSpec};

/// A world as a save names it: all that makes it again (D47).
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub(super) struct SavedWorld {
    pub key: u64,
    pub kind: String,
    pub seed: u64,
}

impl SavedWorld {
    pub fn of(spec: &WorldSpec) -> Self {
        Self {
            key: spec.key.0,
            kind: spec.kind.clone(),
            seed: spec.seed,
        }
    }

    pub fn spec(&self) -> WorldSpec {
        WorldSpec {
            key: InstanceKey(self.key),
            kind: self.kind.clone(),
            seed: self.seed,
        }
    }
}

/// A step of the way back (`portal::Return`).
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub(super) struct SavedReturn {
    world: SavedWorld,
    at: [f32; 3],
    toward: [f32; 3],
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(super) struct SaveFile {
    pub version: u32,
    pub player: SavedPlayer,
    /// The world the player is in.
    pub here: SavedWorld,
    /// The way back through the gates come by, the last crossed first to return (D470).
    #[serde(default)]
    pub returns: Vec<SavedReturn>,
    /// Each world's own record, by its key: every world the player has been in.
    pub instances: BTreeMap<u64, Instance>,
    /// The characters of those who have joined this world, each by who they are (their
    /// public key, in hex): kept by the host with its world (D592, `save::guest`).
    #[serde(default)]
    pub guests: BTreeMap<String, SavedPlayer>,
}

/// The save as last read or gathered, every world's record with it, while a save is chosen;
/// and whether the player has been put where it says yet.
#[derive(Resource, Default)]
pub(in crate::play) struct Kept {
    pub(super) file: Option<SaveFile>,
    pub(super) placed: bool,
}

impl Kept {
    /// Takes what was gathered of the player and the world they are in: that world's record
    /// replaced, the others kept as they were left.
    pub(super) fn keep(
        &mut self,
        spec: &WorldSpec,
        returns: &Returns,
        (player, instance): (SavedPlayer, Instance),
    ) -> &SaveFile {
        // What is gathered is the player where they stand: there is nowhere left to put them.
        self.placed = true;
        let here = SavedWorld::of(spec);
        let returns = returns
            .0
            .iter()
            .map(|back| SavedReturn {
                world: SavedWorld::of(&back.world),
                at: back.at.into(),
                toward: back.toward.into(),
            })
            .collect();
        let (mut instances, guests) = self
            .file
            .take()
            .map(|file| (file.instances, file.guests))
            .unwrap_or_default();
        instances.insert(here.key, instance);
        self.file.insert(SaveFile {
            version: VERSION,
            player,
            here,
            returns,
            instances,
            guests,
        })
    }

    /// The way back as the save has it.
    pub(super) fn returns(&self) -> Vec<Return> {
        let saved = self.file.as_ref().map(|file| file.returns.as_slice());
        saved
            .unwrap_or_default()
            .iter()
            .map(|back| Return {
                world: back.world.spec(),
                at: back.at.into(),
                toward: back.toward.into(),
            })
            .collect()
    }
}

/// Reads a save, saying in the log why not if it cannot be.
pub(super) fn read(path: &Path) -> Option<SaveFile> {
    let Ok(text) = std::fs::read_to_string(path) else {
        info!("No saved game at {}; starting afresh.", path.display());
        return None;
    };
    // Its version first, so a save of another build is refused as one, not as unreadable.
    #[derive(Deserialize)]
    struct Versioned {
        version: u32,
    }
    if let Ok(Versioned { version }) = ron::from_str::<Versioned>(&text)
        && version != VERSION
    {
        warn!(
            "The saved game at {} was saved by an older build; start a new game.",
            path.display()
        );
        return None;
    }
    match ron::from_str(&text) {
        Ok(save) => Some(save),
        Err(problem) => {
            warn!(
                "Could not read the saved game at {}: {problem}",
                path.display()
            );
            None
        }
    }
}
