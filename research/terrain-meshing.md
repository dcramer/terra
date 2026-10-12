# Meshing the terrain: how fast, and what it took

2026-10-11. Question: can `terra_voxel` mesh a 32-cubed chunk of a real generated planet within the 6 ms the toolchain spike measured, on Linux and Windows, and what stands in the way?

## Answer

Yes, at about half the budget, once nearby voxels share the work of asking where the ground is. Meshing itself was never the cost.

| Measured, release build, 40 chunks on the surface, average | Linux (WSL) | Windows (RTX 5090 machine) |
|---|---|---|
| A 16-metre chunk, 32 voxels a side, each voxel asked on its own | 12.8 ms | not measured |
| The same, with the land's heights shared through a lattice | 3.1 ms | 3.5 ms |
| With two transition faces toward finer chunks | 4.4 ms | 5.1 ms |
| After 40 digs, one in each chunk | 4.3 ms | 4.9 ms |
| The mesher alone, on a plain sphere that costs nothing to ask about | 0.4 to 0.6 ms | 0.6 ms |

About 2,650 triangles a chunk.

Around one viewer on the ground of the 2 km test planet, the tree chooses 882 chunks in 7.8 ms, 525 of which hold surface, 1.14 million triangles in all, and meshes the lot in 4.2 s on one thread (Linux). That was 22.7 s until distant chunks blended fewer lattice points: a coarse chunk's voxels are metres apart, so they shared no lattice points and each paid for four. Now a coarse chunk blends every second, fourth or eighth point, the same exact points, and reads a seam with a finer chunk at the finer one's spacing, so they still meet without cracks (tested at levels 1 and 3). Run it with `cargo run --release -p terra_voxel`, or `cargo xtask win -p terra_voxel --release` on Windows.

## What it took

- **Asking the ground was 95% of the time.** Each voxel worked out the land's height from eight layers of noise. The spike's 6 ms came from a formula of five sine waves, which is why it was so much cheaper.
- **The fix is a lattice of heights**, a metre apart over six cube faces bent round the planet, worked out exactly and blended between. That is now the ground's definition for every question, so walking, ray casts, digging and meshing agree; nearby asks share lattice points through a memo. The blend is within 2 cm of the noise, and the land has no features finer than a few metres.
- **Materials are only asked near the surface.** Ore is another two layers of noise, and only voxels within two of the surface ever reach a vertex.

## A bug in the `transvoxel` crate

Version 2.0's `extract_from_field` with `FieldCaching::CacheCentralBlockOnly` reads each finer neighbour from the central block instead of the neighbour's, so a transition face samples the wrong place and the seam cracks. Its uncached path is right. `terra_voxel` caches the central block itself and relays neighbours with `block.high_res_neighbour_to(side)`, which is what the uncached path does; a test meshes a coarse chunk against four finer ones and finds no open edges. Not yet reported upstream.

## Limits

- Timing is single-threaded and per chunk; a game meshes many chunks across threads, which this does not measure.
- Edits are checked one by one per voxel; with many edits near a chunk the cost grows, and a spatial index is the next step when it shows.
- Not compared with Voxel Tools, the brief's other criterion: Godot is not installed on this machine.
