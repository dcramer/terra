# The mountain lake's reference: Far Cry 5's valleys and Morskie Oko's photographs

2026-10-15. Question: what should the mountain lake demo ([docs/demos.md](../docs/demos.md#1-a-mountain-lake), D321) look like, measured, before anything is built, and what do the real place and the game frames say about its rules? Step 1 of the demo's order of work.

Tags as in [2026-10-15-biome-demos.md](2026-10-15-biome-demos.md): **[P]** seen in the frames or photographs themselves, **[I]** inferred.

## What was collected

Everything is kept outside the repository, under `~/.cache/terra-review/refs/mountain-lake/` (it is someone else's work and the repository is public).

- **Game frames, the look** (`game/`): eleven frames of Far Cry 5's Montana by day, forested valleys, rivers and lakes under far ranges. Three from Ubisoft's Steam store screenshots (app 552520), eight sampled every 2 s from four of its five store trailers (the HLS streams the store's `appdetails` lists), letterboxes trimmed. The reference `tools/sandbox/looks/mountain-lake-day.json` (`look.ts reference --open`).
  - **Sons of the Forest** (1326470) was looked through first: its eleven store screenshots and four trailers show forest interiors, beaches, caves and snow under trees, and only one snowy peak, behind a beach. None looks across a lake to forested slopes, so none is in the reference.
  - **Red Dead Redemption 2** (1174180): five screenshots and two trailers; deserts, plains, swamps, towns and a snowy pass at the player's feet. Nothing of a mountain lake by day.
- **Photographs, the rules** (`photos/board/`): eighteen photographs of Morskie Oko in the Tatras in summer and early autumn by day, from Wikimedia Commons' category (296 files), looking across the lake to spruce slopes, dwarf pine, scree fans, rock walls and snow patches. The reference `tools/sandbox/looks/morskie-oko-day.json`, and the photo board for the review. Credits, each licence checked: Andrzej Harassek (CC BY-SA 3.0), Marek Mróz (CC BY 4.0, four), Łukasz Śmigasiewicz (CC BY-SA 3.0 pl), Sławomir Hutryk (CC BY-SA 3.0), Guntars Mednis (CC BY-SA 3.0, two), MARELBU (CC BY 3.0, three), Dariusz Zięcik (CC BY 3.0), Kameleon1608 (CC BY 3.0), Gumijagodzianka (CC BY 3.0), Jakub Hałun (CC BY-SA 4.0, three). Pragser Wildsee (370 files) and Moraine Lake (42) are downloaded as previews for later lakes.

## Measured by band

`look.ts` now measures an open view by band (D326): the sky, read down each column from the top while it stays sky-like (bluer than green and red, or bright, pale and smooth as cloud is), its skyline smoothed by a running median; the far land, a tenth of the frame below the skyline; the near ground, the lower third; the middle distance between. Checked by eye on every frame with `--draw-bands`; it misreads a frame faded to black (dropped) and lets a lone tree's crown take some sky.

| | Far Cry 5 | Morskie Oko |
|---|---|---|
| Whole frame mean, p5 | 0.386, 0.060 | 0.497, 0.210 |
| Below 0.08 | 10% | 1% |
| Red / green, blue / green | 1.03, 0.93 | 0.92, 0.93 |
| Local contrast | 0.28 | 0.17 |
| Sky: share, mean, blue / green | 25%, 0.73, 1.05 | 25%, 0.76, 1.06 |
| Far land: mean, blue / green, saturation, contrast | 0.40, 0.90, 0.29, 0.23 | 0.51, 1.00, 0.17, 0.13 |
| Middle: mean, blue / green, saturation | 0.28, 0.83, 0.37 | 0.42, 0.87, 0.26 |
| Near: mean, blue / green, saturation | 0.24, 0.74, 0.45 | 0.38, 0.79, 0.37 |
| Far land under the sky | 0.45 | 0.34 |
| Far bluer than middle | 0.07 | 0.13 |

- **The game is the photographs with more contrast and colour** [P]. Its far land is darker under the sky (0.45 against 0.34) and less blue than the middle distance (0.07 against 0.13): the game keeps far mountains readable, and the real air washes them more. Saturation falls with distance in both (near 0.45 to far 0.29 in the game, 0.37 to 0.17 in the photographs).
- **So the demo's air is aimed between them** [I], nearer the game: far land 0.35 to 0.45 under the sky, 0.07 to 0.13 bluer than the middle distance. Today's air, distances times 18 (D212), would wash a ridge 2 km off as 36 km of real air; the far band says how far to bring it down (step 7).

## What the photographs say of the rules

Each is a check the field maps (step 2) and the views must pass. From the photo board [P]:

- **The forest reaches the water** on every gentle shore, spruce in a closed canopy. On the far side it climbs the lower slopes in tongues up the gentler ground and ends in a ragged band, not a contour: higher on ridges' sunny flanks, lower in gullies where avalanches and scree run (D323).
- **Above it, dwarf pine** in dark green mats over the lower rock, thinning into alpine grass that runs up in green strips between rock ribs.
- **Scree fans** spread from every gully's foot, pale grey, at a steady angle, some reaching the shore; boulders at the toe.
- **Rock** is most of the upper half of every view: grey granite in walls and ribs, darker where wet, with green ledges on the gentler steps.
- **Snow in early summer** lies only in gullies and under north walls, as narrow white tongues and patches, never as a cap; by autumn a few patches are left.
- **The water** is dark green-teal, clear over pale stones in the shallows near the eye, mirroring the far slopes in still air and the sky's grey where wind ruffles it.
- **Far peaks** behind stand a little bluer and paler than the near walls, but every rib still reads.

How Far Cry 5 builds its land is in [2026-10-15-biome-demos.md](2026-10-15-biome-demos.md#mountains-and-valleys): Houdini tools generating water, cliffs and vegetation together [P].

## What it leaves open

- The tree line's height against the slope's facing in the photographs is measured by eye only; the field maps will give ours in numbers.
- A dusk reference and a reference at the shore, under the trees, come with the views that need them.
