use super::*;
use crate::walker::Steer;
use bevy::ecs::system::RunSystemOnce;
use std::time::Duration;
use terra_core::{PlanetSpec, Seed};

const TICK: f32 = 1.0 / 64.0;

/// A world with a smooth planet, its trees' index, the walk's bodies and a tick's time, and a
/// player standing on its ground facing +X with full stamina, steered as given, holding an
/// action.
fn standing(steer: Steer, action: Option<Action>) -> (World, Entity) {
    let planet = Planet::smooth(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    let ground = planet.ground_at(Vec3::Y * 2100.0);
    let mut world = World::new();
    world.insert_resource(planet);
    world.init_resource::<flora::Obstacles>();
    world.init_resource::<Movers>();
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs_f32(TICK));
    world.insert_resource(time);
    let mut motion = Motion::facing(Vec3::X);
    motion.on_ground = true;
    let mut intent = Intent::default();
    if let Some(action) = action {
        intent.hold(action);
    }
    let player = world
        .spawn((
            Player,
            Transform::from_translation(ground),
            motion,
            Stance { steer, ..default() },
            Stamina::default(),
            View {
                forward: Vec3::X,
                pitch: 0.0,
            },
            intent,
        ))
        .id();
    (world, player)
}

/// A tick of the walk for a player standing on the ground pressing an action: its stamina
/// left, and whether it rolled and jumped.
fn press(steer: Steer, action: Action) -> (f32, bool, bool) {
    let (mut world, player) = standing(steer, Some(action));
    world.run_system_once(walk).unwrap();
    let rolled = world.get::<Stance>(player).unwrap().dodging > 0.0;
    let jumped = world.get::<Motion>(player).unwrap().rising > 0.0;
    (world.get::<Stamina>(player).unwrap().left, rolled, jumped)
}

#[test]
fn a_jump_costs_the_same_with_a_sword_drawn_and_a_roll_is_left_to_the_stance() {
    let full = Stamina::FULL;
    let sheathed = Steer::default();
    // As the sword stance steers a body: facing set, its own pace and drive, its own roll.
    let drawn = Steer {
        face: Some(Vec3::X),
        pace: Some(2.2),
        drive: Some(Vec3::ZERO),
        no_roll: true,
    };
    assert_eq!(
        press(sheathed, Action::Jump),
        (full - JUMP_STAMINA, false, true)
    );
    assert_eq!(
        press(drawn, Action::Jump),
        (full - JUMP_STAMINA, false, true)
    );
    assert_eq!(
        press(sheathed, Action::Dodge),
        (full - DODGE_STAMINA, true, false)
    );
    // Drawn, the walker neither rolls nor charges: the stance's roll pays its own cost
    // (`terra_creatures::melee`).
    assert_eq!(press(drawn, Action::Dodge), (full, false, false));
}

#[test]
fn deep_snow_and_mud_slow_a_walker_and_frozen_ground_does_not() {
    let mut ground = ClimateSample::default();
    assert_eq!(going(&ground), 1.0);
    ground.snow_depth_m = 0.3;
    assert!((going(&ground) - 0.5).abs() < 1e-6);
    ground.snow_depth_m = 0.0;
    ground.ground_wetness = 1.0;
    let mud = going(&ground);
    assert!(mud < 0.9);
    ground.ice_m = 0.1;
    assert_eq!(going(&ground), 1.0, "frozen hard");
}

#[test]
fn the_walks_speeds_are_a_mans() {
    let man = man();
    assert_eq!((JOG_SPEED, SPRINT_SPEED), (man.jog, man.sprint));
}

/// The walk drives the mover: held forward for two seconds the player jogs over the ground,
/// standing on it, the figure facing the way they go; moved elsewhere (a warp), they walk on
/// from there, not from where the mover last had them; gone, their body is let go.
#[test]
fn a_player_jogs_on_the_ground_through_the_mover_and_a_warp_is_followed() {
    let (mut world, player) = standing(Steer::default(), Some(Action::Forward));
    let start = world.get::<Transform>(player).unwrap().translation;
    for _ in 0..128 {
        world.run_system_once(walk).unwrap();
    }
    let feet = world.get::<Transform>(player).unwrap().translation;
    let motion = world.get::<Motion>(player).unwrap();
    let (on_ground, speed, facing) = (motion.on_ground, motion.velocity.length(), motion.facing);
    let ran = (feet - start).reject_from(start.normalize()).length();
    // Two seconds at jog, less the run-up.
    assert!(ran > JOG_SPEED * 1.6 && ran < JOG_SPEED * 2.0, "ran {ran}");
    assert!(on_ground);
    assert!((speed - JOG_SPEED).abs() < 0.05, "at {speed} m/s");
    let above = world.resource::<Planet>().distance(feet);
    assert!(above.abs() < 0.03, "feet {above} m off the ground");
    assert!(facing.dot(Vec3::X) > 0.99);

    let planet = world.resource::<Planet>();
    let warped = planet.ground_at(Vec3::new(0.0, 2100.0, 40.0));
    world.get_mut::<Transform>(player).unwrap().translation = warped;
    world.run_system_once(walk).unwrap();
    let feet = world.get::<Transform>(player).unwrap().translation;
    assert!(
        feet.distance(warped) < 0.2,
        "{} m from the warp",
        feet.distance(warped)
    );

    world.despawn(player);
    world.run_system_once(walk).unwrap();
    assert_eq!(world.resource::<Movers>().bodies.iter().count(), 0);
}
