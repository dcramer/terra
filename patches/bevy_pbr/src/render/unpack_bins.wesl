//! A compute shader that unpacks bins.
//!
//! This shader runs before mesh preprocessing in order to generate
//! `PreprocessWorkItem`s from cached bins. One invocation of this shader
//! corresponds to one binned entity. Each shader invocation builds the work item
//! for its entity by copying over the mesh input uniform index and calculating
//! the position of the command in the indirect parameters buffer that will draw
//! that entity.
//!
//! Terra's patch (D613): Bevy 0.20 dispatched this once a batch set. A dispatch
//! now unpacks every batch set of one (view, phase, mesh class), their buffers
//! laid end to end; each invocation finds its batch set's `BinUnpackingJob` by a
//! binary search over the dispatch's jobs, as GPU-driven renderers find a draw's
//! record through a table of offsets.

import bevy_render::occlusion_culling::mesh_preprocess_types::{BinMetadata, PreprocessWorkItem};

/// Information needed to unpack the bins of one dispatch's batch sets.
struct BinUnpackingMetadata {
    /// The index of this dispatch's first `BinUnpackingJob` in `jobs`.
    first_job: u32,
    /// The number of batch sets in this dispatch.
    job_count: u32,
    /// The number of binned mesh instances in the `binned_mesh_instances`
    /// array, every batch set's.
    binned_mesh_instance_count: u32,
    /// Padding.
    pad_a: u32,
    /// Padding.
    pad_b: array<vec4<u32>, 15>,
};

/// Where one batch set's bins lie (CPU-side `GpuBinUnpackingJob`).
struct BinUnpackingJob {
    /// The batch set's first binned mesh instance in `binned_mesh_instances`.
    instance_start: u32,
    /// The number of binned mesh instances in the batch set.
    instance_count: u32,
    /// The index of the first `PreprocessWorkItem` that the batch set writes to.
    base_output_work_item_index: u32,
    /// The index of the first GPU indirect parameters command for the batch set.
    base_indirect_parameters_index: u32,
    /// The batch set's first `BinMetadata` in `bin_metadata`.
    bin_metadata_start: u32,
    /// The batch set's first entry in `bin_index_to_bin_metadata_index`.
    bin_index_map_start: u32,
    /// Padding.
    pad_a: u32,
    /// Padding.
    pad_b: u32,
};

/// One mesh instance in a bin.
///
/// This corresponds to the CPU-side `GpuRenderBinnedMeshInstance` structure.
///
/// Note that this structure isn't sorted within the
/// `binned_mesh_instances` buffer. Instances from the same bin aren't
/// guaranteed to be adjacent to one another.
struct BinnedMeshInstance {
    /// The index of the `MeshInputUniform` corresponding to this mesh instance
    /// in the `MeshInputUniform` buffer.
    input_uniform_index: u32,
    /// The index of the bin that this mesh instance belongs to, within its
    /// batch set.
    bin_index: u32,
};

/// Metadata for the dispatch.
@group(0) @binding(0) var<uniform> bin_unpacking_metadata: BinUnpackingMetadata;

/// The input array of `BinnedMeshInstance`s, every batch set's end to end.
///
/// Note that this array isn't sorted.
@group(0) @binding(1) var<storage> binned_mesh_instances: array<BinnedMeshInstance>;

/// The output list of `PreprocessWorkItem`s.
@group(0) @binding(2) var<storage, read_write> preprocess_work_items: array<PreprocessWorkItem>;

/// The bin metadata, which contains the index of the GPU indirect parameters for
/// each bin, relative to the start of the indirect parameters for its batch
/// set; every batch set's end to end.
///
/// This is indexed by the bin metadata index below.
@group(0) @binding(3) var<storage> bin_metadata: array<BinMetadata>;

/// A mapping from the bin index to the metadata index within the batch set's
/// part of the `bin_metadata` buffer; every batch set's end to end.
@group(0) @binding(4) var<storage> bin_index_to_bin_metadata_index: array<u32>;

/// Every batch set's `BinUnpackingJob` this frame, this dispatch's from
/// `first_job`.
@group(0) @binding(5) var<storage> jobs: array<BinUnpackingJob>;

@compute
@workgroup_size(64)
fn main(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    // Figure out which instance we're looking at.
    let global_id = global_invocation_id.x;
    if (global_id >= bin_unpacking_metadata.binned_mesh_instance_count) {
        return;
    }

    // Find its batch set: the last whose first instance is not past it.
    var lo = bin_unpacking_metadata.first_job;
    var hi = bin_unpacking_metadata.first_job + bin_unpacking_metadata.job_count;
    while (hi - lo > 1u) {
        let mid = (lo + hi) / 2u;
        if (jobs[mid].instance_start <= global_id) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let job = jobs[lo];
    let local_id = global_id - job.instance_start;

    // Unpack the `BinnedMeshInstance`.
    let input_uniform_index = binned_mesh_instances[global_id].input_uniform_index;
    let bin_index = binned_mesh_instances[global_id].bin_index;

    // Look up the indirect parameters index for this bin, relative to the first
    // indirect parameters offset for this batch set.
    let bin_metadata_index = job.bin_metadata_start +
        bin_index_to_bin_metadata_index[job.bin_index_map_start + bin_index];
    let indirect_parameters_offset = bin_metadata[bin_metadata_index].indirect_parameters_offset;

    // Determine the location we should write the work item to.
    let output_index = job.base_output_work_item_index + local_id;

    // Write out the resulting work item.
    preprocess_work_items[output_index].input_index = input_uniform_index;
    preprocess_work_items[output_index].output_or_indirect_parameters_index =
        job.base_indirect_parameters_index + indirect_parameters_offset;
}
