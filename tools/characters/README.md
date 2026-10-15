# Characters

The asset pipeline for bodies and what they wear and hold (D115, D128, D316): Blender scripts that turn the source art in [art/](../../art/README.md) into the files the game loads in [assets/](../../assets). The step by step, with what to do when a check fails, is the `character` skill (`.agents/skills/character/SKILL.md`).

Run everything from the repository root. Blender scripts run headless through [tools/blender/run.sh](../blender/README.md); paths given to them are relative to the root. What a script writes and we do not keep goes to `tools/characters/out/`, which is ignored.

| Script | What it does |
|---|---|
| `make_character.sh NAME SHEET HEIGHT` | The whole body pipeline on Studio credits: keeps the sheet, its front view, Tripo's model and rig in `art/characters/NAME/`, then runs `finish_character.sh` |
| `finish_character.sh NAME HEIGHT` | The free part, rerun for every body when it improves: convert, check, render, export |
| `convert_rig.py` | Tripo's V3 rig (Unreal skeleton) onto ours, keeping its twist bones and weights |
| `clips.py` | Cuts the clips in `art/animation/clips.txt` out of Quaternius's animation libraries into the game's mannequin file |
| `made_clips.py` | The clips we made rather than took from the libraries (`art/animation/made.json`), each put into the mannequin file in place of the library's: its best looping stride, carried onto our skeleton by world rotation from the T-pose, run in place, the upper limbs' roll handed to the forearm and shin |
| `motion.ts CLIP [--keep N]` | Makes a clip's takes with Kimodo from its entry in `made.json`, measures each on the mannequin (`gait_check.py`), and keeps one as its take (below) |
| `clip_check.py` | Checks every clip is one our skinning can follow: no roll of the upper arm or thigh, no step too big after the twist step |
| `twist.py` | The twist step: a limb's roll moved to the hand and foot and halved on the twist bones, baked into each clip as it is sampled for a body |
| `deform_check.py` | Rings of skin along every limb through the clips: how much each squeezes, flattens and turns |
| `rig_check.py` | Checks a rig against our skeleton and the mannequin, whose scores are `art/characters/rig-baseline.json` |
| `rig_render.py` | A sheet of the body through our animations |
| `export_character.py` | The body as the game loads it: skeleton, sockets, our clips re-sampled on it, skin patches (`patches.py`) |
| `shell_garment.py` | Clothing cut from the base body, from a model of that body wearing it (`art/gear/dressed-<body>-<item>/`) |
| `fit_gear.py` | Rigid gear (a pack, a quiver) fitted to a body at its socket, from `art/gear/<item>/`; straps from the skin (`bands.py`) |
| `gear_check.py` | How much skin comes through each worn piece, through our clips; `--fix` records how far each part must move out to clear it (the clipping fixer, per morph) |
| `build_bodies.sh` | Rebuilds every character asset the game loads from `art/`: the base, its gear, every morph, the skins, and the game's files |
| `skin_piece.py` | What pieces made from the body's surface share: lining up a dressed model, the smooth cut, evening the edge, the bake of colour, normals and roughness, the turned-in rim (used by `shell_garment.py` and `bands.py`) |
| `gait_check.py` | How a body moves through the locomotion clips against the mannequin they were made on: arms inside the torso, hand height, elbow flare, trunk lean, shoulder lift and foot slide, one line per clip and body (`tools/blender/run.sh tools/characters/gait_check.py`) |
| `pace.py` | Each moving clip's own pace, the speed its planted feet move back, which the game plays it against so feet do not slide |
| `lift.py` | The hips lifted by body size where a clip meets the floor: the mannequin measured once (run it on its own), each body's lift baked into its clips, each morph's kept for the game to mix |
| `front_view.sh SHEET FRONT` | A turnaround sheet's front view, cut at the first clear gap after it, at 2048 square, as Tripo is given it to model |
| `make_morph.sh NAME "CHANGE"` or `NAME --design BODY.glb` | A morph of the base body end to end: design (free), model (Studio credits), registration, export, gear carried and checked; skips what `art/` already keeps |
| `morph.py` | Registers the base onto a design's model: pose by the base's skeleton, shape by a pull smoothed over distance, back to the T-pose; the morph |
| `carry_gear.py` | A worn look made on the base carried onto a morph: skinned parts follow the skin, standing-off parts and looks hung on sockets move as one to keep their clearance (a hung look's shift per morph in its extras, for the game), and the clipping fixer's pushes are baked in |
| `paint_skin.sh NAME FRONT` | A template's own skin, painted by Tripo on the base's mesh in its shape (`paint_skin.py`), baked onto the base's layout |
| `compare.py` | Bodies side by side, dressed with `a.glb+b.glb` (worn or hung as in the game), at rest or held at a moment of a clip (`--pose`), or close on one bone from four sides (`--focus`) |
| `sockets.py` | Where a body file's sockets and main joints stand, at rest or partway through a clip (`--pose`), as `SOCKETS` lines: what the sandbox workbench's `--joints` is checked against |
| `fit_held.py` | The held look of each tool, from the models in `assets/models/tools`: free ones, and ours made by [tools/props](../props/README.md) |

`look.py` holds what the scripts share: paths, our palette and light, contact sheets and close-ups, and `skin_as_game`, which skins a loaded body as the game does (weight-blended, from `assets/models/character/skinning.json`).

Everything the game loads is rebuilt from `art/` by one command, free (the playbook's section 7):

```bash
tools/characters/build_bodies.sh
```

It writes `assets/models/character/base.glb` (the base body with every morph), `assets/models/character/skins/`, and `assets/models/gear/<item>.glb` (each worn look, made once, with the same morphs). Everything between, such as the base's own export `out/base-male-game.glb` and the gear as made on it `out/gear/base-male/`, stays in `tools/characters/out/`.

## Made clips

Where Quaternius's clips read as a cartoon on our bodies, the walk and jog first (research/2026-10-14-animation.md), we make our own with NVIDIA's [Kimodo](https://github.com/nv-tlabs/kimodo), a motion model trained on 700 hours of capture, run on this machine. Its weights are under NVIDIA's Open Model License, which allows commercial use and claims no ownership of what it makes. Each clip's prompt, speed (0 to stand), length and seed are in `art/animation/made.json`, with `"still": true` for one that stays where it stands, such as the idle, which loops between its two most alike frames rather than a stride; its kept take, a BVH on Kimodo's skeleton, in `art/animation/made/`. `made_clips.py` puts them into the game's clips as part of `build_bodies.sh`; no one needs Kimodo to build.

To make one, Kimodo is set up once, outside the repository, in `~/.cache/terra-kimodo` (or `KIMODO_HOME`): a Python 3.12 environment with PyTorch for CUDA and Kimodo installed from its repository (`uv venv --python 3.12 venv`, `uv pip install torch --index-url https://download.pytorch.org/whl/cu128`, `uv pip install -e kimodo`), and its text encoder in `text_encoders/`: Meta's Llama 3 8B Instruct at `meta-llama/Meta-Llama-3-8B-Instruct` (Meta's gated copy, or NousResearch's, which its licence lets be shared), and McGill's two LLM2Vec adapters at `McGill-NLP/<name>` with their `base_model_name_or_path` pointed at that folder. The encoder runs on the processor, about 16 GB of memory; the motion model takes about 3 GB of the graphics card, so ask the other sessions for the card first.

```bash
node tools/characters/motion.ts Sprint_Loop            # three takes, each measured
node tools/characters/motion.ts Sprint_Loop --keep 1   # keep take 1
tools/characters/build_bodies.sh                       # into the game
```

