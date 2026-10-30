//! Something held is used in one pose and carried in another: swung, the grip holds it as a
//! hand would to strike; walking or standing, its arm takes the carry pose its file names
//! (`carry_hold`, docs/attachment.md, "Carried"), the look untipped in the fist.

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
/// On a hand socket: whether its arm is in the carry pose of what it holds now (`CarryHold`
/// laid in a carried clip with no stance hold), which `carry_arm` holds in the body's frame.
#[derive(Component, Default)]
pub struct CarryPosed(pub bool);

/// On a hand socket: the carry pose of the look it holds, its file's `carry_hold` (D560: a lit
/// torch on a raised arm), a clip whose first pose that hand's arm takes whenever the look is
/// carried, laid by the hold layer on that arm alone (`layer`, `figure`). Every look held in
/// the fist names one (`every_look_in_hand_names_a_carry_pose`).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct CarryHold {
    pub look: Entity,
    pub clip: String,
    /// Whether the pose must stay upright however the chest leans (its file's `carry_upright`,
    /// from the pose's `upright` in art/animation/sets/fight.json: the torch's flame held up),
    /// its arm then held in the body's frame (`carry_arm`); else the arm leans with the chest,
    /// as the hold layer lays it (a hanging tool's Carry_Tool).
    pub upright: bool,
}

/// Something held, a child of a hand socket, or of the bone it is strapped to (`strap`).
#[derive(Component)]
pub struct Held {
    /// Its turn and scale against its parent's frame: none on a hand socket; strapped, the
    /// strap's turn and the socket's scale against the bone's.
    pub(crate) turn: Quat,
    pub(crate) fit: f32,
}

impl Default for Held {
    fn default() -> Self {
        Held {
            turn: Quat::IDENTITY,
            fit: 1.0,
        }
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
/// on the hand socket it is held in, before a strap moves it to its bone (`CarryHold`).
pub(super) fn held_ready(
    ready: On<WorldInstanceReady>,
    held: Query<(), With<Held>>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    extras: Query<&GltfExtras>,
    mut commands: Commands,
) {
    if !held.contains(ready.entity) {
        return;
    }
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
                    upright: given("carry_upright").and_then(|u| u.as_bool()) == Some(true),
                });
            }
            None => {
                commands.entity(socket).remove::<CarryHold>();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// A look held in the fist is carried in a pose its file names (D584's holding as an
    /// overlay from a clip): none is left to hang from the clip's hand as it was used, tipped by
    /// nothing. A look hung from the hand or strapped is carried as its hold or strap places it.
    #[test]
    fn every_look_in_hand_names_a_carry_pose() {
        for file in crate::model_files::glbs("models/held") {
            let json = crate::model_files::json(&file);
            let has = |key| crate::model_files::extra(&json, key).is_some();
            assert!(
                has("carry_hold") || has("hold") || has("strap"),
                "{file}: a look in the fist names no carry pose; give its art/held/<item>.json a \
                 \"carry_hold\" (a pose of art/animation/sets/fight.json: Carry_Tool for a \
                 one-handed tool) and fit it again (node tools/characters/fit.ts <item>)"
            );
        }
    }

    /// Nothing tips a look in the fist any more (its carry pose holds it, D546): a look whose
    /// file still gives a `carry_tilt` would be carried other than its maker meant, unsaid.
    #[test]
    fn no_held_look_gives_a_carry_tilt() {
        let looks = crate::model_files::glbs("models/held");
        assert!(!looks.is_empty());
        for file in &looks {
            let json = crate::model_files::json(file);
            assert!(
                crate::model_files::extra(&json, "carry_tilt").is_none(),
                "{file}: gives a carry_tilt, which nothing reads; tip the look by its carry pose \
                 (\"carry_hold\" in art/held/<item>.json) and fit it again (node \
                 tools/characters/fit.ts <item>)"
            );
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

    /// Whether a carry pose stays upright is the pose's data (its `upright` in its set,
    /// art/animation/sets/fight.json), copied into each look naming it when the look is fitted;
    /// a look fitted before its pose changed says so here, not as an arm in the wrong frame.
    #[test]
    fn every_look_carries_its_pose_upright_as_its_set_says() {
        let set = crate::model_files::asset("../art/animation/sets/fight.json");
        let set: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&set).unwrap()).unwrap();
        for file in crate::model_files::glbs("models/held") {
            let json = crate::model_files::json(&file);
            let Some(pose) = crate::model_files::extra(&json, "carry_hold") else {
                continue;
            };
            let pose = pose.as_str().unwrap();
            let wanted = set[pose]["upright"].as_bool() == Some(true);
            let has = crate::model_files::extra(&json, "carry_upright").and_then(|u| u.as_bool());
            assert_eq!(
                has == Some(true),
                wanted,
                "{file}: its pose {pose} is {}upright in art/animation/sets/fight.json but the look \
                 says otherwise; fit it again (node tools/characters/fit.ts <item>)",
                if wanted { "" } else { "not " }
            );
        }
    }
}
