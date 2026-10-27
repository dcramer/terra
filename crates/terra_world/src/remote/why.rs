//! `sandbox.why {entity, component?}`: what changed this? (D548). Who spawned an entity and,
//! for each of its components (or those named), the line of code that last changed it and
//! on which frame: the data breakpoint's question, asked after the fact, as Godot's remote
//! inspector and Unity's and Unreal's debuggers let a person ask of a running game.
//!
//! Bevy records the place in the source of every spawn, insert and mutable borrow of a
//! component when built with its `track_location` feature, which the sandbox's dev builds
//! turn on (`terra_sandbox`'s `track_location`) and its release and bench builds never do.
//! Without it the answer still gives each change's frame, and says the lines are not kept.
//!
//! Bevy stamps a change with a tick, which counts system runs, not frames; [`FrameTicks`]
//! keeps the tick each frame began on, so a tick is told as the frame it fell in.
//!
//! A body drawn between ticks (`tick::Smoothed`) has its `Transform` last written by that
//! drawing each frame, so its answer also says what last moved where it truly stands: the
//! system in a tick, or what moved it outside the ticks, such as a teleport.

use crate::tick::Smoothed;
use bevy::{
    diagnostic::FrameCount,
    ecs::{
        change_detection::{DetectChanges, MaybeLocation, Tick},
        system::SystemChangeTick,
    },
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::panic::Location;

/// How many frames' first ticks are kept: ten minutes at 60 frames a second.
const FRAMES_KEPT: usize = 36_000;

/// The world's change tick at the start of each recent frame, oldest first.
#[derive(Resource, Default)]
pub(super) struct FrameTicks(VecDeque<(u64, u32)>);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FrameTicks>()
        .add_systems(First, note_frame);
    super::method(app, "sandbox.why", why);
}

fn note_frame(ticks: SystemChangeTick, frame: Res<FrameCount>, mut kept: ResMut<FrameTicks>) {
    if kept.0.len() == FRAMES_KEPT {
        kept.0.pop_front();
    }
    kept.0
        .push_back((u64::from(frame.0), ticks.this_run().get()));
}

/// The frame a tick fell in: the newest frame that began at or before it, measured back
/// from `now` so the count wrapping round does not matter. None if it is older than every
/// frame kept, such as a change made while the app was built.
fn frame_of(frames: &VecDeque<(u64, u32)>, now: u32, tick: u32) -> Option<u64> {
    let age = now.wrapping_sub(tick);
    frames
        .iter()
        .rev()
        .find(|(_, start)| now.wrapping_sub(*start) >= age)
        .map(|(frame, _)| *frame)
}

/// A place in the source as it reads best: a workspace file as the workspace names it, a
/// crate's from the registry from its folder (`bevy_transform-0.20.0/src/systems.rs`), and
/// with `/` between folders, as a Windows build names them `\`.
fn short_path(file: &str) -> String {
    let file = file.replace('\\', "/");
    match file.find("/registry/src/") {
        Some(at) => {
            let rest = &file[at + "/registry/src/".len()..];
            // Past the index's own folder, `index.crates.io-<hash>/`.
            rest.split_once('/')
                .map_or(rest, |(_, path)| path)
                .to_string()
        }
        None => file,
    }
}

fn place(location: Option<&'static Location<'static>>) -> Value {
    location.map_or(
        Value::Null,
        |at| json!({ "file": short_path(at.file()), "line": at.line() }),
    )
}

/// How well a component's type path answers what was asked: its whole path, or its name
/// without the path in any case (`Transform`, `transform`), before a part of it (`Trans`).
fn rank(path: &str, short: &str, wanted: &str) -> u8 {
    if path == wanted {
        3
    } else if short.eq_ignore_ascii_case(wanted) {
        2
    } else if path.to_lowercase().contains(&wanted.to_lowercase()) {
        1
    } else {
        0
    }
}

#[derive(Deserialize)]
struct Why {
    /// An entity's id, as `sandbox.find` and Bevy's `world.*` methods give it, or a name.
    entity: Value,
    component: Option<String>,
}

/// The entity an id or a name means: called exactly that, then in any case, then whose name
/// contains it, as `sandbox.find` matches; several such are refused, naming them.
fn entity(world: &mut World, wanted: &Value) -> Result<Entity, String> {
    if let Some(bits) = wanted.as_u64() {
        return Entity::try_from_bits(bits)
            .filter(|entity| world.get_entity(*entity).is_ok())
            .ok_or_else(|| format!("no entity {bits}"));
    }
    let Some(name) = wanted.as_str() else {
        return Err("entity is an id or a name".into());
    };
    let named: Vec<(Entity, String)> = world
        .query::<(Entity, &Name)>()
        .iter(world)
        .map(|(entity, each)| (entity, each.as_str().to_string()))
        .collect();
    let exact = named.iter().filter(|(_, each)| each == name);
    let any_case = named
        .iter()
        .filter(|(_, each)| each.eq_ignore_ascii_case(name));
    let within = named.iter().filter(|(_, each)| each.contains(name));
    for found in [
        exact.collect::<Vec<_>>(),
        any_case.collect(),
        within.collect(),
    ] {
        match found.as_slice() {
            [] => continue,
            [(entity, _)] => return Ok(*entity),
            several => {
                let listed: Vec<String> = several
                    .iter()
                    .take(10)
                    .map(|(entity, each)| format!("{each} ({})", entity.to_bits()))
                    .collect();
                return Err(format!(
                    "{} entities are called {name}: {}; ask by id",
                    several.len(),
                    listed.join(", ")
                ));
            }
        }
    }
    Err(format!("no entity is called {name}"))
}

fn why(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Why {
        entity: wanted,
        component,
    } = parse_some(params)?;
    let entity = entity(world, &wanted).map_err(super::refused)?;
    let at = world.entity(entity);
    let (spawned_by, spawn_tick) = (at.spawned_by().into_option(), at.spawn_tick());
    let name = at.get::<Name>().map(|name| name.as_str().to_string());
    let mut parts: Vec<_> = world
        .inspect_entity(entity)
        .map_err(super::refused)?
        .map(|(id, info)| {
            let path = info.name().to_string();
            let short = info.name().shortname().to_string();
            (id, path, short, info.mutable())
        })
        .collect();
    if let Some(wanted) = &component {
        let best = parts
            .iter()
            .map(|(_, path, short, _)| rank(path, short, wanted))
            .max()
            .unwrap_or(0);
        if best == 0 {
            let has: Vec<&str> = parts
                .iter()
                .map(|(_, _, short, _)| short.as_str())
                .collect();
            return Err(super::refused(format!(
                "it has no component called {wanted}; it has {}",
                has.join(", ")
            )));
        }
        parts.retain(|(_, path, short, _)| rank(path, short, wanted) == best);
    }
    let mut changes = Vec::new();
    for (id, path, short, mutable) in parts {
        let ticks = world.entity(entity).get_change_ticks_by_id(id);
        // Bevy keeps the line for every component, but hands it out without `unsafe` only
        // through a mutable borrow, which a component that cannot change refuses; reading it
        // marks nothing changed.
        let by = if mutable {
            world
                .get_mut_by_id(entity, id)
                .and_then(|borrowed| borrowed.changed_by().into_option())
        } else {
            None
        };
        changes.push((id, path, short, mutable, ticks, by));
    }
    let smoothed = world.entity(entity).get::<Smoothed>().copied();
    let transform = world.components().component_id::<Transform>();

    let now = world.read_change_tick().get();
    let frames = &world.resource::<FrameTicks>().0;
    let current = frames.back().map(|(frame, _)| *frame);
    let when = |tick: Tick| {
        let frame = frame_of(frames, now, tick.get());
        json!({ "frame": frame, "ago": frame.zip(current).map(|(frame, current)| current - frame) })
    };
    let mut components: Vec<Value> = changes
        .into_iter()
        .map(|(id, path, short, mutable, ticks, by)| {
            let mut part = json!({
                "name": short,
                "type": path,
                "by": place(by),
                "changed": ticks.map(|ticks| when(ticks.changed)),
                "added": ticks.map(|ticks| when(ticks.added)),
                "immutable": !mutable,
            });
            // Drawn between ticks: what last moved where it truly stands, too.
            if let Some(smoothed) = smoothed.filter(|_| Some(id) == transform) {
                part["drawn"] = json!(true);
                part["moved"] = smoothed.moved().map_or(Value::Null, |moved| {
                    json!({
                        "by": place(moved.by.into_option()),
                        "changed": when(moved.tick),
                        "in_tick": moved.in_tick,
                    })
                });
            }
            part
        })
        .collect();
    components.sort_by_key(|part| part["changed"]["ago"].as_u64().unwrap_or(u64::MAX));
    Ok(json!({
        "entity": entity.to_bits(),
        "name": name,
        "tracked": MaybeLocation::caller().into_option().is_some(),
        "frame": current,
        "kept": frames.len(),
        "spawned": { "by": place(spawned_by), "when": when(spawn_tick) },
        "components": components,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tick_is_told_as_the_frame_it_fell_in_across_the_count_wrapping() {
        // Frames 10, 11 and 12 begin 40 ticks apart, the count wrapping round in frame 11.
        let start = u32::MAX - 50;
        let frames: VecDeque<(u64, u32)> = (0..3)
            .map(|i| (10 + i as u64, start.wrapping_add(40 * i)))
            .collect();
        let now = start.wrapping_add(100);
        assert_eq!(frame_of(&frames, now, start), Some(10));
        assert_eq!(frame_of(&frames, now, start.wrapping_add(39)), Some(10));
        assert_eq!(frame_of(&frames, now, start.wrapping_add(41)), Some(11));
        assert_eq!(frame_of(&frames, now, start.wrapping_add(80)), Some(12));
        assert_eq!(frame_of(&frames, now, start.wrapping_sub(1)), None);
    }

    #[test]
    fn paths_read_as_the_workspace_and_the_registry_name_them() {
        assert_eq!(
            short_path("crates/terra_walker/src/walker.rs"),
            "crates/terra_walker/src/walker.rs"
        );
        assert_eq!(
            short_path(
                "/home/a/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_transform-0.20.0/src/systems.rs"
            ),
            "bevy_transform-0.20.0/src/systems.rs"
        );
        assert_eq!(
            short_path(
                r"C:\Users\a\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\bevy_ecs-0.20.0\src\world\mod.rs"
            ),
            "bevy_ecs-0.20.0/src/world/mod.rs"
        );
    }

    #[test]
    fn a_component_is_found_by_its_name_before_a_part_of_it() {
        let path = "bevy_transform::components::transform::Transform";
        assert_eq!(rank(path, "Transform", path), 3);
        assert_eq!(rank(path, "Transform", "transform"), 2);
        assert_eq!(
            rank(
                "bevy_transform::components::global_transform::GlobalTransform",
                "GlobalTransform",
                "Transform"
            ),
            1
        );
        assert_eq!(rank(path, "Transform", "Velocity"), 0);
    }

    /// The method itself, in a world with no window: a system that moves an entity is named
    /// by its file and line, on the frame it ran, and asking does not count as a change.
    #[test]
    fn the_line_that_last_moved_an_entity_is_named_on_its_frame() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        plugin(&mut app);
        app.world_mut()
            .spawn((Name::new("Mover"), Transform::default()));
        #[derive(Resource)]
        struct MoveOn(u64);
        app.insert_resource(MoveOn(3));
        let line = line!() + 6;
        app.add_systems(
            Update,
            |frame: Res<FrameCount>, on: Res<MoveOn>, mut moved: Query<&mut Transform>| {
                if u64::from(frame.0) == on.0 {
                    for mut transform in &mut moved {
                        transform.translation.x += 1.0;
                    }
                }
            },
        );
        for _ in 0..6 {
            app.update();
        }
        let ask = |app: &mut App| {
            app.world_mut()
                .run_system_cached_with(
                    why,
                    Some(json!({"entity": "mover", "component": "transform"})),
                )
                .unwrap()
                .unwrap()
        };
        let answer = ask(&mut app);
        let first = ask(&mut app);
        assert_eq!(answer, first, "asking changed what it asked about");
        assert_eq!(answer["components"].as_array().unwrap().len(), 1);
        let part = &answer["components"][0];
        assert_eq!(part["name"], "Transform");
        assert_eq!(part["changed"]["frame"], 3);
        assert_eq!(
            part["changed"]["ago"],
            answer["frame"].as_u64().unwrap() - 3
        );
        if answer["tracked"] == true {
            assert_eq!(part["by"]["file"], "crates/terra_world/src/remote/why.rs");
            assert_eq!(part["by"]["line"], line);
            assert_eq!(
                answer["spawned"]["by"]["file"],
                "crates/terra_world/src/remote/why.rs"
            );
        } else {
            assert!(part["by"].is_null());
        }
    }

    /// A body drawn between ticks: its Transform is told as drawn by the tick's drawing, and
    /// what moved where it truly stands beside it, first the system in a tick on its frame,
    /// then a teleport outside the ticks on its own.
    #[test]
    fn a_smoothed_body_is_told_moved_by_the_tick_and_drawn_by_its_drawing() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        // A tick a frame, at Bevy's 64 ticks a second.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_micros(
            15_625,
        )));
        crate::tick::plugin(&mut app);
        plugin(&mut app);
        app.world_mut()
            .spawn((Name::new("Body"), Transform::default(), Smoothed::default()));
        let in_tick = line!() + 6;
        app.add_systems(
            FixedUpdate,
            |frame: Res<FrameCount>, mut moved: Query<&mut Transform, With<Smoothed>>| {
                if frame.0 == 3 {
                    for mut transform in &mut moved {
                        transform.translation.x += 1.0;
                    }
                }
            },
        );
        let outside = line!() + 6;
        app.add_systems(
            Update,
            |frame: Res<FrameCount>, mut moved: Query<&mut Transform, With<Smoothed>>| {
                if frame.0 == 8 {
                    for mut transform in &mut moved {
                        transform.translation.x = 10.0;
                    }
                }
            },
        );
        let ask = |app: &mut App| {
            app.world_mut()
                .run_system_cached_with(
                    why,
                    Some(json!({"entity": "Body", "component": "Transform"})),
                )
                .unwrap()
                .unwrap()["components"][0]
                .clone()
        };
        for _ in 0..6 {
            app.update();
        }
        let part = ask(&mut app);
        assert_eq!(part["drawn"], true);
        let moved = &part["moved"];
        assert_eq!(moved["in_tick"], true, "{part}");
        assert_eq!(moved["changed"]["frame"], 3, "{part}");
        let tracked = MaybeLocation::caller().into_option().is_some();
        if tracked {
            assert_eq!(moved["by"]["file"], "crates/terra_world/src/remote/why.rs");
            assert_eq!(moved["by"]["line"], in_tick);
            assert_eq!(
                part["by"]["file"], "crates/terra_world/src/tick.rs",
                "{part}"
            );
        }
        for _ in 0..6 {
            app.update();
        }
        let part = ask(&mut app);
        let moved = &part["moved"];
        assert_eq!(moved["in_tick"], false, "{part}");
        assert_eq!(moved["changed"]["frame"], 8, "{part}");
        if tracked {
            assert_eq!(moved["by"]["line"], outside);
        }
    }
}
