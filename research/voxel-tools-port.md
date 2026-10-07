# Porting Voxel Tools to Rust

2026-10-05. Question: could we reimplement the parts of Voxel Tools we need in Rust, and how much is there?

## Answer

Yes. It is MIT-licensed, and we need roughly a seventh of it.

## Size

Counted at version 1.7, the one the Godot build pinned. About 142 thousand lines of C++ of its own, not counting bundled third-party code.

| Part | Lines | Needed in a Rust port? |
|---|---|---|
| Level-of-detail terrain (chunk tree, streaming, updates) | 9,800 | Yes, the core |
| Voxel storage and edited blocks | 8,100 | Partly |
| Editing tools (sphere, raycast) | 7,800 | A small part |
| Task engine | 3,100 | Partly; Rust has good equivalents |
| Transvoxel mesher | 4,600 | Not at first; the `transvoxel` crate covers it, with the gap below |
| Node-graph generators | 14,100 | No, our generator is plain Rust |
| Plant instancing | 9,200 | Later, or our own |
| Saving (region files, SQLite) | 8,200 | No, we store edits as operations |
| Blocky terrain, fixed-detail terrain, editor, detail rendering, utilities, tests | about 77,000 | No, or replaced by Rust's standard library |

That leaves on the order of 20 thousand lines of C++ behaviour to reproduce. Which parts count as needed is a judgement from reading the layout, not from attempting the port.

## Why a port is easier than writing it fresh

- The design is proven, and the scheduling and edge cases can be read from the source.
- The original runs headless on our machines, so a port can be tested against it: same density function in, compare voxel data and meshes out.
- We need one configuration: one terrain type, one mesher, one generator, sphere edits.

## Known gap in the transvoxel crate

Its README says a mesh has its seam stitching baked in: "it is not possible to 'flip' a transition face status on a block, without re-extracting a new mesh for the block". When a chunk's neighbour changes detail level, the chunk must be re-meshed.

Voxel Tools stores both vertex positions and lets the shader choose, so no re-meshing is needed. If re-meshing proves costly, its mesher is the part to port next.

## What a port does not give us

- Integration with Bevy's renderer and with physics has no reference to copy.
- We become the maintainers; upstream fixes stop arriving for free.

## Sources

- [Voxel Tools](https://github.com/Zylann/godot_voxel)
- [transvoxel crate](https://gnurfos.github.io/transvoxel_rs/)
