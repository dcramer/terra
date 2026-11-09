//! The leaf, frond and flower cards grown plants are drawn from: one atlas a realm, its
//! cells and outlines read from the data `tools/textures`' cards step writes.

use crate::data;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use serde::Deserialize;

/// One card in the atlas: the top-left of its cell and how far across the cell is, the
/// outline round its leaves in the atlas's coordinates, their average colour, linear, and,
/// for a composed card, how long a leaf or needle on it is as a share of its width, so a
/// plant can be grown with its leaves at their real size (D276).
#[derive(Clone, Debug)]
pub struct Card {
    pub cell: Vec2,
    pub across: f32,
    pub outline: Vec<(f32, f32)>,
    pub colour: Vec3,
    pub leaf: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CardFile {
    atlas: String,
    normals: String,
    cut: f32,
    across: f32,
    cards: HashMap<String, CardEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CardEntry {
    cell: (f32, f32),
    outline: Vec<(f32, f32)>,
    colour: (f32, f32, f32),
    /// How long its leaves are as a share of its width; 0, or left out, for a picture.
    #[serde(default)]
    leaf: f32,
}

/// The cards there are (`assets/data/foliage.ron`), the atlas they are in, and its normal
/// map: each leaf's relief and tilt, in OpenGL's convention, flat for cards drawn from pictures.
#[derive(Resource)]
pub struct Cards {
    pub atlas: String,
    pub normals: String,
    pub cut: f32,
    pub(super) cards: HashMap<String, Card>,
    file: String,
}

impl Cards {
    pub const FILE: &str = "foliage.ron";

    pub fn read() -> Self {
        Self::read_file(Self::FILE)
    }

    /// The cards of another atlas made the same way, such as the forest floor's litter
    /// (`litter.ron`).
    /// The cards a world of a kind grows its plants from: its realm's own atlas where its
    /// row names one (`data/realms.ron`'s `foliage`), the first realm's otherwise.
    pub fn of(kind: &str) -> Self {
        match crate::realm::Realms::shared()
            .of(kind)
            .and_then(|realm| realm.foliage.as_deref())
        {
            Some(leaves) => Self::read_file(&format!("{leaves}.ron")),
            None => Self::read(),
        }
    }

    pub fn read_file(name: &str) -> Self {
        let file: CardFile = data::read(name);
        let cards = file
            .cards
            .into_iter()
            .map(|(name, entry)| {
                let colour = Color::srgb(entry.colour.0, entry.colour.1, entry.colour.2)
                    .to_linear()
                    .to_vec3();
                (
                    name,
                    Card {
                        cell: Vec2::new(entry.cell.0, entry.cell.1),
                        across: file.across,
                        outline: entry.outline,
                        colour,
                        leaf: (entry.leaf > 0.0).then_some(entry.leaf),
                    },
                )
            })
            .collect();
        Self {
            atlas: file.atlas,
            normals: file.normals,
            cut: file.cut,
            cards,
            file: name.to_string(),
        }
    }

    /// A card by name. A name the file lacks is a mistake in the data, caught by a test.
    pub fn get(&self, name: &str) -> &Card {
        self.cards
            .get(name)
            .unwrap_or_else(|| panic!("no card {name} in {}", self.file))
    }

    pub fn has(&self, name: &str) -> bool {
        self.cards.contains_key(name)
    }
}
