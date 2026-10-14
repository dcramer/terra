//! What each base holds in memory (docs/building-plan.md, checking it; D69): on the CPU its
//! cubes, the copy its sections are laid from and its pieces laid whole; on the GPU its
//! sections' meshes and its cells far off. Read live with `build.memory` over the remote,
//! and said as the run ends, so a benchmark reports it beside its frames.

use super::store::{Section, each};
use super::{Build, BuildLook, Built, Site};
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

/// How many bytes a texture takes on the GPU: every level of every layer, by its format's
/// blocks.
pub(super) fn texture_bytes(image: &Image) -> usize {
    let descriptor = &image.texture_descriptor;
    let format = descriptor.format;
    let (bw, bh) = format.block_dimensions();
    let block = format.block_copy_size(None).unwrap_or(4) as usize;
    let size = descriptor.size;
    (0..descriptor.mip_level_count)
        .map(|level| {
            let w = (size.width >> level).max(1).div_ceil(bw) as usize;
            let h = (size.height >> level).max(1).div_ceil(bh) as usize;
            w * h * block * size.depth_or_array_layers as usize
        })
        .sum()
}

/// What building draws with in view, and holds of textures (D290): how many of its meshes
/// are drawn this frame and in how many materials, each mesh at least a draw a pass, and
/// how many MiB of textures its materials hold on the GPU.
fn drawing(world: &mut World) -> (usize, usize, f32) {
    let mut sections = world.query_filtered::<(), With<Section>>();
    let mut meshes = world.query::<(
        &ViewVisibility,
        &ChildOf,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>();
    let mut drawn = 0;
    let mut materials = std::collections::BTreeSet::new();
    let found: Vec<(Entity, bool, Option<AssetId<StandardMaterial>>)> = meshes
        .iter(world)
        .map(|(seen, parent, material)| (parent.parent(), seen.get(), material.map(|m| m.id())))
        .collect();
    for (parent, seen, material) in found {
        if seen && sections.get(world, parent).is_ok() {
            drawn += 1;
            if let Some(material) = material {
                materials.insert(material);
            }
        }
    }
    let mut bytes = 0;
    if let (Some(look), Some(standard), Some(images)) = (
        world.get_resource::<BuildLook>(),
        world.get_resource::<Assets<StandardMaterial>>(),
        world.get_resource::<Assets<Image>>(),
    ) {
        let mut seen = std::collections::BTreeSet::new();
        for handle in look.materials.all() {
            let Some(material) = standard.get(handle) else {
                continue;
            };
            for image in [
                &material.base_color_texture,
                &material.normal_map_texture,
                &material.metallic_roughness_texture,
                &material.occlusion_texture,
                &material.depth_map,
            ]
            .into_iter()
            .flatten()
            {
                if seen.insert(image.id())
                    && let Some(image) = images.get(image)
                {
                    bytes += texture_bytes(image);
                }
            }
        }
    }
    (drawn, materials.len(), bytes as f32 / (1024.0 * 1024.0))
}

/// As the run ends, what the bases hold: in all, a median base's and the most any holds.
fn say_on_exit(world: &mut World, mut said: Local<bool>) {
    let exited = world
        .get_resource::<Messages<AppExit>>()
        .is_some_and(|exits| !exits.is_empty());
    if !exited || *said {
        return;
    }
    *said = true;
    let (drawn, materials, textures) = drawing(world);
    info!(
        "Builds drawn in view: {drawn} meshes in {materials} materials; their textures {textures:.0} MiB on the GPU."
    );
    let mut state = bevy::ecs::system::SystemState::<(ResMut<Site>, Query<&mut Built>)>::new(world);
    let Ok((mut site, mut builds)) = state.get_mut(world) else {
        return;
    };
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
