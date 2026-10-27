//! Set pieces told over the remote (docs/troll-fight.md): a staged figure is a model standing
//! on the ground that does what a script tells it, as a film's actors do their blocking, until
//! minds of their own tell it the same things. Over the remote, `staged.spawn` stands one in
//! the world (a body shaped by a template, dressed, holding looks on its sockets, playing the
//! named clips of its own file or of another on its skeleton); `staged.play` blends it into a
//! clip, looping or once and held on its last pose; `staged.move` walks it on the ground to a
//! point at a speed, a stride clip's playback following the pace so its feet do not slide
//! (`terra_walker::pace`); `staged.face` turns it to a point or another figure at its turn rate; and
//! `staged.impact` raises what a heavy blow raises where it lands (`impact`), and a piece
//! spawned `burns` keeps a fire at its foot, as a campfire; `staged.rate`
//! runs a figure's own time slower or faster, its clip and its going, as a blow's hit-stop
//! freezes both fighters for a few frames; and `staged.bend` turns some of its bones over its
//! clip (`pose`); a fire it set down is moved clear of the feet of a figure at rest by it
//! (`fireside`); `staged.morph` and `staged.glow` ease its morph targets and its painted glow,
//! as a creature that wakes (`wake`). Nothing here
//! names a model, an item or a clip: all of it is in the calls.

mod fireside;
pub(crate) mod impact;
pub mod pose;
mod quench;
pub mod wake;

use crate::fliers::placed;
use bevy::{
    animation::AnimationTargetId,
    ecs::system::SystemState,
    gltf::{GltfAssetLabel, GltfNode},
    platform::collections::{HashMap, HashSet},
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;
use terra_walker::layer::Nodes;
use terra_walker::{appearance, character};
use terra_world::planet::Planet;
use terra_world::remote::{method, refused};

/// How fast a figure turns when its call says nothing, degrees a second.
const TURN_DEG_S: f32 = 150.0;
/// How long a clip blends into the next when the call says nothing.
const BLEND_MS: u64 = 250;
/// Moving slower than this, metres a second, a figure is standing: its clip plays as asked.
const STILL_MPS: f32 = 0.05;
/// What share of the clips' bones a model must have for them to play on it.
const FITS: f32 = 0.5;

/// A figure on the stage: its name in the calls, where it is going, what it faces, its
/// heading, how fast it turns, how fast it moved last frame, its size against its model's,
/// and the clip it was last told to play, counted so each call is played once.
#[derive(Component)]
pub struct Staged {
    name: String,
    going: Option<Going>,
    face: Face,
    facing: Vec3,
    turn: f32,
    speed_now: f32,
    scale: f32,
    asked: Option<Asked>,
    asked_n: u32,
    /// How fast its own time runs: 1 as the world's, near 0 held in a hit-stop.
    rate: f32,
}

/// What a mind in the game tells a figure, as the calls do (the sparring partner, D405).
impl Staged {
    /// Blends it into a clip, as `staged.play` does.
    pub fn tell_play(&mut self, clip: &str, speed: f32, once: bool, blend_ms: u64, from_s: f32) {
        self.asked = Some(Asked {
            clip: clip.to_string(),
            speed,
            once,
            blend: Duration::from_millis(blend_ms),
            from_s: from_s.max(0.0),
            hold: None,
        });
        self.asked_n += 1;
    }

    /// Blends it into a clip with the arms holding `hold`'s first pose over it (a stance's
    /// hold, `terra_walker::layer`), looping.
    pub fn tell_play_held(&mut self, clip: &str, hold: &str, speed: f32, blend_ms: u64) {
        self.tell_play(clip, speed, false, blend_ms, 0.0);
        self.lay_hold(hold);
    }

    /// Lays `hold`'s first pose over the arms of the clip just told (a reel under the guard).
    pub fn lay_hold(&mut self, hold: &str) {
        if let Some(asked) = &mut self.asked {
            asked.hold = Some(hold.to_string());
        }
    }

    /// Changes the speed of the clip playing without starting it again.
    pub fn set_clip_speed(&mut self, speed: f32) {
        if let Some(asked) = &mut self.asked {
            asked.speed = speed;
        }
    }

    /// Walks it to a point at a speed, or stops it.
    pub fn tell_go(&mut self, to: Option<Vec3>, speed: f32) {
        self.going = to.map(|to| Going { to, speed });
    }

    /// Turns it toward a point.
    pub fn tell_face(&mut self, point: Vec3) {
        self.face = Face::Point(point);
    }

    /// Runs its own time slower or faster, as a hit-stop holds it.
    pub fn tell_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(0.0, 4.0);
    }

    pub fn facing(&self) -> Vec3 {
        self.facing
    }
}

/// Going to a point at a speed, metres a second.
#[derive(Clone, Copy, Debug)]
struct Going {
    to: Vec3,
    speed: f32,
}

/// What a figure turns to face.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Face {
    /// The way it goes, and where it last faced when it stops.
    Way,
    Point(Vec3),
    Figure(Entity),
}

#[derive(Clone, Debug)]
struct Asked {
    clip: String,
    speed: f32,
    once: bool,
    blend: Duration,
    /// Seconds into the clip it starts from, past a slow start.
    from_s: f32,
    /// A clip whose first pose the arms hold over it (`terra_walker::layer`).
    hold: Option<String>,
}

/// A figure whose model and clips are still loading: what to dress it in once they are in.
#[derive(Component)]
struct Dressing {
    model: Handle<Gltf>,
    clips: Option<Handle<Gltf>>,
    shape: Option<appearance::Appearance>,
    hold: Vec<(String, String)>,
}

/// On a figure's animation player: whose, its clips by name, at their places in its graph
/// (two of each so a clip played again blends from itself, and the hold layer's,
/// `terra_walker::layer`), the stride clips' paces, what it plays (which copy, and whether
/// under a hold), and the holds laid.
#[derive(Component)]
struct Animates {
    staged: Entity,
    clips: HashMap<String, Nodes>,
    paces: HashMap<String, f32>,
    playing: Option<(String, usize, bool)>,
    applied: u32,
    holds: Vec<terra_walker::layer::Held>,
}

pub fn plugin(app: &mut App) {
    figures(app);
    app.add_systems(
        Update,
        (
            fireside::keep_clear.after(animate),
            quench::quench,
            quench::unshadow,
            wake::ease,
        ),
    );
    impact::plugin(app);
    method(app, "staged.spawn", spawn);
    method(app, "staged.play", play);
    method(app, "staged.move", go);
    method(app, "staged.face", face);
    method(app, "staged.rate", rate);
    method(app, "staged.bend", bend);
    method(app, "staged.morph", wake::morph);
    method(app, "staged.glow", wake::glow);
}

/// What dresses, moves, animates and bends the figures, without the remote's methods or the
/// fires they keep, so a headless app runs the same systems the game does.
fn figures(app: &mut App) {
    app.add_systems(Update, (dress, stage, animate).chain())
        .add_systems(
            PostUpdate,
            hold.after(bevy::animation::transition::advance_transitions)
                .before(bevy::animation::animate_targets),
        )
        .add_systems(
            PostUpdate,
            pose::bend
                .after(bevy::app::AnimationSystems)
                .before(bevy::transform::TransformSystems::Propagate),
        );
}

/// Whether clips fit a model's skeleton: most of the bones they move are its bones, by the
/// path of names glTF's targets are known by.
fn fits(
    model: &HashSet<AnimationTargetId>,
    moved: impl IntoIterator<Item = AnimationTargetId>,
) -> bool {
    let (mut all, mut found) = (0usize, 0usize);
    for target in moved {
        all += 1;
        found += usize::from(model.contains(&target));
    }
    all > 0 && found as f32 >= all as f32 * FITS
}

/// Each figure dressed once its model and clips are in: its body, the graph of its clips, and
/// what it holds put in its hands as its body comes in.
#[allow(clippy::too_many_arguments)]
fn dress(
    mut commands: Commands,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    worlds: Res<Assets<WorldAsset>>,
    clips: Res<Assets<AnimationClip>>,
    nodes: Res<Assets<GltfNode>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut baked: Option<ResMut<terra_walker::feet::Baked>>,
    waiting: Query<(Entity, &Staged, &Dressing)>,
) {
    for (entity, staged, dressing) in &waiting {
        let moves_file = dressing.clips.as_ref().unwrap_or(&dressing.model);
        if !assets.is_loaded_with_dependencies(&dressing.model)
            || !assets.is_loaded_with_dependencies(moves_file)
        {
            continue;
        }
        let (Some(model), Some(moves)) = (gltfs.get(&dressing.model), gltfs.get(moves_file)) else {
            continue;
        };
        commands.entity(entity).remove::<Dressing>();
        let Some(own) = model.default_scene.clone() else {
            error!("Staged: {}'s model has no scene.", staged.name);
            continue;
        };
        // Its own body, unless the clips are another file's that do not fit its skeleton: then
        // the clips file's own body, which they were made on.
        let bones: HashSet<AnimationTargetId> = worlds
            .get(&own)
            .map(|scene| {
                scene
                    .world
                    .iter_entities()
                    .filter_map(|each| each.get::<AnimationTargetId>().copied())
                    .collect()
            })
            .unwrap_or_default();
        let moved = moves
            .named_animations
            .values()
            .filter_map(|clip| clips.get(clip))
            .flat_map(|clip| clip.curves().keys().copied());
        let scene = if dressing.clips.is_none() || fits(&bones, moved) {
            own
        } else {
            warn!(
                "Staged: {}'s clips do not fit its model's skeleton; it is the clips' own body.",
                staged.name
            );
            match moves.default_scene.clone() {
                Some(scene) => scene,
                None => own,
            }
        };
        let mut paces: HashMap<String, f32> = terra_walker::pace::paces(model, &nodes)
            .into_iter()
            .collect();
        paces.extend(terra_walker::pace::paces(moves, &nodes));
        // Its clips' foot-down markers, which its feet are kept by (`terra_walker::feet`).
        if let Some(baked) = baked.as_mut() {
            baked.register(moves, &nodes);
        }
        let (graph, named) = terra_walker::layer::graph(moves, &nodes);
        let graph = graphs.add(graph);
        let mut body = commands.spawn((
            WorldAssetRoot(scene),
            // The models face +Z; things here face -Z.
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            ChildOf(entity),
        ));
        if let Some(shape) = dressing.shape.clone() {
            body.insert((character::Body, shape));
        }
        let (hold, name) = (dressing.hold.clone(), staged.name.clone());
        body.observe(
            move |ready: On<WorldInstanceReady>,
                  mut commands: Commands,
                  assets: Res<AssetServer>,
                  children: Query<&Children>,
                  (players, drawn): (Query<(), With<AnimationPlayer>>, Query<(), With<Mesh3d>>),
                  names: Query<&Name>| {
                let parts = children
                    .iter_descendants(ready.entity)
                    .filter(|&c| drawn.contains(c));
                info!(
                    "Staged: {name}'s model is in, {} parts drawn.",
                    parts.count()
                );
                if let Some(animator) = children
                    .iter_descendants(ready.entity)
                    .find(|&child| players.contains(child))
                {
                    commands.entity(animator).insert((
                        AnimationGraphHandle(graph.clone()),
                        AnimationTransitions::new(),
                        Animates {
                            staged: entity,
                            clips: named.clone(),
                            paces: paces.clone(),
                            playing: None,
                            applied: 0,
                            holds: Vec::new(),
                        },
                    ));
                }
                for (socket, file) in &hold {
                    match character::find(ready.entity, socket, &children, &names) {
                        Some(hand) => {
                            let look =
                                assets.load(GltfAssetLabel::Scene(0).from_asset(file.clone()));
                            character::hold(&mut commands, look, hand);
                        }
                        None => warn!("Staged: no socket {socket} to hold {file} in."),
                    }
                }
            },
        );
    }
}

/// Where a figure is after a step: on the ground, turned toward what it faces no faster than
/// it turns, and whether it has got where it was going.
#[derive(Debug)]
struct Stepped {
    at: Vec3,
    facing: Vec3,
    arrived: bool,
}

/// A step of `dt` seconds for a figure at `here` heading `facing` (level): on toward where it
/// is going at its speed, stopping there, its feet put on the ground (`ground`, the nearest
/// ground to a point) and turned toward `toward` (its way if none, while it goes) by at most
/// `turn` radians a second, about its up.
fn step(
    ground: impl Fn(Vec3) -> Vec3,
    here: Vec3,
    facing: Vec3,
    going: Option<Going>,
    toward: Option<Vec3>,
    turn: f32,
    dt: f32,
) -> Stepped {
    let up = here.normalize_or(Vec3::Y);
    let mut at = here;
    let mut arrived = false;
    let mut way = None;
    if let Some(going) = going {
        let level = (going.to - here).reject_from(up);
        let left = level.length();
        let stride = going.speed.max(0.0) * dt;
        if left <= stride.max(1e-3) {
            at = here + level;
            arrived = true;
        } else {
            way = Some(level / left);
            at = here + level / left * stride;
        }
    }
    let at = ground(at);
    let up = at.normalize_or(up);
    let wanted = toward
        .map(|point| (point - at).reject_from(up))
        .or(way)
        .map(|w| w.normalize_or_zero())
        .filter(|w| *w != Vec3::ZERO);
    let facing = facing
        .reject_from(up)
        .normalize_or(up.any_orthonormal_vector());
    let facing = match wanted {
        Some(wanted) => {
            let angle = libm::atan2f(facing.cross(wanted).dot(up), facing.dot(wanted));
            let most = turn * dt;
            Quat::from_axis_angle(up, angle.clamp(-most, most)) * facing
        }
        None => facing,
    };
    Stepped {
        at,
        facing,
        arrived,
    }
}

/// Each figure moved by what it was told: on toward where it is going, kept on the ground
/// every frame, turned toward what it faces.
fn stage(
    time: Res<Time>,
    planet: Res<Planet>,
    mut figures: Query<(Entity, &mut Staged, &mut Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let places: HashMap<Entity, Vec3> = figures
        .iter()
        .map(|(entity, _, transform)| (entity, transform.translation))
        .collect();
    for (_, mut staged, mut transform) in &mut figures {
        let here = transform.translation;
        // Its own time: held in a hit-stop, it all but stops where it is.
        let dt = dt * staged.rate;
        let toward = match staged.face {
            Face::Way => None,
            Face::Point(point) => Some(point),
            Face::Figure(other) => places.get(&other).copied(),
        };
        let next = step(
            |point| planet.ground_at(point),
            here,
            staged.facing,
            staged.going,
            toward,
            staged.turn,
            dt,
        );
        if next.arrived {
            staged.going = None;
        }
        // How fast it goes in its own time, so its stride keeps pace once that is slowed too.
        if dt > 1e-6 {
            staged.speed_now = (next.at - here)
                .reject_from(here.normalize_or(Vec3::Y))
                .length()
                / dt;
        }
        staged.facing = next.facing;
        let up = next.at.normalize_or(Vec3::Y);
        *transform = Transform::from_translation(next.at)
            .looking_to(next.facing, up)
            .with_scale(Vec3::splat(staged.scale));
    }
}

/// How fast a figure plays its clip: as asked, or a stride (one with a `pace`) at the speed it
/// goes against that pace. A pace is metres a second in its model file's own world, as
/// `tools/characters/pace.py` measures it through the armature object's scale (the Cetri's
/// rig is 3.3 times its bones', the troll's 2.2), so it is scaled by the figure's size against
/// its model alone: by the animation player's world scale, which carries the armature's own
/// scale again, the Cetri's 0.87 m/s walk asked 0.30 of its speed and played at the slowest,
/// half (round 13, docs/creatures.md).
fn played(staged: &Staged, pace: Option<f32>) -> f32 {
    match pace {
        Some(pace) if staged.speed_now > STILL_MPS => {
            terra_walker::pace::rate(staged.speed_now, pace, staged.scale)
        }
        _ => staged.asked.as_ref().map_or(1.0, |asked| asked.speed),
    }
}

/// Each figure's clip as it was last told, blended in; a stride clip's playback at the speed
/// it moves, so its feet keep to the ground.
fn animate(
    figures: Query<&Staged>,
    mut feet: Query<&mut terra_walker::feet::Kept>,
    mut animators: Query<(
        &mut Animates,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
) {
    for (mut animates, mut player, mut transitions) in &mut animators {
        let Ok(staged) = figures.get(animates.staged) else {
            continue;
        };
        if staged.asked_n != animates.applied {
            animates.applied = staged.asked_n;
            if let Some(asked) = &staged.asked {
                match animates.clips.get(&asked.clip).copied() {
                    Some(nodes) => {
                        // The same clip again blends from itself, by its other copy.
                        let copy = match &animates.playing {
                            Some((name, copy, _)) if *name == asked.clip => 1 - copy,
                            _ => 0,
                        };
                        let hold = asked.hold.as_ref().and_then(|h| animates.clips.get(h));
                        let held = hold.map(|hold| hold.over);
                        let node = nodes.of(copy, held.is_some());
                        let active = transitions.play(&mut player, node, asked.blend);
                        if !asked.once {
                            active.repeat();
                        }
                        if asked.from_s > 0.0 {
                            active.seek_to(asked.from_s);
                        }
                        animates.playing = Some((asked.clip.clone(), copy, held.is_some()));
                        if let Some(over) = held {
                            let holds = &mut animates.holds;
                            terra_walker::layer::lay(holds, &mut player, over, node);
                        }
                    }
                    None => warn!("Staged: {} has no clip {}.", staged.name, asked.clip),
                }
            }
        }
        let Some((name, copy, held)) = animates.playing.clone() else {
            continue;
        };
        let Some(active) = animates
            .clips
            .get(&name)
            .and_then(|nodes| player.animation_mut(nodes.of(copy, held)))
        else {
            continue;
        };
        // As asked, or a stride at the pace it goes; either in the figure's own time.
        let speed = played(staged, animates.paces.get(&name).copied());
        active.set_speed(speed * staged.rate);
        if let Ok(mut kept) = feet.get_mut(animates.staged) {
            let pace = animates.paces.get(&name).copied().unwrap_or(0.0);
            kept.stride(staged.facing, pace * staged.scale, speed * staged.rate, 1.0);
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Spawn {
    name: String,
    model: String,
    template: Option<String>,
    clips: Option<String>,
    #[serde(default)]
    wears: Vec<String>,
    #[serde(default)]
    hold: std::collections::BTreeMap<String, String>,
    at: [f32; 3],
    facing: [f32; 3],
    scale: Option<f32>,
    turn_deg_s: Option<f32>,
    /// Bones turned over its clips (`pose`): degrees about the body's x, y and z.
    #[serde(default)]
    bend: std::collections::BTreeMap<String, [f32; 3]>,
    #[serde(default)]
    base: bool,
    /// Flames kept burning at its foot, so tall, so wide and so hot (`fire::Kindled`), as on
    /// a campfire or a brazier set on the ground.
    burns: Option<[f32; 3]>,
    /// How high over its foot those flames stand, in its model's metres: a torch's head on
    /// its staff (the fire sheet's torch, docs/fire-module.md). On its foot when not given.
    burns_up: Option<f32>,
}

/// The figure called a name, if there is one.
/// Each hold takes the weight of the clips under it (`terra_walker::layer::ride`).
fn hold(mut animators: Query<(&mut Animates, &mut AnimationPlayer)>) {
    for (mut animates, mut player) in &mut animators {
        terra_walker::layer::ride(&mut animates.holds, &mut player);
    }
}

fn figure(world: &mut World, name: &str) -> Option<Entity> {
    let mut state = SystemState::<Query<(Entity, &Staged)>>::new(world);
    let figures = state.get(world).ok()?;
    figures
        .iter()
        .find(|(_, staged)| staged.name == name)
        .map(|(entity, _)| entity)
}

fn named(world: &mut World, name: &str) -> Result<Entity, bevy::remote::BrpError> {
    figure(world, name).ok_or_else(|| refused(format!("there is no figure called {name}")))
}

/// Stands a figure on the ground at a point, facing another; one of the same name goes.
pub fn spawn(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Spawn = parse_some(params)?;
    let at = placed(world, asked.at, asked.base)?;
    let toward = placed(world, asked.facing, asked.base)?;
    let shape = match &asked.template {
        Some(template) => Some(
            appearance::Appearance::template(template)
                .ok_or_else(|| refused(format!("there is no template {template}")))?,
        ),
        None => None,
    };
    if let Some(old) = figure(world, &asked.name) {
        world.despawn(old);
    }
    let feet = world.resource::<Planet>().ground_at(at);
    let up = feet.normalize();
    let facing = (toward - feet)
        .reject_from(up)
        .normalize_or(up.any_orthonormal_vector());
    let scale = asked.scale.unwrap_or(1.0);
    let assets = world.resource::<AssetServer>();
    let dressing = Dressing {
        model: assets.load(asked.model.clone()),
        clips: asked.clips.as_ref().map(|file| assets.load(file.clone())),
        shape,
        hold: asked.hold.into_iter().collect(),
    };
    // A scripted run stands still until it is in (`loading::hold`), so a shot frames on
    // never finds it missing on a busy machine.
    if let Some(mut loading) = world.get_resource_mut::<terra_world::loading::Loading>() {
        loading.wait_for(&dressing.model);
        if let Some(clips) = &dressing.clips {
            loading.wait_for(clips);
        }
    }
    let mut entity = world.spawn((
        Name::new(format!("Staged: {}", asked.name)),
        Staged {
            name: asked.name.clone(),
            going: None,
            face: Face::Way,
            facing,
            turn: asked.turn_deg_s.unwrap_or(TURN_DEG_S).to_radians(),
            speed_now: 0.0,
            scale,
            asked: None,
            asked_n: 0,
            rate: 1.0,
        },
        dressing,
        Transform::from_translation(feet)
            .looking_to(facing, up)
            .with_scale(Vec3::splat(scale)),
        Visibility::default(),
    ));
    if !asked.bend.is_empty() {
        let mut bends = pose::Bends::default();
        bends.tell(&asked.bend, 0.0);
        entity.insert(bends);
    }
    if let Some([tall, wide, heat]) = asked.burns {
        // On the ground it burns on the campfire's logs, the one hearth's model there is to
        // stage a fire on, as the campfire does (the baked body, D493). Raised on a model's
        // head it is a torch: on no logs, its flames round its head's middle (on the campfire's
        // logs at the torch's scale they licked along logs a metre and a half across, a glowing
        // shell round its head), lit by a torch's pitch light and rig as the dungeon's are
        // (`TORCH_LIGHT`, `Rig::Torch`, torch rounds t3 and t4).
        let (body, light, rig, logs) = match asked.burns_up {
            None => (
                terra_world::fire::Body::Baked,
                terra_world::fire::HEARTH_LIGHT,
                terra_world::fire::Rig::Hearth,
                terra_world::fire::Logs::CAMPFIRE,
            ),
            Some(_) => (
                terra_world::fire::Body::Marched,
                terra_world::fire::TORCH_LIGHT,
                terra_world::fire::Rig::Torch,
                terra_world::fire::Logs::NONE,
            ),
        };
        let kindled = terra_world::fire::Kindled {
            tall,
            wide,
            heat,
            logs,
            body,
            light,
            rig,
        };
        match asked.burns_up {
            Some(up) => {
                // Its head black with soot and pitch, not its glTF's pale wrap (`Charred`).
                let held = (quench::HoldsUp, terra_survival::props::Charred);
                entity.insert(held).with_child((
                    Name::new(format!("Staged: {}'s flames", asked.name)),
                    kindled,
                    Transform::from_xyz(0.0, up, 0.0),
                ));
            }
            None => {
                entity.insert(kindled);
            }
        }
    }
    if !asked.wears.is_empty() {
        entity.insert(character::Outfit(
            asked.wears.iter().map(character::Clothing::new).collect(),
        ));
    }
    let entity = entity.id();
    // Where, in the terms the calls and --at take, and how deep the water is there.
    let local = world.resource::<terra_world::start::Start>().local(feet);
    let deep = world.resource::<Planet>().water_radius_m(feet) - feet.length();
    info!(
        "Staged: {} stands {:.1} m east, {:.1} m up and {:.1} m south of the start{}.",
        asked.name,
        local.x,
        local.y,
        local.z,
        if deep > 0.0 {
            format!(", in {deep:.2} m of water")
        } else {
            String::new()
        }
    );
    Ok(json!({ "done": true, "entity": entity.to_bits() }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Play {
    name: String,
    clip: String,
    speed: Option<f32>,
    #[serde(default)]
    once: bool,
    blend_ms: Option<u64>,
    from_s: Option<f32>,
    /// A clip whose first pose the arms hold over it (a stance's hold, `terra_walker::layer`).
    hold: Option<String>,
}

/// Blends a figure into a clip: looping, or once and held on its last pose until the next,
/// from its start or so far into it, with a hold over its arms if asked.
fn play(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Play = parse_some(params)?;
    let entity = named(world, &asked.name)?;
    if let Some(mut staged) = world.get_mut::<Staged>(entity) {
        staged.asked = Some(Asked {
            clip: asked.clip,
            speed: asked.speed.unwrap_or(1.0),
            once: asked.once,
            blend: Duration::from_millis(asked.blend_ms.unwrap_or(BLEND_MS)),
            from_s: asked.from_s.unwrap_or(0.0).max(0.0),
            hold: asked.hold,
        });
        staged.asked_n += 1;
    }
    Ok(json!({ "done": true }))
}

/// What a figure faces, as a call gives it: a point, or another figure by name.
fn facing_of(
    world: &mut World,
    at: Option<[f32; 3]>,
    toward: Option<&str>,
    base: bool,
) -> Result<Option<Face>, bevy::remote::BrpError> {
    if let Some(name) = toward {
        return Ok(Some(Face::Figure(named(world, name)?)));
    }
    at.map(|at| placed(world, at, base).map(Face::Point))
        .transpose()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Move {
    name: String,
    to: [f32; 3],
    speed: f32,
    face: Option<[f32; 3]>,
    toward: Option<String>,
    #[serde(default)]
    base: bool,
}

/// Walks or slides a figure on the ground to a point at a speed, facing a point or a figure
/// as it goes, or its way.
fn go(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Move = parse_some(params)?;
    let entity = named(world, &asked.name)?;
    let to = placed(world, asked.to, asked.base)?;
    let face = facing_of(world, asked.face, asked.toward.as_deref(), asked.base)?;
    let here = world.get::<Transform>(entity).map_or(to, |t| t.translation);
    if let Some(mut staged) = world.get_mut::<Staged>(entity) {
        staged.going = Some(Going {
            to,
            speed: asked.speed,
        });
        staged.face = face.unwrap_or(Face::Way);
    }
    let metres = (to - here).reject_from(here.normalize_or(Vec3::Y)).length();
    Ok(json!({ "done": true, "metres": metres, "seconds": metres / asked.speed.max(1e-3) }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FaceCall {
    name: String,
    at: Option<[f32; 3]>,
    toward: Option<String>,
    #[serde(default)]
    base: bool,
}

/// Turns a figure to face a point, or another figure as it moves, at its turn rate.
fn face(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: FaceCall = parse_some(params)?;
    let entity = named(world, &asked.name)?;
    let Some(face) = facing_of(world, asked.at, asked.toward.as_deref(), asked.base)? else {
        return Err(refused("face needs `at` or `toward`"));
    };
    if let Some(mut staged) = world.get_mut::<Staged>(entity) {
        staged.face = face;
    }
    Ok(json!({ "done": true }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RateCall {
    name: String,
    rate: f32,
}

/// Runs a figure's own time at a rate, its clip and its going: near 0 for a hit-stop, 1 again
/// after.
fn rate(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: RateCall = parse_some(params)?;
    let entity = named(world, &asked.name)?;
    if let Some(mut staged) = world.get_mut::<Staged>(entity) {
        staged.rate = asked.rate.clamp(0.0, 4.0);
    }
    Ok(json!({ "done": true }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BendCall {
    name: String,
    bend: std::collections::BTreeMap<String, [f32; 3]>,
    blend_ms: Option<u64>,
}

/// Turns some of a figure's bones over its clips, blended from how they were turned before;
/// any bent before and not named now blend back to the clip's own pose.
fn bend(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: BendCall = parse_some(params)?;
    let entity = named(world, &asked.name)?;
    let blend = asked.blend_ms.unwrap_or(BLEND_MS) as f32 / 1000.0;
    let mut figure = world.entity_mut(entity);
    match figure.get_mut::<pose::Bends>() {
        Some(mut bends) => bends.tell(&asked.bend, blend),
        None => {
            let mut bends = pose::Bends::default();
            bends.tell(&asked.bend, blend);
            figure.insert(bends);
        }
    }
    Ok(json!({ "done": true }))
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod walk_tests;
