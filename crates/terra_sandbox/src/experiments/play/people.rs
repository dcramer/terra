//! People (game design 001, people.md): figures in the world who are not the player.
//!
//! They share the player's figure in their own colours. Each has a role and a few
//! states: a prisoner kneels bound until freed, then follows you; a smith who follows you
//! settles at a smithy in your claim and works it (D90); villagers go about their day
//! and say what is going on when you speak to them (D92), from `settlement`.

use super::Player;
use super::bag::Notice;
use super::claim::Claim;
use super::crafting::{SmithAtHand, Station};
use super::creatures::{Creature, Species};
use super::interact::{CanInteract, Focus, Focusing, Interactable};
use super::props::Prop;
use crate::experiments::build::Site;
use crate::planet::Planet;
use bevy::{
    prelude::*,
    world_serialization::{WorldAsset, WorldInstanceReady},
};
use std::time::Duration;

const MODEL: &str = "models/character/figure.gltf";
const BLEND: Duration = Duration::from_millis(220);
/// How close the spiders may be while a prisoner is freed.
const GUARDED_M: f32 = 16.0;
const WALK_MPS: f32 = 1.5;
const JOG_MPS: f32 = 4.6;

/// What someone is doing.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Doing {
    /// Bound, waiting to be freed.
    Bound,
    /// Freed, and keeping up with the player.
    Following,
    /// At work at a spot, such as a smithy.
    Working(Vec3),
    /// Standing about at a spot, facing a way.
    Standing,
    /// Sitting at a spot.
    Sitting,
}

#[derive(Component)]
pub struct Person {
    pub name: &'static str,
    /// What they say when spoken to, if anything in particular.
    pub says: Option<&'static str>,
    pub doing: Doing,
    /// Whether they can work a smithy.
    pub smith: bool,
    facing: Vec3,
    /// Whether they have asked for a smithy yet, so they ask once.
    asked: bool,
}

impl Person {
    pub fn new(name: &'static str, doing: Doing, smith: bool) -> Self {
        Self {
            name,
            says: None,
            doing,
            smith,
            facing: Vec3::ZERO,
            asked: false,
        }
    }
}

/// The figure's model and the animations people use.
#[derive(Resource)]
struct Figure {
    model: Handle<Gltf>,
    ready: Option<(Handle<WorldAsset>, Handle<AnimationGraph>, Clips)>,
}

#[derive(Clone, Copy)]
struct Clips {
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    jog: AnimationNodeIndex,
    kneel: AnimationNodeIndex,
    sit: AnimationNodeIndex,
    work: AnimationNodeIndex,
}

/// On a person's animation player: whose it is, and what it plays.
#[derive(Component)]
struct Animates {
    person: Entity,
    clips: Clips,
    playing: Option<AnimationNodeIndex>,
}

/// Asks for a person to be stood in the world, in a colour of their own.
#[derive(Message, Clone, Copy)]
pub struct Arrive {
    pub name: &'static str,
    pub at: Vec3,
    pub facing: Vec3,
    pub doing: Doing,
    pub smith: bool,
    pub colour: Color,
    pub interact: Option<Interactable>,
    pub says: Option<&'static str>,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Arrive>()
        .add_systems(Startup, load)
        .add_systems(
            Update,
            (
                prepare,
                arrive,
                free.after(Focusing),
                follow,
                settle,
                animate,
            )
                .chain(),
        );
}

fn load(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Figure {
        model: assets.load(MODEL),
        ready: None,
    });
}

fn prepare(
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut figure: ResMut<Figure>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if figure.ready.is_some() || !assets.is_loaded_with_dependencies(&figure.model) {
        return;
    }
    let Some(gltf) = gltfs.get(&figure.model) else {
        return;
    };
    let Some(scene) = gltf.default_scene.clone() else {
        return;
    };
    let clip = |name: &str| gltf.named_animations[name].clone();
    let (graph, nodes) = AnimationGraph::from_clips([
        clip("Idle_Loop"),
        clip("Walk_Loop"),
        clip("Jog_Fwd_Loop"),
        clip("Crouch_Idle_Loop"),
        clip("Sitting_Idle_Loop"),
        clip("Fixing_Kneeling"),
    ]);
    figure.ready = Some((
        scene,
        graphs.add(graph),
        Clips {
            idle: nodes[0],
            walk: nodes[1],
            jog: nodes[2],
            kneel: nodes[3],
            sit: nodes[4],
            work: nodes[5],
        },
    ));
}

/// Stands each person asked for in the world, once the figure is ready.
fn arrive(
    mut commands: Commands,
    figure: Res<Figure>,
    planet: Res<Planet>,
    site: Res<Site>,
    mut asked: MessageReader<Arrive>,
    mut waiting: Local<Vec<Arrive>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    waiting.extend(asked.read().copied());
    let Some((scene, graph, clips)) = &figure.ready else {
        return;
    };
    for arrival in waiting.drain(..) {
        let feet = planet.drawn(arrival.at.normalize());
        let up = feet.normalize();
        // Where, in the terms --at and --face take, so a scripted run can go to them.
        let local = site.frame.compute_affine().inverse().transform_point3(feet);
        info!(
            "{} is {:.1} m east and {:.1} m south of the outpost.",
            arrival.name, local.x, local.z
        );
        let facing = arrival
            .facing
            .reject_from(up)
            .normalize_or(up.any_orthonormal_vector());
        let mut person = commands.spawn((
            Name::new(arrival.name),
            Person {
                facing,
                says: arrival.says,
                ..Person::new(arrival.name, arrival.doing, arrival.smith)
            },
            Transform::from_translation(feet).looking_to(facing, up),
            Visibility::default(),
        ));
        if let Some(what) = arrival.interact {
            person.insert(CanInteract(what));
        }
        let person = person.id();
        let (graph, clips) = (graph.clone(), *clips);
        let clothes = materials.add(StandardMaterial {
            base_color: arrival.colour,
            perceptual_roughness: 0.8,
            ..default()
        });
        commands
            .spawn((
                WorldAssetRoot(scene.clone()),
                // The model faces +Z; things here face -Z.
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                ChildOf(person),
            ))
            .observe(
                move |ready: On<WorldInstanceReady>,
                      mut commands: Commands,
                      children: Query<&Children>,
                      players: Query<(), With<AnimationPlayer>>,
                      looks: Query<&MeshMaterial3d<StandardMaterial>>,
                      materials: Res<Assets<StandardMaterial>>| {
                    for child in children.iter_descendants(ready.entity) {
                        if players.contains(child) {
                            commands.entity(child).insert((
                                AnimationGraphHandle(graph.clone()),
                                AnimationTransitions::new(),
                                Animates {
                                    person,
                                    clips,
                                    playing: None,
                                },
                            ));
                        }
                        // The pale body takes their colour; the dark joints stay.
                        let pale = looks
                            .get(child)
                            .ok()
                            .and_then(|look| materials.get(&look.0))
                            .is_some_and(|look| look.base_color.to_srgba().red > 0.5);
                        if pale {
                            commands
                                .entity(child)
                                .insert(MeshMaterial3d(clothes.clone()));
                        }
                    }
                },
            );
    }
}

/// F at a prisoner frees them, once the spiders guarding them are dead.
fn free(
    mut commands: Commands,
    focus: Res<Focus>,
    creatures: Query<(&Creature, &Transform)>,
    mut people: Query<(&mut Person, &Transform), Without<Creature>>,
    mut notice: ResMut<Notice>,
) {
    if !focus.used(Interactable::Prisoner) {
        return;
    }
    let Some(entity) = focus.entity else { return };
    let Ok((mut person, at)) = people.get_mut(entity) else {
        return;
    };
    let guarded = creatures.iter().any(|(creature, there)| {
        creature.species == Species::Spider
            && creature.alive()
            && there.translation.distance(at.translation) < GUARDED_M
    });
    if guarded {
        notice.say("Not while the spiders are near");
        return;
    }
    person.doing = Doing::Following;
    commands.entity(entity).remove::<CanInteract>();
    info!("Freed {}.", person.name);
    notice.say(format!(
        "{}: \"Thank you. I'm the smith from the village. I'll come with you.\"",
        person.name
    ));
}

/// Followers keep up with the player, walking when near and jogging when behind.
fn follow(
    time: Res<Time>,
    planet: Res<Planet>,
    player: Single<&Transform, With<Player>>,
    mut people: Query<(&mut Person, &mut Transform), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.1);
    for (mut person, mut at) in &mut people {
        let here = at.translation;
        let up = here.normalize();
        let goal = match person.doing {
            Doing::Following => player.translation,
            Doing::Working(spot) => spot,
            _ => continue,
        };
        let toward = (goal - here).reject_from(up);
        let distance = toward.length();
        let keep = if person.doing == Doing::Following {
            2.5
        } else {
            0.4
        };
        let speed = match distance {
            d if d < keep => 0.0,
            d if d < 8.0 => WALK_MPS,
            _ => JOG_MPS,
        };
        let direction = toward.normalize_or_zero();
        if speed > 0.0 {
            let turn = 1.0 - libm::expf(-dt * 8.0);
            person.facing = person
                .facing
                .lerp(direction, turn)
                .reject_from(up)
                .normalize_or(direction);
        }
        let next = planet.drawn((here + direction * speed * dt).normalize());
        *at = Transform::from_translation(next).looking_to(person.facing, next.normalize());
    }
}

/// A smith who has followed you into your claim settles at its smithy and works it.
fn settle(
    claim: Res<Claim>,
    props: Query<(&Prop, &Transform)>,
    mut people: Query<(&mut Person, &Transform), Without<Prop>>,
    mut smith_at_hand: ResMut<SmithAtHand>,
    mut notice: ResMut<Notice>,
) {
    for (mut person, at) in &mut people {
        if person.doing != Doing::Following || !person.smith || !claim.covers(at.translation) {
            continue;
        }
        let smithy = props
            .iter()
            .find(|(prop, there)| {
                prop.item.station == Some(Station::Smithy) && claim.covers(there.translation)
            })
            .map(|(_, there)| there.translation);
        match smithy {
            Some(smithy) => {
                let up = smithy.normalize();
                let spot = smithy + up.any_orthonormal_vector() * 3.5;
                person.doing = Doing::Working(spot);
                smith_at_hand.0 = true;
                info!("{} took up the smithy.", person.name);
                notice.say(format!(
                    "{} takes up the smithy: iron can be worked now",
                    person.name
                ));
            }
            None if !person.asked => {
                person.asked = true;
                notice.say(format!(
                    "{}: \"Build a smithy here and I'll work it.\"",
                    person.name
                ));
            }
            None => {}
        }
    }
}

fn animate(
    people: Query<(&Person, &Transform)>,
    mut animators: Query<(
        &mut Animates,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
    mut last: Local<Vec<(Entity, Vec3)>>,
) {
    let mut moved = Vec::new();
    for (mut animates, mut player, mut transitions) in &mut animators {
        let Ok((person, at)) = people.get(animates.person) else {
            continue;
        };
        let before = last
            .iter()
            .find(|(entity, _)| *entity == animates.person)
            .map(|(_, at)| *at)
            .unwrap_or(at.translation);
        moved.push((animates.person, at.translation));
        let step = before.distance(at.translation);
        let clips = animates.clips;
        let wanted = match person.doing {
            Doing::Bound => clips.kneel,
            Doing::Sitting => clips.sit,
            _ if step > 0.05 => clips.jog,
            _ if step > 0.005 => clips.walk,
            Doing::Working(_) => clips.work,
            _ => clips.idle,
        };
        if animates.playing == Some(wanted) {
            continue;
        }
        animates.playing = Some(wanted);
        transitions.play(&mut player, wanted, BLEND).repeat();
    }
    *last = moved;
}
