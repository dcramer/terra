//! Live control of a running sandbox (D69), over Bevy's remote protocol, so a script can
//! look and change things without a restart. On with `--remote`; it listens on
//! `localhost:15702`, or the port `--remote-port` gives, so several runs (other sessions'
//! included) are driven apart. `tools/sandbox/remote.ts` is the client.
//!
//! Besides Bevy's own `world.*` methods, the sandbox answers:
//!
//! - `sandbox.find {name}`: entities whose name contains `name`, with where they are.
//! - `sandbox.place {name, translation?, rotation?, scale?}`: sets the local transform of
//!   the entity called exactly `name`; rotation in degrees about X, Y, then Z.
//! - `sandbox.look {at, distance?, around?, height?, aim?, from?}`: holds the camera on an
//!   entity by name, or a point, following it; `around` is degrees round from the model's
//!   front, `aim` how far above it to look, as at a person's chest rather than their feet;
//!   `from` puts the eye at a point in the world instead; `vantage`, six numbers, puts the
//!   eye at the first three and looks at the last three, in metres east, up and south of
//!   the start, as `--vantage` and the kit sheet's log say; with `--vantage-ground` it moves
//!   the run's fixed camera, heights above the ground, as `--vantage` does then.
//!   `sandbox.release` lets the camera go.
//! - `sandbox.pause {paused}`: stops or restarts the game's clock, animations included.
//! - `sandbox.step {frames}`: lets a stopped clock run for so many frames, then stops it.
//! - `sandbox.press {key, frames?, after?}`: holds a key down for so many frames, one by
//!   default, starting so many frames from now, none by default.
//! - `sandbox.unpress`: lets go of every key a script is holding or has queued.
//! - `sandbox.point {x, y}`: moves the pointer to a place in the window, in logical pixels
//!   from its top left, so what is under it is hovered.
//! - `sandbox.click {x, y, button?, frames?}`: moves the pointer there and, the next frame,
//!   presses a mouse button, `left` by default or `right`, for so many frames, one by
//!   default: what a person's click on a button does.
//! - `sandbox.status`: the frame, the game's time, whether it is stopped, frames still to
//!   step, and the number the run's next line will get.
//! - `sandbox.frames {seconds?}`: the frames of the last so many seconds (10, a minute at
//!   most), on the real clock: how many, and the median, 99th percentile and worst of the
//!   time between them, their work, the game's update and the GPU's, in milliseconds.
//! - `sandbox.said {since?, containing?, most?}`: what the run has said, from the line
//!   numbered `since`, only lines containing some text if asked (`record`).
//! - `sandbox.shot {path}`: saves a screenshot; the reply comes before the file does.
//! - `sandbox.card {at, facing, size?, albedo?}`: stands a matte grey card, a reference of
//!   known reflectance, the way a photographer holds one up to judge colour and exposure: its
//!   middle at `at`, metres east, above the ground and south of the start, its face toward
//!   `facing` (east, up, south), `size` metres square (half a metre unless told), its colour
//!   `albedo` in linear light (0.18, an 18% grey card, unless told), rough, not metal. The
//!   material bench stands them beside what it measures (`cargo xtask material`).
//! - `sandbox.quit`: closes the sandbox.
//! - `sandbox.whoami`: the token it was started with (`--remote-token`), so a script can
//!   tell the run it started from another session's on the same port.
//!
//! An experiment adds its own methods with [`method`].

use crate::loading::Screen;
use crate::planet::MainCamera;
use crate::record::Record;
use crate::shell::{self, Options};
use bevy::{
    diagnostic::FrameCount,
    input::InputSystems,
    picking::PickingSystems,
    prelude::*,
    remote::{
        BrpError, BrpResult, RemoteMethodSystemId, RemoteMethods, RemotePlugin,
        builtin_methods::parse_some, http::RemoteHttpPlugin,
    },
    render::view::screenshot::save_to_disk,
    time::TimeSystems,
    transform::TransformSystems,
    world_serialization::WorldAssetRoot,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::VecDeque;

mod pointer;
use pointer::{Pointing, click, drive_pointer, point};

/// How far from what it looks at the camera stands, unless told.
const LOOK_DISTANCE_M: f32 = 2.5;

pub fn plugin(app: &mut App) {
    let options = app.world().resource::<Options>();
    let (port, listen) = (options.remote_port, options.remote);
    let calls = options.calls.clone().map(|path| Calls::read(&path));
    app.add_plugins(RemotePlugin::default());
    // A run given only calls answers no one: its methods are there for its own script.
    if listen {
        app.add_plugins(RemoteHttpPlugin::default().with_port(port));
    }
    if let Some(calls) = calls {
        match calls {
            Ok(calls) => {
                info!("Calls: {} to make", calls.0.len());
                app.insert_resource(calls).add_systems(
                    PreUpdate,
                    make_calls
                        .after(shell::scripted_presses)
                        .after(InputSystems),
                );
            }
            Err(why) => error!("Calls: {why}"),
        }
    }
    app.init_resource::<HeldCamera>()
        .init_resource::<Steps>()
        .init_resource::<Pressed>()
        .init_resource::<Pointing>()
        .init_resource::<Quitting>()
        .configure_sets(PostUpdate, CameraHold.before(TransformSystems::Propagate))
        .add_systems(First, step.before(TimeSystems))
        .add_systems(
            PreUpdate,
            (
                press_keys,
                drive_pointer.before(PickingSystems::ProcessInput),
            )
                .after(InputSystems)
                .after(shell::scripted_presses),
        )
        .init_resource::<Burst>()
        .add_systems(Update, (quit_when_asked, burst))
        .add_systems(PostUpdate, hold_camera.in_set(CameraHold));
    method(app, "sandbox.find", find);
    method(app, "sandbox.place", place);
    method(app, "sandbox.look", look);
    method(app, "sandbox.release", release);
    method(app, "sandbox.pause", pause);
    method(app, "sandbox.step", start_steps);
    method(app, "sandbox.press", press);
    method(app, "sandbox.unpress", unpress);
    method(app, "sandbox.point", point);
    method(app, "sandbox.click", click);
    method(app, "sandbox.status", status);
    method(app, "sandbox.frames", frames);
    method(app, "sandbox.said", said);
    method(app, "sandbox.shot", shot);
    method(app, "sandbox.card", card);
    method(app, "sandbox.quit", quit);
    method(app, "sandbox.whoami", whoami);
    method(app, "world.obstacles", obstacles);
    crate::gpu_memory::remote(app);
    if listen {
        info!("remote: listening on localhost:{port}");
    }
}

/// Where a held camera is placed: after an experiment places its own camera, before
/// transforms are worked out, so the view and what it culls agree. An experiment that moves
/// its camera in `PostUpdate` does so `.before(CameraHold)`.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CameraHold;

/// Adds a remote method, if the remote protocol is on.
pub fn method<M>(
    app: &mut App,
    name: &str,
    handler: impl IntoSystem<In<Option<Value>>, BrpResult, M> + 'static,
) {
    if !app.world().contains_resource::<RemoteMethods>() {
        return;
    }
    let id = app.world_mut().register_system(handler);
    app.world_mut()
        .resource_mut::<RemoteMethods>()
        .insert(name, RemoteMethodSystemId::Instant(id));
}

/// A request that cannot be done, said in words.
pub fn refused(message: impl ToString) -> BrpError {
    BrpError::internal(message)
}

/// The first entity called exactly `name`.
pub fn named(world: &mut World, name: &str) -> Option<Entity> {
    world
        .query::<(Entity, &Name)>()
        .iter(world)
        .find(|(_, each)| each.as_str() == name)
        .map(|(entity, _)| entity)
}

fn triple(value: Vec3) -> [f32; 3] {
    value.to_array()
}

fn degrees(rotation: Quat) -> [f32; 3] {
    let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
    [x.to_degrees(), y.to_degrees(), z.to_degrees()]
}

#[derive(Deserialize)]
struct Find {
    name: String,
}

fn find(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Find { name } = parse_some(params)?;
    let start = world.get_resource::<crate::start::Start>().copied();
    let mut found = Vec::new();
    let mut query = world.query::<(Entity, &Name, Option<&Transform>, Option<&GlobalTransform>)>();
    for (entity, each, local, global) in query.iter(world) {
        if !each.as_str().contains(&name) {
            continue;
        }
        found.push(json!({
            "entity": entity.to_bits(),
            "name": each.as_str(),
            "at": global.map(|global| triple(global.translation())),
            // East, up and south of the start, the terms scripts give places in.
            "from_start": global
                .zip(start)
                .map(|(global, start)| triple(start.local(global.translation()))),
            "translation": local.map(|local| triple(local.translation)),
            "rotation": local.map(|local| degrees(local.rotation)),
            "scale": local.map(|local| triple(local.scale)),
        }));
        if found.len() == 100 {
            break;
        }
    }
    Ok(Value::Array(found))
}

#[derive(Deserialize)]
struct Place {
    name: String,
    translation: Option<[f32; 3]>,
    rotation: Option<[f32; 3]>,
    scale: Option<f32>,
}

fn place(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Place {
        name,
        translation,
        rotation,
        scale,
    } = parse_some(params)?;
    let entity = named(world, &name).ok_or_else(|| refused(format!("nothing called {name}")))?;
    let mut transform = world
        .get_mut::<Transform>(entity)
        .ok_or_else(|| refused(format!("{name} has no transform")))?;
    if let Some(at) = translation {
        transform.translation = Vec3::from_array(at);
    }
    if let Some([x, y, z]) = rotation {
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            x.to_radians(),
            y.to_radians(),
            z.to_radians(),
        );
    }
    if let Some(scale) = scale {
        transform.scale = Vec3::splat(scale);
    }
    Ok(json!({
        "translation": triple(transform.translation),
        "rotation": degrees(transform.rotation),
        "scale": triple(transform.scale),
    }))
}

/// What the camera is held on, if anything.
#[derive(Resource, Default)]
pub struct HeldCamera(Option<Hold>);

impl HeldCamera {
    /// Whether a script holds the camera, taking pictures rather than playing.
    pub fn held(&self) -> bool {
        self.0.is_some()
    }
}

struct Hold {
    at: Aim,
    aim: f32,
    distance: f32,
    around: f32,
    height: f32,
    /// Where the eye is, in the world, when asked for rather than worked out.
    from: Option<Vec3>,
}

enum Aim {
    Entity(Entity),
    Point(Vec3),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum At {
    Name(String),
    Point([f32; 3]),
}

impl Default for At {
    fn default() -> Self {
        At::Point([0.0; 3])
    }
}

#[derive(Deserialize)]
struct Look {
    #[serde(default)]
    at: At,
    vantage: Option<[f32; 6]>,
    aim: Option<f32>,
    distance: Option<f32>,
    around: Option<f32>,
    height: Option<f32>,
    from: Option<[f32; 3]>,
}

fn look(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Look {
        at,
        vantage,
        aim,
        distance,
        around,
        height,
        from,
    } = parse_some(params)?;
    // With --vantage-ground, a vantage is the run's own fixed camera moved, its heights above
    // the ground and its eye out of any trunk, as --vantage gives it.
    if let Some(v) = vantage
        && world.resource::<Options>().vantage_ground
    {
        let view = (Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5]));
        world.resource_mut::<Options>().vantage = Some(view);
        world.resource_mut::<HeldCamera>().0 = None;
        return Ok(json!("vantage"));
    }
    let (at, from) = match vantage {
        Some(v) => {
            let start = world
                .get_resource::<crate::start::Start>()
                .ok_or_else(|| refused("there is no start to stand by"))?;
            let place = |x: f32, y: f32, z: f32| start.transform_point(Vec3::new(x, y, z));
            (
                At::Point(place(v[3], v[4], v[5]).to_array()),
                Some(place(v[0], v[1], v[2]).to_array()),
            )
        }
        None => (at, from),
    };
    let target = match at {
        At::Name(name) => Aim::Entity(
            named(world, &name).ok_or_else(|| refused(format!("nothing called {name}")))?,
        ),
        At::Point(point) => Aim::Point(Vec3::from_array(point)),
    };
    world.resource_mut::<HeldCamera>().0 = Some(Hold {
        at: target,
        aim: aim.unwrap_or(0.0),
        distance: distance.unwrap_or(LOOK_DISTANCE_M),
        around: around.unwrap_or(30.0),
        height: height.unwrap_or(0.3),
        from: from.map(Vec3::from_array),
    });
    Ok(json!("held"))
}

fn release(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    world.resource_mut::<HeldCamera>().0 = None;
    Ok(json!("released"))
}

/// Keeps the camera where `sandbox.look` asked, after whatever else moved it this frame.
pub(crate) fn hold_camera(
    held: Res<HeldCamera>,
    globals: Query<&GlobalTransform>,
    parents: Query<&ChildOf>,
    roots: Query<(), With<WorldAssetRoot>>,
    mut cameras: Query<&mut Transform, With<MainCamera>>,
) {
    let Some(hold) = &held.0 else { return };
    let (target, front) = match hold.at {
        Aim::Point(point) => (point, None),
        Aim::Entity(entity) => {
            let Ok(global) = globals.get(entity) else {
                return;
            };
            // The front of the model it belongs to: its root's +Z, as glTF models face.
            let root = parents
                .iter_ancestors(entity)
                .find(|ancestor| roots.contains(*ancestor));
            let front = root
                .and_then(|root| globals.get(root).ok())
                .map(|root| root.rotation() * Vec3::Z);
            (global.translation(), front)
        }
    };
    // The planet's centre is the origin, so up is away from it.
    let up = target.normalize_or(Vec3::Y);
    let target = target + up * hold.aim;
    let front = front
        .map(|front| front.reject_from(up))
        .filter(|front| front.length() > 1e-3)
        .unwrap_or_else(|| up.any_orthonormal_vector())
        .normalize();
    let side = up.cross(front);
    let turn = hold.around.to_radians();
    let out = front * libm::cosf(turn) + side * libm::sinf(turn);
    let eye = hold
        .from
        .unwrap_or(target + out * hold.distance + up * hold.height);
    for mut camera in &mut cameras {
        *camera = Transform::from_translation(eye).looking_at(target, up);
    }
}

#[derive(Deserialize)]
struct Pause {
    paused: bool,
}

fn pause(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Pause { paused } = parse_some(params)?;
    let mut time = world.resource_mut::<Time<Virtual>>();
    if paused {
        time.pause();
    } else {
        time.unpause();
    }
    Ok(json!({ "paused": paused }))
}

/// Frames left to run before the clock stops again, after `sandbox.step`.
#[derive(Resource, Default)]
struct Steps {
    left: u32,
    stepping: bool,
}

#[derive(Deserialize)]
struct Step {
    frames: u32,
}

fn start_steps(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Step { frames } = parse_some(params)?;
    world.resource_mut::<Time<Virtual>>().pause();
    *world.resource_mut::<Steps>() = Steps {
        left: frames,
        stepping: true,
    };
    Ok(json!({ "frames": frames }))
}

/// Runs the clock for the frames asked for, then stops it, before the frame's time is
/// worked out.
fn step(mut steps: ResMut<Steps>, mut time: ResMut<Time<Virtual>>) {
    if steps.left > 0 {
        time.unpause();
        steps.left -= 1;
    } else if steps.stepping {
        time.pause();
        steps.stepping = false;
    }
}

/// Keys a script has pressed: how many frames until each goes down, then how many it is
/// held for.
#[derive(Resource, Default)]
struct Pressed(Vec<Held>);

struct Held {
    key: KeyCode,
    wait: u32,
    left: u32,
    /// Whether it has been held down for a frame already.
    down: bool,
}

#[derive(Deserialize)]
struct Press {
    key: String,
    frames: Option<u32>,
    after: Option<u32>,
}

fn press(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Press { key, frames, after } = parse_some(params)?;
    let code = shell::parse_key(&key)
        .ok_or_else(|| refused(format!("wanted {}, not `{key}`", shell::KEYS)))?;
    let frames = frames.unwrap_or(1).max(1);
    let after = after.unwrap_or(0);
    world.resource_mut::<Pressed>().0.push(Held {
        key: code,
        wait: after,
        left: frames,
        down: false,
    });
    Ok(json!({ "key": key, "after": after, "frames": frames }))
}

fn unpress(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let held = std::mem::take(&mut world.resource_mut::<Pressed>().0);
    let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
    for each in &held {
        keys.release(each.key);
    }
    Ok(json!({ "let_go": held.len() }))
}

/// Holds down what a script pressed, after the real keyboard is read and, in a scripted
/// run, after its own presses, and lets each key up once its frames are done, unless
/// another press is holding it. A key is newly pressed only on its first frame: an
/// unattended run lets every key up each frame before this, so without saying so a key
/// held down would be pressed again every frame.
fn press_keys(mut pressed: ResMut<Pressed>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    let mut done = Vec::new();
    pressed.0.retain_mut(|held| {
        if held.wait > 0 {
            held.wait -= 1;
        } else if held.left > 0 {
            keys.press(held.key);
            if held.down {
                keys.clear_just_pressed(held.key);
            }
            held.down = true;
            held.left -= 1;
        } else {
            done.push(held.key);
            return false;
        }
        true
    });
    for key in done {
        let still = pressed
            .0
            .iter()
            .any(|held| held.key == key && held.wait == 0);
        if !still {
            keys.release(key);
        }
    }
}

fn status(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let time = world.resource::<Time<Virtual>>();
    let (_, next) = world
        .get_resource::<Record>()
        .map_or((Vec::new(), 0), |record| record.read(u64::MAX, None, 0));
    Ok(json!({
        "frame": world.resource::<FrameCount>().0,
        "seconds": time.elapsed_secs_f64(),
        "paused": time.is_paused(),
        "loading": *world.resource::<State<Screen>>().get() == Screen::Loading,
        "stepping": world.resource::<Steps>().left,
        "next_line": next,
    }))
}

#[derive(Deserialize)]
struct Recent {
    seconds: Option<f32>,
}

/// How the frames of the last few seconds went (`debug::recent`).
fn frames(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Recent { seconds } = parse_some(params.or(Some(json!({}))))?;
    let (count, spreads) = crate::debug::recent(world, seconds.unwrap_or(10.0))
        .ok_or_else(|| refused("no frames are timed in this run"))?;
    let spread = |spread: Option<crate::debug::Spread>| {
        spread.map_or(
            Value::Null,
            |s| json!({ "median": s.median, "p99": s.high, "worst": s.worst }),
        )
    };
    let [frame, work, game, gpu] = spreads.map(spread);
    Ok(json!({ "frames": count, "frame": frame, "work": work, "game": game, "gpu": gpu }))
}

#[derive(Deserialize)]
struct Said {
    since: Option<u64>,
    containing: Option<String>,
    most: Option<usize>,
}

fn said(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Said {
        since,
        containing,
        most,
    } = parse_some(params.or(Some(json!({}))))?;
    let record = world
        .get_resource::<Record>()
        .ok_or_else(|| refused("this run keeps no record"))?;
    let (lines, next) = record.read(
        since.unwrap_or(0),
        containing.as_deref(),
        most.unwrap_or(200),
    );
    let lines: Vec<Value> = lines
        .into_iter()
        .map(|line| {
            json!({
                "line": line.number,
                "frame": line.frame,
                "level": line.level.as_str(),
                "source": line.source,
                "text": line.text,
            })
        })
        .collect();
    Ok(json!({ "lines": lines, "next_line": next }))
}

#[derive(Deserialize)]
struct Shot {
    path: String,
    /// So many frames one after another, each to the path with its number before the
    /// extension (`f.png` as `f-00000.png` on), to compare frame by frame what a change
    /// does on screen as it happens: what pops or flickers, or a frame with a gap.
    #[serde(default)]
    frames: Option<u32>,
}

/// Frames being saved one after another (`sandbox.shot` with `frames`): where, the next's
/// number and how many.
#[derive(Resource, Default)]
struct Burst {
    path: Option<std::path::PathBuf>,
    next: u32,
    count: u32,
}

/// Saves this frame, while a burst of frames is being saved.
fn burst(
    mut commands: Commands,
    mut burst: ResMut<Burst>,
    picture: Option<Res<crate::shell::Picture>>,
) {
    let Some(path) = burst.path.clone() else {
        return;
    };
    if burst.next >= burst.count {
        burst.path = None;
        return;
    }
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    let file = path.with_file_name(format!("{stem}-{:05}.png", burst.next));
    std::fs::remove_file(&file).ok();
    commands
        .spawn(crate::shell::screenshot(picture.as_deref()))
        .observe(save_to_disk(file));
    burst.next += 1;
}

fn shot(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Shot { path, frames } = parse_some(params)?;
    let path = std::path::absolute(&path).map_err(refused)?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(refused)?;
    }
    if let Some(count) = frames {
        let count = count.min(600);
        *world.resource_mut::<Burst>() = Burst {
            path: Some(path.clone()),
            next: 0,
            count,
        };
        return Ok(json!({ "path": path, "frames": count }));
    }
    std::fs::remove_file(&path).ok();
    let shot = crate::shell::screenshot(world.get_resource::<crate::shell::Picture>());
    world.spawn(shot).observe(save_to_disk(path.clone()));
    Ok(json!({ "path": path }))
}

#[derive(Deserialize)]
struct Card {
    at: [f32; 3],
    facing: [f32; 3],
    size: Option<f32>,
    albedo: Option<f32>,
}

/// Stands a grey card (`sandbox.card`): a thin slab, so it casts a shadow and is lit only on
/// the face it shows.
fn card(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Card {
        at,
        facing,
        size,
        albedo,
    } = parse_some(params)?;
    let start = *world
        .get_resource::<crate::start::Start>()
        .ok_or_else(|| refused("there is no start to stand by"))?;
    let planet = world
        .get_resource::<crate::planet::Planet>()
        .ok_or_else(|| refused("there is no ground to stand on"))?;
    let ground = planet.ground_at(start.transform_point(Vec3::new(at[0], 0.0, at[2])));
    let up = ground.normalize();
    let middle = ground + up * at[1];
    let facing = (start.rotation * Vec3::from_array(facing)).normalize_or_zero();
    if facing == Vec3::ZERO {
        return Err(refused(
            "a card faces some way: `facing` is three numbers, not nothing",
        ));
    }
    // A cuboid's thin side is its z: looking along the reverse of its facing puts +z, its face,
    // toward it. Upright unless it faces up or down.
    let hint = if facing.dot(up).abs() > 0.99 {
        start.rotation * Vec3::Z
    } else {
        up
    };
    let rotation = Transform::IDENTITY.looking_to(-facing, hint).rotation;
    let size = size.unwrap_or(0.5).clamp(0.01, 10.0);
    let albedo = albedo.unwrap_or(0.18).clamp(0.0, 1.0);
    let mesh = world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(size, size, 0.01));
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::linear_rgb(albedo, albedo, albedo),
            perceptual_roughness: 1.0,
            metallic: 0.0,
            ..default()
        });
    let entity = world
        .spawn((
            Name::new(format!("grey card {:.0}%", albedo * 100.0)),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(middle).with_rotation(rotation),
        ))
        .id();
    Ok(json!({ "entity": entity.to_bits(), "at": triple(middle) }))
}

#[derive(Deserialize)]
struct Near {
    at: [f32; 2],
    within: f32,
}

/// What stops a walker or holds one up within so far of a spot, east and south of the start,
/// nearest first (D243, D267): each thing's middle, east and south; how far round it reaches;
/// how far the spot is from its middle, `apart`, and from the edge of its nearest piece too
/// tall to step onto, `gap` (less than nothing inside it, none if it has no such piece);
/// its tallest piece's top over the ground, if any, and how many pieces it has.
fn obstacles(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Near { at, within } = parse_some(params)?;
    let start = *world.resource::<crate::start::Start>();
    let planet = world.resource::<crate::planet::Planet>().clone();
    let centre = planet.ground_at(start.at(at[0], at[1]));
    let store = world.resource::<crate::flora::Obstacles>();
    let mut found: Vec<(f32, Value)> = store
        .solids_near(centre, within)
        .filter_map(|solid| {
            let local = start.local(solid.at);
            let apart = Vec2::new(local.x - at[0], local.z - at[1]).length();
            let (plan, _) = solid.plan(centre);
            let gap = solid
                .pieces
                .iter()
                .map(|piece| piece.distance(plan).0)
                .reduce(f32::min);
            (gap.unwrap_or(apart - solid.reach) <= within).then(|| {
                (
                    gap.unwrap_or(apart),
                    json!({
                        "at": [local.x, local.z],
                        "radius": solid.reach,
                        "apart": apart,
                        "gap": gap,
                        "top": solid.top().map(|top| top.min(99.0)),
                        "pieces": solid.pieces.len(),
                    }),
                )
            })
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(Value::Array(
        found.into_iter().map(|(_, v)| v).take(200).collect(),
    ))
}

/// Asked to close. Requests are answered after the frame's last systems, too late for
/// anything that does its work as the game closes, such as saving, so the game closes on
/// the next frame, the way it would if its window were closed.
#[derive(Resource, Default)]
struct Quitting(bool);

fn quit(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    world.resource_mut::<Quitting>().0 = true;
    Ok(json!("closing"))
}

fn whoami(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    Ok(json!({ "token": world.resource::<Options>().remote_token }))
}

fn quit_when_asked(quitting: Res<Quitting>, mut exit: MessageWriter<AppExit>) {
    if quitting.0 {
        exit.write(AppExit::Success);
    }
}

/// Remote calls a scripted run makes on given frames (`--calls`), soonest first: a script
/// that builds, harms and moves the camera on the frames it says, so a run with `--shots`
/// is the same every time, and one with `--bench` times the same thing as it happens
/// (tools/sandbox/qa.ts). Each is made through the method the remote protocol answers, so
/// a scripted film and a script driving a run live do the same thing.
#[derive(Resource, Default, Debug)]
pub struct Calls(VecDeque<Call>);

#[derive(Deserialize, Debug, PartialEq)]
struct Call {
    frame: u32,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

impl Calls {
    /// The calls in a file, one a line as JSON, blank lines left out; in frame order, and
    /// in the file's order within a frame.
    fn read(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self, String> {
        let mut calls = Vec::new();
        for (number, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let call: Call = serde_json::from_str(line)
                .map_err(|error| format!("line {}: {error}", number + 1))?;
            calls.push(call);
        }
        calls.sort_by_key(|call| call.frame);
        Ok(Self(calls.into()))
    }
}

/// Moving the camera is said nowhere, as it is called every frame of a film.
const UNSAID: [&str; 3] = ["sandbox.look", "sandbox.release", "build.look"];

/// Makes the calls due by this frame, saying what each did, and warning of each refused.
fn make_calls(world: &mut World) {
    let frame = world.resource::<shell::ScriptFrame>().0;
    if frame == 0 {
        return;
    }
    loop {
        let due = world
            .resource::<Calls>()
            .0
            .front()
            .is_some_and(|call| call.frame <= frame);
        if !due {
            break;
        }
        let Some(call) = world.resource_mut::<Calls>().0.pop_front() else {
            break;
        };
        let handler = world.resource::<RemoteMethods>().get(&call.method).copied();
        let Some(RemoteMethodSystemId::Instant(id)) = handler else {
            warn!("Calls: frame {frame}: there is no method {}", call.method);
            continue;
        };
        match world.run_system_with(id, call.params) {
            Ok(Ok(answer)) => {
                let (said, refused) = summary(&answer);
                if let Some(why) = refused {
                    warn!(
                        "Calls: frame {frame}: {}: {said}: refused: {why}",
                        call.method
                    );
                } else if !UNSAID.contains(&call.method.as_str()) {
                    info!("Calls: frame {frame}: {}: {said}", call.method);
                }
            }
            Ok(Err(error)) => warn!(
                "Calls: frame {frame}: {}: refused: {}",
                call.method, error.message
            ),
            Err(error) => warn!("Calls: frame {frame}: {}: {error}", call.method),
        }
    }
}

/// What an answer says was done, in a few words, and why the first thing refused was, if
/// one was: building's answers are each `{done, what, refused?}`, one or a list of them.
fn summary(answer: &Value) -> (String, Option<String>) {
    let each: Vec<&Value> = match answer {
        Value::Array(list) => list.iter().collect(),
        other => vec![other],
    };
    let refused = each
        .iter()
        .find(|each| each.get("done") == Some(&Value::Bool(false)))
        .map(|each| {
            each.get("refused")
                .and_then(Value::as_str)
                .unwrap_or("no reason given")
                .to_string()
        });
    let what: Vec<&str> = each
        .iter()
        .filter_map(|each| each.get("what").and_then(Value::as_str))
        .collect();
    let said = match what.as_slice() {
        [] if matches!(answer, Value::Array(list) if list.is_empty()) => "nothing to do".into(),
        [] => {
            let text = answer.to_string();
            text.chars().take(160).collect()
        }
        [one] => one.to_string(),
        [first, .., last] => format!("{} steps, {first} to {last}", what.len()),
    };
    (said, refused)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_turns_by_degrees_and_says_the_turn_it_was_given() {
        let mut world = World::new();
        let thing = world.spawn((Name::new("thing"), Transform::default())).id();
        let turn = |world: &mut World, degrees: [f32; 3]| {
            let asked = json!({ "name": "thing", "rotation": degrees });
            let reply = world
                .run_system_cached_with(place, Some(asked))
                .unwrap()
                .unwrap();
            let said: [f32; 3] = serde_json::from_value(reply["rotation"].clone()).unwrap();
            (world.get::<Transform>(thing).unwrap().rotation, said)
        };
        // A quarter turn about Y takes the front, +Z, to +X; about X, up to +Z.
        let (about_y, _) = turn(&mut world, [0.0, 90.0, 0.0]);
        assert!((about_y * Vec3::Z).distance(Vec3::X) < 1e-5);
        let (about_x, _) = turn(&mut world, [90.0, 0.0, 0.0]);
        assert!((about_x * Vec3::Y).distance(Vec3::Z) < 1e-5);
        // A turn about all three is said back as it was given, by place and by find.
        let asked = [30.0, -50.0, 70.0];
        let (_, said) = turn(&mut world, asked);
        let found = world
            .run_system_cached_with(find, Some(json!({ "name": "thing" })))
            .unwrap()
            .unwrap();
        let found: [f32; 3] = serde_json::from_value(found[0]["rotation"].clone()).unwrap();
        for (axis, want) in asked.into_iter().enumerate() {
            assert!((said[axis] - want).abs() < 1e-3, "{said:?}");
            assert!((found[axis] - want).abs() < 1e-3, "{found:?}");
        }
    }

    #[derive(Resource, Default)]
    struct Heard(Vec<(u32, String)>);

    fn note(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
        let frame = world.resource::<shell::ScriptFrame>().0;
        let said = params.map_or(String::new(), |p| p["say"].as_str().unwrap().to_string());
        world.resource_mut::<Heard>().0.push((frame, said.clone()));
        Ok(json!({ "done": true, "what": said }))
    }

    #[test]
    fn calls_are_made_on_their_frames_in_the_order_written_within_a_frame() {
        let mut world = World::new();
        let id = world.register_system(note);
        let mut methods = RemoteMethods::new();
        methods.insert("test.note", RemoteMethodSystemId::Instant(id));
        world.insert_resource(methods);
        world.init_resource::<Heard>();
        world.init_resource::<shell::ScriptFrame>();
        // Out of order, two on one frame, a blank line, and a method no one answers.
        let calls = Calls::parse(concat!(
            r#"{"frame": 3, "method": "test.note", "params": {"say": "c"}}"#,
            "\n\n",
            r#"{"frame": 1, "method": "test.note", "params": {"say": "a"}}"#,
            "\n",
            r#"{"frame": 2, "method": "no.such"}"#,
            "\n",
            r#"{"frame": 1, "method": "test.note", "params": {"say": "b"}}"#,
        ))
        .unwrap();
        world.insert_resource(calls);
        for frame in 0..=4 {
            world.resource_mut::<shell::ScriptFrame>().0 = frame;
            world.run_system_cached(make_calls).unwrap();
        }
        let heard = &world.resource::<Heard>().0;
        let want = [(1, "a"), (1, "b"), (3, "c")];
        assert_eq!(heard.len(), want.len(), "{heard:?}");
        for ((frame, said), (want_frame, want_said)) in heard.iter().zip(want) {
            assert_eq!((*frame, said.as_str()), (want_frame, want_said));
        }
        assert!(world.resource::<Calls>().0.is_empty());
        assert!(Calls::parse("{\"frame\": 1}").is_err());
    }
}
