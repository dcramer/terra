# Block modules

Question (building-plan.md, step 0): **can stone built of pieces chosen by their neighbours, cut from Tripo's stone and conformed at their edges, look as good as the stone kit's whole sections, with no seams?** And beside it, the main other way: clean bevelled cubes carrying a real stone texture with its relief displaced, as Enshrouded seems to.

`modules.py` builds the same five shapes both ways from the same cubes: a straight wall, an L, a T, a wall with a cross carved in it, and a wall part-built.

- **A, conformed modules.** The three stone sections (`assets/models/kits/StoneWall*.glb`) are each cut into their 32 eighth-cube pieces. Each eighth of each cube takes a piece whose open sides match its own (a face, an edge, a corner, a top), from the section's place it would hold if sections tiled the wall, so neighbouring pieces mostly come from neighbouring places and their stones run on. Where they cannot, the seam is pressed into a mortar joint.
- **B, bevelled cubes.** The cubes' outer faces, bevelled, finely remeshed, displaced by Poly Haven's `stone_wall` height (CC0), textured with its colour and normal, then cut to about A's triangles.

```bash
tools/blender/run.sh spike/block-modules/modules.py assets/models/spike spike/block-modules/out assets/models/kits/StoneWall.glb assets/models/kits/StoneWall2.glb assets/models/kits/StoneWall3.glb
cargo run -p terra_sandbox -- --experiment play --level yard --kit-sheet --model models/spike/BlockModulesA.glb --model models/spike/BlockModulesB.glb
```

The kit sheet logs a vantage for each model. Its models (`assets/models/spike/`) and textures (`out/`) are ignored. The finding goes in a dated note in research, then this folder is deleted.
