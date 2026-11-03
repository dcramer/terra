//! Terra's patch (D613): the bin buffers of one (view, phase, mesh class)'s
//! multidrawn batch sets laid end to end, so `unpack_bins` and each step of
//! `allocate_uniforms` run once for them all rather than once a batch set.
//!
//! Bevy 0.20 dispatched both shaders once a batch set, each dispatch with its
//! batch set's own buffers: some 150 dispatches of each a frame at Terra's lake,
//! most of one workgroup, about 2 µs of the GPU's each in the barrier and the
//! dispatch rather than in work (play-lake-fps j15). GPU-driven renderers run
//! such bookkeeping as one dispatch over every draw, each thread finding its
//! draw's record through a table of offsets (Wihlidal, "Optimizing the Graphics
//! Pipeline with Compute", GDC 2016; Haar and Aaltonen, "GPU-Driven Rendering
//! Pipelines", SIGGRAPH 2015): here each batch set's offsets are a
//! [`GpuBinUnpackingJob`] and a [`GpuUniformAllocationJob`], and a thread finds
//! its batch set by a binary search over its dispatch's jobs.
//!
//! The merged buffers are written only when a batch set changes, as D583 has it:
//! while the same batch sets keep the same lengths, a changed one is written in
//! place, three writes as before; otherwise the three buffers are written whole,
//! three writes for every batch set.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use wgpu::BufferUsages;

use super::gpu_preprocessing::{
    BatchSetSceneUnpackingMetadata, BinUnpackingMetadataIndex, GpuBinMetadata,
    GpuBinUnpackingJob, GpuBinUnpackingMetadata, GpuUniformAllocationJob,
    GpuUniformAllocationMetadata, SceneUnpackingJob, UniformAllocationMetadataIndex,
};
use crate::{
    render_phase::{
        GpuRenderBinnedMeshInstance, RenderMultidrawableBatchSetGpuBuffers,
        UNIFORM_ALLOCATION_WORKGROUP_SIZE,
    },
    render_resource::{RawBufferVec, UninitBufferVec},
    renderer::{RenderDevice, RenderQueue},
};
use nonmax::NonMaxU32;

/// Whether a (view, phase, mesh class)'s batch sets share one dispatch (true, the
/// default) or each has its own, as Bevy 0.20 had them, so a probe can measure the
/// one against the other in one run. The game sets it from its tunable
/// `render.merged_unpacking`.
pub static MERGE_SCENE_UNPACKING: AtomicBool = AtomicBool::new(true);

/// Writes a (view, phase, mesh class)'s batch sets' bin buffers into `merged`, all
/// in one unless [`MERGE_SCENE_UNPACKING`] is off, and pushes the dispatches to
/// `jobs`.
pub(crate) fn write_merged(
    merged: &mut Vec<MergedBinBuffers>,
    sets: &mut [MergedBatchSet],
    tables: &mut MergedTables,
    jobs: &mut Vec<SceneUnpackingJob>,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) {
    let together = MERGE_SCENE_UNPACKING.load(Ordering::Relaxed);
    let size = if together { sets.len().max(1) } else { 1 };
    let groups = sets.len().div_ceil(size);
    merged.truncate(groups);
    merged.resize_with(groups, MergedBinBuffers::default);
    for (buffers, group) in merged.iter_mut().zip(sets.chunks_mut(size)) {
        jobs.extend(buffers.write(group, tables, render_device, render_queue));
    }
}

/// One (view, phase, mesh class)'s batch sets' bin buffers, end to end.
pub struct MergedBinBuffers {
    /// Every batch set's `GpuRenderBinnedMeshInstance`s.
    instances: RawBufferVec<GpuRenderBinnedMeshInstance>,
    /// Every batch set's `GpuBinMetadata`.
    bin_metadata: RawBufferVec<GpuBinMetadata>,
    /// Every batch set's bin-index-to-bin-metadata-index map.
    bin_index_map: RawBufferVec<u32>,
    /// A fan entry for each chunk of 256 bins, every batch set's.
    fan: UninitBufferVec<u32>,
    /// The batch sets and lengths the three buffers above hold.
    layout: Vec<MergedRange>,
}

impl Default for MergedBinBuffers {
    fn default() -> Self {
        let mut instances = RawBufferVec::new(BufferUsages::STORAGE);
        instances.set_label(Some("merged render bin entry buffer"));
        let mut bin_metadata = RawBufferVec::new(BufferUsages::STORAGE);
        bin_metadata.set_label(Some("merged bin metadata buffer"));
        let mut bin_index_map = RawBufferVec::new(BufferUsages::STORAGE);
        bin_index_map.set_label(Some("merged bin-index-to-bin-metadata-index buffer"));
        let mut fan = UninitBufferVec::new(BufferUsages::STORAGE);
        fan.set_label(Some("merged fan buffer"));
        MergedBinBuffers {
            instances,
            bin_metadata,
            bin_index_map,
            fan,
            layout: Vec::new(),
        }
    }
}

/// A batch set and the lengths of its three bin buffers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct MergedRange {
    /// [`RenderMultidrawableBatchSetGpuBuffers::id`].
    id: u64,
    /// Its binned mesh instances.
    instances: u32,
    /// Its bin metadata.
    bin_metadata: u32,
    /// Its bin-index-to-bin-metadata-index map.
    bin_index_map: u32,
}

/// A batch set to merge this frame.
pub(crate) struct MergedBatchSet<'a> {
    /// Its bin buffers.
    pub(crate) gpu: &'a mut RenderMultidrawableBatchSetGpuBuffers,
    /// Where it draws.
    pub(crate) metadata: BatchSetSceneUnpackingMetadata,
    /// Its number of bins.
    pub(crate) bin_count: u32,
}

/// The jobs of one merged dispatch, worked out from its batch sets alone.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MergedPlan {
    pub(crate) layout: Vec<MergedRange>,
    pub(crate) unpacking_jobs: Vec<GpuBinUnpackingJob>,
    pub(crate) allocation_jobs: Vec<GpuUniformAllocationJob>,
    pub(crate) instance_count: u32,
    pub(crate) chunk_count: u32,
    pub(crate) multi_chunk: bool,
}

/// The shape of a batch set the plan needs, apart from its buffers.
#[derive(Clone, Copy)]
pub(crate) struct BatchSetShape {
    pub(crate) range: MergedRange,
    pub(crate) bin_count: u32,
    pub(crate) metadata: BatchSetSceneUnpackingMetadata,
}

/// Lays the batch sets end to end, in their order.
pub(crate) fn plan(sets: &[BatchSetShape]) -> MergedPlan {
    let mut plan = MergedPlan {
        layout: Vec::with_capacity(sets.len()),
        unpacking_jobs: Vec::with_capacity(sets.len()),
        allocation_jobs: Vec::with_capacity(sets.len()),
        instance_count: 0,
        chunk_count: 0,
        multi_chunk: false,
    };
    let (mut bin_metadata_start, mut bin_index_map_start) = (0, 0);
    for set in sets {
        let chunks = set.bin_count.div_ceil(UNIFORM_ALLOCATION_WORKGROUP_SIZE);
        plan.unpacking_jobs.push(GpuBinUnpackingJob {
            instance_start: plan.instance_count,
            instance_count: set.range.instances,
            base_output_work_item_index: set.metadata.base_output_work_item_index,
            base_indirect_parameters_index: set.metadata.base_indirect_parameters_index,
            bin_metadata_start,
            bin_index_map_start,
            pad: [0; 2],
        });
        plan.allocation_jobs.push(GpuUniformAllocationJob {
            batch_set_index: set.metadata.batch_set_index,
            bin_count: set.bin_count,
            first_indirect_parameters_index: set.metadata.first_indirect_parameters_index,
            first_output_mesh_uniform_index: set.metadata.first_output_mesh_uniform_index,
            bin_metadata_start,
            first_chunk: plan.chunk_count,
            pad: [0; 2],
        });
        plan.layout.push(set.range);
        plan.instance_count += set.range.instances;
        plan.chunk_count += chunks;
        plan.multi_chunk |= chunks > 1;
        bin_metadata_start += set.range.bin_metadata;
        bin_index_map_start += set.range.bin_index_map;
    }
    plan
}

impl MergedBinBuffers {
    /// Writes the batch sets' bin buffers end to end where they changed, pushes
    /// their jobs and their dispatch's metadata, and returns the dispatch, or
    /// `None` when there are no batch sets.
    pub(crate) fn write(
        &mut self,
        sets: &mut [MergedBatchSet],
        tables: &mut MergedTables,
        render_device: &RenderDevice,
        render_queue: &RenderQueue,
    ) -> Option<SceneUnpackingJob> {
        let shapes: Vec<BatchSetShape> = sets
            .iter()
            .map(|set| BatchSetShape {
                range: MergedRange {
                    id: set.gpu.id,
                    instances: set.gpu.render_binned_mesh_instance_buffer.len() as u32,
                    bin_metadata: set.gpu.bin_metadata_buffer.len() as u32,
                    bin_index_map: set.gpu.bin_index_to_bin_metadata_index_buffer.len() as u32,
                },
                bin_count: set.bin_count,
                metadata: set.metadata,
            })
            .collect();
        let plan = plan(&shapes);

        if plan.layout != self.layout || !self.write_in_place(sets, render_queue) {
            self.instances.clear();
            self.bin_metadata.clear();
            self.bin_index_map.clear();
            for set in sets.iter() {
                let gpu = &*set.gpu;
                self.instances
                    .extend(gpu.render_binned_mesh_instance_buffer.values().iter().copied());
                self.bin_metadata
                    .extend(gpu.bin_metadata_buffer.values().iter().copied());
                self.bin_index_map.extend(
                    gpu.bin_index_to_bin_metadata_index_buffer
                        .values()
                        .iter()
                        .copied(),
                );
            }
            self.instances.write_buffer(render_device, render_queue);
            self.bin_metadata.write_buffer(render_device, render_queue);
            self.bin_index_map.write_buffer(render_device, render_queue);
            self.layout = plan.layout.clone();
        }
        for set in sets.iter_mut() {
            set.gpu.dirty = false;
        }

        self.fan.clear();
        self.fan.add_multiple(plan.chunk_count as usize);
        self.fan.write_buffer(render_device);

        let (Some(instances), Some(bin_metadata), Some(fan), Some(bin_index_map)) = (
            self.instances.buffer(),
            self.bin_metadata.buffer(),
            self.fan.buffer(),
            self.bin_index_map.buffer(),
        ) else {
            return None;
        };
        if plan.instance_count == 0 || plan.chunk_count == 0 {
            return None;
        }

        let first_unpacking_job = tables.bin_unpacking_jobs.len() as u32;
        let first_allocation_job = tables.uniform_allocation_jobs.len() as u32;
        let job_count = plan.unpacking_jobs.len() as u32;
        tables.bin_unpacking_jobs.extend(plan.unpacking_jobs);
        tables.uniform_allocation_jobs.extend(plan.allocation_jobs);
        let bin_unpacking_metadata_index =
            tables.bin_unpacking_metadata.push(GpuBinUnpackingMetadata {
                first_job: first_unpacking_job,
                job_count,
                binned_mesh_instance_count: plan.instance_count,
                pad: [0; _],
            });
        let uniform_allocation_metadata_index =
            tables
                .uniform_allocation_metadata
                .push(GpuUniformAllocationMetadata {
                    first_job: first_allocation_job,
                    job_count,
                    chunk_count: plan.chunk_count,
                    pad_a: 0,
                    pad: [0; _],
                });

        Some(SceneUnpackingJob {
            render_binned_mesh_instance_buffer: instances.clone(),
            bin_metadata_buffer: bin_metadata.clone(),
            fan_buffer: fan.clone(),
            bin_index_to_bin_metadata_index_buffer: bin_index_map.clone(),
            bin_unpacking_metadata_index: BinUnpackingMetadataIndex(NonMaxU32::new(
                bin_unpacking_metadata_index as u32,
            )?),
            uniform_allocation_metadata_index: UniformAllocationMetadataIndex(NonMaxU32::new(
                uniform_allocation_metadata_index as u32,
            )?),
            mesh_instance_count: plan.instance_count,
            chunk_count: plan.chunk_count,
            job_count,
            multi_chunk: plan.multi_chunk,
        })
    }

    /// Writes each changed batch set's buffers over its own range, the layout
    /// being the one the GPU holds. Returns false when a range could not be
    /// written, so the caller writes the buffers whole.
    fn write_in_place(&mut self, sets: &[MergedBatchSet], render_queue: &RenderQueue) -> bool {
        let (mut instances, mut bin_metadata, mut bin_index_map) = (0, 0, 0);
        for set in sets {
            let gpu = &*set.gpu;
            let source_instances = gpu.render_binned_mesh_instance_buffer.values();
            let source_bin_metadata = gpu.bin_metadata_buffer.values();
            let source_bin_index_map = gpu.bin_index_to_bin_metadata_index_buffer.values();
            if gpu.dirty {
                let ok = write_range(
                    &mut self.instances,
                    instances,
                    source_instances,
                    render_queue,
                ) && write_range(
                    &mut self.bin_metadata,
                    bin_metadata,
                    source_bin_metadata,
                    render_queue,
                ) && write_range(
                    &mut self.bin_index_map,
                    bin_index_map,
                    source_bin_index_map,
                    render_queue,
                );
                if !ok {
                    return false;
                }
            }
            instances += source_instances.len();
            bin_metadata += source_bin_metadata.len();
            bin_index_map += source_bin_index_map.len();
        }
        true
    }
}

/// Copies `source` over `merged` from `start` and writes that range to the GPU.
fn write_range<T: bytemuck::NoUninit>(
    merged: &mut RawBufferVec<T>,
    start: usize,
    source: &[T],
    render_queue: &RenderQueue,
) -> bool {
    let end = start + source.len();
    if source.is_empty() {
        return true;
    }
    let Some(target) = merged.values_mut().get_mut(start..end) else {
        return false;
    };
    target.copy_from_slice(source);
    merged.write_buffer_range(render_queue, start..end).is_ok()
}

/// The frame's tables every merged dispatch pushes its jobs and metadata to.
pub(crate) struct MergedTables<'a> {
    pub(crate) bin_unpacking_jobs: &'a mut RawBufferVec<GpuBinUnpackingJob>,
    pub(crate) uniform_allocation_jobs: &'a mut RawBufferVec<GpuUniformAllocationJob>,
    pub(crate) bin_unpacking_metadata: &'a mut RawBufferVec<GpuBinUnpackingMetadata>,
    pub(crate) uniform_allocation_metadata: &'a mut RawBufferVec<GpuUniformAllocationMetadata>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(id: u64, instances: u32, bins: u32) -> BatchSetShape {
        BatchSetShape {
            range: MergedRange {
                id,
                instances,
                bin_metadata: bins,
                bin_index_map: bins + 1,
            },
            bin_count: bins,
            metadata: BatchSetSceneUnpackingMetadata {
                base_output_work_item_index: 1000 * id as u32,
                base_indirect_parameters_index: 100 * id as u32,
                batch_set_index: id as u32,
                first_indirect_parameters_index: 100 * id as u32,
                first_output_mesh_uniform_index: 7 * id as u32,
            },
        }
    }

    /// The batch set a thread of the merged dispatch finds, as `unpack_bins.wesl`
    /// and `allocate_uniforms.wesl` search: the last whose start is not past it.
    fn find(starts: &[u32], id: u32) -> usize {
        let (mut lo, mut hi) = (0, starts.len());
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if starts[mid] <= id {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    #[test]
    fn every_instance_and_chunk_finds_its_own_batch_set() {
        let sets = [
            shape(1, 3, 2),
            shape(2, 700, 300),
            shape(3, 1, 1),
            shape(4, 64, 256),
            shape(5, 65, 257),
        ];
        let plan = plan(&sets);
        assert_eq!(plan.instance_count, 3 + 700 + 1 + 64 + 65);
        assert_eq!(plan.chunk_count, 1 + 2 + 1 + 1 + 2);
        assert!(plan.multi_chunk);

        let instance_starts: Vec<u32> = plan.unpacking_jobs.iter().map(|j| j.instance_start).collect();
        let mut seen = vec![0u32; sets.len()];
        for id in 0..plan.instance_count {
            let job = find(&instance_starts, id);
            let local = id - plan.unpacking_jobs[job].instance_start;
            assert!(local < sets[job].range.instances, "instance {id} past its batch set");
            seen[job] += 1;
        }
        let counts: Vec<u32> = sets.iter().map(|s| s.range.instances).collect();
        assert_eq!(seen, counts, "each batch set's instances unpacked once");

        let chunk_starts: Vec<u32> = plan.allocation_jobs.iter().map(|j| j.first_chunk).collect();
        let mut chunks = vec![0u32; sets.len()];
        for chunk in 0..plan.chunk_count {
            chunks[find(&chunk_starts, chunk)] += 1;
        }
        let wanted: Vec<u32> = sets.iter().map(|s| s.bin_count.div_ceil(256)).collect();
        assert_eq!(chunks, wanted, "each batch set's chunks scanned once");

        // Each batch set's bins and map start where the previous one's end.
        let mut bin_metadata = 0;
        let mut bin_index_map = 0;
        for ((set, unpacking), allocation) in sets
            .iter()
            .zip(&plan.unpacking_jobs)
            .zip(&plan.allocation_jobs)
        {
            assert_eq!(unpacking.bin_metadata_start, bin_metadata);
            assert_eq!(allocation.bin_metadata_start, bin_metadata);
            assert_eq!(unpacking.bin_index_map_start, bin_index_map);
            assert_eq!(unpacking.base_output_work_item_index, set.metadata.base_output_work_item_index);
            assert_eq!(allocation.first_output_mesh_uniform_index, set.metadata.first_output_mesh_uniform_index);
            bin_metadata += set.range.bin_metadata;
            bin_index_map += set.range.bin_index_map;
        }
    }

    #[test]
    fn small_batch_sets_need_no_global_scan() {
        let plan = plan(&[shape(1, 10, 256), shape(2, 10, 1)]);
        assert!(!plan.multi_chunk);
        assert_eq!(plan.chunk_count, 2);
    }

    #[test]
    fn the_same_batch_sets_at_the_same_lengths_keep_the_layout() {
        let a = plan(&[shape(1, 10, 2), shape(2, 5, 1)]);
        assert_eq!(a.layout, plan(&[shape(1, 10, 2), shape(2, 5, 1)]).layout);
        assert_ne!(a.layout, plan(&[shape(2, 5, 1), shape(1, 10, 2)]).layout);
        assert_ne!(a.layout, plan(&[shape(1, 11, 2), shape(2, 5, 1)]).layout);
        assert_ne!(a.layout, plan(&[shape(1, 10, 2), shape(3, 5, 1)]).layout);
    }
}
