# Judging

Quality is judged by an adversarial loop toward realism (D634): a harsh judge scores each biome's still against that biome's realistic references with its gaps; fix and judge again until it approves (7) or two rounds bring no gain. A blind pairwise judge says only whether a change won. The working copies live outside the repository in `~/.cache/terra-review/iso-kit/`: `judge.md` (the rubric and the pair brief), `refs/<biome>/` (the 1920 px refs, their judges' copies in `960/`, `refs.md` with each one's source, licence, author, what it shows and the ranges), `refs/stats.json`; the measure is `tools/iso-art/stats.ts`. If they are missing, rebuild them from this page.

## The bar

The developer: "you should use adversary loops for it when you are trying to assess quality. we don't mean to compare to baldurs gate anymore, not for graphics, the name of the game is high quality, realistic topdown game", and "realistic meaning leaning into realism". Believable materials, light, scale, weathering and detail; not a photo match and not stylised fantasy. The judge rewards each step toward realism, faults anything cartoonish, toy-like or plasticky, does not demand photorealism, and still requires that figures, hits and paths read from the camera.

## The references (never commit them)

Per biome, two to four drone or aerial photographs at or near the camera's angle (looking down 50-55 degrees from 15-20 m, a person 0.10-0.15 of the frame), the bar for materials, scale and weathering; beside them the most realistic top-down game frames found, the bar for a night key, effects and figures at this camera. Collected 2026-11-06 (iso-kit j2, j3):

| Biome | Photos | Game frames | p50 | Fine | Sat |
|---|---|---|---|---|---|
| neon-street | 3 Pexels (steep night streets, a wet intersection), 1 Commons (Shibuya in rain, shallow) | The Ascent x2 | 0.008-0.085 | 0.116-0.246 | 0.12-0.65 |
| forest | 3 Pexels (a pond with a footbridge, a park pond, trees into water) | Titan Quest II | 0.031-0.101 | 0.149-0.244 | 0.26-0.69 |
| marsh | 4 Pexels (a reed channel with a heron, a boardwalk over dark channels at dusk, dark water, a boat in reeds) | Gord (night swamp, cropped off its UI), Titan Quest II | 0.021-0.145 | 0.084-0.237 | 0.23-0.44 |
| ruin | 3 Pexels (a stone court at 50 deg, roofless walls from above, a floodlit theatre at night) | Diablo IV (night court), Titan Quest II | 0.005-0.091 | 0.165-0.211 | 0.29-0.71 |
| wasteland | 4 Pexels (a ruined gas station, a derelict lot, a junk lot, overgrown concrete ruins) | Wasteland 3 (half realistic), Diablo IV, Titan Quest II | 0.042-0.233 | 0.103-0.146 | 0.21-0.54 |

Each `refs.md` says how to read its biome's refs (the marsh and ruin photos are day or dusk: materials and scale from them, the night key from the game frames) and what is still missing (a marsh or unlit ruin at night from the camera's height; rusted cars at the angle). To add one: `WebFetch https://www.pexels.com/search/<query>/?orientation=landscape` asking for each photo's id, photographer slug and alt text, then curl `https://images.pexels.com/photos/<id>/pexels-photo-<id>.jpeg?auto=compress&w=640` (candidates, viewed at 640 px) or `w=1920` (kept); Steam frames from `store.steampowered.com/api/appdetails?appids=<id>`. curl to pexels.com itself is blocked; Commons and Openverse searches return mostly ground-level or 100 m+ shots. The scripts are in `~/.cache/terra-handoff/iso-kit-scripts/`.

D628's stylised ARPG store frames (Diablo IV, Path of Exile 2, Last Epoch, V Rising, Hades II, in `~/.cache/terra-review/iso-art/refs/`) were the bar until D634 and are not any more.

## The measures (stats.ts)

Each image resized to 960x540 (Lanczos), so any size compares at one. `L` is linear-light luminance: sRGB decoded (v/12.92 under 0.04045, else ((v+0.055)/1.055)^2.4), Rec.709 weights 0.2126, 0.7152, 0.0722.
- `luma_p5`, `luma_p50`, `luma_p95`, `luma_mean`: percentiles of L, 0 to 1. p5 is the blacks (the sky fill), p95 the pools.
- `fine_detail`: mean |L minus its 3x3 box blur (edges padded)| over mean L: texture sharpness and small dressing.
- `saturation`: mean (max-min)/max of the sRGB channels over pixels with max over 0.02.
- `dark_share` L under 0.05, `mid_share` 0.05 to 0.35, `bright_share` over 0.35.

`node tools/iso-art/stats.ts <out.json> <still>...` adds each still to the JSON and prints every row (`name=path` copies the still beside the JSON first). Node, no dependencies: a PNG decoded with node:zlib, a JPEG through ffmpeg, the downscale Pillow's own Lanczos, so a PNG's numbers are the old `stats.py`'s to the fourth place and a JPEG's within 0.0005. Put every still's line in the handoff against its biome's ranges, comparing it with the refs that show the same thing (a night still with the night ones).

## The rubric (judge.md)

**(a) The scene.** Inputs: the 1920x1080 still at the game camera (a frame strip or film when atmosphere or effects are judged), its stats line. A harsh judge, briefed to find what makes the frame read unreal, scores 0 to 10: 1 materials (each reads as itself; roughness varies; no plastic sheen or tiling); 2 light (a key for the hour, pools with falloff, true blacks, contact shadows, grade in the refs' range); 3 scale (true sizes, a person 1.7-1.8 m; nothing a miniature or toy); 4 weathering and detail (grime, moss, debris, wear where things meet; dressed ground; fine detail in range); 5 atmosphere and life (haze, weather, motes, moving water and foliage, never hiding the fight); 6 effects (a hot core, light cast, smoke, a fade; no cartoon shapes or sprite edges); 7 readability from the camera (hero, enemies, hits and paths at a glance). Anchors:
- **9**: taken for a shipped realistic top-down game's frame at its best beside the refs, its materials holding up next to the photos; nothing toy-like.
- **7**: a shipped realistic top-down game with one or two tells (a material that reads as a texture, a clean corner, a sprite-like effect); nothing cartoonish or plasticky. **Approve at 7.**
- **5**: right camera, readable, light that works, plainly CG beside the refs: uniform or clean materials, scale off in places, thin weathering, lifted blacks or flat light, blob effects.
- **3**: a tech demo or a toy: plastic sheen, tiling, wrong scale, clean seams at the ground, flat light, programmer-art effects.

Between is allowed, naming the nearest anchor; judges spread ±0.5, so a change under 0.5 is noise. Overall is the nearest anchor, not the mean, at most 1 above the lowest of materials, light, scale and readability. Output: the seven, overall, the three biggest gaps (what, where in the frame, why it reads unreal against which ref, the likely cause someone can act on), approve yes or no.

**(b) The blind pair.** Two stills of one biome as `a.png` and `b.png` in random order: which leans further toward the refs' realism, A, B or same; slight, clear or strong; the one thing that decided it. Nothing else.

**(c) One asset** (D628's, its bar now the biome's refs): the asset at the game camera in the scene's light at its frame height, beside a figure cropped from the biome's game frames at the same on-screen size, and a strip of 6 to 8 frames of each clip. Score silhouette, material read, texture detail, realism (no plastic, melted details, wrong-handed straps, mush hands, seams) and animation read; the worse of silhouette and realism caps the overall.

## The loop (D634)

1. Render each biome's still on the worker (D319) and measure it against its biome's ranges.
2. The scene judge scores it; take its first gap, fix it, render, judge again. Stop at approval (7) or when two rounds bring no gain.
3. The blind pair judges a change against the biome's last kept still: it says whether the change won, never how good the frame is.
4. The ratchet over all biomes: a change to the kit is kept only if no biome loses its pair (a clear or strong loss, or a slight loss twice with the order swapped); a biome whose still did not change ties without a judge. Each round's scores, pairs and stats go in the history.

## Briefing a judge

A `general-purpose` subagent, one at a time, given the paste-in brief at the end of `judge.md`: it reads only `judge.md`, the biome's `refs.md` and `refs/<biome>/960/` images, and the stills given, about 20k of its own tokens (the old brief that read everything cost 87k). It judges only: no edits, renders or GPU; it writes its verdict to `<round>/verdict.md`. Keep which pair file is which outside its brief. The adversarial review gate before a biome is called done (the environment skill) is this judge at 7.

## Scores so far

Against D628's store frames each biome stood near 5 of 10 ([research note](../../../../research/2026-11-05-iso-arpg-art-pipeline.md)); those scores do not carry over. Every biome is re-baselined against its realistic refs (D634).
