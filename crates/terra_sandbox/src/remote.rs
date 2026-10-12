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
//! - `sandbox.look {at, distance?, around?, height?, aim?}`: holds the camera on an entity
//!   by name, or a point, following it; `around` is degrees round from the model's front,
//!   `aim` how far above it to look, as at a person's chest rather than their feet.
//!   `sandbox.release` lets the camera go.
//! - `sandbox.pause {paused}`: stops or restarts the game's clock, animations included.
//! - `sandbox.shot {path}`: saves a screenshot; the reply comes before the file does.
//! - `sandbox.quit`: closes the sandbox.
//!
//! An experiment adds its own methods with [`method`].

use crate::planet::MainCamera;
use crate::shell::Options;
use bevy::{
    prelude::*,
    remote::{
        BrpError, BrpResult, RemoteMethodSystemId, RemoteMethods, RemotePlugin,
        builtin_methods::parse_some, http::RemoteHttpPlugin,
    },
    render::view::screenshot::{Screenshot, save_to_disk},
    transform::TransformSystems,
    world_serialization::WorldAssetRoot,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// How far from what it looks at the camera stands, unless told.
const LOOK_DISTANCE_M: f32 = 2.5;

pub fn plugin(app: &mut App) {
    let port = app.world().resource::<Options>().remote_port;
    app.add_plugins((
        RemotePlugin::default(),
        RemoteHttpPlugin::default().with_port(port),
    ))
    .init_resource::<HeldCamera>()
    .configure_sets(PostUpdate, CameraHold.before(TransformSystems::Propagate))
    .add_systems(PostUpdate, hold_camera.in_set(CameraHold));
    method(app, "sandbox.find", find);
    method(app, "sandbox.place", place);
    method(app, "sandbox.look", look);
    method(app, "sandbox.release", release);
    method(app, "sandbox.pause", pause);
    method(app, "sandbox.shot", shot);
    method(app, "sandbox.quit", quit);
    info!("remote: listening on localhost:{port}");
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
struct HeldCamera(Option<Hold>);

struct Hold {
    at: Aim,
    aim: f32,
    distance: f32,
    around: f32,
    height: f32,
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

#[derive(Deserialize)]
struct Look {
    at: At,
    aim: Option<f32>,
    distance: Option<f32>,
    around: Option<f32>,
    height: Option<f32>,
}

fn look(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Look {
        at,
        aim,
        distance,
        around,
        height,
    } = parse_some(params)?;
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
    });
    Ok(json!("held"))
}

fn release(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    world.resource_mut::<HeldCamera>().0 = None;
    Ok(json!("released"))
}

/// Keeps the camera where `sandbox.look` asked, after whatever else moved it this frame.
fn hold_camera(
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
    let eye = target + out * hold.distance + up * hold.height;
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

#[derive(Deserialize)]
struct Shot {
    path: String,
}

fn shot(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Shot { path } = parse_some(params)?;
    let path = std::path::absolute(&path).map_err(refused)?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(refused)?;
    }
    std::fs::remove_file(&path).ok();
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()));
    Ok(json!({ "path": path }))
}

fn quit(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    world.write_message(AppExit::Success);
    Ok(json!("closing"))
}
