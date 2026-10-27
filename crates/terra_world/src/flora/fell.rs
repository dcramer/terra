//! Felling the trees round a spot on asking (`world.fell`), as a level designer clears trees
//! from round a set piece by hand: a scene clears the woods so a giant walking through them
//! towers. Whatever planted a tree, it is found by its trunk ([`Trunks`]), taken out of the
//! world with everything drawn for it, and no longer stops a walker ([`Obstacles`]). What a
//! forest baked from its crowns, its floor's shade, stays.

use super::{Growth, Obstacles, Trunk, Trunks};
use crate::remote::method;
use bevy::{
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) fn plugin(app: &mut App) {
    method(app, "world.fell", fell);
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Asked {
    /// The middle, metres east and south of the start.
    at: [f32; 2],
    /// How far round it, across the ground, metres.
    within: f32,
    /// Only the trees taller than this, metres.
    #[serde(default)]
    taller_than: f32,
}

/// Fells the trees standing within so far of a spot, east and south of the start, those
/// taller than so many metres if asked; says how many, where each stood and how tall it was.
fn fell(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Asked {
        at,
        within,
        taller_than,
    } = parse_some(params)?;
    let start = *world.resource::<crate::start::Start>();
    let centre = world
        .resource::<crate::planet::Planet>()
        .ground_at(start.at(at[0], at[1]));
    let tall: Vec<Trunk> = world
        .resource::<Trunks>()
        .near(centre, within, world.resource::<Obstacles>())
        .filter(|trunk| trunk.height_m > taller_than)
        .collect();
    let mut growth = world.query::<(Entity, &Growth, &Transform)>();
    let found: Vec<(Entity, Option<Vec3>, Trunk)> = growth
        .iter(world)
        .filter_map(|(entity, growth, placed)| {
            let foot = growth.obstacle.unwrap_or(placed.translation);
            tall.iter()
                .find(|trunk| trunk.foot.distance(foot) < 0.5)
                .map(|trunk| (entity, growth.obstacle, *trunk))
        })
        .collect();
    let mut felled = Vec::new();
    for (entity, obstacle, trunk) in found {
        if let Some(stood) = obstacle {
            world.resource_mut::<Obstacles>().remove(stood);
        }
        world.despawn(entity);
        let local = start.local(trunk.foot);
        felled.push(json!({ "at": [local.x, local.z], "height": trunk.height_m }));
    }
    Ok(json!({ "felled": felled.len(), "trees": felled }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planet::Planet;
    use crate::start::Start;
    use terra_core::{PlanetSpec, Seed};

    #[test]
    fn felling_takes_only_the_tall_trees_within_reach_with_what_they_stop() {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(2),
            radius_m: 2000.0,
        });
        let start = Start(Transform::from_translation(
            planet.ground_at(Vec3::Y * 2100.0),
        ));
        let mut world = World::new();
        let mut trunks = Trunks::default();
        let mut obstacles = Obstacles::default();
        // A tall tree near, a short one near, a tall one far.
        let trees: Vec<(Vec3, Entity)> = [(3.0, 14.0), (-3.0, 6.0), (20.0, 14.0)]
            .into_iter()
            .map(|(east, height_m)| {
                let foot = planet.ground_at(start.at(east, 0.0));
                trunks.add(Trunk {
                    foot,
                    radius_m: 0.4,
                    height_m,
                    solid: true,
                });
                obstacles.add(foot, 0.4);
                let tree = world
                    .spawn((
                        Transform::from_translation(foot),
                        Growth::new(None, Some(foot), 1.4),
                    ))
                    .with_children(|tree| {
                        tree.spawn(Transform::default());
                    })
                    .id();
                (foot, tree)
            })
            .collect();
        world.insert_resource(planet);
        world.insert_resource(start);
        world.insert_resource(trunks);
        world.insert_resource(obstacles);
        let said = fell(
            In(Some(
                json!({ "at": [0.0, 0.0], "within": 10.0, "taller_than": 9.5 }),
            )),
            &mut world,
        )
        .unwrap();
        assert_eq!(said["felled"], 1, "{said}");
        let stops = |world: &World, foot: Vec3| {
            world.resource::<Obstacles>().solids_near(foot, 0.5).count()
        };
        let [(tall, felled), (short, kept), (far, also)] = trees[..] else {
            unreachable!()
        };
        assert!(world.get_entity(felled).is_err());
        assert_eq!(stops(&world, tall), 0);
        assert!(world.get_entity(kept).is_ok() && world.get_entity(also).is_ok());
        assert_eq!(stops(&world, short) + stops(&world, far), 2);
        // Only the two trees left, each with what is drawn for it.
        assert_eq!(world.query::<&Transform>().iter(&world).count(), 4);
    }
}
