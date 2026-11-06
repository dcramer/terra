---
name: iso-art
description: Make art and biomes for Terra's 2.5D isometric experiment (iso-art, an action RPG's camera on spike/iso): a hero, creature, NPC, prop or scenery from a description to a checked GLB with its clips by tools/iso-art over Tripo's own rig and presets (concept, model, rig, animate, export in place with cuts and loops, check, budgets, the ledger, the cost per kind); a biome scene at the ARPG camera from the repository's scanned materials, lit against measured reference frames, with effects, ambient and living nature, and its film; and judging either against shipped ARPGs' store frames by a rubric, measured stats and a blind pairwise judge. Use for any task that makes, places, lights, films or judges iso art or an iso biome.
---

# Iso art

The isometric experiment's art pipeline, as it proved out from 2026-11-05 (experiment iso-art). Its choices are D628 (Decided, the agent's); the camera is D629 and the budgets against main Terra's D630, both Open for the developers. Background: [research/2026-11-05-iso-arpg-art-pipeline.md](../../../research/2026-11-05-iso-arpg-art-pipeline.md).

## Where things are

- **The tool**: [tools/iso-art](../../../tools/iso-art/README.md), `node tools/iso-art/iso-art.ts`, TypeScript, no dependencies; `node --test tools/iso-art/iso-art.test.ts`. It depends on no spike (D133).
- **The assets**: `assets/models/iso/<name>.glb`, each with its ledger `art/iso/NAME/asset.json`; `node tools/iso-art/iso-art.ts status` lists them.
- **The scenes**: the experiment's code lives in a spike, `spike/iso/` (D133: throwaway, deleted once its research note is written; nothing lasting may depend on it). A biome is a `--scene` there; the ruin at night is `spike/iso/src/ruin/`.
- **Judging**: outside the repository in `~/.cache/terra-review/iso-art/`: `judge.md`, `refs/` (store frames never committed, `refs.md`, `stats.json`); the measure is in the repository, `node tools/iso-art/stats.ts`. What a fresh agent needs from them is in [judging.md](references/judging.md).
- **Handoffs**: `~/.cache/terra-handoff/iso-art-j<n>.md`, the coordinator's `iso-art-lead.md`. Read only the last one and what it points to.

## The steps (each a page beside this one)

1. [Making a good asset](references/asset.md): concept, model, rig, presets, export (in place, cuts, loops), check, budgets, the ledger, the cost per kind.
2. [Making a good biome scene](references/scene.md): camera, ground and walls from the scanned materials, light against the refs' numbers, the fight, effects, ambient and living nature, the film.
3. [Judging](references/judging.md): the references and how they were collected, the measures, the rubric for an asset and a scene, the blind pairwise step, briefing a judge.
4. [Traps](references/traps.md): what cost jobs before. Read it before your first shot.

## The loop a job follows

1. Read the last handoff's next steps and only the files it names. `node tools/iso-art/iso-art.ts status` for assets; `node tools/tripo/tripo.ts balance` before anything that spends.
2. Take one gap: the judge's first, else the handoff's first. Say the number it should move (a stats measure against the refs of the same kind, a check, a clip's read).
3. Make the change in its own module; one look a change: shoot the still on the render worker (scene.md), measure it with stats.ts, look at it at 960 px and a 1:1 crop where it matters.
4. Land each checked step on main as its own commit naming iso-art, with the numbers before and after and the still's path in the message: an asset with its ledger, a scene module, a tool fix with its test. Spike tests pass on the worker; `node --test` for the tool.
5. At a round's end, the blind pairwise judge, then the scorer (judging.md), run by the lead, not the helper (a helper reaches its bound before judging). Keep a change only on a clear or strong win; revert one that loses, even slightly (the marsh's mist, twice); a slight win that only fixes a fault (a creature hidden in shade) may stay, said so in the log. Two rounds without a gain stops the line of work.
6. Hand off: commits, each still's stats line, scores, what failed and why, next steps in order, a `FOR REVIEW` line for each still or film for the developer (the review-desk skill).

## Rules

- **Tripo's rig and presets as they come** (D628): one mesh a character with its gear, never converted onto our skeleton (the character skill's traps are why). What a preset cannot do, the game adds: hit flash, flinch, hit-stop, rim light, an effect.
- **Every asset is judged at the game camera** in the scene's light, at the frame height it plays at, beside a ref at the same on-screen size. Up close is not the test.
- **Numbers before taste**: every still gets its stats line against the refs of its kind (night, day, dungeon); tune light to them, not by eye.
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
