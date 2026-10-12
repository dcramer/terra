//! People (game design 001, people.md): figures in the world who are not the player.
//!
//! Each stands on a base body, as the player does, and wears an outfit of their own
//! (`character::Outfit`, shown by `worn`). Each has a role and a few
//! states: a prisoner kneels bound until freed, then follows you; a smith who follows you
//! settles at a forge in your claim and works it (D90); villagers go about their day
//! and say what is going on when you speak to them (D92), from `settlement`.

use super::bag::Notice;
use super::bodies::Body;
use super::catalogue::Rest;
use super::claim::Claim;
use super::crafting::Station;
use super::creatures::Creature;
use super::interact::{CanInteract, Focus, Focusing, Interactable};
use super::props::Prop;
use super::{IsLocalPlayer, LocalPlayer, Player};
use crate::build::Site;
use crate::character;
use crate::loading::Loading;
use crate::planet::Planet;
use bevy::{
    prelude::*,
    world_serialization::{WorldAsset, WorldInstanceReady},
};
use std::time::Duration;

/// The base bodies people may have (`character::Body`), by the name their gear is fitted under.
const BODIES: [&str; 2] = ["base-male", "base-female"];
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
    /// Freed, and keeping up with the player who freed them.
    Following(Entity),
    /// At work at a spot, such as a forge.
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
    /// Their trade, if they have one: what stations they can work (`worked_by` in
    /// `assets/data/stations.ron`), such as a smith a forge.
    pub trade: Option<&'static str>,
    facing: Vec3,
    /// Whether they have asked for a station to work yet, so they ask once.
    asked: bool,
}

impl Person {
    pub fn new(name: &'static str, doing: Doing, trade: Option<&'static str>) -> Self {
        Self {
            name,
            says: None,
            doing,
            trade,
            facing: Vec3::ZERO,
            asked: false,
        }
    }
}

/// Each base body's model and the animations people use, once loaded.
#[derive(Resource)]
struct Figures(Vec<Figure>);

struct Figure {
    body: &'static str,
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

/// Asks for a person to be stood in the world, on a base body (one of `BODIES`), wearing
/// items by id (`character::item_id`), each dyed a colour if given (its leather, or the
/// role its look names).
#[derive(Message, Clone)]
pub struct Arrive {
    pub name: &'static str,
    pub at: Vec3,
    pub facing: Vec3,
    pub doing: Doing,
    pub trade: Option<&'static str>,
    pub body: &'static str,
    pub wears: Vec<Dyed>,
    pub interact: Option<Interactable>,
    pub says: Option<&'static str>,
}

/// Something a person wears, by item id, and the colour its main material is dyed, if any.
#[derive(Clone, Copy)]
pub struct Dyed(pub &'static str, pub Option<(&'static str, Color)>);

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
                sit,
            )
                .chain(),
        );
}

fn load(mut commands: Commands, assets: Res<AssetServer>, mut loading: ResMut<Loading>) {
    commands.insert_resource(Figures(
        BODIES
            .into_iter()
            .map(|body| {
                let model = assets.load(format!("models/character/{body}.glb"));
                loading.wait_for(&model);
                Figure {
                    body,
                    model,
                    ready: None,
                }
            })
            .collect(),
    ));
}

fn prepare(
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut figures: ResMut<Figures>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    for figure in &mut figures.0 {
        if figure.ready.is_some() || !assets.is_loaded_with_dependencies(&figure.model) {
            continue;
        }
        let Some(gltf) = gltfs.get(&figure.model) else {
            continue;
        };
        let Some(scene) = gltf.default_scene.clone() else {
            continue;
        };
        figure.ready = Some(prepared(gltf, scene, &mut graphs));
    }
}

/// A body's scene, and the clips people use built into a graph.
fn prepared(
    gltf: &Gltf,
    scene: Handle<WorldAsset>,
    graphs: &mut Assets<AnimationGraph>,
) -> (Handle<WorldAsset>, Handle<AnimationGraph>, Clips) {
    let clip = |name: &str| gltf.named_animations[name].clone();
    let (graph, nodes) = AnimationGraph::from_clips([
        clip("Idle_Loop"),
        clip("Walk_Loop"),
        clip("Jog_Fwd_Loop"),
        clip("Crouch_Idle_Loop"),
        clip("Sitting_Idle_Loop"),
        clip("Fixing_Kneeling"),
    ]);
    (
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
    )
}

/// Stands each person asked for in the world, once their body is ready.
fn arrive(
    mut commands: Commands,
    figures: Res<Figures>,
    planet: Res<Planet>,
    site: Res<Site>,
    mut asked: MessageReader<Arrive>,
    mut waiting: Local<Vec<Arrive>>,
) {
    waiting.extend(asked.read().cloned());
    let mut later = Vec::new();
    for arrival in waiting.drain(..) {
        let Some(figure) = figures.0.iter().find(|figure| figure.body == arrival.body) else {
            warn!(
                "{} has a body there is no model of: {}",
                arrival.name, arrival.body
            );
            continue;
        };
        let Some((scene, graph, clips)) = &figure.ready else {
            later.push(arrival);
            continue;
        };
        let feet = planet.ground_at(arrival.at);
        let up = feet.normalize();
        // Where, in the terms --at and --face take, so a scripted run can go to them.
        let local = site.frame.compute_affine().inverse().transform_point3(feet);
        info!(
            "{} is {:.1} m east and {:.1} m south of the start.",
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
                ..Person::new(arrival.name, arrival.doing, arrival.trade)
            },
            Transform::from_translation(feet).looking_to(facing, up),
            Visibility::default(),
            character::Outfit(
                arrival
                    .wears
                    .iter()
                    .map(|Dyed(item, dye)| character::Clothing {
                        item: item.to_string(),
                        tint: character::Tint(
                            dye.iter()
                                .map(|(role, colour)| (role.to_string(), *colour))
                                .collect(),
                        ),
                    })
                    .collect(),
            ),
        ));
        if let Some(what) = arrival.interact {
            person.insert(CanInteract(what));
        }
        // Someone bound holds their place; others are shoved like anyone.
        person.insert(match arrival.doing {
            Doing::Bound | Doing::Sitting => Body::fixed(0.35),
            _ => Body::new(0.35, 2.0),
        });
        let person = person.id();
        let (graph, clips) = (graph.clone(), *clips);
        commands
            .spawn((
                character::Body { name: figure.body },
                WorldAssetRoot(scene.clone()),
                // The model faces +Z; things here face -Z.
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                ChildOf(person),
            ))
            .observe(
                move |ready: On<WorldInstanceReady>,
                      mut commands: Commands,
                      children: Query<&Children>,
                      players: Query<(), With<AnimationPlayer>>| {
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
                    }
                },
            );
    }
    *waiting = later;
}

/// F at a prisoner frees them, once the spiders guarding them are dead.
fn free(
    mut commands: Commands,
    focus: Res<Focus>,
    creatures: Query<(&Creature, &Transform)>,
    mut people: Query<(&mut Person, &Transform), Without<Creature>>,
    player: Single<Entity, IsLocalPlayer>,
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
        creature.species.hostile()
            && creature.alive()
            && there.translation.distance(at.translation) < GUARDED_M
    });
    if guarded {
        notice.say("Not while the spiders are near");
        return;
    }
    person.doing = Doing::Following(*player);
    commands.entity(entity).remove::<CanInteract>();
    info!("Freed {}.", person.name);
    notice.say(match person.trade {
        Some(trade) => format!(
            "{}: \"Thank you. I'm the {trade} from the village. I'll come with you.\"",
            person.name
        ),
        None => format!("{}: \"Thank you. I'll come with you.\"", person.name),
    });
}

/// Followers keep up with the player they follow, walking when near and jogging when behind.
fn follow(
    time: Res<Time>,
    planet: Res<Planet>,
    players: Query<&Transform, With<Player>>,
    mut people: Query<(&mut Person, &mut Transform), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.1);
    for (mut person, mut at) in &mut people {
        let here = at.translation;
        let up = here.normalize();
        let goal = match person.doing {
            Doing::Following(leader) => match players.get(leader) {
                Ok(leader) => leader.translation,
                Err(_) => continue,
            },
            Doing::Working(spot) => spot,
            _ => continue,
        };
        let toward = (goal - here).reject_from(up);
        let distance = toward.length();
        let keep = if matches!(person.doing, Doing::Following(_)) {
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
        let next = planet.ground_at(here + direction * speed * dt);
        *at = Transform::from_translation(next).looking_to(person.facing, next.normalize());
    }
}

/// Someone with a trade who has followed a player into their claim settles at a station in
/// it that their trade works, such as a smith at a forge, and works it.
fn settle(
    claims: Query<(&Claim, Has<LocalPlayer>)>,
    props: Query<(&Prop, &Transform)>,
    mut people: Query<(&mut Person, &Transform), Without<Prop>>,
    mut notice: ResMut<Notice>,
) {
    for (mut person, at) in &mut people {
        let Doing::Following(leader) = person.doing else {
            continue;
        };
        let Ok((claim, ours)) = claims.get(leader) else {
            continue;
        };
        let Some(trade) = person.trade else {
            continue;
        };
        if !claim.covers(at.translation) {
            continue;
        }
        // Only what happens to this machine's own player is said on its screen.
        let mut say = |text: String| {
            if ours {
                notice.say(text);
            }
        };
        let worked = props
            .iter()
            .filter_map(|(prop, there)| Some((prop.item.station()?, there.translation)))
            .find(|(station, there)| station.worked_by() == Some(trade) && claim.covers(*there));
        let wanted = Station::all().find(|station| station.worked_by() == Some(trade));
        match (worked, wanted) {
            (Some((station, at)), _) => {
                let up = at.normalize();
                let spot = at + up.any_orthonormal_vector() * 1.8;
                person.doing = Doing::Working(spot);
                let station = station.name().to_lowercase();
                info!("{} took up the {station}.", person.name);
                say(format!("{} takes up the {station}", person.name));
            }
            (None, Some(station)) if !person.asked => {
                person.asked = true;
                say(format!(
                    "{}: \"Build a {} here and I'll work it.\"",
                    person.name,
                    station.name().to_lowercase()
                ));
            }
            _ => {}
        }
    }
}

/// How far from a seat someone told to sit looks for it.
const SEAT_REACH_M: f32 = 1.5;
/// How far above a seat's top the hips rest.
const HIPS_ABOVE_SEAT_M: f32 = 0.08;

/// Puts whoever is sitting on the nearest seat: wherever their sitting pose puts their
/// hips, they are moved so the hips rest on its top. Someone with no seat near sits
/// where they are.
fn sit(
    seats: Query<(&Prop, &GlobalTransform)>,
    joints: Query<&GlobalTransform, Without<Prop>>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut people: Query<(Entity, &Person, &mut Transform)>,
) {
    for (entity, person, mut at) in &mut people {
        if person.doing != Doing::Sitting {
            continue;
        }
        let Some(seat_top) = seats
            .iter()
            .filter(|(prop, _)| prop.item.rest == Some(Rest::Sit))
            .map(|(prop, seat)| seat.transform_point(Vec3::Y * prop.half.y * 2.0))
            .filter(|top| top.distance(at.translation) < SEAT_REACH_M)
            .min_by(|a, b| {
                a.distance(at.translation)
                    .total_cmp(&b.distance(at.translation))
            })
        else {
            continue;
        };
        let Some(hips) = character::find(entity, "DEF-hips", &children, &names)
            .and_then(|hips| joints.get(hips).ok())
        else {
            continue;
        };
        let (Ok(body), up) = (joints.get(entity), seat_top.normalize()) else {
            continue;
        };
        let wanted = seat_top + up * HIPS_ABOVE_SEAT_M;
        at.translation = body.translation() + (wanted - hips.translation());
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
