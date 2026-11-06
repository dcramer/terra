# Judging

Every round: measure the still, a harsh judge scores it on the rubric, and a blind pairwise judge says whether it beat the last kept one. The pairwise answer decides whether the change is kept. The working copies live outside the repository in `~/.cache/terra-review/iso-art/`: `judge.md` (the rubric below), `refs/` (the stills, `refs.md`, `stats.json`) and `stats.py`. If they are missing, rebuild them from this page.

## The references (never commit them)

Fourteen Steam store screenshots, 1920x1080, first-party, two or three a game, picked from all 108 store frames of the five games (kept in `refs/raw/`) for a fight at the closest game camera. The URL is `https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/` + the source below. Hero is the hero's head-to-foot height over the frame's, by eye on 960x540, ±20%.

| File | Source (after the prefix) | Kind | Hero | p5 | p50 | p95 | Fine | Sat |
|---|---|---|---|---|---|---|---|---|
| d4-dungeon-fight | 2344520/ee4b4e1ba9c8a07f40aff563c7c43fd04477d60c/ss_ee4b4e1ba9c8a07f40aff563c7c43fd04477d60c.1920x1080.jpg | night | 0.11 | 0.0007 | 0.0077 | 0.043 | 0.13 | 0.42 |
| d4-lightning | 2344520/7905f91c07b2cb828d0036389bc783300350fb74/ss_7905f91c07b2cb828d0036389bc783300350fb74.1920x1080.jpg | dungeon | 0.09 | 0.0006 | 0.0045 | 0.026 | 0.21 | 0.44 |
| d4-fire-breath | 2344520/73d3413a15dcde01592a1e8e3c998ec128ef9676/ss_73d3413a15dcde01592a1e8e3c998ec128ef9676.1920x1080.jpg | day, snow | 0.13 | 0.036 | 0.148 | 0.439 | 0.06 | 0.10 |
| poe2-nest-fight | 2694490/578654a7e1e88e6e186ffe1d3c055e9c1ceb0300/ss_578654a7e1e88e6e186ffe1d3c055e9c1ceb0300.1920x1080.jpg | skylit | 0.13 | 0.0034 | 0.023 | 0.265 | 0.28 | 0.37 |
| poe2-shock-nova | 2694490/2028c61a964a5e1f8a7ff3e6d8a6bee64153af1f/ss_2028c61a964a5e1f8a7ff3e6d8a6bee64153af1f.1920x1080.jpg | day | 0.16 | 0.0072 | 0.090 | 0.305 | 0.23 | 0.27 |
| poe2-fire-spiral | 2694490/03c29d83816331321cbb49dc1e7230da9207de79/ss_03c29d83816331321cbb49dc1e7230da9207de79.1920x1080.jpg | night | 0.13 | 0.0006 | 0.0086 | 0.138 | 0.23 | 0.69 |
| le-dungeon-slash | 899770/b147fd10d7b8dd0213c4acb99a6bc69be23ef220/ss_b147fd10d7b8dd0213c4acb99a6bc69be23ef220.1920x1080.jpg | dungeon | 0.20 | 0.0043 | 0.018 | 0.122 | 0.15 | 0.65 |
| le-river-beam | 899770/6a329eefc0bd1545843f235b4fac5564b7b6dba7/ss_6a329eefc0bd1545843f235b4fac5564b7b6dba7.1920x1080.jpg | day, water | 0.10 | 0.0063 | 0.060 | 0.295 | 0.15 | 0.47 |
| le-desert-fight | 899770/ss_3d0b726afdabc5b741065fab2ca45516c92e5189.1920x1080.jpg | day | 0.11 | 0.019 | 0.105 | 0.396 | 0.10 | 0.38 |
| vr-pond-slash | 1604030/ss_0fd29f799db0e9351a92b932899d9fa98b43b9c1.1920x1080.jpg | night, water | 0.11 | 0.0036 | 0.038 | 0.184 | 0.13 | 0.72 |
| vr-lamplit-street | 1604030/ss_949ccc5fc9042f8c8b186de7213f2173a96d18a4.1920x1080.jpg | night | 0.16 | 0.0002 | 0.0145 | 0.127 | 0.14 | 0.75 |
| vr-fog-lightning | 1604030/ss_230c0a379a2205b52be1c257d460e7a4ffa37428.1920x1080.jpg | fog | 0.15 | 0.0028 | 0.060 | 0.337 | 0.06 | 0.38 |
| h2-glade-fight | 1145350/ss_ef0f63061d0a0a9a7e46f3b84f125d25330e8f19.1920x1080.jpg | painted 2D | 0.07 | 0.0019 | 0.017 | 0.166 | 0.14 | 0.51 |
| h2-torch-street | 1145350/ss_f366f20f4fb699f6735581e04c7c45a1ef7bd1b8.1920x1080.jpg | painted 2D, night | 0.08 | 0.0010 | 0.018 | 0.666 | 0.08 | 0.45 |

What each shows is in `refs/refs.md` (D4: candle and brazier pools, decals, near-black floor; PoE2: flipbook fire and embers on a mossy ruin; Last Epoch: ribbon slashes, river glints; V Rising: lamplit cobbles, volumetric fog; Hades II: a style apart, not a target). A new biome adds two or three refs of its own kind (a forest by day, a marsh) the same way: store frames of a fight at the closest camera, the URL kept, measured.

## The measures (stats.py)

Each image resized to 960x540 (Lanczos), so any size compares at one. `L` is linear-light luminance: sRGB decoded (v/12.92 under 0.04045, else ((v+0.055)/1.055)^2.4), Rec.709 weights 0.2126, 0.7152, 0.0722.
- `luma_p5`, `luma_p50`, `luma_p95`, `luma_mean`: percentiles of L, 0 to 1. p5 is the blacks (the sky fill), p95 the pools.
- `fine_detail`: mean |L minus its 3x3 box blur (edges padded)| over mean L: texture sharpness and small dressing.
- `saturation`: mean (max-min)/max of the sRGB channels over pixels with max over 0.02.
- `dark_share` L under 0.05, `mid_share` 0.05 to 0.35, `bright_share` over 0.35.

`python3 -I ~/.cache/terra-review/iso-art/stats.py <out.json> <still>...` adds each still to the JSON and prints every row (`name=path` copies the still beside the JSON first). It needs numpy and Pillow, so it stays out of the repository (D121: tooling is TypeScript); a TypeScript port reading raw RGB from `ffmpeg -vf scale=960:540:flags=lanczos -f rawvideo -pix_fmt rgb24` would let it in. Put every still's line in the handoff against the refs of its kind.

## The rubric (judge.md)

Be harsh: the bar is a shipped ARPG's store screenshot, not "good for a spike". Scale for both kinds:
- **9**: could be one of the refs with no one picking it out; every corner dressed, light and effects as good as poe2-fire-spiral or vr-lamplit-street.
- **7**: a shipped indie ARPG, the same bracket as le-desert-fight with one or two tells (a flat material, a bland corner, a weak effect). **Approve at 7.**
- **5**: a good vertical slice: right camera, readable hero and creatures, light that works, but plainly a prototype beside the refs (blurry or clean textures, placeholder effects, empty ground, lifted blacks).
- **3**: a tech demo: right camera only; assets untextured-looking or wrong scale, flat light, effects absent or programmer art.
- Between is allowed, naming the nearest anchor. Judges spread ±0.5: a change under 0.5 is noise, so the pair decides.

**(a) One asset.** Inputs: the asset at the game camera in the scene's light at the scene's frame height, a crop of a ref hero or creature at the same on-screen size beside it (a hero at 0.13 of 1080p is about 140 px), and a strip of 6 to 8 frames of each clip. Score 0 to 10: 1 silhouette (filled black at game size it says what it is; head, weapon, limbs separate; reads against the scene's ground); 2 material read (metal, cloth, leather, fur, chitin at that size; PBR response, a rim, a glint; no plastic sheen or baked light); 3 texture detail (against the ref crop; no blur, noise or shimmer); 4 style match (docs/art-style.md and the ref's bracket; Tripo tells: melted details, wrong-handed straps, mush hands, seams); 5 animation read (each clip's key pose reads at game size: anticipation, contact, follow-through; feet planted; the attack's arc not along the camera's axis; a death falls). Overall is not the mean: the worse of silhouette and style caps it. Output: the five, overall, three biggest gaps (what, where, why it reads unshipped, likely cause), approve yes or no.

**(b) The scene.** Inputs: the 1920x1080 still at the game camera, a 3 to 6 s film or a frame strip, its stats line, the last kept still. Score 0 to 10: 1 composition (the hero and fight read first; dressed ground and framing props at the edges; depth front to back); 2 light (a key and two or more local sources with pools and falloff; true blacks between; rim or contrast lifting the figures); 3 effects (shape and colour at a glance, a bright core over bloom, light on the ground, fading not popping; hit-stop or a flash at contact in the film); 4 ambient (fog, motes or embers, moving foliage, glinting water, noticed in the film, never hiding the fight); 5 detail and contrast (each measure with its number and the refs' of the same kind). Output: the five with numbers, overall, three biggest gaps, what is now good, approve yes or no.

## The blind pairwise step

The iso-forest spike's absolute scores stopped at 4.0 while a pairwise judge still saw gains, so every round also runs this, in a fresh subagent that has seen neither the scores nor what changed:
1. Copy the new still and the last kept one to `blind/a.png` and `blind/b.png` in random order (note which is which outside the brief), with two refs of the same kind.
2. Ask: which is closer to the refs as a shipped game's frame, A or B, or "same"; how sure (slight, clear, strong); the one thing that decided it.
3. Keep the change if the new one wins clearly or strongly; slight or same is no gain. Two rounds in a row with no gain stops the line of work.

## Briefing a judge

A `general-purpose` subagent, one at a time, told to read the rubric and `refs/refs.md` itself (paths given), the still's stats line and the refs' in the brief, and to write its verdict to `<round>/verdict.md`; it costs about 87k of its own tokens. It judges only: no edits, renders or GPU. Run the pairwise judge before the scorer, and the scorer after a gain. Before calling a biome done, the adversarial review gate (the environment skill) is this judge at the bar of 7.

## Scores so far (the ruin at night)

r1 3.5 (scene-r1/step3-ruin: a tile slab over nothing, programmer-art fire, a fight that does not read); r3 4.5, won its pair (scene-r3/step3-fight); scene-r4/step1-closer, camera at 19 m, unjudged. The open gaps: an empty, clean floor (fine 0.063 against 0.13+), soft-blob spells and hits, figures without a rim and the knight with no visible sword.
