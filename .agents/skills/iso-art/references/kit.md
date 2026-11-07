# The biome kit and its loop (D634)

A biome is a data file, `spike/iso/biomes/<name>.ron`, never code (D136); the shared kit in `spike/iso/src/kit/` draws it. A biome that needs code shows the kit is missing something: fix the kit, with a test, never the biome. The kit holds no genre's assumptions: a neon street in rain is made like a ruin.

## A biome file's fields (the schema: `src/kit/biome.rs`)

Unknown fields are refused; a parse error names the file, line, column, field and fix (D556). Optional fields go bare (`water: (kind: Wet)`, no `Some`).

- `kind: [Night, Rain, Water]`: what its stills are judged as (Day, Dusk, Night, Rain, Snow, Water, Interior), the kinds of reference they stand beside.
- `refs: "ruin"`: its references' folder, `~/.cache/terra-review/iso-kit/refs/<refs>/` (full size, `960/` copies, `refs.md`), measured in `refs/stats.json`.
- `layout: Ruin | Forest | Marsh`: how the scene is built (the land, where the fight stands, the camera). For now each runs its biome's modules (`ruin/`, `wood/`, `marsh/`); the kit's layouts replace them as its jobs land.
- `light: (preset: "ruin-night", overrides: (...))`: a preset in `biomes/light/<name>.ron` (`src/kit/light.rs`: hour, key's direction, colour, lux and shadows, sky fill, `ev100`, tone curve, grade, haze) and the numbers this biome changes of it. The preset sets the hour unless `--hour` says.
- `ground: (layers: [...], broad, scorch)`: up to eight layers, each a scan of the `materials/ground` stack by name (`kit/ground.rs` `STACK`, true sizes) with `mask`, `amount`, `tint`, `breakup` (drifts), `wets`, `triplanar`; one without a material only wets. Drawn by the kit for the forest and marsh (below).
- `scatter: [(models, density, scale, on)]`, `decals: [(texture, count, size, on)]`, `water: (kind, level, deep)`, `weather: (wind, rain, snow, mist)`, `effects: [(preset, at)]`, `life: [(kind, count)]`.

The fields after `light` are parsed and carried, each naming the iso-kit job that draws it; the ruin, forest and marsh still draw those from their own code.

## The kit's modules

- `src/kit/mod.rs`: reading and parsing a file, the errors.
- `src/kit/biome.rs`: the schema; its tests parse every biome and a full non-fantasy one.
- `src/kit/ground.rs` and `ground.wesl`: the ground's layers, rule masks and material (below).
- `src/kit/light.rs`: the light presets, `Light` (a resource): `Light::spawn` the key and fill, `Light::camera` the exposure, curve, grade and haze.

## The ground (`src/kit/ground.rs`, `ground.wesl`)

- Layers height-blend where their masks overlap (Mishkinis' splat: the highest of weight x 1.4 plus the scan's height wins over 0.25), then the broad variation, wet (darker 0.65, roughness 0.45), the wear slot's packed soil and the scorches.
- A mask is the layout's own map, `Site("grass")` (terrain.rs `SITES`: grass, leaves, moss, rock, mud, roots, per vertex; shade and wear in UV_0), or a rule baked from the land into two textures at 0.25 m: `Mud` (within ~1.5 m of water, under 0.5 m over it), `Wet` (~1 m, a few cm over it; `weather.rain` wets hollows), `Moss` (shade or creases, not steep), `Snow` (faces up; its `amount` takes it down steeper), `Grime` (hollows, the heights' Laplacian), `Steep`; each edge ragged by noise at two scales. Each rule has a test.
- A paved street is the same file: its scans stacked first (no asphalt or concrete in the stack yet).
- A laid floor (slabs, pavers, cobbles, bricks, kerbs) is `ground.laid` (`src/kit/laid.rs`, `laid.wesl`): one scan of `assets/materials/` at its true `metres`, drawn straight through StandardMaterial on the layout's paving mesh so its joints run true; grime in the joints (`joints`, the scan's AO marks them), `cracks`, the detail `grain`, `moss` and `wet` by the paving's maps (`Site(name)` from `laid::Place`, the second UV's x, y and the colour's alpha) or `Base`. The ruin's court is one. Its `courses` (each course's start and joints, measured off the scan's colour map) tell the units apart for `spread` (each unit's value and roughness); `dirt` (drifts and grit), `damp` (patches) and `feet` (a map where walls rise) weather it further.

## Adding a biome

1. Its references first: `refs/<name>/` with `960/` copies, measured into `refs/stats.json` (`node tools/iso-art/stats.ts`, judging.md).
2. `spike/iso/biomes/<name>.ron` with `kind`, `refs`, a `layout` and a light preset (a new one in `biomes/light/<preset>.ron` when none fits); `cargo test` in the spike parses it (on the worker).
3. Shoot it: `node spike/iso/loop.ts shoot --biomes <name> --dirty`; the first round has no kept still, so it is judged alone and kept by `verdict`. By hand: `--biome <name> --shot f.png` (scene.md's recipe).

## The loop's command: `node spike/iso/loop.ts` (tests: `node --test spike/iso/loop.test.ts`)

- `shoot [--biomes a,b] [--film] [--dirty]`: one worker job runs the spike's tests and every biome's still (with `--film`, its 16 s film), on the film's fixed step, so unchanged code gives the same still (two shots of the ruin agree to 0.0001). It files them in `~/.cache/terra-review/iso-kit/rounds/<round>/` and prints a line a biome: each measure beside its refs' range (`↓` below, `↑` above) and what moved from its kept still. A biome changed when a measure moved more than 0.0005 or more than 0.025% of pixels by more than two levels; an unchanged one ties with no judge.
- For each changed biome it writes two briefs in `rounds/<round>/briefs/`: `<biome>-judge.md`, the adversarial judge (the still at 960 and 1920 and its stats line, `judge.md` part (a)), and `<biome>-pair1.md`, the blind pair (`iso-kit/blind/<round>-<biome>-1/a.png, b.png`, order random, the key only in `history.json`, `judge.md` part (b)). The lead runs each brief as a subagent; the command never judges.
- `record <round> <biome> pair <A|B|same> <slight|clear|strong>` and `record <round> <biome> score <overall> "<gaps>"`. A slight loss on the first pair prepares the second, the order swapped.
- `verdict <round>`: KEEP if no biome lost (a clear or strong loss, or a slight loss on both pairs), and the round's stills become the kept ones (`iso-kit/kept/`); else REVERT naming the losers: undo the change (git revert). It refuses while a pair waits for its answer.
- `rebase <round> "<why>"`: its stills kept unjudged, only when how stills are shot changed (the fixed step, the size, the camera), never the look; the first fixed-step round (261106-2239) was kept so, j1's seeds being real-time shots.
- `history`: each biome's rounds, pairs, scores, verdicts and kept still. The kept stills were seeded from iso-kit j1's (`iso-kit/j1/kit-*.png`).
