//! The game's world as the mover asks it (D53, `terra_motion::mover`; docs/controller.md
//! "What stops a body"): each source of what stops or carries a body, gathered near it, in
//! the mover's own shapes, so the walk chains them into one `World.ground` (`(&a, &b)` pairs)
//! with the planet's field.
//!
//! - props: a `bodies::Solid`'s box from its foot, turned as it stands; hidden ones none.
//! - flora: trees', rocks', stumps' and logs' solids, borrowed from their index, not copied.
//! - a base: its cubes' solid layers and its doors' leaves (`terra_build_layout::solids`), put
//!   into the world by the build's frame.
//! - other bodies: creatures' and people's circles as standing capsules, that do not give way.
//! - water: the sea's or a lake's surface where the ground is under it.

use crate::bodies;
use bevy::prelude::*;
use terra_build::Base;
use terra_build::kit::Kits;
use terra_motion::mover::{Block, Capsule};
use terra_world::flora;
use terra_world::planet::Planet;

/// A prop's box as the mover's: `bodies::Solid` stands from its transform's place, `half.y`
/// twice up, turned as it is (its scale is not its size).
pub fn prop(solid: &bodies::Solid, transform: &Transform) -> Block {
    let turn = transform.rotation;
    Block {
        centre: transform.translation + turn * Vec3::Y * solid.half.y,
        axes: [turn * Vec3::X, turn * Vec3::Y, turn * Vec3::Z],
        half: solid.half,
    }
}

/// The props within a reach of a point, as boxes: a hidden one (a ghost, one carried) is
/// walked through.
pub fn props<'a>(
    solids: impl IntoIterator<Item = (&'a bodies::Solid, &'a Transform, &'a Visibility)>,
    at: Vec3,
    reach: f32,
) -> Vec<Block> {
    solids
        .into_iter()
        .filter(|(_, _, shown)| **shown != Visibility::Hidden)
        .map(|(solid, transform, _)| prop(solid, transform))
        .filter(|block| block.centre.distance(at) < reach + block.half.length())
        .collect()
}

/// The trees, rocks, stumps and logs within a reach of a point, borrowed where flora keeps
/// them: `terra_motion::mover::Obstacles` over these asks them in place.
pub fn flora(obstacles: &flora::Obstacles, at: Vec3, reach: f32) -> Vec<&terra_core::Solid> {
    obstacles.solids_near(at, reach).collect()
}

/// A base's solid parts within a reach of a point, as boxes in the world: its cubes' layers
/// and its doors' and shutters' leaves, with the kits it is laid by, placed by its frame (a
/// turn and a place; a frame is never scaled).
pub fn base(
    base: &Base,
    frame: &Transform,
    at: Vec3,
    reach: f32,
    kits: Option<&Kits>,
) -> Vec<Block> {
    let local = frame.compute_affine().inverse().transform_point3(at);
    terra_build_layout::solids::near(base, local, reach, kits)
        .into_iter()
        .map(|slab| Block {
            centre: frame.transform_point(slab.centre),
            axes: slab.axes.map(|axis| frame.rotation * axis),
            half: slab.half,
        })
        .collect()
}

/// Other bodies (creatures, people) as the mover's pushboxes, each by an ID the walk gives
/// it: a capsule standing at its place as wide as its circle and so tall, still. They are not
/// the mover's bodies, so they stop a body and are not pushed by it (open question 4).
pub fn others<'a>(
    bodies: impl IntoIterator<Item = (u32, &'a bodies::Body, &'a Transform)>,
    height: f32,
) -> Vec<(u32, Capsule, Vec3)> {
    bodies
        .into_iter()
        .map(|(id, body, transform)| {
            let feet = transform.translation;
            let up = feet.normalize_or(Vec3::Y);
            (
                id,
                Capsule::standing(feet, up, body.radius, height),
                Vec3::ZERO,
            )
        })
        .collect()
}

/// How far from the centre the water's surface over a point is, where the ground there is
/// under water (the sea or a lake, D322); none on dry land. The mover's `World.water`.
pub fn water(planet: &Planet, at: Vec3) -> Option<f32> {
    planet.under_sea(at).then(|| planet.water_radius_m(at))
}

#[cfg(test)]
#[path = "mover_world_tests.rs"]
mod tests;
