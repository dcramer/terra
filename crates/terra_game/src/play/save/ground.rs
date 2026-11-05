//! What a saved game keeps of the world itself: how the ground was dug and filled, in order
//! (D21), and the woods' grown trees felled (D622) with the stumps they left. The rest of the
//! land comes from the world's rules.

use crate::planet::Planet;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use terra_core::{MaterialId, Operation};
use terra_world::woods::Stump;

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

/// A stump a tree felled for its logs left, as kept: where its tree's foot stood, how it was
/// turned, how thick its trunk was and whether it was a conifer's.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(super) struct SavedStump {
    at: [f32; 3],
    turn: [f32; 4],
    trunk_m: f32,
    conifer: bool,
}

/// The stumps standing, as saved.
pub(super) fn stumps_kept(stumps: &[Stump]) -> Vec<SavedStump> {
    (stumps.iter())
        .map(|stump| SavedStump {
            at: stump.at.to_array(),
            turn: stump.turn.to_array(),
            trunk_m: stump.trunk_m,
            conifer: stump.conifer,
        })
        .collect()
}

/// The stumps saved, each spawned as a stump still to be drawn, which the forest draws where
/// it stood (`woods::Stump`).
pub(super) fn stumps_again(commands: &mut Commands, stumps: &[SavedStump]) {
    if !stumps.is_empty() {
        info!("Woods: {} stumps left as saved", stumps.len());
    }
    for saved in stumps {
        commands.spawn(Stump {
            at: Vec3::from(saved.at),
            turn: Quat::from_array(saved.turn),
            trunk_m: saved.trunk_m,
            conifer: saved.conifer,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stumps standing, written as a save writes them and read back, are spawned again
    /// as they stood, each a stump still to be drawn.
    #[test]
    fn stumps_saved_come_back_as_they_stood() {
        let stood = [
            Stump {
                at: Vec3::new(3.5, 2001.25, -7.0),
                turn: Quat::from_rotation_y(0.7),
                trunk_m: 0.42,
                conifer: true,
            },
            Stump {
                at: Vec3::new(-12.0, 1999.5, 4.75),
                turn: Quat::from_rotation_x(0.1),
                trunk_m: 0.3,
                conifer: false,
            },
        ];
        let text = ron::to_string(&stumps_kept(&stood)).unwrap();
        let read: Vec<SavedStump> = ron::from_str(&text).unwrap();
        let mut world = World::new();
        stumps_again(&mut world.commands(), &read);
        world.flush();
        let back: Vec<Stump> = world.query::<&Stump>().iter(&world).copied().collect();
        assert_eq!(back.len(), 2);
        for stump in &stood {
            assert!(back.contains(stump), "{stump:?} back in {back:?}");
        }
    }
}
