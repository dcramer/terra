# Role pieces

Question: which way of giving each exposed face, edge and corner of a cube a look chosen by its role and neighbours brings stone, logs and roofs on par with Enshrouded's blending (D211)? Findings: [research/role-pieces.md](../../research/role-pieces.md). Nothing depends on this spike; delete it once the developers have judged.

- `src/roles.rs`: the role function, Bevy-free: each cube's faces by what is beyond them, its edges by role (arris, jamb, crest, sill, soffit, head, foot), gathered into runs with their mitres. `cargo test` runs its tests.
- `src/today.rs`: candidate 1, what the game draws today, from the game's own layouts.
- `src/layers.rs`, `src/masonry.rs`: candidate 2, the block surface with a role per point (material, inset, shade) and the masks.
- `src/profiles.rs`: candidate 3, profiles swept along role runs and corner stones, over 2's surface; timber's role members.
- `src/modules.rs`: candidate 4, a modelled module per exposed face, chosen by its edges' roles.
- `blender/pieces.py`: the profiles, corner stones and modules, authored in Blender.

## Running it

```bash
tools/blender/run.sh spike/role-pieces/blender/pieces.py $PWD/spike/role-pieces/out
cd spike/role-pieces && CARGO_TARGET_DIR=out/target cargo run --release   # or name cases: keen L-stone
```

It writes `assets/models/spike/RolePieces-<case>.glb` (ignored), each the case drawn by every candidate side by side, nodes named `RP-<candidate>-<case>` (1, 2, 3, 4, and 3f for 3 flush), and `out/counts.tsv` (case, candidate, triangles, materials, cubes, open faces). Stand them on the yard, after two empty spacer rows that keep them clear of the kit sheet's dug slope:

```bash
node tools/sandbox/remote.ts start --windows -- --experiment play --level yard --kit-sheet --sheet-rows none \
  --clean --time 14 --save role-pieces-spike.ron \
  --model models/spike/RolePieces-spacer.glb --model models/spike/RolePieces-spacer.glb \
  --model models/spike/RolePieces-keen-stone.glb   # and the others
node tools/sandbox/remote.ts look RP-3-keen-stone --distance 8.5 --around -35 --height 3.4 --aim 2.1
node tools/sandbox/remote.ts shot shots/keen-3.png
node tools/sandbox/remote.ts stop
```
