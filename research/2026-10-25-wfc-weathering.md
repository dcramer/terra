# Weathering a building by wave function collapse: a blind test against noise masks

2026-10-25. The question: does a building read as more convincingly aged when its weathering motifs (cracks, lost stones, moss, lichen, streaks, soot) are solved by wave function collapse (WFC) over example patches, constrained by masks taken from the building's own shape, than when the same motifs are cut from noise by those masks alone? And what does it cost in CPU time and memory?

## Answer

Yes, but neither is good yet. WFC motifs (C) beat noise-cut motifs (B) in both blind panels, by +0.36 and +0.42 out of 10, p < 0.01. Both are far below the reference photograph (C scores 4.3 to 4.4, where 10 is the photograph and 3 is a clean asset). C costs about 3x the CPU time of B: 270-340 ms against 93-110 ms per building. Stored uncompressed, either way takes 15.6 MB for one house. A mixed BC4 layout brings that to 4.6 MB (3.7 KB per face) with a small error. Every WFC solve succeeded on its first try (156 of 156). Recorded as D480. The spike ships Off (`assets/data/weathering.ron`).

## Method

- **The house**: the bench village's L (rubble masonry block with a door and a window, a log wing, a gabled roof): 1290 exposed cube faces in 39 wall planes.
- **Masks** (`crates/terra_weathering/src/masks.rs`, the same in B and C): for every face, from the building's cubes alone, its edge, cavity, up and below, the sills, ledges and heads of openings within five cubes, shelter, height over the ground and age. Their product is a per-motif *want* (0..1).
- **B, masks**: each motif layer is cut from noise by its want (ridged noise for cracks, thresholded noise for moss, spots, stretched noise for streaks).
- **C, WFC** (`wfc.rs`, the overlapping model ported from Gumin's WaveFunctionCollapse, MIT): one solve per wall plane and motif layer, so a crack or a moss patch runs across cube seams. N x N patterns are learned from small ASCII example patches (`motifs.rs`): crack N = 3 at 8 px per half-metre face; loss, growth and flow N = 2 at 4 px. The masks weigh every cell's symbols and ban them where the want is nought. A contradiction restarts the plane; a plane that still fails is left without motifs, so a failure costs looks, never correctness (this is why WFC is acceptable here, though [building-tech](../docs/building-tech.md) keeps it out of wall layouts).
- **Raster** (`raster.rs`): per face, 32 x 32 texels in three RGBA8 layers (crack, loss, moss, streak; grime, wear, lichen, damp; soot, cavity, edge). The layers are packed into an atlas plus a lookup from cube and face to slot. The building shader (`assets/shaders/build.wesl`, `weather_at`) blends them over the material. Roofs take noise moss and lichen in both B and C, because the cube steps under a sloped roof banded the per-face maps.
- **Renders**: on the worker (RTX 4090) at noon, overcast, and a nearer view of the stone block's window; A is the house unweathered.
- **Judging** (D472, tools/review): 8 fresh judges per build. Each judge saw one build's three views against two fixed anchors: a photograph of a Dartmoor longhouse at 10 and an early render at 3. The brief is `tools/review/briefs/weathering.md`.

## Scores

| Panel | A (off) | B (masks) | C (WFC) | C - B | C - A |
|---|---|---|---|---|---|
| r3 | 3.65 | 4.06 | 4.42 | +0.36 [+0.12, +0.60], p 0.008 | +0.77 |
| r3b (after the patina iteration) | 3.95 | 3.91 | 4.33 | +0.42 [+0.26, +0.59], p 0.000 | +0.38 |

C led in every view (r3: noon 4.56, overcast 4.44, window 4.26). Between the panels, one iteration added an age patina over the stone, dark joints, cracks kept to the joints and pale wall lichen. It made C worse against A (+0.77 to +0.38) and was reverted. A's images were the same in both panels, so its 0.30 rise shows how far one panel's level drifts from the next. Compare builds within a panel only.

## What WFC added

The judges gave scores, not reasons, so this comes from the stills and from how the method works. With masks, a motif is a threshold of noise, so it reads as speckle laid over the wall: short drips under the window, moss in scattered dots. With WFC, a motif keeps the shape of its examples and runs across the cube seams: a crack is a connected line, moss gathers in patches at the wall foot and in the corners where the masks allow it, and a streak is a single run downward that fades. The round's harsh review of C said these read as real: the darker, damper plinth course, moss kept to the ground and the corners, fine moss specks along the base, and grime in the window reveal and sill.

## CPU time

Measured on the worker's CPU, on Bevy's main thread at frame 20, for the whole house:

| Way | Time per building | Solves |
|---|---|---|
| Masks (B) | 93-110 ms | none |
| WFC (C), 50 years | 270-340 ms | 156 of 156, all at the first try, 144,480 cells |
| WFC (C), 5 years | 216 ms | 156 of 156 at the first try |

A village of 40 houses weathered this way would take about 12 s. It must run once per building off the main thread (it is pure, so `AsyncComputeTaskPool` can take it), be cached by the building's shape and seed, and cover only buildings near the camera.

## Memory and compression

The maps are 11 channels per texel (the fourth channel of the last layer is a constant and is dropped). Measured by `Weathered::compression` (`crates/terra_weathering/src/compress.rs`) on the WFC house at 50 years. Errors are in 0..255 steps over every texel of every channel kept:

| Layout | Bytes for the house | Per face | RMS | Max | Worst channel |
|---|---|---|---|---|---|
| Raw RGBA8, as first uploaded | 15.6 MB | 12 KB | 0 | 0 | |
| BC4 per channel, full size | 7.0 MB | 5.4 KB | 0.85 | 18 | |
| Half size (16 texels a side), uncompressed | 3.8 MB | 2.9 KB | 5.3 | 219 | lichen, 8.8 |
| Half size and BC4 | 1.8 MB | 1.4 KB | 5.4 | | |
| **Mixed**, as uploaded now: motif channels BC4 at full size, mask channels BC4 at half size | **4.6 MB** | **3.7 KB** | **1.65** | **40** | edge, 3.6 |

Half size alone ruins the sharp motifs (a max error of 219 is a crack or a lichen spot lost). The mixed layout keeps them and halves only the smooth masks. No face was blank, so sharing one empty slot saves nothing on this house. The runtime uploads the mixed layout: `terra_weathering::bc4` encodes BC4 block bytes, two R-channel BC4 arrays hold the motif channels at 32 texels a face and the masks at 16 (4.6 MB for the house against 15.2 MB for the raw atlas), and `weather_at` samples both; R8 arrays of the same planes where the GPU lacks BC. Rendered beside the raw upload the house is unchanged (RMSE 0.3% of full scale, no visible difference at the window). Deriving the mask channels (edge, cavity, height) in the shader from the cube data the material already has would leave only the motif channels, about 3 KB per face.

## Remaining gaps

From the harsh review of C and the stills:

- **Flat decal moss**: saturated green blobs with hard cut-out edges that sit on top of the surface, not cushions in the joints with a damp band fading up from the ground.
- **A glossy roof with dark rectangles**: a strong specular sheen on both slopes. The aged areas are large dark panels aligned to the tile courses, where real old roofs go matte and chalky with lichen along the eaves and the ridge.
- **A 1 px dark streak from the ridge**: the WFC streak motif is one pixel wide and near black, so it starts under the ridge from nothing and runs down to the window.
- Also: the stone stays clean and bright with no crustose lichen; cracks cross single stones rather than following the joints; the log wing is near-black and uniform rather than silvered; corners are pin-sharp with no stone lost.

## Recommendation

- Build on WFC motifs constrained by shape masks, not on noise thresholds. The gain is measured and significant at 3x the CPU time, which runs once per building and is cacheable.
- Store the maps in the mixed BC4 layout (3.7 KB per face) and derive the mask channels in the shader to reach about 3 KB. Weather only buildings near the camera, off the main thread.
- Spend the next rounds on the look, not the solver: moss as joint-following cushions with height and roughness, a matte roof with lichen along its courses instead of dark panels, a wider and fainter streak motif that starts at a sill or the wall head, and silvered timber. Judge each change with a D472 panel against the same anchors. A patina laid over everything measured worse.

## Reproducing

- Code: `crates/terra_weathering` (pure), `crates/terra_bases/src/weathering.rs` (the sandbox side, reading `assets/data/weathering.ron`: `way` Off, Masks or Wfc; `age`; `seed`; `build`, part of the build's name).
- Render: set `way` in the ron, then `cargo xtask sandbox --offscreen --clean --experiment play --level yard --village --time 12 [--weather overcast] --vantage 77,1.8,-73.5,69.5,1.4,-64@0 --shots <dir>/v@180,380,580` (window view `70,1.4,-70.5,71.5,0.8,-66`). The log line "Weathering, stored smaller" gives the compression table.
- Panels: `~/.cache/terra-review/review/panels/weathering-r3` and `weathering-r3b` (local to the developer's machine).
