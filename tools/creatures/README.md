# Creatures

Blender scripts that make the game's animals from their source art in `art/creatures/`, for animals whose bodies Tripo's auto-rig does not serve (docs/demos.md, a living town).

- `hen.py` makes `assets/models/creatures/Chicken.glb` from Tripo's export in `art/creatures/chicken/model.glb`: real size, feet at the origin, facing +Z as `Deer.glb` does, re-skinned on a skeleton of our own (Tripo's avian rig weighted the feet to the tail), with `Idle`, `Walk` (in place, 0.375 m/s) and `Peck` keyed through leg IK and baked.
- `sheet.py` renders a preview sheet of a model beside the figure for scale, and frames of its clips, to check against docs/art-style.md.
- `dragon.py` makes `assets/models/creatures/Dragon.glb` from Tripo's unrigged export in `art/creatures/dragon/model.glb`: 13 m nose to tail, front +Z, its origin the body's centre of mass since it flies; the mouth Tripo modelled shut cut open along the lips, lined and given teeth; skinned by bone heat on a skeleton of our own (spine, six neck bones, head and jaw, ten tail bones, hind legs, and each wing an arm with a thumb and four two-bone fingers), with a `Mouth` empty on the head for the breath, and `Fly`, `Glide`, `Hover`, `Breathe`, `FlyBreathe` and `Roar` keyed as loops at 30 fps, in place (the game moves and banks it). No clip library has wings, so the clips are made here.
- `dragon_sheet.py` renders the dragon's contact sheets: its rest pose beside the figure, and each clip at four moments from the front, the side and the three-quarter, with its head close and the mouth's socket marked. `cycles` renders on the CPU, so it needs no turn of the GPU.

Run them through `tools/blender/run.sh`, and anything that renders under the GPU's turn: `flock ~/.cache/terra/gpu.lock tools/blender/run.sh tools/creatures/sheet.py assets/models/creatures/Chicken.glb sheet.png`.
