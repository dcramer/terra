//! The carried arm held in the body's frame (D560): an arm the hold layer lays a carry pose on
//! (a look's `carry_hold`; `figure`'s one-arm hold, `layer::Nodes::over_one`) keeps that pose's
//! turn against the body, not against the chest, so a torch held flame up stays up through a
//! sprint's lean, a crouch's hunch and a jump's landing, where the layer alone, laying the arm
//! in its parent's frame, tipped it up to 83 degrees off with the chest.
//!
//! Which frame an overlay's arm is laid in is the overlay's own data, as in ALS (Advanced
//! Locomotion System; docs only), whose overlay poses each carry an arm's local-space and
//! mesh-space curves (`Arm_R_LS`, `Arm_R_MS`), and Lyra's Layered Blend per Bone with "Mesh
//! Space Rotation Blend" set per branch. Mesh space, here the body's frame: the shoulder
//! blade's turn in the body is the carry pose's on the chest as it stands at rest, whatever
//! the clip does to the chest, and the shoulder still goes where the chest takes it.
//!
//! A pose its set's data marks `upright` (`CarryHold::upright`: the torch's) is held so and no
//! more. Any other, a hanging one (a tool's Carry_Tool, a sword's, a knife's), is held so too
//! and **gives as the legs fold** (`fold`, `give`). Left leaning with the chest, a landing's
//! crouch tipped a pickaxe 85 degrees down and 34.6 cm under the ground, and a sprint's lean
//! 6.9 cm under it; held in the body's frame and no more, the hand came down with the hip to
//! the knees (the pickaxe 32 cm under the ground, a thigh 6.8 cm into its haft). So as the
//! legs fold the elbow bends, bringing the forearm and the look up, and the arm comes out
//! from the side, clear of the knee, as a person lands with a tool in hand. Its twin is
//! `tools/characters/carry_arm.ts`, which the move check lays over the carried clips
//! (`moves.ts --carry`) and which reads the give's numbers from this file; change both.

use bevy::{prelude::*, world_serialization::WorldInstanceReady};

use crate::bend::{Kept, find, in_body};
use crate::character::{Body, CarryHold, CarryPosed, HAND_L, HAND_R};

/// How a hanging carried arm gives with the legs folded all the way: the elbow bent this much
/// more (the forearm brought forward and up about the body's side axis) and the arm this much
/// out from the side at the shoulder (about the body's forward axis). The least that clears
/// the ground and the legs in every carried clip on every body
/// (`node tools/characters/holds.ts`, its `carry.ground` and `carry.legs`).
pub const GIVE_ELBOW_DEG: f32 = 40.0;
pub const GIVE_OUT_DEG: f32 = 25.0;
/// The legs' fold a give starts at (a stride's bend gives nothing) and is whole at (a
/// landing's crouch): how much shorter hip to ankle is than thigh and shin laid end to end.
pub const FOLD_FROM: f32 = 0.08;
pub const FOLD_TO: f32 = 0.3;

/// How far a body's legs are folded, 0 standing to 1: of the straighter leg (each its hip,
/// knee and ankle in the body), how much shorter hip to ankle is than thigh and shin laid end
/// to end, from `FOLD_FROM` to `FOLD_TO`, eased. It has no scale, so it is one on every body.
pub fn fold(legs: [[Vec3; 3]; 2]) -> f32 {
    let bent = |[hip, knee, ankle]: [Vec3; 3]| {
        let laid = hip.distance(knee) + knee.distance(ankle);
        if laid > 0.0 {
            1.0 - hip.distance(ankle) / laid
        } else {
            0.0
        }
    };
    let least = bent(legs[0]).min(bent(legs[1]));
    let x = ((least - FOLD_FROM) / (FOLD_TO - FOLD_FROM)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The turns in the body that give a hanging arm at `fold`, the right arm or the left: at the
/// upper arm (out from the side) and at the forearm (the elbow). The model faces +Z, +X its
/// left, so a right arm goes out turning about -Z.
pub fn give(fold: f32, right: bool) -> (Quat, Quat) {
    let out = (GIVE_OUT_DEG * fold).to_radians() * if right { -1.0 } else { 1.0 };
    (
        Quat::from_rotation_z(out),
        Quat::from_rotation_x(-(GIVE_ELBOW_DEG * fold).to_radians()),
    )
}

/// On a body whose model has both arms: each side's (left, right) hand socket, shoulder blade
/// and the chest it hangs from, the chest's turn in the body at rest, and what was written;
/// and each leg's hip, knee and ankle, which a hanging arm's give reads.
#[derive(Component)]
pub struct CarryArms {
    sides: [Side; 2],
    legs: [[Entity; 3]; 2],
}

struct Side {
    socket: Entity,
    clavicle: Entity,
    chest: Entity,
    rest: Quat,
    kept: Kept,
    /// The upper arm and the forearm, which a hanging pose's give turns, and what was written.
    upper: (Entity, Kept),
    lower: (Entity, Kept),
}

pub fn plugin(app: &mut App) {
    app.add_observer(find_arms).add_systems(
        PostUpdate,
        // On the pose the clips and holds gave, before the fit and every fix-up after it (the
        // fists, the arms kept off the hips, the shield arm, the head), which read it as held.
        hold_in_body
            .in_set(crate::pose::Layer::CarryArm)
            .after(bevy::app::AnimationSystems)
            .before(crate::Fitted),
    );
}

/// When a body's model is in, still at rest: each arm's bones and its chest's turn, and the legs.
fn find_arms(
    ready: On<WorldInstanceReady>,
    bodies: Query<(), With<Body>>,
    children: Query<&Children>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    transforms: Query<&mut Transform>,
    mut commands: Commands,
) {
    let root = ready.entity;
    if !bodies.contains(root) {
        return;
    }
    let Some(arms) = find(
        root,
        [
            HAND_L,
            "clavicle_l",
            "upperarm_l",
            "lowerarm_l",
            HAND_R,
            "clavicle_r",
            "upperarm_r",
            "lowerarm_r",
        ],
        &children,
        &names,
    ) else {
        return;
    };
    let Some(legs) = find(
        root,
        ["thigh_l", "calf_l", "foot_l", "thigh_r", "calf_r", "foot_r"],
        &children,
        &names,
    ) else {
        return;
    };
    let side = |[socket, clavicle, upper, lower]: [Entity; 4]| {
        let chest = parents.get(clavicle).ok()?.parent();
        let rest = in_body(chest, root, &parents, &transforms)
            .to_scale_rotation_translation()
            .1;
        Some(Side {
            socket,
            clavicle,
            chest,
            rest,
            kept: Kept::default(),
            upper: (upper, Kept::default()),
            lower: (lower, Kept::default()),
        })
    };
    let (Some(left), Some(right)) = (
        side([arms[0], arms[1], arms[2], arms[3]]),
        side([arms[4], arms[5], arms[6], arms[7]]),
    ) else {
        return;
    };
    commands.entity(root).insert(CarryArms {
        sides: [left, right],
        legs: [[legs[0], legs[1], legs[2]], [legs[3], legs[4], legs[5]]],
    });
}

/// `bone` turned by `turn` in the body from where its clip put it, under its parent as it
/// stands now (a parent turned before it this frame included).
fn turned(
    (bone, kept): &mut (Entity, Kept),
    turn: Quat,
    root: Entity,
    parents: &Query<&ChildOf>,
    transforms: &mut Query<&mut Transform>,
) {
    let Ok(parent) = parents.get(*bone).map(ChildOf::parent) else {
        return;
    };
    let parent = in_body(parent, root, parents, transforms)
        .to_scale_rotation_translation()
        .1;
    if let Ok(mut t) = transforms.get_mut(*bone) {
        let clip = kept.clip(t.rotation);
        t.rotation = kept.turn(clip, parent, turn);
    }
}

/// Each arm in a carry pose turned at its shoulder blade by the chest's turn from its rest,
/// undone; a hanging pose's then given by the legs' fold.
pub fn hold_in_body(
    mut bodies: Query<(Entity, &mut CarryArms)>,
    posed: Query<(&CarryPosed, &CarryHold)>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    for (root, mut arms) in &mut bodies {
        let legs = arms.legs;
        for (k, side) in arms.sides.iter_mut().enumerate() {
            let Ok((carried, hold)) = posed.get(side.socket) else {
                continue;
            };
            if !carried.0 {
                continue;
            }
            let chest = in_body(side.chest, root, &parents, &transforms)
                .to_scale_rotation_translation()
                .1;
            let Ok(now) = transforms.get(side.clavicle).map(|t| t.rotation) else {
                continue;
            };
            let clip = side.kept.clip(now);
            let written = side.kept.turn(clip, chest, side.rest * chest.inverse());
            if let Ok(mut t) = transforms.get_mut(side.clavicle) {
                t.rotation = written;
            }
            if hold.upright {
                continue;
            }
            let at = |bone| Vec3::from(in_body(bone, root, &parents, &transforms).translation);
            let folded = fold(legs.map(|leg| leg.map(at)));
            let (out, elbow) = give(folded, k == 1);
            turned(&mut side.upper, out, root, &parents, &mut transforms);
            turned(&mut side.lower, elbow, root, &parents, &mut transforms);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// What a test's body is: its chest leaning `lean` degrees forward from its rest, its legs
    /// standing or folded as a landing folds them, its right hand carrying in a pose or not,
    /// the pose upright (the torch's) or hanging (a tool's).
    struct Case {
        lean: f32,
        folded: bool,
        carried: bool,
        upright: bool,
    }

    struct Made {
        root: Entity,
        clavicle: Entity,
        lower: Entity,
        socket: Entity,
    }

    /// A body with a chest, on it a right arm hanging straight down (shoulder blade, upper
    /// arm, forearm, hand socket), and two legs.
    fn body(world: &mut World, case: Case) -> Made {
        let root = world
            .spawn(Transform::from_rotation(Quat::from_rotation_y(
                std::f32::consts::PI,
            )))
            .id();
        let rest = Quat::from_rotation_x(0.1);
        let under = |world: &mut World, parent: Entity, t: Transform| {
            world.spawn((t, ChildOf(parent))).id()
        };
        let chest = under(
            world,
            root,
            Transform::from_xyz(0.0, 1.3, 0.0)
                .with_rotation(Quat::from_rotation_x(case.lean.to_radians()) * rest),
        );
        let clavicle = under(
            world,
            chest,
            Transform::from_xyz(-0.05, 0.2, 0.0).with_rotation(rest.inverse()),
        );
        let upper = under(world, clavicle, Transform::from_xyz(-0.15, 0.0, 0.0));
        let lower = under(world, upper, Transform::from_xyz(0.0, -0.3, 0.0));
        let socket = under(world, lower, Transform::from_xyz(0.0, -0.25, 0.0));
        world.entity_mut(socket).insert(CarryPosed(case.carried));
        let look = world.spawn(ChildOf(socket)).id();
        world.entity_mut(socket).insert(CarryHold {
            look,
            clip: "Carry_Tool".into(),
            upright: case.upright,
        });
        // A leg of two 0.45 m bones: straight, or its knee forward so hip to ankle is 0.45 m.
        let leg = |world: &mut World, x: f32| {
            let (knee, ankle) = if case.folded {
                (Vec3::new(0.0, -0.225, 0.39), Vec3::new(0.0, -0.225, -0.39))
            } else {
                (Vec3::new(0.0, -0.45, 0.0), Vec3::new(0.0, -0.45, 0.0))
            };
            let hip = under(world, root, Transform::from_xyz(x, 0.95, 0.0));
            let knee = under(world, hip, Transform::from_translation(knee));
            let ankle = under(world, knee, Transform::from_translation(ankle));
            [hip, knee, ankle]
        };
        let legs = [leg(world, 0.1), leg(world, -0.1)];
        let side = |socket, upper, lower| Side {
            socket,
            clavicle,
            chest,
            rest,
            kept: Kept::default(),
            upper: (upper, Kept::default()),
            lower: (lower, Kept::default()),
        };
        let other = world.spawn_empty().id();
        world.entity_mut(root).insert(CarryArms {
            sides: [side(other, other, other), side(socket, upper, lower)],
            legs,
        });
        Made {
            root,
            clavicle,
            lower,
            socket,
        }
    }

    fn place_in_body(world: &mut World, root: Entity, bone: Entity) -> (Quat, Vec3) {
        let at = world
            .run_system_once(
                move |parents: Query<&ChildOf>, transforms: Query<&mut Transform>| {
                    in_body(bone, root, &parents, &transforms)
                },
            )
            .unwrap();
        (at.to_scale_rotation_translation().1, at.translation.into())
    }

    /// The body of `case` after three frames of the hold.
    fn held(case: Case) -> (World, Made) {
        let mut world = World::new();
        let made = body(&mut world, case);
        for _ in 0..3 {
            world.run_system_once(hold_in_body).unwrap();
        }
        (world, made)
    }

    /// The torch's complaint the layer alone had (D560), and a hanging tool's in a sprint: the
    /// chest leans, a crouch's 80 degrees or a sprint's 30, and the carried arm keeps its turn
    /// in the body, frame after frame, upright or hanging.
    #[test]
    fn a_carried_arm_keeps_its_turn_in_the_body_however_the_chest_leans() {
        for upright in [true, false] {
            let case = |lean| Case {
                lean,
                folded: false,
                carried: true,
                upright,
            };
            let (mut world, made) = held(case(0.0));
            let straight = place_in_body(&mut world, made.root, made.lower).0;
            for lean in [-20.0, 30.0, 60.0, 80.0_f32] {
                let (mut world, made) = held(case(lean));
                for bone in [made.clavicle, made.lower] {
                    let now = place_in_body(&mut world, made.root, bone).0;
                    let off = now.angle_between(straight).to_degrees();
                    assert!(
                        off < 0.1,
                        "upright {upright}, lean {lean}: the arm {off} degrees off its carried turn"
                    );
                }
            }
        }
    }

    /// An arm not in a carry pose (a stance's hold, a used look) leans with the chest and gives
    /// nothing: the hold layer's local space.
    #[test]
    fn an_arm_not_carried_is_left_as_the_clip_has_it() {
        for folded in [false, true] {
            let mut world = World::new();
            let made = body(
                &mut world,
                Case {
                    lean: 40.0,
                    folded,
                    carried: false,
                    upright: false,
                },
            );
            let before = place_in_body(&mut world, made.root, made.socket);
            world.run_system_once(hold_in_body).unwrap();
            let after = place_in_body(&mut world, made.root, made.socket);
            assert_eq!(after, before, "folded {folded}");
        }
    }

    /// The pickaxe under the ground in a landing: with the legs folded a hanging arm's elbow
    /// bends and the arm comes out, so the hand is where the give's two turns put it from the
    /// shoulder (worked here from the arm's lengths), higher, further out and further forward
    /// than standing; an upright pose's arm gives nothing.
    #[test]
    fn a_hanging_arm_gives_as_the_legs_fold_and_an_upright_one_does_not() {
        let case = |folded, upright| Case {
            lean: 50.0,
            folded,
            carried: true,
            upright,
        };
        let hand = |case: Case| {
            let (mut world, made) = held(case);
            let hand = place_in_body(&mut world, made.root, made.socket).1;
            let shoulder = place_in_body(&mut world, made.root, made.clavicle).1;
            hand - shoulder
        };
        let standing = hand(case(false, false));
        assert!(standing.distance(Vec3::new(-0.15, -0.55, 0.0)) < 1e-4);
        let given = hand(case(true, false));
        let out = Quat::from_rotation_z(-GIVE_OUT_DEG.to_radians());
        let elbow = Quat::from_rotation_x(-GIVE_ELBOW_DEG.to_radians());
        let wanted = Vec3::new(-0.15, 0.0, 0.0)
            + out * Vec3::new(0.0, -0.3, 0.0)
            + elbow * out * Vec3::new(0.0, -0.25, 0.0);
        assert!(
            given.distance(wanted) < 1e-4,
            "the hand at {given} from the shoulder, not {wanted}"
        );
        assert!(
            given.y > standing.y + 0.05 && given.x < standing.x - 0.1 && given.z > 0.1,
            "the hand at {given} is not up, out and forward of {standing}"
        );
        let upright = hand(case(true, true));
        assert!(
            upright.distance(standing) < 1e-4,
            "an upright arm gave: {upright}"
        );
    }

    /// The fold is of the straighter leg: nothing standing or in a stride (one knee up), all
    /// of it with both legs folded to 0.7 of their length, whatever the body's size.
    #[test]
    fn the_legs_fold_is_the_straighter_legs_and_has_no_scale() {
        let straight = [Vec3::Y, Vec3::Y * 0.5, Vec3::ZERO];
        let bent = [Vec3::Y * 0.7, Vec3::new(0.0, 0.35, 0.37), Vec3::ZERO];
        assert_eq!(fold([straight, straight]), 0.0);
        assert_eq!(fold([straight, bent]), 0.0);
        assert_eq!(fold([bent, bent]), 1.0);
        let half = [Vec3::Y * 0.81, Vec3::new(0.0, 0.405, 0.2932), Vec3::ZERO];
        let small = half.map(|p| p * 0.5);
        let of = fold([half, half]);
        assert!((of - 0.5).abs() < 0.02, "fold {of} at 0.19 short");
        assert!((fold([small, small]) - of).abs() < 1e-5);
    }
}
