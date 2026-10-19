//! What fills the GPU's memory (D71): every allocation wgpu made, read from its allocator's
//! report (`wgpu::Device::generate_allocator_report`, gpu-allocator under Vulkan and DX12) and
//! summed by the label it was made with, numbers folded to `#` so a thousand mesh slabs or
//! chunks read as one line. Sampled in the render world twice a second; the peak is kept with
//! the textures and meshes then prepared (Bevy's `RenderAssets`), the textures by the asset
//! they came from and the meshes by the named entity that draws them. Said in the log at each
//! new 256 MiB of peak and in full as the run ends, and read live with `gpu.memory`.
//!
//! The allocator's figure is this process alone, where `nvidia-smi` counts the whole card;
//! the driver's own (pipelines, the context) is in neither report.

use bevy::{
    asset::UntypedAssetId,
    mesh::MeshVertexBufferLayoutRef,
    prelude::*,
    remote::BrpResult,
    render::{
        Render, RenderApp, RenderSystems,
        mesh::{RenderMesh, RenderMeshBufferInfo},
        render_asset::RenderAssets,
        renderer::RenderDevice,
        texture::GpuImage,
    },
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MIB: f64 = 1024.0 * 1024.0;
/// How often the allocator is read.
const EVERY: Duration = Duration::from_millis(500);
/// How far the peak rises before the textures and meshes are counted again.
const RECOUNT: u64 = 64 << 20;
/// How far the peak rises between lines in the log.
const STEP: u64 = 256 << 20;

/// What the allocator held at one moment.
#[derive(Clone, Default)]
struct Sample {
    /// Seconds since the first sample.
    at: f32,
    allocated: u64,
    reserved: u64,
    blocks: usize,
    /// Bytes and how many allocations, by folded label, most first.
    labels: Vec<(String, u64, usize)>,
}

/// The textures and meshes prepared at the peak.
#[derive(Clone, Default)]
struct Prepared {
    images: Vec<(AssetId<Image>, u64, String)>,
    /// Each mesh's bytes and its vertex layout (its stride and attributes), which says
    /// what made a mesh nothing draws.
    meshes: Vec<(AssetId<Mesh>, u64, String)>,
}

#[derive(Default)]
struct State {
    started: Option<Instant>,
    last: Option<Instant>,
    now: Sample,
    peak: Sample,
    prepared: Prepared,
    prepared_at: u64,
    said_at: u64,
    /// Who drew each mesh when the peak's meshes were counted, noted by the main world in
    /// the frame after, so a mesh let go since is still put to who drew it then.
    drawers: Option<HashMap<AssetId<Mesh>, String>>,
}

/// Shared between the render world, which samples, and the main world, which says it.
#[derive(Resource, Clone, Default)]
struct Shared(Arc<Mutex<State>>);

pub fn plugin(app: &mut App) {
    let shared = Shared::default();
    app.insert_resource(shared.clone())
        .add_systems(Last, (note_drawers, say_on_exit));
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .insert_resource(shared)
            .add_systems(Render, sample.in_set(RenderSystems::Cleanup));
    }
}

/// The remote's `gpu.memory`: the allocator now and at its peak, by label, and the peak's
/// textures and meshes. `{"top": n}` keeps the n largest of each (20 by default).
pub fn remote(app: &mut App) {
    crate::remote::method(app, "gpu.memory", memory);
}

/// A label with its numbers folded, so every slab or chunk of a kind sums together.
pub fn fold(label: &str) -> String {
    let mut folded = String::with_capacity(label.len());
    let mut in_number = false;
    for c in label.chars() {
        if c.is_ascii_digit() {
            if !in_number {
                folded.push('#');
            }
            in_number = true;
        } else {
            folded.push(c);
            in_number = false;
        }
    }
    folded
}

/// Bytes by key, with how many, most first.
fn sum_by<K: std::hash::Hash + Eq>(items: impl Iterator<Item = (K, u64)>) -> Vec<(K, u64, usize)> {
    let mut sums: HashMap<K, (u64, usize)> = HashMap::new();
    for (key, bytes) in items {
        let sum = sums.entry(key).or_default();
        sum.0 += bytes;
        sum.1 += 1;
    }
    let mut sums: Vec<_> = sums.into_iter().map(|(k, (b, n))| (k, b, n)).collect();
    sums.sort_by_key(|s| std::cmp::Reverse(s.1));
    sums
}

/// Every level of every layer of a texture, by its format's blocks.
pub fn texture_bytes(
    size: bevy::render::render_resource::Extent3d,
    format: bevy::render::render_resource::TextureFormat,
    mips: u32,
    samples: u32,
) -> u64 {
    let (bw, bh) = format.block_dimensions();
    let block = u64::from(format.block_copy_size(None).unwrap_or(4));
    (0..mips)
        .map(|level| {
            let w = u64::from((size.width >> level).max(1).div_ceil(bw));
            let h = u64::from((size.height >> level).max(1).div_ceil(bh));
            w * h * block * u64::from(size.depth_or_array_layers)
        })
        .sum::<u64>()
        * u64::from(samples.max(1))
}

/// A mesh's vertex layout: its stride and its attributes' ids (`MeshVertexAttribute::id`).
fn layout_of(mesh: &RenderMesh) -> String {
    let MeshVertexBufferLayoutRef(layout) = &mesh.layout;
    let ids: Vec<String> = layout
        .attribute_ids()
        .iter()
        .map(|id| format!("{id:?}").replace("MeshVertexAttributeId", ""))
        .collect();
    format!(
        "[stride {}, attributes {}]",
        layout.layout().array_stride,
        ids.join(" ")
    )
}

fn mesh_bytes(mesh: &RenderMesh) -> u64 {
    let MeshVertexBufferLayoutRef(layout) = &mesh.layout;
    let vertices = u64::from(mesh.vertex_count) * layout.layout().array_stride;
    let indices = match mesh.buffer_info {
        RenderMeshBufferInfo::Indexed {
            count,
            index_format,
        } => {
            u64::from(count)
                * match index_format {
                    bevy::render::render_resource::IndexFormat::Uint16 => 2,
                    bevy::render::render_resource::IndexFormat::Uint32 => 4,
                }
        }
        RenderMeshBufferInfo::NonIndexed => 0,
    };
    vertices + indices
}

fn sample(
    shared: Res<Shared>,
    device: Res<RenderDevice>,
    images: Option<Res<RenderAssets<GpuImage>>>,
    meshes: Option<Res<RenderAssets<RenderMesh>>>,
) {
    let mut state = shared
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let now = Instant::now();
    if state.last.is_some_and(|last| now - last < EVERY) {
        return;
    }
    state.last = Some(now);
    let started = *state.started.get_or_insert(now);
    let Some(report) = device.wgpu_device().generate_allocator_report() else {
        return;
    };
    let labels = sum_by(report.allocations.iter().map(|a| (fold(&a.name), a.size)));
    state.now = Sample {
        at: (now - started).as_secs_f32(),
        allocated: report.total_allocated_bytes,
        reserved: report.total_reserved_bytes,
        blocks: report.blocks.len(),
        labels,
    };
    if state.now.reserved < state.peak.reserved {
        return;
    }
    state.peak = state.now.clone();
    let reserved = state.peak.reserved;
    if reserved >= state.prepared_at + RECOUNT || state.prepared.images.is_empty() {
        state.prepared_at = reserved;
        state.drawers = None;
        state.prepared = Prepared {
            images: images.map_or_else(Vec::new, |images| {
                images
                    .iter()
                    .map(|(id, image)| {
                        let d = &image.texture_descriptor;
                        let bytes =
                            texture_bytes(d.size, d.format, d.mip_level_count, d.sample_count);
                        let shape = format!(
                            "{:?} {}x{}x{} {} mips",
                            d.format,
                            d.size.width,
                            d.size.height,
                            d.size.depth_or_array_layers,
                            d.mip_level_count
                        );
                        (id, bytes, shape)
                    })
                    .collect()
            }),
            meshes: meshes.map_or_else(Vec::new, |meshes| {
                meshes
                    .iter()
                    .map(|(id, mesh)| (id, mesh_bytes(mesh), layout_of(mesh)))
                    .collect()
            }),
        };
    }
    if reserved >= state.said_at + STEP {
        state.said_at = reserved - reserved % STEP;
        let top: Vec<String> = state
            .peak
            .labels
            .iter()
            .take(12)
            .map(|(l, b, _)| format!("{l} {:.0}", *b as f64 / MIB))
            .collect();
        info!(
            "GPU memory: {:.0} MiB reserved, {:.0} allocated at {:.0} s; most (MiB): {}",
            reserved as f64 / MIB,
            state.peak.allocated as f64 / MIB,
            state.peak.at,
            top.join(", ")
        );
    }
}

/// Each drawn mesh's drawer: the nearest named entity up from one that draws it.
fn drawers_now(world: &mut World) -> HashMap<AssetId<Mesh>, String> {
    let mut drawer: HashMap<AssetId<Mesh>, String> = HashMap::new();
    let mut query = world.query::<(Entity, &Mesh3d)>();
    let drawn: Vec<(Entity, AssetId<Mesh>)> = query.iter(world).map(|(e, m)| (e, m.id())).collect();
    for (entity, mesh) in drawn {
        if drawer.contains_key(&mesh) {
            continue;
        }
        let mut at = Some(entity);
        let mut name = None;
        for _ in 0..12 {
            let Some(here) = at else { break };
            let Ok(entity) = world.get_entity(here) else {
                break;
            };
            if let Some(n) = entity.get::<Name>() {
                name = Some(fold(n.as_str()));
                break;
            }
            at = entity.get::<ChildOf>().map(ChildOf::parent);
        }
        drawer.insert(mesh, name.unwrap_or_else(|| "(unnamed)".to_string()));
    }
    drawer
}

/// Notes who draws each mesh once the render world has counted the peak's meshes.
fn note_drawers(world: &mut World) {
    let Some(shared) = world.get_resource::<Shared>().cloned() else {
        return;
    };
    let wanted = {
        let state = shared
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.drawers.is_none() && !state.prepared.meshes.is_empty()
    };
    if wanted {
        let drawers = drawers_now(world);
        shared
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .drawers = Some(drawers);
    }
}

/// The peak's textures by the asset they were loaded from (or their shape, if made in code),
/// and its meshes by the nearest named entity that draws them; a mesh nothing draws is
/// counted as such.
fn owners(
    world: &mut World,
    prepared: &Prepared,
    drawers: Option<HashMap<AssetId<Mesh>, String>>,
) -> (Vec<(String, u64, usize)>, Vec<(String, u64, usize)>) {
    let server = world.get_resource::<AssetServer>();
    let images = sum_by(prepared.images.iter().map(|(id, bytes, shape)| {
        let path =
            server.and_then(|s| s.get_path(UntypedAssetId::from(*id)).map(|p| p.to_string()));
        (
            path.map_or_else(
                || format!("(made) {shape}"),
                |p| format!("{} [{shape}]", fold(&p)),
            ),
            *bytes,
        )
    }));
    let drawer = drawers.unwrap_or_else(|| drawers_now(world));
    let meshes = sum_by(prepared.meshes.iter().map(|(id, bytes, layout)| {
        (
            drawer
                .get(id)
                .cloned()
                .unwrap_or_else(|| format!("(drawn by nothing) {layout}")),
            *bytes,
        )
    }));
    (images, meshes)
}

fn lines(sums: &[(String, u64, usize)], top: usize) -> Vec<Value> {
    sums.iter()
        .take(top)
        .map(|(name, bytes, n)| json!({"name": name, "mib": (*bytes as f64 / MIB * 10.0).round() / 10.0, "count": n}))
        .collect()
}

fn sample_json(sample: &Sample, top: usize) -> Value {
    json!({
        "at_s": sample.at,
        "reserved_mib": (sample.reserved as f64 / MIB).round(),
        "allocated_mib": (sample.allocated as f64 / MIB).round(),
        "blocks": sample.blocks,
        "labels": lines(&sample.labels, top),
    })
}

fn memory(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let top = params
        .as_ref()
        .and_then(|p| p.get("top"))
        .and_then(Value::as_u64)
        .unwrap_or(20) as usize;
    let shared = world.resource::<Shared>().clone();
    let (now, peak, prepared, drawers) = {
        let state = shared
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (
            state.now.clone(),
            state.peak.clone(),
            state.prepared.clone(),
            state.drawers.clone(),
        )
    };
    let (images, meshes) = owners(world, &prepared, drawers);
    Ok(json!({
        "now": sample_json(&now, top),
        "peak": sample_json(&peak, top),
        "peak_textures": lines(&images, top),
        "peak_meshes": lines(&meshes, top),
    }))
}

fn say_on_exit(world: &mut World, mut said: Local<bool>) {
    let exited = world
        .get_resource::<Messages<AppExit>>()
        .is_some_and(|exits| !exits.is_empty());
    if !exited || *said {
        return;
    }
    *said = true;
    let shared = world.resource::<Shared>().clone();
    let (peak, prepared, drawers) = {
        let state = shared
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (
            state.peak.clone(),
            state.prepared.clone(),
            state.drawers.clone(),
        )
    };
    if peak.reserved == 0 {
        return;
    }
    let (images, meshes) = owners(world, &prepared, drawers);
    let say = |sums: &[(String, u64, usize)]| -> String {
        sums.iter()
            .take(40)
            .map(|(name, bytes, n)| format!("\n  {:>8.1} MiB  {n:>6}  {name}", *bytes as f64 / MIB))
            .collect()
    };
    let total = |sums: &[(String, u64, usize)]| sums.iter().map(|s| s.1).sum::<u64>() as f64 / MIB;
    info!(
        "GPU memory at its peak, {:.0} s in: {:.0} MiB in {} blocks, {:.0} allocated; by label (MiB, allocations):{}",
        peak.at,
        peak.reserved as f64 / MIB,
        peak.blocks,
        peak.allocated as f64 / MIB,
        say(&peak.labels)
    );
    info!(
        "GPU memory's textures at the peak, {:.0} MiB by asset:{}",
        total(&images),
        say(&images)
    );
    // Whether textures are compressed and mipmapped is most of what they cost.
    let formats = sum_by(prepared.images.iter().map(|(_, bytes, shape)| {
        let format = shape.split(' ').next().unwrap_or_default();
        let mipped = if shape.ends_with(" 1 mips") {
            ", no mips"
        } else {
            ""
        };
        (format!("{format}{mipped}"), *bytes)
    }));
    info!(
        "GPU memory's textures at the peak by format:{}",
        say(&formats)
    );
    info!(
        "GPU memory's meshes at the peak, {:.0} MiB used by who draws them:{}",
        total(&meshes),
        say(&meshes)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_fold_so_a_kind_sums_as_one() {
        assert_eq!(
            fold("general mesh slab 12 (vertex buffer)"),
            "general mesh slab # (vertex buffer)"
        );
        assert_eq!(fold("chunk 3,-14"), "chunk #,-#");
        assert_eq!(fold("taa_history"), "taa_history");
        let sums = sum_by([("a", 3), ("b", 10), ("a", 4)].into_iter());
        assert_eq!(sums, vec![("b", 10, 1), ("a", 7, 2)]);
    }

    #[test]
    fn a_texture_counts_every_level_and_layer_by_its_blocks() {
        use bevy::render::render_resource::{Extent3d, TextureFormat};
        let size = Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 2,
        };
        // 16 + 4 + 1 texels a layer, 4 bytes each.
        assert_eq!(
            texture_bytes(size, TextureFormat::Rgba8Unorm, 3, 1),
            2 * 21 * 4
        );
        // One 4x4 block of 16 bytes at each of the first three levels.
        assert_eq!(
            texture_bytes(size, TextureFormat::Bc7RgbaUnorm, 3, 1),
            2 * 3 * 16
        );
    }
}
