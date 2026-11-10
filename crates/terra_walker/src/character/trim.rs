//! A worn look's skin cut down to the joints it uses (`super::Trimmed`), and whether a mesh
//! recorded as cut still is.

use super::used_joints;
use bevy::{
    mesh::{VertexAttributeValues, skinning::SkinnedMeshInverseBindposes},
    prelude::*,
};

/// A mesh's sets of four influences a vertex: both where it has eight (D601).
const SETS: [(
    bevy::mesh::MeshVertexAttribute,
    bevy::mesh::MeshVertexAttribute,
); 2] = [
    (Mesh::ATTRIBUTE_JOINT_INDEX, Mesh::ATTRIBUTE_JOINT_WEIGHT),
    (
        Mesh::ATTRIBUTE_JOINT_INDEX_1,
        Mesh::ATTRIBUTE_JOINT_WEIGHT_1,
    ),
];

/// Whether `mesh`, recorded as cut down to `kept` joints (`Trimmed`), no longer is: a vertex
/// has a weight on a joint past them, so its data is its file's again and the record is stale.
/// A mesh whose file's numbering is the cut's own reads as cut, rightly: the two are the same.
/// Left stale, the skin's indices run past its wearer's joints into other skins' matrices and
/// the figure is drawn in strips between bodies (held-explode j2).
pub fn untrimmed(mesh: &Mesh, kept: usize) -> bool {
    SETS.into_iter().any(|(index, weight)| {
        let (Some(VertexAttributeValues::Uint16x4(j)), Some(VertexAttributeValues::Float32x4(w))) =
            (mesh.attribute(index), mesh.attribute(weight))
        else {
            return false;
        };
        j.iter().zip(w).any(|(j, w)| {
            j.iter()
                .zip(w)
                .any(|(j, w)| *w > 0.0 && *j as usize >= kept)
        })
    })
}

/// Cuts mesh `id`'s skin down to the joints it uses, once: its joint indices renumbered and its
/// bind poses (from its file, before any body reshapes them) kept for those alone.
pub(super) fn trim(
    id: AssetId<Mesh>,
    poses: &Handle<SkinnedMeshInverseBindposes>,
    count: usize,
    meshes: &mut Assets<Mesh>,
    bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
) -> Option<(Vec<usize>, Handle<SkinnedMeshInverseBindposes>)> {
    let all: Vec<Mat4> = bindposes.get(poses)?.to_vec();
    let mut mesh = meshes.get_mut(id)?;
    let sets = SETS;
    let mut influences = (Vec::new(), Vec::new());
    let mut vertices = 0;
    for (i, (index, weight)) in sets.iter().enumerate() {
        match (mesh.attribute(*index), mesh.attribute(*weight)) {
            (
                Some(VertexAttributeValues::Uint16x4(j)),
                Some(VertexAttributeValues::Float32x4(w)),
            ) if j.len() == w.len() && (i == 0 || w.len() == vertices) => {
                vertices = w.len();
                influences.0.extend_from_slice(j);
                influences.1.extend_from_slice(w);
            }
            _ if i == 0 => return None,
            _ => {}
        }
    }
    let (kept, index) = used_joints(&influences.0, &influences.1, count);
    if kept.is_empty() || kept.len() == count || vertices == 0 {
        return None;
    }
    let mut weights = influences.1.chunks(vertices);
    for (joint_index, _) in sets {
        let Some(ws) = weights.next() else { break };
        let Some(VertexAttributeValues::Uint16x4(indices)) = mesh.attribute_mut(joint_index) else {
            break;
        };
        for (joints, ws) in indices.iter_mut().zip(ws) {
            for (j, w) in joints.iter_mut().zip(ws) {
                // An unused slot points at the first joint kept, with no weight.
                *j = if *w > 0.0 { index[*j as usize] } else { 0 };
            }
        }
    }
    // The glTF loader's bounds (its default `DynamicSkinnedMeshBounds`, the box frustum
    // culling tests each frame) name joints by the file's numbering: left so, they track
    // other joints or none, and a worn piece could be culled while it is on screen. Made
    // again from the renumbered skin. (Not the iso hero's roll, where his torso is hidden
    // behind his own hips as he dives, iso-hero j8.)
    if mesh.skinned_mesh_bounds().is_some()
        && let Err(e) = mesh.generate_skinned_mesh_bounds()
    {
        warn!(
            "a worn mesh's bounds could not follow its trimmed skin ({e:?}): it may be culled on screen"
        );
        mesh.set_skinned_mesh_bounds(None);
    }
    let handle = bindposes.add(SkinnedMeshInverseBindposes::from(
        kept.iter().map(|i| all[*i]).collect::<Vec<_>>(),
    ));
    Some((kept, handle))
}
