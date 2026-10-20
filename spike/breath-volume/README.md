# Spike: a raymarched breath (D133)

One question: can a raymarched volumetric breath, the cinematic tier (docs/fire.md, "LOD and budgets"), read as AAA where the breath's cards have stalled at 3-3.5/10?

Not the game's breath, and nothing may depend on it. It is a shader made from main's `fire.wesl` and a small edit to `crates/terra_world/src/fire.rs`, applied in a throwaway checkout:

- `gas.wesl`: the breath's density, temperature and soot fields along the stream and in the fireball.
- `march.wesl`: the front-to-back march through the hull, ended at the depth prepass, emissive-absorbing, saying how much flame it found for the film mask.
- `assemble.sh <main fire.wesl> <out>`: main's shader with those two put in, the fireball marched, the rod widened to the whole stream, the sprites hidden and the mask written (`edits.ts`).
- `edits-rs.ts <fire.rs>`: draws the hulls a second time into the alpha (the film mask), as the hearths' flames are.

The answer is in research/ (2026-10-19-raymarched-breath.md).
