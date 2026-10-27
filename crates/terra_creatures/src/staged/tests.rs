use super::*;
use bevy::ecs::system::RunSystemOnce;
use terra_core::{PlanetSpec, Seed};

/// Ground that rises a metre for every ten east, on a planet's scale too large to curve.
fn slope(point: Vec3) -> Vec3 {
    Vec3::new(point.x, 1000.0 + point.x * 0.1, point.z)
}

#[test]
fn a_figure_walks_to_its_point_at_its_speed_on_the_ground_and_stops_there() {
    let to = Vec3::new(20.0, 1002.0, 0.0);
    let going = Some(Going { to, speed: 2.0 });
    let (mut at, mut facing) = (slope(Vec3::ZERO), Vec3::X);
    let dt = 1.0 / 60.0;
    let mut frames = 0;
    let mut arrived = false;
    while !arrived && frames < 60 * 30 {
        let next = step(slope, at, facing, going, None, 3.0, dt);
        (at, facing, arrived) = (next.at, next.facing, next.arrived);
        // Always on the ground, wherever along the slope.
        assert!(
            (at.y - (1000.0 + at.x * 0.1)).abs() < 1e-3,
            "off the ground at {at}"
        );
        frames += 1;
    }
    assert!(arrived, "never got there: {at}");
    // About 20 m at 2 m/s (a little more, along the level): ten seconds.
    let seconds = frames as f32 * dt;
    assert!((seconds - 10.0).abs() < 0.2, "took {seconds} s");
    assert!(
        (at.x - 20.0).abs() < 0.01 && at.z.abs() < 0.01,
        "stopped at {at}"
    );
    // Standing, it stays.
    let next = step(slope, at, facing, None, None, 3.0, dt);
    assert!(next.at.distance(at) < 1e-3);
}

#[test]
fn a_figure_turns_to_face_a_point_no_faster_than_it_turns() {
    let here = Vec3::new(0.0, 1000.0, 0.0);
    let flat = |p: Vec3| Vec3::new(p.x, 1000.0, p.z);
    // Facing north (-z), told to face east (+x): a quarter turn at 90° a second.
    let mut facing = Vec3::NEG_Z;
    let turn = 90f32.to_radians();
    let point = Vec3::new(50.0, 1000.0, 0.0);
    let after = |facing: Vec3, seconds: f32| {
        let mut f = facing;
        for _ in 0..(seconds * 60.0) as usize {
            f = step(flat, here, f, None, Some(point), turn, 1.0 / 60.0).facing;
        }
        f
    };
    let half = after(facing, 0.5);
    let angle = half.angle_between(Vec3::NEG_Z).to_degrees();
    assert!(
        (angle - 45.0).abs() < 2.0,
        "turned {angle}° in half a second"
    );
    // It turned the short way, toward the east.
    assert!(half.x > 0.5, "{half}");
    facing = after(facing, 2.0);
    assert!(facing.dot(Vec3::X) > 0.999, "facing {facing}");
}

#[test]
fn moving_with_nothing_to_face_it_turns_its_way() {
    let flat = |p: Vec3| Vec3::new(p.x, 1000.0, p.z);
    let going = Some(Going {
        to: Vec3::new(0.0, 1000.0, 40.0),
        speed: 1.5,
    });
    let mut facing = Vec3::NEG_Z;
    let mut at = flat(Vec3::ZERO);
    for _ in 0..240 {
        let next = step(flat, at, facing, going, None, 3.0, 1.0 / 60.0);
        (at, facing) = (next.at, next.facing);
    }
    assert!(facing.dot(Vec3::Z) > 0.99, "facing {facing}");
}

#[test]
fn clips_fit_a_model_whose_bones_they_move() {
    let id = |path: &[&str]| {
        let names: Vec<Name> = path.iter().map(|n| Name::new(n.to_string())).collect();
        AnimationTargetId::from_names(names.iter())
    };
    let model: HashSet<_> = [
        id(&["root"]),
        id(&["root", "pelvis"]),
        id(&["root", "pelvis", "spine_01"]),
    ]
    .into_iter()
    .collect();
    assert!(fits(
        &model,
        [id(&["root", "pelvis"]), id(&["root", "pelvis", "spine_01"])]
    ));
    // Another skeleton's bones, named alike but under another root, do not.
    assert!(!fits(
        &model,
        [
            id(&["Armature", "pelvis"]),
            id(&["Armature", "pelvis", "spine_01"])
        ]
    ));
    assert!(!fits(&model, []));
}

#[test]
fn a_figure_told_to_go_somewhere_goes_there_on_the_planets_ground_and_faces_another() {
    let planet = Planet::smooth(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    let ground = planet.ground_at(Vec3::Y * 2100.0);
    let mut world = World::new();
    world.insert_resource(planet.clone());
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs_f32(1.0 / 60.0));
    world.insert_resource(time);
    let staged = |name: &str, going, face| Staged {
        name: name.into(),
        going,
        face,
        facing: Vec3::NEG_Z,
        turn: 3.0,
        speed_now: 0.0,
        scale: 1.0,
        asked: None,
        asked_n: 0,
        rate: 1.0,
    };
    let troll_at = planet.ground_at(ground + Vec3::new(6.0, 3.0, 0.0));
    let troll = world
        .spawn((
            staged("troll", None, Face::Way),
            Transform::from_translation(troll_at),
        ))
        .id();
    let to = ground + Vec3::new(0.0, 0.0, 5.0);
    let man = world
        .spawn((
            staged("man", Some(Going { to, speed: 2.0 }), Face::Figure(troll)),
            // A metre over the ground: it is put on it.
            Transform::from_translation(ground + Vec3::Y),
        ))
        .id();
    for _ in 0..240 {
        world.run_system_once(stage).unwrap();
    }
    let at = world.get::<Transform>(man).unwrap();
    assert!(
        planet.distance(at.translation).abs() < 0.01,
        "not on the ground"
    );
    let level = |v: Vec3| v.reject_from(at.translation.normalize());
    assert!(
        level(at.translation - to).length() < 0.05,
        "stopped at {}",
        at.translation
    );
    assert!(world.get::<Staged>(man).unwrap().going.is_none());
    // Strafing past it, it kept the troll in front.
    let to_troll = level(troll_at - at.translation).normalize();
    assert!(at.forward().dot(to_troll) > 0.99, "not facing the troll");
}
#[test]
fn a_figure_whose_time_runs_at_a_quarter_goes_a_quarter_as_far() {
    let planet = Planet::smooth(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    let ground = planet.ground_at(Vec3::Y * 2100.0);
    let mut world = World::new();
    world.insert_resource(planet.clone());
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs_f32(1.0 / 60.0));
    world.insert_resource(time);
    let to = ground + Vec3::new(0.0, 0.0, 50.0);
    let moved = |world: &mut World, rate: f32| {
        let figure = world
            .spawn((
                Staged {
                    name: "man".into(),
                    going: Some(Going { to, speed: 2.0 }),
                    face: Face::Way,
                    facing: Vec3::Z,
                    turn: 3.0,
                    speed_now: 0.0,
                    scale: 1.0,
                    asked: None,
                    asked_n: 0,
                    rate,
                },
                Transform::from_translation(ground),
            ))
            .id();
        // A second at 2 m/s: two metres in its own time at the world's.
        for _ in 0..60 {
            world.run_system_once(stage).unwrap();
        }
        let at = world.get::<Transform>(figure).unwrap().translation;
        world.despawn(figure);
        (at - ground).length()
    };
    let whole = moved(&mut world, 1.0);
    let quarter = moved(&mut world, 0.25);
    assert!((whole - 2.0).abs() < 0.05, "went {whole} m");
    assert!((quarter - 0.5).abs() < 0.02, "went {quarter} m");
}

/// A stride's pace is metres a second in its model file's own world, through its armature
/// object's scale (`tools/characters/pace.py`), so a figure at its model's size going at the
/// clip's own pace plays it at its own speed, however large the armature's scale is. The pace
/// is known another way: the keyed walk's step length over its step time. Scaling by the
/// animation player's world scale (which carries the armature's 3.3 again) played the Cetri's
/// walk at 0.30 of its speed, clamped to half (round 13).
#[test]
fn a_figure_going_at_its_strides_pace_plays_it_at_its_own_speed_whatever_its_rigs_scale() {
    use terra_walker::model_files::{extra, json};
    let file = json("models/characters/cetri.glb");
    let pace = extra(&file, "paces").unwrap()["Giant_Walk"]
        .as_f64()
        .unwrap() as f32;
    let rig = file["scenes"][0]["nodes"][0].as_u64().unwrap() as usize;
    let rig_scale = file["nodes"][rig]["scale"][1].as_f64().unwrap_or(1.0) as f32;
    assert!(
        rig_scale > 3.0,
        "the case this covers: an armature scaled {rig_scale}"
    );
    let keyed = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../art/animation/keyed/giant_walk.json");
    let keyed: Value = serde_json::from_str(&std::fs::read_to_string(keyed).unwrap()).unwrap();
    let own = (keyed["step_m"].as_f64().unwrap() / keyed["step_s"].as_f64().unwrap()) as f32;
    assert!(
        (pace - own).abs() < 0.05 * own,
        "the file's pace {pace} against the keyed {own}"
    );
    let going = |speed_now: f32, scale: f32| Staged {
        name: "cetri".into(),
        going: None,
        face: Face::Way,
        facing: Vec3::NEG_Z,
        turn: 1.0,
        speed_now,
        scale,
        asked: None,
        asked_n: 0,
        rate: 1.0,
    };
    for (speed, scale, expect) in [
        (own, 1.0, 1.0),
        (2.0 * own, 2.0, 1.0),
        (0.75 * own, 1.0, 0.75),
    ] {
        let rate = played(&going(speed, scale), Some(pace));
        assert!(
            (rate - expect).abs() < 0.05,
            "at {speed} m/s, {scale} times its model's size: played at {rate}, not {expect}"
        );
    }
}
