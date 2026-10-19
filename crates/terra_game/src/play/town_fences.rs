//! A town's fences drawn and made solid (D346): each module the layout gives
//! (`terra_town::town::Fence`) is its model fitted between the module's ends, as the model
//! contract fits a module along +X from its start socket to its end (D303), stretched only
//! along its line; and a capsule along it among the obstacles, so the player and every body
//! are stopped by it as by a trunk.

use super::catalogue::Builds;
use super::flora::Obstacles;
use super::props::Models;
use crate::planet::Planet;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use terra_core::{Piece, Solid};
use terra_town::town::{FURNISHING, Fence, FenceKind};

/// A module of a town's fences, drawn.
#[derive(Component)]
pub struct TownFence;

/// How far round its line a fence stops a body, and how tall it stands.
const FENCE_REACH_M: f32 = 0.1;

/// The catalogue item a fence of a kind is drawn as, the length between its sockets, and
/// how tall it stands.
fn module_of(kind: FenceKind) -> (&'static str, f32, f32) {
    match kind {
        FenceKind::Wattle => ("WattleFence", FURNISHING.wattle_m, 1.05),
        FenceKind::Rail => ("RailFence", FURNISHING.rail_m, 1.15),
    }
}

/// Draws every fence module at its place on the ground, `to_world` giving a point of the
/// town's plan on the planet's ground, and files each among the obstacles. How many it laid.
pub fn lay(
    commands: &mut Commands,
    models: &Models,
    builds: &Builds,
    planet: &Planet,
    mut obstacles: Option<&mut Obstacles>,
    to_world: impl Fn(Vec2) -> Vec3,
    fences: &[Fence],
) -> usize {
    let mut laid = 0;
    for fence in fences {
        let (file, module_m, tall) = module_of(fence.kind);
        let Some(item) = builds.item_from(file) else {
            error!("Town: the town wants {file}, which is not in the catalogue.");
            return laid;
        };
        let Some(model) = models.of(&item) else {
            continue;
        };
        let (a, b) = (to_world(fence.from), to_world(fence.to));
        let middle = to_world((fence.from + fence.to) * 0.5);
        let up = middle.normalize();
        // Stood on the lowest of its ends and middle, a little sunk, so neither end floats.
        let lowest = a.length().min(b.length()).min(middle.length());
        let foot = up * (lowest - 0.04);
        let along = (b - a)
            .reject_from(up)
            .normalize_or(up.any_orthonormal_vector());
        let rotation = Quat::from_mat3(&Mat3::from_cols(along, up, along.cross(up)));
        let stretch = fence.from.distance(fence.to) / module_m;
        let mut drawn = commands.spawn((
            Name::new(file),
            TownFence,
            Transform::from_translation(foot)
                .with_rotation(rotation)
                .with_scale(Vec3::new(stretch, 1.0, 1.0)),
            Visibility::default(),
        ));
        for piece in &model.pieces {
            let mesh = (
                Mesh3d(piece.mesh.clone()),
                MeshMaterial3d(piece.material.clone()),
            );
            if piece.glows {
                drawn.with_child((mesh, NotShadowCaster));
            } else {
                drawn.with_child(mesh);
            }
        }
        if let Some(obstacles) = obstacles.as_deref_mut() {
            let ground = planet.ground_at(middle);
            let frame = Solid::of_pieces(ground, Vec::new());
            let line = Piece {
                hull: vec![frame.plan(a).0, frame.plan(b).0],
                radius: FENCE_REACH_M,
                top: tall,
                crest: tall,
            };
            obstacles.add_solid(ground, Solid::of_pieces(ground, vec![line]));
        }
        laid += 1;
    }
    laid
}
