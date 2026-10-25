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
use terra_build_layout::solids::Slab;
use terra_motion::mover::{Block, Capsule, Kind};
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

/// The props within a reach of a point, as boxes, added to a list: a hidden one (a ghost, one
/// carried) is walked through.
pub fn props<'a>(
    solids: impl IntoIterator<Item = (&'a bodies::Solid, &'a Transform, &'a Visibility)>,
    at: Vec3,
    reach: f32,
    out: &mut Vec<Block>,
) {
    out.extend(
        solids
            .into_iter()
            .filter(|(_, _, shown)| **shown != Visibility::Hidden)
            .map(|(solid, transform, _)| prop(solid, transform))
            .filter(|block| block.centre.distance(at) < reach + block.half.length()),
    );
}

/// The trees, rocks, stumps and logs within a reach of a point, borrowed where flora keeps
/// them and added to a list: `terra_motion::mover::Obstacles` over these asks them in place.
pub fn flora<'a>(
    obstacles: &'a flora::Obstacles,
    at: Vec3,
    reach: f32,
    out: &mut Vec<&'a terra_core::Solid>,
) {
    out.extend(obstacles.solids_near(at, reach));
}

/// A base's solid parts within a reach of a point, as boxes in the world added to a list: its
/// cubes' layers and its doors' and shutters' leaves, with the kits it is laid by, placed by its
/// frame (a turn and a place; a frame is never scaled). `slabs` is room to work in, kept by the
/// caller.
pub struct AtBase<'a> {
    pub base: &'a Base,
    pub frame: &'a Transform,
    pub kits: Option<&'a Kits>,
}

pub fn base(at_base: AtBase, at: Vec3, reach: f32, slabs: &mut Vec<Slab>, out: &mut Vec<Block>) {
    let AtBase { base, frame, kits } = at_base;
    let local = frame.compute_affine().inverse().transform_point3(at);
    slabs.clear();
    terra_build_layout::solids::near_into(base, local, reach, kits, slabs);
    out.extend(slabs.iter().map(|slab| Block {
        centre: frame.transform_point(slab.centre),
        axes: slab.axes.map(|axis| frame.rotation * axis),
        half: slab.half,
    }));
}

/// Other bodies (creatures, people) as the mover's pushboxes, each by an ID the walk gives
/// it: a capsule standing at its place as wide as its circle and as tall as the kind, still.
/// They are not the mover's bodies, so they are not pushed by it: a creature stops a body at
/// its width, and a person (D479) only at the kind's core, giving way by itself
/// (`bodies::keep_apart`).
pub fn others<'a>(
    bodies: impl IntoIterator<Item = (u32, &'a bodies::Body, &'a Transform)>,
    kind: &Kind,
    out: &mut Vec<(u32, Capsule, Vec3)>,
) {
    out.extend(bodies.into_iter().map(|(id, body, transform)| {
        let feet = transform.translation;
        let up = feet.normalize_or(Vec3::Y);
        // A person gives way (D479): met only by a man's core, the rest left to them to part.
        let radius = if body.soft() {
            kind.core_m.min(body.radius)
        } else {
            body.radius
        };
        (
            id,
            Capsule::standing(feet, up, radius, kind.height),
            Vec3::ZERO,
        )
    }));
}

/// How far from the centre the water's surface over a point is, where the ground there is
/// under water (the sea or a lake, D322); none on dry land. The mover's `World.water`.
pub fn water(planet: &Planet, at: Vec3) -> Option<f32> {
    planet.under_sea(at).then(|| planet.water_radius_m(at))
}

/// A Bevy task pool as the mover's lanes' executor (D53, `Bodies::tick_on`): the game's is its
/// compute pool, whose threads also run its systems, so the bodies' tick shares them rather than
/// adding a pool of its own beside them. The calling thread runs lanes too while it waits.
pub struct OnTasks<'a>(pub &'a bevy::tasks::TaskPool);

impl terra_motion::mover::Executor for OnTasks<'_> {
    fn each(&self, lanes: usize, lane: &(dyn Fn(usize) + Sync)) {
        self.0.scope(|scope| {
            for k in 0..lanes {
                scope.spawn(async move { lane(k) });
            }
        });
    }
}

#[cfg(test)]
#[path = "mover_world_tests.rs"]
mod tests;
