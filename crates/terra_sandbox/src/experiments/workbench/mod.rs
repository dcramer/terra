//! The model workbench (D71): models on a plain floor in the game's own light, playing
//! their animations, holding and wearing things as they would in play, so a model can be
//! checked in the real renderer by a person or by an agent.
//!
//! `--model <path>` loads a .glb or .gltf under `assets/`; given more than once, the
//! models stand side by side doing the same thing. `--anim <clip>` plays a clip
//! (repeatable; the first plays when looking by hand). `--hold <item>` puts an item's held
//! look in the right hand, `--wear <item>` puts on an item's look for each model's body, and
//! `--attach <file>[@<socket>]` puts on any look, on a socket if given, otherwise as the file
//! says (`character`). `--focus <name>` looks closely at one
//! part. `--sheet <prefix>` saves a contact sheet per clip and exits (`sheet`). With
//! `--remote`, `workbench.pose {clip, fraction}` holds every model at a moment of a clip.
//! Clips, joints and sockets are printed when a model loads.

mod sheet;

use crate::planet::MainCamera;
use bevy::{
    asset::RecursiveDependencyLoadState,
    gltf::{Gltf, GltfAssetLabel},
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
    world_serialization::{WorldAssetRoot, WorldInstanceReady},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{character, planet::Planet, remote, shell::Options};

/// How far apart models stand, side by side.
const SPACING_M: f32 = 1.2;
/// How far the camera stands from a whole figure, and from a part looked at closely.
const FIGURE_VIEW_M: f32 = 3.6;
const FOCUS_VIEW_M: f32 = 0.9;

#[derive(Resource)]
struct Bench {
    files: Vec<Handle<Gltf>>,
    clips: Vec<String>,
    /// Where the models stand, and which way is up and front there.
    stand: Vec3,
    up: Vec3,
    front: Vec3,
    spawned: bool,
    models: Vec<Model>,
}

/// One model on the bench: where it stands, its animation player once it has one, and
/// its clips as places in its own animation graph, by name, with their lengths.
struct Model {
    root: Entity,
    player: Option<Entity>,
    clips: Vec<(String, AnimationNodeIndex, f32)>,
}

impl Bench {
    /// Every model's player and its node and length for `clip`.
    fn playing<'a>(
        &'a self,
        clip: &'a str,
    ) -> impl Iterator<Item = (Entity, AnimationNodeIndex, f32)> + 'a {
        self.models.iter().filter_map(move |model| {
            let (_, node, length) = model.clips.iter().find(|(name, _, _)| name == clip)?;
            Some((model.player?, *node, *length))
        })
    }

    fn ready(&self) -> bool {
        self.spawned && self.models.iter().all(|model| model.player.is_some())
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (spawn, sheet::capture).chain());
    remote::method(app, "workbench.pose", pose_method);
    sheet::plugin(app);
}

fn setup(
    mut commands: Commands,
    options: Res<Options>,
    planet: Res<Planet>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if options.models.is_empty() {
        error!("the workbench needs --model <path under assets/>");
        return;
    }
    let stand = planet.start();
    let up = stand.normalize();
    // Models face +Z, as glTF models do; at the start, up is close to +Y.
    let front = Vec3::X.cross(up).normalize();
    // A plain floor, so the models are judged on their own.
    commands.spawn((
        Name::new("Workbench floor"),
        Mesh3d(meshes.add(Cylinder::new(6.0, 0.05))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.6, 0.56),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_translation(stand + up * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, up)),
    ));
    commands.insert_resource(Bench {
        files: options
            .models
            .iter()
            .map(|path| assets.load(path.clone()))
            .collect(),
        clips: options.anims.clone(),
        stand: stand + up * 0.525,
        up,
        front,
        spawned: false,
        models: Vec::new(),
    });
}

/// Once every model has loaded: report what is in each, build its clips into a graph,
/// stand them on the floor and point the camera at them.
fn spawn(
    mut commands: Commands,
    bench: Option<ResMut<Bench>>,
    options: Res<Options>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut bench) = bench else { return };
    if bench.spawned {
        return;
    }
    for (file, path) in bench.files.iter().zip(&options.models) {
        if let RecursiveDependencyLoadState::Failed(error) =
            assets.recursive_dependency_load_state(file)
        {
            // Say why and stop, rather than wait for a model that will never come.
            error!("workbench: {path} did not load: {error}");
            bench.spawned = true;
            if options.sheet.is_some() {
                exit.write(AppExit::error());
            }
            return;
        }
    }
    if !bench
        .files
        .iter()
        .all(|file| assets.is_loaded_with_dependencies(file))
    {
        return;
    }
    bench.spawned = true;
    if bench.clips.is_empty() {
        let first = gltfs.get(&bench.files[0]);
        bench.clips = ["Idle_Loop", "Walk_Loop"]
            .into_iter()
            .filter(|name| first.is_some_and(|gltf| gltf.named_animations.contains_key(*name)))
            .map(String::from)
            .collect();
    }
    let side = bench.up.cross(bench.front);
    let count = bench.files.len();
    let mut models = Vec::new();
    for (index, (file, path)) in bench.files.iter().zip(&options.models).enumerate() {
        let Some(gltf) = gltfs.get(file) else {
            continue;
        };
        let mut names: Vec<&str> = gltf.named_animations.keys().map(AsRef::as_ref).collect();
        names.sort();
        info!("workbench: {path}: {} clips: {:?}", names.len(), names);
        let mut graph = AnimationGraph::new();
        let mut nodes = Vec::new();
        for name in &bench.clips {
            let Some(handle) = gltf.named_animations.get(name.as_str()) else {
                warn!("workbench: {path} has no clip named {name}");
                continue;
            };
            let length = clips.get(handle).map_or(1.0, AnimationClip::duration);
            nodes.push((
                name.clone(),
                graph.add_clip(handle.clone(), 1.0, graph.root),
                length,
            ));
        }
        let graph = graphs.add(graph);
        let Some(scene) = gltf.default_scene.clone() else {
            continue;
        };
        let offset = (index as f32 - (count - 1) as f32 * 0.5) * SPACING_M;
        // The body's name, which its gear is fitted under (`character::look`).
        let body = std::path::Path::new(path)
            .file_stem()
            .map_or(String::new(), |stem| stem.to_string_lossy().into_owned());
        let root = commands
            .spawn((
                Name::new(format!("Workbench model {index}")),
                WorldAssetRoot(scene),
                Transform::from_translation(bench.stand + side * offset)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Y, bench.up)),
            ))
            .observe(
                move |ready: On<WorldInstanceReady>,
                      mut commands: Commands,
                      children: Query<&Children>,
                      names: Query<&Name>,
                      mut players: Query<&mut AnimationPlayer>,
                      mut bench: ResMut<Bench>,
                      options: Res<Options>,
                      assets: Res<AssetServer>| {
                    let descendants: Vec<Entity> =
                        children.iter_descendants(ready.entity).collect();
                    let named = |prefix: &str| {
                        descendants
                            .iter()
                            .filter_map(|each| names.get(*each).ok())
                            .filter(|name| name.starts_with(prefix))
                            .map(|name| name.as_str().to_string())
                            .collect::<Vec<_>>()
                    };
                    info!(
                        "workbench: model {index}: {} DEF- joints, sockets {:?}",
                        named("DEF-").len(),
                        named("socket.")
                    );
                    dress(
                        &mut commands,
                        ready.entity,
                        &body,
                        &options,
                        &assets,
                        &children,
                        &names,
                    );
                    let Some(model) = bench
                        .models
                        .iter_mut()
                        .find(|model| model.root == ready.entity)
                    else {
                        return;
                    };
                    let Some(entity) = descendants
                        .iter()
                        .copied()
                        .find(|each| players.contains(*each))
                    else {
                        info!("workbench: model {index} has no animation player");
                        return;
                    };
                    commands
                        .entity(entity)
                        .insert(AnimationGraphHandle(graph.clone()));
                    if let (Ok(mut player), Some((_, node, _))) =
                        (players.get_mut(entity), model.clips.first())
                    {
                        player.play(*node).repeat();
                    }
                    model.player = Some(entity);
                },
            )
            .id();
        models.push(Model {
            root,
            player: None,
            clips: nodes,
        });
    }
    bench.models = models;
    if let Ok(mut transform) = camera.single_mut() {
        *transform = whole_view(&bench, 30.0);
    }
}

/// Puts what `--hold`, `--attach` and `--wear` ask for on a model once it has loaded.
fn dress(
    commands: &mut Commands,
    root: Entity,
    body: &str,
    options: &Options,
    assets: &AssetServer,
    children: &Query<&Children>,
    names: &Query<&Name>,
) {
    if let Some(item) = &options.hold {
        match character::find(root, character::HAND_R, children, names) {
            Some(hand) => {
                commands.entity(hand).insert(character::Carrying::default());
                let file = character::held_look(item);
                character::hold(
                    commands,
                    assets.load(GltfAssetLabel::Scene(0).from_asset(file)),
                    hand,
                );
            }
            None => warn!("workbench: the model has no {}", character::HAND_R),
        }
    }
    for attach in &options.attaches {
        let (file, socket) = match attach.split_once('@') {
            Some((file, socket)) => (file, Some(socket)),
            None => (attach.as_str(), None),
        };
        let model = assets.load(GltfAssetLabel::Scene(0).from_asset(file.to_string()));
        match socket {
            Some(socket) => match character::find(root, socket, children, names) {
                Some(at) => {
                    character::hang(commands, model, at);
                }
                None => warn!("workbench: the model has no {socket}"),
            },
            None => {
                character::wear(commands, model, root);
            }
        }
    }
    for item in &options.wears {
        let file = character::look(body, item);
        character::wear(
            commands,
            assets.load(GltfAssetLabel::Scene(0).from_asset(file)),
            root,
        );
    }
}

/// Where the camera stands to see every model whole from `degrees` round their front.
fn whole_view(bench: &Bench, degrees: f32) -> Transform {
    let target = bench.stand + bench.up * 0.9;
    let distance = FIGURE_VIEW_M + SPACING_M * (bench.files.len() - 1) as f32;
    around(bench, target, degrees, distance, 0.7)
}

/// The camera `distance` from `target`, `degrees` round from the models' front.
fn around(bench: &Bench, target: Vec3, degrees: f32, distance: f32, height: f32) -> Transform {
    let side = bench.up.cross(bench.front);
    let turn = degrees.to_radians();
    let out = (bench.front * libm::cosf(turn) + side * libm::sinf(turn)).normalize();
    Transform::from_translation(target + out * distance + bench.up * height)
        .looking_at(target, bench.up)
}

/// The camera for a view from `degrees`: on the `--focus` part of the first model if
/// there is one, otherwise on every model whole.
fn view(
    bench: &Bench,
    focus: Option<&str>,
    degrees: f32,
    children: &Query<&Children>,
    names: &Query<&Name>,
    globals: &Query<&GlobalTransform>,
) -> Transform {
    let part = focus.zip(bench.models.first()).and_then(|(name, model)| {
        let part = character::find(model.root, name, children, names)?;
        globals.get(part).ok().map(GlobalTransform::translation)
    });
    match part {
        Some(target) => around(bench, target, degrees, FOCUS_VIEW_M, 0.15),
        None => whole_view(bench, degrees),
    }
}

/// Holds every model at `fraction` of the way through `clip`, or plays it from the start,
/// with anything in hand carried or used as that clip has it.
fn pose(
    bench: &Bench,
    clip: &str,
    fraction: Option<f32>,
    players: &mut Query<&mut AnimationPlayer>,
    hands: &mut Query<&mut character::Carrying>,
) {
    for mut hand in hands.iter_mut() {
        hand.0 = character::CARRIED_IN.contains(&clip);
    }
    for (entity, node, length) in bench.playing(clip) {
        let Ok(mut player) = players.get_mut(entity) else {
            continue;
        };
        player.stop_all();
        match fraction {
            Some(fraction) => {
                player.play(node).seek_to(length * fraction).pause();
            }
            None => {
                player.play(node).repeat();
            }
        }
    }
}

#[derive(Deserialize)]
struct Pose {
    clip: String,
    fraction: Option<f32>,
}

fn pose_method(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Pose { clip, fraction } = parse_some(params)?;
    let mut state = bevy::ecs::system::SystemState::<(
        Res<Bench>,
        Query<&mut AnimationPlayer>,
        Query<&mut character::Carrying>,
    )>::new(world);
    let (bench, mut players, mut hands) = state.get_mut(world).map_err(remote::refused)?;
    if bench.playing(&clip).next().is_none() {
        return Err(remote::refused(format!(
            "no model plays {clip}; the clips are those given with --anim"
        )));
    }
    pose(&bench, &clip, fraction, &mut players, &mut hands);
    Ok(json!({ "clip": clip, "fraction": fraction }))
}
