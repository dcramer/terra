use super::*;

/// A camera put somewhere else at once is a cut; one flying, however fast, is not; nor
/// is the first frame, with nothing before it.
#[test]
fn a_jump_of_the_eye_is_a_cut() {
    let here = Vec3::new(10.0, 2.0, -3.0);
    assert!(jumped(Some(here), here + Vec3::new(40.0, 5.0, 12.0)));
    // A dragon chase at 60 m/s moves a metre a step.
    assert!(!jumped(Some(here), here + Vec3::X));
    assert!(!jumped(None, here));
}

fn keys(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    points.to_vec()
}

/// A path goes through every key at its time, whatever its ease.
#[test]
fn a_path_passes_through_its_keys() {
    let path = keys(&[(0.0, 0.0), (1.0, 4.0), (3.0, -2.0), (4.5, 10.0)]);
    for ease in [Ease::InOut, Ease::In, Ease::Out, Ease::None] {
        for &(t, v) in &path {
            let at = sample(&path, t, ease).unwrap();
            assert!((at - v).abs() < 1e-5, "{ease:?} at {t}: {at} not {v}");
        }
    }
}

/// Eased in and out, a move from one key to another starts and ends at rest, passes the
/// middle halfway through, and never goes past either end: smoothstep's shape.
#[test]
fn two_keys_eased_are_a_smooth_start_and_stop() {
    let path = keys(&[(0.0, 0.0), (2.0, 10.0)]);
    let at = |t: f32| sample(&path, t, Ease::InOut).unwrap();
    let dt = 1e-3;
    assert!(at(dt) / dt < 0.05, "moving at the start");
    assert!((10.0 - at(2.0 - dt)) / dt < 0.05, "moving at the end");
    assert!((at(1.0) - 5.0).abs() < 1e-5);
    // Smoothstep: 3s² - 2s³ of the way at s.
    let s: f32 = 0.25;
    assert!((at(0.5) - 10.0 * (3.0 * s * s - 2.0 * s * s * s)).abs() < 1e-4);
    let mut last = 0.0;
    for i in 0..=200 {
        let now = at(i as f32 / 100.0);
        assert!(now >= last - 1e-6 && now <= 10.0 + 1e-6, "overshot at {i}");
        last = now;
    }
}

/// Keys evenly spaced along a line, cut into and out of moving, give a steady dolly: the
/// same speed all the way, through the keys too.
#[test]
fn evenly_spaced_keys_not_eased_are_a_steady_move() {
    let path = keys(&[(0.0, 0.0), (1.0, 3.0), (2.0, 6.0), (3.0, 9.0)]);
    let dt = 1e-3;
    for i in 0..30 {
        let t = i as f32 * 0.1;
        let speed = (sample(&path, t + dt, Ease::None).unwrap()
            - sample(&path, t, Ease::None).unwrap())
            / dt;
        assert!((speed - 3.0).abs() < 0.01, "speed {speed} at {t}");
    }
}

/// Through a key in the middle the path keeps its speed: no kink where segments meet.
#[test]
fn a_path_has_no_kink_at_a_key() {
    let path = keys(&[(0.0, 0.0), (1.0, 5.0), (4.0, 6.0)]);
    let dt = 1e-3;
    let before = (sample(&path, 1.0, Ease::InOut).unwrap()
        - sample(&path, 1.0 - dt, Ease::InOut).unwrap())
        / dt;
    let after = (sample(&path, 1.0 + dt, Ease::InOut).unwrap()
        - sample(&path, 1.0, Ease::InOut).unwrap())
        / dt;
    assert!((before - after).abs() < 0.02, "{before} then {after}");
    // The Catmull-Rom tangent there: from the key before to the key after.
    assert!((before - 6.0 / 4.0).abs() < 0.02, "{before}");
}

/// A 35 mm lens on Super 35 sees 39.1 degrees across (published lens tables); in a 16:9
/// frame that is 22.6 degrees high.
#[test]
fn a_lens_sees_what_lens_tables_say() {
    let across = 2.0 * libm::atanf(libm::tanf(vertical_fov(35.0, 1.0) / 2.0));
    assert!(
        (across.to_degrees() - 39.1).abs() < 0.1,
        "{}",
        across.to_degrees()
    );
    let high = vertical_fov(35.0, 16.0 / 9.0).to_degrees();
    assert!((high - 22.6).abs() < 0.1, "{high}");
    // A longer lens sees less.
    assert!(vertical_fov(85.0, 1.0) < vertical_fov(35.0, 1.0));
}

/// 180 degrees at 30 frames a second is open for a sixtieth of a second: one whole step
/// of a run that steps sixty a second.
#[test]
fn a_film_shutter_is_one_step_open() {
    let look = Look::default();
    assert_eq!(look.shutter, 180.0);
    assert!((look.shutter_angle() - 1.0).abs() < 1e-6);
}

/// The start 100 m up the y axis, and flat ground 2 m under it everywhere.
fn flat_places() -> (Transform, impl Fn(Vec3) -> Vec3) {
    let start = Transform::from_xyz(0.0, 100.0, 0.0);
    (start, |at: Vec3| Vec3::new(at.x, 98.0, at.z))
}

fn shot(json: &str) -> Shot {
    Shot::parse(json).unwrap()
}

/// Orbit keys go round on a true arc: the camera stays as far from what it goes round
/// all the way, where a path through points on the circle would cut its corners.
#[test]
fn an_orbit_keeps_its_distance() {
    let (start, ground) = flat_places();
    let places = Places {
        start: &start,
        ground: &ground,
        player: None,
    };
    let orbit = shot(
        r#"{"seconds": 6, "rig": {"around": [10, 0, 0]}, "keys": [
            {"t": 0, "orbit": [0, 20, 5], "look": [10, 0, 0]},
            {"t": 6, "orbit": [120, 20, 5]}]}"#,
    );
    let centre = start.transform_point(Vec3::new(10.0, 0.0, 0.0));
    for i in 0..=12 {
        let eye = orbit.framing(i as f32 * 0.5, &places).eye;
        let flat = Vec3::new(eye.x - centre.x, 0.0, eye.z - centre.z);
        assert!(
            (flat.length() - 20.0).abs() < 1e-3,
            "{} at {i}",
            flat.length()
        );
        assert!((eye.y - centre.y - 5.0).abs() < 1e-3);
    }
    // Bearing 0 is north, which is minus south; a third of the way round is east of it.
    let first = orbit.framing(0.0, &places).eye - centre;
    assert!(first.z < -19.9 && first.x.abs() < 1e-3, "{first}");
    assert!(orbit.framing(6.0, &places).eye.x > centre.x);
}

/// A cut starts a set-up of its own: up to it the camera keeps to the take before, from
/// it to the next, and nothing of one take's path bends the other's.
#[test]
fn a_cut_starts_a_take_of_its_own() {
    let (start, ground) = flat_places();
    let places = Places {
        start: &start,
        ground: &ground,
        player: None,
    };
    let cut = shot(
        r#"{"seconds": 10, "ease": "none", "keys": [
            {"t": 0, "at": [0, 10, 0], "look": [0, 0, -20]},
            {"t": 5, "at": [10, 10, 0]},
            {"t": 5, "at": [100, 30, 0], "look": [100, 0, -20], "cut": true},
            {"t": 10, "at": [120, 30, 0]}]}"#,
    );
    let east = |t: f32| cut.framing(t, &places).eye.x;
    // Steady along each take: the second take's far keys do not pull the first's end.
    assert!((east(2.5) - 5.0).abs() < 1e-3, "{}", east(2.5));
    assert!((east(4.99) - 9.98).abs() < 1e-2, "{}", east(4.99));
    assert!((east(5.0) - 100.0).abs() < 1e-3);
    assert!((east(7.5) - 110.0).abs() < 1e-3, "{}", east(7.5));
    assert!(cut.framing(7.5, &places).target.x > 99.0);
    assert!(cut.cuts_between(4.98, 5.0) && !cut.cuts_between(5.0, 5.02));
    assert!(!cut.cuts_between(0.0, 4.9));
}

/// Keys over the ground stand that high over the ground under them, and a tracking
/// shot keeps its offset from the player wherever they walk.
#[test]
fn keys_over_the_ground_and_following_the_player() {
    let (start, ground) = flat_places();
    let places = Places {
        start: &start,
        ground: &ground,
        player: None,
    };
    let crane = shot(
        r#"{"seconds": 4, "rig": {"ground": true}, "keys": [
            {"t": 0, "at": [0, 1.5, 0], "look": [0, 1, -20]},
            {"t": 4, "at": [0, 12, 0]}]}"#,
    );
    assert!((crane.framing(0.0, &places).eye.y - 99.5).abs() < 1e-4);
    assert!((crane.framing(4.0, &places).eye.y - 110.0).abs() < 1e-4);
    let follow = shot(
        r#"{"seconds": 4, "rig": {"follow": true}, "keys": [
            {"t": 0, "at": [2, 1.7, 4], "look": "player"}]}"#,
    );
    for feet in [Vec3::new(5.0, 98.0, 7.0), Vec3::new(-30.0, 98.0, 2.0)] {
        let places = Places {
            start: &start,
            ground: &ground,
            player: Some(feet),
        };
        let framing = follow.framing(1.0, &places);
        assert!((framing.eye - (feet + Vec3::new(2.0, 1.7, 4.0))).length() < 1e-4);
        assert!((framing.target - (feet + feet.normalize() * PLAYER_LOOK_M)).length() < 1e-4);
        // Focus follows what it looks at.
        assert!((framing.focus_m - framing.eye.distance(framing.target)).abs() < 1e-4);
    }
}

/// A shot file that cannot be filmed says why.
#[test]
fn a_shot_that_cannot_be_filmed_is_refused() {
    let refused = |json: &str| Shot::parse(json).is_err();
    assert!(refused(r#"{"seconds": 0}"#));
    // No keys: the game's own camera films it, with the shot's look.
    let followed = shot(r#"{"seconds": 25, "look": {"aperture": 4}}"#);
    assert!(!followed.holds_camera() && followed.focused());
    assert!(refused(
        r#"{"seconds": 2, "keys": [{"t": 1, "at": [0,0,0], "look": [1,0,0]}, {"t": 1, "at": [1,0,0]}]}"#
    ));
    assert!(refused(
        r#"{"seconds": 2, "rig": {"around": [0,0,0]}, "keys": [{"t": 0, "at": [0,0,0], "look": [1,0,0]}, {"t": 1, "orbit": [0,5,0]}]}"#
    ));
    assert!(refused(
        r#"{"seconds": 2, "keys": [{"t": 0, "orbit": [0,5,0], "look": [1,0,0]}]}"#
    ));
    assert!(refused(
        r#"{"seconds": 2, "keys": [{"t": 0, "at": [0,0,0]}]}"#
    ));
    let fine = Shot::parse(
        r#"{"seconds": 2, "place": "lush", "look": {"exposure": 11.5, "letterbox": 2.39},
            "keys": [{"t": 0, "at": [0,0,0], "look": "player", "focus": "look", "aperture": 2.8}]}"#,
    )
    .unwrap();
    assert_eq!(fine.look.exposure, Exposing::Ev100(11.5));
    assert!(fine.focused());
    // Its detail drawn as the preset has it unless it biases it further.
    assert_eq!(fine.look.detail, 1.0);
    assert_eq!(
        shot(r#"{"seconds": 2, "look": {"detail": 1.8}}"#)
            .look
            .detail,
        1.8
    );
    let held = shot(r#"{"seconds": 2, "keys": [{"t": 0, "at": [0,0,0], "look": [1,0,0]}]}"#);
    assert_eq!(held.look.exposure, Exposing::default());
    assert!(!held.focused());
}

/// The handheld drift stays within its amount and moves slowly.
#[test]
fn handheld_drift_is_small_and_slow() {
    let amount = 0.5_f32;
    let mut last = drift(0.0, amount);
    for i in 1..600 {
        let now = drift(i as f32 / 60.0, amount);
        assert!(now.0.abs() <= amount.to_radians() + 1e-6);
        assert!(
            (now.0 - last.0).abs() < amount.to_radians() * 0.05,
            "jerked at {i}"
        );
        last = now;
    }
}
