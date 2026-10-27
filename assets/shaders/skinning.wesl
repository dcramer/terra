// Weight-blended skinning, in place of Bevy's linear blend for every skinned mesh
// (crates/terra_walker/src/skinning.rs, research/skin-deformation.md): each vertex is skinned
// partly with the linear blend and partly with dual quaternions, by a share of its own, as
// Maya's "weight blended" skinning and Blender's armature modifiers mixed by a group do.
// A linear blend of two joints turned far apart about a limb averages their matrices, and the
// skin between collapses to a neck (the "candy wrapper"); dual quaternions turn it instead,
// keeping its volume (Kavan et al., "Skinning with Dual Quaternions", 2007), but bulge at a
// wide, deep bend such as the hips. skinning.rs works each vertex's share out from its joints
// (assets/models/character/skinning.json) and carries it in its joint weights, which then sum
// to one plus the share; weights that sum to one, as any mesh it did not touch has, are skinned
// with the linear blend alone, as Bevy would.
//
// Bevy's own render/skinning.wesl with only `skin_model` and `skin_prev_model` changed; the
// rest, and every binding, as it has them. It stands in for Bevy's as the module
// `bevy_pbr::render::skinning`, so `package` here is `bevy_pbr`. A joint matrix here may carry
// a uniform scale (a skeleton scaled as an object, a body made taller), blended on its own;
// never a shear. That split, scale blended linearly beside the rigid part's dual quaternions,
// is the two-phase skinning of Kavan et al., "Geometric Skinning with Approximate Dual
// Quaternion Blending" (ACM TOG 27(4), 2008), on joints that are not rigid. Bevy's joint matrices (joint
// GlobalTransform x inverse bindpose, bevy_pbr render/skin.rs) carry a rig's own scale only
// where the bindposes do not cancel it: the Cetri's Rig node scales 3.296 and its matrices 1.0.
// TERRA_DUAL_QUATERNION_SKINNING (skinning.rs looks for this to know the shader is ours)

import package::render::mesh_types::SkinnedMesh;
import package::render::mesh_bindings::mesh;

@if(SKINNED && SKINS_USE_UNIFORM_BUFFERS)
@group(2) @binding(1) var<uniform> joint_matrices: SkinnedMesh;
@if(SKINNED && !SKINS_USE_UNIFORM_BUFFERS)
@group(2) @binding(1) var<storage> joint_matrices: array<mat4x4<f32>>;

// An array of matrices specifying the joint positions from the previous frame.
//
// This is used for motion vector computation.
//
// If this is the first frame, or we're otherwise prevented from using data from
// the previous frame, this is simply the same as `joint_matrices` above.
@if(SKINNED && SKINS_USE_UNIFORM_BUFFERS)
@group(2) @binding(6) var<uniform> prev_joint_matrices: SkinnedMesh;
@if(SKINNED && !SKINS_USE_UNIFORM_BUFFERS)
@group(2) @binding(6) var<storage> prev_joint_matrices: array<mat4x4<f32>>;

// The rotation of a rotation matrix as a unit quaternion (x, y, z, w).
@if(SKINNED)
fn quat_from_rotation(m: mat3x3<f32>) -> vec4<f32> {
    let trace = m[0].x + m[1].y + m[2].z;
    if trace > 0.0 {
        let s = sqrt(trace + 1.0) * 2.0;
        return vec4<f32>((m[1].z - m[2].y) / s, (m[2].x - m[0].z) / s, (m[0].y - m[1].x) / s, 0.25 * s);
    }
    if m[0].x > m[1].y && m[0].x > m[2].z {
        let s = sqrt(1.0 + m[0].x - m[1].y - m[2].z) * 2.0;
        return vec4<f32>(0.25 * s, (m[1].x + m[0].y) / s, (m[2].x + m[0].z) / s, (m[1].z - m[2].y) / s);
    }
    if m[1].y > m[2].z {
        let s = sqrt(1.0 + m[1].y - m[0].x - m[2].z) * 2.0;
        return vec4<f32>((m[1].x + m[0].y) / s, 0.25 * s, (m[2].y + m[1].z) / s, (m[2].x - m[0].z) / s);
    }
    let s = sqrt(1.0 + m[2].z - m[0].x - m[1].y) * 2.0;
    return vec4<f32>((m[2].x + m[0].z) / s, (m[2].y + m[1].z) / s, 0.25 * s, (m[0].y - m[1].x) / s);
}

// One joint's matrix as a dual quaternion (real, dual) and its uniform scale.
@if(SKINNED)
struct Joint {
    real: vec4<f32>,
    dual: vec4<f32>,
    scale: f32,
}

@if(SKINNED)
fn joint_of(m: mat4x4<f32>) -> Joint {
    let scale = length(m[0].xyz);
    let q = quat_from_rotation(mat3x3<f32>(m[0].xyz / scale, m[1].xyz / scale, m[2].xyz / scale));
    let t = m[3].xyz;
    // dual = 0.5 * (t, 0) * q
    let dual = 0.5 * vec4<f32>(q.w * t + cross(t, q.xyz), -dot(t, q.xyz));
    return Joint(q, dual, scale);
}

// The blend of four joints, as a matrix: each joint's dual quaternion weighted, the shorter way
// round from the first (q and -q are the same rotation), then normalised.
@if(SKINNED)
fn blend(a: Joint, b: Joint, c: Joint, d: Joint, weights: vec4<f32>) -> mat4x4<f32> {
    let wb = select(weights.y, -weights.y, dot(a.real, b.real) < 0.0);
    let wc = select(weights.z, -weights.z, dot(a.real, c.real) < 0.0);
    let wd = select(weights.w, -weights.w, dot(a.real, d.real) < 0.0);
    var real = weights.x * a.real + wb * b.real + wc * c.real + wd * d.real;
    var dual = weights.x * a.dual + wb * b.dual + wc * c.dual + wd * d.dual;
    let scale = dot(weights, vec4<f32>(a.scale, b.scale, c.scale, d.scale));
    let n = length(real);
    real = real / n;
    dual = dual / n;
    let x = real.x;
    let y = real.y;
    let z = real.z;
    let w = real.w;
    // translation = 2 * dual * conjugate(real)
    let t = 2.0 * (w * dual.xyz - dual.w * real.xyz + cross(real.xyz, dual.xyz));
    return mat4x4<f32>(
        vec4<f32>(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y), 0.0) * scale,
        vec4<f32>(2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x), 0.0) * scale,
        vec4<f32>(2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y), 0.0) * scale,
        vec4<f32>(t, 1.0),
    );
}

// The skin matrix of four joints by `weights`: their linear blend, mixed with their dual
// quaternion blend by the share the weights carry (see above).
@if(SKINNED)
fn skinned(m0: mat4x4<f32>, m1: mat4x4<f32>, m2: mat4x4<f32>, m3: mat4x4<f32>, weights: vec4<f32>) -> mat4x4<f32> {
    let total = weights.x + weights.y + weights.z + weights.w;
    let w = weights / total;
    let linear = w.x * m0 + w.y * m1 + w.z * m2 + w.w * m3;
    let share = clamp(total - 1.0, 0.0, 1.0);
    if share < 0.001 {
        return linear;
    }
    let dual = blend(joint_of(m0), joint_of(m1), joint_of(m2), joint_of(m3), w);
    return linear * (1.0 - share) + dual * share;
}

@if(SKINNED)
fn skin_model(
    indexes: vec4<u32>,
    weights: vec4<f32>,
    instance_index: u32,
) -> mat4x4<f32> {
@if(SKINS_USE_UNIFORM_BUFFERS)
    return skinned(
        joint_matrices.data[indexes.x],
        joint_matrices.data[indexes.y],
        joint_matrices.data[indexes.z],
        joint_matrices.data[indexes.w],
        weights,
    );
@if(!SKINS_USE_UNIFORM_BUFFERS)
    let skin_index = mesh[instance_index].current_skin_index;
@if(!SKINS_USE_UNIFORM_BUFFERS)
    return skinned(
        joint_matrices[skin_index + indexes.x],
        joint_matrices[skin_index + indexes.y],
        joint_matrices[skin_index + indexes.z],
        joint_matrices[skin_index + indexes.w],
        weights,
    );
}

// Returns the skinned position of a vertex with the given weights from the
// previous frame.
//
// This is used for motion vector computation.
@if(SKINNED)
fn skin_prev_model(
    indexes: vec4<u32>,
    weights: vec4<f32>,
    instance_index: u32,
) -> mat4x4<f32> {
@if(SKINS_USE_UNIFORM_BUFFERS)
    return skinned(
        prev_joint_matrices.data[indexes.x],
        prev_joint_matrices.data[indexes.y],
        prev_joint_matrices.data[indexes.z],
        prev_joint_matrices.data[indexes.w],
        weights,
    );
@if(!SKINS_USE_UNIFORM_BUFFERS)
    let skin_index = mesh[instance_index].current_skin_index;
@if(!SKINS_USE_UNIFORM_BUFFERS)
    return skinned(
        prev_joint_matrices[skin_index + indexes.x],
        prev_joint_matrices[skin_index + indexes.y],
        prev_joint_matrices[skin_index + indexes.z],
        prev_joint_matrices[skin_index + indexes.w],
        weights,
    );
}

@if(SKINNED)
fn inverse_transpose_3x3m(in: mat3x3<f32>) -> mat3x3<f32> {
    let x = cross(in[1], in[2]);
    let y = cross(in[2], in[0]);
    let z = cross(in[0], in[1]);
    let det = dot(in[2], z);
    return mat3x3<f32>(
        x / det,
        y / det,
        z / det
    );
}

@if(SKINNED)
fn skin_normals(
    world_from_local: mat4x4<f32>,
    normal: vec3<f32>,
) -> vec3<f32> {
    return normalize(
        inverse_transpose_3x3m(
            mat3x3<f32>(
                world_from_local[0].xyz,
                world_from_local[1].xyz,
                world_from_local[2].xyz
            )
        ) * normal
    );
}
