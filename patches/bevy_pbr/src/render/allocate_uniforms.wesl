//! A compute shader that allocates `MeshUniform`s.
//!
//! This shader runs before mesh preprocessing in order to determine the
//! positions of `MeshUniform`s. Unlike `MeshInputUniform`s, which are scattered
//! throughout the buffer, `MeshUniform`s are indexed by instance ID, and so we
//! must place instances of the same mesh together in the buffer. One thread
//! corresponds to one bin (a.k.a. draw, a.k.a. batch).
//!
//! Terra's patch (D613): Bevy 0.20 dispatched each step once a batch set (i.e.
//! one multidraw operation). A dispatch of each step now covers every batch set
//! of one (view, phase, mesh class), their bins laid end to end, each batch
//! set's numbers an `UniformAllocationJob`: a workgroup of the local scan and
//! the fan finds its batch set by a binary search over the dispatch's jobs by
//! its chunk, and the global scan runs a workgroup a batch set.
//!
//! Essentially, the goal of this shader is to perform a prefix sum, using the
//! "scan-then-fan" approach. It has three phases:
//!
//! 1. *Local scan*: Perform a [Hillis-Steele scan] on each chunk of draws, where
//! the size of each chunk (i.e. the number of draws) is equal to the workgroup
//! size (256). Write the total size for this chunk to the fan buffer.
//!
//! 2. *Global scan*: Do a Hillis-Steele scan on the fan buffer. Now we know the
//! running total for each chunk.
//!
//! 3. *Fan*: Copy the running total for each chunk to every element of that
//! chunk.
//!
//! Note that, for batch sets (i.e. multidraw indirect calls) that have fewer
//! than 256 batches in them, we only need step (1). This is the common case.
//!
//! [Hillis-Steele scan]: https://en.wikipedia.org/wiki/Prefix_sum#Algorithm_1:_Shorter_span,_more_parallel

import bevy_render::occlusion_culling::mesh_preprocess_types::{BinMetadata, IndirectParametersMetadata};

/// Information needed to allocate `MeshUniform`s for one dispatch's batch sets.
struct UniformAllocationMetadata {
    /// The index of this dispatch's first `UniformAllocationJob` in `jobs`.
    first_job: u32,

    /// The number of batch sets in this dispatch.
    job_count: u32,

    /// The number of chunks of 256 bins, every batch set's.
    chunk_count: u32,

    /// Padding.
    pad_a: u32,

    /// Padding.
    pad: array<vec4<f32>, 15u>,
};

/// One batch set's numbers (CPU-side `GpuUniformAllocationJob`).
struct UniformAllocationJob {
    /// The index of this batch set in the `IndirectBatchSet` array.
    ///
    /// We write this into the `indirect_parameters_metadata`.
    batch_set_index: u32,

    /// The number of bins (a.k.a. draws, a.k.a. batches) in this batch set.
    bin_count: u32,

    /// The index of the first set of indirect parameters for this batch set.
    ///
    /// This is also the index of the first `IndirectParametersMetadata`, as
    /// that's a parallel array with the indirect parameters.
    first_indirect_parameters_index: u32,

    /// The index of the first `MeshUniform` slot for this batch set.
    first_output_mesh_uniform_index: u32,

    /// The batch set's first `BinMetadata` in `bin_metadata`.
    bin_metadata_start: u32,

    /// The batch set's first chunk in the dispatch and in `fan_buffer`.
    first_chunk: u32,

    /// Padding.
    pad_a: u32,

    /// Padding.
    pad_b: u32,
};

/// The number of threads in a workgroup.
const WORKGROUP_SIZE: u32 = 256u;

/// Information needed to allocate `MeshUniform`s.
@group(0) @binding(0) var<uniform> allocate_uniforms_metadata: UniformAllocationMetadata;

/// Information for each bin, including the indirect parameters offset and the
/// instance count; every batch set's end to end.
@group(0) @binding(1) var<storage> bin_metadata: array<BinMetadata>;

/// The array of indirect parameters metadata that we fill out, one for each
/// batch.
@group(0) @binding(2) var<storage, read_write> indirect_parameters_metadata:
    array<IndirectParametersMetadata>;

/// A temporary buffer that stores the mesh uniform index of the last instance
/// plus one for each workgroup (i.e. for each 256-bin chunk), every batch set's
/// chunks from its `first_chunk`.
///
/// This is accumulated in the second stage and written out in the third.
@group(0) @binding(3) var<storage, read_write> fan_buffer: array<u32>;

/// Every batch set's `UniformAllocationJob` this frame, this dispatch's from
/// `first_job`.
@group(0) @binding(4) var<storage> jobs: array<UniformAllocationJob>;

/// Scratch memory that stores the prefix sum for every element in our chunk.
var<workgroup> output_offsets: array<u32, 256>;

/// The index in `jobs` of the batch set this workgroup works on.
var<workgroup> workgroup_job: u32;

/// The first step of the prefix sum. This computes the prefix sum for each
/// 256-element chunk.
///
/// Note that this will be the *only* step in the operation if the total number
/// of bins in this batch set is 256 or fewer. Thus we must fill in the indirect
/// parameters metadata for each batch here, as we can't guarantee that the
/// following two steps will be run at all.
@compute @workgroup_size(256, 1, 1)
fn allocate_local_scan(
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let job = jobs[find_job(local_id.x, group_id.x)];
    let chunk = group_id.x - job.first_chunk;
    let bin_count = job.bin_count;

    let block_start = chunk * WORKGROUP_SIZE;
    let block_end = min(block_start + WORKGROUP_SIZE, bin_count);
    let bin = block_start + local_id.x;

    // If this is the first workgroup, take the first output index from the
    // metadata into account. But if this is the second chunk or beyond, don't
    // do that, as the second and third phases will add it in and we don't want
    // to double-count it.
    if (chunk == 0u) {
        output_offsets[local_id.x] = job.first_output_mesh_uniform_index;
    } else {
        output_offsets[local_id.x] = 0u;
    }
    workgroupBarrier();

    // We're doing an inclusive sum, so put the instance count in the *next* bin.
    if (bin < block_end && local_id.x < WORKGROUP_SIZE - 1u) {
        output_offsets[local_id.x + 1] = bin_metadata[job.bin_metadata_start + bin].instance_count;
    }
    workgroupBarrier();

    // Prefix sum within our workgroup.
    hillis_steele_scan(local_id.x);

    // Now write the indirect parameters metadata for this batch. We fill in the
    // `base_output_index` with the value of the prefix sum (which might be
    // incomplete if this isn't the first chunk). We also populate a few
    // bookkeeping fields for later rendering passes to use.
    if (bin < block_end) {
        let indirect_parameters_offset = job.first_indirect_parameters_index +
            bin_metadata[job.bin_metadata_start + bin].indirect_parameters_offset;
        indirect_parameters_metadata[indirect_parameters_offset].base_output_index =
            output_offsets[local_id.x];
        indirect_parameters_metadata[indirect_parameters_offset].batch_set_index =
            job.batch_set_index;
        // These parameters get filled in later. Initialize them to zero for now.
        // This is required in the case of the early/late instance counts
        // because the mesh preprocessing shader will atomically increment them.
        indirect_parameters_metadata[indirect_parameters_offset].mesh_index = 0u;
        indirect_parameters_metadata[indirect_parameters_offset].early_instance_count = 0u;
        indirect_parameters_metadata[indirect_parameters_offset].late_instance_count = 0u;
    }

    // If this is the last element in the workgroup, put the total number of
    // instances (plus the first output mesh uniform index if we're the first
    // workgroup) in the fan buffer in preparation for the next phase.
    if (local_id.x == WORKGROUP_SIZE - 1u) {
        var chunk_total = output_offsets[WORKGROUP_SIZE - 1u];
        if (bin < block_end) {
            chunk_total += bin_metadata[job.bin_metadata_start + bin].instance_count;
        }
        fan_buffer[job.first_chunk + chunk] = chunk_total;
    }
}

/// The second step of the prefix sum.
///
/// This step takes the intermediate fan values computed in the previous step
/// (i.e. the sum going out of each chunk) and performs one or more Hillis-Steele
/// scans in order to compute the fan value going into each chunk.
///
/// This step is omitted if no batch set has more than 256 draws. A workgroup
/// scans one batch set's chunks, and does nothing for a batch set of one chunk.
@compute @workgroup_size(256, 1, 1)
fn allocate_global_scan(
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let job = jobs[allocate_uniforms_metadata.first_job + group_id.x];
    var sum = 0u;
    let chunk_count = div_ceil(job.bin_count, WORKGROUP_SIZE);
    if (chunk_count <= 1u) {
        return;
    }

    // Do a sequential loop over each block of 256 chunks. Because each
    // iteration of this loop covers 64K meshes, the fact that it's sequential
    // isn't going to be a problem in practice.
    for (var block_start = 0u; block_start < chunk_count; block_start += WORKGROUP_SIZE) {
        // Set up the Hillis-Steele scan.
        let block_end = min(block_start + WORKGROUP_SIZE, chunk_count);
        let global_id = block_start + local_id.x;
        if (global_id < block_end) {
            output_offsets[local_id.x] = fan_buffer[job.first_chunk + global_id];
        }
        workgroupBarrier();

        // Perform the scan.
        hillis_steele_scan(local_id.x);

        // Write the value back.
        // Note that we don't need a workgroup barrier here because
        // `hillis_steele_scan` already did one.
        if (global_id < block_end) {
            fan_buffer[job.first_chunk + global_id] = sum + output_offsets[local_id.x];
        }

        // Save the sum coming out of this block for the next one.
        sum += output_offsets[WORKGROUP_SIZE - 1u];
        workgroupBarrier();
    }
}

/// The third step of the prefix sum.
///
/// We take the summed fan value computed in the previous step and add it in to
/// each value of each chunk beyond the first. A workgroup a chunk, as in step
/// (1); a batch set's first chunk has nothing to do.
///
/// This step is omitted if no batch set has more than 256 draws.
@compute @workgroup_size(256, 1, 1)
fn allocate_fan(
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let job = jobs[find_job(local_id.x, group_id.x)];
    let chunk = group_id.x - job.first_chunk;
    let id = chunk * WORKGROUP_SIZE + local_id.x;
    if (chunk == 0u || id >= job.bin_count) {
        return;
    }

    let fan_value = fan_buffer[job.first_chunk + chunk - 1u];
    let indirect_parameters_offset = job.first_indirect_parameters_index +
        bin_metadata[job.bin_metadata_start + id].indirect_parameters_offset;
    indirect_parameters_metadata[indirect_parameters_offset].base_output_index += fan_value;
}

/// Finds the index in `jobs` of the batch set whose chunks hold this
/// workgroup's chunk, `group`: the last of the dispatch's jobs whose first chunk
/// is not past it. One invocation searches and the workgroup shares its answer,
/// so it is uniform across the workgroup.
fn find_job(local_id: u32, group: u32) -> u32 {
    if (local_id == 0u) {
        var lo = allocate_uniforms_metadata.first_job;
        var hi = allocate_uniforms_metadata.first_job + allocate_uniforms_metadata.job_count;
        while (hi - lo > 1u) {
            let mid = (lo + hi) / 2u;
            if (jobs[mid].first_chunk <= group) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        workgroup_job = lo;
    }
    return workgroupUniformLoad(&workgroup_job);
}

/// Calculates a running exclusive sum.
/// https://en.wikipedia.org/wiki/Prefix_sum#Algorithm_1:_Shorter_span,_more_parallel
fn hillis_steele_scan(local_id: u32) {
    for (var offset = 1u; offset < WORKGROUP_SIZE; offset *= 2u) {
        var term = 0u;
        if (local_id >= offset) {
            term = output_offsets[local_id - offset];
        }
        workgroupBarrier();
        output_offsets[local_id] += term;
        workgroupBarrier();
    }
}

/// Divides unsigned integer a by b, rounding up.
fn div_ceil(a: u32, b: u32) -> u32 {
    return (a + b - 1u) / b;
}
