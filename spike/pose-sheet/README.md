# Pose sheet to clip

Question: can an agent make a fight clip that meshes with our guard by reading a generated pose sheet and writing key poses on our own skeleton, with no motion generator in between?

The developer, 2026-11-01: "I have found using chatgpt to create a pose sheet of the animation sequence, and then I get opus to animate against the pose sheet reasonably effective. lets try this". Scope is one blow, the slash, on the player's body. Findings are in [the note](../../research/2026-11-01-pose-sheet-blow.md). This folder stays until the developer has judged the blow, then it moves to `tools/characters` or is deleted.

- `key.ts` turns `slash.json`, a list of key poses, into a clip: hips and chest, the sword hand's place and blade, the feet. Arms and legs are solved to reach them. The same keys always give the same clip.
- `slash-sheet.jpg` is the sheet the keys were read from.
- `stills.py` renders the clip at the sheet's panels from the sheet's camera, to lay beside it.
- Output goes in `out/`, which git ignores.

```bash
node spike/pose-sheet/key.ts spike/pose-sheet/slash.json spike/pose-sheet/out/body.glb
```
