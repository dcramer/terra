# Characters

The asset pipeline for bodies and what they wear and hold (D115, D128, D316): Blender scripts that turn the source art in [art/](../../art/README.md) into the files the game loads in [assets/](../../assets). The step by step, with what to do when a check fails, is the `character` skill (`.agents/skills/character/SKILL.md`); how the figure goes from clip to clip is the `anim` skill's.

Run everything from the repository root. Blender scripts run headless through [tools/blender/run.sh](../blender/README.md); paths given to them are relative to the root. What a script writes and we do not keep goes to `tools/characters/out/`, which is ignored.

- **Where it stands**: `cargo xtask characters` (`status.ts`, D625), in 40 lines from recorded results, running no check; each check records its result with `record.ts`.
- **Every script**, its arguments, what it reads and writes and what it says: [scripts.md](scripts.md), a row a script. Read the row of the script you run, `grep -n "^| \`motion.ts" tools/characters/scripts.md`, never the whole file.
- **The clips we make** (Kimodo, Uthana, keyed and derived clips, `art/animation/made.json`): [Made clips](#made-clips).

## Made clips

How Kimodo is set up and how each kind of made clip is made: [made-clips.md](made-clips.md), a section a kind. Read the section of the kind you make (`grep -n "^## " tools/characters/made-clips.md` lists them), never the whole file.

## Everything into the game

Everything the game loads is rebuilt from `art/` by one command, free (the character skill's [rebuilding](../../.agents/skills/character/references/rebuilding.md)):

```bash
tools/characters/build_bodies.sh
```

It writes the body files in `assets/models/character/`, each template's skin in `skins/`, and `assets/models/gear/<item>.glb` (each worn look, made once, with the same morphs); its header lists them. Everything between stays in `tools/characters/out/`. A line a step (built, skipped or failed); it ends with `BUILD pass` or `BUILD fail` and every failure (`build_verdict.ts`). `--plan` says what would run.

## The scripts, by stage

A script's full entry is its row in [scripts.md](scripts.md).

| Stage | Scripts |
|---|---|
| Status | `status.ts` (`cargo xtask characters`), `record.ts` (a check's result for the status) |
| Quaternius's parts (D599) | `vendor.ts` (list and vendor a pack's parts), `vendored.ts` (the guard on them), `theirs.py` (refuses a script that would reshape their mesh), `intact.ts` (the intact-parts check) |
| A body by Tripo | `make_character.sh` (on Studio credits), `finish_character.sh` (the free part: convert, check, render, export), `front_view.sh` (a sheet's front view), `convert_rig.py` (Tripo's rig onto ours), `stretch.py` and `proportions.py` (D593), `rig_check.py`, `deform_check.py`, `rig_render.py` (a sheet of the body through our clips), `export_character.py` (the body as the game loads it), `patches.py` (skin patches), `lift.py` (the hips lifted by body size), `sockets.py`, `compare.py`, `look.py` (what the scripts share: paths, palette, light, sheets, `skin_as_game`) |
| Morphs, skins, creatures | `make_morph.sh`, `morph.py`, `carry_gear.py` (a look carried onto a morph), `paint_skin.sh` and `paint_skin.py`, `tint.ts`, `eyes.sh` (the creator's eye colours, D111), `marks.ts` (a mark's glyph for the arm, D590), `wake.py` (a waking creature's face), `crown.py` (a tree creature's crown) |
| The wardrobe (D600, D611) | `export_pieces.py` (a wardrobe body's pieces), `wardrobe.ts` (the wardrobe check; `--build` adds an outfit just vendored), `draft_rows.ts` (its catalogue rows drafted), `pairings.ts` (the pairing matrix), `neck.ts` (our bare torso against their head), `layers.ts` (one piece over another through the clips), `cut_slots.py`, `slots.ts`, `slot_stills.py` (D600's trial), `skin_piece.py`, `bands.py` (straps over pieces) |
| Gear, worn | `gear_list.ts`, `shell_garment.py`, `hung_garment.py`, `fit_gear.py`, `gear_check.py` (skin through each piece) |
| Gear, held (D546, D552) | `detect.ts` (the detector), `fit.ts` and `fit_cache.ts` (the fitter), `holds.ts` and `hold_pose.ts` (the hold report: `node tools/characters/holds.ts`, how every held look is held, in numbers, before any still), `carry_clear.ts` and `rust_const.ts` (the carry's clearance solver, the game's twin, its numbers read from the game's source), `solve.ts`, `tune.ts` and `specs/` (the hold solver: `node tools/characters/solve.ts tools/characters/specs/basket.json` searches a carry pose's and the give's numbers for the pose nearest the authored one that clears the hold report's bars by a target margin, never a sweep by hand; it says how far each joint turned, and a joint past 15 degrees wants a still before applying), `fit_held.py`, `haft.py`, `held.ts`, `nock.ts`, `attach_contract.ts` (the attachment contract's check), `grip.ts`, `carry_arm.ts`, `shield_arm.ts`, `shield_place.ts`, `shield_path.ts`, `surface.ts`, `skin.ts`, `arms.ts` |
| Clips | `clips.py` (cut from the libraries), `made_clips.py`, `motion.ts` (Kimodo takes), `keyed_walk.py`, `key.ts` (a clip from key poses, D588), `retarget.py` (D542), `aim.py`, `hold.py`, `twist.py`, `clip_check.py`, `gait_check.py`, `stance.ts`, `rest.ts` (empty hands hanging), `pace.py`, `stride.ts`, `markers.ts` (foot locks and distance curves), `clear.ts` (swing clearance), `mirror.ts`, `additive.ts` (the carried arm's swing), `blend.ts`, `glb_clips.ts`, `glb_diff.ts`, `glb.ts`, `clip_sheet.ts` and `clip_sheet.py`, `sheet_stills.py`, `audit.ts` (the render audit) |
| Fight clips (D447, D450) | `moves.ts` (the move check), `phases.ts`, `mesh.ts`, `blow.ts`, `root_drive.ts`, `clip_spec.ts`, `clip.ts` (the clip step), `kimodo_keys.py`, `keys.ts`, `wrist.ts`, `wrist_limit.ts`, `clash.ts` |
| The build | `build_bodies.sh`, `build.ts` (stamps, as make and ninja), `build_steps.ts` (the steps as data, from `bodies.json`), `build_item.sh` (one worn item alone), `build_verdict.ts` |
| Blender's tests | `test_checks.py` (the faults rig_check and gear_check must catch, planted, D632), `test_stretch.py`, `test_bands.py`, `test_hold.py`, `test_twist.py` (run as `tools/blender/run.sh tools/characters/test_<name>.py`); the TypeScript tests are each script's `.test.ts` |
