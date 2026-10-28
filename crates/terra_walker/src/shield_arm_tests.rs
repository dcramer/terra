use super::*;
use crate::appearance::MovedNode;
use bevy::ecs::system::RunSystemOnce;

/// A shield board as the heater's: 15 cm below the fist, its face out of the forearm's -Z.
fn board() -> Place {
    Place {
        centre: DVec3::new(0.038, -0.151, -0.061),
        face: DVec3::NEG_Z,
        up: DVec3::NEG_X,
    }
}

/// A left arm's joints in their parents' frames, each bone along its own +Y as a rig's are:
/// the chest, shoulder blade, upper arm, forearm, hand and fist.
fn joints() -> [Transform; 6] {
    let turned = |x: f32, y: f32, z: f32| Quat::from_euler(EulerRot::XYZ, x, y, z);
    [
        Transform::from_xyz(0.0, 1.35, 0.02).with_rotation(turned(0.1, 0.0, 0.05)),
        Transform::from_xyz(0.03, 0.14, 0.02).with_rotation(turned(0.0, 0.2, -1.4)),
        Transform::from_xyz(0.0, 0.16, 0.0).with_rotation(turned(0.2, 0.1, -1.4)),
        Transform::from_xyz(0.0, 0.27, 0.0).with_rotation(turned(-1.3, 0.3, 0.2)),
        Transform::from_xyz(0.004, 0.25, -0.006).with_rotation(turned(0.1, 0.2, 0.0)),
        Transform::from_xyz(0.01, 0.08, 0.03),
    ]
}

fn arm_of(j: &[Transform; 6]) -> Arm {
    let p = j.map(|t| Pose::of(&t));
    Arm {
        parent: p[0],
        locals: [p[1], p[2], p[3]],
        strap: strap_in(&p[4], &p[5]),
    }
}

/// A place in the body as the chest at `chest`, at its rest and so not turned from it, has it.
fn in_chest(chest: &Pose, p: &Place) -> Place {
    Place {
        centre: p.centre - chest.t,
        ..*p
    }
}

#[test]
fn the_solve_puts_an_arms_board_on_a_place_it_can_reach() {
    let arm = arm_of(&joints());
    // A place the arm reaches with known turns: the solve finds one as near.
    let known = [0.05, -0.02, 0.03, 0.2, -0.1, 0.15, -0.1, 0.25, 0.05];
    let want = board_at(&arm, &board(), &known);
    let (cm0, _) = place_off(&board_at(&arm, &board(), &[0.0; 9]), &want);
    assert!(cm0 > 5.0, "the case moves the board ({cm0} cm)");
    let (cm, deg) = place_off(
        &board_at(&arm, &board(), &solve_arm(&arm, &board(), &want)),
        &want,
    );
    assert!(cm < 0.2 && deg < 0.6, "{cm} cm, {deg} deg off");
}

/// A body of `joints` with a shield strapped at its fist, the guard's target where the base
/// body's board is with its fist `base_fist` from where `joints` has it, and the shape moving
/// its hand by `hand_by`: (world, body, the arm's bones, the target in the body).
fn body(base_fist: Vec3, hand_by: Vec3, target_by: DVec3) -> (World, Entity, Place) {
    let j = joints();
    let mut world = World::new();
    let root = world.spawn((Body, Transform::default())).id();
    let mut parent = root;
    let mut nodes = [root; 6];
    for (k, t) in j.iter().enumerate() {
        let mut t = *t;
        if k == 4 {
            t.translation += hand_by;
        }
        parent = world.spawn((t, ChildOf(parent))).id();
        nodes[k] = parent;
    }
    let [spine, c, u, l, hand, fist] = nodes;
    world.spawn((
        Strapped {
            hand,
            socket: fist,
            turn: Quat::IDENTITY,
        },
        Transform::default(),
        ChildOf(l),
    ));
    let mut base = j;
    base[5].translation += base_fist;
    let chest = Pose::of(&j[0]);
    let mut target = in_chest(&chest, &board_at(&arm_of(&base), &board(), &[0.0; 9]));
    target.centre += target_by;
    world.insert_resource(ShieldTargets {
        board: board(),
        targets: vec![target],
    });
    world.entity_mut(root).insert((
        ShieldArm {
            spine,
            bones: [c, u, l],
            hand,
            fist,
            chest_rest: chest.r,
            kept: [Kept::default(); 3],
        },
        Moved(vec![MovedNode {
            node: hand,
            rest: j[4].translation,
            by: hand_by,
            held: None,
        }]),
    ));
    (world, root, target)
}

/// The board's place on the body as it stands, against the target: (cm, degrees).
fn off(world: &mut World, root: Entity, target: &Place) -> (f64, f64) {
    let arm = world.get::<ShieldArm>(root).unwrap();
    let nodes = [
        arm.spine,
        arm.bones[0],
        arm.bones[1],
        arm.bones[2],
        arm.hand,
        arm.fist,
    ];
    let j = nodes.map(|n| *world.get::<Transform>(n).unwrap());
    let chest = Pose::of(&j[0]);
    place_off(
        &in_chest(&chest, &board_at(&arm_of(&j), &board(), &[0.0; 9])),
        target,
    )
}

#[test]
fn a_body_whose_forearm_is_shorter_has_its_board_put_back_on_the_guards_target() {
    // The shape takes the hand 2.5 cm up the forearm, as the female template's shorter one.
    let (mut world, root, target) = body(Vec3::ZERO, Vec3::new(0.0, -0.025, 0.0), DVec3::ZERO);
    let (before, _) = off(&mut world, root, &target);
    assert!(before > 2.0, "the case moves the board ({before} cm)");
    world.run_system_once(fit_shield_arm).unwrap();
    let (cm, deg) = off(&mut world, root, &target);
    assert!(cm < 0.3 && deg < 1.0, "{cm} cm, {deg} deg off");
    // A frame no clip posed the arm again: turned from the clip's pose, not on top of its own.
    world.run_system_once(fit_shield_arm).unwrap();
    let again = off(&mut world, root, &target);
    assert!(
        (again.0 - cm).abs() < 1e-3 && (again.1 - deg).abs() < 1e-2,
        "{again:?}"
    );
}

#[test]
fn a_strap_moved_two_centimetres_leaves_the_board_on_its_target() {
    // The base body's fist 2 cm along the forearm from where the target was taken.
    let (mut world, root, target) = body(Vec3::new(0.0, 0.02, 0.0), Vec3::ZERO, DVec3::ZERO);
    let (before, _) = off(&mut world, root, &target);
    assert!(before > 1.5, "the case moves the board ({before} cm)");
    world.run_system_once(fit_shield_arm).unwrap();
    let (cm, _) = off(&mut world, root, &target);
    assert!(cm < 0.5, "{cm} cm off");
}

#[test]
fn a_board_far_from_every_target_is_left_as_the_clip_has_it() {
    // The guard's target 20 cm away: another pose (a block's raise), not pulled.
    let (mut world, root, _) = body(Vec3::ZERO, Vec3::ZERO, DVec3::new(0.0, 0.2, 0.0));
    let bones = world.get::<ShieldArm>(root).unwrap().bones;
    let before = bones.map(|b| world.get::<Transform>(b).unwrap().rotation);
    world.run_system_once(fit_shield_arm).unwrap();
    assert_eq!(
        bones.map(|b| world.get::<Transform>(b).unwrap().rotation),
        before
    );
}

#[test]
fn the_kept_targets_hold_the_guard_and_the_strapped_board() {
    let kept = ShieldTargets::default();
    assert!(!kept.targets.is_empty());
    assert!((kept.board.face.length() - 1.0).abs() < 1e-3);
    // The board hangs off the fist by a forearm's span at most.
    assert!(kept.board.centre.length() < 0.4, "{}", kept.board.centre);
}
