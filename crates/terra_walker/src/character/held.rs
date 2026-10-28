//! Something held is used at one angle and carried at another: swung, the grip holds it as a
//! hand would to strike; walking or standing, it tips forward and down by its file's
//! `carry_tilt`, or else by the carry rule (`carry_rule`, docs/attachment.md, "Carried").

use super::extra;
use bevy::{
    gltf::GltfExtras, mesh::VertexAttributeValues, prelude::*,
    world_serialization::WorldInstanceReady,
};

/// The clips in which something in hand is carried rather than used.
pub const CARRIED_IN: [&str; 10] = [
    "Idle_Loop",
    "Idle_Talking_Loop",
    "Walk_Loop",
    "Jog_Fwd_Loop",
    "Sprint_Loop",
    "Jump_Start",
    "Jump_Loop",
    "Jump_Land",
    "Crouch_Idle_Loop",
    "Crouch_Fwd_Loop",
];
/// How quickly something in hand turns between carried and used, in seconds.
const TURN_S: f32 = 0.12;

/// Carried, a look in hand whose file gives no `carry_tilt` tips its working end this far,
/// degrees, forward and down from the hanging fist, as a tool hangs from a relaxed hand, so
/// a new item needs no number set by hand (docs/attachment.md, "Carried"). 45 clears every
/// look in hand on every template in every carried clip with the arm spacing (`arms::HELD`);
/// level or raised, a bent arm's sprint turns the working end back into the head (the move
/// check's sweep, item-attach job 5).
pub const CARRY_DOWN: f32 = 45.0;
/// ...but its working end no further than this below the grip, metres: the hanging fist is
/// about 0.75 m up on the base, so a long look (a spear) tips less and stays off the ground.
pub const CARRY_REACH: f32 = 0.55;

/// The tilt a look in hand whose working end reaches `ahead` metres up its handle from the
/// grip is carried at, by the rule (`CARRY_DOWN`, `CARRY_REACH`). Its twin is
/// `tools/characters/held.ts`'s `carryRule`.
pub fn carry_rule(ahead: f32) -> f32 {
    let most = libm::sinf(CARRY_DOWN.to_radians());
    let down = if ahead > CARRY_REACH {
        (CARRY_REACH / ahead).min(most)
    } else {
        most
    };
    -libm::asinf(down).to_degrees()
}

/// On a hand socket: whether what it holds is being carried (`CARRIED_IN`) or used.
#[derive(Component, Default)]
pub struct Carrying(pub bool);

/// On a hand socket: whether its arm is in the carry pose of what it holds now (`CarryHold`
/// laid in a carried clip with no stance hold), which `carry_arm` holds in the body's frame.
#[derive(Component, Default)]
pub struct CarryPosed(pub bool);

/// On a hand socket: the carry pose of the look it holds, its file's `carry_hold` (D560: a lit
/// torch on a raised arm), a clip whose first pose that hand's arm takes whenever the look is
/// carried, laid by the hold layer on that arm alone (`layer`, `figure`). A look whose file
/// gives none is carried by the rule (`carry_rule`).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct CarryHold {
    pub look: Entity,
    pub clip: String,
}

/// Something held, a child of a hand socket, or of the bone it is strapped to (`strap`).
/// Carried, it turns `carry_tilt` degrees about its X: negative tips its working end forward
/// and down when the arm hangs, positive raises it.
#[derive(Component)]
pub struct Held {
    pub carry_tilt: f32,
    now: f32,
    /// Its turn and scale against its parent's frame before any tilt: none on a hand socket;
    /// strapped, the strap's turn and the socket's scale against the bone's.
    pub(crate) turn: Quat,
    pub(crate) fit: f32,
}

impl Held {
    pub fn new(carry_tilt: f32) -> Self {
        Held {
            carry_tilt,
            now: 0.0,
            turn: Quat::IDENTITY,
            fit: 1.0,
        }
    }

    /// Its turn against its parent's frame now, the share of its tilt it takes in the fist laid
    /// on (`in_fist`).
    pub(crate) fn turned(&self) -> Quat {
        self.turn * Quat::from_rotation_x(self.in_fist().to_radians())
    }

    /// How far, degrees, it is turned in the fist now: its tilt up to `carry_hand::OBLIQUE`, a
    /// handle run slantwise across the palm as a power grip holds it; the wrist turns the rest
    /// (`at_wrist`), so the fingers still close round it.
    pub fn in_fist(&self) -> f32 {
        let most = crate::carry_hand::OBLIQUE;
        self.now.clamp(-most, most)
    }

    /// How far, degrees, the hand holding it turns at the wrist now, about the hand socket's X
    /// (`carry_hand`): its tilt past `in_fist`.
    pub fn at_wrist(&self) -> f32 {
        self.now - self.in_fist()
    }

    /// How far it has turned toward its carried tilt, 0 used to 1 carried; 0 with no tilt.
    pub fn carried(&self) -> f32 {
        if self.carry_tilt == 0.0 {
            0.0
        } else {
            (self.now / self.carry_tilt).clamp(0.0, 1.0)
        }
    }
}

/// Turns what is held toward its carried or used angle.
pub(super) fn tilt(
    time: Res<Time>,
    hands: Query<&Carrying>,
    mut held: Query<(&mut Held, &mut Transform, &ChildOf)>,
) {
    let ease = 1.0 - libm::expf(-time.delta_secs() / TURN_S);
    for (mut held, mut transform, parent) in &mut held {
        let carried = hands.get(parent.parent()).is_ok_and(|hand| hand.0);
        let goal = if carried { held.carry_tilt } else { 0.0 };
        // Settled: left alone, so what hangs from it is not moved again for nothing.
        if held.now == goal {
            continue;
        }
        held.now = if (goal - held.now).abs() < 0.01 {
            goal
        } else {
            held.now + (goal - held.now) * ease
        };
        transform.rotation = held.turned();
    }
}

/// Every point of the meshes under a look's `root`, placed by its nodes' transforms under it:
/// in the look's own frame (the grip at the origin, the handle along +Y).
pub(crate) fn look_points(
    root: Entity,
    children: &Query<&Children>,
    parents: &Query<&ChildOf>,
    transforms: &Query<&Transform>,
    meshes3d: &Query<&Mesh3d>,
    meshes: &Assets<Mesh>,
) -> Vec<Vec3> {
    let mut points = Vec::new();
    for each in children.iter_descendants(root) {
        let Some(mesh) = meshes3d.get(each).ok().and_then(|m| meshes.get(&m.0)) else {
            continue;
        };
        let Some(VertexAttributeValues::Float32x3(at)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        let mut place = bevy::math::Affine3A::IDENTITY;
        let mut node = each;
        while node != root {
            place = transforms
                .get(node)
                .map_or(bevy::math::Affine3A::IDENTITY, |t| t.compute_affine())
                * place;
            let Ok(up) = parents.get(node) else {
                break;
            };
            node = up.parent();
        }
        points.extend(at.iter().map(|&p| place.transform_point3(Vec3::from(p))));
    }
    points
}

/// Once a held look has loaded, how it is carried: its file's `carry_hold` (a carry pose) goes
/// on the hand socket it is held in, before a strap moves it to its bone (`CarryHold`); and how
/// far it tips: its file's `carry_tilt` where it gives one, else none for a look hung from the
/// hand, strapped or carried in a pose (`hold`, `strap`, `carry_hold`), else the rule from how
/// far its working end reaches (`carry_rule`).
#[allow(clippy::too_many_arguments)]
pub(super) fn held_ready(
    ready: On<WorldInstanceReady>,
    mut held: Query<&mut Held>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    transforms: Query<&Transform>,
    extras: Query<&GltfExtras>,
    meshes3d: Query<&Mesh3d>,
    meshes: Res<Assets<Mesh>>,
    mut commands: Commands,
) {
    let Ok(mut held) = held.get_mut(ready.entity) else {
        return;
    };
    let given = |key| {
        children
            .iter_descendants(ready.entity)
            .find_map(|each| extra(extras.get(each).ok()?, key))
    };
    let pose = given("carry_hold").and_then(|c| c.as_str().map(str::to_string));
    if let Ok(socket) = parents.get(ready.entity).map(ChildOf::parent) {
        match &pose {
            Some(clip) => {
                commands.entity(socket).insert(CarryHold {
                    look: ready.entity,
                    clip: clip.clone(),
                });
            }
            None => {
                commands.entity(socket).remove::<CarryHold>();
            }
        }
    }
    held.carry_tilt = if let Some(tilt) = given("carry_tilt").and_then(|t| t.as_f64()) {
        tilt as f32
    } else if given("hold").is_some() || given("strap").is_some() || pose.is_some() {
        0.0
    } else {
        let points = look_points(
            ready.entity,
            &children,
            &parents,
            &transforms,
            &meshes3d,
            &meshes,
        );
        carry_rule(points.iter().map(|p| p.y).fold(0.0, f32::max))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A short look's working end tips `CARRY_DOWN`; a long one's no lower than `CARRY_REACH`
    /// under the grip, found from where its end lands rather than the formula.
    #[test]
    fn a_look_in_hand_is_carried_tipped_down_but_its_end_kept_off_the_ground() {
        for ahead in [0.1, 0.3, 0.6, 0.7] {
            assert!((carry_rule(ahead) + CARRY_DOWN).abs() < 1e-4, "{ahead}");
        }
        for ahead in [0.9, 1.05, 1.6] {
            let tilt = carry_rule(ahead).to_radians();
            // The end of the handle in a hanging hand's frame: +Y forward, +Z up the arm.
            let end = Quat::from_rotation_x(tilt) * Vec3::Y * ahead;
            assert!((end.z + CARRY_REACH).abs() < 1e-4, "{ahead}: {end}");
            assert!(tilt < 0.0 && tilt > -CARRY_DOWN.to_radians());
        }
    }

    #[test]
    fn every_held_look_says_how_it_tips_in_a_number() {
        let looks = crate::model_files::glbs("models/held");
        assert!(!looks.is_empty());
        for file in &looks {
            let json = crate::model_files::json(file);
            if let Some(tilt) = crate::model_files::extra(&json, "carry_tilt") {
                assert!(tilt.as_f64().is_some(), "{file}: carry_tilt {tilt}");
            }
        }
    }

    /// A carry pose named wrong is never laid, and nothing says so (D560): every look's
    /// `carry_hold` is a clip of the figure's file.
    #[test]
    fn every_carry_pose_a_look_names_is_a_clip_the_figure_plays() {
        let clips = crate::model_files::animations(&crate::model_files::json(crate::figure::CLIPS));
        for file in crate::model_files::glbs("models/held") {
            let json = crate::model_files::json(&file);
            if let Some(pose) = crate::model_files::extra(&json, "carry_hold") {
                let pose = pose
                    .as_str()
                    .unwrap_or_else(|| panic!("{file}: carry_hold {pose}"));
                assert!(clips.iter().any(|c| c == pose), "{file}: no clip {pose}");
            }
        }
    }
}
