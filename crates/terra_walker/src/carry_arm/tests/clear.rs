//! `carry_arm`'s tests of the look kept clear by `carry_clear` (held-constraint j2), on its
//! tests' body.

use super::*;

/// A 60 cm look pointing down and ahead of the hand, as its sampled points.
const LONG: [Vec3; 3] = [
    Vec3::ZERO,
    Vec3::new(0.0, -0.26, 0.15),
    Vec3::new(0.0, -0.52, 0.3),
];

/// What a clearing test's body is: its chest `chest` over the ground (1.3 standing; the
/// hand hangs 0.35 under it), its right hand carrying a look of `points` or not, its
/// right leg's capsules `leg` thick, the switch on or off, the hand's bone known or not.
struct Crouch<'a> {
    chest: f32,
    points: &'a [Vec3],
    folded: bool,
    carried: bool,
    leg: f32,
    clear: bool,
    wrist: bool,
}

/// The body of `crouch` after three frames of the hold.
fn crouched(crouch: Crouch) -> (World, Made) {
    let mut world = World::new();
    let case = Case {
        lean: 0.0,
        folded: crouch.folded,
        carried: crouch.carried,
        upright: false,
    };
    let made = body(&mut world, case);
    let chest = world.get::<ChildOf>(made.clavicle).unwrap().parent();
    world.get_mut::<Transform>(chest).unwrap().translation.y = crouch.chest;
    world
        .entity_mut(made.look)
        .insert((Transform::IDENTITY, Reach(crouch.points.to_vec())));
    let mut arms = world.get_mut::<CarryArms>(made.root).unwrap();
    arms.clear = crouch.clear;
    arms.radii = [0.0, 0.0, crouch.leg, crouch.leg, 0.0];
    // A hand straight down its forearm, its palm ahead: a lift's turn back is flexion.
    let axes = [Vec3::NEG_Y, Vec3::Z, Vec3::X];
    let rest = carry_clear::bend(Vec3::NEG_Y, Quat::IDENTITY, axes);
    arms.sides[1].hand = crouch
        .wrist
        .then(|| (made.hand, Kept::default(), axes, rest));
    for _ in 0..3 {
        world.run_system_once(hold_in_body).unwrap();
    }
    (world, made)
}

/// The look's sampled points in the body, and how far, degrees, it points from where the
/// body's own axes would have it (the carry pose's direction: the arm hangs unturned).
fn look_now(world: &mut World, made: &Made, points: &[Vec3]) -> (Vec<Vec3>, f32) {
    let (turn, at) = place_in_body(world, made.root, made.look);
    let tip = points[points.len() - 1];
    (
        points.iter().map(|&p| at + turn * p).collect(),
        (turn * tip).angle_between(tip).to_degrees(),
    )
}

fn turns(world: &World, made: &Made) -> [Quat; 3] {
    [made.upper, made.lower, made.hand].map(|bone| world.get::<Transform>(bone).unwrap().rotation)
}

const DEEP: Crouch = Crouch {
    chest: 0.7,
    points: &LONG,
    folded: false,
    carried: true,
    leg: 0.0,
    clear: true,
    wrist: true,
};

#[test]
fn a_long_look_in_a_deep_crouch_ends_over_the_ground_pointing_as_it_was_carried() {
    let lowest = |points: Vec<Vec3>| points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    // The faults planted: switched off, the look's end is 17 cm under the ground; cleared
    // with no hand to turn back, the look tips past the bar of 10 degrees.
    let (mut world, made) = crouched(Crouch {
        clear: false,
        ..DEEP
    });
    assert!(lowest(look_now(&mut world, &made, &LONG).0) < -0.15);
    let (mut world, made) = crouched(Crouch {
        wrist: false,
        ..DEEP
    });
    assert!(look_now(&mut world, &made, &LONG).1 > 10.0);
    let (mut world, made) = crouched(DEEP);
    let (points, tipped) = look_now(&mut world, &made, &LONG);
    let low = lowest(points);
    assert!(low >= carry_clear::CLEAR_GROUND_M - 1e-3, "lowest {low}");
    assert!(low < carry_clear::CLEAR_GROUND_M + 0.01, "lowest {low}");
    assert!(tipped < 3.0, "tipped {tipped}");
}

#[test]
fn a_look_in_a_shin_is_pushed_clear_outward() {
    // The hand hangs at x -0.2, the right leg's axis at -0.1: a point of the look 8 cm in
    // from the hand at shin height is 2 cm from the axis of a 6 cm calf.
    let points = [Vec3::ZERO, Vec3::new(0.08, -0.1, 0.0)];
    let shin = Crouch {
        chest: 0.85,
        points: &points,
        leg: 0.06,
        ..DEEP
    };
    let off = |world: &mut World, made: &Made| {
        let p = look_now(world, made, &points).0[1];
        assert!((0.05..0.5).contains(&p.y), "beside the calf: {p}");
        Vec3::new(p.x + 0.1, 0.0, p.z)
    };
    let (mut world, made) = crouched(Crouch {
        clear: false,
        ..shin
    });
    assert!(off(&mut world, &made).length() < 0.03, "the fault planted");
    let (mut world, made) = crouched(shin);
    let out = off(&mut world, &made);
    assert!(
        out.length() >= 0.06 + carry_clear::CLEAR_BODY_M - 1e-3,
        "off {out}"
    );
    assert!(out.x < 0.0, "out on the hand's side: {out}");
}

#[test]
fn switched_off_the_pose_is_the_gives_bit_for_bit() {
    let folded = Crouch {
        folded: true,
        chest: 0.4,
        ..DEEP
    };
    // The give alone: the switch off and no look to read.
    let (alone, made) = crouched(Crouch {
        clear: false,
        points: &[],
        ..folded
    });
    let gives = turns(&alone, &made);
    let (off, made) = crouched(Crouch {
        clear: false,
        ..folded
    });
    assert_eq!(turns(&off, &made), gives);
    assert!(
        off.get::<CarryArms>(made.root).unwrap().sides[1]
            .cleared
            .is_none()
    );
    // The fault planted: switched on, the same body's arm is laid otherwise.
    let (on, made) = crouched(folded);
    assert_ne!(turns(&on, &made), gives);
}

#[test]
fn an_empty_hand_and_a_clear_look_are_left_as_they_were() {
    // A hand carrying nothing: no bone is written, whatever hangs under the ground.
    let (world, made) = crouched(Crouch {
        carried: false,
        ..DEEP
    });
    assert_eq!(turns(&world, &made), [Quat::IDENTITY; 3]);
    assert!(
        world.get::<CarryArms>(made.root).unwrap().sides[1]
            .cleared
            .is_none()
    );
    // A look already clear (the body standing): the give's pose, bit for bit, and no move kept.
    let standing = Crouch {
        chest: 1.3,
        folded: true,
        ..DEEP
    };
    let (off, made) = crouched(Crouch {
        clear: false,
        ..standing
    });
    let gives = turns(&off, &made);
    let (on, made) = crouched(standing);
    assert_eq!(turns(&on, &made), gives);
    assert!(
        on.get::<CarryArms>(made.root).unwrap().sides[1]
            .cleared
            .is_none()
    );
}
