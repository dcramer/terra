# Creatures

Blender scripts that make the game's animals from their source art in `art/creatures/`, for animals whose bodies Tripo's auto-rig does not serve (docs/demos.md, a living town).

- `hen.py` makes `assets/models/creatures/Chicken.glb` from Tripo's export in `art/creatures/chicken/model.glb`: real size, feet at the origin, facing +Z as `Deer.glb` does, re-skinned on a skeleton of our own (Tripo's avian rig weighted the feet to the tail), with `Idle`, `Walk` (in place, 0.375 m/s) and `Peck` keyed through leg IK and baked.
- `sheet.py` renders a preview sheet of a model beside the figure for scale, and frames of its clips, to check against docs/art-style.md.

Run both through `tools/blender/run.sh`, and anything that renders under the GPU's turn: `flock ~/.cache/terra/gpu.lock tools/blender/run.sh tools/creatures/sheet.py assets/models/creatures/Chicken.glb sheet.png`.
