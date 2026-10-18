//! Set pieces told over the remote (docs/troll-fight.md): a staged figure is a model standing
//! on the ground that does what a script tells it, as a film's actors do their blocking, until
//! minds of their own tell it the same things. Over the remote, `staged.spawn` stands one in
//! the world (a body shaped by a template, dressed, holding looks on its sockets, playing the
//! named clips of its own file or of another on its skeleton); `staged.play` blends it into a
//! clip, looping or once and held on its last pose; `staged.move` walks it on the ground to a
//! point at a speed, a stride clip's playback following the pace so its feet do not slide
//! (`crate::pace`); `staged.face` turns it to a point or another figure at its turn rate; and
//! `staged.impact` raises what a heavy blow raises where it lands (`impact`); `staged.rate`
//! runs a figure's own time slower or faster, its clip and its going, as a blow's hit-stop
//! freezes both fighters for a few frames; and `staged.bend` turns some of its bones over its
//! clip (`pose`). Nothing here
//! names a model, an item or a clip: all of it is in the calls.

mod impact;
mod pose;

use super::fliers::placed;
use crate::planet::Planet;
use crate::remote::{method, refused};
use crate::{appearance, character};
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
}

/// A figure whose model and clips are still loading: what to dress it in once they are in.
#[derive(Component)]
struct Dressing {
    model: Handle<Gltf>,
    clips: Option<Handle<Gltf>>,
    shape: Option<appearance::Appearance>,
    hold: Vec<(String, String)>,
}

/// On a figure's animation player: whose, its clips by name, two of each so a clip played
/// again blends from itself, the stride clips' paces, and what it plays.
#[derive(Component)]
struct Animates {
    staged: Entity,
    clips: HashMap<String, [AnimationNodeIndex; 2]>,
    paces: HashMap<String, f32>,
    playing: Option<(String, usize)>,
    applied: u32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (dress, stage, animate).chain())
        .add_systems(
            PostUpdate,
            pose::bend
                .after(bevy::app::AnimationSystems)
                .before(bevy::transform::TransformSystems::Propagate),
        );
    impact::plugin(app);
    method(app, "staged.spawn", spawn);
    method(app, "staged.play", play);
    method(app, "staged.move", go);
    method(app, "staged.face", face);
    method(app, "staged.rate", rate);
    method(app, "staged.bend", bend);
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
        let mut paces: HashMap<String, f32> =
            crate::pace::paces(model, &nodes).into_iter().collect();
        paces.extend(crate::pace::paces(moves, &nodes));
        let mut graph = AnimationGraph::new();
        let root = graph.root;
        let named: HashMap<String, [AnimationNodeIndex; 2]> = moves
            .named_animations
            .iter()
            .map(|(name, clip)| {
                let a = graph.add_clip(clip.clone(), 1.0, root);
                let b = graph.add_clip(clip.clone(), 1.0, root);
                (name.to_string(), [a, b])
            })
            .collect();
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
        let hold = dressing.hold.clone();
        body.observe(
            move |ready: On<WorldInstanceReady>,
                  mut commands: Commands,
                  assets: Res<AssetServer>,
                  children: Query<&Children>,
                  players: Query<(), With<AnimationPlayer>>,
                  names: Query<&Name>| {
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

/// Each figure's clip as it was last told, blended in; a stride clip's playback at the speed
/// it moves, so its feet keep to the ground.
fn animate(
    figures: Query<&Staged>,
    mut animators: Query<(
        &mut Animates,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &GlobalTransform,
    )>,
) {
    for (mut animates, mut player, mut transitions, place) in &mut animators {
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
                            Some((name, copy)) if *name == asked.clip => 1 - copy,
                            _ => 0,
                        };
                        let active = transitions.play(&mut player, nodes[copy], asked.blend);
                        if !asked.once {
                            active.repeat();
                        }
                        if asked.from_s > 0.0 {
                            active.seek_to(asked.from_s);
                        }
                        animates.playing = Some((asked.clip.clone(), copy));
                    }
                    None => warn!("Staged: {} has no clip {}.", staged.name, asked.clip),
                }
            }
        }
        let Some((name, copy)) = animates.playing.clone() else {
            continue;
        };
        let Some(active) = animates
            .clips
            .get(&name)
            .and_then(|nodes| player.animation_mut(nodes[copy]))
        else {
            continue;
        };
        // As asked, or a stride at the pace it goes; either in the figure's own time.
        let speed = match animates.paces.get(&name) {
            Some(pace) if staged.speed_now > STILL_MPS => {
                crate::pace::rate(staged.speed_now, *pace, place.scale().y)
            }
            _ => staged.asked.as_ref().map_or(1.0, |asked| asked.speed),
        };
        active.set_speed(speed * staged.rate);
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
}

/// The figure called a name, if there is one.
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
fn spawn(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
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
    if !asked.wears.is_empty() {
        entity.insert(character::Outfit(
            asked.wears.iter().map(character::Clothing::new).collect(),
        ));
    }
    let entity = entity.id();
    // Where, in the terms the calls and --at take, and how deep the water is there.
    let local = world.resource::<crate::start::Start>().local(feet);
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
}

/// Blends a figure into a clip: looping, or once and held on its last pose until the next,
/// from its start or so far into it.
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
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use terra_core::{PlanetSpec, Seed};

    /// Ground that rises a metre for every ten east, on a planet's scale too large to curve.
    fn slope(point: Vec3) -> Vec3 {
        Vec3::new(point.x, 1000.0 + point.x * 0.1, point.z)
    }

    #[test]
    fn a_figure_walks_to_its_point_at_its_speed_on_the_ground_and_stops_there() {
        let to = Vec3::new(20.0, 1002.0, 0.0);
        let going = Some(Going { to, speed: 2.0 });
        let (mut at, mut facing) = (slope(Vec3::ZERO), Vec3::X);
        let dt = 1.0 / 60.0;
        let mut frames = 0;
        let mut arrived = false;
        while !arrived && frames < 60 * 30 {
            let next = step(slope, at, facing, going, None, 3.0, dt);
            (at, facing, arrived) = (next.at, next.facing, next.arrived);
            // Always on the ground, wherever along the slope.
            assert!(
                (at.y - (1000.0 + at.x * 0.1)).abs() < 1e-3,
                "off the ground at {at}"
            );
            frames += 1;
        }
        assert!(arrived, "never got there: {at}");
        // About 20 m at 2 m/s (a little more, along the level): ten seconds.
        let seconds = frames as f32 * dt;
        assert!((seconds - 10.0).abs() < 0.2, "took {seconds} s");
        assert!(
            (at.x - 20.0).abs() < 0.01 && at.z.abs() < 0.01,
            "stopped at {at}"
        );
        // Standing, it stays.
        let next = step(slope, at, facing, None, None, 3.0, dt);
        assert!(next.at.distance(at) < 1e-3);
    }

    #[test]
    fn a_figure_turns_to_face_a_point_no_faster_than_it_turns() {
        let here = Vec3::new(0.0, 1000.0, 0.0);
        let flat = |p: Vec3| Vec3::new(p.x, 1000.0, p.z);
        // Facing north (-z), told to face east (+x): a quarter turn at 90° a second.
        let mut facing = Vec3::NEG_Z;
        let turn = 90f32.to_radians();
        let point = Vec3::new(50.0, 1000.0, 0.0);
        let after = |facing: Vec3, seconds: f32| {
            let mut f = facing;
            for _ in 0..(seconds * 60.0) as usize {
                f = step(flat, here, f, None, Some(point), turn, 1.0 / 60.0).facing;
            }
            f
        };
        let half = after(facing, 0.5);
        let angle = half.angle_between(Vec3::NEG_Z).to_degrees();
        assert!(
            (angle - 45.0).abs() < 2.0,
            "turned {angle}° in half a second"
        );
        // It turned the short way, toward the east.
        assert!(half.x > 0.5, "{half}");
        facing = after(facing, 2.0);
        assert!(facing.dot(Vec3::X) > 0.999, "facing {facing}");
    }

    #[test]
    fn moving_with_nothing_to_face_it_turns_its_way() {
        let flat = |p: Vec3| Vec3::new(p.x, 1000.0, p.z);
        let going = Some(Going {
            to: Vec3::new(0.0, 1000.0, 40.0),
            speed: 1.5,
        });
        let mut facing = Vec3::NEG_Z;
        let mut at = flat(Vec3::ZERO);
        for _ in 0..240 {
            let next = step(flat, at, facing, going, None, 3.0, 1.0 / 60.0);
            (at, facing) = (next.at, next.facing);
        }
        assert!(facing.dot(Vec3::Z) > 0.99, "facing {facing}");
    }

    #[test]
    fn clips_fit_a_model_whose_bones_they_move() {
        let id = |path: &[&str]| {
            let names: Vec<Name> = path.iter().map(|n| Name::new(n.to_string())).collect();
            AnimationTargetId::from_names(names.iter())
        };
        let model: HashSet<_> = [
            id(&["root"]),
            id(&["root", "pelvis"]),
            id(&["root", "pelvis", "spine_01"]),
        ]
        .into_iter()
        .collect();
        assert!(fits(
            &model,
            [id(&["root", "pelvis"]), id(&["root", "pelvis", "spine_01"])]
        ));
        // Another skeleton's bones, named alike but under another root, do not.
        assert!(!fits(
            &model,
            [
                id(&["Armature", "pelvis"]),
                id(&["Armature", "pelvis", "spine_01"])
            ]
        ));
        assert!(!fits(&model, []));
    }

    #[test]
    fn a_figure_told_to_go_somewhere_goes_there_on_the_planets_ground_and_faces_another() {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        let ground = planet.ground_at(Vec3::Y * 2100.0);
        let mut world = World::new();
        world.insert_resource(planet.clone());
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(1.0 / 60.0));
        world.insert_resource(time);
        let staged = |name: &str, going, face| Staged {
            name: name.into(),
            going,
            face,
            facing: Vec3::NEG_Z,
            turn: 3.0,
            speed_now: 0.0,
            scale: 1.0,
            asked: None,
            asked_n: 0,
            rate: 1.0,
        };
        let troll_at = planet.ground_at(ground + Vec3::new(6.0, 3.0, 0.0));
        let troll = world
            .spawn((
                staged("troll", None, Face::Way),
                Transform::from_translation(troll_at),
            ))
            .id();
        let to = ground + Vec3::new(0.0, 0.0, 5.0);
        let man = world
            .spawn((
                staged("man", Some(Going { to, speed: 2.0 }), Face::Figure(troll)),
                // A metre over the ground: it is put on it.
                Transform::from_translation(ground + Vec3::Y),
            ))
            .id();
        for _ in 0..240 {
            world.run_system_once(stage).unwrap();
        }
        let at = world.get::<Transform>(man).unwrap();
        assert!(
            planet.distance(at.translation).abs() < 0.01,
            "not on the ground"
        );
        let level = |v: Vec3| v.reject_from(at.translation.normalize());
        assert!(
            level(at.translation - to).length() < 0.05,
            "stopped at {}",
            at.translation
        );
        assert!(world.get::<Staged>(man).unwrap().going.is_none());
        // Strafing past it, it kept the troll in front.
        let to_troll = level(troll_at - at.translation).normalize();
        assert!(at.forward().dot(to_troll) > 0.99, "not facing the troll");
    }
    #[test]
    fn a_figure_whose_time_runs_at_a_quarter_goes_a_quarter_as_far() {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        let ground = planet.ground_at(Vec3::Y * 2100.0);
        let mut world = World::new();
        world.insert_resource(planet.clone());
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(1.0 / 60.0));
        world.insert_resource(time);
        let to = ground + Vec3::new(0.0, 0.0, 50.0);
        let moved = |world: &mut World, rate: f32| {
            let figure = world
                .spawn((
                    Staged {
                        name: "man".into(),
                        going: Some(Going { to, speed: 2.0 }),
                        face: Face::Way,
                        facing: Vec3::Z,
                        turn: 3.0,
                        speed_now: 0.0,
                        scale: 1.0,
                        asked: None,
                        asked_n: 0,
                        rate,
                    },
                    Transform::from_translation(ground),
                ))
                .id();
            // A second at 2 m/s: two metres in its own time at the world's.
            for _ in 0..60 {
                world.run_system_once(stage).unwrap();
            }
            let at = world.get::<Transform>(figure).unwrap().translation;
            world.despawn(figure);
            (at - ground).length()
        };
        let whole = moved(&mut world, 1.0);
        let quarter = moved(&mut world, 0.25);
        assert!((whole - 2.0).abs() < 0.05, "went {whole} m");
        assert!((quarter - 0.5).abs() < 0.02, "went {quarter} m");
    }
}
