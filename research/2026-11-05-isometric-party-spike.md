# What a realistic isometric party game in the manner of Baldur's Gate II looks like on our engine

Dated 2026-11-05 (spike `spike/iso`, jobs iso-forest j1 to j8). The developer asked for "a example of how we could make a realistic game that would be 2.5d isometric topdown in the style of like baldurs gate 2", then a woodland scene "with a more interesting armored model, with some trees, rocky terrain, grass, maybe some animals", "much more detailed, go for realism". Not part of the game and not a design: a picture of one direction, to look at.

## What was built

A separate Cargo workspace in `spike/iso` that reads the repository's `assets/` and carries the same Bevy patches (`cd spike/iso && cargo run`; its README lists the controls and the scripted still).

- **The view**: one orthographic camera, 42° down and turned 28°, never rotated, as the Infinity Engine's painted backgrounds are. The scene is drawn live in 3D instead of painted: PBR, cascaded sun shadows, point-light shadows, SSAO, TAA, bloom, distance haze. Day, dusk and night are on `T`.
- **The party**: a Tripo knight and three of our male body in its ranger and peasant pieces, on our clips (idle, walk, jog, sit). The controls are BG-style: green rings at the chosen's feet, a drag box, a wedge formation, pause with a red frame, live portraits rendered from each face, and a log of barks. A grid A* with string-pulling runs round trees, rocks and the pond.
- **The place**: a glade in an oak wood. It has a heightfield with a ridge and a pond, a blended ground shader (grass, dirt, trodden paths, the shade under each crown), and crowns that sway and dither out above the party. Ferns, rocks, grass drifts, deer, a boar, a fox, ducks and fireflies fill it.
- **The camp**: a fire of stones and logs with flame tongues, coals, embers and smoke. There are two trunks to sit on, bedrolls and a woodpile. The props are Poly Haven scans run through `tools/blender/scan.py`, plus two Tripo props (`art/props/camp_*`). With `--demo` the party makes camp: three sit by the fire and the knight keeps watch.

## How it was judged

Each round a judge subagent scored the dusk still out of 10 against four shipped frames: Pillars of Eternity's camp at dusk, a Pillars II forest, a Pathfinder: Wrath of the Righteous forest and a Baldur's Gate 3 forest. It measured luminance bands, fine detail and the colour of named regions, and named its three biggest gaps. From round 9 on, a blind pairwise judge also picked between the new still and the last kept one. The stills, verdicts and references are kept outside the repository, in `~/.cache/terra-review/iso-forest/` (the references in `refs/`).

| Round | Score | Change judged |
|---|---|---|
| r1 | 3.5 | The woodland camp after job 4: Tripo knight, oak wood, pond, wildlife |
| r2 | 3.5 | Job 5: firelight, the ground's layers, scanned props |
| r3 | 4.2 | Wider view (14 m tall), smaller glade framed by trunks, the fire the main light |
| r4 | 3.7 | Cool dusk with a blue fill (reverted: a lifted, colourless murk) |
| r5, r6 | 3.8, 4.0 | Shadow-desaturating grades (not kept) |
| r7 | 4.0 | Dusk as the blue hour, the near crowns silhouettes |
| r8 | 4.0 | The party sitting round the fire |
| r9 | 4.0 | Fire rebuilt; the pairwise judge clearly preferred it to r8 |

The score stopped moving at about 4.0, within the judges' spread of ±0.5, while the pairwise judge still saw gains. For look work at this level a blind pairwise choice against the last kept still is the better signal than an absolute score.

## Findings

- **The camera and controls carry the genre on their own.** A fixed orthographic view, selection rings, portraits and pause read as BG2 at once, and none of it needed anything from the engine that Bevy lacks.
- **Our assets work at this angle.** The body, its clothing pieces and its clips play unchanged from 20 m up. So do the scans from `tools/blender/scan.py`. The knight from Tripo held up best: its armour reads at this size where cloth does not.
- **Exposure and the fire's fall-off match the references.** At dusk the playfield's mean luminance is 0.081 against Pillars' camp at 0.088. Mean luminance in rings 100, 200, 300 and 400 px from the fire is 0.32, 0.18, 0.12 and 0.11, against Pillars' 0.34, 0.20, 0.15 and 0.11.
- **Detail and contrast fall well short.** Fine detail (the mean of |L − its 3×3 blur| over mean L) is 0.049, against 0.134 for Pillars and 0.085 for Pathfinder. The blacks are lifted (p05 0.031 against 0.009), and 74% of pixels sit in one grey band (Pillars 57%). The likely causes are TAA's softening with no sharpening pass after it, haze over the darks, and ground and canopy textures with no small-scale detail.
- **Hand-painted backgrounds win on composition, not light.** The references dress every corner with readable shapes: worn earth rings, separate stones, carts, several light sources. Our clearing is one pool of light in a dark mass. Closing that gap is set dressing and art direction more than rendering.

## Traps found on the way

- **The fire's point-light shadows.** A point light's shadow from small casters near it (the ring of stones, logs, a pot) throws black spokes across the whole clearing. The fix was to let only the party and the camp's big things cast: a render layer the light sees (`camp::FIRE_CASTS`). The light is also split into a near shadowed part, gone by 10 m, and a far part that casts no shadow.
- **Tonemapping a fire.** AgX's per-channel curve bleaches a fire's orange to beige (205,163,120). Tony McMapface keeps it orange, so the spike uses it at dusk and night.
- **Picking an animator.** A clothing piece's glTF has its own `AnimationPlayer`, and an animator that takes the first player it finds drives the piece, not the body, leaving the body in T-pose. Pick the body's player by name.
- **Blending a ground patch.** Vertex-colour alpha on a blended `StandardMaterial` patch did not fade it. Cutting the patch's mesh to the outline did.
- **Tripo models in Blender.** Tripo's GLBs are meshopt-compressed, which Blender cannot open. Export them with `tripo.ts export --format glb` first.

## Not measured

No frame time: the spike was never benched. Its stills were made offscreen on the RTX 3080 at 1920×1080.

## What it would take to go further

The judge's three biggest gaps after r9:

1. Sharpen after TAA (contrast-adaptive sharpening), take the haze off the darks, and give the camp's ground trodden dirt with stones and roots.
2. A flipbook flame with a white-hot core driving bloom, visible embers, and smoke that widens and fades instead of rising as a beam.
3. A cool moonlight key, so trunks and the canopy's edge read, and a second warm light. Remove the magenta the sky fill puts on wood and fur, and dress the camp.

Whether Terra wants a view like this at all, for a realm or a mode, is a design question for the developers. This spike decides nothing.
