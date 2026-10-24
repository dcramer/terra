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
| `phases.ts FILE.glb CLIP... [--tip 0.83]` | Measures a blow's active frames: the sword's tip, followed through the skeleton from `socket.hand.R`, while it is ahead of the hips and fast; the moves data takes its spans (D405) |
| `moves.ts [--set FILE.ron] [--template male,...] [--fps 15]` | The move check (D447): every move of a set, and what a drawn fighter shows besides, on every body template: how deep hands, arms and held looks go into the body, where a blow's tip crosses (bearing and reach) and which way it sweeps, a raised shield's facing, and how far a move's last pose is from the next one's and from the guard's; ✗ over a bar, exit 1. `glb.ts` is the reader it and `phases.ts` share |
| `motion.ts CLIP [--keep N]` | Makes a clip's takes with Kimodo from its entry in `made.json`, measures each on the mannequin (`gait_check.py`), and keeps one as its take (below) |
| `clip_spec.ts` | A move's clip spec (D450), an entry of a set's list with its guard (`"guard"`), right- and left-hand keys (`"hand"`, `"left"`) and blow (`"blow"`: its move, sweep and contact bearing): reading it, the sweep and contact check `moves.ts --spec` fails a blow on, the take kept, and phases written into a move set. The one-command clip step that will use it is not built yet |
| `kimodo_keys.py GUARD.npz KEYS.json OUT.json` | Run in Kimodo's own Python: a spec's Kimodo constraints, the guard pose as a full-body key at the first and last frames and each hand key a pose solved by two-bone IK from the guard; a `KEY` line a key says where its fist and blade ended up |
| `clip_check.py` | Checks every clip is one our skinning can follow: no roll of the upper arm or thigh, no step too big after the twist step |
| `twist.py` | The twist step: a limb's roll moved to the hand and foot and halved on the twist bones, baked into each clip as it is sampled for a body |
| `deform_check.py` | Rings of skin along every limb through the clips: how much each squeezes, flattens and turns |
| `rig_check.py` | Checks a rig against our skeleton and the mannequin, whose scores are `art/characters/rig-baseline.json` |
| `rig_render.py` | A sheet of the body through our animations |
| `export_character.py` | The body as the game loads it: skeleton, sockets, our clips re-sampled on it, skin patches (`patches.py`) |
| `shell_garment.py` | Clothing cut from the base body, from a model of that body wearing it (`art/gear/dressed-<body>-<item>/`) |
| `hung_garment.py` | What hangs from the body rather than lying on it, a long skirt or an apron: rings round the body and what it is worn over, the pelvis's weight falling to the thighs toward the hem, baked from the dressed model |
| `fit_gear.py` | Rigid gear (a pack, a quiver) fitted to a body at its socket, from `art/gear/<item>/`; straps from the skin (`bands.py`) |
| `gear_check.py` | How much skin comes through each worn piece, through our clips; `--fix` records how far each part must move out to clear it (the clipping fixer, per morph) |
| `build_bodies.sh` | Rebuilds every character asset the game loads from `art/`: the base, its gear, every morph, the skins, and the game's files |
| `skin_piece.py` | What pieces made from the body's surface share: lining up a dressed model, the smooth cut, evening the edge, the bake of colour, normals and roughness, the turned-in rim (used by `shell_garment.py` and `bands.py`) |
| `gait_check.py` | How a body moves through the locomotion clips against the mannequin they were made on: arms inside the torso, hand height, elbow flare, trunk lean, shoulder lift and foot slide, one line per clip and body (`tools/blender/run.sh tools/characters/gait_check.py`) |
| `aim.py` | The clips marked `"aim"` in `made.json` carried onto a body by where the mannequin's limbs point rather than by its bones' rotations, so the body's own rest stance (legs splayed, feet turned out) does not carry into them, as Unreal's IK Retargeter retargets from a reference pose |
| `pace.py` | Each moving clip's own pace, the speed its planted feet move back, which the game plays it against so feet do not slide |
| `lift.py` | The hips lifted by body size where a clip meets the floor: the mannequin measured once (run it on its own), each body's lift baked into its clips, each morph's kept for the game to mix |
| `front_view.sh SHEET FRONT` | A turnaround sheet's front view, cut at the first clear gap after it, at 2048 square, as Tripo is given it to model |
| `make_morph.sh NAME "CHANGE"` or `NAME --design BODY.glb` | A morph of the base body end to end: design (free), model (Studio credits), registration, export, gear carried and checked; skips what `art/` already keeps |
| `morph.py` | Registers the base onto a design's model: pose by the base's skeleton, shape by a pull smoothed over distance, back to the T-pose; the morph |
| `carry_gear.py` | A worn look made on the base carried onto a morph: skinned parts follow the skin, standing-off parts and looks hung on sockets move as one to keep their clearance (a hung look's shift per morph in its extras, for the game), and the clipping fixer's pushes are baked in |
| `paint_skin.sh NAME FRONT` | A template's own skin, painted by Tripo on the base's mesh in its shape (`paint_skin.py`), baked onto the base's layout |
| `compare.py` | Bodies side by side, dressed with `a.glb+b.glb` (worn or hung as in the game), at rest or held at a moment of a clip (`--pose`), or close on one bone from four sides (`--focus`) |
| `sockets.py` | Where a body file's sockets and main joints stand, at rest or partway through a clip (`--pose`), as `SOCKETS` lines: what the sandbox workbench's `--joints` is checked against |
| `fit_held.py` | The held look of each tool, from the models in `assets/models/tools`: free ones, and ours made by [tools/props](../props/README.md); and of what hangs from the hand by a handle (a market basket), from its prop |

`look.py` holds what the scripts share: paths, our palette and light, contact sheets and close-ups, and `skin_as_game`, which skins a loaded body as the game does (weight-blended, from `assets/models/character/skinning.json`).

Everything the game loads is rebuilt from `art/` by one command, free (the playbook's section 7):

```bash
tools/characters/build_bodies.sh
```

It writes `assets/models/character/base.glb` (the base body with every morph), `assets/models/character/skins/`, and `assets/models/gear/<item>.glb` (each worn look, made once, with the same morphs). Everything between, such as the base's own export `out/base-male-game.glb` and the gear as made on it `out/gear/base-male/`, stays in `tools/characters/out/`.

## Made clips

Where Quaternius's clips read as a cartoon on our bodies, the walk and jog first (research/2026-10-14-animation.md), we make our own with NVIDIA's [Kimodo](https://github.com/nv-tlabs/kimodo), a motion model trained on 700 hours of capture, run on this machine. Its weights are under NVIDIA's Open Model License, which allows commercial use and claims no ownership of what it makes. Each clip's prompt, speed (0 to stand), length, seed and number of takes (`samples`, 3 unless it says) are in `art/animation/made.json`, with `"still": true` for one that stays where it stands, such as the idle, which loops between its two most alike frames rather than a stride; its kept take, a BVH on Kimodo's skeleton, in `art/animation/made/`. `made_clips.py` puts them into the game's clips as part of `build_bodies.sh`; no one needs Kimodo to build.

What people do where they stand is made the same way (D347): a clip of work or talk is `"still"`, with `"loop"` (seconds, least and most) for how long its loop may be, and `"busy": true` for one that works (a hammer, an axe), looped in its busiest stretch, since a take that stops and stands is stillest and would otherwise loop where it does nothing. A still clip plays at the take's own pace, Kimodo's 30 frames a second keyed on the scene's 24; a stride keeps a key a frame, the game setting its pace from its feet. Kimodo follows a short plain prompt, as its own examples are ("A person repeatedly hammers something..."), better than a long one naming every limb. A clip in `clips.txt` that no library has must be one of these; `clips.py` leaves it to `made_clips.py`, and with no library here the build keeps the library's clips already in the committed mannequin.

A stride's entry may also carry what the take alone gets wrong (D360). `"step"` (metres) sets how far apart the feet land, and has the stride carried onto each body by where the mannequin's limbs point (`aim.py`). `"arms"` lays a runner's arms over the take, its legs and spine untouched (`made_clips.py`'s `swing_arms`): the upper arms' swing mapped onto `swing` (degrees forward and back of the trunk), both timed by the left arm; the elbow bent from `elbow[0]` behind to `elbow[1]` in front; the forearm across the body at most `across` degrees; the wrist within `wrist` of the forearm's line; and the hand rolled within `palm` of thumb-side-up. `"fist"` closes the fingers that share of the way into the library's clenched fist, since Kimodo leaves them straight.

`"way"` (degrees from where the body faces, 90 to its right, -90 to its left) lays a stride's path that way with Kimodo's heading held ahead, so a strafe steps sideways facing forward; `made_clips.py` turns it by its travel less `way` and prints the way the take really went (`WAY`). `"once": true` makes a one-shot (a blow, a block, a dodge): its take is held to no path, kept from start to end or over `"span"` (seconds in the take), played at the take's pace, in place, and not closed into a loop; its `ONCE` line says when each hand is raised highest and moves fastest, when the faster hand's blow lands (it first slows below 30% of its fastest), and how far the body travels, which a scene moves it by. A set of clips for something other than the game's bodies (a creature, a demo) has its own list, given to both tools as `--list FILE`: `art/animation/sets/fight.json`, its kept takes in `art/animation/sets/fight/`, made onto the mannequin into a copy named for it (`made_clips.py tools/characters/out/fight-mannequin.glb --list=art/animation/sets/fight.json`) that carries the game's clips and the set's, for `export_character.py --clips=`, so the body plays both; the game's list and mannequin are left alone.

To make one, Kimodo is set up once, outside the repository, in `~/.cache/terra-kimodo` (or `KIMODO_HOME`): a Python 3.12 environment with PyTorch for CUDA and Kimodo installed from its repository (`uv venv --python 3.12 venv`, `uv pip install torch --index-url https://download.pytorch.org/whl/cu128`, `uv pip install -e kimodo`), and its text encoder in `text_encoders/`: Meta's Llama 3 8B Instruct at `meta-llama/Meta-Llama-3-8B-Instruct` (Meta's gated copy, or NousResearch's, which its licence lets be shared), and McGill's two LLM2Vec adapters at `McGill-NLP/<name>` with their `base_model_name_or_path` pointed at that folder. The encoder runs on the processor, about 16 GB of memory; the motion model takes about 3 GB of the graphics card, so ask the other sessions for the card first.

```bash
node tools/characters/motion.ts Sprint_Loop            # three takes, each measured
node tools/characters/motion.ts Sprint_Loop --keep 1   # keep take 1
tools/characters/build_bodies.sh                       # into the game
```

