//! The player's body: a rigged figure from a free animation library (see
//! THIRD_PARTY.md), in our colours, with a visor, a pack and the builder tool fixed to
//! its bones. Which animation plays follows what the player is doing.
//!
//! It is a stand-in. The game's own character is a design question (docs/player.md).

use super::Player;
use bevy::{prelude::*, world_serialization::WorldInstanceReady};
use std::time::Duration;

const MODEL: &str = "models/character/figure.gltf";
/// How long one animation takes to give way to the next.
const BLEND: Duration = Duration::from_millis(180);

#[derive(Resource)]
struct FigureModel(Handle<Gltf>);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Doing {
    Idle,
    Jog,
    Sprint,
    Fall,
    Use,
    Down,
    Attack,
    Flinch,
    Roll,
    Guard,
}

/// The animations, as places in the figure's animation graph.
#[derive(Resource)]
struct Moves {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    jog: AnimationNodeIndex,
    sprint: AnimationNodeIndex,
    fall: AnimationNodeIndex,
    using: AnimationNodeIndex,
    down: AnimationNodeIndex,
    attack: AnimationNodeIndex,
    flinch: AnimationNodeIndex,
    roll: AnimationNodeIndex,
    guard: AnimationNodeIndex,
}

/// What the figure is animated as doing.
#[derive(Component)]
struct Animated(Doing);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load).add_systems(
        Update,
        (spawn.run_if(not(resource_exists::<Moves>)), animate),
    );
}

fn load(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(FigureModel(assets.load(MODEL)));
}

/// Once the model has loaded, stands it on the player.
fn spawn(
    mut commands: Commands,
    model: Res<FigureModel>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    player: Single<Entity, With<Player>>,
) {
    if !assets.is_loaded_with_dependencies(&model.0) {
        return;
    }
    let Some(gltf) = gltfs.get(&model.0) else {
        return;
    };
    let clip = |name: &str| gltf.named_animations[name].clone();
    let (graph, nodes) = AnimationGraph::from_clips([
        clip("Idle_Loop"),
        clip("Jog_Fwd_Loop"),
        clip("Sprint_Loop"),
        clip("Jump_Loop"),
        clip("Interact"),
        clip("Death01"),
        clip("Sword_Attack"),
        clip("Hit_Chest"),
        clip("Roll"),
        clip("Sword_Idle"),
    ]);
    commands.insert_resource(Moves {
        graph: graphs.add(graph),
        idle: nodes[0],
        jog: nodes[1],
        sprint: nodes[2],
        fall: nodes[3],
        using: nodes[4],
        down: nodes[5],
        attack: nodes[6],
        flinch: nodes[7],
        roll: nodes[8],
        guard: nodes[9],
    });
    let Some(scene) = gltf.default_scene.clone() else {
        return;
    };
    commands
        .spawn((
            Name::new("Figure"),
            WorldAssetRoot(scene),
            // The model faces +Z; things here face -Z.
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            ChildOf(*player),
        ))
        .observe(dress);
}

/// When the figure's bones exist, starts it animating and fixes its kit to them.
#[allow(clippy::too_many_arguments)]
fn dress(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    moves: Res<Moves>,
    children: Query<&Children>,
    mut animators: Query<&mut AnimationPlayer>,
    bones: Query<(Entity, &Name)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // The figure's own animation player, among everything its model spawned; creatures
    // have theirs.
    let Some(entity) = children
        .iter_descendants(ready.entity)
        .find(|&child| animators.contains(child))
    else {
        return;
    };
    let Ok(mut animator) = animators.get_mut(entity) else {
        return;
    };
    let mut transitions = AnimationTransitions::new();
    transitions
        .play(&mut animator, moves.idle, Duration::ZERO)
        .repeat();
    commands.entity(entity).insert((
        AnimationGraphHandle(moves.graph.clone()),
        transitions,
        Animated(Doing::Idle),
    ));

    let mut look = |colour: Color, glow: LinearRgba| {
        materials.add(StandardMaterial {
            base_color: colour,
            emissive: glow,
            perceptual_roughness: 0.6,
            ..default()
        })
    };
    let dark = look(Color::srgb(0.1, 0.11, 0.14), LinearRgba::BLACK);
    let orange = look(Color::srgb(0.9, 0.46, 0.17), LinearRgba::BLACK);
    let pale = look(Color::srgb(0.83, 0.8, 0.74), LinearRgba::BLACK);
    let glow = look(Color::srgb(0.3, 0.9, 1.0), LinearRgba::rgb(1.0, 6.0, 8.0));
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.1, 0.14),
        emissive: LinearRgba::rgb(0.1, 0.5, 0.7),
        perceptual_roughness: 0.1,
        reflectance: 0.8,
        ..default()
    });
    let bone = |name: &str| {
        children
            .iter_descendants(ready.entity)
            .filter_map(|child| bones.get(child).ok())
            .find(|(_, bone)| bone.as_str() == name)
            .map(|(entity, _)| entity)
    };
    // Each piece: the bone it rides on, its shape, its look, and where it sits on the
    // bone. A bone's own +Y runs along it.
    let kit: [(&str, Mesh, &Handle<StandardMaterial>, Transform); 8] = [
        (
            "DEF-head",
            Cuboid::new(0.2, 0.09, 0.06).into(),
            &glass,
            Transform::from_xyz(0.0, 0.13, 0.105),
        ),
        (
            "DEF-spine.003",
            Cuboid::new(0.32, 0.4, 0.16).into(),
            &pale,
            Transform::from_xyz(0.0, 0.06, -0.19),
        ),
        (
            "DEF-spine.003",
            Cuboid::new(0.34, 0.07, 0.18).into(),
            &orange,
            Transform::from_xyz(0.0, 0.2, -0.19),
        ),
        (
            "DEF-spine.003",
            Cuboid::new(0.1, 0.1, 0.03).into(),
            &glow,
            Transform::from_xyz(0.0, 0.04, -0.275),
        ),
        (
            "DEF-spine.003",
            Cylinder::new(0.035, 0.3).into(),
            &dark,
            Transform::from_xyz(0.19, 0.06, -0.19),
        ),
        (
            "DEF-hand.R",
            Cuboid::new(0.06, 0.24, 0.08).into(),
            &dark,
            Transform::from_xyz(0.0, 0.1, 0.05),
        ),
        (
            "DEF-hand.R",
            Cuboid::new(0.08, 0.05, 0.1).into(),
            &orange,
            Transform::from_xyz(0.0, 0.2, 0.05),
        ),
        (
            "DEF-hand.R",
            Cuboid::new(0.04, 0.04, 0.04).into(),
            &glow,
            Transform::from_xyz(0.0, 0.25, 0.05),
        ),
    ];
    for (name, mesh, material, transform) in kit {
        let Some(bone) = bone(name) else {
            warn!("The figure has no bone called {name}.");
            continue;
        };
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            transform,
            ChildOf(bone),
        ));
    }
}

/// Picks the animation that fits what the player is doing, and blends into it.
fn animate(
    moves: Option<Res<Moves>>,
    player: Single<&Player>,
    mut figures: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut Animated,
    )>,
) {
    let Some(moves) = moves else { return };
    let speed = player.velocity.length();
    let doing = if player.down.is_some() {
        Doing::Down
    } else if player.dodging > 0.0 {
        Doing::Roll
    } else if player.flinch > 0.0 {
        Doing::Flinch
    } else if player.attacking > 0.0 {
        Doing::Attack
    } else if player.guarding {
        Doing::Guard
    } else if player.airborne > 0.12 {
        Doing::Fall
    } else if speed > 5.6 {
        Doing::Sprint
    } else if speed > 0.5 {
        Doing::Jog
    } else if player.since_acting < 0.9 {
        Doing::Use
    } else {
        Doing::Idle
    };
    for (mut animator, mut transitions, mut animated) in &mut figures {
        if animated.0 == doing {
            continue;
        }
        animated.0 = doing;
        let node = match doing {
            Doing::Idle => moves.idle,
            Doing::Jog => moves.jog,
            Doing::Sprint => moves.sprint,
            Doing::Fall => moves.fall,
            Doing::Use => moves.using,
            Doing::Down => moves.down,
            Doing::Attack => moves.attack,
            Doing::Flinch => moves.flinch,
            Doing::Roll => moves.roll,
            Doing::Guard => moves.guard,
        };
        let playing = transitions.play(&mut animator, node, BLEND);
        if !matches!(
            doing,
            Doing::Use | Doing::Down | Doing::Attack | Doing::Flinch | Doing::Roll
        ) {
            playing.repeat();
        }
    }
}
