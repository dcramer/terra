//! The shield arm fitted to each body after the clips (D554): every body shape plays the one
//! fight.glb, whose shield arm hold.py's `shield` op solved on the base body, so a body whose
//! arm differs (the female template's shoulder sits 5.4 cm in, her forearm is 2 cm shorter)
//! carries the board centimetres off its pose's place against the chest. After the clips and
//! the arms kept off the hips, the shoulder blade, upper arm and forearm are turned the least
//! way that puts the strapped board back on its target of `art/animation/shield_targets.json`
//! (the near targets blended by inverse distance, so a crossfade between two poses' places
//! never snaps): wholly where the base body's arm, as the clip has it, holds the board within
//! `NEAR` of a target, and not at all from `FAR`, so a pose with the board elsewhere (the
//! block's raise) is left as its clip has it and every shape of one pose is pulled alike.
//!
//! It is the run-time correction shipped games lay over a clip for a body's proportions
//! (Unreal's IK Rig retargeting and Control Rig hand IK): the bake gets the board close, this
//! takes up the centimetres. The solve is hold.py's (a damped Gauss-Newton over each bone's
//! turn in its own frame, 3 degrees weighing as 1 cm), with Levenberg-Marquardt where that is
//! left far off, in f64 as the tools solve it. Its twin is
//! `tools/characters/shield_arm.ts`, which the move check lays over the clips; change both.

use bevy::{
    math::{DQuat, DVec3},
    prelude::*,
    world_serialization::WorldInstanceReady,
};
use serde::Deserialize;

use crate::appearance::Moved;
use crate::bend::{Kept, find};
use crate::character::{Body, HAND_L};
use crate::strap::Strapped;

/// The bones turned, from the shoulder blade down, and what a turn of each costs per radian
/// against a centimetre off: the shoulder blade moves least, so it shrugs only when the arm
/// cannot reach. hold.py's COST.
pub const ARM: [&str; 3] = ["clavicle_l", "upperarm_l", "lowerarm_l"];
pub const COST: [f64; 3] = [0.4, 0.05, 0.05];
/// 3 degrees off the board's face or up weigh as 1 cm off its centre (the move check's bars).
pub const PER_RADIAN: f64 = 180.0 / (3.0 * std::f64::consts::PI);
/// How far off its nearest target (cm, or degrees / 3, the larger) the base body's board is
/// pulled wholly onto it, and from how far not at all: Low_Guard's bake is 0.7 off, a strap
/// moved 2 cm about 3; the block's raise is far off every target.
pub const NEAR: f64 = 5.0;
pub const FAR: f64 = 10.0;
/// The power of the off in the targets' blend (`between`): 4 keeps a strap moved 2 cm off the
/// guard's target within 0.2 cm of it with the strafes' 7 cm away.
const SHEPARD: i32 = 4;
const STEPS: usize = 12;
/// How near (cm, or degrees / 3) a Gauss-Newton solve must end to be kept without a
/// Levenberg-Marquardt one: half the move check's bar.
const CLOSE: f64 = 0.5;

/// The shield poses' board places and the strapped board, as the tools keep them.
const TARGETS: &str = include_str!("../../../art/animation/shield_targets.json");

/// A board's place: its centre (m), its face (its painted front) and its up, in some frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub centre: DVec3,
    pub face: DVec3,
    pub up: DVec3,
}

#[derive(Deserialize)]
struct Kept3 {
    centre: [f64; 3],
    face: [f64; 3],
    up: [f64; 3],
}

impl From<&Kept3> for Place {
    fn from(p: &Kept3) -> Self {
        let v = DVec3::from_array;
        Place {
            centre: v(p.centre),
            face: v(p.face),
            up: v(p.up),
        }
    }
}

/// The strapped board in the strap's frame (the true forearm's, at the fist; metres on the base
/// body) and each shield pose's board place in the chest's frame (spine_03's joint, turned as
/// the chest has turned from its rest).
#[derive(Resource, Clone, Debug)]
pub struct ShieldTargets {
    pub board: Place,
    pub targets: Vec<Place>,
}

impl ShieldTargets {
    /// From shield_targets.json's text: its `board` and every pose's `targets` entry.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        #[derive(Deserialize)]
        struct File {
            board: Kept3,
            targets: std::collections::BTreeMap<String, Kept3>,
        }
        let file: File = serde_json::from_str(text)?;
        Ok(ShieldTargets {
            board: (&file.board).into(),
            targets: file.targets.values().map(Place::from).collect(),
        })
    }
}

impl Default for ShieldTargets {
    fn default() -> Self {
        Self::parse(TARGETS).expect("art/animation/shield_targets.json")
    }
}

/// A node's pose: translation, rotation and scale, in f64.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub t: DVec3,
    pub r: DQuat,
    pub s: DVec3,
}

impl Pose {
    pub const IDENTITY: Pose = Pose {
        t: DVec3::ZERO,
        r: DQuat::IDENTITY,
        s: DVec3::ONE,
    };

    pub fn of(t: &Transform) -> Pose {
        Pose {
            t: t.translation.as_dvec3(),
            r: t.rotation.as_dquat(),
            s: t.scale.as_dvec3(),
        }
    }

    /// `local` under this, as Bevy composes transforms.
    pub fn apply(&self, local: &Pose) -> Pose {
        Pose {
            t: self.place(local.t),
            r: self.r * local.r,
            s: self.s * local.s,
        }
    }

    /// A point of this pose's frame in its parent's.
    pub fn place(&self, p: DVec3) -> DVec3 {
        self.t + self.r * (self.s * p)
    }
}

/// The turn of a rotation vector (radians about its own way).
fn turn_of(v: DVec3) -> DQuat {
    let a = v.length();
    if a < 1e-12 {
        return DQuat::IDENTITY;
    }
    let s = libm::sin(a / 2.0) / a;
    DQuat::from_xyzw(v.x * s, v.y * s, v.z * s, libm::cos(a / 2.0))
}

/// The arm as the solve sees it: the shoulder blade's parent in the body, the three bones' own
/// poses, and the strap's frame in the forearm's.
#[derive(Clone, Copy, Debug)]
pub struct Arm {
    pub parent: Pose,
    pub locals: [Pose; 3],
    pub strap: Pose,
}

/// The strap's frame in the forearm's from the hand's and the fist's own poses: at the fist,
/// turned the least way to take the forearm's +Y onto the wrist (`strap::at_fist`).
pub fn strap_in(hand: &Pose, fist: &Pose) -> Pose {
    Pose {
        t: hand.place(fist.t),
        r: DQuat::from_rotation_arc(DVec3::Y, hand.t.normalize_or(DVec3::Y)),
        s: DVec3::ONE,
    }
}

/// The strapped board (`board`, in the strap's frame) in the body with the bones turned by
/// `x` (a rotation vector a bone, in its own frame).
pub fn board_at(arm: &Arm, board: &Place, x: &[f64; 9]) -> Place {
    let mut w = arm.parent;
    for (k, l) in arm.locals.iter().enumerate() {
        let turn = turn_of(DVec3::new(x[3 * k], x[3 * k + 1], x[3 * k + 2]));
        w = w.apply(&Pose {
            r: l.r * turn,
            ..*l
        });
    }
    let (t, r) = (w.place(arm.strap.t), w.r * arm.strap.r);
    Place {
        centre: t + r * board.centre,
        face: r * board.face,
        up: r * board.up,
    }
}

/// `a` x = `b` by elimination with partial pivoting.
fn solve_linear(mut a: [[f64; 9]; 9], mut b: [f64; 9]) -> [f64; 9] {
    for c in 0..9 {
        let p = (c..9).fold(c, |p, r| if a[r][c].abs() > a[p][c].abs() { r } else { p });
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..9 {
            let k = a[r][c] / a[c][c];
            let pivot = a[c];
            for (each, p) in a[r].iter_mut().zip(pivot).skip(c) {
                *each -= k * p;
            }
            b[r] -= k * b[c];
        }
    }
    let mut x = [0.0; 9];
    for r in (0..9).rev() {
        let s = b[r] - (r + 1..9).map(|j| a[r][j] * x[j]).sum::<f64>();
        x[r] = s / a[r][r];
    }
    x
}

/// The bones' turns (a rotation vector a bone, in its own frame) that put the board on `want`
/// (in the body), the least way: hold.py's solve.
pub fn solve_arm(arm: &Arm, board: &Place, want: &Place) -> [f64; 9] {
    let residual = |x: &[f64; 9]| {
        let b = board_at(arm, board, x);
        let mut r = [0.0; 18];
        let (c, f, u) = (b.centre - want.centre, b.face - want.face, b.up - want.up);
        for j in 0..3 {
            r[j] = 100.0 * c[j];
            r[3 + j] = PER_RADIAN * f[j];
            r[6 + j] = PER_RADIAN * u[j];
        }
        for j in 0..9 {
            r[9 + j] = COST[j / 3] * x[j];
        }
        r
    };
    let sq = |r: &[f64; 18]| r.iter().map(|v| v * v).sum::<f64>();
    let jacobian = |x: &[f64; 9], r: &[f64; 18]| {
        let mut jac = [[0.0; 9]; 18];
        for j in 0..9 {
            let mut e = *x;
            e[j] += 1e-6;
            let rj = residual(&e);
            for i in 0..18 {
                jac[i][j] = (rj[i] - r[i]) / 1e-6;
            }
        }
        jac
    };
    let step_of = |jac: &[[f64; 9]; 18], r: &[f64; 18], damp: f64| {
        let mut a = [[0.0; 9]; 9];
        let mut g = [0.0; 9];
        for p in 0..9 {
            for q in 0..9 {
                a[p][q] = (0..18).map(|i| jac[i][p] * jac[i][q]).sum::<f64>();
            }
            a[p][p] += damp;
            g[p] = -(0..18).map(|i| jac[i][p] * r[i]).sum::<f64>();
        }
        solve_linear(a, g)
    };
    let norm = |d: &[f64; 9]| d.iter().map(|v| v * v).sum::<f64>().sqrt();
    let start = sq(&residual(&[0.0; 9]));
    let mut gn = [0.0; 9];
    let mut settled = false;
    for _ in 0..STEPS {
        let r = residual(&gn);
        let d = step_of(&jacobian(&gn, &r), &r, 1e-6);
        for j in 0..9 {
            gn[j] += d[j];
        }
        if norm(&d) < 1e-7 {
            settled = true;
            break;
        }
    }
    let off = residual(&gn);
    let part = |k: usize| off[k..k + 3].iter().map(|v| v * v).sum::<f64>().sqrt();
    if (settled && sq(&off) <= start) || part(0).max(part(3)).max(part(6)) <= CLOSE {
        return gn;
    }
    // Thrown further than it started or left far off (a place at the arm's full stretch or past
    // it: a short arm's raised block): Levenberg-Marquardt, a step kept only when it brings the
    // board nearer, else damped harder and tried again; the nearer of the two kept.
    let mut x = [0.0; 9];
    let mut r = residual(&x);
    let mut jac = jacobian(&x, &r);
    let mut damp = 1e-6;
    for _ in 0..2 * STEPS {
        let d = step_of(&jac, &r, damp);
        let mut tried = x;
        for j in 0..9 {
            tried[j] += d[j];
        }
        let rt = residual(&tried);
        if sq(&rt) > sq(&r) {
            damp *= 10.0;
            continue;
        }
        (x, r, damp) = (tried, rt, (damp / 10.0).max(1e-6));
        if norm(&d) < 1e-7 {
            break;
        }
        jac = jacobian(&x, &r);
    }
    if sq(&residual(&gn)) < sq(&r) { gn } else { x }
}

/// A place's turn as three square axes: its face, its up made square to it, and their cross.
fn axes(p: &Place) -> [DVec3; 3] {
    let f = p.face.normalize();
    let u = (p.up - f * p.up.dot(f)).normalize();
    [f, u, f.cross(u)]
}

/// How far one place is from another: its centre in cm, its turn in degrees.
pub fn place_off(a: &Place, b: &Place) -> (f64, f64) {
    let (a3, b3) = (axes(a), axes(b));
    let trace: f64 = (0..3).map(|i| a3[i].dot(b3[i])).sum();
    let deg = libm::acos(((trace - 1.0) / 2.0).clamp(-1.0, 1.0)).to_degrees();
    ((a.centre - b.centre).length() * 100.0, deg)
}

/// How far one place is from another in one number: cm, or degrees / 3, the larger.
pub fn off_by(a: &Place, b: &Place) -> f64 {
    let (cm, deg) = place_off(a, b);
    cm.max(deg / 3.0)
}

/// How wholly a board `off` (`off_by`) from its target is pulled onto it: 1 within `NEAR`,
/// 0 from `FAR`.
pub fn weight_of(off: f64) -> f64 {
    let s = ((off - NEAR) / (FAR - NEAR)).clamp(0.0, 1.0);
    1.0 - s * s * (3.0 - 2.0 * s)
}

/// A body's shield arm at a frame, as the fit reads it: the chest (spine_03) in the body and
/// its turn at rest; the shoulder blade's, upper arm's, forearm's, hand's and fist's own poses
/// as the clip and the shape have them, and how far the shape moved each from the base body's.
#[derive(Clone, Copy, Debug)]
pub struct Reading {
    pub chest: Pose,
    pub chest_rest: DQuat,
    pub locals: [Pose; 5],
    pub moved: [DVec3; 5],
}

/// The bones' turns (as `solve_arm`'s) that fit a body's shield arm, each already weighed by how
/// near the base body's board is to its nearest target; none when it is not pulled at all.
pub fn fit(reading: &Reading, board: &Place, targets: &[Place]) -> Option<[f64; 9]> {
    let chest = reading.chest;
    let turned = chest.r * reading.chest_rest.inverse();
    let wants: Vec<Place> = targets
        .iter()
        .map(|p| Place {
            centre: chest.t + turned * p.centre,
            face: turned * p.face,
            up: turned * p.up,
        })
        .collect();
    let [c, u, l, hand, fist] = reading.locals;
    let arm = Arm {
        parent: chest,
        locals: [c, u, l],
        strap: strap_in(&hand, &fist),
    };
    // The base body's arm: each joint where the clip has it, without the shape's move.
    let base: [Pose; 5] = std::array::from_fn(|k| Pose {
        t: reading.locals[k].t - reading.moved[k],
        ..reading.locals[k]
    });
    let base_arm = Arm {
        parent: chest,
        locals: [base[0], base[1], base[2]],
        strap: strap_in(&base[3], &base[4]),
    };
    let at = board_at(&base_arm, board, &[0.0; 9]);
    let offs: Vec<f64> = wants.iter().map(|w| off_by(&at, w)).collect();
    let w = weight_of(offs.iter().copied().fold(f64::INFINITY, f64::min));
    if w <= 0.0 {
        return None;
    }
    Some(solve_arm(&arm, board, &between(&wants, &offs)).map(|v| v * w))
}

/// The targets blended by Shepard's inverse distance weighting (each by 1 / off^`SHEPARD`), so a
/// board on one target goes to it and a board crossfading between two near ones (the guard's
/// and the strafes', 7 cm apart, both in the pull) is pulled along between them instead of
/// snapping from one to the other halfway.
fn between(wants: &[Place], offs: &[f64]) -> Place {
    let by: Vec<f64> = offs
        .iter()
        .map(|d| 1.0 / d.max(1e-6).powi(SHEPARD))
        .collect();
    let all: f64 = by.iter().sum();
    let sum = |f: fn(&Place) -> DVec3| -> DVec3 {
        wants.iter().zip(&by).map(|(p, b)| f(p) * (b / all)).sum()
    };
    let face = sum(|p| p.face).normalize();
    let up = sum(|p| p.up);
    Place {
        centre: sum(|p| p.centre),
        face,
        up: (up - face * up.dot(face)).normalize(),
    }
}

/// On a body with a shield arm: its chest, the arm's bones (`ARM`), its hand and fist, the
/// chest's turn at rest, and what was written over each bone.
#[derive(Component)]
pub struct ShieldArm {
    spine: Entity,
    bones: [Entity; 3],
    hand: Entity,
    fist: Entity,
    chest_rest: DQuat,
    kept: [Kept; 3],
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ShieldTargets>()
        .add_observer(find_arm)
        .add_systems(
            PostUpdate,
            fit_shield_arm
                .in_set(crate::pose::Layer::ShieldArm)
                .after(crate::arms::keep_off_hips)
                .before(TransformSystems::Propagate),
        );
}

/// `entity`'s pose in the body whose model root is `root`, its root's own transform left out.
fn in_body(
    entity: Entity,
    root: Entity,
    parents: &Query<&ChildOf>,
    at: impl Fn(Entity) -> Option<Transform>,
) -> Pose {
    let mut pose = Pose::IDENTITY;
    let mut each = entity;
    while each != root {
        let Some(local) = at(each) else { break };
        pose = Pose::of(&local).apply(&pose);
        let Ok(up) = parents.get(each) else { break };
        each = up.parent();
    }
    pose
}

fn find_arm(
    ready: On<WorldInstanceReady>,
    bodies: Query<(), With<Body>>,
    children: Query<&Children>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    transforms: Query<&Transform>,
    mut commands: Commands,
) {
    let root = ready.entity;
    if !bodies.contains(root) {
        return;
    }
    let wanted = ["spine_03", ARM[0], ARM[1], ARM[2], "hand_l", HAND_L];
    let Some([spine, c, u, l, hand, fist]) = find(root, wanted, &children, &names) else {
        return;
    };
    // As it appears, the body stands at rest.
    let chest_rest = in_body(spine, root, &parents, |e| transforms.get(e).ok().copied()).r;
    commands.entity(root).insert(ShieldArm {
        spine,
        bones: [c, u, l],
        hand,
        fist,
        chest_rest,
        kept: [Kept::default(); 3],
    });
}

/// Each body's shield arm, with a shield strapped at its fist, fitted to the nearest target.
pub(crate) fn fit_shield_arm(
    mut bodies: Query<(Entity, &mut ShieldArm, Option<&Moved>)>,
    straps: Query<&Strapped>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    targets: Res<ShieldTargets>,
    mut transforms: Query<&mut Transform>,
) {
    'bodies: for (root, mut arm, moved) in &mut bodies {
        let arm = &mut *arm;
        let strapped = children.get(arm.bones[2]).is_ok_and(|looks| {
            looks.iter().any(|look| {
                straps
                    .get(look)
                    .is_ok_and(|s| s.hand == arm.hand && s.socket == arm.fist)
            })
        });
        let nodes = [arm.bones[0], arm.bones[1], arm.bones[2], arm.hand, arm.fist];
        let mut locals = [Pose::IDENTITY; 5];
        let mut clip = [Quat::IDENTITY; 3];
        for (k, node) in nodes.into_iter().enumerate() {
            let Ok(t) = transforms.get(node) else {
                continue 'bodies;
            };
            let mut t = *t;
            if k < 3 {
                clip[k] = arm.kept[k].clip(t.rotation);
                t.rotation = clip[k];
            }
            locals[k] = Pose::of(&t);
        }
        let turns = strapped
            .then(|| {
                let by = |node| {
                    moved
                        .and_then(|m| m.0.iter().find(|each| each.node == node))
                        .map_or(Vec3::ZERO, |each| each.by)
                };
                let height = transforms
                    .get(root)
                    .map_or(1.0, |t| t.scale.x as f64)
                    .max(0.01);
                let reading = Reading {
                    chest: in_body(arm.spine, root, &parents, |e| {
                        transforms.get(e).ok().copied()
                    }),
                    chest_rest: arm.chest_rest,
                    locals,
                    moved: nodes.map(|n| by(n).as_dvec3()),
                };
                // The shield keeps its own size whatever the body's height (`keep_held_size`).
                let board = Place {
                    centre: targets.board.centre / height,
                    ..targets.board
                };
                fit(&reading, &board, &targets.targets)
            })
            .flatten();
        for k in 0..3 {
            let Ok(mut t) = transforms.get_mut(arm.bones[k]) else {
                continue;
            };
            let written = match turns {
                Some(x) => {
                    let turn = turn_of(DVec3::new(x[3 * k], x[3 * k + 1], x[3 * k + 2])).as_quat();
                    arm.kept[k].turn(clip[k], Quat::IDENTITY, clip[k] * turn * clip[k].inverse())
                }
                None => {
                    arm.kept[k] = Kept::default();
                    clip[k]
                }
            };
            if t.rotation != written {
                t.rotation = written;
            }
        }
    }
}

#[cfg(test)]
#[path = "shield_arm_tests.rs"]
mod tests;
