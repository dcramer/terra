---
name: environment
description: Build or judge how a place looks in Terra: a biome's forest, ground cover, trees, light, shade, haze and time of day, against a measured reference of the look aimed at (another game's frames, photographs). Covers making the reference, the views and films a look is judged on, measuring before tuning, the checks every change passes (popping, collision, frame time), the adversarial review at milestones, and the traps that cost jobs before. Use when asked to make a biome or environment look better or like a game or place, to tune light, shade, fog or night, or to review how a place looks.
---

# Building how a place looks

The lush forest (2026-10-13 to 14) went from an adversarial score of 3/10 to over 6 by this loop, and lost most of its jobs where it was skipped: tuning by eye, fixing a reviewer's symptom without finding its cause, or measuring the wrong thing. Do it in this order. The art guide ([docs/art-style.md](../../../docs/art-style.md)) decides how anything looks, and [its Places section](../../../docs/art-style.md#places) what each place is aimed at, the numbers it is held to and the rules every place keeps, each with its measure (D356); read it first. This is how to get there and know it.

## 1. A reference before anything

A look is aimed at something you can measure, never at memory or adjectives.

- **Collect frames of the target** from first-party sources: a game's Steam store screenshots and trailers (`https://store.steampowered.com/api/appdetails?appids=<id>` lists both; trailers are HLS streams `ffmpeg` can sample every two seconds), and photographs where the look is the real world's. Keep only frames of the thing aimed at: drop letterboxed cinematics, menus, torch- or flashlight-lit shots, and other biomes. One reference per condition: day, dusk, night are different targets.
- **Keep the pictures out of the repository** (it is public and they are someone else's): under `~/.cache/terra-review/refs/<name>/`, on disk (`/tmp` is RAM on the developers' machine and is wiped by a restart).
- **Make the reference** from them: `node tools/sandbox/look.ts reference <name> --about "…" --source <url> <frames…>` writes `tools/sandbox/looks/<name>.json`, the measured means and where they came from, which is committed. For open views add `--open`, which measures each frame by band too (sky, far land, middle, near; D326), so peaks lost in haze or standing too sharp show; `--draw-bands <dir>` draws where the bands fell. Existing: `sotf-forest-day`, `sotf-forest-night`, `sotf-forest-snow` (its winter), `mountain-lake-day` (Far Cry 5, by band), `morskie-oko-day` (photographs, by band), `tundra-autumn-day` (photographs of tundra in late summer and early autumn, by band).
- **Write a research note** of what the reference shows and how ours differs, with numbers ([research/2026-10-14-sotf-forest-look.md](../../../research/2026-10-14-sotf-forest-look.md) is the model), and what is known of how the target is built (marked primary or inferred).

## 2. The views it is judged on

- **The vantage sheet**, `cargo xtask vantage` ([tools/sandbox/vantage.ts](../../../tools/sandbox/vantage.ts)): fixed views at two hours, the canopy's sky share looking up, how much the middle layer hides at eye height, and a "look" line per view against the reference. A new biome adds its own views to `VIEWS` and `FOREST_VIEWS`.
- **A season**: `--date <month-day>` on the vantage sheet, a film or any run sets the world's clock to that day of the year (docs/climate.md); judge a season at a few dates, late August, mid October, late November, January, as research/2026-10-15-seasons.md did.
- **Films**, `cargo xtask film --time <hour>` (the walk bench's route, with a look up into the canopy) and `node tools/sandbox/film.ts --time <hour> --out <file> -- <sandbox options with --route …>` for a route through a particular stand. Check a film's walker keeps moving (the route walker detours round what blocks it, D276) and the camera presents the place well: a reviewer judges what the film shows.
- **Measure a film or shots**: `node tools/sandbox/look.ts measure --player --against <reference> <film.mp4 | shots…>`.

## 3. Measure, find the cause, then change one thing

- **Before tuning, measure where the gap is**: the look numbers, crops of the thing at 1 to 5 m beside the reference's, luminance spread inside a crop, frame differences for motion.
- **Find what draws a pixel** by colouring a shader's output (a throwaway edit: everything the foliage shader draws red); find which light makes it so by turning one source off at a time **in code**, rebuilt, shot with `--vantage` and `--shots`. Removing or changing camera components over the remote protocol does not take effect, and `daylight` rewrites the lights every frame: such a test shows nothing changed and misleads.
- **Mind the eye**: auto exposure re-normalises every frame, so a change that darkens one thing brightens the rest. Compare ratios inside one frame (a sun fleck against the shade beside it), or the reference's numbers, not one number before and after.
- **Values are sRGB**, as a person sees them; a ratio of sRGB values understates the linear one (roughly its square).
- **One change, measured, kept or reverted.** A reviewer's "too blue" can be the haze, the sky light, the leaves' reflection or the air's aerial perspective; only switching each off tells which.

## 4. Every change passes

- `node tools/sandbox/lodcheck.ts`: nothing pops as levels of detail change, nothing pure black (a light gone missing, NaNs).
- `cargo xtask collide`: what should stop a walker does.
- The fire sheet, when the change touches fire, its light, the fog its light shines in, or the tonemapper (D415): its gate runs on main continuously (`cargo xtask continuous`), not before landing; after landing, `.git/terra-continuous/status.txt` says whether a fire got worse than the held baseline at noon, dusk or night.
- `cargo xtask bench walk`: within the frame budget ([development.md](../../../docs/development.md#measuring-performance), D25). Frame time matters as much as the look: prefer free changes (exposure, keys, grading, constants) and say what anything else costs.
- **`cargo xtask foliage`** ([tools/textures/foliage.ts](../../../tools/textures/foliage.ts), D368, also in `cargo xtask check`): every leaf, needle and frond card the plants wear measured from its atlas and normal map against what reads as leaves (the guide's rule 12): leaves a card, texels a leaf and a pixel at 2 m, relief and tilt leaf to leaf, a thickness map, detail and spread in a leaf, light and hue leaf to leaf, living green. A table a card; `--draw <dir>` shows the leaves it counted. A card changed or added passes it, or its failure goes in `tools/textures/foliage.json`'s `known` with why. In the picture, `looks` holds flat foliage, leaf colour spread and leaves catching the light.
- **`cargo xtask looks --place <name>`** ([tools/sandbox/looks.ts](../../../tools/sandbox/looks.ts), D356): shoots the place's measured views and prints each number the style holds against its reference, pass or fail, with the view furthest out; it fails on any. `--from <dir>` measures views already shot (a vantage run's). A new place gets its entry in `tools/sandbox/style.json` under the guide's rules; a new rule a picture can show gets a measure in `look.ts`, tested on made-up pictures whose answer is known, then a row in each place's entry.

## 5. The review at milestones

Not every change: when a pass is done, a fresh subagent judges it adversarially against the reference frames, side by side, and scores it. Give it the films and vantage shots, the reference frames' folder, the previous reviews' scores and gaps, what changed since, and tell it to measure, not to edit or use the GPU. Ask for an overall score and sub-scores (foliage density, foliage asset quality, trees, ground, light and shade, atmosphere, motion and wind, glitches including the camera), the top six gaps with where they are seen, why they read as not shipped quality and a guess at the cause, what is now good, and whether it approves. Then find each gap's cause yourself before handing any of it on: the reviewer's guess was wrong about as often as right. Give the reviewer the guide's rules and the looks table too; a gap it finds that comes back from place to place is a rule: write it in the guide, and measure it in the check where a picture can show it.

## 6. Working with agents on it

- **Every GPU run goes through `cargo xtask gpu <class> -- <command>`** (D319, [tools/gpu](../../../tools/gpu/README.md)): stills, sheets and films start at once and share the card, with no queue; only a benchmark (`bench`) waits for it alone. Measure frame time only on a quiet machine (the 1-minute load under 8): other agents' builds slow the game's CPU work and spoil the numbers.
- Brief each agent with this skill, the gap in the reviewer's own words, your measurements of the cause, the files, the checks above, and where its scratch goes (`~/.cache/terra-review/<task>/`, kept small).
- Decisions: what the place should be (an oak wood or a conifer stand, leaves flat or true to life) is the developers'; ask, record their words. How to draw it is yours; record the measured reason.

## Traps found the hard way

Each cost a job in the lush forest; check for them first.

- **Bevy's alpha test multiplies the vertex colour's alpha into the base colour; the prepass does not.** A mesh carrying something else in its vertex alpha (the crowns' sky seen) is cut in the main pass and kept in the prepass, and stands black in its own shape (D272).
- **Bevy's own diffuse transmission lights a leaf's far side from the open sky**, unshaded by anything; and an irradiance volume replaces the environment map's diffuse but drops its multiple-scattering term, a seam where it ends (D270, D279).
- **Bevy's distance fog and volumetric fog's ambient light are mixed in after the exposure, at the brightness given**: left at the day's, a night is lit from within by its haze (D310).
- **A small planet's atmosphere drawn Earth-sized with distances scaled up** gives a trunk fifty metres off a mountain's aerial blue; under trees the scale must shrink (D299).
- **`GeneratedEnvironmentMapLight` refilters every frame** (nearly 2 ms); filtered only after a repaint, the first window must outlast its pipelines' compiling, or every shade is black (D270).
- **A colour grade's temperature is strong** (0.08 turned the forest orange) and a day's grade must not stay on at night (D302, D310).
- **Wind phase from each vertex's own place** makes every leaf card wobble like jelly; flutter takes a phase at a branch's scale (D282).
- **Leaf cards need real texel density per leaf** (over 100 px a leaf near), colours graded to living leaves, and normal and gloss maps, or leaves read as flat fills (D291, D292); `cargo xtask foliage` measures each card so. **A good card still reads flat** when the plant is drawn so its leaves differ only in brightness: the lush forest's hazel, bramble and oak scrub cards measure as well as the oaks', and the bushes still read flat (one saturation throughout, flat foliage 82% by the path against Sons of the Forest's 18%) until they were tinted and shaded as the crowns (D359). Look at the picture's numbers, not only the card's (D368).
- **A third-person camera boom that collides with every trunk pumps in a forest**: keep the lens out of solids, let thin trunks occlude briefly, fade whole plants in the way (D274, D285).
- **lodcheck's views run one at a time** now: a heavy forest no longer fits several sandboxes on the GPU, and an out-of-memory run dies silently.
