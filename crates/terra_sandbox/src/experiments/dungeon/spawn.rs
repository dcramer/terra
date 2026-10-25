//! The dungeon put up: its cubes as a build, and what stands in it (props, fires, chests,
//! doors, the goal and keys), all under one entity at the dungeon's frame; its creatures once
//! their kinds are ready.

use super::{Pending, dir};
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use std::f32::consts::FRAC_PI_2;
use terra_creatures::creatures::{Form, Kinds};
use terra_creatures::species::Bestiary;
use terra_dungeon::stamp::{LightKind, ObjectRole};
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
/// A door leaf's hinge is at its side; the leaf is so wide.
const DOOR_WIDE_M: f32 = 0.89;

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
            // A door leaf hangs from its hinge, half its width along the wall from the middle.
            let at = match object.role {
                ObjectRole::Door => object.at - turn * Vec3::X * (DOOR_WIDE_M * 0.5),
                _ => object.at,
            };
            inside.spawn((
                Name::new(object.name.clone()),
                DungeonObject {
                    role: object.role,
                    name: object.name.clone(),
                    locked: object.locked,
                },
                scene(&assets, &object.model),
                Transform::from_translation(at).with_rotation(turn),
            ));
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
