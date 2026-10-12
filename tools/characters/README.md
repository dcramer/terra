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
| `build_bodies.sh` | Rebuilds every character asset the game loads from `art/`: the base, its gear, every morph, the skins, and the game's files |
| `make_morph.sh NAME "CHANGE"` or `NAME --design BODY.glb` | A morph of the base body end to end: design (free), model (55 credits), registration, export, gear carried and checked; skips what `art/` already keeps |
| `morph.py` | Registers the base onto a design's model: pose by the base's skeleton, shape by a pull smoothed over distance, back to the T-pose; the morph |
| `carry_gear.py` | A worn look made on the base carried onto a morph |
| `paint_skin.sh NAME FRONT` | A template's own skin, painted by Tripo on the base's mesh in its shape (`paint_skin.py`), baked onto the base's layout |
| `compare.py` | Bodies side by side, dressed with `a.glb+b.glb`, for a person to look at |
| `fit_held.py` | The held look of each tool, from the free models in `assets/models/tools` |

`look.py` holds what the scripts share: paths, our palette and light, and contact sheets.

Everything the game loads is rebuilt from `art/` by one command, free (the playbook's section 7):

```bash
tools/characters/build_bodies.sh
```

It writes `assets/models/character/base.glb` (the base body with every morph), `assets/models/character/skins/`, and `assets/models/gear/<item>.glb` (each worn look, made once, with the same morphs). Everything between, such as the base's own export `out/base-male-game.glb` and the gear as made on it `out/gear/base-male/`, stays in `tools/characters/out/`.
