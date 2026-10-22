//! What hides an animal from an eye besides the trunks, rocks and logs that stop a walker
//! (`Obstacles`) and the crowns: the bushes and young trees whose leaves stand in the way,
//! and the stones too low to stop anyone that still stand between an eye at a walker's
//! height and a duck on the water or a fox in the grass (a lake's shore stones). An animal is
//! put out where none of it is in the way and walks into sight (docs/scene-rules.md,
//! Wildlife), as a shipped game's ambient spawner asks its sightlines of everything solid.

use bevy::camera::primitives::Aabb;
use bevy::math::Affine3A;
use bevy::prelude::*;
use terra_world::flora::Growth;
use terra_world::flora_kinds::Cut;
use terra_world::parting::{ObstructionFade, crosses};
use terra_world::spatial::SpatialHash;

/// The bushes and young trees whose leaves hide what is behind them (those the camera fades,
/// `ObstructionFade`), and the stones, filed by place once their meshes are in.
#[derive(Resource)]
pub struct Thickets {
    plants: SpatialHash<Entity>,
    stones: SpatialHash<Stone>,
}

impl Default for Thickets {
    fn default() -> Self {
        Self {
            plants: SpatialHash::new(16.0),
            stones: SpatialHash::new(16.0),
        }
    }
}

/// A stone's box, in its own frame.
#[derive(Clone, Copy, Debug)]
pub struct Stone {
    to_stone: Affine3A,
    middle: Vec3,
    half: Vec3,
}

/// A thicket's leaves reach at most so far from where it is filed.
const THICKET_REACH_M: f32 = 6.0;
/// A stone reaches at most so far from its middle; a bigger one stops a walker and is among
/// the obstacles already (`terra_world::flora`, a stone over 0.9 m).
const STONE_REACH_M: f32 = 3.0;
/// How much of its box a stone fills, each way: a rough block, rounded at its corners.
const STONE_MASS: f32 = 0.8;

impl Thickets {
    /// Files a stone, `bounds` its mesh's box drawn at `at`.
    pub fn add_stone(&mut self, bounds: &Aabb, at: &GlobalTransform) {
        let stone = Stone {
            to_stone: at.affine().inverse(),
            middle: Vec3::from(bounds.center),
            half: Vec3::from(bounds.half_extents) * STONE_MASS,
        };
        let middle = at.transform_point(stone.middle);
        if (at.affine().matrix3 * bounds.half_extents).length() <= STONE_REACH_M {
            self.stones.insert(middle, stone);
        }
    }

    /// Whether leaves or a stone stand on the line between two points.
    pub fn hide(&self, plants: &Query<&ObstructionFade>, eye: Vec3, at: Vec3) -> bool {
        let middle = eye.lerp(at, 0.5);
        let half = eye.distance(at) * 0.5;
        self.plants
            .near(middle, half + THICKET_REACH_M)
            .filter_map(|(_, plant)| plants.get(*plant).ok())
            .any(|plant| plant.hides(eye, at))
            || self
                .stones
                .near(middle, half + STONE_REACH_M)
                .any(|(_, stone)| {
                    let from = stone.to_stone.transform_point3(eye) - stone.middle;
                    let to = stone.to_stone.transform_point3(at) - stone.middle;
                    crosses(from, to, stone.half)
                })
    }
}

/// Filed already among the thickets.
#[derive(Component)]
pub(super) struct Filed;

/// Files each new bush and young tree, and each stone of the land's, once its box is known.
pub(super) fn file_thickets(
    mut commands: Commands,
    mut thickets: ResMut<Thickets>,
    plants: Query<(Entity, &ObstructionFade), Without<Filed>>,
    stones: Query<
        (Entity, &Growth, &Aabb, &GlobalTransform),
        (Without<Filed>, Without<ObstructionFade>),
    >,
) {
    for (plant, fade) in &plants {
        if let Some((centre, reach)) = fade.round() {
            if reach <= THICKET_REACH_M * 1.5 {
                thickets.plants.insert(centre, plant);
            }
            commands.entity(plant).insert(Filed);
        }
    }
    for (stone, growth, bounds, at) in &stones {
        if growth
            .cut
            .as_ref()
            .is_some_and(|cut| matches!(cut.what, Cut::Rock | Cut::Ore))
        {
            thickets.add_stone(bounds, at);
        }
        commands.entity(stone).insert(Filed);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Surroundings, tests::surroundings_world};
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use terra_world::flora::Obstacles;
    use terra_world::flora_kinds::{Cuttable, FloraKind};

    #[test]
    fn a_shore_stone_too_low_to_stop_a_walker_hides_a_duck_behind_it_not_one_beside_it() {
        // The lake film's meeting (lake round 28): ducks on the water 16 m ahead of an eye 2 m
        // up, a shore stone 0.7 m tall and a metre across 3 m short of them, too low to stop
        // anyone (no obstacle), so only what hides counts it.
        let (mut world, eye, up) = surroundings_world();
        let ground = eye - up * 2.0;
        let rock = |what| {
            let cut = Cuttable {
                what,
                gives: (FloraKind(0), 1),
                seconds: 1.0,
            };
            Growth::new(Some(cut), None, 0.3)
        };
        let bounds = Aabb::from_min_max(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 0.7, 0.5));
        let place = |at: Vec3| {
            let turn = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(0.4);
            GlobalTransform::from(Transform::from_translation(at).with_rotation(turn))
        };
        world.spawn((rock(Cut::Rock), bounds, place(ground + Vec3::X * 13.0)));
        // A plant's box of the same size is not a stone.
        let beside = ground + Vec3::X * 13.0 + Vec3::Z * 6.1;
        world.spawn((rock(Cut::Plant), bounds, place(beside)));
        world.run_system_once(file_thickets).unwrap();
        world.flush();
        let duck = move |side: f32| ground + Vec3::X * 16.0 + Vec3::Z * side + up * 0.15;
        let seen = world
            .run_system_once(move |around: Surroundings, obstacles: Res<Obstacles>| {
                [
                    obstacles.sweep(eye, Vec3::X, 15.0, 0.15).is_some(),
                    around.in_view(eye, duck(0.0)),
                    around.in_view(eye, duck(2.5)),
                    around.in_view(eye, duck(7.5)),
                    around.in_view(eye, duck(0.0) + up * 3.0),
                ]
            })
            .unwrap();
        assert_eq!(
            seen,
            [false, false, true, true, true],
            "a walker's obstacle; behind the stone; beside it; behind the plant; over the stone"
        );
    }
}
