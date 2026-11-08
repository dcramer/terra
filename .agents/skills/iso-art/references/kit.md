# The biome kit and its loop (D634)

A biome is a data file, `crates/terra_iso/biomes/<name>.ron`, never code (D136); the shared kit in `crates/terra_iso/src/kit/` draws it. A biome that needs code shows the kit is missing something: fix the kit, with a test, never the biome. The kit holds no genre's assumptions: a neon street in rain is made like a ruin.

## A biome file's fields (the schema: `src/kit/biome.rs`)

Unknown fields are refused; a parse error names the file, line, column, field and fix (D556). Optional fields go bare (`water: (kind: Wet)`, no `Some`).

- `kind: [Night, Rain, Water]`: what its stills are judged as (Day, Dusk, Night, Rain, Snow, Water, Interior), the kinds of reference they stand beside.
- `refs: "ruin"`: its references' folder, `~/.cache/terra-review/iso-kit/refs/<refs>/` (full size, `960/` copies, `refs.md`), measured in `refs/stats.json`.
- `layout: Ruin | Forest | Marsh`: how the scene is built (the land, where the fight stands, the camera). For now each runs its biome's modules (`ruin/`, `wood/`, `marsh/`); the kit's layouts replace them as its jobs land.
- `light: (preset: "ruin-night", overrides: (...))`: a preset in `biomes/light/<name>.ron` (`src/kit/light.rs`: hour, key's direction, colour, lux and shadows, sky fill, `ev100`, tone curve, grade, haze) and the numbers this biome changes of it. The preset sets the hour unless `--hour` says.
- `ground: (layers: [...], broad, scorch)`: up to eight layers, each a scan of the `materials/ground` stack by name (`kit/ground.rs` `STACK`, true sizes) with `mask`, `amount`, `tint`, `breakup` (drifts), `wets`, `triplanar`; one without a material only wets. Drawn by the kit for the forest and marsh (below).
- `scatter: [(models | plant, density, scale, on, clump, tilt, sink, over, seed)]` (below), `decals: [(texture, count, size, on)]`, `water: (kind, level, deep)`, `weather: (wind, rain, snow, mist)`, `effects: [(preset, at)]`, `life: [(kind, count)]`.

The fields after `light` but `ground`, `scatter` (marsh and street) and the marsh's and forest's `water` are parsed and carried, each naming the iso-kit job that draws it; the ruin, forest and marsh still draw those from their own code.

## The kit's modules

- `src/kit/mod.rs`: reading and parsing a file, the errors.
- `src/kit/biome.rs`: the schema; its tests parse every biome and a full non-fantasy one.
- `src/kit/ground.rs` and `ground.wesl`: the ground's layers, rule masks and material (below).
- `src/kit/light.rs`: the light presets, `Light` (a resource): `Light::spawn` the key and fill, `Light::camera` the exposure, curve, grade and haze.
- `src/kit/water.rs` and `water.wesl`: a biome's water (below).

## The ground (`src/kit/ground.rs`, `ground.wesl`)

- Layers height-blend where their masks overlap (Mishkinis' splat: the highest of weight x 1.4 plus the scan's height wins over 0.25), then the broad variation, wet (darker 0.65, roughness 0.45), the wear slot's packed soil and the scorches.
- A mask is the layout's own map, `Site("grass")` (terrain.rs `SITES`: grass, leaves, moss, rock, mud, roots, per vertex; shade and wear in UV_0), or a rule baked from the land into two textures at 0.25 m: `Mud` (within ~1.5 m of water, under 0.5 m over it), `Wet` (~1 m, a few cm over it; `weather.rain` wets hollows), `Moss` (shade or creases, not steep), `Snow` (faces up; its `amount` takes it down steeper), `Grime` (hollows, the heights' Laplacian), `Steep`; each edge ragged by noise at two scales. Each rule has a test.
- A paved street is the same file: its scans stacked first (no asphalt or concrete in the stack yet).
- A laid floor (slabs, pavers, cobbles, bricks, kerbs) is `ground.laid` (`src/kit/laid.rs`, `laid.wesl`): one scan of `assets/materials/` at its true `metres`, drawn straight through StandardMaterial on the layout's paving mesh so its joints run true; grime in the joints (`joints`, the scan's AO marks them), `cracks`, the detail `grain`, `moss` and `wet` by the paving's maps (`Site(name)` from `laid::Place`, the second UV's x, y and the colour's alpha) or `Base`. The ruin's court is one. Its `courses` (each course's start and joints, measured off the scan's colour map) tell the units apart for `spread` (each unit's value and roughness); `dirt` (drifts and grit, `grit` its specks' share: 1 by default, 0.2 to 0.25 on a street's paving, where at 1 they read as confetti), `damp` (patches, darker and glossy: wet), `grime.tar` (an old lot's sealcoat and soaked oil, broad, dark, dry, crisp ragged edges) and `feet` (a map where walls rise) weather it further. Its `marks` (up to 16: `White`/`Yellow` paint worn in soft patches frayed by the grain, each repeat its own way (noise in the mark's frame before its repeats), `Stain` for tyres and oil, `Patch` for sealed repairs) are rectangles at world places, repeated by `every` and `count` (a dashed line, a zebra): the street's road markings. `cracks` is a Voronoi web: on asphalt or flags it reads as dried mud, so 1.0 there.

## The water (`src/kit/water.rs`, `water.wesl`)

- A biome's `water: (kind, level, deep, absorb, shore, film, scum, pads, mirror, ripples, streaks, rain)` is one face (`spawn_face` on a mesh from `surface`, its depth in the second UV), the marsh's first, the forest's pond since iso-kit j28 (a stand-in `Glint` on layer 10 for a sun's lane, unseen: a light on the faces' layer 9 lights the whole scene; its `gain` 0 there since j33), the ruin's pool since j38 (pool.rs deleted): its face a skin 2 cm over the flags with a made-up `depth` (ruin/water.rs), so its line is the fade's smooth curve out on whole flags, not the paving's cut, and the flags wet under it and damp beyond (the laid floor's `wet` site). A street's puddles are faces `level` over the road and each pavement where `puddled` stands (`Street::standing`), one mirror at the road's.
- The body absorbs per channel by depth (Beer-Lambert, `absorb` a metre) and scatters `deep`, lit as the ground; premultiplied, so the bed shows through the shallows. `shore` fades it in at the bank; `film` is a fixed depth for wet floors.
- The mirror is planar (Unreal's and Unity's flat water): a camera mirrored under the face's level renders the scene into a half-size HDR texture before the main one, the faces on layer 9 it does not see; the face samples it where it projects, moved by the ripples' slope (`mirror.distort`), at Fresnel's 2% times `lift`; `sky` and `nits` its clear colour; `smear` (a fraction of the frame, 0.04 on the street) streaks it up and down the view in 12 tent-weighted taps, fully on a thin film, a quarter on a deep puddle, as rough wet asphalt stretches a sign or a window (no crisp mirrored text). Lights light a layer-9 face: Bevy 0.20 filters lights per view, not per mesh.
- Ripples are one tiling slope map read at `fine` and `broad` metres drifting downwind (`weather.wind`); `calm` stills the margin. The key's lane and each `Glint`'s streak are pool.wesl's anisotropic Beckmann lobe (`streaks`); a `Glint` is a point or spot light, the six brightest at the eye kept (`seen`).

## The scatter (`src/kit/scatter.rs`)

- A set is `models` (GLBs, one picked a place) or a `plant` (Tuft, Fern, Leaves, Mat: wood/lawn.rs's cards, a Mat's blades lodged flat with no broad leaves beneath, a Tuft's standing over three, `tint` a palette walked by broad noise, `grey` 0..1 how far each texel of the card goes to its own grey before the tint, as no multiplied tint makes dead grass of a green card); `density` a m² at a full mask and a clump's heart; `on` a `Site(..)` of the layout's maps or a baked rule (Mud, Wet, Moss, Snow, Grime, Steep); `clump: (broad, fine, amount)` two-scale noise gathering it; `scale` log-uniform; `tilt` degrees, `sink` m; `over: (from, to)` m over the water's face, fading at both ends; sets sharing a `seed` share clumps.
- A layout offers its `Ground` (maps, height, water, baked rules, `keep` for the fight and props, the span): the marsh (marsh/lawn.rs) and the street (`strew`) so far. `budget` prints each set's count and triangles once loaded. Tests: density follows the mask, sizes spread, none outside `over`, clumps gather, a bad set names its fix.

## The effects (`src/kit/effects.rs`, `effects.wesl`)

- A biome's `effects: [(preset, at)]` names presets in `biomes/effects/<name>.ron` (slash, bolt, hit-spark, smoke-puff, spell-burst, steam, embers); none listed, the staged fight's five. An unknown name stops the run naming the folder's presets.
- A preset: its `cue` (Blow, Bolt, Swing, Loop(s)), `at` (Strike, Bark, Hero, Ground, or the file's `at`), `still` (the age a still shows), `life`; a `flash` (core in a halo), `lamps` (a light, `ground` a pool), `sparks` (streaks stretched by a `shutter`, thrown over a `delay`, falling, cooling hot to `cool` to dark), `puffs` (smoke lit by the lights at it plus the key and sky), a `trail` (slash.rs's strip; `streak: true` a projectile's, spell.rs's bolts: a hot line in a halo narrowing down it). Values are nits and lumens, times the view's exposure.
- Cues: hit.rs writes `Cues.blow` as the blow lands, spell.rs `Cues.bolt` as a bolt strikes; the kit spawns no lamp or flash for a trail preset (slash.rs lights the slash's at the edge; the bolts carry their own two lights and draw the `bolt` flash as their head).
- One shader, four modes (glow, lit puff, trail: a blade's motion streak, a thin hot glint on the edge's path in a feathered glow, nearly additive; streak), each soft against the depth prepass (Lorach 2007); a view drawing them or the fight's decals needs `DepthPrepass` (the water's mirror camera too).
- Budget: 400 quads and 8 lights a biome, printed at startup.

## The weather (`src/kit/weather.rs`, `weather.wesl`)

- `weather: (wind, rain, mist, drops: (fall, shutter, width, bright, splashes), layer: (height, water, hollows, drift, banks, thick, colour, forward, keylit, lamps, glow, clear))`; a layout inserts its `Floor` (the street's strew, the marsh's land).
- Rain: one mesh of streaks (8000 at `rain: 1`) and splashes (6000: a bright fleck spreading to a ring of droplets, 1 to 4 cm, lying on the floor, 25 times a streak's share), moved in the vertex shader: camera-facing, stretched by the shutter, lit only by the 16 brightest point and spot lights near them (`lit`, a trace of key and sky), so they show in the cones; splashes off the water; the water's own `rain` draws the rings. Budget printed.
- Mist: Bevy's volumetric fog in a box over the floor, a 3D density thickest at the surface (e-fold `height`), `water` times over open water, more in hollows, drifting banks, none at the centre of each `clear` circle (x, z, r); the lamps get `VolumetricLight` if `lamps`, the key if `keylit` (a fire lighting the mist blooms a halo round it: lamps off, iso-kit j36). Never sheets (traps.md).

## Adding a biome

1. Its references first: `refs/<name>/` with `960/` copies, measured into `refs/stats.json` (`node tools/iso-art/stats.ts`, judging.md).
2. `crates/terra_iso/biomes/<name>.ron` with `kind`, `refs`, a `layout` and a light preset (a new one in `biomes/light/<preset>.ron` when none fits); `cargo test --release -p terra_iso` parses it (on the worker).
3. Shoot it: `node crates/terra_iso/loop.ts shoot --biomes <name> --dirty`; the first round has no kept still, so it is judged alone and kept by `verdict`. By hand: `--biome <name> --shot f.png` (scene.md's recipe).

## The loop's command: `node crates/terra_iso/loop.ts` (tests: `node --test crates/terra_iso/loop.test.ts`)

- `shoot [--biomes a,b] [--film] [--dirty]`: one worker job runs terra_iso's tests and every biome's still (with `--film`, its 16 s film), on the film's fixed step with pipelines compiled synchronously, so unchanged code gives the same still (four biomes 0.00% px apart over three shots; async pipelines left neon's paving black on a cold shader cache). Anything ranked and cut to a count (lamps, glints) needs a total order, never spawn or entity order. It files them in `~/.cache/terra-review/iso-kit/rounds/<round>/` and prints a line a biome: each measure beside its refs' range (`↓` below, `↑` above) and what moved from its kept still. A biome changed when a measure moved more than 0.0005 or more than 0.025% of pixels by more than two levels; an unchanged one ties with no judge.
- For each changed biome it writes two briefs in `rounds/<round>/briefs/`: `<biome>-judge.md`, the adversarial judge (the still at 960 and 1920 and its stats line, `judge.md` part (a)), and `<biome>-pair1.md`, the blind pair (`iso-kit/blind/<round>-<biome>-1/a.png, b.png`, order random, the key only in `history.json`, `judge.md` part (b)). The lead runs each brief as a subagent; the command never judges.
- `record <round> <biome> pair <A|B|same> <slight|clear|strong>` and `record <round> <biome> score <overall> "<gaps>"` (a second score keeps the first's in its `earlier`). A slight loss on the first pair prepares the second, the order swapped. `--rejudge [n]` puts a blind judge's answer over pair n's earlier one (kept in its `earlier`) and warns when the round's verdict no longer follows.
- `verdict <round>`: KEEP if no biome lost (a clear or strong loss, or a slight loss on both pairs), and the round's stills become the kept ones (`iso-kit/kept/`); else REVERT naming the losers: undo the change (git revert). It refuses while a pair waits for its answer (`--revert` closes such a round, copying no stills: the change undone, or unable to win), and on a round older than a biome's kept still (keeping it would put the older stills back; restore one on purpose with `rebase`).
- `rebase <round> "<why>"`: its stills kept unjudged, only when how stills are shot changed (the fixed step, the size, the camera), never the look; the first fixed-step round (261106-2239) was kept so, j1's seeds being real-time shots.
- `drop <round> "<why>"`: closes a round no judge will answer (a plain re-shoot, a withdrawn or superseded change, a film-only round) as DROP; its stills are never kept and no shoot measures against it. A round with a judge's answer closes by `verdict` (or `--revert`), unless that answer was against a kept still since replaced.
- `history`: each biome's rounds, pairs, scores, verdicts and kept still. The kept stills were seeded from iso-kit j1's (`iso-kit/j1/kit-*.png`).
