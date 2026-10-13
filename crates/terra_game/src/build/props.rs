//! Doors and windows fitted into the player's base (construction-001, rule 7 and gap 15;
//! D192): each a prop the base holds in a hole of built cubes, drawn from its kit's door or
//! window model fitted to the hole, a door's leaf swung open on its hinge, its frame staying,
//! when it is open. What a
//! walker meets follows the base: a shut door's cubes stop them (`terra_build::walk`). The
//! interact key opens and shuts a door, through the same operation the remote uses.

use super::act::Building;
use super::{BuildLook, Site, pieces};
use crate::play::interact::{CanInteract, Interactable, Interacted};
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_build::ops::Op;
use terra_build::{Base, CELL_M, Prop, PropKind};

/// The kinds of prop, as the operations number them.
pub const DOOR: PropKind = PropKind(0);
pub const WINDOW: PropKind = PropKind(1);

/// How far a door swings open, in degrees, inward.
const SWING_DEGREES: f32 = 95.0;
/// How thick a fitted door or window is drawn, about a log's width, in the middle of its
/// cube.
const DEPTH_M: f32 = 0.32;

/// A door or window drawn where the base has one fitted: its lowest cube, and how it was
/// when drawn.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Fitted {
    pub at: [i32; 3],
    pub prop: Prop,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (show, open_and_shut));
}

/// The name of a kind of prop.
pub fn name(kind: PropKind) -> &'static str {
    if kind == WINDOW { "window" } else { "door" }
}

/// Draws each prop the base has fitted, as it is, whenever the base or the kit's models
/// change.
fn show(
    mut commands: Commands,
    site: Res<Site>,
    look: Res<BuildLook>,
    drawn: Query<(Entity, &Fitted)>,
    mut seen: Local<Option<(u64, bool)>>,
) {
    let now = (site.revision, look.kit_pieces.is_some());
    if *seen == Some(now) {
        return;
    }
    *seen = Some(now);
    let wanted: BTreeMap<[i32; 3], Prop> = site
        .base
        .props()
        .filter(|_| site.by_plans)
        .map(|prop| (prop.min, *prop))
        .collect();
    let mut have: BTreeMap<[i32; 3], Prop> = BTreeMap::new();
    for (entity, fitted) in &drawn {
        if wanted.get(&fitted.at) == Some(&fitted.prop) && look.kit_pieces.is_some() {
            have.insert(fitted.at, fitted.prop);
        } else {
            commands.entity(entity).despawn();
        }
    }
    let Some(kit) = look.kit_pieces.as_deref() else {
        return;
    };
    for (at, prop) in wanted {
        if have.contains_key(&at) {
            continue;
        }
        let door = prop.kind != WINDOW;
        let (foot, parts) = parts(kit, &site.base, &prop);
        let mut entity = commands.spawn((
            Name::new(if door { "Door" } else { "Window" }),
            Fitted { at, prop },
            Transform::from_translation(foot),
            Visibility::default(),
            ChildOf(site.root),
        ));
        if door {
            entity.insert(CanInteract(Interactable::Door));
        }
        for (mesh, material, placed) in parts {
            entity.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(-foot) * placed,
            ));
        }
    }
}

/// A fitted door's or window's model, fitted to its hole, in the base's own space: the
/// middle of its foot, and each part with where it goes. Open, a door's leaf swings into the
/// room on its hinge; its frame stays.
pub(super) fn parts(
    kit: &pieces::KitPieces,
    base: &Base,
    prop: &Prop,
) -> (
    Vec3,
    Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)>,
) {
    let (low, high) = (IVec3::from_array(prop.min), IVec3::from_array(prop.max));
    let tile = IVec3::new(low.x, terra_build::storey_of(low.y), low.z);
    let along_x = high.x > low.x || (high.z == low.z && base.along(tile) == IVec3::X);
    let (min, max) = (
        low.as_vec3() * CELL_M,
        (high + IVec3::ONE).as_vec3() * CELL_M,
    );
    // As thick as a wall's logs, in the middle of the cubes.
    let middle = (min + max) * 0.5;
    let half_depth = Vec3::new(DEPTH_M * 0.5, 0.0, DEPTH_M * 0.5);
    let across = if along_x { Vec3::Z } else { Vec3::X };
    let (min, max) = (
        min * (Vec3::ONE - across) + (middle - half_depth) * across,
        max * (Vec3::ONE - across) + (middle + half_depth) * across,
    );
    let foot = Vec3::new(middle.x, min.y, middle.z);
    let door = prop.kind != WINDOW;
    // Open, a door swings into the room, away from the way out.
    let swing = if door && prop.open {
        let inward = base
            .outward(tile)
            .map_or(1.0, |out| -out.as_vec3().dot(across).signum());
        let sign = if along_x { -1.0 } else { 1.0 };
        (SWING_DEGREES * inward * sign).to_radians()
    } else {
        0.0
    };
    (foot, pieces::opening(kit, min, max, along_x, door, swing))
}

/// Opens or shuts a door a player uses.
fn open_and_shut(
    mut used: MessageReader<Interacted>,
    fitted: Query<&Fitted>,
    mut building: Building,
) {
    for interacted in used.read() {
        let Some(door) = interacted
            .on(Interactable::Door)
            .and_then(|entity| fitted.get(entity).ok())
        else {
            continue;
        };
        let open = !door.prop.open;
        let what = if open {
            "open the door"
        } else {
            "shut the door"
        };
        let _ = building.act(
            Some(interacted.player),
            what,
            &[Op::SetOpen { at: door.at, open }],
        );
    }
}
