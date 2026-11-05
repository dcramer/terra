//! What a saved game keeps of the world itself: how the ground was dug and filled, in order
//! (D21), and the woods' grown trees felled (D622). The rest of the land comes from the
//! world's rules.

use crate::planet::Planet;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use terra_core::{MaterialId, Operation};

/// One change to the ground: a sphere dug out, or filled with a material, or a box dug
/// out.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(super) struct SavedEdit {
    centre: [f32; 3],
    radius: f32,
    /// The material it was filled with, for a fill; nothing for a dig.
    #[serde(default)]
    fill: Option<u16>,
    /// For a box, half its size each way and how it is turned; the radius is then unused.
    #[serde(default)]
    cut: Option<SavedBox>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct SavedBox {
    half: [f32; 3],
    rotation: [f32; 4],
}

/// The ground as it was dug and filled, replayed in order, once, on a world not yet changed.
pub(super) fn replay(planet: &mut Planet, ground: &[SavedEdit]) {
    if !planet.operations().is_empty() || ground.is_empty() {
        return;
    }
    info!("Replayed {} changes to the ground.", ground.len());
    for edit in ground {
        let centre = Vec3::from(edit.centre);
        planet.apply(&match (&edit.cut, edit.fill) {
            (Some(cut), _) => Operation::DigBox {
                centre,
                half: Vec3::from(cut.half),
                rotation: Quat::from_array(cut.rotation),
            },
            (None, None) => Operation::Dig {
                centre,
                radius: edit.radius,
            },
            (None, Some(material)) => Operation::Fill {
                centre,
                radius: edit.radius,
                material: MaterialId(material),
            },
        });
    }
}

/// How the ground has been dug and filled, to save.
pub(super) fn kept(planet: &Planet) -> Vec<SavedEdit> {
    planet
        .operations()
        .into_iter()
        .filter_map(|operation| match operation {
            Operation::Dig { centre, radius } => Some(SavedEdit {
                centre: centre.into(),
                radius,
                fill: None,
                cut: None,
            }),
            Operation::DigBox {
                centre,
                half,
                rotation,
            } => Some(SavedEdit {
                centre: centre.into(),
                radius: 0.0,
                fill: None,
                cut: Some(SavedBox {
                    half: half.into(),
                    rotation: rotation.to_array(),
                }),
            }),
            Operation::Fill {
                centre,
                radius,
                material,
            } => Some(SavedEdit {
                centre: centre.into(),
                radius,
                fill: Some(material.0),
                cut: None,
            }),
            _ => None,
        })
        .collect()
}

/// The woods' grown trees felled, by where their feet stood, felled again as the game is
/// loaded: whether they stand already or are still to stream in (D622).
pub(super) fn fell_again(
    felled: &[[f32; 3]],
    fell: &mut MessageWriter<terra_world::woods::FellAgain>,
) {
    if felled.is_empty() {
        return;
    }
    fell.write(terra_world::woods::FellAgain(
        felled.iter().copied().map(Vec3::from).collect(),
    ));
}
