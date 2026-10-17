//! The fliers' tests.

use super::*;
use bevy::ecs::system::RunSystemOnce;

/// A world with the fliers' kinds and the clock stepping a sixtieth of a second.
fn world() -> World {
    let mut world = World::new();
    let defs: Vec<FlierDef> = terra_world::data::read(FILE);
    world.insert_resource(Kinds {
        models: Vec::new(),
        ready: vec![None; defs.len()],
        defs,
    });
    let mut time = Time::<()>::default();
    time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
    world.insert_resource(time);
    world
}

#[test]
fn a_flier_banks_into_a_turn_its_inside_wing_down_and_flies_level_out_of_it() {
    let mut world = world();
    let up = Vec3::Y;
    let start = Vec3::new(0.0, 2000.0, 0.0);
    // North (-z), then a right turn east (+x), then on east.
    let through = [
        start + Vec3::new(0.0, 0.0, -80.0),
        start + Vec3::new(60.0, 0.0, -140.0),
        start + Vec3::new(260.0, 0.0, -140.0),
    ];
    let def = world.resource::<Kinds>().defs[0].clone();
    let planned = plan::plan(start, Vec3::NEG_Z, 24.0, &through, 24.0, &def.wings(), up);
    let seconds = planned.seconds();
    let mut flier = Flier::new(
        0,
        Flight::Path {
            plan: planned,
            seconds: 0.0,
            then_face: start,
        },
    );
    flier.hovering = 0.0;
    let flier = world
        .spawn((
            flier,
            Transform::from_translation(start).looking_to(Vec3::NEG_Z, up),
        ))
        .id();
    let mut lowest_right = f32::INFINITY;
    let mut most_turn = 0.0f32;
    // Until before it brakes into its hover, its nose up.
    for _ in 0..(60.0 * (seconds - 7.0)) as usize {
        world.run_system_once(fly).unwrap();
        let t = world.get::<Transform>(flier).unwrap();
        lowest_right = lowest_right.min(t.right().dot(up));
        most_turn = most_turn.min(world.get::<Flier>(flier).unwrap().turn_rate);
    }
    // In the right turn its right wing went down, well into a bank, and it turned right.
    assert!(
        lowest_right < -0.3,
        "its right wing went no lower than {lowest_right}"
    );
    assert!(
        most_turn < -0.1,
        "it turned right at most {most_turn} rad/s"
    );
    // Out of it, flying east, it is level again.
    let t = world.get::<Transform>(flier).unwrap();
    assert!(
        t.right().dot(up).abs() < 0.1,
        "still banked {}",
        t.right().dot(up)
    );
    assert!(
        t.forward().dot(Vec3::X) > 0.9,
        "not heading east: {}",
        t.forward()
    );
}

#[test]
fn a_flier_braking_into_a_hover_raises_its_nose() {
    let mut world = world();
    let up = Vec3::Y;
    let start = Vec3::new(0.0, 2000.0, 0.0);
    let def = world.resource::<Kinds>().defs[0].clone();
    let through = [start + Vec3::new(0.0, 0.0, -200.0)];
    let planned = plan::plan(start, Vec3::NEG_Z, 24.0, &through, 24.0, &def.wings(), up);
    let seconds = planned.seconds();
    let mut flier = Flier::new(
        0,
        Flight::Path {
            plan: planned,
            seconds: 0.0,
            then_face: start + Vec3::new(0.0, 0.0, -400.0),
        },
    );
    flier.hovering = 0.0;
    let flier = world
        .spawn((
            flier,
            Transform::from_translation(start).looking_to(Vec3::NEG_Z, up),
        ))
        .id();
    let mut cruising_nose = None;
    let mut braking_nose = f32::MIN;
    for frame in 0..(60.0 * (seconds - 0.5)) as usize {
        world.run_system_once(fly).unwrap();
        let t = world.get::<Transform>(flier).unwrap();
        let nose = t.forward().dot(up);
        if frame == 60 * 4 {
            cruising_nose = Some(nose);
        }
        if world.get::<Flier>(flier).unwrap().accel < -1.0 {
            braking_nose = braking_nose.max(nose);
        }
    }
    let cruising = cruising_nose.unwrap();
    assert!(cruising.abs() < 0.05, "nose {cruising} cruising level");
    assert!(
        braking_nose > 0.25,
        "nose rose only to {braking_nose} braking"
    );
}

#[test]
fn the_fliers_file_reads_and_every_kind_has_its_model() {
    let defs: Vec<FlierDef> = terra_world::data::read(FILE);
    assert!(!defs.is_empty());
    for def in &defs {
        assert!(
            terra_world::data::assets().join(&def.model.0).exists(),
            "{} has no model {}",
            def.name,
            def.model.0
        );
        assert!(
            def.cruise_mps > def.hover_mps
                && def.most_bank_deg > 0.0
                && def.most_bank_deg < 80.0
                && def.accel_mps2 > 0.0
                && def.brake_mps2 > 0.0
        );
    }
}
