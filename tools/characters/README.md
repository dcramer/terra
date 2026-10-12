# Characters

The asset pipeline for bodies and what they wear and hold (D115, D128): Blender scripts that turn the source art in [art/](../../art/README.md) into the files the game loads in [assets/](../../assets). The step by step, with what to do when a check fails, is the `character` skill (`.claude/skills/character/SKILL.md`).

Run everything from the repository root. Blender scripts run headless through [tools/blender/run.sh](../blender/README.md); paths given to them are relative to the root. What a script writes and we do not keep goes to `tools/characters/out/`, which is ignored.

| Script | What it does |
|---|---|
| `make_character.sh NAME SHEET HEIGHT` | The whole body pipeline on Studio credits: keeps the sheet, its front view, Tripo's model and rig in `art/characters/NAME/`, then runs `finish_character.sh` |
| `finish_character.sh NAME HEIGHT` | The free part, rerun for every body when it improves: convert, check, render, export |
| `convert_rig.py` | Tripo's skeleton onto ours, with forearm twist bones (`twist.py`) |
| `rig_check.py` | Checks a rig against our skeleton and the mannequin, whose scores are `art/characters/rig-baseline.json` |
| `rig_render.py` | A sheet of the body through our animations |
| `export_character.py` | The body as the game loads it: skeleton, sockets, our clips re-sampled on it, skin patches (`patches.py`) |
| `rig_base.py` | The fallback: rigs a body with Blender's own weights |
| `shell_garment.py` | Clothing cut from the base body, from a model of that body wearing it (`art/gear/dressed-<body>-<item>/`) |
| `fit_gear.py` | Rigid gear (a pack, a quiver) fitted to a body at its socket, from `art/gear/<item>/`; straps from the skin (`bands.py`) |
| `gear_check.py` | How much skin comes through each worn piece, through our clips |
| `fit_held.py` | The held look of each tool, from the free models in `assets/models/tools` |

`look.py` holds what the scripts share: paths, our palette and light, and contact sheets.

```bash
tools/characters/finish_character.sh base-male 1.80
cp tools/characters/out/base-male-game.glb assets/models/character/base-male.glb
```
