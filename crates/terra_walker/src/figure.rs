//! The player's body: the base body (D112, `appearance`), shaped as the player's template,
//! made with Tripo and rigged onto the skeleton of a free animation library (D115,
//! THIRD_PARTY.md), with what is in hand held in its right hand's socket (`character`).
//! Every player stands on one, and which animation plays follows what that player is doing.
//!
//! A base body has no hair or clothes of its own; those are parts still to come (D111,
//! D112).
//!
//! Its clips are the fight file's (`CLIPS`: the base's and the sword fight's, on the base's
//! skeleton), two of each so a clip played again blends from itself. A layer above may ask
//! for a clip in place of what walking shows (`Acting`), as the sword fight does (`melee`).

use crate::character::{CARRIED_IN, Carrying};
use crate::{Downed, Motion, Player, Stance};
use bevy::{
    gltf::GltfNode, platform::collections::HashMap, prelude::*,
    world_serialization::WorldInstanceReady,
};
use std::time::Duration;
use terra_world::loading::Loading;

/// The template the player's body is shaped as (`appearance::Appearance::template`).
pub const TEMPLATE: &str = "male";
/// How long one animation takes to give way to the next.
const BLEND: Duration = Duration::from_millis(180);
/// The figure's clips: the base's and the sword fight's, on the base's skeleton.
pub const CLIPS: &str = "models/character/fight.glb";

#[derive(Resource)]
struct FigureModel {
    model: Handle<Gltf>,
    clips: Handle<Gltf>,
}

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
    /// Playing what a layer above asked (`Acting`).
    Acting,
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
            Doing::Acting => "Fight_Guard",
        }
    }

    /// Whether it plays once and holds, rather than looping.
    fn once(self) -> bool {
        matches!(
            self,
            Doing::Use | Doing::Down | Doing::Attack | Doing::Flinch | Doing::Roll
        )
    }
}

/// The animations, as places in the figure's animation graph: every clip by name, twice.
#[derive(Resource)]
pub struct Moves {
    graph: Handle<AnimationGraph>,
    clips: HashMap<String, [AnimationNodeIndex; 2]>,
    /// The stride clips' own paces (`pace.rs`), so their feet do not slide.
    paces: HashMap<String, f32>,
}

impl Moves {
    /// A stride clip's own pace, metres a second at its own speed, if it has one.
    pub fn pace(&self, clip: &str) -> Option<f32> {
        self.paces.get(clip).copied()
    }
}

/// The socket in a figure's right hand (`character::HAND_R`), and whose hand it is. What is
/// in hand is drawn as its children, by the layers that know what it is (`held`, `melee`).
#[derive(Component)]
pub struct Grip {
    pub player: Entity,
}

/// On a figure's animation player: whose figure it is, its hand, what it is animated as
/// doing, the clip playing and which copy, and the last ask of `Acting` it played.
#[derive(Component)]
struct Animated {
    player: Entity,
    hand: Option<Entity>,
    doing: Doing,
    playing: Option<(String, usize)>,
    applied: u32,
}

/// A clip a layer above asks a player's figure to play in place of what its walking shows.
#[derive(Component, Default)]
pub struct Acting {
    pub clip: Option<Act>,
    asked: u32,
}

/// How to play an asked clip: its speed (0 holds it), from how far in, once or looping, and
/// where to hold it.
#[derive(Clone, Debug, PartialEq)]
pub struct Act {
    pub clip: String,
    pub speed: f32,
    pub from_s: f32,
    pub once: bool,
    /// Held still once the clip reaches this time.
    pub hold_at: Option<f32>,
    pub blend: Duration,
}

impl Act {
    pub fn looping(clip: &str, speed: f32) -> Self {
        Self {
            clip: clip.to_string(),
            speed,
            from_s: 0.0,
            once: false,
            hold_at: None,
            blend: BLEND,
        }
    }

    pub fn once(clip: &str, from_s: f32, speed: f32) -> Self {
        Self {
            from_s,
            once: true,
            ..Self::looping(clip, speed)
        }
    }
}

impl Acting {
    /// Plays a clip, from its start even if it is playing already.
    pub fn play(&mut self, act: Act) {
        self.clip = Some(act);
        self.asked += 1;
    }

    /// The speed of the clip playing, as a hit-stop slows it, without starting it again.
    pub fn set_speed(&mut self, speed: f32) {
        if let Some(act) = &mut self.clip {
            act.speed = speed;
        }
    }
}

/// On a player who has been given a figure.
#[derive(Component)]
struct Figured;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load).add_systems(
        Update,
        (
            prepare.run_if(not(resource_exists::<Moves>)),
            stand.run_if(resource_exists::<Moves>),
            animate,
        )
            .chain(),
    );
}

fn load(mut commands: Commands, assets: Res<AssetServer>, mut loading: ResMut<Loading>) {
    let model = assets.load(crate::appearance::BASE);
    let clips = assets.load(CLIPS);
    loading.wait_for(&model);
    loading.wait_for(&clips);
    commands.insert_resource(FigureModel { model, clips });
}

/// Once the clips have loaded, makes the animations ready.
fn prepare(
    mut commands: Commands,
    model: Res<FigureModel>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<GltfNode>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if !assets.is_loaded_with_dependencies(&model.clips) {
        return;
    }
    let Some(gltf) = gltfs.get(&model.clips) else {
        return;
    };
    let mut graph = AnimationGraph::new();
    let mut clips = HashMap::new();
    for (name, clip) in &gltf.named_animations {
        let first = graph.add_clip(clip.clone(), 1.0, graph.root);
        let second = graph.add_clip(clip.clone(), 1.0, graph.root);
        clips.insert(name.to_string(), [first, second]);
    }
    commands.insert_resource(Moves {
        graph: graphs.add(graph),
        clips,
        paces: crate::pace::paces(gltf, &gltf_nodes).into_iter().collect(),
    });
}

/// Stands a figure on each player who has none yet.
fn stand(
    mut commands: Commands,
    model: Res<FigureModel>,
    gltfs: Res<Assets<Gltf>>,
    players: Query<Entity, (With<Player>, Without<Figured>)>,
) {
    let Some(scene) = gltfs
        .get(&model.model)
        .and_then(|gltf| gltf.default_scene.clone())
    else {
        return;
    };
    for player in &players {
        commands.entity(player).insert(Figured);
        commands
            .spawn((
                Name::new("Figure"),
                crate::character::Body,
                crate::appearance::Appearance::template(TEMPLATE).unwrap_or_default(),
                WorldAssetRoot(scene.clone()),
                // The model faces +Z; things here face -Z.
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                ChildOf(player),
            ))
            .observe(dress);
    }
}

/// When the figure's bones exist, starts it animating and gives it a hand to hold things in.
fn dress(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    moves: Res<Moves>,
    children: Query<&Children>,
    mut animators: Query<&mut AnimationPlayer>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
) {
    let Ok(player) = parents.get(ready.entity).map(ChildOf::parent) else {
        return;
    };
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
    if let Some(idle) = moves.clips.get(Doing::Idle.clip()) {
        transitions
            .play(&mut animator, idle[0], Duration::ZERO)
            .repeat();
    }
    // Whatever is in hand is held in the hand's socket (`held`).
    let hand = crate::character::find(ready.entity, crate::character::HAND_R, &children, &names);
    match hand {
        Some(hand) => {
            commands
                .entity(hand)
                .insert((Grip { player }, Carrying(true)));
        }
        None => warn!("The figure has no {}.", crate::character::HAND_R),
    }
    commands.entity(entity).insert((
        AnimationGraphHandle(moves.graph.clone()),
        transitions,
        Animated {
            player,
            hand,
            doing: Doing::Idle,
            playing: Some((Doing::Idle.clip().to_string(), 0)),
            applied: 0,
        },
    ));
}

/// Picks the animation that fits what each player is doing, or what a layer above asks,
/// and blends their figure into it.
fn animate(
    moves: Option<Res<Moves>>,
    players: Query<(&Motion, &Stance, Has<Downed>, Option<&Acting>), With<Player>>,
    mut figures: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut Animated,
        &GlobalTransform,
    )>,
    mut hands: Query<&mut Carrying, With<Grip>>,
) {
    let Some(moves) = moves else { return };
    for (mut animator, mut transitions, mut animated, place) in &mut figures {
        let Ok((motion, stance, downed, acting)) = players.get(animated.player) else {
            continue;
        };
        let act = acting
            .and_then(|acting| acting.clip.as_ref())
            .filter(|_| !downed);
        let doing = match act {
            Some(_) => Doing::Acting,
            None => doing(motion, stance, downed),
        };
        let asked = acting.map_or(0, |acting| acting.asked);
        if animated.doing != doing || (act.is_some() && animated.applied != asked) {
            animated.doing = doing;
            animated.applied = asked;
            let (clip, once, from_s, blend) = match act {
                Some(act) => (act.clip.as_str(), act.once, act.from_s, act.blend),
                None => (doing.clip(), doing.once(), 0.0, BLEND),
            };
            if let Some(mut hand) = animated.hand.and_then(|hand| hands.get_mut(hand).ok()) {
                hand.0 = CARRIED_IN.contains(&clip);
            }
            match moves.clips.get(clip) {
                Some(nodes) => {
                    // The same clip again blends from itself, by its other copy.
                    let copy = match &animated.playing {
                        Some((name, copy)) if name == clip => 1 - copy,
                        _ => 0,
                    };
                    let playing = transitions.play(&mut animator, nodes[copy], blend);
                    if !once {
                        playing.repeat();
                    }
                    if from_s > 0.0 {
                        playing.seek_to(from_s);
                    }
                    animated.playing = Some((clip.to_string(), copy));
                }
                None => warn!("The figure has no clip {clip}."),
            }
        }
        let Some((name, copy)) = animated.playing.clone() else {
            continue;
        };
        let Some(playing) = moves
            .clips
            .get(&name)
            .and_then(|nodes| animator.animation_mut(nodes[copy]))
        else {
            continue;
        };
        match act {
            // As asked, held where asked.
            Some(act) => {
                let held =
                    act.speed >= 0.0 && act.hold_at.is_some_and(|at| playing.seek_time() >= at);
                playing.set_speed(if held { 0.0 } else { act.speed });
            }
            // The jog and sprint at the speed the player moves, so their feet do not slide.
            None if matches!(doing, Doing::Jog | Doing::Sprint) => {
                if let Some(pace) = moves.paces.get(&name) {
                    let scale = place.scale().y;
                    playing.set_speed(crate::pace::rate(motion.velocity.length(), *pace, scale));
                }
            }
            None => {}
        }
    }
}

/// What a player is doing, as their figure shows it.
fn doing(motion: &Motion, stance: &Stance, downed: bool) -> Doing {
    let speed = motion.velocity.length();
    if downed {
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
    }
}
