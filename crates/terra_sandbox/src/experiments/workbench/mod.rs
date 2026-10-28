//! The model workbench (D71): models on a plain floor in the game's own light, playing
//! their animations, holding and wearing things as they would in play, so a model can be
//! checked in the real renderer by a person or by an agent.
//!
//! `--model <path>` loads a .glb or .gltf under `assets/`; given more than once, the
//! models stand side by side doing the same thing. `--anim <clip>` plays a clip
//! (repeatable; the first plays when looking by hand); `--anim Walk_Loop+Keyed_Guard` plays a
//! clip with another's first pose laid over its arms, as the game lays a stance's hold
//! (`terra_walker::layer`). A body on the figure's skeleton plays the figure's clips
//! (`figure::CLIPS`, the fight file) besides its own, as in the game, so a fight pose shows on
//! a template's shapes; a carried clip lays the carry pose of what a hand holds over that arm
//! (`CarryHold`, D560), as the figure does. `--hold <item>[@<socket>]` puts an item's held
//! look in the right hand or the hand socket given (repeatable, `heater_shield@socket.hand.L`
//! for the off hand), `--wear <item>` puts on an item's look for each model's body, and
//! `--attach <file>[@<socket>]` puts on any look, on a socket if given (held as play holds
//! it, on a hand), otherwise as the file says (`character`). `--focus <name>` looks closely
//! at one part and whole at what it holds, or at a whole model without it (`--focus whole`,
//! a part's model), of the first model. `--sheet <prefix>` saves a contact sheet per clip and exits (`sheet`). With
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
use terra_walker::figure::{self, Carry, Hand};
use terra_walker::layer::{self, Arms, Nodes};

/// How far apart models stand, side by side.
const SPACING_M: f32 = 1.2;
/// The gap between props in the row, and the height a body is taken to have until its
/// skin is measured (`measure`).
const PROP_GAP_M: f32 = 0.6;
const BODY_M: f32 = 1.8;
/// How far the camera stands from a whole figure, and from a part looked at closely.
const FIGURE_VIEW_M: f32 = 3.6;
const FOCUS_VIEW_M: f32 = 0.9;
/// The closest the camera comes to a small thing framed whole, such as a collar.
const CLOSEST_M: f32 = 0.25;
/// How far under the floor a model may reach before it is stood on it (`measure`): past a
/// body's toes in its bounds, short of a part made about a point inside it.
const SUNK_M: f32 = 0.02;

#[derive(Resource)]
struct Bench {
    files: Vec<Handle<Gltf>>,
    /// The figure's clips file (`figure::CLIPS`), whose clips a model on its skeleton plays
    /// as well as its own, as the game's figure does.
    figure_clips: Handle<Gltf>,
    clips: Vec<String>,
    /// The clip every model was last posed in, and where in it (`pose`).
    posed: Option<(String, Option<f32>)>,
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

/// One model on the bench: where it stands, its animation player once it has one, its
/// clips as places in its own animation graph, by name, with their lengths, every clip of
/// its graph, and the carry pose each arm takes in a carried clip (`recarry`).
struct Model {
    root: Entity,
    /// Its scene has been spawned; a model without animations never gets a player.
    loaded: bool,
    player: Option<Entity>,
    clips: Vec<Clip>,
    placed: bevy::platform::collections::HashMap<String, Nodes>,
    carry: Carry,
}

/// A clip as a model's graph has it: its name, its places, its length, and the node of the
/// hold laid over its arms if its name gives one (`Walk_Loop+Keyed_Guard`).
struct Clip {
    name: String,
    nodes: Nodes,
    length: f32,
    hold: Option<AnimationNodeIndex>,
}

impl Bench {
    /// Whether any model plays `clip`.
    fn plays(&self, clip: &str) -> bool {
        self.models
            .iter()
            .any(|model| model.player.is_some() && model.clips.iter().any(|c| c.name == clip))
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
    // Posed after the clips as in play (the fists, the arms, the carry, the shield arm).
    terra_walker::pose::plugin(app);
    app.add_systems(Startup, setup)
        .add_systems(Update, (spawn, measure, recarry, sheet::capture).chain())
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
        figure_clips: assets.load(figure::CLIPS),
        clips: options.anims.clone(),
        posed: None,
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
        .chain([&bench.figure_clips])
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
        let files = clip_files((gltf, file), &bench.figure_clips, &gltfs, &gltf_nodes, path);
        let (graph, placed, nodes) = placed_clips(&files, &gltf_nodes, &clips, &bench.clips, path);
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
                    let first = model.clips.first().map(|clip| clip.name.clone());
                    if let (Ok(mut player), Some(clip)) =
                        (players.get_mut(entity), model.clips.first())
                    {
                        lay(model, clip, None, &mut player);
                    }
                    model.player = Some(entity);
                    model.loaded = true;
                    bench.posed = bench.posed.take().or(first.map(|clip| (clip, None)));
                },
            )
            .id();
        models.push(Model {
            root,
            loaded: false,
            player: None,
            clips: nodes,
            placed,
            carry: [None; 2],
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
    for given in &options.holds {
        let (item, socket) = given
            .split_once('@')
            .unwrap_or((given.as_str(), character::HAND_R));
        match character::find(root, socket, children, names) {
            Some(hand) => {
                commands.entity(hand).insert(character::Carrying::default());
                let file = character::held_look(item);
                character::hold(
                    commands,
                    assets.load(GltfAssetLabel::Scene(0).from_asset(file)),
                    hand,
                );
            }
            None => warn!("workbench: the model has no {socket}"),
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
                // On a hand it is held as play holds a thing, its carry pose from its file.
                Some(hand) if socket == character::HAND_R || socket == character::HAND_L => {
                    commands.entity(hand).insert(character::Carrying::default());
                    character::hold(commands, model, hand);
                }
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
    mut roots: Query<&mut Transform, Without<MainCamera>>,
) {
    let Some(mut bench) = bench else { return };
    if bench.measured || !bench.ready() {
        return;
    }
    let mut spans = Vec::new();
    for model in &bench.models {
        let found = children
            .iter_descendants(model.root)
            .filter_map(|each| boxes.get(each).ok())
            .map(|(bounds, at)| (*bounds, *at));
        // Skin is bounded a frame or two after its scene spawns.
        let Some(span) = standing_span(found, bench.stand, bench.up) else {
            return;
        };
        spans.push((model.root, span));
    }
    bench.measured = true;
    // A model reaching under the floor (a part's origin at its eye, a haft's at its grip)
    // is stood on it, so none of it is hidden; a body's feet are on it already.
    let mut heights = Vec::new();
    for (root, (low, high)) in spans {
        if low < -SUNK_M
            && let Ok(mut at) = roots.get_mut(root)
        {
            at.translation -= bench.up * low;
        }
        heights.push(high - low.min(0.0));
    }
    for (index, height) in heights.iter().enumerate() {
        info!("workbench: model {index} stands {height:.2} m tall");
    }
    bench.figure = heights.first().copied().unwrap_or(BODY_M);
    bench.tallest = heights.iter().copied().fold(bench.props_high, f32::max);
    if let Ok(mut transform) = camera.single_mut() {
        *transform = whole_view(&bench, 30.0);
    }
}

/// How far above `stand`, along `up`, the lowest and the highest corner of any of `boxes`
/// reach, each box in its entity's space; `None` when there are none.
fn standing_span(
    boxes: impl Iterator<Item = (Aabb, GlobalTransform)>,
    stand: Vec3,
    up: Vec3,
) -> Option<(f32, f32)> {
    boxes
        .flat_map(|(bounds, at)| corners(&bounds, &at))
        .map(|corner| (corner - stand).dot(up))
        .fold(None, |was, h| match was {
            Some((low, high)) => Some((f32::min(low, h), f32::max(high, h))),
            None => Some((h, h)),
        })
}

/// The eight corners of `bounds`, a box in the space of `at`, in the world.
fn corners(bounds: &Aabb, at: &GlobalTransform) -> impl Iterator<Item = Vec3> + use<> {
    let (middle, half) = (Vec3::from(bounds.center), Vec3::from(bounds.half_extents));
    let at = *at;
    (0..8u32).map(move |corner| {
        let sign = Vec3::new(
            if corner & 1 == 0 { -1.0 } else { 1.0 },
            if corner & 2 == 0 { -1.0 } else { 1.0 },
            if corner & 4 == 0 { -1.0 } else { 1.0 },
        );
        at.transform_point(middle + half * sign)
    })
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

/// The camera for a view from `degrees`: with a `--focus`, close on that part of the first
/// model and whole what it holds, or on the model itself if it has no such part; otherwise
/// on every model whole.
fn view(
    bench: &Bench,
    focus: Option<&str>,
    degrees: f32,
    children: &Query<&Children>,
    names: &Query<&Name>,
    (globals, boxes): (&Query<&GlobalTransform>, &Query<(&Aabb, &GlobalTransform)>),
) -> Transform {
    // A model without the part (a part model, `--focus whole`) is itself looked at closely.
    let part = focus.zip(bench.models.first()).map(|(name, model)| {
        character::find(model.root, name, children, names).unwrap_or(model.root)
    });
    let Some((part, at)) = part.and_then(|part| Some((part, globals.get(part).ok()?))) else {
        return whole_view(bench, degrees);
    };
    // A part of a body bigger than a person is looked at from as much further.
    let scale = f32::max(1.0, bench.figure / BODY_M);
    // What it holds is seen end to end: a tool in a hand framed whole, not the hand alone.
    let held = children
        .iter_descendants(part)
        .filter_map(|each| boxes.get(each).ok())
        .map(|(bounds, at)| (*bounds, *at));
    let (target, distance) = match world_box(held) {
        Some((low, high)) => {
            // Its height up a 45° frame, its breadth across a sheet's cell, 3/4 as wide.
            let size = high - low;
            let tall = size.dot(bench.up).abs();
            let wide = (size - bench.up * size.dot(bench.up)).length() / 0.75;
            let fits = f32::max(tall, wide) * 0.5 / libm::tanf(22.5f32.to_radians()) * 1.15;
            ((low + high) * 0.5, f32::max(CLOSEST_M * scale, fits))
        }
        None => (at.translation(), FOCUS_VIEW_M * scale),
    };
    around(bench, target, degrees, distance, 0.15 * scale)
}

/// The world's least and greatest corner round all of `boxes`, each in its entity's space
/// (a socket's held look); `None` when there are none.
fn world_box(boxes: impl Iterator<Item = (Aabb, GlobalTransform)>) -> Option<(Vec3, Vec3)> {
    boxes
        .flat_map(|(bounds, at)| corners(&bounds, &at))
        .fold(None, |was, point| match was {
            Some((low, high)) => Some((Vec3::min(low, point), Vec3::max(high, point))),
            None => Some((point, point)),
        })
}

/// The files a model's clips come from, its clips named in the log: its own, then the
/// figure's (`figure::CLIPS`) if it is on the figure's skeleton, as in the game, so a fight
/// pose shows on a template's shapes (base.glb has them, fight.glb no clips of the fight).
fn clip_files<'a>(
    (gltf, file): (&'a Gltf, &Handle<Gltf>),
    figure_clips: &Handle<Gltf>,
    gltfs: &'a Assets<Gltf>,
    gltf_nodes: &Assets<bevy::gltf::GltfNode>,
    path: &str,
) -> Vec<&'a Gltf> {
    let mut names: Vec<&str> = gltf.named_animations.keys().map(AsRef::as_ref).collect();
    names.sort();
    info!("workbench: {path}: {} clips: {:?}", names.len(), names);
    let mut files = vec![gltf];
    if let Some(more) = gltfs.get(figure_clips)
        && file.id() != figure_clips.id()
        && layer::shares_skeleton(gltf, more, gltf_nodes)
    {
        info!("workbench: {path}: plays {}'s clips too", figure::CLIPS);
        files.push(more);
    }
    files
}

/// A model's graph of the clips of `files` (`layer::graph_of`, its own file first), every clip
/// in it, and the bench's clips placed in it, a clip named `A+B` as `A` under `B`'s hold.
fn placed_clips(
    files: &[&Gltf],
    gltf_nodes: &Assets<bevy::gltf::GltfNode>,
    clips: &Assets<AnimationClip>,
    names: &[String],
    path: &str,
) -> (
    AnimationGraph,
    bevy::platform::collections::HashMap<String, Nodes>,
    Vec<Clip>,
) {
    let (graph, placed) = layer::graph_of(files, gltf_nodes);
    let mut nodes = Vec::new();
    for name in names {
        let (under, over) = match name.split_once('+') {
            Some((under, over)) => (under, Some(over)),
            None => (name.as_str(), None),
        };
        let handle = files
            .iter()
            .find_map(|gltf| gltf.named_animations.get(under));
        let (Some(handle), Some(at)) = (handle, placed.get(under)) else {
            warn!("workbench: {path} has no clip named {under}");
            continue;
        };
        let hold = over.and_then(|over| placed.get(over)).map(|at| at.over);
        if over.is_some() && hold.is_none() {
            warn!("workbench: {path} has no clip named {name}'s hold");
        }
        nodes.push(Clip {
            name: name.clone(),
            nodes: *at,
            length: clips.get(handle).map_or(1.0, AnimationClip::duration),
            hold,
        });
    }
    (graph, placed, nodes)
}

/// Plays `clip` on `model`'s player, held at `fraction` of the way through or looping from
/// its start, its arms laid as the game's figure lays them (`figure::laid_arms`): under the
/// stance's hold its name gives, or in a carried clip under each arm's carry pose.
fn lay(model: &Model, clip: &Clip, fraction: Option<f32>, player: &mut AnimationPlayer) {
    player.stop_all();
    let arms = figure::laid_arms(&clip.name, clip.hold.is_some(), &model.carry);
    // Each hold at its first pose, over its arms (`terra_walker::layer`).
    if let Some(hold) = clip.hold {
        player.play(hold).pause();
    } else if arms != Arms::None {
        for over in model.carry.iter().flatten() {
            player.play(*over).pause();
        }
    }
    let node = clip.nodes.played(0, arms);
    match fraction {
        Some(fraction) => {
            player.play(node).seek_to(clip.length * fraction).pause();
        }
        None => {
            player.play(node).repeat();
        }
    }
}

/// Holds every model at `fraction` of the way through `clip`, or plays it from the start,
/// with anything in hand carried or used as that clip has it.
fn pose(
    bench: &mut Bench,
    clip: &str,
    fraction: Option<f32>,
    players: &mut Query<&mut AnimationPlayer>,
    hands: &mut Query<&mut character::Carrying>,
) {
    for mut hand in hands.iter_mut() {
        hand.0 = character::CARRIED_IN.contains(&clip);
    }
    bench.posed = Some((clip.to_string(), fraction));
    for model in &bench.models {
        let Some(found) = model.clips.iter().find(|c| c.name == clip) else {
            continue;
        };
        if let Some(Ok(mut player)) = model.player.map(|entity| players.get_mut(entity)) {
            lay(model, found, fraction, &mut player);
        }
    }
}

/// Each model's carry poses (`CarryHold`, D560): what its hands hold once loaded, laid as the
/// figure lays them; when they change, every model is posed again as it was.
#[allow(clippy::too_many_arguments)]
fn recarry(
    bench: Option<ResMut<Bench>>,
    children: Query<&Children>,
    names: Query<&Name>,
    poses: Query<&character::CarryHold>,
    looks: Query<(), With<character::Held>>,
    mut players: Query<&mut AnimationPlayer>,
    mut hands: Query<&mut character::Carrying>,
) {
    let Some(mut bench) = bench else { return };
    if !bench.ready() {
        return;
    }
    let mut changed = false;
    for model in &mut bench.models {
        let mut carry = [None; 2];
        for (side, socket, hand) in [
            (0, character::HAND_L, Hand::Off),
            (1, character::HAND_R, Hand::Main),
        ] {
            let pose = character::find(model.root, socket, &children, &names)
                .and_then(|socket| poses.get(socket).ok());
            carry[side] = figure::carry_pose(&model.placed, hand, pose, &looks);
        }
        if carry != model.carry {
            model.carry = carry;
            changed = true;
        }
    }
    if let Some((clip, fraction)) = bench.posed.clone().filter(|_| changed) {
        pose(&mut bench, &clip, fraction, &mut players, &mut hands);
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
        ResMut<Bench>,
        Query<&mut AnimationPlayer>,
        Query<&mut character::Carrying>,
    )>::new(world);
    let (mut bench, mut players, mut hands) = state.get_mut(world).map_err(remote::refused)?;
    if !bench.plays(&clip) {
        return Err(remote::refused(format!(
            "no model plays {clip}; the clips are those given with --anim"
        )));
    }
    pose(&mut bench, &clip, fraction, &mut players, &mut hands);
    Ok(json!({ "clip": clip, "fraction": fraction }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The workbench poses a body after its clips with every layer play does, so its stills
    /// show the fists closed, the shield arm fitted and the carry held as the game shows them.
    #[test]
    fn the_workbench_poses_a_body_with_plays_layers() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        plugin(&mut app);
        let names = terra_walker::pose::system_names(&mut app, PostUpdate);
        for layer in terra_walker::pose::SYSTEMS {
            let found = names
                .iter()
                .filter(|n| n.ends_with(&format!("::{layer}")))
                .count();
            assert_eq!(found, 1, "{layer} in {names:?}");
        }
    }

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
        let (low, high) = standing_span([trunk, crown].into_iter(), stand, up).unwrap();
        assert!(
            low.abs() < 1e-4 && (high - 8.5).abs() < 1e-4,
            "{low} {high}"
        );
        assert_eq!(standing_span(std::iter::empty(), stand, up), None);
        // A part made about a point inside it, an axe head's eye, reaches under the floor
        // by its beard: 5 cm, past SUNK_M, so the bench stands it up on the floor.
        let head = (
            Aabb::from_min_max(Vec3::new(-0.01, -0.05, -0.15), Vec3::new(0.01, 0.07, 0.04)),
            GlobalTransform::from_translation(stand),
        );
        let (low, high) = standing_span([head].into_iter(), stand, up).unwrap();
        assert!((low + 0.05).abs() < 1e-4 && (high - 0.07).abs() < 1e-4 && low < -SUNK_M);
    }

    /// A held tool is framed by the box round all its parts in the world: an axe hanging
    /// head down from a hand, its haft and its head turned half over, spans both.
    #[test]
    fn a_held_look_is_boxed_round_all_its_parts() {
        let hand = Vec3::new(0.3, 1.0, 0.0);
        let down = Quat::from_rotation_x(std::f32::consts::PI);
        let haft = (
            Aabb::from_min_max(Vec3::new(-0.02, -0.08, -0.02), Vec3::new(0.02, 0.6, 0.02)),
            GlobalTransform::from(Transform::from_translation(hand).with_rotation(down)),
        );
        let head = (
            Aabb::from_min_max(Vec3::new(-0.01, 0.0, -0.12), Vec3::new(0.01, 0.1, 0.03)),
            GlobalTransform::from(
                Transform::from_translation(hand - Vec3::Y * 0.55).with_rotation(down),
            ),
        );
        let (low, high) = world_box([haft, head].into_iter()).unwrap();
        // The head, turned over, hangs 0.55 to 0.65 m below the hand; the haft's butt 8 cm above it.
        assert!(
            (low.y - 0.35).abs() < 1e-4 && (high.y - 1.08).abs() < 1e-4,
            "{low} {high}"
        );
        assert!(
            (low.z + 0.03).abs() < 1e-4 && (high.z - 0.12).abs() < 1e-4,
            "{low} {high}"
        );
        assert_eq!(world_box(std::iter::empty()), None);
    }
}
