//! A grown tree felled takes its far card with it. The card stands for the tree past its
//! last level of detail (`Card::grown`), so a tree cut down near the camera would come back
//! as a billboard once walked away from. As an impostor batch is rebuilt when one of its
//! instances goes (SpeedTree's and Unreal's HLOD clusters are rebuilt for what changed), the
//! square holding the felled tree's card is merged again without it, the rest as they were.

use super::{Card, SQUARE_M, foot_key, merged};
use crate::flora::Growth;
use bevy::{camera::primitives::Aabb, platform::collections::HashMap, prelude::*};

/// A grown tree of the woods: where its foot stands, which finds its far card.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct WoodsTree {
    pub foot: Vec3,
}

/// A square of far cards holding a grown tree's card: its cards, to merge again without one.
#[derive(Component, Debug)]
pub(super) struct FarSquare {
    pub cards: Vec<Card>,
}

/// The squares holding grown trees' cards, by where they lie on the woods' map about its
/// middle, east and north.
#[derive(Resource, Debug)]
pub(super) struct FarSquares {
    pub middle: Vec3,
    pub east: Vec3,
    pub north: Vec3,
    pub by_square: HashMap<(i32, i32), Entity>,
}

impl FarSquares {
    pub fn new(middle: Vec3, east: Vec3, north: Vec3) -> Self {
        Self {
            middle,
            east,
            north,
            by_square: HashMap::default(),
        }
    }

    /// The square `at` lies in, as `woods::squares` merges them.
    pub fn square_of(&self, at: Vec3) -> (i32, i32) {
        square_of(self.middle, self.east, self.north, at)
    }
}

/// The square of `SQUARE_M` about `middle` that a tree at `at` is merged in.
pub(super) fn square_of(middle: Vec3, east: Vec3, north: Vec3, at: Vec3) -> (i32, i32) {
    let offset = at - middle;
    let square = (Vec2::new(offset.dot(east), offset.dot(north)) / SQUARE_M)
        .floor()
        .as_ivec2();
    (square.x, square.y)
}

/// Merges again, without its card, each square a grown tree felled since last time stood in
/// (felling takes its `Growth`, `terra_survival::harvest`); a square left with none goes.
pub(super) fn unfar_the_felled(
    mut commands: Commands,
    mut felled: RemovedComponents<Growth>,
    trees: Query<&WoodsTree>,
    index: Option<Res<FarSquares>>,
    mut squares: Query<(&mut FarSquare, &mut Mesh3d, &mut Transform, &mut Aabb)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    terra_gen::stat_scope!("flora");
    let Some(index) = index else {
        felled.clear();
        return;
    };
    let mut changed: Vec<Entity> = Vec::new();
    for entity in felled.read() {
        let Ok(tree) = trees.get(entity) else {
            continue;
        };
        let Some(&square) = index.by_square.get(&index.square_of(tree.foot)) else {
            continue;
        };
        let Ok((mut far, ..)) = squares.get_mut(square) else {
            continue;
        };
        let key = foot_key(tree.foot);
        let before = far.cards.len();
        far.cards.retain(|card| foot_key(card.tree.at) != key);
        if far.cards.len() < before && !changed.contains(&square) {
            changed.push(square);
        }
    }
    for square in changed {
        let Ok((far, mut mesh, mut transform, mut aabb)) = squares.get_mut(square) else {
            continue;
        };
        if far.cards.is_empty() {
            commands.entity(square).despawn();
            continue;
        }
        info!(
            "Woods: a felled tree's far card is gone, its square merged again with {} cards",
            far.cards.len()
        );
        let (merged, middle, bounds) = merged(&far.cards);
        mesh.0 = meshes.add(merged);
        transform.translation = middle;
        *aabb = bounds;
    }
}
