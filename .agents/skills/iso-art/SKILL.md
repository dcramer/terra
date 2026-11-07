---
name: iso-art
description: Make art and biomes for Terra's 2.5D isometric experiment (iso-art, an action RPG's camera on spike/iso): a hero, creature, NPC, prop or scenery from a description to a checked GLB with its clips by tools/iso-art over Tripo's own rig and presets (concept, model, rig, animate, export in place with cuts and loops, check, budgets, the ledger, the cost per kind); a biome scene at the ARPG camera from the repository's scanned materials, lit against measured reference frames, with effects, ambient and living nature, and its film; and judging either toward realism against each biome's drone photographs and the most realistic top-down game frames by an adversarial judge's rubric, measured stats and a blind pairwise judge (D634). Use for any task that makes, places, lights, films or judges iso art or an iso biome.
---

# Iso art

The isometric experiment's art pipeline, as it proved out from 2026-11-05 (experiment iso-art). Its choices are D628 (Decided, the agent's); the camera is D629 and the budgets against main Terra's D630, both Open for the developers. Background: [research/2026-11-05-iso-arpg-art-pipeline.md](../../../research/2026-11-05-iso-arpg-art-pipeline.md).

## Where things are

- **The tool**: [tools/iso-art](../../../tools/iso-art/README.md), `node tools/iso-art/iso-art.ts`, TypeScript, no dependencies; `node --test tools/iso-art/iso-art.test.ts`. It depends on no spike (D133).
- **The assets**: `assets/models/iso/<name>.glb`, each with its ledger `art/iso/NAME/asset.json`; `node tools/iso-art/iso-art.ts status` lists them.
- **Judging** (D634): outside the repository in `~/.cache/terra-review/iso-kit/`: `judge.md` (the adversarial rubric toward realism and the blind pair), `refs/<biome>/` (photographs and game frames, never committed, with `refs.md` and judges' copies in `960/`), `refs/stats.json`; the measure is in the repository, `node tools/iso-art/stats.ts`. What a fresh agent needs from them is in [judging.md](references/judging.md).
- **The scenes**: the experiment's code lives in a spike, `spike/iso/` (D133: throwaway, deleted once its research note is written; nothing lasting may depend on it). A biome is a data file there, `spike/iso/biomes/<name>.ron`, drawn by the shared kit in `spike/iso/src/kit/` and run by `--biome NAME` (D634, [kit.md](references/kit.md)); the ruin at night's modules are still `spike/iso/src/ruin/`.
- **The loop**: `node spike/iso/loop.ts` shoots, measures and prepares the judges of every biome and keeps a round by the ratchet ([kit.md](references/kit.md)); its rounds, kept stills, blind pairs and `history.json` are in `~/.cache/terra-review/iso-kit/`, beside `refs/` and `judge.md`.
- **Handoffs**: `~/.cache/terra-handoff/iso-art-j<n>.md`, the coordinator's `iso-art-lead.md`. Read only the last one and what it points to.

## The steps (each a page beside this one)

1. [Making a good asset](references/asset.md): concept, model, rig, presets, export (in place, cuts, loops), check, budgets, the ledger, the cost per kind.
2. [Making a good biome scene](references/scene.md): camera, ground and walls from the scanned materials, light against the refs' numbers, the fight, effects, ambient and living nature, the film.
3. [Judging](references/judging.md): the bar, each biome's realistic references and how they were collected, the measures, the rubric for a scene and an asset, the blind pair, the adversarial loop and its ratchet, briefing a judge.
4. [Traps](references/traps.md): what cost jobs before. Read it before your first shot.
5. [The biome kit and its loop](references/kit.md): a biome file's fields, the kit's modules, adding a biome, `loop.ts`'s commands.

## The loop a job follows (biomes and the kit, D634)

1. **Read** the last handoff's next steps and only the files it names; `node spike/iso/loop.ts history` for where each biome stands.
2. **Shoot** the baseline: `node spike/iso/loop.ts shoot` (a line a biome: its measures beside its refs' ranges, and whether it matches its kept still). Start it in the background; it waits on the worker.
3. **Look** at the still the judge's first gap names (960 px, a 1:1 crop where it matters) and say the number it should move: a measure against the refs, a judge's gap.
4. **Change one thing**, in the kit (a layer, mask, scatter, water, effect, light preset) or a biome's file: a kit change reaches every biome, so every biome is shot after it.
5. **Shoot** again with `--dirty` (or after landing it): a biome whose still did not move ties without a judge; each that moved gets two briefs in `rounds/<round>/briefs/`.
6. **Judge**: the lead runs each brief as a subagent, the blind pair and the adversarial judge (judging.md; a helper reaches its bound before judging), and records each answer: `record <round> <biome> pair <A|B|same> <margin>`, `record <round> <biome> score <n> "<gaps>"`. A slight answer either way prepares a second pair, order swapped: run it too (the same letter winning both is position bias, a tie).
7. **Verdict**: `node spike/iso/loop.ts verdict <round>`. KEEP: land the change on main naming iso-kit, with each biome's line and score before and after in the message; the round's stills are kept. REVERT: undo it (the losers named). A round whose changed biomes only tied is refused: undo the change and record it with `verdict <round> --revert`, so its stills never become the kept ones. A change to one biome alone is kept only on its clear or strong win; a slight win that only fixes a fault (a creature hidden in shade) may stay, said so. The quality bar is the judge's score (approve at 7), never the pair alone; two rounds without a gain stop the line of work.
8. **Hand off**: commits, each biome's line and score, what failed and why, next steps in order, a `FOR REVIEW` line for each still or film for the developer (the review-desk skill).

## The loop for an asset

1. `node tools/iso-art/iso-art.ts status`; `node tools/tripo/tripo.ts balance` before anything that spends. Take one gap: the judge's first, else the handoff's first, and its number (a check, a clip's read, a measure).
2. Make the change; look at it at the game camera in a biome (scene.md's still), measure it with stats.ts.
3. Land each checked step on main as its own commit naming iso-art, with the numbers before and after and the still's path: an asset with its ledger, a tool fix with its test (`node --test`). The blind pair and the scorer at a round's end, run by the lead; keep a change only on a clear or strong win, revert one that loses even slightly (the marsh's mist, twice).

## Rules

- **Tripo's rig and presets as they come** (D628): one mesh a character with its gear, never converted onto our skeleton (the character skill's traps are why). What a preset cannot do, the game adds: hit flash, flinch, hit-stop, rim light, an effect.
- **Every asset is judged at the game camera** in the scene's light, at the frame height it plays at, beside a ref at the same on-screen size. Up close is not the test.
- **Numbers before taste**: every still gets its stats line against its biome's refs that show the same thing (the night ones for a night still); tune light to them, not by eye.
- **A biome fills the frame**: dressed ground, framing props at the edges, depth front to back; no margin of bare ground. Materials are the repository's scans at their true size (`assets/materials/`).
- **Every effect has a light**, a bright core over bloom, and fades. A fixed pool of entities, nothing spawned at run time.
- **Living nature shows in the film**: wind, cloth, motes, creatures moving. A biome is done with a still and a 12 to 20 s film on the desk.
- **No local builds where embargoed** (`TERRA_NO_LOCAL_BUILDS=1`): build, test and shoot in one `cargo xtask remote-render` job; node, the docs check and Tripo are fine anywhere.
- **Credits are the team's**: a spend is named in the brief or the handoff; adopt an existing project first; never spend past the balance to try another model.
- **Look little**: about 120k tokens is three shots and a judge; one look an effect, not a tweak.

## What is still unknown

- Model and rig minutes for a new asset (only adopted projects were timed); whether `--standard` models pass the asset judge.
- Where it stands (2026-11-06): each biome near 5 of 10, none at the approval bar of 7; scores, costs and the verdict in [the research note](../../../research/2026-11-05-iso-arpg-art-pipeline.md#is-it-good-enough-at-what-cost-2026-11-06-after-jobs-j1-to-j49). The gaps every judge names: surface density and saturation (fine detail 0.09 to 0.12 against 0.13 to 0.23), water (forest confetti glints, marsh one milky blue), effects without a hot core.
- The budgets' 1M-triangle frame share: no bench of any biome yet (D185); the ruin's scatter (j46) added about 270k triangles of skulls and bone heaps, undecimated. The rigged ogre, tree-monster and treants are decimated (`decimate`, asset.md), the bone-pile, torch and skull not yet.
- Model and rig minutes for a new asset, and whether a `--standard` model passes the asset judge: nothing new was affordable.
- Fog over night water: two sheet designs lost (traps.md); a lit volume in a lamp's cone is untried.
