//! The player's body: the base body (D112, `appearance`), shaped as the player's template,
//! made with Tripo and rigged onto the skeleton of a free animation library (D115,
//! THIRD_PARTY.md), with what is in hand held in its right hand's socket (`character`).
//! Every player stands on one, and which animation plays follows what that player is doing.
//!
//! A base body has no hair or clothes of its own; those are parts still to come (D111,
//! D112).
//!
//! Its clips are the fight file's (`CLIPS`: the base's and the sword fight's, on the base's
//! skeleton), two of each so a clip played again blends from itself, and the same masked for
//! the hold layer (`layer`): two off the arms, for a clip played under a hold, and one on the
//! arms alone, for the hold. A layer above may ask for a clip in place of what walking shows
//! (`Acting`), with a stance's hold over it, as the sword fight does (`melee`).

use crate::character::{CARRIED_IN, CarryHold, CarryPosed, Carrying, Held};
use crate::layer::{Arms, Nodes};
use crate::{Downed, Motion, Player, Stance};
use bevy::{
    gltf::GltfNode, platform::collections::HashMap, prelude::*,
    world_serialization::WorldInstanceReady,
};
use std::time::Duration;
use terra_motion::feet::Markers;
pub use terra_motion::hold::Hand;
use terra_world::loading::Loading;

/// The template the player's body is shaped as (`appearance::Appearance::template`).
pub const TEMPLATE: &str = "male";
/// How long one animation takes to give way to the next.
const BLEND: Duration = Duration::from_millis(180);
/// How long a swim's clip takes to give way, wading into a swim and out, or a stroke to a
/// tread: long enough that the blend from an upright clip to a stroke made lying reads as the
/// body lying down in the water, as `swim::Pitch` eases its pitch.
const SWIM_BLEND: Duration = Duration::from_millis(400);
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
    /// Swimming (`swim`): treading water, a stroke at the surface or a dive.
    Swim(crate::swim::Swim),
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
            Doing::Swim(swim) => swim.clip(),
            Doing::Acting => "Fight_Guard",
        }
    }

    /// Whether it strokes through the water (not treading it).
    fn strokes(self) -> bool {
        matches!(
            self,
            Doing::Swim(crate::swim::Swim::Stroke | crate::swim::Swim::Dive)
        )
    }

    /// Whether it plays once and holds, rather than looping.
    fn once(self) -> bool {
        matches!(
            self,
            Doing::Use | Doing::Down | Doing::Attack | Doing::Flinch | Doing::Roll
        )
    }
}

/// The animations, as places in the figure's animation graph: every clip by name.
#[derive(Resource)]
pub struct Moves {
    graph: Handle<AnimationGraph>,
    clips: HashMap<String, Nodes>,
    /// The stride clips' own paces (`pace.rs`), so their feet do not slide.
    paces: HashMap<String, f32>,
    /// Every clip's foot-down markers (`feet::markers`), by which a stride loop taking over
    /// from another is put in step with it (`synced`).
    markers: HashMap<String, Markers>,
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
    /// The clip playing, which copy, and the arms holds are laid on over it.
    playing: Option<(String, usize, Arms)>,
    applied: u32,
    /// Clips played mixed into another (`Act::mix`), each with the one it rides on.
    mixed: Vec<Mixed>,
    /// Holds laid over the clips playing (`Act::hold`, `carry`), each with the clips under it.
    holds: Vec<crate::layer::Held>,
    /// The carry pose each arm (left, right) takes in a carried clip, as a hold on that arm
    /// alone: its hand's look's (`CarryHold`, D560).
    carry: Carry,
    /// Where the player's body was shown last frame (`shown_velocity`).
    shown: Option<Vec3>,
}

/// The hold each arm (left, right) takes in a carried clip, if any: a clip's node on that arm
/// alone (`Nodes::over_one`).
pub type Carry = [Option<AnimationNodeIndex>; 2];

/// The carry each arm of a figure whose hands are `hands` takes now: the carry pose of what
/// its hand holds (`CarryHold`), while that look is there. The main hand is the right until
/// D463's mirror, as `character::HAND_R` is.
fn carry_of(
    moves: &Moves,
    hands: &[Entity],
    grips: &Query<(&Grip, Option<&CarryHold>)>,
    looks: &Query<(), With<Held>>,
) -> Carry {
    let mut carry = [None; 2];
    for &hand in hands {
        let Ok((grip, pose)) = grips.get(hand) else {
            continue;
        };
        let side = match grip.hand {
            Hand::Off => 0,
            Hand::Main => 1,
        };
        if let Some(node) = carry_pose(&moves.clips, grip.hand, pose, looks) {
            carry[side] = Some(node);
        }
    }
    carry
}

/// The arms a carry lays holds on.
fn carry_arms(carry: &Carry) -> Arms {
    let side = |i: usize, arm| if carry[i].is_some() { arm } else { Arms::None };
    side(0, Arms::Left).and(side(1, Arms::Right))
}

/// The arms holds are laid on over `clip`: both under a stance's hold (`held`); in a carried
/// clip without one, each arm whose hand carries in a pose (D560); else none.
pub fn laid_arms(clip: &str, held: bool, carry: &Carry) -> Arms {
    if held {
        Arms::Both
    } else if CARRIED_IN.contains(&clip) {
        carry_arms(carry)
    } else {
        Arms::None
    }
}

/// The carry pose of what the hand socket `hand` (`Hand::Off` the left) holds, as a hold on
/// its arm alone in `clips`, while that look is there (`CarryHold`).
pub fn carry_pose(
    clips: &HashMap<String, Nodes>,
    hand: Hand,
    pose: Option<&CarryHold>,
    looks: &Query<(), With<Held>>,
) -> Option<AnimationNodeIndex> {
    let pose = pose.filter(|pose| looks.contains(pose.look))?;
    let arm = match hand {
        Hand::Off => Arms::Left,
        Hand::Main => Arms::Right,
    };
    clips.get(&pose.clip).and_then(|n| n.over(arm))
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
    /// A clip whose first pose the arms hold over this one, by the hold layer's mask
    /// (`layer`): a stance's hold, the guard's arms over a walk (D458).
    pub hold: Option<String>,
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
            hold: None,
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

/// On a player: the clip its figure shows (the one coming in), for the walk's record.
#[derive(Component, Default, Debug)]
pub struct Shows {
    pub clip: String,
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
                crate::swim::lay
                    .in_set(crate::pose::Layer::Swim)
                    .after(crate::stepping::show),
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
    let (graph, clips) = crate::layer::graph(gltf, &gltf_nodes);
    baked.register(gltf, &gltf_nodes);
    commands.insert_resource(Moves {
        graph: graphs.add(graph),
        clips,
        paces: crate::pace::paces(gltf, &gltf_nodes).into_iter().collect(),
        markers: crate::feet::markers(gltf, &gltf_nodes)
            .into_iter()
            .collect(),
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
        commands
            .entity(player)
            .insert((Figured, Shows::default(), crate::swim::Pitch::default()));
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
            .play(&mut animator, idle.whole[0], Duration::ZERO)
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
                commands.entity(grip).insert((
                    Grip { player, hand },
                    Carrying(true),
                    CarryPosed(false),
                ));
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
            playing: Some((Doing::Idle.clip().to_string(), 0, Arms::None)),
            applied: 0,
            mixed: Vec::new(),
            holds: Vec::new(),
            carry: [None; 2],
            shown: None,
        },
    ));
}

/// Picks the animation that fits what each player is doing, or what a layer above asks,
/// and blends their figure into it.
fn animate(
    moves: Option<Res<Moves>>,
    time: Res<Time>,
    mut players: Query<
        (
            &Motion,
            &Transform,
            &Stance,
            Has<Downed>,
            Option<&Acting>,
            Option<&mut crate::swim::Pitch>,
            Option<&mut Shows>,
        ),
        With<Player>,
    >,
    mut figures: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut Animated,
        &GlobalTransform,
        &Transform,
    )>,
    mut hands: Query<(&mut Carrying, &mut CarryPosed, &Grip)>,
    (grips, looks): (Query<(&Grip, Option<&CarryHold>)>, Query<(), With<Held>>),
    mut feet: Query<&mut crate::feet::Kept>,
) {
    let Some(moves) = moves else { return };
    for (mut animator, mut transitions, mut animated, place, own) in &mut figures {
        let Ok((motion, body, stance, downed, acting, pitch, shows)) =
            players.get_mut(animated.player)
        else {
            continue;
        };
        let swim = crate::swim::swim(motion, animated.doing.strokes());
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
            None => doing(velocity.length(), motion.airborne, stance, downed, swim),
        };
        if let Some(mut pitch) = pitch {
            pitch.asked = match doing {
                Doing::Swim(swim) => crate::swim::pitch(motion, Some(swim)),
                _ => 0.0,
            };
        }
        let asked = acting.map_or(0, |acting| acting.asked);
        // A carry pose taken up or put down (a torch lit or put away) shows at once in a
        // carried clip, played again with the hold; in any other, at the next clip.
        let carry = carry_of(&moves, &animated.hands, &grips, &looks);
        let recarry = animated.carry != carry
            && animated.playing.as_ref().is_some_and(|(name, _, arms)| {
                *arms != Arms::Both && CARRIED_IN.contains(&name.as_str())
            });
        animated.carry = carry;
        if animated.doing != doing || (act.is_some() && animated.applied != asked) || recarry {
            change(
                &moves,
                &mut animated,
                &mut animator,
                &mut transitions,
                &mut hands,
                (doing, act, asked),
            );
        }
        let Some((name, copy, arms)) = animated.playing.clone() else {
            continue;
        };
        if let Some(mut shows) = shows
            && shows.clip != name
        {
            shows.clip.clone_from(&name);
        }
        let main = moves.clips[&name].played(copy, arms);
        let Some(playing) = animator.animation_mut(main) else {
            continue;
        };
        let mut warp = 1.0;
        match act {
            // As asked, held where asked.
            Some(act) => {
                let held =
                    act.speed >= 0.0 && act.hold_at.is_some_and(|at| playing.seek_time() >= at);
                let speed = if held { 0.0 } else { act.speed };
                playing.set_speed(speed);
                // A clip mixed into it keeps in step.
                for m in animated.mixed.iter().filter(|m| m.main == main) {
                    if let Some(mixed) = animator.animation_mut(m.mix) {
                        mixed.set_speed(speed);
                    }
                }
            }
            // The jog and sprint at the player's speed, played and warped (`pace::gait`).
            None if matches!(doing, Doing::Jog | Doing::Sprint) => {
                if let Some(pace) = moves.paces.get(&name) {
                    let gait = stride(velocity.length(), *pace, place, own);
                    playing.set_speed(gait.rate);
                    warp = gait.warp;
                }
            }
            None => {}
        }
        // The ground a travelling clip's feet stand on passes under the body as fast as the
        // clip strides, the way the body goes: its planted feet are kept on it (`feet`).
        if let Ok(mut kept) = feet.get_mut(animated.player) {
            let pace = moves.paces.get(&name).copied().unwrap_or(0.0);
            let rate = animator.animation(main).map_or(0.0, |a| a.speed());
            kept.stride(velocity, pace * crate::pace::size(place, own), rate, warp);
        }
    }
}

/// How a jog or sprint of `pace` is shown at `speed` on a figure whose animation player is at
/// `place` with its own node's transform `own`: scaled by the figure's size against its model
/// alone (`pace::size`). By the player's world scale, which carries base.glb's armature scale
/// (1.078) again, every stride played 7% slow and its feet slid.
fn stride(speed: f32, pace: f32, place: &GlobalTransform, own: &Transform) -> crate::pace::Gait {
    crate::pace::gait(speed, pace, crate::pace::size(place, own))
}

/// The clip a figure's doing or a layer's ask now shows, blended in from what played, with a
/// hold laid over it and a clip mixed into it as asked.
fn change(
    moves: &Moves,
    animated: &mut Animated,
    animator: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    hands: &mut Query<(&mut Carrying, &mut CarryPosed, &Grip)>,
    (doing, act, asked): (Doing, Option<&Act>, u32),
) {
    let swims = matches!(doing, Doing::Swim(_)) || matches!(animated.doing, Doing::Swim(_));
    animated.doing = doing;
    animated.applied = asked;
    let (clip, once, from_s, blend) = match act {
        Some(act) => (act.clip.as_str(), act.once, act.from_s, act.blend),
        None if swims => (doing.clip(), doing.once(), 0.0, SWIM_BLEND),
        None => (doing.clip(), doing.once(), 0.0, BLEND),
    };
    let hold = act
        .and_then(|act| act.hold.as_ref())
        .and_then(|hold| moves.clips.get(hold));
    // Both hands carry in a carried clip and use what they hold in any other; an arm whose hand
    // carries in a pose, with no stance's hold over it, is held so in the body (`carry_arm`).
    let carried = CARRIED_IN.contains(&clip);
    for &grip in &animated.hands {
        if let Ok((mut hand, mut posed, grip)) = hands.get_mut(grip) {
            hand.0 = carried;
            let side = match grip.hand {
                Hand::Off => 0,
                Hand::Main => 1,
            };
            posed.0 = carried && hold.is_none() && animated.carry[side].is_some();
        }
    }
    // A stride loop taking over from another goes on at the same step (sync groups).
    let synced = act
        .is_none()
        .then(|| {
            let (from, ..) = animated.playing.as_ref()?;
            let t = animator
                .animation(transitions.get_main_animation()?)?
                .seek_time();
            synced(&moves.markers, &moves.paces, from, t, clip)
        })
        .flatten();
    match moves.clips.get(clip) {
        Some(nodes) => {
            // The same clip again blends from itself, by its other copy.
            let copy = match &animated.playing {
                Some((name, copy, _)) if name == clip => 1 - copy,
                _ => 0,
            };
            // A stance's hold takes both arms; in a carried clip without one, each arm whose
            // hand carries in a pose takes it (D560).
            let arms = laid_arms(clip, hold.is_some(), &animated.carry);
            let node = nodes.played(copy, arms);
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
            animated.mixed.retain(|m| m.mix != node);
            let playing = transitions.play(animator, node, blend);
            if !once {
                playing.repeat();
            }
            if from_s > 0.0 {
                playing.seek_to(from_s);
            }
            if let Some(at) = synced {
                playing.seek_to(at);
            }
            animated.playing = Some((clip.to_string(), copy, arms));
            if let Some(hold) = hold {
                crate::layer::lay(&mut animated.holds, animator, hold.over, node);
            } else if arms != Arms::None {
                let overs: Vec<_> = animated.carry.iter().flatten().copied().collect();
                crate::layer::lay_all(&mut animated.holds, animator, &overs, node);
            }
            if let Some((mix, share)) = act.and_then(|act| act.mix.as_ref())
                && let Some(both) = moves.clips.get(mix)
            {
                let taken = |node| animated.mixed.iter().any(|m| m.mix == node);
                let node = if taken(both.whole[0]) {
                    both.whole[1]
                } else {
                    both.whole[0]
                };
                animator
                    .start(node)
                    .seek_to(from_s)
                    .set_speed(act.map_or(1.0, |act| act.speed))
                    .set_weight(0.0);
                animated.mixed.push(Mixed {
                    main: nodes.played(copy, arms),
                    mix: node,
                    share: *share,
                });
            }
        }
        None => warn!("The figure has no clip {clip}."),
    }
}

/// Where stride loop `to` starts when it takes over from stride loop `from`, `t` seconds in:
/// at the same step by their foot-down markers (`Markers::synced`), as Unreal's sync groups
/// keep a group's clips on one marker phase. Restarted at its first instant, as every switch
/// was, the foot coming down stood still in the body's frame through the fade, read as a
/// swing by the lock, and slid with the body (sprint 29.6 cm on the yard film). None, so it
/// starts as any clip does, unless both are stride loops with markers.
fn synced(
    markers: &HashMap<String, Markers>,
    paces: &HashMap<String, f32>,
    from: &str,
    t: f32,
    to: &str,
) -> Option<f32> {
    if !paces.contains_key(from) || !paces.contains_key(to) {
        return None;
    }
    markers.get(to)?.synced(markers.get(from)?, t)
}

/// Each mixed clip takes its share of the weight the transitions gave the clip it rides on,
/// and stops with it; each hold takes the weight of the clips under it (`layer::ride`).
fn mix(mut figures: Query<(&mut AnimationPlayer, &mut Animated)>) {
    for (mut animator, mut animated) in &mut figures {
        crate::layer::ride(&mut animated.holds, &mut animator);
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
fn doing(
    speed: f32,
    airborne: f32,
    stance: &Stance,
    downed: bool,
    swim: Option<crate::swim::Swim>,
) -> Doing {
    if downed {
        Doing::Down
    } else if let Some(swim) = swim {
        Doing::Swim(swim)
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
    } else if speed > crate::movement::SPRINT_FROM {
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

    /// A stance's hold takes both arms over any clip; a carry pose takes only its own arm, and
    /// only in a carried clip (an idle, a jog), never in a blow; with nothing carried in a pose
    /// the clip has the arms.
    #[test]
    fn a_carry_pose_takes_its_own_arm_in_a_carried_clip_alone() {
        let torch: Carry = [None, Some(AnimationNodeIndex::new(7))];
        let both: Carry = [Some(AnimationNodeIndex::new(6)), torch[1]];
        assert_eq!(laid_arms("Jog_Fwd_Loop", false, &torch), Arms::Right);
        assert_eq!(laid_arms("Idle_Loop", false, &both), Arms::Both);
        assert_eq!(laid_arms("Fight_Slash", false, &torch), Arms::None);
        assert_eq!(laid_arms("Walk_Loop", true, &torch), Arms::Both);
        assert_eq!(laid_arms("Idle_Loop", false, &[None, None]), Arms::None);
    }

    /// A switch between the jog and the sprint (either way) goes on at the same step, on the
    /// figure's own clips: the foot just planted in the one giving way is just planted in the
    /// one taking over, as far on to the next plant. Restarted at 0 (as every switch was), the
    /// sprint came in with the other foot down and the planted one slid (sprint 29.6 cm).
    #[test]
    fn a_switch_between_gaits_keeps_the_planted_foots_phase() {
        let json = crate::model_files::json(CLIPS);
        let markers: HashMap<String, Markers> =
            serde_json::from_value(crate::model_files::extra(&json, "markers").unwrap()).unwrap();
        let paces: HashMap<String, f32> =
            serde_json::from_value(crate::model_files::extra(&json, "paces").unwrap()).unwrap();
        // Every pair of gaits, the walk too (D585's one stride clock): the clock is each
        // clip's plants (`Markers::synced`), so one foot is down in both.
        for (from, to) in [
            ("Jog_Fwd_Loop", "Sprint_Loop"),
            ("Sprint_Loop", "Jog_Fwd_Loop"),
            ("Walk_Loop", "Jog_Fwd_Loop"),
            ("Jog_Fwd_Loop", "Walk_Loop"),
            ("Walk_Loop", "Sprint_Loop"),
            ("Sprint_Loop", "Walk_Loop"),
        ] {
            let (was, now) = (&markers[from], &markers[to]);
            // Each plant of the one giving way, just after it comes down (not a foot still
            // down over the loop's seam, a span at 0 beside one that wraps round its end).
            let downs = [&was.foot_down_l, &was.foot_down_r];
            for (side, spans) in downs.into_iter().enumerate() {
                let wraps = spans.iter().any(|span| span[1] < span[0]);
                for span in spans.iter().filter(|span| !(wraps && span[0] <= 1e-4)) {
                    let t = span[0] + 0.02;
                    let at = synced(&markers, &paces, from, t, to).unwrap();
                    assert!(
                        now.lock(side, at) >= terra_motion::feet::FULL,
                        "{from} {t:.3} -> {to} {at:.3}: foot {side} is not planted"
                    );
                    let (a, b) = (was.phase_in_loop(t), now.phase_in_loop(at));
                    let ((sa, pa), (sb, pb)) = (a.unwrap(), b.unwrap());
                    assert!(sa == sb && (pa - pb).abs() < 1e-3, "{a:?} -> {b:?}");
                }
            }
        }
        // Into or out of a clip that is no stride loop, a clip starts at its start.
        assert_eq!(
            synced(&markers, &paces, "Idle_Loop", 0.3, "Jog_Fwd_Loop"),
            None
        );
    }

    /// A body its mover says jogs but that is held still is shown standing; one that goes as
    /// fast strides at its speed, a step's rise adds nothing, and a body put far away in a
    /// frame strides no faster than it moves.
    #[test]
    fn the_figure_strides_as_fast_as_the_shown_body_goes_and_no_faster() {
        let (dt, up, jog) = (1.0 / 60.0, Vec3::Y, Vec3::X * crate::movement::JOG_SPEED);
        let at = Vec3::new(3.0, 2000.0, 1.0);
        let shown = |to: Vec3| shown_velocity(jog, Some(at), to, up, dt);
        let held = shown(at);
        assert_eq!(
            doing(held.length(), 0.0, &Stance::default(), false, None),
            Doing::Idle
        );
        let going = shown(at + jog * dt);
        assert!((going - jog).length() < 1e-3, "{going}");
        assert_eq!(
            doing(going.length(), 0.0, &Stance::default(), false, None),
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
                    holds: Vec::new(),
                    carry: [None; 2],
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

    /// A stride's pace is metres a second in its model file's own world, through its armature's
    /// scale (`tools/characters/pace.py`), and the figure's animation player sits on that
    /// armature's node, whose world scale carries it again: a figure at its model's size going
    /// at its jog's or sprint's own pace plays it at its own speed, unwarped, and one twice the
    /// size at twice the speed the same. By the player's world scale, base.glb's 1.078 armature
    /// played them at 0.93.
    #[test]
    fn a_figure_going_at_its_strides_pace_plays_it_at_its_own_speed_whatever_its_rigs_scale() {
        use crate::model_files::{json, pace, root_scale};
        let rig = root_scale(&json(crate::appearance::BASE));
        assert!(rig > 1.05, "the case this covers: an armature scaled {rig}");
        let clips = json(CLIPS);
        for clip in [Doing::Jog.clip(), Doing::Sprint.clip()] {
            let pace = pace(&clips, clip);
            for size in [1.0, 2.0] {
                // The figure turned to face -Z and sized, its model's armature under it.
                let own = Transform::from_scale(Vec3::splat(rig));
                let figure = Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                    .with_scale(Vec3::splat(size));
                let place = GlobalTransform::from(figure) * own;
                let shown = stride(size * pace, pace, &place, &own);
                assert!(
                    (shown.rate - 1.0).abs() < 0.02 && (shown.warp - 1.0).abs() < 0.02,
                    "{clip} at {} m/s, {size} times its model's size: {shown:?}",
                    size * pace
                );
            }
        }
    }
}
