# Spike: gear on a body

Throwaway. Question: can generated equipment be fitted and skinned to one base body automatically, and move without tearing or poking through? Findings go in [research/ai-models-for-art.md](../../research/ai-models-for-art.md).

`fit.py` takes the sandbox's rigged figure and three refined pieces from the bake-off (a jerkin, boots, a helmet), and for each: fits it to its body region from the bones' weights, pushes out what sinks into the body, copies the body's skin weights onto it, binds it to the skeleton, and hides the body's faces under it. Then it plays idle, walk and a sword attack and renders a sheet to `out/`.

```bash
spike/model_bakeoff/run.sh ../character_gear/fit.py
```

## Found, 2026-10-11

- Fitting works for single pieces: the jerkin and helmet fit, follow the body through every pose, and with the skin under them hidden, nothing pokes through.
- A pair generated as one model (the boots) touches in the middle and leaves a flap when cut apart. Generate one boot and mirror it.
- The body is the problem, not the fitting: the sandbox's grey mannequin makes every character look like a test dummy. A great character needs its own base body, designed in the mood board's style, with a rig and blend shapes.

## Rigging a base body, 2026-10-11

`make_character.sh NAME SHEET.png HEIGHT` runs the whole pipeline on Studio credits: crop the front of a T-pose design sheet, make a textured model, rig it with Tripo's own skeleton ([tools/tripo](../../tools/tripo), `--skeleton tripo`), convert the rig onto our skeleton (`convert_rig.py`), check it (`rig_check.py`), render it moving (`rig_render.py`) and export it as the game reads it, with our clips re-sampled on its skeleton (`export_character.py`). `rig_base.py` is the fallback that rigs with Blender's own weights. The checker's baseline is the mannequin's own scores, written to `out/rig-baseline.json` by checking `figure.gltf` first. The step by step, with what to do when a check fails, is the `character` skill in `.claude/skills/character/SKILL.md`.
