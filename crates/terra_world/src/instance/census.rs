//! The census (D469): what the world holds, counted so that a round trip to another world
//! and back can be compared with what was there before. A leak is a census that changed.
//!
//! It counts entities by archetype (their components' names, sorted) and by scope (the
//! world's, global, or neither); `Assets<T>` by type, for every asset type registered for
//! reflection and those a module names (`census_assets`); and the resources present. Read
//! as `world.census` through the remote probe, with the GPU's memory by label and the
//! process's resident memory beside it, which are measured rather than compared exactly.

use super::{Global, WorldScoped};
use bevy::{
    asset::ReflectAsset,
    ecs::{component::ComponentId, resource::IS_RESOURCE},
    prelude::*,
    remote::BrpResult,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::any::TypeId;
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// What a world holds, at one moment.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Census {
    /// Entities by archetype: their components' short names, sorted, joined by ", ".
    pub entities: BTreeMap<String, usize>,
    /// Entities that are the world's ([`WorldScoped`]).
    pub scoped: usize,
    /// Entities that outlive it ([`Global`]).
    pub global: usize,
    /// Entities that are neither: the engine's own (windows, observers, systems), and any
    /// that should have said they are the world's.
    pub neither: usize,
    /// How many of each asset type are loaded, by type path.
    pub assets: BTreeMap<String, usize>,
    /// The resources present, by type name.
    pub resources: BTreeSet<String>,
}

/// Asset types counted beyond those registered for reflection, with how to count each.
#[derive(Resource, Default)]
struct Counted(Vec<(&'static str, fn(&World) -> Option<usize>)>);

/// Counts `Assets<A>` in every census.
pub(super) fn count<A: Asset>(world: &mut World) {
    let mut counted = world.get_resource_or_init::<Counted>();
    let name = A::type_path();
    if !counted.0.iter().any(|(each, _)| *each == name) {
        counted.0.push((name, |world| {
            world.get_resource::<Assets<A>>().map(Assets::len)
        }));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Counted>();
}

/// The remote's `world.census` (`remote.ts call world.census`, and in `r snap`).
pub fn remote(app: &mut App) {
    crate::remote::method(app, "world.census", census);
}

/// What Bevy says when it was built without the names of types.
const NO_NAME: &str = "<Enable the debug feature to see the name>";

/// A component's or resource's name, short (`Transform`) or whole, or its number in a build
/// without names.
fn name_of(world: &World, id: ComponentId, short: bool) -> String {
    match world.components().get_name(id) {
        Some(name) if &*name != NO_NAME && short => name.shortname().to_string(),
        Some(name) if &*name != NO_NAME => name.to_string(),
        _ => format!("component {}", id.index()),
    }
}

impl Census {
    /// What the world holds now.
    pub fn take(world: &World) -> Self {
        let mut census = Census::default();
        let components = world.components();
        let scoped = components.component_id::<WorldScoped>();
        let global = components.component_id::<Global>();
        for archetype in world.archetypes().iter() {
            let n = archetype.len() as usize;
            if n == 0 || archetype.contains(IS_RESOURCE) {
                continue;
            }
            let mut names: Vec<String> = archetype
                .components()
                .iter()
                .map(|id| name_of(world, *id, true))
                .collect();
            names.sort();
            *census.entities.entry(names.join(", ")).or_default() += n;
            let has = |id: Option<ComponentId>| id.is_some_and(|id| archetype.contains(id));
            if has(global) {
                census.global += n;
            } else if has(scoped) {
                census.scoped += n;
            } else {
                census.neither += n;
            }
        }
        let mut present = HashSet::<TypeId>::new();
        for (id, info, _) in world.iter_resources() {
            census.resources.insert(name_of(world, id, false));
            present.extend(info.type_id());
        }
        if let Some(registry) = world.get_resource::<AppTypeRegistry>() {
            let registry = registry.read();
            for (registration, assets) in registry.iter_with_data::<ReflectAsset>() {
                if present.contains(&assets.assets_resource_type_id()) {
                    let name = registration.type_info().type_path().to_string();
                    census.assets.insert(name, assets.len(world));
                }
            }
        }
        if let Some(counted) = world.get_resource::<Counted>() {
            for (name, count) in &counted.0 {
                if let Some(n) = count(world) {
                    census.assets.insert((*name).to_string(), n);
                }
            }
        }
        census
    }

    /// What changed from `self` to `after`, a line a change: `~` a count changed, `+` came,
    /// `-` went. Empty when nothing did.
    pub fn diff(&self, after: &Census) -> Vec<String> {
        let mut lines = Vec::new();
        counts("entities", &self.entities, &after.entities, &mut lines);
        counts("assets", &self.assets, &after.assets, &mut lines);
        for (what, was, now) in [
            ("scoped", self.scoped, after.scoped),
            ("global", self.global, after.global),
            ("neither", self.neither, after.neither),
        ] {
            if was != now {
                lines.push(format!("~ {what}: {was} -> {now}"));
            }
        }
        for went in self.resources.difference(&after.resources) {
            lines.push(format!("- resource {went}"));
        }
        for came in after.resources.difference(&self.resources) {
            lines.push(format!("+ resource {came}"));
        }
        lines
    }
}

fn counts(
    what: &str,
    was: &BTreeMap<String, usize>,
    now: &BTreeMap<String, usize>,
    lines: &mut Vec<String>,
) {
    let keys: BTreeSet<&String> = was.keys().chain(now.keys()).collect();
    for key in keys {
        let (a, b) = (was.get(key).copied(), now.get(key).copied());
        match (a, b) {
            (Some(a), Some(b)) if a != b => lines.push(format!("~ {what} [{key}]: {a} -> {b}")),
            (None, Some(b)) => lines.push(format!("+ {what} [{key}]: {b}")),
            (Some(a), None) => lines.push(format!("- {what} [{key}]: {a}")),
            _ => {}
        }
    }
}

/// The remote's `world.census`: the census, with the world's spec and travel, the GPU's
/// memory now by label and the process's resident memory.
fn census(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let census = Census::take(world);
    let mut value = serde_json::to_value(&census).map_err(crate::remote::refused)?;
    let spec = world
        .get_resource::<super::WorldSpec>()
        .map(|spec| json!({"key": spec.key.0, "kind": spec.kind, "seed": spec.seed}));
    let travel = world
        .get_resource::<State<super::Travel>>()
        .map(|travel| format!("{:?}", travel.get()));
    value["world"] = json!(spec);
    value["travel"] = json!(travel);
    value["entered"] = json!(world.get_resource::<super::Entered>().map(|n| n.0));
    value["gpu"] = crate::gpu_memory::now(world).unwrap_or(Value::Null);
    value["resident_mib"] = json!(resident_mib());
    Ok(value)
}

/// The process's resident memory, where the system says it (Linux's `/proc`).
fn resident_mib() -> Option<f64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: f64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some((pages * 4096.0 / (1024.0 * 1024.0)).round())
}
