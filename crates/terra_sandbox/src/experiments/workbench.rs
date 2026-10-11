//! The model workbench (D71): any model on a plain floor in the game's own light, its
//! animations playable, and a sheet of views captured in one run, so a model can be
//! checked in the real renderer by a person or by an agent.
//!
//! `--model <path>` is a .glb or .gltf under `assets/`. `--anim <clip>` plays a clip
//! (repeatable; the first one plays when looking by hand). `--sheet <prefix>` captures
//! each listed clip, paused partway through, from four sides, saves
//! `<prefix>-<clip>-<degrees>.png` for each, then exits. Clips, joints and size are
//! printed when the model loads.

use std::path::PathBuf;

use bevy::{
    asset::RecursiveDependencyLoadState,
    gltf::Gltf,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    world_serialization::{WorldAssetRoot, WorldInstanceReady},
};

use crate::{planet::Planet, shell::Options};

/// Directions the sheet looks from, in degrees round the model from its front.
const SIDES: [f32; 4] = [30.0, 120.0, 210.0, 300.0];
/// How far through a clip the sheet pauses it.
const PAUSE_AT: f32 = 0.4;
/// Frames to let a pose and the camera settle before each capture.
const SETTLE_VIEW: u32 = 12;

#[derive(Resource)]
struct Bench {
    model: Handle<Gltf>,
    clips: Vec<String>,
    sheet: Option<PathBuf>,
    /// Where the model stands, and which way is up there.
    stand: Vec3,
    up: Vec3,
    spawned: bool,
    nodes: Vec<(String, AnimationNodeIndex, f32)>,
    player: Option<Entity>,
    /// Which view the sheet is on, and frames since it was set.
    view: usize,
    waited: u32,
    saving: Option<PathBuf>,
}

#[derive(Component)]
struct Subject;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (spawn, sheet).chain());
}

fn setup(
    mut commands: Commands,
    options: Res<Options>,
    planet: Res<Planet>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(path) = options.model.clone() else {
        error!("the workbench needs --model <path under assets/>");
        return;
    };
    let stand = planet.start();
    let up = stand.normalize();
    // A plain floor, so the model is judged on its own.
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
        model: assets.load(path),
        clips: options.anims.clone(),
        sheet: options.sheet.clone(),
        stand: stand + up * 0.525,
        up,
        spawned: false,
        nodes: Vec::new(),
        player: None,
        view: 0,
        waited: 0,
        saving: None,
    });
}

/// Once the model has loaded: report what is in it, build its clips into a graph, stand it
/// on the floor and point the camera at it.
fn spawn(
    mut commands: Commands,
    bench: Option<ResMut<Bench>>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut bench) = bench else { return };
    if bench.spawned {
        return;
    }
    if let RecursiveDependencyLoadState::Failed(error) =
        assets.recursive_dependency_load_state(&bench.model)
    {
        // Say why and stop, rather than wait for a model that will never come.
        error!("workbench: the model did not load: {error}");
        bench.spawned = true;
        if bench.sheet.is_some() {
            exit.write(AppExit::error());
        }
        return;
    }
    if !assets.is_loaded_with_dependencies(&bench.model) {
        return;
    }
    let Some(gltf) = gltfs.get(&bench.model) else {
        return;
    };
    bench.spawned = true;
    let mut names: Vec<&str> = gltf.named_animations.keys().map(AsRef::as_ref).collect();
    names.sort();
    info!("workbench: {} clips: {:?}", names.len(), names);
    if bench.clips.is_empty() {
        bench.clips = ["Idle_Loop", "Walk_Loop"]
            .into_iter()
            .filter(|name| gltf.named_animations.contains_key(*name))
            .map(String::from)
            .collect();
    }
    let mut graph = AnimationGraph::new();
    let mut nodes = Vec::new();
    for name in &bench.clips {
        let Some(handle) = gltf.named_animations.get(name.as_str()) else {
            warn!("workbench: no clip named {name}");
            continue;
        };
        let duration = clips.get(handle).map_or(1.0, AnimationClip::duration);
        let node = graph.add_clip(handle.clone(), 1.0, graph.root);
        nodes.push((name.clone(), node, duration));
    }
    bench.nodes = nodes;
    let graph = graphs.add(graph);
    let Some(scene) = gltf.default_scene.clone() else {
        return;
    };
    let rotation = Quat::from_rotation_arc(Vec3::Y, bench.up);
    commands
        .spawn((
            Name::new("Workbench model"),
            Subject,
            WorldAssetRoot(scene),
            Transform::from_translation(bench.stand).with_rotation(rotation),
        ))
        .observe(
            move |ready: On<WorldInstanceReady>,
                  mut commands: Commands,
                  children: Query<&Children>,
                  mut players: Query<&mut AnimationPlayer>,
                  mut bench: ResMut<Bench>,
                  joints: Query<&Name>| {
                let joint_count = children
                    .iter_descendants(ready.entity)
                    .filter(|child| {
                        joints
                            .get(*child)
                            .is_ok_and(|name| name.starts_with("DEF-"))
                    })
                    .count();
                info!("workbench: {joint_count} DEF- joints");
                let Some(entity) = children
                    .iter_descendants(ready.entity)
                    .find(|child| players.contains(*child))
                else {
                    info!("workbench: the model has no animation player");
                    return;
                };
                commands
                    .entity(entity)
                    .insert(AnimationGraphHandle(graph.clone()));
                if let (Ok(mut player), Some((_, node, _))) =
                    (players.get_mut(entity), bench.nodes.first())
                {
                    player.play(*node).repeat();
                }
                bench.player = Some(entity);
            },
        );
    // A view of the whole figure from the front quarter.
    if let Ok(mut transform) = camera.single_mut() {
        *transform = view(&bench, SIDES[0]);
    }
}

/// Where the camera stands to see the model from `degrees` round its front.
fn view(bench: &Bench, degrees: f32) -> Transform {
    let up = bench.up;
    let front = up.cross(Vec3::X).normalize();
    let turn = degrees.to_radians();
    let out = Vec3::new(libm::cosf(turn), 0.0, libm::sinf(turn));
    let out = (front * out.x + front.cross(up) * out.z).normalize();
    let target = bench.stand + up * 0.9;
    Transform::from_translation(target + out * 3.6 + up * 0.7).looking_at(target, up)
}

/// The sheet: each clip, paused partway, from each side; one screenshot each, then exit.
fn sheet(
    mut commands: Commands,
    bench: Option<ResMut<Bench>>,
    options: Res<Options>,
    mut players: Query<&mut AnimationPlayer>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
    mut frames: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut bench) = bench else { return };
    let Some(prefix) = bench.sheet.clone() else {
        return;
    };
    *frames += 1;
    if *frames < options.settle_frames || bench.player.is_none() {
        return;
    }
    if let Some(path) = bench.saving.clone() {
        if path.exists() {
            println!("SANDBOX saved {}", path.display());
            bench.saving = None;
            bench.view += 1;
            bench.waited = 0;
        }
        return;
    }
    let views = bench.nodes.len().max(1) * SIDES.len();
    if bench.view >= views {
        exit.write(AppExit::Success);
        return;
    }
    let (clip, side) = (bench.view / SIDES.len(), bench.view % SIDES.len());
    if bench.waited == 0 {
        if let (Some(entity), Some((_, node, duration))) = (bench.player, bench.nodes.get(clip))
            && let Ok(mut player) = players.get_mut(entity)
        {
            player.stop_all();
            player.play(*node).seek_to(duration * PAUSE_AT).pause();
        }
        if let Ok(mut transform) = camera.single_mut() {
            *transform = view(&bench, SIDES[side]);
        }
    }
    bench.waited += 1;
    if bench.waited < SETTLE_VIEW {
        return;
    }
    let name = bench
        .nodes
        .get(clip)
        .map_or("model", |(name, _, _)| name.as_str());
    let path = PathBuf::from(format!(
        "{}-{}-{}.png",
        prefix.display(),
        name,
        SIDES[side] as u32
    ));
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).ok();
    }
    std::fs::remove_file(&path).ok();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()));
    bench.saving = Some(path);
}
