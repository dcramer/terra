//! The dungeon put up: its cubes as a build, and what stands in it (props, fires, chests,
//! doors, the goal and keys), all under one entity at the dungeon's frame; its creatures once
//! their kinds are ready.

use super::{Pending, dir};
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use terra_creatures::creatures::{Form, Kinds};
use terra_creatures::species::Bestiary;
use terra_dungeon::door::{self, Door, Lock};
use terra_dungeon::stamp::{CUBE, DOORWAY_M, Light, LightKind, ObjectRole};
use terra_game::build::{Build, BuildLook, Built};
use terra_walker::Player;
use terra_walker::bodies::Solid;
use terra_world::cine::{Cine, Places, Subject};
use terra_world::fire::{Kindled, Logs};
use terra_world::flora::FloraPiece;
use terra_world::flora_kinds::Flora;
use terra_world::planet::Planet;
use terra_world::shell::ScriptFrame;
use terra_world::start::Start;

const TORCH: &str = "models/tools/Torch.glb";
const CAMPFIRE: &str = "models/props/Campfire.glb";
const CHEST: &str = "models/props/Chest.glb";
/// The torch's head over its foot, as `terra_survival::torchlight` holds it.
const TORCH_HEAD_M: f32 = 0.47;
/// A wall torch is drawn larger than a held one, charred (`props::Charred`): at a hand's size,
/// half a metre, its stave read as a thin pale grey spike down the crypt's walls (torch round
/// t4), where a sconce's torch stands out from the stone as a dark stave under its flame.
const WALL_TORCH_SCALE: f32 = 1.3;
/// A wall torch leans so far out from its wall, in degrees.
const TORCH_LEAN_DEG: f32 = 20.0;
/// A torch's flames: a hand's width, knee-high at most, less hot than a campfire. Its body
/// the marched torch field (D466), as a staged torch's on its head (`staged`'s `burns_up`):
/// the flat cards it kept were the look torch rounds t1-t8 left behind.
const TORCH_FIRE: Kindled = Kindled {
    tall: 0.45,
    wide: 0.12,
    heat: 0.6,
    logs: Logs::NONE,
    body: terra_world::fire::Body::Marched,
    light: terra_world::fire::TORCH_LIGHT,
    rig: terra_world::fire::Rig::Torch,
};
/// A brazier's fire burns in its bowl, so far under the light's point.
const BRAZIER_DROP_M: f32 = 0.5;
/// A fire bowl by the goal: the campfire's stones and logs at this size on a pillar's top.
const BOWL_SCALE: f32 = 0.45;
/// Its flames: the campfire's at the bowl's size, about a torch's light, burning oil in a
/// torch's whiter light: at 0.9 heat in wood's light the pair blew the goal room out to one
/// flat orange (barrow dress12, film and stills reviewers), so its heat, and its light, halved.
const BOWL_FIRE: Kindled = Kindled {
    tall: 0.7,
    wide: 0.32,
    heat: 0.45,
    logs: terra_world::fire::Logs::CAMPFIRE.below(0.08),
    body: terra_world::fire::Body::Marched,
    light: terra_world::fire::TORCH_LIGHT,
    rig: terra_world::fire::Rig::Hearth,
};
/// Where a fire bowl's light hangs off its flames (`fire::Hung`): out past its stand's front,
/// a stand half a metre square, and up, its key 0.45 m over the stand's top, so the stand's
/// face and the floor at its foot take its light; on its axis, 0.28 m up, they took none and
/// the stands read as black boxes under their fires (barrow round 16).
const BOWL_HUNG_OUT_M: f32 = 0.4;
const BOWL_HUNG_UP_M: f32 = 0.17;

/// The goal's key light, as a game lights its focal point with a light of its own (Skyrim's
/// and Dark Souls' unseen lights over a relic): a warm spot over the relic's front, in its
/// frame, on the relic and its plinth. Lit only by the bowls either side, the relic read as a
/// dark box on a dark plinth between two bright fires (barrow round 17, all three reviewers).
const GOAL_KEY_AT: Vec3 = Vec3::new(0.0, 2.6, 1.8);
const GOAL_KEY_ON: Vec3 = Vec3::new(0.0, 0.3, 0.0);
const GOAL_KEY_LUMENS: f32 = 200_000.0;
const GOAL_KEY_COLOUR: Color = Color::srgb(1.0, 0.74, 0.48);

fn goal_key() -> impl Bundle {
    (
        Name::new("Goal's key light"),
        SpotLight {
            color: GOAL_KEY_COLOUR,
            intensity: GOAL_KEY_LUMENS,
            range: 8.0,
            radius: 0.1,
            shadow_maps_enabled: true,
            inner_angle: 0.25,
            outer_angle: 0.45,
            ..default()
        },
        Transform::from_translation(GOAL_KEY_AT).looking_at(GOAL_KEY_ON, Vec3::Y),
    )
}

/// A hero's lamp (candles, a lantern by a coffin or a bench, `LightKind::Lamp`): a small warm
/// light at its flames, as games light a focal prop from a source in the fiction, so the hero
/// reads in a room whose torch is across it. About a third of a wall torch's light by night
/// and a shorter reach (at 30,000 lm and 4.5 m the robbed tomb's coffin still read as a dark
/// box beside its candles, barrow dress round 3); unshadowed, as the fires' shadows are
/// budgeted (`fire::light`'s `choose`) and a lamp at a hero's foot needs none to read.
const LAMP_LUMENS: f32 = 60_000.0;
const LAMP_RANGE_M: f32 = 5.5;
const LAMP_COLOUR: Color = Color::srgb(1.0, 0.62, 0.30);

fn lamp_light() -> PointLight {
    PointLight {
        color: LAMP_COLOUR,
        intensity: LAMP_LUMENS,
        range: LAMP_RANGE_M,
        radius: 0.08,
        shadow_maps_enabled: false,
        ..default()
    }
}

fn bowl_hung(side: usize) -> terra_world::fire::Hung {
    terra_world::fire::Hung(dir(side) * BOWL_HUNG_OUT_M + Vec3::Y * BOWL_HUNG_UP_M)
}
/// A dungeon object the game makes live: the goal, a key, a door leaf.
#[derive(Component, Debug)]
#[allow(dead_code)]
pub struct DungeonObject {
    pub role: ObjectRole,
    pub name: String,
    pub lock: Lock,
}

/// A doorway's door, pushed open by D512's rule, and how far each leaf opens into the room
/// (`Object::opens`); its transform at the middle of the sill, turned as the doorway faces
/// (+Z out of the room its leaves hang in), its two leaves its children.
#[derive(Component, Debug)]
pub struct Doorway(pub Door, pub [f32; 2]);

/// One of a doorway's two leaves: the first hinged at the jamb at -X, the second at +X; its
/// model so wide and so high (`Theme::door_leaf`), hinged at its origin, reaching along +X.
#[derive(Component, Debug)]
pub struct Leaf {
    second: bool,
    size: Vec2,
}

/// The demo holds every key: taking keys is not built yet (how a key is held is the
/// developers' open question, docs/dungeons.md), so its locked gate opens as any door does.
const KEYS_HELD: bool = true;

/// How often a camera's shot is read ahead for the doors it walks into, in seconds.
const FILM_EVERY_S: f32 = 0.05;

/// The dungeon's creatures, in the world, waiting for their kinds to be ready.
#[derive(Resource)]
pub(super) struct Mobs(Vec<(String, Vec3)>);

/// A turn about up so a model's front (+Z) faces a side.
fn facing(side: usize) -> Quat {
    Quat::from_rotation_y((2.0 - side as f32) * FRAC_PI_2)
}

/// A leaf's place in its doorway's frame with the door `swing` open (D512: 0 shut, 1 into
/// the room, -1 out of it) and its leaves opening `opens` quarter turns into the room, as
/// `door::leaf` hangs it at the room's face of the wall (a leaf's planks and studs are alike
/// on both faces, so the second is the first turned half round); its model `size`.
fn leaf_at(second: bool, swing: f32, opens: f32, size: Vec2) -> Transform {
    let (hinge, turn) = door::leaf(second, swing, opens, DOORWAY_M[0], CUBE);
    let turn = Quat::from_rotation_y(turn);
    Transform::from_translation(hinge - turn * Vec3::X * LAP_M)
        .with_rotation(turn)
        .with_scale(leaf_scale(size))
}

/// How far a shut leaf laps its jamb past the doorway's edge, in metres, its hinge stile
/// reaching that far behind the hinge, as a door hung on a wall's face laps its opening: the
/// jamb's timbers stop a few centimetres short of the opening's edge, and round 7's mine
/// stills showed a lit strip of the room beyond between each leaf and its jamb. Swung a
/// quarter turn the lap stands in the jamb; folded flat it stands over the doorway's edge.
const LAP_M: f32 = 0.08;

/// A leaf model of `size` scaled to half a doorway's width and its lap, and its full height:
/// the studded leaf stretched, one made at the doorway's size (the mine's) nearly as it is.
fn leaf_scale(size: Vec2) -> Vec3 {
    Vec3::new(
        (DOORWAY_M[0] * 0.5 + LAP_M) / size.x,
        DOORWAY_M[1] / size.y,
        1.0,
    )
}

fn scene(assets: &AssetServer, model: &str) -> WorldAssetRoot {
    WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(model.to_string())))
}

/// A stamp's light as it burns: a wall torch, a brazier, a fire bowl, a hero's lamp or the
/// rest's campfire.
fn light_up(inside: &mut ChildSpawnerCommands, assets: &AssetServer, light: &Light) {
    match light.kind {
        LightKind::Torch => {
            let out = dir(light.facing);
            let lean = TORCH_LEAN_DEG.to_radians();
            let up = Vec3::Y * libm::cosf(lean) + out * libm::sinf(lean);
            let tilt = Quat::from_rotation_arc(Vec3::Y, up);
            inside.spawn((
                Name::new("Wall torch"),
                scene(assets, TORCH),
                terra_survival::props::Charred,
                Transform::from_translation(light.at - up * TORCH_HEAD_M * WALL_TORCH_SCALE)
                    .with_rotation(tilt * facing(light.facing))
                    .with_scale(Vec3::splat(WALL_TORCH_SCALE)),
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
            inside.spawn((Name::new("Fire bowl"), scene(assets, CAMPFIRE), placed));
            let fire = (Name::new("Fire bowl's fire"), BOWL_FIRE);
            inside.spawn((fire, bowl_hung(light.facing), placed));
        }
        LightKind::Lamp => {
            inside.spawn((
                Name::new("Hero's lamp light"),
                lamp_light(),
                Transform::from_translation(light.at),
            ));
        }
        LightKind::Campfire => {
            inside.spawn((
                Name::new("Campfire"),
                scene(assets, CAMPFIRE),
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
    let Pending {
        stamp,
        frame,
        blocks,
        figure,
        leaf,
    } = &*pending;
    // Its pieces' solids (a mine's posts, a pier's shaft), each a walker's prop box where it
    // stands in the world: the walk reads a solid's own transform, so none is a child of the
    // dungeon's frame.
    for block in blocks {
        commands.spawn((
            Name::new("Dungeon piece's solid"),
            Solid { half: block.half },
            Transform::from_translation(frame.transform_point(block.foot))
                .with_rotation(frame.rotation * facing(block.facing)),
            Visibility::Visible,
        ));
    }
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
        // The theme's kit (D527), each piece stretched along its own axes, then turned.
        for piece in &stamp.pieces {
            let placed = Transform::from_translation(piece.at)
                .with_rotation(facing(piece.facing))
                .with_scale(piece.scale);
            inside.spawn((
                Name::new(piece.model.clone()),
                placed,
                scene(&assets, &piece.model),
            ));
        }
        for light in &stamp.lights {
            light_up(inside, &assets, light);
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
                let placed = Transform::from_translation(object.at).with_rotation(turn);
                let name = Name::new(object.name.clone());
                let goal = object.role == ObjectRole::Goal;
                if let (true, Some(figure)) = (goal, figure) {
                    let model = &object.model;
                    super::figure::spawn(inside, &assets, (figure, model), placed, (name, live()))
                        .with_child(goal_key());
                    continue;
                }
                let mut spawned =
                    inside.spawn((name, live(), scene(&assets, &object.model), placed));
                if goal {
                    spawned.with_child(goal_key());
                }
                continue;
            }
            // Shut until a body walks into it (D512).
            inside
                .spawn((
                    Name::new(object.name.clone()),
                    live(),
                    Doorway(Door::shut(object.lock), object.opens),
                    Transform::from_translation(object.at).with_rotation(turn),
                    Visibility::default(),
                ))
                .with_children(|doorway| {
                    for second in [false, true] {
                        doorway.spawn((
                            Name::new(format!("{} leaf", object.name)),
                            Leaf {
                                second,
                                size: *leaf,
                            },
                            scene(&assets, &object.model),
                            leaf_at(second, 0.0, object.opens[second as usize], *leaf),
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

/// Pushes each doorway's leaves by D512's rule with every player and camera walking at it,
/// swinging its leaves as it opens; a camera a shot holds pushes as the body walking the
/// shot, from the shot read ahead (`Door::film`). Logs the frame a door is pushed and the
/// frame a body passes its sill with how far open it stands then (frames counted as
/// `--press` counts them), so a film's doors are checked from its log.
pub(super) fn push_doors(
    (time, script): (Res<Time>, Option<Res<ScriptFrame>>),
    (cine, start, planet): (Option<Res<Cine>>, Option<Res<Start>>, Res<Planet>),
    subject: Query<&Transform, With<Subject>>,
    bodies: Query<(Entity, &GlobalTransform, Has<Camera3d>), Or<(With<Player>, With<Camera3d>)>>,
    mut was: Local<HashMap<Entity, Vec3>>,
    mut doorways: Query<(&mut Doorway, &Name, &GlobalTransform, &Children)>,
    mut leaves: Query<(&Leaf, &mut Transform), Without<Subject>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // The shot's eye from now as far as it is read ahead, when a shot holds the camera.
    let filmed: Option<Vec<Vec3>> = cine
        .as_ref()
        .filter(|cine| cine.shot.holds_camera())
        .zip(start.as_ref())
        .and_then(|(cine, start)| {
            let t = cine.t?;
            let ground = |at: Vec3| planet.ground_at(at);
            let places = Places {
                start: &start.0,
                ground: &ground,
                player: subject.iter().next().map(|t| t.translation),
            };
            let eye = |ahead: f32| cine.shot.framing(t + ahead, &places).eye;
            Some(door::film_path(eye, FILM_EVERY_S))
        });
    // Where each body is and was; a camera the shot holds walks it, so it pushes by the shot.
    let moving: Vec<(Vec3, Vec3, bool)> = bodies
        .iter()
        .map(|(body, at, camera)| {
            let at = at.translation();
            let by_shot = camera && filmed.is_some();
            (at, was.insert(body, at).unwrap_or(at), by_shot)
        })
        .collect();
    for (mut doorway, name, frame, children) in &mut doorways {
        let (swing, opened) = (doorway.0.swing, doorway.0.opened());
        let from_world = frame.affine().inverse();
        let near: Vec<(Vec3, Vec3, bool)> = moving
            .iter()
            .map(|&(at, before, by_shot)| {
                let (at, before) = (
                    from_world.transform_point3(at),
                    from_world.transform_point3(before),
                );
                (at, (at - before) / dt, by_shot)
            })
            .collect();
        if let Some(filmed) = &filmed {
            let path: Vec<Vec3> = filmed
                .iter()
                .map(|&eye| from_world.transform_point3(eye))
                .collect();
            doorway
                .0
                .film(&path, FILM_EVERY_S, DOORWAY_M[0], KEYS_HELD, 0.0);
        }
        let walking: Vec<(Vec3, Vec3)> = near
            .iter()
            .filter(|&&(.., by_shot)| !by_shot)
            .map(|&(at, velocity, _)| (at, velocity))
            .collect();
        doorway.0.step(&walking, DOORWAY_M[0], KEYS_HELD, dt);
        let frame = script.as_ref().map_or(0, |f| f.0);
        if !opened && doorway.0.opened() {
            info!("Dungeon door {name} pushed at frame {frame}");
        }
        for &(at, velocity, _) in &near {
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
                *placed = leaf_at(
                    leaf.second,
                    doorway.0.swing,
                    doorway.1[leaf.second as usize],
                    leaf.size,
                );
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

    /// The leaf models the themes hang: the studded (and the crypt's) and the mine's, made
    /// at the doorway's size.
    const SIZES: [Vec2; 2] = [Vec2::new(0.894, 1.939), Vec2::new(1.5, 3.0)];

    /// Shut, the two leaves close the doorway jamb to jamb and sill to lintel, meeting in
    /// the middle, whatever the leaf model's size.
    #[test]
    fn a_doorways_two_leaves_shut_fill_it_jamb_to_jamb_and_meet_in_the_middle() {
        for size in SIZES {
            leaves_shut_fill_the_doorway(size);
        }
    }

    fn leaves_shut_fill_the_doorway(size: Vec2) {
        let mut spans = Vec::new();
        for second in [false, true] {
            let placed = leaf_at(second, 0.0, 2.0, size);
            // The model's corners: hinge foot and the far top corner.
            let far = placed.transform_point(Vec3::new(size.x, size.y, 0.0));
            let hinge = placed.translation;
            spans.push((hinge.x.min(far.x), hinge.x.max(far.x)));
            assert!((far.y - DOORWAY_M[1]).abs() < 1e-4, "height");
            assert!(
                hinge.y.abs() < 1e-5 && (far.z + CUBE).abs() < 1e-4,
                "at the room's face of the wall"
            );
        }
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Each laps its jamb, past the doorway's edge.
        let half = DOORWAY_M[0] * 0.5 + LAP_M;
        assert!((spans[0].0 + half).abs() < 1e-4, "{spans:?}");
        assert!(
            spans[0].1.abs() < 1e-4 && spans[1].0.abs() < 1e-4,
            "{spans:?}"
        );
        assert!((spans[1].1 - half).abs() < 1e-4, "{spans:?}");
    }

    /// Swung open, each leaf stands at its jamb, its full width, so the doorway between the
    /// jambs is clear: out of the room (+Z) along the jamb; into it (-Z) along a side wall
    /// flush with the jamb, or folded back flat on the wall it hangs in, reaching away from
    /// the doorway.
    #[test]
    fn open_leaves_lie_along_their_jambs_or_flat_on_the_wall() {
        let width = DOORWAY_M[0] * 0.5;
        for (swing, opens, size) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, 2.0), (1.0, 2.0)]
            .into_iter()
            .flat_map(|(swing, opens)| SIZES.map(|size| (swing, opens, size)))
        {
            let mut jambs = Vec::new();
            for second in [false, true] {
                let placed = leaf_at(second, swing, opens, size);
                // The hinge, its lap behind it.
                let hinge = placed.transform_point(Vec3::X * LAP_M / placed.scale.x);
                let reach = placed.transform_point(Vec3::X * size.x) - hinge;
                let side = hinge.x.signum();
                let wanted = if swing > 0.0 && opens == 2.0 {
                    Vec3::X * side * width
                } else {
                    Vec3::Z * -swing * width
                };
                assert!(reach.distance(wanted) < 1e-4, "{swing} {opens}: {reach}");
                jambs.push(hinge.x);
            }
            jambs.sort_by(f32::total_cmp);
            assert!((jambs[0] + width).abs() < 1e-4 && (jambs[1] - width).abs() < 1e-4);
        }
    }
}
