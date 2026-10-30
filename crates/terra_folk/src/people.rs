//! People (game design 001, people.md): figures in the world who are not the player.
//!
//! Each stands on the base body, as the player does, shaped by a template
//! (`appearance::Appearance`), and wears an outfit of their own
//! (`character::Outfit`, shown by `worn`). Each has a role and a few
//! states: a prisoner kneels bound until freed, then follows you; a smith who follows you
//! settles at a forge in your claim and works it (D90); villagers go about their day
//! and say what is going on when you speak to them (D92), from `settlement`.

use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
    world_serialization::{WorldAsset, WorldInstanceReady},
};
use std::time::Duration;
use terra_creatures::creatures::Creature;
use terra_survival::catalogue::Rest;
use terra_survival::claim::Claim;
use terra_survival::crafting::{Stations, Worker};
use terra_survival::interact::{CanInteract, Interactable, Interacted};
use terra_survival::props::{Models, Prop};
use terra_survival::told::Told;
use terra_survival::vitals::Effort;
use terra_survival::warmth::BodyHeat;
use terra_walker::Player;
use terra_walker::bodies::Body;
use terra_walker::{appearance, character};
use terra_world::climate::ClimateSample;
use terra_world::loading::Loading;
use terra_world::planet::Planet;
use terra_world::start::Start;
use terra_world::tick::{Sim, Smoothed};

const BLEND: Duration = Duration::from_millis(220);
/// How close the spiders may be while a prisoner is freed.
const GUARDED_M: f32 = 16.0;
/// How fast someone walks and jogs: the walk its clip's own pace (D585), the jog a little over
/// a player's jog (`terra_motion::gaits::JOG`) to catch up, within what D585 shows its clip at.
const WALK_MPS: f32 = terra_town::WALK_PACE;
const JOG_MPS: f32 = terra_motion::gaits::JOG * 1.03;

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

/// Someone in the world. The stations and fires see them by their `Worker`, set from them
/// when they are put in the world and by `set_doing` in the system that changes what they
/// do, so no one reads one a tick old.
#[derive(Component)]
#[require(Smoothed, ClimateSample, BodyHeat, Worker)]
#[component(on_insert = Person::seen_at_work)]
pub struct Person {
    pub name: &'static str,
    /// What they say when spoken to, if anything in particular.
    pub says: Option<&'static str>,
    doing: Doing,
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

    /// What they are doing.
    pub fn doing(&self) -> Doing {
        self.doing
    }

    /// Sets what they are doing, and their worker with it.
    pub fn set_doing(&mut self, doing: Doing, worker: &mut Worker) {
        self.doing = doing;
        *worker = self.worker();
    }

    /// What the stations and fires see of them: who they are, their trade, where they are
    /// at work, and how hard they work (following someone is walking).
    fn worker(&self) -> Worker {
        Worker {
            name: self.name,
            trade: self.trade,
            working_at: match self.doing {
                Doing::Working(spot) => Some(spot),
                _ => None,
            },
            effort: match self.doing {
                Doing::Working(_) => Effort::Working,
                Doing::Following(_) => Effort::Moving,
                _ => Effort::Resting,
            },
        }
    }

    /// Sets the worker of someone just put in the world from them.
    fn seen_at_work(mut world: DeferredWorld, context: HookContext) {
        let Some(worker) = world.get::<Person>(context.entity).map(Person::worker) else {
            return;
        };
        if let Some(mut was) = world.get_mut::<Worker>(context.entity) {
            *was = worker;
        }
    }
}

/// The base body's model and the animations people use, once loaded.
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
    /// The walk's and jog's own paces (`pace.rs`), so their feet do not slide.
    walk_pace: f32,
    jog_pace: f32,
}

/// On a person's animation player: whose it is, and what it plays.
#[derive(Component)]
struct Animates {
    person: Entity,
    clips: Clips,
    playing: Option<AnimationNodeIndex>,
    /// Where the person stood last frame, to tell walking from standing.
    was_at: Option<Vec3>,
}

/// Asks for a person to be stood in the world, on the base body shaped by a template
/// (`appearance::Appearance::template`, such as "female" or "heavy"), wearing
/// items by id (`character::item_id`), each dyed a colour if given (its leather, or the
/// role its look names).
#[derive(Message, Clone)]
pub struct Arrive {
    pub name: &'static str,
    pub at: Vec3,
    pub facing: Vec3,
    pub doing: Doing,
    pub trade: Option<&'static str>,
    pub template: &'static str,
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
        .add_systems(Update, (prepare, arrive, animate, sit).chain())
        .add_systems(
            FixedUpdate,
            (free, follow, settle).chain().in_set(Sim::World),
        );
}

fn load(mut commands: Commands, assets: Res<AssetServer>, mut loading: ResMut<Loading>) {
    let model = assets.load(appearance::BASE);
    loading.wait_for(&model);
    commands.insert_resource(Figure { model, ready: None });
}

fn prepare(
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<bevy::gltf::GltfNode>>,
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
    figure.ready = Some(prepared(gltf, &gltf_nodes, scene, &mut graphs));
}

/// A body's scene, and the clips people use built into a graph.
fn prepared(
    gltf: &Gltf,
    gltf_nodes: &Assets<bevy::gltf::GltfNode>,
    scene: Handle<WorldAsset>,
    graphs: &mut Assets<AnimationGraph>,
) -> (Handle<WorldAsset>, Handle<AnimationGraph>, Clips) {
    let clip = |name: &str| gltf.named_animations[name].clone();
    let paces = terra_walker::pace::paces(gltf, gltf_nodes);
    let pace = |name: &str| paces.get(name).copied().unwrap_or(0.0);
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
            walk_pace: pace("Walk_Loop"),
            jog_pace: pace("Jog_Fwd_Loop"),
        },
    )
}

/// Stands each person asked for in the world, once their body is ready.
fn arrive(
    mut commands: Commands,
    figure: Res<Figure>,
    planet: Res<Planet>,
    start: Res<Start>,
    mut asked: MessageReader<Arrive>,
    mut waiting: Local<Vec<Arrive>>,
) {
    waiting.extend(asked.read().cloned());
    let mut later = Vec::new();
    for arrival in waiting.drain(..) {
        let Some(shape) = appearance::Appearance::template(arrival.template) else {
            warn!(
                "{} has a template there is none of: {}",
                arrival.name, arrival.template
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
        let local = start.local(feet);
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
            _ => Body::person(0.35, 2.0),
        });
        let person = person.id();
        let (graph, clips) = (graph.clone(), *clips);
        commands
            .spawn((
                character::Body,
                shape,
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
                                    was_at: None,
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
    mut interacted: MessageReader<Interacted>,
    creatures: Query<(&Creature, &Transform)>,
    bestiary: Res<terra_creatures::species::Bestiary>,
    mut people: Query<(&mut Person, &mut Worker, &Transform), Without<Creature>>,
    mut told: MessageWriter<Told>,
) {
    for used in interacted.read() {
        let Some(entity) = used.on(Interactable::Prisoner) else {
            continue;
        };
        let Ok((mut person, mut worker, at)) = people.get_mut(entity) else {
            continue;
        };
        let guarded = creatures.iter().any(|(creature, there)| {
            bestiary.hostile(creature.species)
                && creature.alive()
                && there.translation.distance(at.translation) < GUARDED_M
        });
        if guarded {
            told.write(Told::new(used.player, "Not while the spiders are near"));
            continue;
        }
        person.set_doing(Doing::Following(used.player), &mut worker);
        commands.entity(entity).remove::<CanInteract>();
        info!("Freed {}.", person.name);
        let said = match person.trade {
            Some(trade) => format!(
                "{}: \"Thank you. I'm the {trade} from the village. I'll come with you.\"",
                person.name
            ),
            None => format!("{}: \"Thank you. I'll come with you.\"", person.name),
        };
        told.write(Told::new(used.player, said));
    }
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
    claims: Query<&Claim>,
    props: Query<(&Prop, &Transform)>,
    stations: Res<Stations>,
    mut people: Query<(&mut Person, &mut Worker, &Transform), Without<Prop>>,
    mut told: MessageWriter<Told>,
) {
    for (mut person, mut worker, at) in &mut people {
        let Doing::Following(leader) = person.doing else {
            continue;
        };
        let Ok(claim) = claims.get(leader) else {
            continue;
        };
        let Some(trade) = person.trade else {
            continue;
        };
        if !claim.covers(at.translation) {
            continue;
        }
        // Said to the player they follow.
        let mut say = |text: String| {
            told.write(Told::new(leader, text));
        };
        let worked = props
            .iter()
            .filter_map(|(prop, there)| Some((prop.item.station?, there.translation)))
            .find(|&(station, there)| {
                stations.worked_by(station) == Some(trade) && claim.covers(there)
            });
        let wanted = stations
            .all()
            .find(|&station| stations.worked_by(station) == Some(trade));
        match (worked, wanted) {
            (Some((station, at)), _) => {
                let up = at.normalize();
                let spot = at + up.any_orthonormal_vector() * 1.8;
                person.set_doing(Doing::Working(spot), &mut worker);
                let station = stations.name(station).to_lowercase();
                info!("{} took up the {station}.", person.name);
                say(format!("{} takes up the {station}", person.name));
            }
            (None, Some(station)) if !person.asked => {
                person.asked = true;
                say(format!(
                    "{}: \"Build a {} here and I'll work it.\"",
                    person.name,
                    stations.name(station).to_lowercase()
                ));
            }
            _ => {}
        }
    }
}

/// Where someone sitting sits: the top of their seat, and their hips, which are put on it.
#[derive(Component, Clone, Copy)]
struct Seated {
    top: Vec3,
    hips: Entity,
}

/// How far from a seat someone told to sit looks for it.
const SEAT_REACH_M: f32 = 1.5;
/// How far above a seat's top the hips rest.
const HIPS_ABOVE_SEAT_M: f32 = 0.08;

/// Puts whoever is sitting on the nearest seat: wherever their sitting pose puts their
/// hips, they are moved so the hips rest on its top. Someone with no seat near sits
/// where they are.
fn sit(
    mut commands: Commands,
    seats: Query<(&Prop, &GlobalTransform)>,
    models: Res<Models>,
    joints: Query<&GlobalTransform, Without<Prop>>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut people: Query<(Entity, &Person, &mut Transform, Option<&Seated>)>,
) {
    for (entity, person, mut at, seated) in &mut people {
        if person.doing != Doing::Sitting {
            if seated.is_some() {
                commands.entity(entity).remove::<Seated>();
            }
            continue;
        }
        // The seat and the hips are found once, as they sit down.
        let seated = match seated {
            Some(seated) => *seated,
            None => {
                let Some(top) = seats
                    .iter()
                    .filter(|(prop, _)| prop.item.rest == Some(Rest::Sit))
                    .map(|(prop, seat)| {
                        // Its seat socket (docs/art-style.md, the model contract), or
                        // its top for a seat with nothing over it.
                        let top = Vec3::Y * prop.half.y * 2.0;
                        let socket = models.of(&prop.item).and_then(|m| m.socket("seat"));
                        seat.transform_point(socket.unwrap_or(top))
                    })
                    .filter(|top| top.distance(at.translation) < SEAT_REACH_M)
                    .min_by(|a, b| {
                        a.distance(at.translation)
                            .total_cmp(&b.distance(at.translation))
                    })
                else {
                    continue;
                };
                let Some(hips) = character::find(entity, character::HIPS, &children, &names) else {
                    continue;
                };
                let seated = Seated { top, hips };
                commands.entity(entity).insert(seated);
                seated
            }
        };
        let seat_top = seated.top;
        let Ok(hips) = joints.get(seated.hips) else {
            continue;
        };
        let (Ok(body), up) = (joints.get(entity), seat_top.normalize()) else {
            continue;
        };
        let wanted = seat_top + up * HIPS_ABOVE_SEAT_M;
        at.translation = body.translation() + (wanted - hips.translation());
    }
}

/// How fast a walk or jog of `pace` plays at `speed` on a body whose animation player is at
/// `place` with its own node's transform `own`: scaled by the body's size against its model
/// alone (`pace::size`). By the player's world scale, which carries base.glb's armature scale
/// (1.078) again, every stride played 7% slow and its feet slid.
fn stride_rate(speed: f32, pace: f32, place: &GlobalTransform, own: &Transform) -> f32 {
    terra_walker::pace::rate(speed, pace, terra_walker::pace::size(place, own))
}

fn animate(
    time: Res<Time>,
    people: Query<(&Person, &Transform)>,
    mut animators: Query<(
        &mut Animates,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &GlobalTransform,
        &Transform,
    )>,
) {
    let dt = time.delta_secs();
    for (mut animates, mut player, mut transitions, place, own) in &mut animators {
        let Ok((person, at)) = people.get(animates.person) else {
            continue;
        };
        let before = animates.was_at.replace(at.translation);
        let step = before.map_or(0.0, |before| before.distance(at.translation));
        let clips = animates.clips;
        let wanted = match person.doing {
            Doing::Bound => clips.kneel,
            Doing::Sitting => clips.sit,
            _ if step > 0.05 => clips.jog,
            _ if step > 0.005 => clips.walk,
            Doing::Working(_) => clips.work,
            _ => clips.idle,
        };
        // A walk or jog at the speed they move, so their feet do not slide.
        let pace = if wanted == clips.walk {
            clips.walk_pace
        } else if wanted == clips.jog {
            clips.jog_pace
        } else {
            0.0
        };
        if pace > 0.0
            && dt > 0.0
            && let Some(playing) = player.animation_mut(wanted)
        {
            playing.set_speed(stride_rate(step / dt, pace, place, own));
        }
        if animates.playing == Some(wanted) {
            continue;
        }
        animates.playing = Some(wanted);
        transitions.play(&mut player, wanted, BLEND).repeat();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{pooling, settlement};
    use terra_survival::kinds;
    use terra_walker::figure;
    use terra_walker::model_files;

    #[test]
    fn everyone_stood_in_the_world_names_a_template_items_and_colours_there_are() {
        // A name that matches nothing leaves a body unshaped, undressed or uncoloured
        // without a word.
        let mut templates = vec![figure::TEMPLATE, pooling::SMITH_TEMPLATE];
        let mut wears = pooling::SMITH_WEARS.to_vec();
        for villager in &settlement::PEOPLE {
            templates.push(villager.template);
            wears.extend_from_slice(villager.wears);
        }
        for template in templates {
            assert!(
                appearance::Appearance::template(template).is_some(),
                "no template {template}"
            );
        }
        let kinds = kinds::read().kinds;
        let mut dyed = 0;
        for Dyed(item, dye) in wears {
            assert!(
                kinds
                    .iter()
                    .any(|kind| kind.worn.is_some() && character::item_id(&kind.name) == item),
                "no worn item {item} in the catalogue"
            );
            let file = character::look(item);
            assert!(model_files::asset(&file).exists(), "{item}: no {file}");
            let Some((role, _)) = dye else {
                continue;
            };
            // A part's role is its material's name before any `.` (`character::Tint`).
            let materials = model_files::materials(&model_files::json(&file));
            assert!(
                materials
                    .iter()
                    .any(|material| material.split('.').next() == Some(role)),
                "{item}: no material plays {role}, only {materials:?}"
            );
            dyed += 1;
        }
        assert!(dyed > 0, "no colour was checked");
    }

    #[test]
    fn the_stations_and_fires_see_what_someone_is_doing() {
        let mut world = World::new();
        let forge = Vec3::new(3.0, 2000.0, 0.0);
        let bren = Person::new("Bren", Doing::Working(forge), Some("smith"));
        let bren = world.spawn(bren).id();
        let worker = |world: &World| *world.get::<Worker>(bren).unwrap();
        let at_work = Worker {
            name: "Bren",
            trade: Some("smith"),
            working_at: Some(forge),
            effort: Effort::Working,
        };
        assert_eq!(worker(&world), at_work);
        let mut people = world.query::<(&mut Person, &mut Worker)>();
        let (mut person, mut seen) = people.get_mut(&mut world, bren).unwrap();
        person.set_doing(Doing::Following(Entity::PLACEHOLDER), &mut seen);
        let following = Worker {
            working_at: None,
            effort: Effort::Moving,
            ..at_work
        };
        assert_eq!(worker(&world), following);
        let bound = world.spawn(Person::new("Ash", Doing::Bound, None)).id();
        let resting = Worker {
            name: "Ash",
            ..Worker::default()
        };
        assert_eq!(*world.get::<Worker>(bound).unwrap(), resting);
    }

    /// A walk's or jog's pace is metres a second in its model file's own world, through its
    /// armature's scale (`tools/characters/pace.py`), and a body's animation player sits on
    /// that armature's node, whose world scale carries it again: a body at its model's size
    /// going at the clip's own pace plays it at its own speed, and one twice the size at twice
    /// the speed the same. By the player's world scale, base.glb's 1.078 armature played them
    /// at 0.93.
    #[test]
    fn a_body_going_at_its_strides_pace_plays_it_at_its_own_speed_whatever_its_rigs_scale() {
        let file = model_files::json(appearance::BASE);
        let rig = model_files::root_scale(&file);
        assert!(rig > 1.05, "the case this covers: an armature scaled {rig}");
        for clip in ["Walk_Loop", "Jog_Fwd_Loop"] {
            let pace = model_files::pace(&file, clip);
            for size in [1.0, 2.0] {
                // The body turned to face -Z and sized, its model's armature under it.
                let own = Transform::from_scale(Vec3::splat(rig));
                let body = Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                    .with_scale(Vec3::splat(size));
                let place = GlobalTransform::from(body) * own;
                let rate = stride_rate(size * pace, pace, &place, &own);
                assert!(
                    (rate - 1.0).abs() < 0.02,
                    "{clip} at {} m/s, {size} times its model's size: played at {rate}",
                    size * pace
                );
            }
        }
    }

    /// The town's walk and jog paces are the base body's clips' own, and every speed someone
    /// walks or jogs at, here or in a town, is shown within a tenth of its clip (D585): the
    /// walk was played 1.37 times as fast as made, and the town's runners at 0.63.
    #[test]
    fn people_walk_and_jog_within_a_tenth_of_their_clips() {
        let file = model_files::json(appearance::BASE);
        let mut broken = Vec::new();
        for (clip, town) in [
            ("Walk_Loop", terra_town::WALK_PACE),
            ("Jog_Fwd_Loop", terra_town::JOG_PACE),
        ] {
            let pace = model_files::pace(&file, clip);
            assert!(
                (town / pace - 1.0).abs() < 0.01,
                "terra_town's pace for {clip} is {town}, its clip's {pace}: set it to the clip's"
            );
            let spread = terra_town::GAIT_SPREAD;
            for speed in [town * (1.0 - spread), town * (1.0 + spread)] {
                broken.extend(terra_walker::pace::matched(clip, pace, speed).err());
            }
        }
        for (clip, speed) in [("Walk_Loop", WALK_MPS), ("Jog_Fwd_Loop", JOG_MPS)] {
            let pace = model_files::pace(&file, clip);
            broken.extend(terra_walker::pace::matched(clip, pace, speed).err());
        }
        assert!(broken.is_empty(), "{}", broken.join("\n"));
    }
}
