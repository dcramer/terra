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
    world.init_resource::<Messages<Fell>>();
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

/// A player let fall onto the ground is told hurt by the share its kind's numbers give the
/// speed it landed at (D476), once; one let fall from a step's height is not.
#[test]
fn a_long_fall_is_told_hurt_and_a_short_one_not() {
    let fallen = |height: f32| {
        let (mut world, player) = standing(Steer::default(), None);
        let ground = world.get::<Transform>(player).unwrap().translation;
        world.get_mut::<Transform>(player).unwrap().translation =
            ground + ground.normalize() * height;
        let mut fell = Vec::new();
        for _ in 0..192 {
            world.run_system_once(walk).unwrap();
            fell.extend(world.resource_mut::<Messages<Fell>>().drain());
        }
        assert!(fell.iter().all(|f| f.body == player));
        fell.iter().map(|f| f.harm).collect::<Vec<f32>>()
    };
    assert!(fallen(0.5).is_empty());
    // 25 m at the walk's gravity lands at about 28 m/s, past a man's fatal 25.
    let fell = fallen(25.0);
    assert_eq!(fell.len(), 1, "{fell:?}");
    assert!(fell[0] >= 1.0, "{fell:?}");
    let (onset, fatal) = (man().fall_onset, man().fall_fatal);
    let landed = libm::sqrtf(2.0 * GRAVITY * 8.0);
    assert!(landed > onset && landed < fatal);
    let fell = fallen(8.0);
    assert!(
        fell.len() == 1 && fell[0] > 0.0 && fell[0] < 1.0,
        "{fell:?}"
    );
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

/// People push softly (D479): a player jogging at a person standing just off their way keeps
/// to it, met only by a man's core, and the person gives way; someone bound there gives none
/// and turns the player aside round their width.
#[test]
fn a_player_jogs_on_past_a_person_who_gives_way_and_round_one_bound() {
    let ran = |body: crate::bodies::Body| {
        let (mut world, player) = standing(Steer::default(), Some(Action::Forward));
        let start = world.get::<Transform>(player).unwrap().translation;
        let planet = world.resource::<Planet>();
        let stood = planet.ground_at(start + Vec3::new(3.0, 0.0, 0.1));
        let them = world.spawn((body, Transform::from_translation(stood))).id();
        let mut aside = 0.0_f32;
        for _ in 0..96 {
            world.run_system_once(walk).unwrap();
            world.run_system_once(crate::bodies::keep_apart).unwrap();
            let feet = world.get::<Transform>(player).unwrap().translation;
            aside = aside.max((feet - start).reject_from(start.normalize()).z.abs());
        }
        let feet = world.get::<Transform>(player).unwrap().translation;
        let at = world.get::<Transform>(them).unwrap().translation;
        let went = (feet - start).reject_from(start.normalize()).x;
        (went, aside, at.distance(stood))
    };
    // A second and a half at a jog, less the run-up.
    let free = JOG_SPEED * 1.4;
    let (went, aside, gave) = ran(crate::bodies::Body::person(0.35, 2.0));
    assert!(went > free, "went {went} m of {free}");
    // No farther aside than a man's core and his own radius reach past where they stood.
    assert!(
        aside < man().core_m + 0.3 - 0.1 + 0.02,
        "turned {aside} m aside"
    );
    assert!(gave > 0.1, "gave way {gave} m");
    let (_, aside, gave) = ran(crate::bodies::Body::fixed(0.35));
    assert!(
        aside > 0.3 + 0.35 - 0.1 - 0.05,
        "turned only {aside} m aside"
    );
    assert_eq!(gave, 0.0);
}
