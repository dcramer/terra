# Fire

The fire module's automatic check (docs/fire.md, "The automatic check"; D415): every fire measured against photographs of real fire, so a bad one fails before a person sees it. Node 22.18 or later and ffmpeg; no dependencies.

```bash
node tools/fire/measure.ts <still.png> 800x500+0+420                    # a fire's numbers in a crop
node tools/fire/measure.ts <still.png> 800x500+0+420 --class structure  # held to a class: ✗ where one falls out, exit 1
node tools/fire/references.ts [--fetch]                                 # measure the photographs into references.json
```

- **measure.ts** finds the flame (max channel at least 110 with red the largest and not grey, or near white) and measures its colour bands (white, pale, yellow, orange, red, red plus orange), B/R and G/R and dim share at the picture's own size, then resamples the crop so the flame is 256 px tall and measures the soot round it, the soot at its edge, the dark pockets in it (per flame-height square) and its detail by octave (fine, mid, coarse). The method is MEASURED.md's, from the castle film's rounds; the tests know their answers by construction.
- **references.ts** measures every photograph in `photos.json` (file, class, crop round its flame, light, and its Commons credit) into `references.json`: per class each number's median, least and most. A fire is held to least to most, widened by each number's tolerance (`METRICS` in measure.ts), never to the median: real fires vary.
- **The photographs stay out of the repository**: their licences (CC BY, CC BY-SA) ask for attribution and share-alike terms. They live in `~/.cache/terra-review/refs/fire-real`; `--fetch` downloads any missing from Wikimedia Commons as the same 2000 px rendition their crops were chosen on.
