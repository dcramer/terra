//! The dungeon put up: its cubes as a build, and what stands in it (props, fires, chests,
//! doors, the goal and keys), all under one entity at the dungeon's frame; its creatures once
//! their kinds are ready.

use super::{Pending, dir};
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use std::f32::consts::FRAC_PI_2;
use terra_creatures::creatures::{Form, Kinds};
use terra_creatures::species::Bestiary;
use terra_dungeon::stamp::{DOORWAY_M, LightKind, ObjectRole};
use terra_game::build::{Build, BuildLook, Built};
use terra_world::fire::{Kindled, Logs};
use terra_world::planet::Planet;

const TORCH: &str = "models/tools/Torch.glb";
const CAMPFIRE: &str = "models/props/Campfire.glb";
const CHEST: &str = "models/props/Chest.glb";
/// The torch's head over its foot, as `terra_survival::torchlight` holds it.
const TORCH_HEAD_M: f32 = 0.47;
/// A wall torch leans so far out from its wall, in degrees.
const TORCH_LEAN_DEG: f32 = 20.0;
/// A torch's flames: a hand's width, knee-high at most, less hot than a campfire.
const TORCH_FIRE: Kindled = Kindled {
    tall: 0.45,
    wide: 0.12,
    heat: 0.6,
    logs: Logs::NONE,
    body: terra_world::fire::Body::Cards,
};
/// A brazier's fire burns in its bowl, so far under the light's point.
const BRAZIER_DROP_M: f32 = 0.5;
/// A fire bowl by the goal: the campfire's stones and logs at this size on a pillar's top.
const BOWL_SCALE: f32 = 0.45;
/// Its flames: the campfire's at the bowl's size, about twice a torch's light.
const BOWL_FIRE: Kindled = Kindled {
    tall: 0.7,
    wide: 0.32,
    heat: 0.9,
    logs: terra_world::fire::Logs::CAMPFIRE.below(0.08),
    body: terra_world::fire::Body::Marched,
};
/// The studded leaf's model, hinged at its origin and reaching along +X: so wide and so high.
const LEAF_M: Vec2 = Vec2::new(0.894, 1.939);

/// A dungeon object the game makes live: the goal, a key, a door leaf.
#[derive(Component, Debug)]
#[allow(dead_code)]
pub struct DungeonObject {
    pub role: ObjectRole,
    pub name: String,
    pub locked: bool,
}

/// The dungeon's creatures, in the world, waiting for their kinds to be ready.
#[derive(Resource)]
pub(super) struct Mobs(Vec<(String, Vec3)>);

/// A turn about up so a model's front (+Z) faces a side.
fn facing(side: usize) -> Quat {
    Quat::from_rotation_y((2.0 - side as f32) * FRAC_PI_2)
}

/// A doorway's two leaves, a double door as a gate of its width is hung: each hinged at one
/// jamb and reaching to the middle, the second turned half round so it reaches back (a
/// leaf's planks and studs are alike on both faces); where each hinge stands and how it
/// turns, from the middle of the sill and the doorway's turn.
fn leaves(sill: Vec3, turn: Quat) -> [(Vec3, Quat); 2] {
    let along = turn * Vec3::X * (DOORWAY_M[0] * 0.5);
    [
        (sill - along, turn),
        (
            sill + along,
            turn * Quat::from_rotation_y(std::f32::consts::PI),
        ),
    ]
}

/// The leaves swung open into the room, each from its hinge back along its jamb, so the
/// doorway between is clear: how a door opens, and whether a walker or a key opens it, is
/// the developers' call (docs/dungeons.md); until then the demo's doors stand open, so a
/// walker and the camera pass.
fn open(sill: Vec3, turn: Quat) -> [(Vec3, Quat); 2] {
    leaves(sill, turn).map(|(hinge, _)| (hinge, turn * Quat::from_rotation_y(FRAC_PI_2)))
}

/// The studded leaf stretched to half a doorway's width and its full height.
fn leaf_scale() -> Vec3 {
    Vec3::new(DOORWAY_M[0] * 0.5 / LEAF_M.x, DOORWAY_M[1] / LEAF_M.y, 1.0)
}

fn scene(assets: &AssetServer, model: &str) -> WorldAssetRoot {
    WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(model.to_string())))
}

/// Lays the dungeon in cubes and spawns what stands in it, once building's look is in.
pub(super) fn put_up(
    mut commands: Commands,
    pending: Res<Pending>,
    look: Res<BuildLook>,
    planet: Res<Planet>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Pending { stamp, frame } = &*pending;
    let mut build = Build::new(&mut commands, &look, *frame, "Dungeon");
    // Without kits while it is laid, so building lays no footings or door steps of its own
    // (the roof's slab would turn to footing stone, and steps would fill the dug stair).
    build.kits = None;
    build.replace_with(&stamp.ops, &mut commands, &planet, &mut meshes);
    build.kits = look.kits.clone();
    build.redraw_all();
    commands.spawn((Name::new("Dungeon, as built"), Built(build)));

    let mut root = commands.spawn((Name::new("Dungeon contents"), *frame, Visibility::default()));
    root.with_children(|inside| {
        for prop in &stamp.props {
            inside.spawn((
                Name::new(prop.model.clone()),
                scene(&assets, &prop.model),
                Transform::from_translation(prop.at)
                    .with_rotation(facing(prop.facing))
                    .with_scale(Vec3::splat(prop.scale)),
            ));
        }
        for light in &stamp.lights {
            match light.kind {
                LightKind::Torch => {
                    let out = dir(light.facing);
                    let lean = TORCH_LEAN_DEG.to_radians();
                    let up = Vec3::Y * libm::cosf(lean) + out * libm::sinf(lean);
                    let tilt = Quat::from_rotation_arc(Vec3::Y, up);
                    inside.spawn((
                        Name::new("Wall torch"),
                        scene(&assets, TORCH),
                        Transform::from_translation(light.at - up * TORCH_HEAD_M)
                            .with_rotation(tilt * facing(light.facing)),
                    ));
                    inside.spawn((
                        Name::new("Wall torch's fire"),
                        TORCH_FIRE,
                        Transform::from_translation(light.at),
                    ));
                }
                LightKind::Brazier => {
                    inside.spawn((
                        Name::new("Brazier's fire"),
                        terra_survival::props::CAMPFIRE,
                        Transform::from_translation(light.at - Vec3::Y * BRAZIER_DROP_M),
                    ));
                }
                LightKind::Bowl => {
                    // The fire's logs are laid by its scale, as the model's are.
                    let placed = Transform::from_translation(light.at)
                        .with_rotation(facing(light.facing))
                        .with_scale(Vec3::splat(BOWL_SCALE));
                    inside.spawn((Name::new("Fire bowl"), scene(&assets, CAMPFIRE), placed));
                    inside.spawn((Name::new("Fire bowl's fire"), BOWL_FIRE, placed));
                }
                LightKind::Campfire => {
                    inside.spawn((
                        Name::new("Campfire"),
                        scene(&assets, CAMPFIRE),
                        Transform::from_translation(light.at).with_rotation(facing(light.facing)),
                    ));
                    inside.spawn((
                        Name::new("Campfire's fire"),
                        terra_survival::props::CAMPFIRE,
                        Transform::from_translation(light.at),
                    ));
                }
            }
        }
        for chest in &stamp.chests {
            inside.spawn((
                Name::new("Dungeon chest"),
                scene(&assets, CHEST),
                Transform::from_translation(chest.at).with_rotation(facing(chest.facing)),
            ));
        }
        for object in &stamp.objects {
            let turn = facing(object.facing);
            let live = || DungeonObject {
                role: object.role,
                name: object.name.clone(),
                locked: object.locked,
            };
            if object.role != ObjectRole::Door {
                inside.spawn((
                    Name::new(object.name.clone()),
                    live(),
                    scene(&assets, &object.model),
                    Transform::from_translation(object.at).with_rotation(turn),
                ));
                continue;
            }
            for (at, turn) in open(object.at, turn) {
                inside.spawn((
                    Name::new(object.name.clone()),
                    live(),
                    scene(&assets, &object.model),
                    Transform::from_translation(at)
                        .with_rotation(turn)
                        .with_scale(leaf_scale()),
                ));
            }
        }
    });
    let mobs = stamp
        .mobs
        .iter()
        .map(|mob| (mob.species.clone(), frame.transform_point(mob.at)))
        .collect();
    commands.insert_resource(Mobs(mobs));
    commands.remove_resource::<Pending>();
}

/// Forms the dungeon's creatures where they stand, once their kinds are ready.
pub(super) fn form_mobs(
    mut commands: Commands,
    mobs: Option<Res<Mobs>>,
    kinds: Option<Res<Kinds>>,
    bestiary: Option<Res<Bestiary>>,
    mut form: MessageWriter<Form>,
) {
    let (Some(mobs), Some(kinds), Some(bestiary)) = (mobs, kinds, bestiary) else {
        return;
    };
    if !kinds.ready() {
        return;
    }
    for (species, at) in &mobs.0 {
        match bestiary.named(species) {
            Some(kind) => {
                form.write(Form(kind, *at));
            }
            None => warn!("Dungeon: no creature {species}"),
        }
    }
    info!("Dungeon: {} creatures formed", mobs.0.len());
    commands.remove_resource::<Mobs>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two leaves close the doorway jamb to jamb and sill to lintel, meeting in the
    /// middle, for every way a doorway faces.
    #[test]
    fn a_doorways_two_leaves_fill_it_jamb_to_jamb_and_meet_in_the_middle() {
        let scale = leaf_scale();
        for side in 0..4 {
            let turn = facing(side);
            let sill = Vec3::new(3.0, -2.0, 7.0);
            let across = turn * Vec3::X;
            let mut spans = Vec::new();
            for (hinge, leaf) in leaves(sill, turn) {
                // The model's corners: hinge foot and the far top corner.
                let far = hinge + leaf * (Vec3::new(LEAF_M.x, LEAF_M.y, 0.0) * scale);
                let (a, b) = ((hinge - sill).dot(across), (far - sill).dot(across));
                spans.push((a.min(b), a.max(b)));
                assert!(
                    (far.y - hinge.y - DOORWAY_M[1]).abs() < 1e-4,
                    "{side}: height"
                );
                assert!((hinge.y - sill.y).abs() < 1e-5, "{side}: on the sill");
            }
            spans.sort_by(|a, b| a.0.total_cmp(&b.0));
            let half = DOORWAY_M[0] * 0.5;
            assert!((spans[0].0 + half).abs() < 1e-4, "{side}: {spans:?}");
            assert!(
                spans[0].1.abs() < 1e-4 && spans[1].0.abs() < 1e-4,
                "{side}: {spans:?}"
            );
            assert!((spans[1].1 - half).abs() < 1e-4, "{side}: {spans:?}");
        }
    }

    /// Open, each leaf stands at its jamb reaching straight into the room (away from the
    /// way the doorway faces), its full width, so the doorway between the jambs is clear.
    #[test]
    fn open_leaves_stand_at_their_jambs_reaching_into_the_room() {
        let scale = leaf_scale();
        let width = DOORWAY_M[0] * 0.5;
        for side in 0..4 {
            let turn = facing(side);
            let sill = Vec3::new(3.0, -2.0, 7.0);
            let (across, out) = (turn * Vec3::X, turn * Vec3::Z);
            let mut jambs = Vec::new();
            for (hinge, leaf) in open(sill, turn) {
                let far = hinge + leaf * (Vec3::X * LEAF_M.x * scale.x);
                let along = (far - hinge).dot(across);
                assert!(along.abs() < 1e-4, "{side}: along the jamb, {along}");
                let into = -(far - hinge).dot(out);
                assert!((into - width).abs() < 1e-4, "{side}: into the room, {into}");
                jambs.push((hinge - sill).dot(across));
            }
            jambs.sort_by(f32::total_cmp);
            assert!((jambs[0] + width).abs() < 1e-4 && (jambs[1] - width).abs() < 1e-4);
        }
    }
}
