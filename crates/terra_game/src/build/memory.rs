//! What each base holds in memory (docs/building-plan.md, checking it; D69): on the CPU its
//! cubes, the copy its sections are laid from and its pieces laid whole; on the GPU its
//! sections' meshes and its cells far off. Read live with `build.memory` over the remote,
//! and said as the run ends, so a benchmark reports it beside its frames.

use super::store::each;
use super::{Build, Built, Site};
use crate::remote::method;
use bevy::{prelude::*, remote::BrpResult};
use serde_json::{Value, json};

/// What a base holds.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Held {
    pub cubes: usize,
    /// Sections laid and drawn, and set aside far off.
    pub laid: usize,
    pub far_off: usize,
    /// About how many bytes on the CPU, and exactly how many of meshes on the GPU.
    pub cpu: usize,
    pub gpu: usize,
}

/// What a build holds now.
pub(super) fn held(build: &Build) -> Held {
    let sections = &build.sections;
    let base = build.base.bytes();
    Held {
        cubes: build.base.store().len(),
        laid: sections.drawn.len(),
        far_off: sections.far_off.len(),
        cpu: base
            + sections
                .snapshot
                .as_ref()
                .map_or(0, |snapshot| snapshot.bytes())
            + sections.whole.as_ref().map_or(0, |whole| whole.bytes()),
        gpu: sections.drawn.values().map(|d| d.gpu_bytes).sum::<usize>() + sections.cells.gpu_bytes,
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Last, say_on_exit);
}

/// The remote's `build.memory`, for a run driven live.
pub fn remote(app: &mut App) {
    method(app, "build.memory", memory);
}

/// Every base's memory, by name, and their totals, in bytes.
fn memory(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let mut bases = Vec::new();
    let mut total = Held::default();
    let mut add = |name: String, held: Held| {
        total.cubes += held.cubes;
        total.laid += held.laid;
        total.far_off += held.far_off;
        total.cpu += held.cpu;
        total.gpu += held.gpu;
        bases.push(json!({
            "name": name,
            "cubes": held.cubes,
            "sections_laid": held.laid,
            "sections_far_off": held.far_off,
            "cpu_bytes": held.cpu,
            "gpu_bytes": held.gpu,
        }));
    };
    if let Some(site) = world.get_resource::<Site>() {
        add("the player's base".to_string(), held(&site.build));
    }
    let mut builds = world.query::<(&Built, Option<&Name>)>();
    for (built, name) in builds.iter(world) {
        add(
            name.map_or_else(|| "a base".to_string(), |n| n.to_string()),
            held(&built.0),
        );
    }
    Ok(json!({
        "bases": bases,
        "total": {
            "bases": bases.len(),
            "cubes": total.cubes,
            "sections_laid": total.laid,
            "sections_far_off": total.far_off,
            "cpu_bytes": total.cpu,
            "gpu_bytes": total.gpu,
        },
    }))
}

/// As the run ends, what the bases hold: in all, a median base's and the most any holds.
fn say_on_exit(
    mut exits: MessageReader<AppExit>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
) {
    if exits.read().next().is_none() {
        return;
    }
    let mut all: Vec<Held> = each(&mut site, &mut builds)
        .map(|build| held(build))
        .filter(|held| held.cubes > 0)
        .collect();
    if all.is_empty() {
        return;
    }
    let mib = |bytes: usize| bytes as f32 / (1024.0 * 1024.0);
    let summary = |pick: fn(&Held) -> usize, all: &mut Vec<Held>| {
        all.sort_by_key(pick);
        (
            mib(all.iter().map(pick).sum()),
            mib(pick(&all[all.len() / 2])),
            mib(all.iter().map(pick).max().unwrap_or(0)),
        )
    };
    let (gpu, gpu_median, gpu_most) = summary(|h| h.gpu, &mut all);
    let (cpu, cpu_median, cpu_most) = summary(|h| h.cpu, &mut all);
    let laid: usize = all.iter().map(|h| h.laid).sum();
    let far_off: usize = all.iter().map(|h| h.far_off).sum();
    info!(
        "Builds hold: {} bases, {laid} sections laid and {far_off} set aside far off; {gpu:.0} MiB of meshes on the GPU, a median {gpu_median:.2} MiB a base and the most {gpu_most:.1}; {cpu:.0} MiB on the CPU, a median {cpu_median:.2} MiB and the most {cpu_most:.1}.",
        all.len()
    );
}
