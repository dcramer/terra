//! A staged figure's clips played on its loaded model in a headless app, against the same
//! file played by Bevy alone.

use super::*;
use bevy::animation::AnimatedBy;
use bevy::mesh::VertexAttributeValues;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::world_serialization::WorldAssetRoot;
use terra_core::{PlanetSpec, Seed};
use terra_walker::model_files::{extra, json};

const FPS: f32 = 30.0;

/// The trailer's bends (tools/sandbox/trailer/scenes/cetri.ts): asleep, its thighs turned in;
/// roaring; and coming on, which names no thigh, so they blend back to the clip's.
const ASLEEP: &[(&str, [f32; 3])] = &[
    ("upperarm_l", [-8.0, 0.0, 150.0]),
    ("upperarm_r", [-8.0, 0.0, -150.0]),
    ("lowerarm_l", [0.0, 0.0, 25.0]),
    ("lowerarm_r", [0.0, 0.0, -25.0]),
    ("thigh_l", [0.0, 0.0, -12.0]),
    ("thigh_r", [0.0, 0.0, 12.0]),
    ("spine_01", [2.0, 0.0, 3.0]),
    ("spine_03", [-16.0, 0.0, 0.0]),
    ("neck_01", [-6.0, 0.0, 0.0]),
];
const ROARS: &[(&str, [f32; 3])] = &[
    ("upperarm_l", [-20.0, 0.0, 35.0]),
    ("upperarm_r", [-20.0, 0.0, -35.0]),
    ("spine_03", [-6.0, 0.0, 0.0]),
    ("neck_01", [-6.0, 0.0, 0.0]),
];
const COMES: &[(&str, [f32; 3])] = &[
    ("spine_01", [8.0, 0.0, 0.0]),
    ("spine_02", [4.0, 0.0, 0.0]),
    ("neck_01", [10.0, 0.0, 0.0]),
    ("upperarm_l", [-10.0, 0.0, -30.0]),
    ("upperarm_r", [-10.0, 0.0, 30.0]),
];

fn bends(bend: &[(&str, [f32; 3])]) -> std::collections::BTreeMap<String, [f32; 3]> {
    bend.iter().map(|(n, d)| (n.to_string(), *d)).collect()
}

/// A figure's bends told at once, as `staged.spawn`'s `bend` tells them.
fn bent(bend: &[(&str, [f32; 3])]) -> pose::Bends {
    let mut out = pose::Bends::default();
    out.tell(&bends(bend), 0.0);
    out
}

/// A figure's bends told again over `blend_s`, as `staged.bend` tells them.
fn rebend(app: &mut App, figure: Entity, bend: &[(&str, [f32; 3])], blend_s: f32) {
    let world = app.world_mut();
    let mut told = world.get_mut::<pose::Bends>(figure).unwrap();
    told.tell(&bends(bend), blend_s);
}

/// The entity under `root` called `name`.
fn named_under(world: &mut World, root: Entity, name: &str) -> Option<Entity> {
    let mut all = world.query::<(Entity, &Name)>();
    let candidates: Vec<Entity> = all
        .iter(world)
        .filter(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .collect();
    candidates.into_iter().find(|&e| under(world, e, root))
}

/// Whether `entity` is `root` or below it.
fn under(world: &World, mut entity: Entity, root: Entity) -> bool {
    loop {
        if entity == root {
            return true;
        }
        match world.get::<ChildOf>(entity) {
            Some(parent) => entity = parent.parent(),
            None => return false,
        }
    }
}

/// How far the heights of `samples` range.
fn height_range(samples: &[Vec3]) -> f32 {
    let (lo, hi) = samples.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
        (lo.min(p.y), hi.max(p.y))
    });
    hi - lo
}

/// The largest skinned mesh under `figure`: the body, not its eyes or mouth.
fn body_skin(world: &mut World, figure: Entity) -> Entity {
    let mut skins = world.query_filtered::<(Entity, &Mesh3d), With<SkinnedMesh>>();
    let skins: Vec<(Entity, Handle<Mesh>)> =
        skins.iter(world).map(|(e, m)| (e, m.0.clone())).collect();
    let meshes = world.resource::<Assets<Mesh>>();
    skins
        .into_iter()
        .filter(|(e, _)| under(world, *e, figure))
        .max_by_key(|(_, m)| meshes.get(m).map_or(0, Mesh::count_vertices))
        .expect("the figure has a skinned mesh")
        .0
}

/// Where the skin round `joint` is in `frame`'s frame: the mean of the vertices it weighs most
/// on, each skinned by the linear blend of its joints' matrices (a joint's GlobalTransform times
/// its inverse bindpose), as Bevy's shader skins them. skinning.wesl's dual quaternions put
/// them within 4 mm of this on the Cetri's walk (round 15's probe).
fn skin_round(world: &World, skin: Entity, frame: Entity, joint: &str) -> Vec3 {
    let skinned = world.get::<SkinnedMesh>(skin).unwrap();
    let mesh = world.get::<Mesh3d>(skin).unwrap();
    let mesh = world.resource::<Assets<Mesh>>().get(&mesh.0).unwrap();
    let poses = world.resource::<Assets<SkinnedMeshInverseBindposes>>();
    let poses = poses.get(&skinned.inverse_bindposes).unwrap();
    let matrices: Vec<Mat4> = skinned
        .joints
        .iter()
        .zip(poses.iter())
        .map(|(j, pose)| Mat4::from(world.get::<GlobalTransform>(*j).unwrap().affine()) * *pose)
        .collect();
    let picked: Vec<bool> = skinned
        .joints
        .iter()
        .map(|j| world.get::<Name>(*j).is_some_and(|n| n.as_str() == joint))
        .collect();
    let (
        Some(VertexAttributeValues::Float32x3(at)),
        Some(VertexAttributeValues::Uint16x4(index)),
        Some(VertexAttributeValues::Float32x4(weight)),
    ) = (
        mesh.attribute(Mesh::ATTRIBUTE_POSITION),
        mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX),
        mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT),
    )
    else {
        panic!("the skin has no positions, joint indices or weights as Bevy loads them");
    };
    let to_frame = Mat4::from(world.get::<GlobalTransform>(frame).unwrap().affine()).inverse();
    let (mut sum, mut n) = (Vec3::ZERO, 0);
    for ((p, i), w) in at.iter().zip(index).zip(weight) {
        let most = (0..4).max_by(|&a, &b| w[a].total_cmp(&w[b])).unwrap();
        if !picked[i[most] as usize] {
            continue;
        }
        let total: f32 = w.iter().sum();
        let m = (0..4).fold(Mat4::ZERO, |m, k| {
            m + matrices[i[k] as usize] * (w[k] / total)
        });
        sum += to_frame.transform_point3(m.transform_point3(Vec3::from(*p)));
        n += 1;
    }
    assert!(n > 0, "no vertex of the skin is weighed most on {joint}");
    sum / n as f32
}

/// Runs until the staged `figure`'s player is given its graph by `dress`, and a plain body of
/// the same file is in, its player playing Giant_Walk alone: the two players' entities, the
/// staged one's and the plain body's root.
fn come_in(app: &mut App, model: &Handle<Gltf>, figure: Entity) -> (Entity, Entity) {
    let server = app.world().resource::<AssetServer>().clone();
    let start = std::time::Instant::now();
    let mut plain: Option<(Entity, Handle<AnimationGraph>, AnimationNodeIndex)> = None;
    loop {
        app.update();
        let world = app.world_mut();
        if plain.is_none() && server.is_loaded_with_dependencies(model) {
            let gltf = world.resource::<Assets<Gltf>>().get(model).unwrap();
            let scene = gltf.default_scene.clone().unwrap();
            let walk = gltf.named_animations["Giant_Walk"].clone();
            let (graph, node) = AnimationGraph::from_clip(walk);
            let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
            let root = world
                .spawn((WorldAssetRoot(scene), Transform::default()))
                .id();
            plain = Some((root, graph, node));
        }
        let mut animators = world.query::<(Entity, &Animates)>();
        let staged = animators
            .iter(world)
            .find(|(_, a)| a.staged == figure)
            .map(|(e, _)| e);
        let mut players = world.query_filtered::<Entity, With<AnimationPlayer>>();
        let all: Vec<Entity> = players.iter(world).collect();
        let reference = plain
            .as_ref()
            .and_then(|(root, _, _)| all.iter().copied().find(|&p| under(world, p, *root)));
        if let (Some(staged), Some(reference), Some((root, graph, node))) =
            (staged, reference, plain.clone())
        {
            world
                .entity_mut(reference)
                .insert(AnimationGraphHandle(graph));
            world
                .get_mut::<AnimationPlayer>(reference)
                .unwrap()
                .play(node)
                .repeat();
            return (staged, root);
        }
        assert!(start.elapsed().as_secs() < 90, "the model never came in");
    }
}

/// How many of Giant_Walk's curve targets are bones animated by `player`, of how many.
fn targets_found(world: &mut World, model: &Handle<Gltf>, player: Entity) -> (usize, usize) {
    let gltf = world.resource::<Assets<Gltf>>().get(model).unwrap();
    let walk = gltf.named_animations["Giant_Walk"].clone();
    let clips = world.resource::<Assets<AnimationClip>>();
    let targets: Vec<AnimationTargetId> =
        clips.get(&walk).unwrap().curves().keys().copied().collect();
    let mut bones = world.query::<(&AnimationTargetId, &AnimatedBy)>();
    let on_body: Vec<AnimationTargetId> = bones
        .iter(world)
        .filter(|(_, by)| by.0 == player)
        .map(|(id, _)| *id)
        .collect();
    let found = targets.iter().filter(|t| on_body.contains(t)).count();
    (found, targets.len())
}

/// A headless app that loads models and plays their clips, with the stage's own systems.
fn app() -> App {
    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: assets.to_string_lossy().into_owned(),
            ..default()
        },
        ImagePlugin::default(),
        bevy::mesh::MeshPlugin,
        bevy::scene::ScenePlugin,
        bevy::world_serialization::WorldSerializationPlugin,
        TransformPlugin,
        bevy::animation::AnimationPlugin,
        terra_world::gltf_gate::GatedGltfPlugin::default(),
    ));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_secs_f32(1.0 / FPS),
    ));
    app.insert_resource(Planet::smooth(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    }));
    figures(&mut app);
    app.finish();
    app.cleanup();
    app
}

/// The clip playing on a staged player, by name, with its weight and time: for the record.
fn playing(world: &World, player: Entity) -> Vec<String> {
    let animates = world.get::<Animates>(player).unwrap();
    let name_of = |node: AnimationNodeIndex| {
        animates.clips.iter().find_map(|(name, n)| {
            let all = n.whole.iter().chain(n.under.iter()).chain([&n.over]);
            all.copied().any(|i| i == node).then(|| name.clone())
        })
    };
    world
        .get::<AnimationPlayer>(player)
        .unwrap()
        .playing_animations()
        .map(|(node, active)| {
            format!(
                "{:?} (node {}) weight {:.3} at {:.2} s speed {:.2}{}{}",
                name_of(*node),
                node.index(),
                active.weight(),
                active.seek_time(),
                active.speed(),
                if active.is_finished() {
                    " finished"
                } else {
                    ""
                },
                if active.is_paused() { " paused" } else { "" },
            )
        })
        .collect()
}

/// The Cetri's walk moves its body in the game as its clip does in its file (round 14): a
/// figure staged as the trailer stages it (asleep in its idle, a roar played once and held,
/// then the walk blended in while it goes at the walk's pace) raises and drops its hips and
/// lifts its feet as far as the same file's Giant_Walk does on a body Bevy plays alone, one
/// clip in its graph. That body is the answer known another way; its hips must rise and fall
/// most of the clip's own 0.28 m (0.085 of the rig's units, scaled 3.3), so a still file fails
/// too. Rounds 10-13 filmed legs that barely moved whatever was changed around them. And the
/// skin follows its bones: round the left foot it lifts most of what the foot bone does.
#[test]
fn a_staged_walk_moves_the_hips_and_feet_as_its_clip_does_after_an_idle_and_a_roar() {
    const CLIP_HIPS_M: f32 = 0.28;
    const FILE: &str = "models/characters/cetri.glb";
    let mut app = app();
    let pace = extra(&json(FILE), "paces").unwrap()["Giant_Walk"]
        .as_f64()
        .unwrap() as f32;
    let ground = app.world().resource::<Planet>().ground_at(Vec3::Y * 2100.0);
    let server = app.world().resource::<AssetServer>().clone();
    let model: Handle<Gltf> = server.load(FILE);
    let up = ground.normalize();
    let facing = up.any_orthonormal_vector();
    let figure = app
        .world_mut()
        .spawn((
            Staged {
                name: "cetri".into(),
                going: None,
                face: Face::Way,
                facing,
                turn: 0.5,
                speed_now: 0.0,
                scale: 1.0,
                asked: None,
                asked_n: 0,
                rate: 1.0,
            },
            Dressing {
                model: model.clone(),
                clips: None,
                shape: None,
                hold: Vec::new(),
            },
            Transform::from_translation(ground).looking_to(facing, up),
            Visibility::default(),
            bent(ASLEEP),
        ))
        .id();

    let (staged_player, plain_root) = come_in(&mut app, &model, figure);
    let run = |app: &mut App, seconds: f32| {
        for _ in 0..(seconds * FPS) as usize {
            app.update();
        }
    };
    let tell = |app: &mut App, clip: &str, speed: f32, once: bool, blend_ms: u64, from_s: f32| {
        let world = app.world_mut();
        let mut staged = world.get_mut::<Staged>(figure).unwrap();
        staged.tell_play(clip, speed, once, blend_ms, from_s);
    };
    // As tools/sandbox/trailer/scenes/cetri.ts stages it.
    tell(&mut app, "Troll_Idle", 0.04, false, 0, 0.0);
    run(&mut app, 2.0);
    tell(&mut app, "Troll_Roar", 0.6, true, 1200, 0.0);
    rebend(&mut app, figure, ROARS, 2.2);
    run(&mut app, 8.0);
    tell(&mut app, "Giant_Walk", 1.0, false, 900, 0.7);
    rebend(&mut app, figure, COMES, 0.9);
    let far = ground + facing * 500.0;
    let world = app.world_mut();
    world
        .get_mut::<Staged>(figure)
        .unwrap()
        .tell_go(Some(far), pace);
    run(&mut app, 1.5);
    for line in playing(app.world(), staged_player) {
        println!("staged plays {line}");
    }

    let (found, all) = targets_found(app.world_mut(), &model, staged_player);
    println!("Giant_Walk's {all} targets: {found} on the staged body");

    // Three seconds, the walk's cycle: each bone's place in its body's frame, every frame.
    let names = ["pelvis", "foot_l", "foot_r"];
    let world = app.world_mut();
    let staged_bones: Vec<Entity> = names
        .iter()
        .map(|b| named_under(world, figure, b).unwrap())
        .collect();
    let plain_bones: Vec<Entity> = names
        .iter()
        .map(|b| named_under(world, plain_root, b).unwrap())
        .collect();
    let mut staged_at = vec![Vec::new(); names.len()];
    let mut plain_at = vec![Vec::new(); names.len()];
    let skin = body_skin(app.world_mut(), figure);
    let mut skin_at = Vec::new();
    for _ in 0..(3.0 * FPS) as usize {
        app.update();
        let world = app.world();
        skin_at.push(skin_round(world, skin, figure, "foot_l"));
        let local = |frame: Entity, bone: Entity| {
            let frame = world
                .get::<GlobalTransform>(frame)
                .unwrap()
                .affine()
                .inverse();
            frame.transform_point3(world.get::<GlobalTransform>(bone).unwrap().translation())
        };
        for (at, &bone) in staged_at.iter_mut().zip(&staged_bones) {
            at.push(local(figure, bone));
        }
        for (at, &bone) in plain_at.iter_mut().zip(&plain_bones) {
            at.push(local(plain_root, bone));
        }
    }
    let staged: Vec<f32> = staged_at.iter().map(|s| height_range(s)).collect();
    let alone: Vec<f32> = plain_at.iter().map(|s| height_range(s)).collect();
    for ((name, staged), alone) in names.iter().zip(&staged).zip(&alone) {
        println!("{name}: rises and falls {staged:.3} m staged, {alone:.3} m played alone");
    }
    assert!(
        alone[0] > 0.8 * CLIP_HIPS_M,
        "played alone, the file's walk moves its hips {:.3} m, not most of {CLIP_HIPS_M}",
        alone[0]
    );
    for ((name, staged), alone) in names.iter().zip(&staged).zip(&alone) {
        assert!(
            *staged > 0.8 * alone,
            "staged, the walk moves its {name} {staged:.3} m; played alone, {alone:.3} m"
        );
    }
    // The skin follows: round the left foot it lifts most of the bone's lift (0.56 of 0.62 m).
    let skin_lift = height_range(&skin_at);
    println!("the skin round foot_l rises and falls {skin_lift:.3} m");
    assert!(
        skin_lift > 0.8 * staged[1],
        "the skin round the left foot rises {skin_lift:.3} m; its bone {:.3} m",
        staged[1]
    );
}
