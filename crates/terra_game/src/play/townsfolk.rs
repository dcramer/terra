//! A town's people drawn (D324): each on the base body, shaped and dressed by their role's
//! look (`assets/data/townsfolk.ron`) from their seed, standing where `terra_life` has them,
//! playing the clip of what they are doing, a walk or a run at the speed they go so their
//! feet keep to the ground (`pace.rs`). Indoors, out at the fields or beyond every viewer,
//! nobody is drawn.

use super::bodies::Body;
use super::tick::{Sim, Smoothed};
use super::town::Market;
use crate::planet::{MainCamera, Planet};
use crate::{appearance, character};
use bevy::{
    platform::collections::HashMap,
    prelude::*,
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};
use serde::Deserialize;
use std::time::Duration;
use terra_build::town::ThingKind;
use terra_life::{Detail, Shows};

/// How someone's clips blend into each other.
const BLEND: Duration = Duration::from_millis(300);
/// The slowest a walk is played against its pace: slower, it is a shuffle, not a slide.
const SLOWEST_RATE: f32 = 0.2;
const FASTEST_RATE: f32 = 1.6;
/// Where the looks are said.
const LOOKS: &str = "townsfolk.ron";
/// How far above a seat's top the hips rest, as `people` seats a sitter.
const HIPS_ABOVE_SEAT_M: f32 = 0.08;
/// A bench's seat, where someone sits on it.
const LOG_TOP_M: f32 = 0.46;

/// A role's look: the bodies it is shaped as, and what it wears; on a woman's body (the
/// `female` template), what `women` says, where it says anything.
#[derive(Deserialize, Clone, Debug)]
struct Look {
    bodies: Vec<String>,
    wears: Vec<Garment>,
    #[serde(default)]
    women: Vec<Garment>,
}

/// Something worn, by item id, its main material's role, and the colours it is dyed; how
/// likely someone of the role is to wear it, and the group of things only one of which is
/// worn (a coat or a shirt), each by their seed.
#[derive(Deserialize, Clone, Debug)]
struct Garment {
    item: String,
    dye: String,
    colours: Vec<(f32, f32, f32)>,
    #[serde(default = "always")]
    chance: f32,
    #[serde(default)]
    group: Option<String>,
}

fn always() -> f32 {
    1.0
}

/// What someone wears of a list, by their seed: each garment by its chance, and of each
/// group the one their seed draws.
fn worn(list: &[Garment], seed: u32) -> Vec<(usize, &Garment)> {
    list.iter()
        .enumerate()
        .filter(|&(n, garment)| {
            if draw(seed, 40 + n as u32) >= garment.chance {
                return false;
            }
            let Some(group) = &garment.group else {
                return true;
            };
            let members: Vec<usize> = (0..list.len())
                .filter(|&m| list[m].group.as_ref() == Some(group))
                .collect();
            let pick = (draw(seed, 60) * members.len() as f32) as usize;
            members.get(pick.min(members.len() - 1)) == Some(&n)
        })
        .collect()
}

#[derive(Deserialize, Clone, Debug)]
struct Looks {
    mix: f32,
    roles: std::collections::BTreeMap<String, Look>,
    /// What is carried in the right hand for an activity, on the way to it and while doing
    /// it, by held item (`models/held/<item>.glb`): the smith's hammer, the guard's spear.
    #[serde(default)]
    holds: std::collections::BTreeMap<String, String>,
}

/// The held items by activity, as the looks say them.
#[derive(Resource, Default)]
struct Holds(std::collections::BTreeMap<String, String>);

/// A body's right hand, once its model is in, and what it holds now.
#[derive(Component)]
struct Hand {
    socket: Entity,
    showing: Option<String>,
}

/// A held item's look in someone's hand.
#[derive(Component)]
struct InHand;

/// One of the town's people: who in the town's life they are.
#[derive(Component)]
#[require(Smoothed)]
pub struct Folk {
    pub index: usize,
}

/// The body and its clips, once loaded.
#[derive(Resource)]
struct Figure {
    model: Handle<Gltf>,
    ready: Option<Ready>,
}

#[derive(Clone)]
struct Ready {
    scene: Handle<WorldAsset>,
    graph: Handle<AnimationGraph>,
    /// A graph of nothing, for a body whose pose is not worked out this frame.
    rest: Handle<AnimationGraph>,
    clips: Clips,
}

/// The clips the town's people play, by name, and the moving ones' paces.
#[derive(Clone)]
struct Clips {
    named: HashMap<String, AnimationNodeIndex>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    jog: AnimationNodeIndex,
    walk_pace: f32,
    jog_pace: f32,
}

/// On a body's animation player: whose it is, what it plays, and how many frames apart its
/// pose is worked out.
#[derive(Component)]
struct Plays {
    folk: Entity,
    playing: Option<AnimationNodeIndex>,
    every: u32,
}

/// How often a body's pose is worked out, by how far it is from the eye, as Unreal's
/// animation budget allocator lowers a far character's rate (D324): every frame within the
/// first distance, then every so many frames, its clip played that much faster when it is.
const TIERS: [(f32, u32); 3] = [(25.0, 1), (50.0, 2), (80.0, 3)];
const FAR_EVERY: u32 = 6;

fn every(distance: f32) -> u32 {
    TIERS
        .iter()
        .find(|(within, _)| distance <= *within)
        .map_or(FAR_EVERY, |(_, every)| *every)
}

/// How far off a body is drawn at all: past it, a person is a few pixels high, and the
/// town's smoke and murmur carry it (docs/demos.md's tiers).
const DRAWN_M: f32 = 150.0;

/// Within how far of the eye a body casts a shadow: past it, its shadow is lost among the
/// buildings' and costs the GPU every shadow map it would be drawn into.
const SHADOWS_M: f32 = 30.0;

/// Whether a body casts its shadow now, as last set on its meshes.
#[derive(Component, Default)]
struct Casting(Option<bool>);

/// Where a sitter sits and their hips, found once they sit.
#[derive(Component, Clone, Copy)]
struct Sat {
    top: Vec3,
    hips: Entity,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Gait>()
        .add_systems(Startup, load)
        .add_systems(
            Update,
            (
                prepare,
                spawn.run_if(resource_exists::<Market>),
                animate,
                tell_gait,
                hold.run_if(resource_exists::<Market>),
                seat,
                shade,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            skip.run_if(resource_exists::<Market>)
                .before(bevy::app::AnimationSystems),
        )
        .add_systems(
            FixedUpdate,
            place
                .run_if(resource_exists::<Market>)
                .in_set(Sim::World)
                .after(super::town::Live),
        );
}

fn load(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Figure {
        model: assets.load(appearance::BASE),
        ready: None,
    });
}

/// The body's scene and every clip the plan's activities name, in one graph.
fn prepare(
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<bevy::gltf::GltfNode>>,
    market: Option<Res<Market>>,
    mut figure: ResMut<Figure>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let Some(market) = market else {
        return;
    };
    if figure.ready.is_some() || !assets.is_loaded_with_dependencies(&figure.model) {
        return;
    }
    let Some(gltf) = gltfs.get(&figure.model) else {
        return;
    };
    let Some(scene) = gltf.default_scene.clone() else {
        return;
    };
    let mut names: Vec<String> = vec![
        "Idle_Loop".into(),
        "Walk_Loop".into(),
        "Jog_Fwd_Loop".into(),
    ];
    for activity in market.life.plan.activities.values() {
        if let Some(clip) = &activity.clip
            && !names.contains(clip)
        {
            names.push(clip.clone());
        }
    }
    names.retain(|name| {
        let there = gltf.named_animations.contains_key(name.as_str());
        if !there {
            warn!("Townsfolk: the body has no clip {name}");
        }
        there
    });
    let (graph, nodes) = AnimationGraph::from_clips(
        names
            .iter()
            .map(|name| gltf.named_animations[name.as_str()].clone()),
    );
    let named: HashMap<String, AnimationNodeIndex> =
        names.iter().cloned().zip(nodes.iter().copied()).collect();
    let paces = crate::pace::paces(gltf, &gltf_nodes);
    let pace = |name: &str| paces.get(name).copied().unwrap_or(1.0);
    figure.ready = Some(Ready {
        scene,
        graph: graphs.add(graph),
        rest: graphs.add(AnimationGraph::new()),
        clips: Clips {
            idle: named["Idle_Loop"],
            walk: named["Walk_Loop"],
            jog: named["Jog_Fwd_Loop"],
            walk_pace: pace("Walk_Loop"),
            jog_pace: pace("Jog_Fwd_Loop"),
            named,
        },
    });
}

/// A repeatable draw from a seed and a number, 0 to 1.
fn draw(seed: u32, salt: u32) -> f32 {
    let x = terra_gen::hash::lowbias32(seed ^ salt.wrapping_mul(0x9e37_79b9));
    (x >> 8) as f32 / (1u32 << 24) as f32
}

fn pick<T>(choices: &[T], roll: f32) -> Option<&T> {
    choices.get(((roll * choices.len() as f32) as usize).min(choices.len().saturating_sub(1)))
}

/// Everyone in the town as a body, once: shaped and dressed by their role's look, hidden
/// until their life puts them out of doors.
fn spawn(mut commands: Commands, figure: Res<Figure>, market: Res<Market>, mut done: Local<bool>) {
    if *done {
        return;
    }
    let Some(ready) = figure.ready.clone() else {
        return;
    };
    *done = true;
    let looks: Looks = match std::fs::read_to_string(terra_world::data::path(LOOKS))
        .map_err(|why| why.to_string())
        .and_then(|text| ron::from_str(&text).map_err(|why| why.to_string()))
    {
        Ok(looks) => looks,
        Err(why) => {
            error!("Townsfolk: {LOOKS}: {why}");
            return;
        }
    };
    commands.insert_resource(Holds(looks.holds.clone()));
    let templates = appearance::templates();
    for (index, person) in market.life.people.iter().enumerate() {
        let role = market.life.role(index);
        let Some(look) = looks
            .roles
            .get(role)
            .or_else(|| looks.roles.get("townsfolk"))
        else {
            continue;
        };
        let seed = person.seed;
        // A body of the role's, with a little of another's.
        let first = pick(&look.bodies, draw(seed, 1))
            .cloned()
            .unwrap_or_default();
        let second = pick(&look.bodies, draw(seed, 2))
            .cloned()
            .unwrap_or_default();
        let mut shape = templates.get(&first).cloned().unwrap_or_default();
        if let Some(other) = templates.get(&second) {
            let share = draw(seed, 3) * looks.mix;
            for (morph, weight) in &other.morphs {
                if !morph.starts_with("female") || shape.weight("female") > 0.0 {
                    shape.morphs.push((morph.clone(), weight * share));
                }
            }
        }
        shape.height = person.scale;
        let list = if first == "female" && !look.women.is_empty() {
            &look.women
        } else {
            &look.wears
        };
        let wears: Vec<character::Clothing> = worn(list, seed)
            .into_iter()
            .map(|(n, garment)| {
                let (r, g, b) = pick(&garment.colours, draw(seed, 10 + n as u32))
                    .copied()
                    .unwrap_or((0.4, 0.35, 0.3));
                // Each a shade of its own, as no two lengths of cloth dye alike.
                let shade = 0.88 + draw(seed, 20 + n as u32) * 0.24;
                character::Clothing {
                    item: garment.item.clone(),
                    tint: character::Tint(vec![(
                        garment.dye.clone(),
                        Color::srgb(r * shade, g * shade, b * shade),
                    )]),
                }
            })
            .collect();
        let folk = commands
            .spawn((
                Name::new(format!("Townsfolk {index}, {role}")),
                Folk { index },
                Casting::default(),
                Transform::from_translation(market.middle),
                Visibility::Hidden,
                character::Outfit(wears),
            ))
            .id();
        let graph = ready.graph.clone();
        commands
            .spawn((
                character::Body,
                shape,
                WorldAssetRoot(ready.scene.clone()),
                // The model faces +Z; things here face -Z.
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                ChildOf(folk),
            ))
            .observe(
                move |ready: On<WorldInstanceReady>,
                      mut commands: Commands,
                      children: Query<&Children>,
                      names: Query<&Name>,
                      players: Query<(), With<AnimationPlayer>>| {
                    if let Some(socket) =
                        character::find(ready.entity, character::HAND_R, &children, &names)
                    {
                        commands.entity(folk).insert(Hand {
                            socket,
                            showing: None,
                        });
                    }
                    for child in children.iter_descendants(ready.entity) {
                        if players.contains(child) {
                            commands.entity(child).insert((
                                AnimationGraphHandle(graph.clone()),
                                AnimationTransitions::new(),
                                Plays {
                                    folk,
                                    playing: None,
                                    every: 1,
                                },
                            ));
                        }
                    }
                },
            );
    }
    info!(
        "Townsfolk: {} people dressed by role.",
        market.life.people.len()
    );
}

/// Each body where its life has it this tick, on the ground and facing its way, shown only
/// out of doors and near a viewer.
fn place(
    mut commands: Commands,
    planet: Res<Planet>,
    market: Res<Market>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut folk: Query<(Entity, &Folk, &mut Transform, &mut Visibility)>,
    rising: Option<Res<super::rising::Rising>>,
    decks: Option<Res<super::scaffold::Decks>>,
) {
    let eye = camera.single().ok().map(|c| market.local(c.translation()));
    for (entity, one, mut at, mut shown) in &mut folk {
        let Some(person) = market.life.people.get(one.index) else {
            continue;
        };
        let beyond = eye.is_some_and(|eye| eye.distance(person.at) > DRAWN_M);
        let hidden = matches!(market.life.shows(one.index), Shows::Hidden)
            || person.detail == Detail::Far
            // In a settlement rising, nobody but its builders until their home is up.
            || rising.as_ref().is_some_and(|r| !r.about(&market, one.index))
            || beyond;
        let wanted = if hidden {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *shown != wanted {
            *shown = wanted;
            // Out of doors someone takes up room, so a player stops at them rather than
            // walking through (`bodies`); indoors they leave none at the door.
            if hidden {
                commands.entity(entity).remove::<Body>();
            } else {
                commands.entity(entity).insert(Body::fixed(person.radius));
            }
        }
        if hidden {
            continue;
        }
        let feet = market.ground(&planet, person.at);
        let up = feet.normalize();
        let facing = market
            .way(person.facing)
            .reject_from(up)
            .normalize_or(market.east);
        let (feet, facing) = decks
            .as_deref()
            .and_then(|decks| on_the_boards(&market, decks, one.index, feet))
            .unwrap_or((feet, facing));
        *at = Transform::from_translation(feet).looking_to(facing, up);
    }
}

/// Where a builder at work on a scaffolded building stands: on its top lift's boards facing
/// its wall, as builders lay the courses over their heads from the scaffold (D381), two in
/// three of them; the rest work from the ground, at the logs.
fn on_the_boards(
    market: &Market,
    decks: &super::scaffold::Decks,
    index: usize,
    feet: Vec3,
) -> Option<(Vec3, Vec3)> {
    let person = market.life.people.get(index)?;
    if !matches!(person.state, terra_life::State::Doing { .. }) || person.seed % 3 == 0 {
        return None;
    }
    let thing = &market.life.town.things[market.life.spots.get(person.spot?)?.thing];
    if thing.kind != ThingKind::Site {
        return None;
    }
    let (frame, walls, lifts) = decks.0.get(&thing.building?)?;
    let local = frame.compute_affine().inverse().transform_point3(feet);
    let (on, toward) = super::scaffold::on_deck(walls, *lifts, Vec2::new(local.x, local.z))?;
    Some((
        frame.transform_point(on),
        frame.rotation * Vec3::new(toward.x, 0.0, toward.y),
    ))
}

/// Each body playing what its person does: a walk or a run at their speed, else their
/// activity's clip.
fn animate(
    mut gait: ResMut<Gait>,
    figure: Res<Figure>,
    market: Option<Res<Market>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    folk: Query<(&Folk, &GlobalTransform)>,
    rising: Option<Res<super::rising::Rising>>,
    mut players: Query<(&mut Plays, &mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    // A time-lapse's people live faster than the film, their clips with them (D381).
    let lived = rising.as_ref().map_or(1.0, |r| r.lapse.pace);
    let (Some(market), Some(ready)) = (market, &figure.ready) else {
        return;
    };
    let eye = camera
        .single()
        .map(|c| c.translation())
        .unwrap_or(market.middle);
    let clips = &ready.clips;
    for (mut plays, mut player, mut transitions) in &mut players {
        let Ok((one, place)) = folk.get(plays.folk) else {
            continue;
        };
        plays.every = every(place.translation().distance(eye));
        // The body's own height against the base's, which its clips' paces are measured on:
        // a child's stride is two thirds of a grown body's, so its legs turn faster for the
        // same speed. (The body is drawn at its height by its shape, not its transform.)
        let scale = market.life.people[one.index].scale.max(0.1);
        let (wanted, rate, pace) = match market.life.shows(one.index) {
            Shows::Hidden => continue,
            Shows::Walk(speed) if speed > 0.05 => (
                clips.walk,
                speed / (clips.walk_pace * scale),
                Some((speed, clips.walk_pace * scale, false)),
            ),
            Shows::Run(speed) if speed > 0.05 => (
                clips.jog,
                speed / (clips.jog_pace * scale),
                Some((speed, clips.jog_pace * scale, true)),
            ),
            Shows::Walk(_) | Shows::Run(_) => (clips.idle, 1.0, None),
            Shows::Clip(name) => (
                clips.named.get(name).copied().unwrap_or(clips.idle),
                1.0,
                None,
            ),
        };
        // How fast the feet move against the ground, as played: 1 when they keep to it.
        if let Some((speed, pace, run)) = pace
            && place.translation().distance(eye) < GAIT_SEEN_M
        {
            let played = rate.clamp(SLOWEST_RATE, FASTEST_RATE);
            gait.feet.push(pace * played / speed);
            gait.running += usize::from(run);
        }
        if plays.playing != Some(wanted) {
            plays.playing = Some(wanted);
            transitions.play(&mut player, wanted, BLEND).repeat();
            // Not all in step: each from its own point in the clip.
            if let Some(active) = player.animation_mut(wanted) {
                active.seek_to(draw(one.index as u32, 99) * 2.0);
            }
        }
        let every = plays.every as f32;
        if let Some(active) = player.animation_mut(wanted) {
            active.set_speed(rate.clamp(SLOWEST_RATE, FASTEST_RATE) * every * lived);
        }
    }
}

/// What each person carries in hand for what they are doing or going to do, swapped as
/// that changes, as a player's figure shows what is in hand (`held`).
fn hold(
    mut commands: Commands,
    market: Res<Market>,
    holds: Option<Res<Holds>>,
    assets: Res<AssetServer>,
    children: Query<&Children>,
    in_hand: Query<(), With<InHand>>,
    mut folk: Query<(&Folk, &mut Hand)>,
) {
    let Some(holds) = holds else {
        return;
    };
    for (one, mut hand) in &mut folk {
        let wanted = market
            .life
            .activity(one.index)
            .and_then(|activity| holds.0.get(activity));
        if hand.showing.as_ref() == wanted {
            continue;
        }
        hand.showing = wanted.cloned();
        for look in children
            .iter_descendants(hand.socket)
            .filter(|&each| in_hand.contains(each))
        {
            commands.entity(look).despawn();
        }
        if let Some(item) = wanted {
            let file = character::held_look(item);
            let look = character::hold(
                &mut commands,
                assets.load(bevy::gltf::GltfAssetLabel::Scene(0).from_asset(file)),
                hand.socket,
            );
            commands.entity(look).insert(InHand);
        }
    }
}

/// Within how far of the eye walkers' feet are measured against the ground.
const GAIT_SEEN_M: f32 = 40.0;

/// How the town's walkers' feet keep to the ground near the eye, gathered as they are
/// animated and said every ten seconds: each walker's feet's speed as played against their
/// speed over the ground (1 keeps to it, more is a quick shuffle that reads as running, less
/// a glide), and how many of them are running.
#[derive(Resource, Default)]
struct Gait {
    feet: Vec<f32>,
    running: usize,
    frames: u32,
    wait: f32,
}

fn tell_gait(time: Res<Time>, mut gait: ResMut<Gait>) {
    gait.frames += 1;
    gait.wait -= time.delta_secs();
    if gait.wait > 0.0 {
        return;
    }
    gait.wait = 10.0;
    let frames = gait.frames.max(1) as f32;
    let mut feet = std::mem::take(&mut gait.feet);
    let running = std::mem::take(&mut gait.running) as f32 / frames;
    gait.frames = 0;
    if feet.is_empty() {
        return;
    }
    feet.sort_by(f32::total_cmp);
    let median = feet[feet.len() / 2];
    let (low, high) = (feet[0], feet[feet.len() - 1]);
    info!(
        "TOWNSFOLK gait within {GAIT_SEEN_M} m: {:.0} walking or running, {running:.0} running; feet at {median:.2} of the ground's speed (from {low:.2} to {high:.2})",
        feet.len() as f32 / frames,
    );
}

/// A body's pose worked out only on its frames: on the others, and while nobody can see it,
/// its player is left without its graph, so Bevy's animation passes it by.
fn skip(
    figure: Res<Figure>,
    market: Res<Market>,
    frames: Res<bevy::diagnostic::FrameCount>,
    folk: Query<&Folk>,
    mut players: Query<(&Plays, &mut AnimationGraphHandle)>,
) {
    let Some(ready) = &figure.ready else {
        return;
    };
    for (plays, mut graph) in &mut players {
        let Ok(one) = folk.get(plays.folk) else {
            continue;
        };
        let seen = !matches!(market.life.shows(one.index), Shows::Hidden)
            && market.life.people[one.index].detail != Detail::Far;
        let due = seen && (frames.0 + one.index as u32).is_multiple_of(plays.every.max(1));
        // Its graph on its frames; an empty one on the others, which Bevy's animation
        // passes by at once, with no component moved.
        let wanted = if due { &ready.graph } else { &ready.rest };
        if graph.0 != *wanted {
            graph.0 = wanted.clone();
        }
    }
}

/// Someone sitting on a bench of logs sits on it: their body moved so their hips rest on
/// the log's top, wherever the sitting clip puts them, as `people` seats a sitter.
#[allow(clippy::too_many_arguments)]
fn seat(
    mut commands: Commands,
    market: Option<Res<Market>>,
    planet: Res<Planet>,
    children: Query<&Children>,
    names: Query<&Name>,
    joints: Query<&GlobalTransform>,
    mut folk: Query<(Entity, &Folk, &mut Transform, Option<&Sat>)>,
) {
    let Some(market) = market else {
        return;
    };
    for (entity, one, mut at, sat) in &mut folk {
        let sitting = market.life.activity(one.index) == Some("Sit")
            && matches!(market.life.shows(one.index), Shows::Clip(_));
        if !sitting {
            if sat.is_some() {
                commands.entity(entity).remove::<Sat>();
            }
            continue;
        }
        let sat = match sat {
            Some(sat) => *sat,
            None => {
                let person = &market.life.people[one.index];
                let Some(spot) = person.spot.map(|s| &market.life.spots[s]) else {
                    continue;
                };
                let thing = &market.life.town.things[spot.thing];
                if thing.kind != ThingKind::Bench {
                    continue;
                }
                // The bench's seat, under where they sit along it.
                let along = (spot.at - thing.at).dot(Vec2::new(
                    libm::cosf(thing.facing),
                    -libm::sinf(thing.facing),
                ));
                let on = thing.place(Vec2::new(along, 0.0));
                let ground = market.ground(&planet, on);
                let top = ground + ground.normalize() * LOG_TOP_M;
                let Some(hips) = character::find(entity, character::HIPS, &children, &names) else {
                    continue;
                };
                let sat = Sat { top, hips };
                commands.entity(entity).insert(sat);
                sat
            }
        };
        let (Ok(hips), Ok(body)) = (joints.get(sat.hips), joints.get(entity)) else {
            continue;
        };
        let wanted = sat.top + sat.top.normalize() * HIPS_ABOVE_SEAT_M;
        at.translation = body.translation() + (wanted - hips.translation());
    }
}

/// Each body's meshes cast shadows only near the eye, set again when it crosses the
/// distance, or when it has been dressed since, a few bodies a frame.
fn shade(
    mut commands: Commands,
    frames: Res<bevy::diagnostic::FrameCount>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    children: Query<&Children>,
    meshes: Query<Has<bevy::light::NotShadowCaster>, With<Mesh3d>>,
    mut folk: Query<(Entity, &Folk, &GlobalTransform, &mut Casting)>,
) {
    let Ok(eye) = camera.single() else {
        return;
    };
    for (entity, one, at, mut casting) in &mut folk {
        let casts = at.translation().distance(eye.translation()) < SHADOWS_M;
        // Looked at again every half second besides, for garments put on since.
        let due = casting.0 != Some(casts) || (frames.0 + one.index as u32).is_multiple_of(30);
        if !due {
            continue;
        }
        casting.0 = Some(casts);
        for part in children.iter_descendants(entity) {
            let Ok(not_casting) = meshes.get(part) else {
                continue;
            };
            if casts && not_casting {
                commands
                    .entity(part)
                    .remove::<bevy::light::NotShadowCaster>();
            } else if !casts && !not_casting {
                commands.entity(part).insert(bevy::light::NotShadowCaster);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_is_posed_less_often_the_further_it_is() {
        assert_eq!(every(5.0), 1);
        assert!(every(40.0) > every(20.0));
        assert!(every(70.0) >= every(40.0));
        assert_eq!(every(500.0), FAR_EVERY);
    }

    #[test]
    fn of_a_group_one_is_worn_and_each_by_its_chance() {
        let garment = |item: &str, chance: f32, group: Option<&str>| Garment {
            item: item.into(),
            dye: "Cloth".into(),
            colours: vec![(0.5, 0.5, 0.5)],
            chance,
            group: group.map(Into::into),
        };
        let list = [
            garment("shirt", 1.0, Some("top")),
            garment("leather_coat", 1.0, Some("top")),
            garment("trousers", 1.0, None),
            garment("cap", 0.0, None),
        ];
        let mut coats = 0;
        for seed in 0..200 {
            let items: Vec<&str> = worn(&list, seed)
                .iter()
                .map(|(_, g)| g.item.as_str())
                .collect();
            let tops = items
                .iter()
                .filter(|i| ["shirt", "leather_coat"].contains(i))
                .count();
            assert_eq!(tops, 1, "{items:?}");
            assert!(
                items.contains(&"trousers") && !items.contains(&"cap"),
                "{items:?}"
            );
            coats += items.contains(&"leather_coat") as usize;
        }
        assert!((60..140).contains(&coats), "{coats} of 200 in a coat");
    }

    #[test]
    fn every_role_in_the_plan_has_a_look_of_garments_there_are() {
        let looks: Looks = ron::from_str(
            &std::fs::read_to_string(crate::model_files::asset("data/townsfolk.ron")).unwrap(),
        )
        .unwrap();
        let plan = terra_life::Plan::parse(
            &std::fs::read_to_string(crate::model_files::asset("data/life.ron")).unwrap(),
        )
        .unwrap();
        for (activity, item) in &looks.holds {
            assert!(
                plan.activities.contains_key(activity),
                "holds: no activity {activity}"
            );
            let file = character::held_look(item);
            assert!(
                crate::model_files::asset(&file).exists(),
                "holds: no {file}"
            );
        }
        for role in &plan.roles {
            let look = looks.roles.get(&role.name).expect("a look for every role");
            for body in &look.bodies {
                assert!(appearance::Appearance::template(body).is_some(), "{body}");
            }
            for garment in look.wears.iter().chain(&look.women) {
                let file = character::look(&garment.item);
                assert!(
                    crate::model_files::asset(&file).exists(),
                    "{}: no {file}",
                    role.name
                );
                let materials = crate::model_files::materials(&crate::model_files::json(&file));
                assert!(
                    materials
                        .iter()
                        .any(|m| m.split('.').next() == Some(garment.dye.as_str())),
                    "{}: {} has no {} to dye",
                    role.name,
                    garment.item,
                    garment.dye
                );
            }
        }
    }
}
