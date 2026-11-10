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
//! from the side, clear of the knee, as a person lands with a tool in hand. The elbow also
//! goes back at the shoulder, which takes most of the forearm's turn out again: bent alone,
//! the elbow tipped a crouching body's axe 28 degrees above level, pointing ahead (the
//! developer's still of 2026-10-28, held.carry.axe_down; `holds.ts`'s `carry.rise`). Its twin is
//! `tools/characters/carry_arm.ts`, which the move check lays over the carried clips
//! (`moves.ts --carry`) and which reads the give's numbers from this file; change both.

use bevy::{
    mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    prelude::*,
    world_serialization::WorldInstanceReady,
};

use crate::arms::{Ends, Reach};
use crate::bend::{Kept, find, in_body};
use crate::carry_clear::{self, Arm, Capsule, Hand};
use crate::carry_radii::{self, PARTS};
use crate::character::{Body, CarryHold, CarryPosed, HAND_L, HAND_R};

/// Whether a hanging carried look is also kept clear of the ground and the body by the
/// constraint solved from the look itself (`carry_clear`), laid beside the give: off until
/// its twin in `tools/characters` can measure it (held-constraint j2), so with it off the pose
/// is the give's, bit for bit. What a body is made with (`CarryArms::clear`).
pub const CLEAR_ON: bool = false;
/// A hand's eased move under this, metres, with nothing to clear, is none: the look is clear
/// and nothing more is laid.
const SETTLED_M: f32 = 1.0e-4;

/// How a hanging carried arm gives with the legs folded all the way: the elbow bent this much
/// more (the forearm brought forward and up about the body's side axis), the upper arm this
/// much back at the shoulder (about the same axis the other way, the elbow going behind the
/// hip) and the arm this much out from the side (about the body's forward axis). The forearm,
/// and the look in its fist, tip up by the elbow's bend less the shoulder's turn back, 15
/// degrees: with no turn back a crouch tipped a tool 28.3 degrees above level, with 20 back
/// 9.4, with 25 back 4.7 (`node tools/characters/holds.ts`, its `carry.rise`, bar 10), every
/// look still clear of the ground, the trunk and the legs in every carried clip on every body
/// (its `carry.ground`, `carry.trunk`, `carry.legs`); 30 back leaves a pickaxe 1.6 cm over
/// the ground where 25 leaves 2.6.
pub const GIVE_ELBOW_DEG: f32 = 40.0;
pub const GIVE_BACK_DEG: f32 = 25.0;
pub const GIVE_OUT_DEG: f32 = 25.0;
/// The legs' fold a give starts at (a stride's bend gives nothing) and is whole at (a
/// landing's crouch): how much shorter hip to ankle is than thigh and shin laid end to end.
pub const FOLD_FROM: f32 = 0.08;
pub const FOLD_TO: f32 = 0.3;
/// How fast a give follows the fold, a second: the rate of the critically damped spring that
/// eases it in time (`ease`), which halves what is left about every 1.68 / rate seconds
/// (0.047 s). The slowest that keeps every look clear through a jump's second crouch and a
/// landing, which fold the legs whole inside 0.1 s (`holds.ts`: at 30 the knife is 1.1 cm into
/// the thigh at Jump_Start 0.27 s, at 20 the sword 3.4 cm into the calf).
pub const GIVE_RATE: f32 = 36.0;

/// The fastest a give's elbow turns, degrees a second, however the fold steps: a critically
/// damped spring's greatest speed toward a step of 1 is rate / e.
pub const ELBOW_MOST_DEG_S: f32 = GIVE_ELBOW_DEG * GIVE_RATE / std::f32::consts::E;

/// A give eased in time: where it is and how fast it moves, in folds (0 to 1).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Eased {
    pub at: f32,
    pub speed: f32,
}

/// `eased` after `dt` seconds toward `fold`: a critically damped spring of `GIVE_RATE`, solved
/// exactly, so a step of any length is stable and the give neither jumps with the fold nor
/// overshoots it. Followed frame by frame, the give popped: Jump_Start opens on a crouch (fold
/// 0.40) and stands 0.07 s later (0.03), so the elbow bent 40 degrees and let go inside it.
///
/// The spring is the one games smooth a value toward a moving target with: Daniel Holden,
/// "Spring-It-On: The Game Developer's Spring-Roll-Call" (2021), its exact critically damped
/// spring (`simple_spring_damper_exact`, the half damping here `GIVE_RATE`), the form of
/// Thomas Lowe's "Critically Damped Ease-In/Ease-Out Smoothing" (Game Programming Gems 4,
/// 1.10), which Unity's `Mathf.SmoothDamp` approximates. Ported from the article's formula.
pub fn ease(eased: Eased, fold: f32, dt: f32) -> Eased {
    let j = eased.speed + (eased.at - fold) * GIVE_RATE;
    let decay = libm::expf(-GIVE_RATE * dt);
    Eased {
        at: fold + decay * (eased.at - fold + j * dt),
        speed: decay * (eased.speed - j * GIVE_RATE * dt),
    }
}

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
/// upper arm (back, then out from the side) and at the forearm (the elbow). The model faces
/// +Z, +X its left, so a right arm goes out turning about -Z, and either goes back about +X.
pub fn give(fold: f32, right: bool) -> (Quat, Quat) {
    let out = (GIVE_OUT_DEG * fold).to_radians() * if right { -1.0 } else { 1.0 };
    (
        Quat::from_rotation_z(out) * Quat::from_rotation_x((GIVE_BACK_DEG * fold).to_radians()),
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
    /// Whether a hanging look is kept clear by `carry_clear` too (`CLEAR_ON`).
    clear: bool,
    /// The trunk's capsule's ends (the pelvis and the neck), if the body has them.
    trunk: Option<[Entity; 2]>,
    /// The capsules' radii, in `carry_radii`'s order, measured once from the body's skin.
    radii: [f32; PARTS],
}

/// A body's capsules as it is posed now: each leg's thigh and calf and the trunk, in the body;
/// a part with no radius is none.
fn capsules(
    ([l, r], trunk, radii): ([[Entity; 3]; 2], Option<[Entity; 2]>, [f32; PARTS]),
    root: Entity,
    parents: &Query<&ChildOf>,
    transforms: &Query<&mut Transform>,
) -> Vec<Capsule> {
    let at = |bone| Vec3::from(in_body(bone, root, parents, transforms).translation);
    let ends = [[l[0], l[1]], [l[1], l[2]], [r[0], r[1]], [r[1], r[2]]];
    ends.into_iter()
        .chain(trunk)
        .zip(radii)
        .filter(|(_, radius)| *radius > 0.0)
        .map(|([a, b], radius)| Capsule {
            a: at(a),
            b: at(b),
            radius,
        })
        .collect()
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
    /// A hanging pose's give, eased in time; none while the arm is not carried so, and the
    /// first frame it is, the give is settled on that frame's fold.
    given: Option<Eased>,
    /// The hand's bone, which `carry_clear` turns back at the wrist, with its axes in its own
    /// frame and its bend at rest (`carry_clear::Hand`); none on a model without its bones.
    hand: Option<(Entity, Kept, [Vec3; 3], [f32; 2])>,
    /// The hand's move that clears the look, eased in time; none while there is nothing to
    /// clear, and the first frame there is, the move is settled on that frame's.
    cleared: Option<carry_clear::Eased>,
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
    skins: Query<(&SkinnedMesh, &Mesh3d)>,
    meshes: Option<Res<Assets<Mesh>>>,
    bindposes: Option<Res<Assets<SkinnedMeshInverseBindposes>>>,
    mut commands: Commands,
) {
    let root = ready.entity;
    if !bodies.contains(root) {
        return;
    }
    // A hand's bone with its axes and its bend at rest, as `wrist.ts` takes them: the wrist,
    // the middle knuckle, the thumb's first joint and the socket's -X out of the palm.
    let hand = |s: &str, socket: Entity, lower: Entity| {
        let bones = [
            format!("hand_{s}"),
            format!("middle_01_{s}"),
            format!("thumb_01_{s}"),
        ];
        let [hand, knuckle, thumb] = find(
            root,
            bones.each_ref().map(String::as_str),
            &children,
            &names,
        )?;
        let place = |bone| in_body(bone, root, &parents, &transforms);
        let at = |bone| Vec3::from(place(bone).translation);
        let turn = |bone| place(bone).to_scale_rotation_translation().1;
        let axes = carry_clear::hand_axes(
            turn(hand),
            at(hand),
            at(knuckle),
            at(thumb),
            turn(socket) * Vec3::NEG_X,
        );
        let rest = carry_clear::bend(at(hand) - at(lower), turn(hand), axes);
        Some((hand, Kept::default(), axes, rest))
    };
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
    let side = |s: &str, [socket, clavicle, upper, lower]: [Entity; 4]| {
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
            given: None,
            hand: hand(s, socket, lower),
            cleared: None,
        })
    };
    let (Some(left), Some(right)) = (
        side("l", [arms[0], arms[1], arms[2], arms[3]]),
        side("r", [arms[4], arms[5], arms[6], arms[7]]),
    ) else {
        return;
    };
    let legs = [[legs[0], legs[1], legs[2]], [legs[3], legs[4], legs[5]]];
    let trunk = find(root, ["pelvis", "neck_01"], &children, &names);
    let [l, r] = legs;
    let none = [Entity::PLACEHOLDER; 2];
    let axes = [
        [l[0], l[1]],
        [l[1], l[2]],
        [r[0], r[1]],
        [r[1], r[2]],
        trunk.unwrap_or(none),
    ];
    let radii = match (meshes, bindposes) {
        (Some(meshes), Some(bindposes)) => {
            carry_radii::measure(root, axes, &children, &names, &skins, &meshes, &bindposes)
        }
        _ => [0.0; PARTS],
    };
    commands.entity(root).insert(CarryArms {
        sides: [left, right],
        legs,
        clear: CLEAR_ON,
        trunk,
        radii,
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
/// undone; a hanging pose's then given by the legs' fold, eased in time.
pub fn hold_in_body(
    time: Res<Time>,
    mut bodies: Query<(Entity, &mut CarryArms)>,
    posed: Query<(&CarryPosed, &CarryHold)>,
    looks: Query<(&Reach, Option<&Ends>)>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    let dt = time.delta_secs();
    for (root, mut arms) in &mut bodies {
        let legs = arms.legs;
        let body = (arms.clear, (arms.legs, arms.trunk, arms.radii));
        for (k, side) in arms.sides.iter_mut().enumerate() {
            let carried = posed.get(side.socket).ok().filter(|(c, _)| c.0);
            let Some((_, hold)) = carried else {
                side.given = None;
                side.cleared = None;
                continue;
            };
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
                side.given = None;
                side.cleared = None;
                continue;
            }
            let at = |bone| Vec3::from(in_body(bone, root, &parents, &transforms).translation);
            let folded = fold(legs.map(|leg| leg.map(at)));
            let given = match side.given {
                Some(given) => ease(given, folded, dt),
                None => Eased {
                    at: folded,
                    speed: 0.0,
                },
            };
            side.given = Some(given);
            let (out, elbow) = give(given.at, k == 1);
            turned(&mut side.upper, out, root, &parents, &mut transforms);
            turned(&mut side.lower, elbow, root, &parents, &mut transforms);
            if !body.0 {
                continue;
            }
            // A hand turned back the frame before and posed by no clip since is read from its
            // clip's pose, as the look in it is.
            if side.cleared.is_some() {
                turned_hand(
                    &mut side.hand,
                    Quat::IDENTITY,
                    root,
                    &parents,
                    &mut transforms,
                );
            }
            // The look as the give leaves it, kept clear by the hand's least move
            // (`carry_clear`): its sampled points in the body, the arm, the hand and the body's
            // capsules as they stand now.
            let place = in_body(hold.look, root, &parents, &transforms);
            let points: Vec<Vec3> = looks.get(hold.look).map_or_else(
                |_| Vec::new(),
                |(reach, ends)| {
                    let ends = ends.map_or([].as_slice(), |e| e.0.as_slice());
                    reach
                        .0
                        .iter()
                        .chain(ends)
                        .map(|&p| place.transform_point3(p))
                        .collect()
                },
            );
            let wrist = side.hand.map_or(side.socket, |hand| hand.0);
            let at = |bone| Vec3::from(in_body(bone, root, &parents, &transforms).translation);
            let arm = Arm {
                shoulder: at(side.upper.0),
                elbow: at(side.lower.0),
                hand: at(wrist),
            };
            let hand = side.hand.map(|(bone, _, axes, rest)| Hand {
                turn: in_body(bone, root, &parents, &transforms)
                    .to_scale_rotation_translation()
                    .1,
                axes,
                rest,
            });
            let want = if points.is_empty() {
                Vec3::ZERO
            } else {
                let capsules = capsules(body.1, root, &parents, &transforms);
                let out = if k == 1 { Vec3::NEG_X } else { Vec3::X };
                carry_clear::clear(&arm, hand.as_ref(), &points, 0.0, &capsules, out)
            };
            let by = match side.cleared {
                Some(eased) => carry_clear::ease(eased, want, GIVE_RATE, dt),
                None => carry_clear::Eased {
                    at: want,
                    speed: Vec3::ZERO,
                },
            };
            // Clear, and nothing left of an earlier move: the pose is the give's.
            let settled = want == Vec3::ZERO && by.at.length() < SETTLED_M;
            let was = side.cleared.is_some();
            side.cleared = (!settled).then_some(by);
            if settled && !was {
                continue;
            }
            let laid = if settled {
                (Quat::IDENTITY, Quat::IDENTITY)
            } else {
                carry_clear::reach(&arm, by.at)
            };
            // Over the give's turns, each bone's whole turn from its clip: the shoulder's
            // swing on the upper arm's, and on the forearm the swing and the elbow's bend
            // about the forearm as the give left it, under a parent the swing has turned.
            let (swing, bend) = laid;
            turned(
                &mut side.upper,
                swing * out,
                root,
                &parents,
                &mut transforms,
            );
            let lower = swing * bend * elbow * swing.inverse();
            turned(&mut side.lower, lower, root, &parents, &mut transforms);
            let back = carry_clear::back(&arm, laid, hand.as_ref());
            turned_hand(&mut side.hand, back, root, &parents, &mut transforms);
        }
    }
}

/// A side's hand, if it has the bone, turned by `turn` in the body from where its clip put it.
fn turned_hand(
    hand: &mut Option<(Entity, Kept, [Vec3; 3], [f32; 2])>,
    turn: Quat,
    root: Entity,
    parents: &Query<&ChildOf>,
    transforms: &mut Query<&mut Transform>,
) {
    if let Some((bone, kept, ..)) = hand {
        let mut written = (*bone, *kept);
        turned(&mut written, turn, root, parents, transforms);
        *kept = written.1;
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
        upper: Entity,
        lower: Entity,
        hand: Entity,
        socket: Entity,
        look: Entity,
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
        let hand = under(world, lower, Transform::from_xyz(0.0, -0.25, 0.0));
        let socket = under(world, hand, Transform::IDENTITY);
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
            given: None,
            hand: None,
            cleared: None,
        };
        world.init_resource::<Time>();
        let other = world.spawn_empty().id();
        world.entity_mut(root).insert(CarryArms {
            sides: [side(other, other, other), side(socket, upper, lower)],
            legs,
            clear: false,
            trunk: None,
            radii: [0.0; PARTS],
        });
        Made {
            root,
            clavicle,
            upper,
            lower,
            hand,
            socket,
            look,
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
    /// bends, the upper arm goes back and the arm comes out, so the hand is where the give's
    /// turns put it from the shoulder (worked here from the arm's lengths), higher and further
    /// out than standing; an upright pose's arm gives nothing. The axe level in a crouch
    /// (held.carry.axe_down): the forearm, and so the look in the fist, tips up by no more than
    /// 20 degrees, which the elbow's bend alone (40) passed.
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
        let out = Quat::from_rotation_z(-GIVE_OUT_DEG.to_radians())
            * Quat::from_rotation_x(GIVE_BACK_DEG.to_radians());
        let elbow = Quat::from_rotation_x(-GIVE_ELBOW_DEG.to_radians());
        let wanted = Vec3::new(-0.15, 0.0, 0.0)
            + out * Vec3::new(0.0, -0.3, 0.0)
            + elbow * out * Vec3::new(0.0, -0.25, 0.0);
        assert!(
            given.distance(wanted) < 1e-4,
            "the hand at {given} from the shoulder, not {wanted}"
        );
        assert!(
            given.y > standing.y + 0.03 && given.x < standing.x - 0.1,
            "the hand at {given} is not up and out of {standing}"
        );
        // The forearm's turn about the body's side axis, standing to given: what a look held
        // across the fist is tipped up by.
        let forearm = |case: Case| {
            let (mut world, made) = held(case);
            place_in_body(&mut world, made.root, made.lower).0
        };
        let turned = forearm(case(true, false)) * forearm(case(false, false)).inverse();
        let tipped = -turned.to_euler(EulerRot::ZXY).1.to_degrees();
        assert!(
            tipped > 5.0 && tipped <= 20.0,
            "the forearm tips {tipped} degrees up as the legs fold: over 20 lifts a carried \
             axe's head above level (GIVE_ELBOW_DEG less GIVE_BACK_DEG)"
        );
        let upright = hand(case(true, true));
        assert!(
            upright.distance(standing) < 1e-4,
            "an upright arm gave: {upright}"
        );
    }

    /// The give's pop at a jump's start (held-carry j4): a fold stepping from nothing to whole
    /// in one frame moves the elbow no faster than `ELBOW_MOST_DEG_S` (529 degrees a second;
    /// followed frame by frame it was 40 degrees in the frame, 2400), by the spring's own
    /// answer, 1 - (1 + rate t) e^(-rate t), never past the fold, and is there in half a second.
    #[test]
    fn a_step_in_the_fold_moves_the_elbow_no_faster_than_its_rate() {
        let dt = 1.0 / 60.0;
        let mut eased = Eased::default();
        for frame in 1..=30 {
            let before = eased.at;
            eased = ease(eased, 1.0, dt);
            let t = frame as f32 * dt;
            let known = 1.0 - (1.0 + GIVE_RATE * t) * libm::expf(-GIVE_RATE * t);
            assert!((eased.at - known).abs() < 1e-4, "frame {frame}: {eased:?}");
            let speed = (eased.at - before) * GIVE_ELBOW_DEG / dt;
            assert!(
                (0.0..=ELBOW_MOST_DEG_S).contains(&speed),
                "frame {frame}: the elbow at {speed} degrees a second"
            );
        }
        assert!((eased.at - 1.0).abs() < 1e-3, "after 0.5 s: {eased:?}");
        // One frame crouched, then standing again: the elbow comes 8.7 degrees up, not 40 (the
        // frame leaves it at 0.122 moving 11.85 a second; from there the spring's answer
        // toward nothing peaks 0.02 s on at 0.218 of the give), and is down in half a second.
        let mut pulse = ease(Eased::default(), 1.0, dt);
        let mut most = pulse.at;
        for _ in 0..30 {
            pulse = ease(pulse, 0.0, dt);
            most = most.max(pulse.at);
        }
        let up = most * GIVE_ELBOW_DEG;
        assert!((8.0..8.8).contains(&up), "the elbow came {up} degrees up");
        assert!(pulse.at.abs() < 1e-3, "{pulse:?}");
    }

    /// The same on a body: carried standing, then its legs folded between two frames a 60th
    /// of a second apart, the forearm has turned some, under the rate; a second on, all of it.
    #[test]
    fn a_body_folding_its_legs_in_one_frame_bends_its_elbow_in_time() {
        let mut world = World::new();
        let made = body(
            &mut world,
            Case {
                lean: 0.0,
                folded: false,
                carried: true,
                upright: false,
            },
        );
        world.run_system_once(hold_in_body).unwrap();
        let standing = place_in_body(&mut world, made.root, made.lower).0;
        let legs = world.get::<CarryArms>(made.root).unwrap().legs;
        for [_, knee, ankle] in legs {
            world.get_mut::<Transform>(knee).unwrap().translation = Vec3::new(0.0, -0.225, 0.39);
            world.get_mut::<Transform>(ankle).unwrap().translation = Vec3::new(0.0, -0.225, -0.39);
        }
        let frame = |world: &mut World, seconds: f32| {
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(seconds));
            world.run_system_once(hold_in_body).unwrap();
            let now = place_in_body(world, made.root, made.lower).0;
            now.angle_between(standing).to_degrees()
        };
        let first = frame(&mut world, 1.0 / 60.0);
        // The forearm turns by the elbow's give and the arm's out: both under their share.
        let most = (GIVE_ELBOW_DEG + GIVE_OUT_DEG) * GIVE_RATE / std::f32::consts::E / 60.0;
        assert!(first > 1.0 && first < most, "{first} degrees in the frame");
        let settled = frame(&mut world, 1.0);
        // Bent 40 at the elbow less 25 back at the shoulder, and 25 out: about 29 in all.
        assert!(
            settled > 25.0,
            "a second on the forearm is {settled} degrees"
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

    mod clear;
}
