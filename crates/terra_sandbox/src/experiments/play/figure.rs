//! The player's body: the male base body, made with Tripo and rigged onto the skeleton
//! of a free animation library (D115, THIRD_PARTY.md), with what is in hand fixed to its
//! right hand. Which animation plays follows what the player is doing.
//!
//! A base body has no hair or clothes of its own; those are parts still to come (D111,
//! D112).

use super::Player;
use super::held::Grip;
use bevy::{prelude::*, world_serialization::WorldInstanceReady};
use std::time::Duration;

const MODEL: &str = "models/character/base-male.glb";
/// How long one animation takes to give way to the next.
const BLEND: Duration = Duration::from_millis(180);
/// Turns a held tool, which runs along +Y, to lie across the right palm with its head
/// above the thumb. Found by looking at pictures of it.
const GRIP_TURN: [f32; 3] = [std::f32::consts::FRAC_PI_2, 0.0, 0.0];

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

/// When the figure's bones exist, starts it animating and gives it a hand to hold things in.
fn dress(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    moves: Res<Moves>,
    children: Query<&Children>,
    mut animators: Query<&mut AnimationPlayer>,
    bones: Query<(Entity, &Name)>,
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

    let bone = |name: &str| {
        children
            .iter_descendants(ready.entity)
            .filter_map(|child| bones.get(child).ok())
            .find(|(_, bone)| bone.as_str() == name)
            .map(|(entity, _)| entity)
    };
    // Whatever is in hand is held here: across the palm, head toward the thumb (`held`).
    if let Some(hand) = bone("DEF-hand.R") {
        commands.spawn((
            Grip,
            Transform::from_xyz(0.0, 0.08, 0.03).with_rotation(Quat::from_euler(
                EulerRot::XYZ,
                GRIP_TURN[0],
                GRIP_TURN[1],
                GRIP_TURN[2],
            )),
            Visibility::default(),
            ChildOf(hand),
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
