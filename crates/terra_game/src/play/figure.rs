//! The player's body: the male base body, made with Tripo and rigged onto the skeleton
//! of a free animation library (D115, THIRD_PARTY.md), with what is in hand held in its
//! right hand's socket (`character`). Which animation plays follows what the player is doing.
//!
//! A base body has no hair or clothes of its own; those are parts still to come (D111,
//! D112).

use super::held::Grip;
use super::{Downed, Motion, Player, Stance};
use crate::character::{CARRIED_IN, Carrying};
use crate::loading::Loading;
use bevy::{prelude::*, world_serialization::WorldInstanceReady};
use std::time::Duration;

/// The base body the player has, which is also the name its gear is fitted under
/// (`character::look`).
pub const BODY: &str = "base-male";
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

impl Doing {
    /// The clip that shows it.
    fn clip(self) -> &'static str {
        match self {
            Doing::Idle => "Idle_Loop",
            Doing::Jog => "Jog_Fwd_Loop",
            Doing::Sprint => "Sprint_Loop",
            Doing::Fall => "Jump_Loop",
            Doing::Use => "Interact",
            Doing::Down => "Death01",
            Doing::Attack => "Sword_Attack",
            Doing::Flinch => "Hit_Chest",
            Doing::Roll => "Roll",
            Doing::Guard => "Sword_Idle",
        }
    }
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

fn load(mut commands: Commands, assets: Res<AssetServer>, mut loading: ResMut<Loading>) {
    let model = assets.load(format!("models/character/{BODY}.glb"));
    loading.wait_for(&model);
    commands.insert_resource(FigureModel(model));
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
    let (graph, nodes) = AnimationGraph::from_clips(
        [
            Doing::Idle,
            Doing::Jog,
            Doing::Sprint,
            Doing::Fall,
            Doing::Use,
            Doing::Down,
            Doing::Attack,
            Doing::Flinch,
            Doing::Roll,
            Doing::Guard,
        ]
        .map(|doing| clip(doing.clip())),
    );
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
            crate::character::Body { name: BODY },
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
    names: Query<&Name>,
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

    // Whatever is in hand is held in the hand's socket (`held`).
    match crate::character::find(ready.entity, crate::character::HAND_R, &children, &names) {
        Some(hand) => {
            commands.entity(hand).insert((Grip, Carrying(true)));
        }
        None => warn!("The figure has no {}.", crate::character::HAND_R),
    }
}

/// Picks the animation that fits what the player is doing, and blends into it.
fn animate(
    moves: Option<Res<Moves>>,
    player: Single<(&Motion, &Stance, Has<Downed>), With<Player>>,
    mut figures: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut Animated,
    )>,
    mut hands: Query<&mut Carrying, With<Grip>>,
) {
    let Some(moves) = moves else { return };
    let (motion, stance, downed) = *player;
    let speed = motion.velocity.length();
    let doing = if downed {
        Doing::Down
    } else if stance.dodging > 0.0 {
        Doing::Roll
    } else if stance.flinch > 0.0 {
        Doing::Flinch
    } else if stance.attacking > 0.0 {
        Doing::Attack
    } else if stance.guarding {
        Doing::Guard
    } else if motion.airborne > 0.12 {
        Doing::Fall
    } else if speed > 5.6 {
        Doing::Sprint
    } else if speed > 0.5 {
        Doing::Jog
    } else if stance.since_acting < 0.9 {
        Doing::Use
    } else {
        Doing::Idle
    };
    for (mut animator, mut transitions, mut animated) in &mut figures {
        if animated.0 == doing {
            continue;
        }
        animated.0 = doing;
        for mut hand in &mut hands {
            hand.0 = CARRIED_IN.contains(&doing.clip());
        }
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
