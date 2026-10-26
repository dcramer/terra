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
pub use terra_motion::hold::Hand;
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

/// A socket in a figure's hand (`character::HAND_R`, the main hand, and `HAND_L`, the off hand;
/// every body is right-handed until D463's mirror), whose figure it is and which hand. What is
/// in hand is drawn as its children, by the layer that knows what it is (`held`).
#[derive(Component)]
pub struct Grip {
    pub player: Entity,
    pub hand: Hand,
}

/// On a figure's animation player: whose figure it is, its hand, what it is animated as
/// doing, the clip playing and which copy, and the last ask of `Acting` it played.
#[derive(Component)]
struct Animated {
    player: Entity,
    hands: Vec<Entity>,
    doing: Doing,
    playing: Option<(String, usize)>,
    applied: u32,
    /// Clips played mixed into another (`Act::mix`), each with the one it rides on.
    mixed: Vec<Mixed>,
    /// Where the player's body was shown last frame (`shown_velocity`).
    shown: Option<Vec3>,
}

/// A clip played in step with another, taking its share of that one's weight as the
/// transitions fade it in and out.
struct Mixed {
    main: AnimationNodeIndex,
    mix: AnimationNodeIndex,
    share: f32,
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
    /// Another clip played in step with it, and its share of the weight: a chained blow's
    /// held take, by how far out the blow it cut short carried its shield (D454).
    pub mix: Option<(String, f32)>,
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
            mix: None,
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
    app.add_systems(Startup, load)
        .add_systems(
            Update,
            (
                prepare.run_if(not(resource_exists::<Moves>)),
                stand.run_if(resource_exists::<Moves>),
                animate,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            mix.after(bevy::animation::transition::advance_transitions)
                .before(bevy::animation::animate_targets),
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
    mut baked: ResMut<crate::feet::Baked>,
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
    baked.register(gltf, &gltf_nodes);
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
    // Whatever is in each hand is held in that hand's socket (`held`).
    let mut hands = Vec::new();
    for (socket, hand) in [
        (crate::character::HAND_R, Hand::Main),
        (crate::character::HAND_L, Hand::Off),
    ] {
        match crate::character::find(ready.entity, socket, &children, &names) {
            Some(grip) => {
                commands
                    .entity(grip)
                    .insert((Grip { player, hand }, Carrying(true)));
                hands.push(grip);
            }
            None => warn!("The figure has no {socket}."),
        }
    }
    commands.entity(entity).insert((
        AnimationGraphHandle(moves.graph.clone()),
        transitions,
        Animated {
            player,
            hands,
            doing: Doing::Idle,
            playing: Some((Doing::Idle.clip().to_string(), 0)),
            applied: 0,
            mixed: Vec::new(),
            shown: None,
        },
    ));
}

/// Picks the animation that fits what each player is doing, or what a layer above asks,
/// and blends their figure into it.
fn animate(
    moves: Option<Res<Moves>>,
    time: Res<Time>,
    players: Query<(&Motion, &Transform, &Stance, Has<Downed>, Option<&Acting>), With<Player>>,
    mut figures: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut Animated,
        &GlobalTransform,
    )>,
    mut hands: Query<&mut Carrying, With<Grip>>,
    mut feet: Query<&mut crate::feet::Kept>,
) {
    let Some(moves) = moves else { return };
    for (mut animator, mut transitions, mut animated, place) in &mut figures {
        let Ok((motion, body, stance, downed, acting)) = players.get(animated.player) else {
            continue;
        };
        let at = body.translation;
        let velocity = shown_velocity(
            motion.velocity,
            animated.shown,
            at,
            *body.up(),
            time.delta_secs(),
        );
        animated.shown = Some(at);
        let act = acting
            .and_then(|acting| acting.clip.as_ref())
            .filter(|_| !downed);
        let doing = match act {
            Some(_) => Doing::Acting,
            None => doing(velocity.length(), motion.airborne, stance, downed),
        };
        let asked = acting.map_or(0, |acting| acting.asked);
        if animated.doing != doing || (act.is_some() && animated.applied != asked) {
            animated.doing = doing;
            animated.applied = asked;
            let (clip, once, from_s, blend) = match act {
                Some(act) => (act.clip.as_str(), act.once, act.from_s, act.blend),
                None => (doing.clip(), doing.once(), 0.0, BLEND),
            };
            // Both hands carry in a carried clip and use what they hold in any other.
            for &grip in &animated.hands {
                if let Ok(mut hand) = hands.get_mut(grip) {
                    hand.0 = CARRIED_IN.contains(&clip);
                }
            }
            match moves.clips.get(clip) {
                Some(nodes) => {
                    // The same clip again blends from itself, by its other copy.
                    let copy = match &animated.playing {
                        Some((name, copy)) if name == clip => 1 - copy,
                        _ => 0,
                    };
                    // The clip giving way fades from its whole weight, its mixed clip's share
                    // given back to it first.
                    if let Some(main) = transitions.get_main_animation() {
                        let given: f32 = animated
                            .mixed
                            .iter()
                            .filter(|m| m.main == main)
                            .filter_map(|m| animator.animation(m.mix).map(|a| a.weight()))
                            .sum();
                        if let Some(main) = animator.animation_mut(main) {
                            let whole = main.weight() + given;
                            main.set_weight(whole);
                        }
                    }
                    animated.mixed.retain(|m| m.mix != nodes[copy]);
                    let playing = transitions.play(&mut animator, nodes[copy], blend);
                    if !once {
                        playing.repeat();
                    }
                    if from_s > 0.0 {
                        playing.seek_to(from_s);
                    }
                    animated.playing = Some((clip.to_string(), copy));
                    if let Some((mix, share)) = act.and_then(|act| act.mix.as_ref())
                        && let Some(both) = moves.clips.get(mix)
                    {
                        let taken = |node| animated.mixed.iter().any(|m| m.mix == node);
                        let node = if taken(both[0]) { both[1] } else { both[0] };
                        animator
                            .start(node)
                            .seek_to(from_s)
                            .set_speed(act.map_or(1.0, |act| act.speed))
                            .set_weight(0.0);
                        animated.mixed.push(Mixed {
                            main: nodes[copy],
                            mix: node,
                            share: *share,
                        });
                    }
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
                let speed = if held { 0.0 } else { act.speed };
                playing.set_speed(speed);
                // A clip mixed into it keeps in step.
                let main = moves.clips[&name][copy];
                for m in animated.mixed.iter().filter(|m| m.main == main) {
                    if let Some(mixed) = animator.animation_mut(m.mix) {
                        mixed.set_speed(speed);
                    }
                }
            }
            // The jog and sprint at the speed the player moves, so their feet do not slide.
            None if matches!(doing, Doing::Jog | Doing::Sprint) => {
                if let Some(pace) = moves.paces.get(&name) {
                    let scale = place.scale().y;
                    playing.set_speed(crate::pace::rate(velocity.length(), *pace, scale));
                }
            }
            None => {}
        }
        // The ground a travelling clip's feet stand on passes under the body as fast as the
        // clip strides, the way the body goes: its planted feet are kept on it (`feet`).
        if let Ok(mut kept) = feet.get_mut(animated.player) {
            let pace = moves.paces.get(&name).copied().unwrap_or(0.0);
            let node = moves.clips[&name][copy];
            let rate = animator.animation(node).map_or(0.0, |a| a.speed());
            kept.stride(velocity, pace * place.scale().y, rate);
        }
    }
}

/// Each mixed clip takes its share of the weight the transitions gave the clip it rides on,
/// and stops with it.
fn mix(mut figures: Query<(&mut AnimationPlayer, &mut Animated)>) {
    for (mut animator, mut animated) in &mut figures {
        animated.mixed.retain(|m| {
            let Some(whole) = animator.animation(m.main).map(|a| a.weight()) else {
                animator.stop(m.mix);
                return false;
            };
            if let Some(main) = animator.animation_mut(m.main) {
                main.set_weight(whole * (1.0 - m.share));
            }
            match animator.animation_mut(m.mix) {
                Some(mixed) => {
                    mixed.set_weight(whole * m.share);
                    true
                }
                None => false,
            }
        });
    }
}

/// The velocity the figure strides at: the body's over the ground, cut to how fast the shown
/// body went over the ground since the last frame (`from` to `to`, level about `up`), so a
/// body its mover says moves but that stands still (held against a wall or a face too steep)
/// stands, and a frame's jump (a body put somewhere) reads no faster than the mover. Lyra's
/// `DisplacementSpeed`, the actor's displacement over the frame, beside its velocity.
fn shown_velocity(velocity: Vec3, from: Option<Vec3>, to: Vec3, up: Vec3, dt: f32) -> Vec3 {
    match from {
        Some(from) if dt > 0.0 => {
            let went = (to - from).reject_from(up).length() / dt;
            velocity.clamp_length_max(went)
        }
        _ => velocity,
    }
}

/// What a player is doing, as their figure shows it, at a speed over the ground.
fn doing(speed: f32, airborne: f32, stance: &Stance, downed: bool) -> Doing {
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
    } else if airborne > 0.12 {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A body its mover says jogs but that is held still is shown standing; one that goes as
    /// fast strides at its speed, a step's rise adds nothing, and a body put far away in a
    /// frame strides no faster than it moves.
    #[test]
    fn the_figure_strides_as_fast_as_the_shown_body_goes_and_no_faster() {
        let (dt, up, jog) = (1.0 / 60.0, Vec3::Y, Vec3::X * 4.3);
        let at = Vec3::new(3.0, 2000.0, 1.0);
        let shown = |to: Vec3| shown_velocity(jog, Some(at), to, up, dt);
        let held = shown(at);
        assert_eq!(
            doing(held.length(), 0.0, &Stance::default(), false),
            Doing::Idle
        );
        let going = shown(at + jog * dt);
        assert!((going - jog).length() < 1e-3, "{going}");
        assert_eq!(
            doing(going.length(), 0.0, &Stance::default(), false),
            Doing::Jog
        );
        let stepping = shown(at + jog * dt + up * 0.3);
        assert!((stepping - jog).length() < 1e-3, "{stepping}");
        let put = shown(at + Vec3::Z * 50.0);
        assert!((put - jog).length() < 1e-3, "{put}");
        let half = shown(at + jog * dt * 0.5);
        assert!((half - jog * 0.5).length() < 1e-3, "{half}");
        // The first frame, and a paused one, take the mover's word.
        assert_eq!(shown_velocity(jog, None, at, up, dt), jog);
        assert_eq!(shown_velocity(jog, Some(at), at, up, 0.0), jog);
    }

    #[test]
    fn a_mixed_clip_takes_its_share_of_its_mains_weight_and_stops_with_it() {
        let mut graph = AnimationGraph::new();
        let main = graph.add_clip(Handle::default(), 1.0, graph.root);
        let held = graph.add_clip(Handle::default(), 1.0, graph.root);
        let mut app = App::new();
        app.add_systems(Update, mix);
        let mut player = AnimationPlayer::default();
        // The transitions have given the main 0.8 (a clip giving way has the rest).
        player.start(main).set_weight(0.8);
        player.start(held);
        let figure = app
            .world_mut()
            .spawn((
                player,
                Animated {
                    player: Entity::PLACEHOLDER,
                    hands: Vec::new(),
                    doing: Doing::Acting,
                    playing: None,
                    applied: 0,
                    mixed: vec![Mixed {
                        main,
                        mix: held,
                        share: 0.25,
                    }],
                    shown: None,
                },
            ))
            .id();
        app.update();
        let weight = |app: &App, node| {
            let player = app.world().get::<AnimationPlayer>(figure).unwrap();
            player.animation(node).map(|a| a.weight())
        };
        assert!((weight(&app, main).unwrap() - 0.6).abs() < 1e-6);
        assert!((weight(&app, held).unwrap() - 0.2).abs() < 1e-6);
        // The main stopped (its fade done): the mixed clip stops and is forgotten.
        app.world_mut()
            .get_mut::<AnimationPlayer>(figure)
            .unwrap()
            .stop(main);
        app.update();
        assert_eq!(weight(&app, held), None);
        assert!(
            app.world()
                .get::<Animated>(figure)
                .unwrap()
                .mixed
                .is_empty()
        );
    }
}
