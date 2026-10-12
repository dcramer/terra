# What Bevy gives a character system, and what we build

2026-10-12. **Question:** for bodies with morphs, gear on a shared skeleton and many characters on screen, what does Bevy 0.19.1 (our pin) do, at what cost, and what changes in 0.20?

**Method:** a read of Bevy's source; paths are under `bevy_*-0.19.1/`. Our assets were measured too:
- **Body:** 27,511 vertices, 4 morphs, 73 joints.
- **Each garment:** carries all 73 joints.
- **Sizes:** coat 7,534 vertices, cap 923, packs about 5,000 in two parts.

## What Bevy does

| Area | Bevy 0.19 | Limits and costs that matter |
|---|---|---|
| **Morph targets** | 256 per mesh (`bevy_mesh/src/morph.rs:15`). On native, 48 bytes per vertex per target in a storage slab | Our body costs 1.3 MB per morph, per mesh asset, shared by every instance. The shader loops over every target and skips zero weights, so the cost is non-zero morphs × vertices × passes, shadows included. Weights are uploaded every frame for every visible morphed mesh |
| **Sharing morph weights** | `MeshMorphWeights::Reference` lets many meshes read one weight set (`bevy_mesh/src/morph.rs:120`) | Matched **by index, not name**: a garment's morphs must be in the body's order. A clip that animates morph weights **overwrites the whole set**, so face animation must not share the body-shape weights |
| **Skinning** | Four influences; 256 joints (enforced only on WebGL2). On native, one storage buffer; skinned meshes batch | **Joint matrices are per skinned mesh, not per skeleton** (`bevy_pbr/src/render/skin.rs:425`), computed on the CPU and the whole buffer uploaded every frame. A dressed character is 5 skinned meshes × 76 joints, about 24 KB a frame: at 1,000 characters, 24 MB a frame and 380,000 matrix products in one thread |
| **Culling** | New in 0.19: skinned bounds from per-joint boxes, every frame | They **ignore morphs and custom vertex shaders**: a heavy body or our blended skinning may be culled at a screen edge |
| **Custom vertex data** | A material extension can add attributes | A new vertex layout means a new slab and pipeline, splitting batches. Our trick of carrying the dual-quaternion share in the joint weights avoids that |
| **Runtime mesh building** | `Mesh::merge` appends attributes only | It does **not** merge morphs or remap joints, so a per-character merge is ours to write. Changing a mesh re-uploads all of it, morphs included |
| **Animation** | Graph with blend and additive nodes, up to 64 masks, crossfades, events, morph weight curves | **No IK, root motion, rest-pose retargeting or spring bones.** Systems slot in after animation and before transform propagation, as ours do |
| **IK crates** | `bevy_mod_inverse_kinematics` 0.12 (runs before animation; would need rescheduling); `bevy_skeleton_modifiers` 0.1.1 (Godot port: two-bone, FABRIK and CCD IK and spring chains, after animation; brand new) | Adopt with care, or build: two-bone IK and spring bones are small |
| **Secondary motion** | `bevy_vrm1` spring bones (early, VRM-only setup); `bevy_silk` and `bevy_verlet` on 0.17 | No mature cloth for 0.19 |
| **LOD and crowds** | Distance ranges with dithered fades, per entity; bindless `StandardMaterial` (2048 a slab, 64 on macOS); `MeshTag` for per-instance data; two-phase occlusion culling | **No mesh simplification.** An `ExtendedMaterial` is bindless only if both halves are, and **our `BodyMask` is not**, so bodies won't multi-draw. Meshlets don't support skinning or morphs |

## Bevy 0.20 (released 2026-10-08)

- **Shaders move to WESL**, and its old import dialect is removed. Our skinning override (by the shader's path), `skinning.wgsl` and `body_mask.wgsl` all need rewriting.
- **Skinning and morph code is unchanged.**
- **Animation:** threaded subgraphs, so a face's bones skip the locomotion nodes.
- **Mesh extraction:** per-column change ticks, claimed 132 times faster.
- **Ecosystem:** `bevy-inspector-egui` doesn't support 0.20 yet, so our pin (D31) holds.

## Hard constraints for our plan

1. **At most four influences per vertex.** Keep under 256 joints if WebGL2 ever matters.
2. **Skinned-mesh count per character multiplies CPU and upload cost.** Trim each garment's skin to the joints it uses, or merge a character's parts into one mesh.
3. **Morph cost is non-zero morphs × vertices × passes.** Never give a per-character mesh its own morph targets.
4. **Garments list their morphs in the body's order.** Body-shape weights are never animated by clips.
5. **Skinned bounds ignore morphs.** Pad them for heavy builds.
6. **Make the body's material bindless** (`BodyMask`), or crowds won't batch.
7. **IK, root motion, spring bones, per-character merging and mesh LOD are ours to build or adopt.** They run between animation and transform propagation.
8. **Moving to 0.20 means rewriting our shaders in WESL.** Spike the skinning override on 0.20 first.

## Worth measuring before relying on it

- A crowd of our dressed character (5 skinned meshes × 73 joints) at 100, 500 and 1,000, on the fast GPU and the integrated one: extract time and upload.
- The cost of 0, 10 and 30 non-zero morphs on the 27,500-vertex body, with shadows.
- Draw calls with the non-bindless `BodyMask`.
