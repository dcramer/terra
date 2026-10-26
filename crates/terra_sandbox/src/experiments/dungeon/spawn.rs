//! The dungeon put up: its cubes as a build, and what stands in it (props, fires, chests,
//! doors, the goal and keys), all under one entity at the dungeon's frame; its creatures once
//! their kinds are ready.

use super::{Pending, dir};
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI};
use terra_creatures::creatures::{Form, Kinds};
use terra_creatures::species::Bestiary;
use terra_dungeon::door::{Door, Lock};
use terra_dungeon::stamp::{DOORWAY_M, LightKind, ObjectRole};
use terra_game::build::{Build, BuildLook, Built};
use terra_walker::Player;
use terra_world::fire::{Kindled, Logs};
use terra_world::flora::FloraPiece;
use terra_world::flora_kinds::Flora;
use terra_world::planet::Planet;
use terra_world::shell::ScriptFrame;

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
    light: terra_world::fire::TORCH_LIGHT,
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
    light: terra_world::fire::HEARTH_LIGHT,
};
/// The studded leaf's model, hinged at its origin and reaching along +X: so wide and so high.
const LEAF_M: Vec2 = Vec2::new(0.894, 1.939);

/// A dungeon object the game makes live: the goal, a key, a door leaf.
#[derive(Component, Debug)]
#[allow(dead_code)]
pub struct DungeonObject {
    pub role: ObjectRole,
    pub name: String,
    pub lock: Lock,
}

/// A doorway's door, pushed open by D512's rule; its transform at the middle of the sill,
/// turned as the doorway faces (+Z out of the room its leaves hang in), its two leaves its
/// children.
#[derive(Component, Debug)]
pub struct Doorway(pub Door);

/// One of a doorway's two leaves: the first hinged at the jamb at -X, the second at +X.
#[derive(Component, Debug)]
pub struct Leaf {
    second: bool,
}

/// The demo holds every key: taking keys is not built yet (how a key is held is the
/// developers' open question, docs/dungeons.md), so its locked gate opens as any door does.
const KEYS_HELD: bool = true;

/// The dungeon's creatures, in the world, waiting for their kinds to be ready.
#[derive(Resource)]
pub(super) struct Mobs(Vec<(String, Vec3)>);

/// A turn about up so a model's front (+Z) faces a side.
fn facing(side: usize) -> Quat {
    Quat::from_rotation_y((2.0 - side as f32) * FRAC_PI_2)
}

/// A leaf's hinge and turn in its doorway's frame with the door `swing` open (D512: 0 shut,
/// 1 a quarter turn into the room, -1 out of it), a double door as a gate of its width is
/// hung: each hinged at one jamb and reaching to the middle when shut, the second turned half
/// round so it reaches back (a leaf's planks and studs are alike on both faces).
fn leaf(second: bool, swing: f32) -> (Vec3, Quat) {
    let half = DOORWAY_M[0] * 0.5;
    let quarter = swing * FRAC_PI_2;
    if second {
        (Vec3::X * half, Quat::from_rotation_y(PI - quarter))
    } else {
        (-Vec3::X * half, Quat::from_rotation_y(quarter))
    }
}

fn leaf_at(second: bool, swing: f32) -> Transform {
    let (hinge, turn) = leaf(second, swing);
    Transform::from_translation(hinge)
        .with_rotation(turn)
        .with_scale(leaf_scale())
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
    flora: Option<Res<Flora>>,
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
            let placed = Transform::from_translation(prop.at)
                .with_rotation(facing(prop.facing))
                .with_scale(Vec3::splat(prop.scale));
            let mut spawned = inside.spawn((Name::new(prop.model.clone()), placed));
            // A rock is drawn as where it grows, in its real stone (round 18: the models as
            // shipped are flat grey, white lumps under the torches).
            match flora.as_ref().and_then(|flora| flora.drawn_as(&prop.model)) {
                Some((kind, model)) => spawned.insert(FloraPiece {
                    kind,
                    model: model.to_string(),
                }),
                None => spawned.insert(scene(&assets, &prop.model)),
            };
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
                lock: object.lock,
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
            // Shut until a body walks into it (D512).
            inside
                .spawn((
                    Name::new(object.name.clone()),
                    live(),
                    Doorway(Door::shut(object.lock)),
                    Transform::from_translation(object.at).with_rotation(turn),
                    Visibility::default(),
                ))
                .with_children(|doorway| {
                    for second in [false, true] {
                        doorway.spawn((
                            Name::new(format!("{} leaf", object.name)),
                            Leaf { second },
                            scene(&assets, &object.model),
                            leaf_at(second, 0.0),
                        ));
                    }
                });
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

/// Pushes each doorway's leaves by D512's rule with every player and camera walking at it
/// (a film's camera walks the tour as a body would), swinging its leaves as it opens. Logs
/// the frame a door is pushed and the frame a body passes its sill with how far open it
/// stands then (frames counted as `--press` counts them), so a film's doors are checked
/// from its log.
pub(super) fn push_doors(
    time: Res<Time>,
    script: Option<Res<ScriptFrame>>,
    bodies: Query<(Entity, &GlobalTransform), Or<(With<Player>, With<Camera3d>)>>,
    mut was: Local<HashMap<Entity, Vec3>>,
    mut doorways: Query<(&mut Doorway, &Name, &GlobalTransform, &Children)>,
    mut leaves: Query<(&Leaf, &mut Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let moving: Vec<(Vec3, Vec3)> = bodies
        .iter()
        .map(|(body, at)| {
            let at = at.translation();
            (at, was.insert(body, at).unwrap_or(at))
        })
        .collect();
    for (mut doorway, name, frame, children) in &mut doorways {
        let (swing, opened) = (doorway.0.swing, doorway.0.opened());
        let from_world = frame.affine().inverse();
        let near: Vec<(Vec3, Vec3)> = moving
            .iter()
            .map(|&(at, before)| {
                let (at, before) = (
                    from_world.transform_point3(at),
                    from_world.transform_point3(before),
                );
                (at, (at - before) / dt)
            })
            .collect();
        doorway.0.step(&near, DOORWAY_M[0], KEYS_HELD, dt);
        let frame = script.as_ref().map_or(0, |f| f.0);
        if !opened && doorway.0.opened() {
            info!("Dungeon door {name} pushed at frame {frame}");
        }
        for &(at, velocity) in &near {
            let before = at - velocity * dt;
            let through =
                at.x.abs() <= DOORWAY_M[0] * 0.5 && (-1.0..=DOORWAY_M[1] + 1.0).contains(&at.y);
            if through && before.z * at.z < 0.0 {
                let open = doorway.0.swing.abs();
                info!("Dungeon door {name} passed at frame {frame}, {open:.2} open");
            }
        }
        if doorway.0.swing == swing {
            continue;
        }
        for &child in children {
            if let Ok((leaf, mut placed)) = leaves.get_mut(child) {
                *placed = leaf_at(leaf.second, doorway.0.swing);
            }
        }
    }
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

    /// Shut, the two leaves close the doorway jamb to jamb and sill to lintel, meeting in
    /// the middle.
    #[test]
    fn a_doorways_two_leaves_shut_fill_it_jamb_to_jamb_and_meet_in_the_middle() {
        let mut spans = Vec::new();
        for second in [false, true] {
            let placed = leaf_at(second, 0.0);
            // The model's corners: hinge foot and the far top corner.
            let far = placed.transform_point(Vec3::new(LEAF_M.x, LEAF_M.y, 0.0));
            let hinge = placed.translation;
            spans.push((hinge.x.min(far.x), hinge.x.max(far.x)));
            assert!((far.y - DOORWAY_M[1]).abs() < 1e-4, "height");
            assert!(
                hinge.y.abs() < 1e-5 && far.z.abs() < 1e-4,
                "in the sill's plane"
            );
        }
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let half = DOORWAY_M[0] * 0.5;
        assert!((spans[0].0 + half).abs() < 1e-4, "{spans:?}");
        assert!(
            spans[0].1.abs() < 1e-4 && spans[1].0.abs() < 1e-4,
            "{spans:?}"
        );
        assert!((spans[1].1 - half).abs() < 1e-4, "{spans:?}");
    }

    /// Swung open, each leaf stands at its jamb reaching straight into the room (-Z) or out
    /// of it (+Z), its full width, so the doorway between the jambs is clear.
    #[test]
    fn open_leaves_stand_at_their_jambs_reaching_into_or_out_of_the_room() {
        let width = DOORWAY_M[0] * 0.5;
        for swing in [1.0, -1.0] {
            let mut jambs = Vec::new();
            for second in [false, true] {
                let placed = leaf_at(second, swing);
                let reach = placed.transform_point(Vec3::X * LEAF_M.x) - placed.translation;
                assert!(reach.x.abs() < 1e-4, "{swing}: along the jamb, {reach}");
                assert!((reach.z + swing * width).abs() < 1e-4, "{swing}: {reach}");
                jambs.push(placed.translation.x);
            }
            jambs.sort_by(f32::total_cmp);
            assert!((jambs[0] + width).abs() < 1e-4 && (jambs[1] - width).abs() < 1e-4);
        }
    }
}
