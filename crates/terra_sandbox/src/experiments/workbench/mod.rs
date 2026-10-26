//! The model workbench (D71): models on a plain floor in the game's own light, playing
//! their animations, holding and wearing things as they would in play, so a model can be
//! checked in the real renderer by a person or by an agent.
//!
//! `--model <path>` loads a .glb or .gltf under `assets/`; given more than once, the
//! models stand side by side doing the same thing. `--anim <clip>` plays a clip
//! (repeatable; the first plays when looking by hand); `--anim Walk_Loop+Keyed_Guard` plays a
//! clip with another's first pose laid over its arms, as the game lays a stance's hold
//! (`terra_walker::layer`). `--hold <item>` puts an item's held
//! look in the right hand, `--wear <item>` puts on an item's look for each model's body, and
//! `--attach <file>[@<socket>]` puts on any look, on a socket if given, otherwise as the file
//! says (`character`). `--focus <name>` looks closely at one
//! part. `--sheet <prefix>` saves a contact sheet per clip and exits (`sheet`). With
//! `--remote`, `workbench.pose {clip, fraction}` holds every model at a moment of a clip, and
//! `workbench.shape {model, template | morphs, skin}` reshapes a model's body live
//! (`appearance`): a template by name, or any mix of morphs, such as `{"heavy": 0.5}`.
//! Clips, joints and sockets are printed when a model loads.
//!
//! `--prop <name>` stands an item of the builder's catalogue beside the models, drawn as it
//! is when placed in play (`play::prop_looks`): its model at the catalogue's size, in our
//! colours, glowing where it glows. Its size is printed, so a prop's proportions can be
//! checked against a body and by number without playing. `--prop all` stands every item.

mod sheet;

use bevy::{
    asset::RecursiveDependencyLoadState,
    camera::primitives::Aabb,
    gltf::{Gltf, GltfAssetLabel},
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
    world_serialization::{WorldAssetRoot, WorldInstanceReady},
};
use serde::Deserialize;
use serde_json::{Value, json};
use terra_world::planet::MainCamera;

use terra_game::appearance::{Appearance, ReportJoints};
use terra_game::{character, planet::Planet, remote, shell::Options};

/// How far apart models stand, side by side.
const SPACING_M: f32 = 1.2;
/// The gap between props in the row, and the height a body is taken to have until its
/// skin is measured (`measure`).
const PROP_GAP_M: f32 = 0.6;
const BODY_M: f32 = 1.8;
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
    /// How far along the row each model stands from its middle; the row's length, and
    /// the height of the tallest thing in it.
    slots: Vec<f32>,
    length: f32,
    tallest: f32,
    /// The tallest prop, and the first model's height once its skin is measured: a body
    /// is framed by its own size, so a 7 m creature is seen whole and looked at closely
    /// from as far as its size asks (`measure`).
    props_high: f32,
    figure: f32,
    measured: bool,
    /// It has props, so the row is wider than a figure and its sheet keeps the whole view.
    wide: bool,
    spawned: bool,
    models: Vec<Model>,
}

/// One model on the bench: where it stands, its animation player once it has one, and
/// its clips as places in its own animation graph, by name, with their lengths.
struct Model {
    root: Entity,
    /// Its scene has been spawned; a model without animations never gets a player.
    loaded: bool,
    player: Option<Entity>,
    clips: Vec<Clip>,
}

/// A clip as a model's graph has it: its name, its node, its length, and the node of the hold
/// laid over its arms if its name gives one (`Walk_Loop+Keyed_Guard`).
type Clip = (String, AnimationNodeIndex, f32, Option<AnimationNodeIndex>);

impl Bench {
    /// Every model's player and its node and length for `clip`.
    fn playing<'a>(
        &'a self,
        clip: &'a str,
    ) -> impl Iterator<Item = (Entity, AnimationNodeIndex, f32, Option<AnimationNodeIndex>)> + 'a
    {
        self.models.iter().filter_map(move |model| {
            let (_, node, length, hold) = model.clips.iter().find(|c| c.0 == clip)?;
            Some((model.player?, *node, *length, *hold))
        })
    }

    fn ready(&self) -> bool {
        self.spawned && self.models.iter().all(|model| model.loaded)
    }
}

/// What the model contract says of a prop on the bench (docs/art-style.md), drawn over it
/// so it reads at a glance: its box, green if it keeps the contract and red if not; its
/// origin, the anchor, as three short lines, X red, Y green and Z (its front) blue; and
/// each socket as a yellow star, its name in the log.
#[derive(Component)]
struct Contract {
    low: Vec3,
    high: Vec3,
    sockets: Vec<(String, Vec3)>,
    keeps: bool,
}

fn draw_contracts(mut gizmos: Gizmos, props: Query<(&Contract, &GlobalTransform)>) {
    for (contract, at) in &props {
        let at = at.compute_transform();
        let middle = (contract.low + contract.high) * 0.5;
        let colour = if contract.keeps {
            Color::srgb(0.3, 0.9, 0.4)
        } else {
            Color::srgb(1.0, 0.25, 0.2)
        };
        gizmos.cube(
            Transform {
                translation: at.transform_point(middle),
                rotation: at.rotation,
                scale: contract.high - contract.low,
            },
            colour,
        );
        let origin = at.translation;
        for (axis, colour) in [
            (Vec3::X, Color::srgb(1.0, 0.2, 0.2)),
            (Vec3::Y, Color::srgb(0.2, 1.0, 0.2)),
            (Vec3::Z, Color::srgb(0.3, 0.5, 1.0)),
        ] {
            gizmos.line(origin, origin + at.rotation * axis * 0.3, colour);
        }
        for (_, socket) in &contract.sockets {
            let point = at.transform_point(*socket);
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                let arm = at.rotation * axis * 0.06;
                gizmos.line(point - arm, point + arm, Color::srgb(1.0, 0.85, 0.1));
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (spawn, measure, sheet::capture).chain())
        .add_systems(Update, draw_contracts);
    remote::method(app, "workbench.pose", pose_method);
    remote::method(app, "workbench.shape", shape_method);
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
    if options.models.is_empty() && options.props.is_empty() {
        error!("the workbench needs --model <path under assets/> or --prop <item>");
        return;
    }
    let stand = planet.start();
    let up = stand.normalize();
    // Models face +Z, as glTF models do; at the start, up is close to +Y.
    let front = Vec3::X.cross(up).normalize();
    let floor = stand + up * 0.525;
    let turned = Quat::from_rotation_arc(Vec3::Y, up);
    let side = up.cross(front);
    // The row: each model in a slot of its own, then each prop in one as wide as it is.
    let lit = materials.add(terra_game::play::prop_lit());
    let glowing = materials.add(terra_game::play::prop_glowing());
    let mut making = terra_game::play::PropMaking {
        meshes: &mut meshes,
        materials: &mut materials,
        assets: &assets,
        lit,
        glowing,
    };
    let looks: Vec<_> = terra_game::play::prop_looks(&options.props, &mut making)
        .into_iter()
        .filter_map(|look| look.map_err(|problem| error!("workbench: {problem}")).ok())
        .collect();
    let widths: Vec<f32> = std::iter::repeat_n(SPACING_M, options.models.len())
        .chain(
            looks
                .iter()
                .map(|look| look.model.half.x * 2.0 + PROP_GAP_M),
        )
        .collect();
    let length: f32 = widths.iter().sum();
    let mut slots = Vec::new();
    let mut along = -length * 0.5;
    for width in &widths {
        slots.push(along + width * 0.5);
        along += width;
    }
    let mut tallest = if options.models.is_empty() {
        0.0
    } else {
        BODY_M
    };
    let mut tallest_prop: f32 = 0.0;
    for (look, offset) in looks.iter().zip(&slots[options.models.len()..]) {
        let size = look.model.half * 2.0;
        let wrong = terra_game::play::contract::check(&look.entry);
        info!(
            "workbench: prop {}: {:.2} m wide, {:.2} m high, {:.2} m deep; {}",
            look.entry.name,
            size.x,
            size.y,
            size.z,
            if wrong.is_empty() {
                "keeps the model contract".to_string()
            } else {
                format!("breaks the model contract: {}", wrong.join("; "))
            }
        );
        for (name, at) in &look.model.sockets {
            info!(
                "workbench: prop {}: socket {name} at {:.2}, {:.2}, {:.2}",
                look.entry.name, at.x, at.y, at.z
            );
        }
        tallest = f32::max(tallest, size.y);
        tallest_prop = f32::max(tallest_prop, size.y);
        let mut prop = commands.spawn((
            Name::new(format!("Workbench prop {}", look.entry.name)),
            Transform::from_translation(floor + side * *offset).with_rotation(turned),
            Visibility::default(),
            Contract {
                low: look.model.low,
                high: look.model.high,
                sockets: look.model.sockets.clone(),
                keeps: wrong.is_empty(),
            },
        ));
        for piece in &look.model.pieces {
            prop.with_child((
                Mesh3d(piece.mesh.clone()),
                MeshMaterial3d(piece.material.clone()),
            ));
        }
    }
    // A plain floor, so the models are judged on their own.
    commands.spawn((
        Name::new("Workbench floor"),
        Mesh3d(meshes.add(Cylinder::new(f32::max(6.0, length * 0.5 + 1.0), 0.05))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.6, 0.56),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_translation(stand + up * 0.5).with_rotation(turned),
    ));
    if options.joints {
        commands.insert_resource(ReportJoints);
    }
    commands.insert_resource(Bench {
        files: options
            .models
            .iter()
            .map(|path| assets.load(path.clone()))
            .collect(),
        clips: options.anims.clone(),
        stand: floor,
        up,
        front,
        slots,
        length,
        tallest,
        props_high: tallest_prop,
        figure: BODY_M,
        measured: false,
        wide: !looks.is_empty(),
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
    gltf_nodes: Res<Assets<bevy::gltf::GltfNode>>,
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
        let first = bench.files.first().and_then(|file| gltfs.get(file));
        bench.clips = ["Idle_Loop", "Walk_Loop"]
            .into_iter()
            .filter(|name| first.is_some_and(|gltf| gltf.named_animations.contains_key(*name)))
            .map(String::from)
            .collect();
    }
    // A model without animations is still looked at, as it stands.
    if bench.clips.is_empty() {
        bench.clips.push("still".to_string());
    }
    let side = bench.up.cross(bench.front);
    let mut models = Vec::new();
    for (index, (file, path)) in bench.files.iter().zip(&options.models).enumerate() {
        let Some(gltf) = gltfs.get(file) else {
            continue;
        };
        let mut names: Vec<&str> = gltf.named_animations.keys().map(AsRef::as_ref).collect();
        names.sort();
        info!("workbench: {path}: {} clips: {:?}", names.len(), names);
        let (graph, nodes) = placed_clips(gltf, &gltf_nodes, &clips, &bench.clips, path);
        let graph = graphs.add(graph);
        let Some(scene) = gltf.default_scene.clone() else {
            continue;
        };
        let offset = bench.slots[index];
        // Shaped as its template, if given one (`appearance`), as tall as `@height` says.
        let shape = options
            .templates
            .get(index)
            .and_then(|given| {
                let (name, height) = match given.split_once('@') {
                    Some((name, height)) => (name, height.parse::<f32>().ok()),
                    None => (given.as_str(), None),
                };
                let found = Appearance::template(name).map(|mut shape| {
                    shape.height = height.unwrap_or(shape.height);
                    shape
                });
                if found.is_none() {
                    warn!("workbench: there is no template {name}");
                }
                found
            })
            .unwrap_or_default();
        let root = commands
            .spawn((
                Name::new(format!("Workbench model {index}")),
                character::Body,
                shape,
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
                        "workbench: model {index}: {} named nodes, sockets {:?}",
                        named("").len(),
                        named("socket.")
                    );
                    dress(
                        &mut commands,
                        ready.entity,
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
                        model.loaded = true;
                        return;
                    };
                    commands
                        .entity(entity)
                        .insert(AnimationGraphHandle(graph.clone()));
                    if let (Ok(mut player), Some((_, node, _, hold))) =
                        (players.get_mut(entity), model.clips.first())
                    {
                        player.play(*node).repeat();
                        if let Some(hold) = hold {
                            player.play(*hold).pause();
                        }
                    }
                    model.player = Some(entity);
                    model.loaded = true;
                },
            )
            .id();
        models.push(Model {
            root,
            loaded: false,
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
        let file = character::look(item);
        character::wear(
            commands,
            assets.load(GltfAssetLabel::Scene(0).from_asset(file)),
            root,
        );
    }
}

/// Once every model's skin has bounds: how tall each stands, so the views frame bodies by
/// their size rather than a person's.
fn measure(
    bench: Option<ResMut<Bench>>,
    children: Query<&Children>,
    boxes: Query<(&Aabb, &GlobalTransform)>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let Some(mut bench) = bench else { return };
    if bench.measured || !bench.ready() {
        return;
    }
    let mut heights = Vec::new();
    for model in &bench.models {
        let found = children
            .iter_descendants(model.root)
            .filter_map(|each| boxes.get(each).ok())
            .map(|(bounds, at)| (*bounds, *at));
        // Skin is bounded a frame or two after its scene spawns.
        let Some(height) = standing_height(found, bench.stand, bench.up) else {
            return;
        };
        heights.push(height);
    }
    bench.measured = true;
    for (index, height) in heights.iter().enumerate() {
        info!("workbench: model {index} stands {height:.2} m tall");
    }
    bench.figure = heights.first().copied().unwrap_or(BODY_M);
    bench.tallest = heights.iter().copied().fold(bench.props_high, f32::max);
    if let Ok(mut transform) = camera.single_mut() {
        *transform = whole_view(&bench, 30.0);
    }
}

/// How high above `stand`, along `up`, the highest corner of any of `boxes` reaches, each
/// box in its entity's space; `None` when there are none.
fn standing_height(
    boxes: impl Iterator<Item = (Aabb, GlobalTransform)>,
    stand: Vec3,
    up: Vec3,
) -> Option<f32> {
    let mut highest: Option<f32> = None;
    for (bounds, at) in boxes {
        let (middle, half) = (Vec3::from(bounds.center), Vec3::from(bounds.half_extents));
        for corner in 0..8 {
            let sign = Vec3::new(
                if corner & 1 == 0 { -1.0 } else { 1.0 },
                if corner & 2 == 0 { -1.0 } else { 1.0 },
                if corner & 4 == 0 { -1.0 } else { 1.0 },
            );
            let height = (at.transform_point(middle + half * sign) - stand).dot(up);
            highest = Some(highest.map_or(height, |was| f32::max(was, height)));
        }
    }
    highest
}

/// Where the camera stands to see every model whole from `degrees` round their front.
fn whole_view(bench: &Bench, degrees: f32) -> Transform {
    let target = bench.stand + bench.up * f32::max(0.9, bench.tallest * 0.5);
    // Far enough for the row's length across a wide frame and its height up a 45° one.
    let fits = f32::max(
        bench.length * 0.5 / libm::tanf(36f32.to_radians()),
        bench.tallest * 0.5 / libm::tanf(22.5f32.to_radians()),
    ) * 1.2;
    let distance = f32::max(
        FIGURE_VIEW_M + SPACING_M * bench.files.len().saturating_sub(1) as f32,
        fits,
    );
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
        Some(target) => {
            // A part of a body bigger than a person is looked at from as much further.
            let scale = f32::max(1.0, bench.figure / BODY_M);
            around(bench, target, degrees, FOCUS_VIEW_M * scale, 0.15 * scale)
        }
        None => whole_view(bench, degrees),
    }
}

/// A model's graph (`terra_walker::layer::graph`) and the bench's clips placed in it, a clip
/// named `A+B` as `A` under `B`'s hold.
fn placed_clips(
    gltf: &Gltf,
    gltf_nodes: &Assets<bevy::gltf::GltfNode>,
    clips: &Assets<AnimationClip>,
    names: &[String],
    path: &str,
) -> (AnimationGraph, Vec<Clip>) {
    let (graph, placed) = terra_walker::layer::graph(gltf, gltf_nodes);
    let mut nodes = Vec::new();
    for name in names {
        let (under, over) = match name.split_once('+') {
            Some((under, over)) => (under, Some(over)),
            None => (name.as_str(), None),
        };
        let (Some(handle), Some(at)) = (gltf.named_animations.get(under), placed.get(under)) else {
            warn!("workbench: {path} has no clip named {under}");
            continue;
        };
        let hold = over.and_then(|over| placed.get(over)).map(|at| at.over);
        if over.is_some() && hold.is_none() {
            warn!("workbench: {path} has no clip named {name}'s hold");
        }
        let length = clips.get(handle).map_or(1.0, AnimationClip::duration);
        nodes.push((name.clone(), at.of(0, hold.is_some()), length, hold));
    }
    (graph, nodes)
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
    for (entity, node, length, hold) in bench.playing(clip) {
        let Ok(mut player) = players.get_mut(entity) else {
            continue;
        };
        player.stop_all();
        // The hold at its first pose, over the arms (`terra_walker::layer`).
        if let Some(hold) = hold {
            player.play(hold).pause();
        }
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

#[derive(Deserialize)]
struct Reshape {
    /// Which model, by its order on the command line; every model if not given.
    model: Option<usize>,
    template: Option<String>,
    morphs: Option<std::collections::HashMap<String, f32>>,
    skin: Option<String>,
    height: Option<f32>,
}

fn shape_method(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Reshape {
        model,
        template,
        morphs,
        skin,
        height,
    } = parse_some(params)?;
    let mut shape = match &template {
        Some(name) => Appearance::template(name)
            .ok_or_else(|| remote::refused(format!("there is no template {name}")))?,
        None => Appearance::default(),
    };
    if let Some(morphs) = morphs {
        let mut morphs: Vec<(String, f32)> = morphs.into_iter().collect();
        morphs.sort_by(|a, b| a.0.cmp(&b.0));
        shape.morphs = morphs;
    }
    if skin.is_some() {
        shape.skin = skin;
    }
    if let Some(height) = height {
        shape.height = height;
    }
    let mut bodies = world.query::<(&Name, &mut Appearance)>();
    let mut changed = 0;
    for (name, mut appearance) in bodies.iter_mut(world) {
        let index = name
            .as_str()
            .strip_prefix("Workbench model ")
            .and_then(|i| i.parse::<usize>().ok());
        if index.is_none() || model.is_some_and(|m| Some(m) != index) {
            continue;
        }
        *appearance = shape.clone();
        changed += 1;
    }
    if changed == 0 {
        return Err(remote::refused("no workbench model matches"));
    }
    Ok(
        json!({ "shaped": changed, "morphs": shape.morphs, "skin": shape.skin, "height": shape.height }),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A body is as tall as the top of its highest box, wherever and however that box
    /// is placed, measured along the ground's up.
    #[test]
    fn a_body_stands_as_tall_as_its_highest_box() {
        let stand = Vec3::new(0.0, 100.0, 0.0);
        let up = Vec3::Y;
        // A 7 m trunk from the ground up, its box in its entity's space.
        let trunk = (
            Aabb::from_min_max(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 7.0, 1.0)),
            GlobalTransform::from_translation(stand),
        );
        // A crown half a metre across, scaled up four times, its middle 7.5 m up.
        let crown = (
            Aabb::from_min_max(Vec3::splat(-0.25), Vec3::splat(0.25)),
            GlobalTransform::from(
                Transform::from_translation(stand + up * 7.5).with_scale(Vec3::splat(4.0)),
            ),
        );
        let height = standing_height([trunk, crown].into_iter(), stand, up).unwrap();
        assert!((height - 8.5).abs() < 1e-4, "{height}");
        assert_eq!(standing_height(std::iter::empty(), stand, up), None);
    }
}
