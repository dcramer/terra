//! A grown tree felled, or cleared away by building or digging where it stood, takes its far
//! card with it. The card stands for the tree past its
//! last level of detail (`Card::grown`), so a tree cut down near the camera would come back
//! as a billboard once walked away from. As an impostor batch is rebuilt when one of its
//! instances goes (SpeedTree's and Unreal's HLOD clusters are rebuilt for what changed), the
//! square holding the felled tree's card is merged again without it, the rest as they were.

use super::near::NearWoods;
use super::{Card, SQUARE_M, foot_key, merged};
use crate::flora::{ClearedAway, Growth, Obstacles, Trunks};
use bevy::ecs::system::SystemParam;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::{camera::primitives::Aabb, prelude::*};

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
    /// Where each grown tree felled since last time stood, filed as its `Growth` went.
    felled: Vec<Vec3>,
}

impl FarSquares {
    pub fn new(middle: Vec3, east: Vec3, north: Vec3) -> Self {
        Self {
            middle,
            east,
            north,
            by_square: HashMap::default(),
            felled: Vec::new(),
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

/// Files where a grown tree of the woods stood as its `Growth` goes: felled for its logs, which
/// takes the `Growth` (`terra_survival::harvest`), or despawned outright, as `world.fell` does.
/// An observer, as the tree is still there to read while it goes; by the next frame a
/// despawned one is not. A tree streamed out is taken out through [`take_out`], unfiled.
///
/// Its trunk goes from the [`Trunks`] at once: `Trunks::near` leaves a felled one out only
/// while nothing stops a walker at its foot, and the stump a tree felled for its logs leaves
/// stands just there, so its trunk was found again, and `world.fell` took the stump for it.
pub(super) fn file_the_felled(
    gone: On<Remove<Growth>>,
    trees: Query<&WoodsTree>,
    (index, trunks): (Option<ResMut<FarSquares>>, Option<ResMut<Trunks>>),
) {
    let Ok(tree) = trees.get(gone.entity) else {
        return;
    };
    if let Some(mut trunks) = trunks {
        trunks.remove(tree.foot);
    }
    if let Some(mut index) = index {
        index.felled.push(tree.foot);
    }
}

/// Takes a grown tree out of the world as it streams out, not felled: it stops being a
/// [`WoodsTree`] first, so [`file_the_felled`] does not file it and its far card stays.
pub(super) fn take_out(commands: &mut Commands, tree: Entity) {
    commands
        .entity(tree)
        .try_remove::<WoodsTree>()
        .try_despawn();
}

/// Merges again, without its card, each square a grown tree felled or cleared away since
/// last time stood in (felling takes its `Growth` or the tree, [`file_the_felled`]; building or
/// digging where it stood hides it, `flora::ClearedAway`); a square left with none goes.
pub(super) fn unfar_the_felled(
    mut commands: Commands,
    mut cleared: MessageReader<ClearedAway>,
    trees: Query<&WoodsTree>,
    (index, mut near): (Option<ResMut<FarSquares>>, Option<ResMut<NearWoods>>),
    mut squares: Query<(&mut FarSquare, &mut Mesh3d, &mut Transform, &mut Aabb)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    terra_gen::stat_scope!("flora");
    let Some(mut index) = index else {
        cleared.clear();
        return;
    };
    let mut changed: Vec<Entity> = Vec::new();
    let mut gone = std::mem::take(&mut index.felled);
    gone.extend(
        cleared
            .read()
            .filter_map(|&ClearedAway(entity)| trees.get(entity).ok().map(|tree| tree.foot)),
    );
    for foot in gone {
        let key = foot_key(foot);
        // Felled or cleared for good: its block streamed in again leaves it out (D622).
        if let Some(near) = near.as_mut() {
            near.felled.insert(key);
        }
        let Some(&square) = index.by_square.get(&index.square_of(foot)) else {
            continue;
        };
        let Ok((mut far, ..)) = squares.get_mut(square) else {
            continue;
        };
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
            "Woods: a felled or cleared tree's far card is gone, its square merged again with {} cards",
            far.cards.len()
        );
        let (merged, middle, bounds) = merged(&far.cards);
        mesh.0 = meshes.add(merged);
        transform.translation = middle;
        *aabb = bounds;
    }
}

/// The grown trees of the woods felled or cleared away for good, by where their feet stood,
/// as a saved game keeps them: they stay felled however often their block streams in again
/// (D622), and a game loaded gives them back with [`FellAgain`].
#[derive(SystemParam)]
pub struct FelledWoods<'w> {
    near: Option<Res<'w, NearWoods>>,
}

impl FelledWoods<'_> {
    /// Where each felled tree's foot stood, in a fixed order, so the same felling is saved
    /// the same.
    pub fn feet(&self) -> Vec<Vec3> {
        let mut keys: Vec<[u32; 3]> = self
            .near
            .iter()
            .flat_map(|near| near.felled.iter().copied())
            .collect();
        keys.sort_unstable();
        keys.into_iter()
            .map(|key| Vec3::from_array(key.map(f32::from_bits)))
            .collect()
    }
}

/// Fells again the grown trees whose feet stood at these places, as a saved game is loaded:
/// those standing are taken out with their trunks, what stops a walker and their far cards,
/// and those still to stream in are left out (D622).
#[derive(Message, Clone, Debug)]
pub struct FellAgain(pub Vec<Vec3>);

pub(super) fn fell_again(
    mut commands: Commands,
    mut asked: MessageReader<FellAgain>,
    (near, index): (Option<ResMut<NearWoods>>, Option<ResMut<FarSquares>>),
    (mut trunks, mut obstacles): (ResMut<Trunks>, Option<ResMut<Obstacles>>),
    placed: Query<&Transform, With<WoodsTree>>,
) {
    let feet: Vec<Vec3> = asked
        .read()
        .flat_map(|FellAgain(feet)| feet.iter().copied())
        .collect();
    let Some(mut near) = near.filter(|_| !feet.is_empty()) else {
        return;
    };
    let keys: HashSet<[u32; 3]> = feet.iter().map(|&foot| foot_key(foot)).collect();
    let taken = near.fell_again(&keys);
    info!(
        "Woods: {} grown trees felled again as saved, {} of them standing",
        keys.len(),
        taken.len()
    );
    for tree in taken {
        super::near::take_out(
            &mut commands,
            (&mut *trunks, obstacles.as_deref_mut()),
            &placed,
            tree,
        );
    }
    // Their far cards go as a felled tree's do.
    if let Some(mut index) = index {
        index.felled.extend(feet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flora::Trunk;

    /// A grown tree of the woods felled for its logs, which takes its `Growth`, is found among
    /// the trunks no more, though its stump stands at its foot and stops a walker there.
    #[test]
    fn a_tree_felled_for_its_logs_is_no_trunk_beside_its_stump() {
        let foot = Vec3::new(0.0, 30_000.0, 0.0);
        let mut world = World::new();
        world.add_observer(file_the_felled);
        let mut trunks = Trunks::default();
        trunks.add(Trunk {
            foot,
            radius_m: 0.3,
            height_m: 18.0,
            solid: true,
        });
        world.insert_resource(trunks);
        let tree = world
            .spawn((WoodsTree { foot }, Growth::new(None, Some(foot), 1.4)))
            .id();
        world.entity_mut(tree).remove::<Growth>();
        // The stump left, sunk a little as its tree was.
        let mut obstacles = Obstacles::default();
        obstacles.add(foot - foot.normalize() * 0.3, 0.3);
        let found = world
            .resource::<Trunks>()
            .near(foot, 2.0, &obstacles)
            .count();
        assert_eq!(found, 0, "the felled tree's trunk is gone");
    }
}
