//! The hands close on what they hold: whenever a hand socket holds a look, the hand's
//! fingers take a grip pose over whatever clip plays, a finger-only layer, as Lyra and ALS lay a
//! hand pose per weapon grip on the finger chains (docs only). The pose is solved, not keyed:
//! each finger joint, knuckle first, closes from the rest pose about the handle's line until its
//! bone, padded by a finger's half-thickness, touches the handle's surface, as GraspIt!'s
//! auto-grasp closes each link until contact (Miller and Allen, "GraspIt!", IEEE Robotics &
//! Automation Magazine 2004). The handle is read from the look's mesh: its section across the
//! grip (y = 0), its middle and its radius; the pose is solved again only when the look turns in
//! the hand (carried or used, `character::tilt`).
//!
//! Grips by the look's mount (docs/attachment.md): in hand a power grip, the fingers and the
//! thumb round the handle; hung (`"hold": "hung"`) a hook grip, the fingers alone; strapped
//! none, the shield's arm being `strap`'s. Its twin is `tools/characters/grip.ts`, which the move
//! check lays over the clips as this lays it over the game's.

use bevy::{
    gltf::GltfExtras,
    math::Affine3A,
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
    world_serialization::WorldInstanceReady,
};

use crate::bend::find;
use crate::character::{Body, HAND_L, HAND_R, Held, extra};

/// A finger's half-thickness, metres: how far its bone runs off the handle when its pad is on it.
pub const FINGER_M: f32 = 0.008;
/// How far, degrees, a finger's joint may open past its rest (negative).
const OPEN: f32 = -40.0;
/// How far, degrees, a finger's knuckle, middle and end joints close: the knuckle's own reach
/// and the ring's and little finger's palm bones cupping the fist (the skeleton has no palm
/// bones; their 10 to 20 degrees go to the knuckle).
const FINGER_MOST: [f32; 3] = [115.0, 105.0, 80.0];
/// The thumb, opposed square to the handle, opens further first: it may rest pointing anywhere
/// round it. Its base is not turned.
const THUMB_OPEN: f32 = -150.0;
const THUMB_MOST: [f32; 3] = [0.0, 150.0, 80.0];
/// How far from the handle's line a point of its section may be, metres: more is a head or a
/// basket's body.
const NEAR_M: f32 = 0.05;
/// The chains a grip closes, by the bones' stems; each runs to its `_04_leaf`.
pub const CHAINS: [&str; 5] = ["thumb", "index", "middle", "ring", "pinky"];

/// How a hand closes on a look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    /// Fingers and thumb round a handle.
    Power,
    /// The fingers hooked over a handle, the thumb left to the clip.
    Hook,
}

/// The handle a fist closes on: a point on its line, the way it runs and its radius, metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Handle {
    pub at: Vec3,
    pub way: Vec3,
    pub r: f32,
}

/// On a held look whose mount asks for a grip: the grip and its handle, in the look's frame.
#[derive(Component, Clone, Copy, Debug)]
pub struct Gripped {
    pub grip: Grip,
    pub handle: Handle,
}

/// The grip a look's extras ask for: none for a strapped look, a hook for a hung one.
pub fn grip_of(strapped: bool, hold: Option<&str>) -> Option<Grip> {
    if strapped {
        return None;
    }
    Some(if hold == Some("hung") {
        Grip::Hook
    } else {
        Grip::Power
    })
}

/// Where a mesh's triangles cross the plane y = 0 (the grip's): each edge that crosses it, cut.
pub fn cut(points: &[Vec3], tris: &[usize]) -> Vec<Vec3> {
    let mut out = Vec::new();
    for tri in tris.chunks_exact(3) {
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let (p, q) = (points[tri[a]], points[tri[b]]);
            if (p.y < 0.0) == (q.y < 0.0) {
                continue;
            }
            let k = p.y / (p.y - q.y);
            out.push(Vec3::new(p.x + (q.x - p.x) * k, 0.0, p.z + (q.z - p.z) * k));
        }
    }
    out
}

/// The handle across a look's grip from its section at y = 0 (`cut`), the points within `NEAR_M`
/// of the +Y axis: their middle across (of their bounds) and their median distance from it.
pub fn handle(section: &[Vec3]) -> Option<Handle> {
    let near: Vec<Vec3> = section
        .iter()
        .copied()
        .filter(|q| q.x.hypot(q.z) < NEAR_M)
        .collect();
    if near.len() < 3 {
        return None;
    }
    let lo = near.iter().fold(Vec3::INFINITY, |a, q| a.min(*q));
    let hi = near.iter().fold(Vec3::NEG_INFINITY, |a, q| a.max(*q));
    let (x, z) = ((lo.x + hi.x) / 2.0, (lo.z + hi.z) / 2.0);
    let mut radii: Vec<f32> = near.iter().map(|q| (q.x - x).hypot(q.z - z)).collect();
    radii.sort_by(f32::total_cmp);
    Some(Handle {
        at: Vec3::new(x, 0.0, z),
        way: Vec3::Y,
        r: radii[radii.len() / 2],
    })
}

/// A handle's line: a point, its unit way, and the reach of a finger's bone touching it.
#[derive(Clone, Copy, Debug)]
pub struct Line {
    pub c: Vec3,
    pub d: Vec3,
    pub reach: f32,
}

impl Line {
    /// The handle held by a look at `look` (against its socket) on a socket at `socket`, in the
    /// socket's parent's frame.
    pub fn of(handle: Handle, socket: &Transform, look: &Transform) -> Line {
        let place = socket.mul_transform(*look);
        Line {
            c: place.transform_point(handle.at),
            d: (place.rotation * handle.way).normalize(),
            reach: (handle.r + FINGER_M) * place.scale.x,
        }
    }

    /// How far a segment comes to the line, from `from` (a share of it) on.
    pub fn to_segment(&self, a: Vec3, b: Vec3, from: f32) -> f32 {
        let mut most = f32::INFINITY;
        for i in (libm::roundf(from * 8.0) as usize)..=8 {
            let v = a.lerp(b, i as f32 / 8.0) - self.c;
            most = most.min((v - self.d * v.dot(self.d)).length());
        }
        most
    }
}

/// A hand's grip pose: each chain's (`CHAINS`) bones' rotations against their parents, from
/// `chains` (each chain's four bones' rest transforms against their parents, the hand's own
/// frame for the first, the last a leaf), closed round `line` in the hand's frame. None where
/// the clip keeps the bone (a hook grip's thumb).
pub fn close(chains: &[[Transform; 4]; 5], line: &Line, grip: Grip) -> [[Option<Quat>; 3]; 5] {
    let mut out = [[None; 3]; 5];
    // Which way about the handle's line a finger closes: on round it the way the index's
    // knuckle bone points at rest. The thumb comes round the other way, to meet the fingers.
    let knuckle = chains[1][0];
    let ahead = knuckle.rotation * chains[1][1].translation;
    let sense = line
        .d
        .cross(knuckle.translation - line.c)
        .dot(ahead)
        .signum();
    for (k, chain) in chains.iter().enumerate() {
        let thumb = CHAINS[k] == "thumb";
        if thumb && grip == Grip::Hook {
            continue;
        }
        // The thumb's base is put as it rests, in the palm, whatever the clip turns it to.
        let first = usize::from(thumb);
        let mut parent = Transform::IDENTITY;
        if thumb {
            parent = chain[0];
            out[k][0] = Some(chain[0].rotation);
        }
        for j in first..3 {
            let at_rest = parent.mul_transform(chain[j]);
            let child = chain[j + 1].translation;
            let mut from = at_rest.rotation;
            let mut u = (from * child).normalize();
            if thumb && j == first {
                // Opposed: the thumb turned square to the handle, to come round it.
                let flat = (u - line.d * u.dot(line.d)).normalize();
                from = Quat::from_rotation_arc(u, flat) * from;
                u = flat;
            }
            // Each joint closes about the handle's way, square to its bone.
            let side = if thumb { -sense } else { sense };
            let hinge = (line.d - u * line.d.dot(u)).normalize() * side;
            let turned = |deg: f32| Transform {
                rotation: Quat::from_axis_angle(hinge, deg.to_radians()) * from,
                ..at_rest
            };
            // A joint past the first starts on the handle already: what touches is the rest
            // of its bone. So does a knuckle already in it (a handle thicker than the fist was
            // made for).
            let on = j > first
                || line.to_segment(at_rest.translation, at_rest.translation, 0.0) <= line.reach;
            let gap = |deg: f32| {
                let p = turned(deg);
                line.to_segment(
                    p.translation,
                    p.transform_point(child),
                    if on { 0.3 } else { 0.0 },
                ) - line.reach
            };
            let opposed = thumb && j == first;
            let (lo, most) = if thumb {
                (if opposed { THUMB_OPEN } else { OPEN }, THUMB_MOST[j])
            } else {
                (OPEN, FINGER_MOST[j])
            };
            let world = turned(closing(gap, lo, most));
            out[k][j] = Some(parent.rotation.inverse() * world.rotation);
            parent = world;
        }
    }
    out
}

/// The least turn from `lo` to `most` degrees, in steps of 2 and then halved, at which the bone
/// touches (`gap` at most 0); if it never does, the turn that brings it nearest. One already in
/// at `lo` starts from the turn where it is least in: there, if it is in at every turn.
fn closing(gap: impl Fn(f32) -> f32, mut lo: f32, most: f32) -> f32 {
    if gap(lo) <= 0.0 {
        let mut d = lo;
        while d <= most {
            if gap(d) > gap(lo) {
                lo = d;
            }
            d += 2.0;
        }
        if gap(lo) <= 0.0 {
            return lo;
        }
    }
    let mut near = lo;
    let mut d = lo + 2.0;
    while d <= most {
        if gap(d) < gap(near) {
            near = d;
        }
        if gap(d) <= 0.0 {
            let (mut a, mut b) = (d - 2.0, d);
            for _ in 0..12 {
                let m = (a + b) / 2.0;
                if gap(m) <= 0.0 {
                    b = m;
                } else {
                    a = m;
                }
            }
            return b;
        }
        d += 2.0;
    }
    near
}

/// One hand of a body: its socket, its chains' bones and their rest rotations, and the pose
/// last solved, for which look turned how.
struct Hand {
    socket: Entity,
    bones: [[Entity; 4]; 5],
    rest: [[Quat; 4]; 5],
    solved: Option<(Entity, Quat, [[Option<Quat>; 3]; 5])>,
}

/// On a body whose model has hands with fingers: the left hand, then the right.
#[derive(Component)]
pub struct Hands([Option<Hand>; 2]);

pub fn plugin(app: &mut App) {
    app.add_observer(find_hands)
        .add_observer(find_handle)
        .add_systems(
            PostUpdate,
            // After the clips and the fit, before the arms are kept off the hips (which count
            // the hand's points) and the propagation the duel record reads (D530).
            close_hands
                .after(bevy::app::AnimationSystems)
                .after(crate::Fitted)
                .before(crate::arms::keep_off_hips)
                .before(TransformSystems::Propagate),
        );
}

/// The four bones of each chain on side `s` ("l" or "r").
fn chain_names(s: &str) -> [[String; 4]; 5] {
    CHAINS.map(|stem| {
        [1, 2, 3]
            .map(|k| format!("{stem}_0{k}_{s}"))
            .into_iter()
            .chain([format!("{stem}_04_leaf_{s}")])
            .collect::<Vec<_>>()
            .try_into()
            .expect("four bones")
    })
}

/// When a body's model is in, still at rest: each hand's socket and fingers, as they rest.
fn find_hands(
    ready: On<WorldInstanceReady>,
    bodies: Query<(), With<Body>>,
    children: Query<&Children>,
    names: Query<&Name>,
    transforms: Query<&Transform>,
    mut commands: Commands,
) {
    let root = ready.entity;
    if !bodies.contains(root) {
        return;
    }
    let hand = |socket: &str, s: &str| -> Option<Hand> {
        let [socket] = find(root, [socket], &children, &names)?;
        let named = chain_names(s);
        let mut bones = [[Entity::PLACEHOLDER; 4]; 5];
        let mut rest = [[Quat::IDENTITY; 4]; 5];
        for (k, chain) in named.iter().enumerate() {
            let wanted: [&str; 4] = [&chain[0], &chain[1], &chain[2], &chain[3]];
            bones[k] = find(root, wanted, &children, &names)?;
            for j in 0..4 {
                rest[k][j] = transforms.get(bones[k][j]).ok()?.rotation;
            }
        }
        Some(Hand {
            socket,
            bones,
            rest,
            solved: None,
        })
    };
    let sides = [hand(HAND_L, "l"), hand(HAND_R, "r")];
    if sides.iter().any(Option::is_some) {
        commands.entity(root).insert(Hands(sides));
    }
}

/// Once a held look has loaded, its grip and handle: the section of its meshes across y = 0,
/// each placed by its nodes' transforms under the look's root.
fn find_handle(
    ready: On<WorldInstanceReady>,
    held: Query<(), With<Held>>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    transforms: Query<&Transform>,
    extras: Query<&GltfExtras>,
    meshes3d: Query<&Mesh3d>,
    meshes: Res<Assets<Mesh>>,
    mut commands: Commands,
) {
    let root = ready.entity;
    if !held.contains(root) {
        return;
    }
    let mut strapped = false;
    let mut hold = None;
    let mut section = Vec::new();
    for each in children.iter_descendants(root) {
        if let Ok(extras) = extras.get(each) {
            strapped |= extra(extras, "strap").is_some();
            hold = hold.or_else(|| extra(extras, "hold")?.as_str().map(str::to_string));
        }
        let Some(mesh) = meshes3d.get(each).ok().and_then(|m| meshes.get(&m.0)) else {
            continue;
        };
        let Some(VertexAttributeValues::Float32x3(at)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        let mut place = Affine3A::IDENTITY;
        let mut node = each;
        while node != root {
            place = transforms
                .get(node)
                .map_or(Affine3A::IDENTITY, |t| t.compute_affine())
                * place;
            let Ok(up) = parents.get(node) else {
                break;
            };
            node = up.parent();
        }
        let points: Vec<Vec3> = at
            .iter()
            .map(|&p| place.transform_point3(Vec3::from(p)))
            .collect();
        let tris: Vec<usize> = match mesh.indices() {
            Some(Indices::U16(i)) => i.iter().map(|&i| usize::from(i)).collect(),
            Some(Indices::U32(i)) => i.iter().map(|&i| i as usize).collect(),
            None => (0..points.len()).collect(),
        };
        section.extend(cut(&points, &tris));
    }
    if let (Some(grip), Some(handle)) = (grip_of(strapped, hold.as_deref()), handle(&section)) {
        commands.entity(root).insert(Gripped { grip, handle });
    }
}

/// Each hand that holds a gripped look closed on it: solved again when the look turns in the
/// hand, written over the clip every frame.
pub fn close_hands(
    mut bodies: Query<&mut Hands>,
    children: Query<&Children>,
    gripped: Query<&Gripped>,
    mut transforms: Query<&mut Transform>,
) {
    for mut hands in &mut bodies {
        for hand in hands.0.iter_mut().flatten() {
            let look = children
                .get(hand.socket)
                .ok()
                .and_then(|c| c.iter().find(|each| gripped.contains(*each)));
            let Some(look) = look else {
                hand.solved = None;
                continue;
            };
            let (Ok(grip), Ok(at), Ok(socket)) = (
                gripped.get(look),
                transforms.get(look).copied(),
                transforms.get(hand.socket).copied(),
            ) else {
                continue;
            };
            if !hand
                .solved
                .is_some_and(|(was, turn, _)| was == look && turn == at.rotation)
            {
                let mut chains = [[Transform::IDENTITY; 4]; 5];
                for k in 0..5 {
                    for j in 0..4 {
                        let Ok(now) = transforms.get(hand.bones[k][j]) else {
                            continue;
                        };
                        chains[k][j] = Transform {
                            rotation: hand.rest[k][j],
                            ..*now
                        };
                    }
                }
                let line = Line::of(grip.handle, &socket, &at);
                hand.solved = Some((look, at.rotation, close(&chains, &line, grip.grip)));
            }
            let Some((_, _, pose)) = hand.solved else {
                continue;
            };
            for (k, chain) in pose.iter().enumerate() {
                for (j, turn) in chain.iter().enumerate() {
                    if let (Some(turn), Ok(mut bone)) = (turn, transforms.get_mut(hand.bones[k][j]))
                    {
                        bone.rotation = *turn;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "grip_tests.rs"]
mod tests;
