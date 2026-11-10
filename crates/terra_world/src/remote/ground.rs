//! `world.ground`: the ground and the water at spots round the start, so a script picks a
//! place by what is there (level, dry) rather than by a name or a guess
//! (tools/sandbox/torchlight.ts).

use bevy::prelude::*;
use bevy::remote::{BrpResult, builtin_methods::parse_some};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Spots {
    at: Vec<[f32; 2]>,
}

/// For each spot, east and south of the start: how far its ground is over the start's level,
/// `up`, and how far the water's surface there (a lake's, else the sea's) is over its ground,
/// `water`: under nothing where it is dry. Metres.
pub(super) fn ground(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Spots { at } = parse_some(params)?;
    let start = *world.resource::<crate::start::Start>();
    let planet = world.resource::<crate::planet::Planet>();
    let spots = at.iter().map(|&[east, south]| {
        let ground = planet.ground_at(start.at(east, south));
        json!({
            "up": start.local(ground).y,
            "water": planet.water_radius_m(ground) - ground.length(),
        })
    });
    Ok(Value::Array(spots.collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{planet::Planet, start::Start};
    use terra_core::{PlanetSpec, Seed};

    /// A spot's `up` is where the planet's ground is there, and its `water` how far the
    /// planet's water stands over that: the point so far east, up and south of the start
    /// lies on the ground, known from the planet itself.
    #[test]
    fn a_spots_ground_and_water_are_the_planets_there() {
        let planet = Planet::new(PlanetSpec {
            seed: Seed(1),
            radius_m: 30_000.0,
        });
        let start = Start::choose(&planet, false);
        let mut world = World::new();
        world.insert_resource(start);
        world.insert_resource(planet);
        let id = world.register_system(ground);
        let at = [[0.0, 0.0], [40.0, -25.0], [-120.0, 60.0]];
        let said = world
            .run_system_with(id, Some(json!({ "at": at })))
            .unwrap()
            .unwrap();
        let planet = world.resource::<Planet>();
        for (spot, [east, south]) in said.as_array().unwrap().iter().zip(at) {
            let up = spot["up"].as_f64().unwrap() as f32;
            let point = start.transform_point(Vec3::new(east, up, south));
            let off = point.distance(planet.ground_at(point));
            assert!(
                off < 0.05,
                "{east} east, {south} south: {off} m off the ground"
            );
            let water = planet.water_radius_m(point) - point.length();
            let told = spot["water"].as_f64().unwrap() as f32;
            assert!(
                (told - water).abs() < 0.05,
                "water {told}, the planet's {water}"
            );
        }
        assert_eq!(said.as_array().unwrap().len(), 3);
    }
}
