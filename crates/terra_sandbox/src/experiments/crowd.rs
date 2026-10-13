//! A crowd of dressed characters, to measure what many cost (docs/characters.md,
//! phase 1): `--crowd <n>` bodies (100 by default) in a grid, every template in turn, each
//! wearing `--wear` items (a coat, cap, pack and quiver by default) and playing an idle, walk
//! or jog from its own point in the clip. Once all are dressed it prints what they add up to:
//! skinned meshes, joint matrices, entities. With `--bench`, the shell times the frames.
//!
//!   cargo run -p terra_sandbox -- --experiment crowd --crowd 500 --bench

use bevy::{
    gltf::Gltf,
    mesh::skinning::SkinnedMesh,
    prelude::*,
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};
use terra_game::{
    appearance::{Appearance, templates},
    character,
    planet::{MainCamera, Planet},
    shell::Options,
};

const BODY: &str = "models/character/base.glb";
const OUTFIT: [&str; 4] = ["leather_coat", "leather_cap", "small_pack", "quiver"];
const CLIPS: [&str; 3] = ["Idle_Loop", "Walk_Loop", "Jog_Fwd_Loop"];
const SPACING_M: f32 = 1.4;

#[derive(Resource)]
struct Crowd {
    body: Handle<Gltf>,
    looks: Vec<Handle<WorldAsset>>,
    count: usize,
    stand: Vec3,
    up: Vec3,
    spawned: bool,
    dressed: usize,
    reported: bool,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (spawn, report));
}

fn setup(
    mut commands: Commands,
    options: Res<Options>,
    planet: Res<Planet>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let stand = planet.start();
    let up = stand.normalize();
    let wears: Vec<String> = if options.wears.is_empty() {
        OUTFIT.iter().map(|w| w.to_string()).collect()
    } else {
        options.wears.clone()
    };
    let count = options.crowd.unwrap_or(100);
    let side = (count as f32).sqrt().ceil();
    // A plain floor under them all.
    commands.spawn((
        Name::new("Crowd floor"),
        Mesh3d(meshes.add(Cylinder::new(side * SPACING_M * 0.8 + 2.0, 0.05))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.6, 0.56),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_translation(stand + up * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, up)),
    ));
    commands.insert_resource(Crowd {
        body: assets.load(BODY),
        looks: wears
            .iter()
            .map(|item| assets.load(GltfAssetLabel::Scene(0).from_asset(character::look(item))))
            .collect(),
        count,
        stand: stand + up * 0.525,
        up,
        spawned: false,
        dressed: 0,
        reported: false,
    });
}

/// Once the body has loaded: the crowd in a square, each a template in turn, the camera on it.
fn spawn(
    mut commands: Commands,
    mut crowd: ResMut<Crowd>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    clips: Res<Assets<AnimationClip>>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    if crowd.spawned || !assets.is_loaded_with_dependencies(&crowd.body) {
        return;
    }
    let Some(gltf) = gltfs.get(&crowd.body) else {
        return;
    };
    let Some(scene) = gltf.default_scene.clone() else {
        error!("crowd: {BODY} has no scene");
        return;
    };
    crowd.spawned = true;
    let handles: Vec<Handle<AnimationClip>> = CLIPS
        .iter()
        .filter_map(|name| gltf.named_animations.get(*name).cloned())
        .collect();
    let lengths: Vec<f32> = handles
        .iter()
        .map(|h| clips.get(h).map_or(1.0, AnimationClip::duration))
        .collect();
    let (graph, nodes) = AnimationGraph::from_clips(handles);
    let graph = graphs.add(graph);
    let mut names: Vec<&String> = templates().keys().collect();
    names.sort();
    let side = (crowd.count as f32).sqrt().ceil() as usize;
    let up = crowd.up;
    let east = Vec3::X.cross(up).normalize().cross(up).normalize();
    let north = up.cross(east);
    let turned = Quat::from_rotation_arc(Vec3::Y, up);
    for index in 0..crowd.count {
        let (row, column) = (index / side, index % side);
        let at = crowd.stand
            + east * (column as f32 - side as f32 * 0.5) * SPACING_M
            + north * (row as f32 - side as f32 * 0.5) * SPACING_M;
        let shape = names
            .get(index % names.len().max(1))
            .and_then(|name| Appearance::template(name))
            .unwrap_or_default();
        let (graph, node, length) = (
            graph.clone(),
            nodes.get(index % nodes.len().max(1)).copied(),
            lengths
                .get(index % lengths.len().max(1))
                .copied()
                .unwrap_or(1.0),
        );
        // Each from its own point in the clip, so no two move in step.
        let start = length * ((index as f32 * 0.618_034) % 1.0);
        let looks = crowd.looks.clone();
        commands
            .spawn((
                Name::new(format!("Crowd {index}")),
                character::Body,
                shape,
                WorldAssetRoot(scene.clone()),
                Transform::from_translation(at).with_rotation(turned),
            ))
            .observe(
                move |ready: On<WorldInstanceReady>,
                      mut commands: Commands,
                      children: Query<&Children>,
                      mut players: Query<&mut AnimationPlayer>,
                      mut crowd: ResMut<Crowd>| {
                    for look in &looks {
                        character::wear(&mut commands, look.clone(), ready.entity);
                    }
                    if let Some(entity) = children
                        .iter_descendants(ready.entity)
                        .find(|each| players.contains(*each))
                    {
                        commands
                            .entity(entity)
                            .insert(AnimationGraphHandle(graph.clone()));
                        if let (Ok(mut player), Some(node)) = (players.get_mut(entity), node) {
                            player.play(node).repeat().seek_to(start);
                        }
                    }
                    crowd.dressed += 1;
                },
            );
    }
    if let Ok(mut transform) = camera.single_mut() {
        let across = side as f32 * SPACING_M;
        let eye = crowd.stand - north * (across * 0.9 + 3.0) + up * (across * 0.45 + 2.0);
        *transform = Transform::from_translation(eye).looking_at(crowd.stand, up);
    }
}

/// Once every body is dressed and its looks are on: what the crowd adds up to.
fn report(
    mut crowd: ResMut<Crowd>,
    skins: Query<&SkinnedMesh>,
    worn: Query<(), With<character::Worn>>,
    entities: Query<Entity>,
    frames: Res<bevy::diagnostic::FrameCount>,
    mut since: Local<Option<u32>>,
) {
    if crowd.reported || !crowd.spawned || crowd.dressed < crowd.count {
        return;
    }
    // A few frames after the last is dressed, for its looks to be put on.
    let first = *since.get_or_insert(frames.0);
    if frames.0 < first + 30 {
        return;
    }
    crowd.reported = true;
    let meshes = skins.iter().count();
    let joints: usize = skins.iter().map(|skin| skin.joints.len()).sum();
    println!(
        "CROWD {} characters: {} looks worn, {} skinned meshes, {} joint matrices a frame ({:.0} KB), {} entities",
        crowd.count,
        worn.iter().count(),
        meshes,
        joints,
        joints as f32 * 64.0 / 1024.0,
        entities.iter().count()
    );
}
