# Fire

The fire module's automatic check (docs/fire.md, "The automatic check"; D415): every fire measured against photographs of real fire, so a bad one fails before a person sees it. Node 22.18 or later and ffmpeg; no dependencies.

```bash
node tools/fire/measure.ts <still.png> 800x500+0+420                    # a fire's numbers in a crop
node tools/fire/measure.ts <still.png> 800x500+0+420 --class structure  # held to a class: ✗ where one falls out, exit 1
node tools/fire/references.ts [--fetch]                                 # measure the photographs into references.json
cargo xtask fire [--time noon,dusk,night] [--gate] [--hold]              # the fire sheet: every fire the game draws, held to its class
node tools/fire/bake/bake.ts <set>... | --all [--draft] [--pack]        # bake flipbooks in Blender (5.2, CPU) into assets/textures/fire/
```

- **sheet.ts** (`cargo xtask fire`) runs the yard at each hour on one of the game's shared GPU slots (D319), not `gpu.queue`, being a small run: a campfire, a log cabin set alight and a dragon breathing on a stone wall, each shot from a fixed camera. On the shot's frame `fire.seen` tells the box each kind of fire projects to on the picture (`terra_world::fire::seen`), which is its crop. It prints a line a fire and hour, writes `shots/fire/sheet.jpg` (a row a fire, a column an hour), `sheet.txt` and `table.json`. `--gate` prints only where a fire is worse than `held.json` (what is out today); `--hold` rewrites that file once a stage brings numbers in.
- **The proof** (`references.test.ts`): every reference photograph holds to its own class, and the castle film r12's wall fires (`r12-walls.json`, their numbers only) fail the structure class.

- **measure.ts** finds the flame (max channel at least 110 with red the largest and not grey, or near white) and measures its colour bands (white, pale, yellow, orange, red, red plus orange), B/R and G/R and dim share at the picture's own size, then resamples the crop so the flame is 256 px tall and measures the soot round it, the soot at its edge, the dark pockets in it (per flame-height square) and its detail by octave (fine, mid, coarse). The method is MEASURED.md's, from the castle film's rounds; the tests know their answers by construction.
- **references.ts** measures every photograph in `photos.json` (file, class, crop round its flame, light, and its Commons credit) into `references.json`: per class each number's median, least and most. A fire is held to least to most, widened by each number's tolerance (`METRICS` in measure.ts), never to the median: real fires vary.
- **The photographs stay out of the repository**: their licences (CC BY, CC BY-SA) ask for attribution and share-alike terms. They live in `~/.cache/terra-review/refs/fire-real`; `--fetch` downloads any missing from Wikimedia Commons as the same 2000 px rendition their crops were chosen on.
- **bake/** bakes the flipbooks (docs/fire.md, "The baker"): `bake.py` simulates a recipe of `bake/sets/` in Mantaflow and renders heat, motion and depth passes in Cycles; `pack.ts` packs them into the flame map, motion map and sidecar; `sheet.ts` draws `shots/fire/bake/<set>/sheet.jpg` and measures the ramp preview against the set's class. `--draft` tries a recipe coarse and short in shots/ in under a minute. **ramp.ts** is the blackbody ramp the preview colours heat with.
