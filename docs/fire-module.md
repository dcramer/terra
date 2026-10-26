# The fire module: boundaries and the general flame

Status: Leaning (D466), the agents' technical design; extends [fire.md](fire.md) (D415). The design questions stay Open in fire.md's last section. The flame's motion from a baked simulation is proposed in its last section (D493), waiting on the developers.

The developer, 2026-10-25: "I would love to work on some effects like fire in general, since it's ass. we wanna make sure that weather states and etc. will work well... do have a campfire optimization, but steer it more towards general fire textures and animations. this should be a self contained module as much as possible." On the review desk he chose to rebuild the flame body with a new technique over more card tuning.

Why: about forty rounds tuned the campfire's cards; the judges still call every kind "painted opaque cards" (fire/HANDOFF.md, m8-m9), and the hearth's paragraph in fire.md grew into hearth-only tricks no other kind shares. The one fire that holds its sheet rows at every hour is the marched breath (D430), a volume.

## Boundaries

`terra_world::fire` is one plugin. The rest of the game talks to it only through:

- **Spawn a fire of a kind at a place**: a `Fire { kind, heat, fuel }` component on an entity with a transform, or a `Blaze` a frame for a sim's many sites (building's fires, D377). The kind names a preset.
- **Set its fuel and heat**: the component's fields. Fire never decides what burns or how long; the sim or gameplay owning it does.
- **Put it out**: heat to 0. Fire plays the die-down (steam in rain, smoke, embers) and reports `Out` when nothing is left to draw.
- **Ask where fire is**: `fire::seen` (crops, the sheet), and the heat discs it gives off (lights, melt, see Weather).

Inside the module:
- **Presets as data**: `assets/data/fire.ron`, one entry a kind (candle, torch, campfire, brazier, house blaze, breath, fireball). Today kinds are told apart by size (`TORCH_M`) and hearth constants in `fire.wesl`; moving those numbers into the preset is the first step, and code naming a kind is a fault (D136).
- **Its own shaders and assets**: `fire*.wesl`, `assets/textures/fire/`, `fire.ron`. Nothing outside reads them.
- **A plain crate, `terra_fire`** (D28), for what needs no engine: presets and their parsing, a fire's sources from its fuel layout (a hearth's logs, a torch's head, a blaze's sites), its flicker and timing, the ramp's keys, the wind and rain rules below. Tested without a GPU. `terra_world::fire` keeps the buffers, materials, lights and passes.

What stays coupled, and why:
- **The film mask and tonemapper** (`fire::film`, D415): a flame's share of the pixel must reach the camera's curve; that is the renderer's, not the fire's.
- **Burnables' looks** (char, glowing cracks) are the burnable's material, driven by fire's flicker signal.
- **Building's fire sim** (D377) hands sites as `Blaze`s; wildfire would too.
- **Snow and wetness** read fire's heat discs; fire does not paint the ground.

## The general flame: one marched body for every kind

Every kind's flame body is one **marched flame field**, drawn in a box round the fire: a shape from the fire's sources, displaced by noise scrolled up through it, its edge a thin sheet, its heat coloured by the shared ramp. Cards stay only for sparks and embers, smoke (large, slow media, six-way lit, D325) and the flare; and for the far LOD until the bench says the march fits there.

The candidates, by what an agent can verify:

| | Raymarched field over noise | Better baked flipbook | Hybrid (field over a baked 3D loop) |
|---|---|---|---|
| Seen from any side | yes: the same flame from above as from the side | no: a card from above is a card | yes |
| Iteration | a shader edit and a still in minutes | a Mantaflow bake of 1.5-2 h per try at 256³ | a bake, then shader edits |
| Evidence here | the marched breath holds its sheet rows at every hour (D430) | our Mantaflow one-sim read as "soft haze" (3.5 against main's 4.06); its vectors were worse than none; a pro CC0 flipbook failed our gate | none yet |
| Wind, rain, put out | a shear, a heat scale, a shrink of the same field | a lean of the card; the frames can't change | as the field |
| Cost | screen area x steps; bounded by the box and LOD | cheapest | as the field, plus 3D texture memory |

**Chosen: the field, procedural first.** It is what a tool can measure soonest (sheet, motion strips, bench), it answers the "cards" fault at its root, and the hybrid stays the upgrade path: the noise it reads is one function, so a baked 3D loop can replace it if a measured gap asks for one. Shipped games mostly bake flipbooks (research/2026-10-19-how-games-draw-fire.md), and that note judged a raymarched fire not worth it for gameplay; we depart because our flipbooks never reached the look in forty rounds, and our breath's march did, as Horizon's clouds and CS2's smoke march large media in play.

**What every kind takes from it** (shared code, one function in `fire.wesl`):
- **Shape**: a union of teardrop lobes over the fire's sources, each as wide as its source and as tall as the preset says; the lobes merge low and part high.
- **Motion**: the field's points are moved before its noise is read, by two octaves of domain-warped value noise (the breath's tiling volume texture) scrolled up at the plume's speed, faster with height, so tongues rise, neck and tear off. No loop and no seam.
- **Edge**: the flame is where the displaced field passes a threshold, over a feather of a few centimetres, so its edges are torn sheets, not soft haze; inside, it is thin enough that the hot core glows through (emission and absorption, as the breath).
- **Heat**: from depth inside the field and height over the fuel, through the shared ramp's row for the fuel (D415's one ramp), the hottest low and inside, red at the torn tips.
- **Writes**: its colour, the flame's share to the film mask, and its depth where it first hides half of what is behind (as the breath does), so fog and the matte treat it as they treat the breath.

**What each kind adds** (preset data only): its sources (a torch head's one point, a campfire's logs, a blaze's sites, a breath's stream), its lobe height and width, its scroll speed (big fires roll slower), its ramp row, its brightness, and its LOD distances. The breath keeps its own gas (the stream and fireball) but shares the march, ramp and edge.

**LOD**: near, the full march; mid, half the steps at half resolution; far, the flare and a glow card; a burning village is measured on the bench before play marches every site.

## Weather and wind

- **Wind leans and stretches**: the field is sheared downwind by the flame's tilt, `tan θ = k (u² / g H)^½` (Thomas's flame-tilt correlation), and stretched along it; smoke bends more, since it rises slower. Gusts move the shear on the plume's clock, so tongues stream rather than the whole body swinging.
- **Rain dims and puts out**: an open fire's heat falls by the rain's rate over what its fuel gives (a preset's `rain_hardiness`), with steam puffs over it; at heat 0 it goes out. A fire under a roof is untouched: the owner tells fire it is sheltered (the sky's view is the building's to know).
- **Snow piles near it**: fire gives each fire's heat disc; the snow layer melts within it and piles beyond, so a campfire stands in a ring of wet ground in a snowfield.
- **Night and day**: unchanged (fire.md's exposure and light rules).

## The fire bench

A place in the sandbox (`tools/sandbox/places.ts` data, D249: no new options) with every kind side by side on the yard: candle, torch, campfire, brazier, a house alight, a breath with its fireball, each from its sheet camera and one wide view of all. It is driven by the season, time and weather keys the forest-walk demo is adding (none on main on 2026-10-25); until they land, by the existing `--time` and `--date` and `cargo xtask remote` setting the `Weather` resource, never a second set of keys. A change to the shared flame is judged on the bench across every kind at once.

## The measure

- **The fire sheet** (D433) gains rows for the torch and candle beside the campfire, house blaze, breath and fireball, every row on its matte, at noon, dusk and night. A shared change lands if it moves more rows into class across kinds than out.
- **Motion strips** for every kind (`fire/motion/tools`, today the campfire's): eight frames 0.25 s apart a kind and hour, judged one build a judge against fixed anchors (D472, `tools/review`), 16 fresh judges a build; a change wins when its mean beats main's with the 95% interval above nought, by a rule written before the panel. The strips cannot see motion (D472, round g17), so a change whose aim is motion passes on motion.ts's targets against the filmed campfires, the sheet twice, and campfire and torch panels not worse.
- **The bench's cost**: `cargo xtask bench` on the 3080 for one near campfire (0.3 ms budget) and the burning village (1.5 ms), fire.md's budgets.

## The plan

1. Done: the field as the campfire's body (rounds g1-g4); it beat the cards in motion and holds the sheet. A kind chooses it by its `Kindled`'s `body: Body::Marched` (`fire/hearth.rs`), the shader's `hearth_marched`; until `fire.ron`, that field is the preset.
2. The torch (done, g6) and house blaze on it, one kind a round, each measured on the sheet and a panel; the hearth's card tricks removed as each goes.
3. `fire.ron` and the `terra_fire` crate: kinds as data, the field's sources and the wind and rain rules, tested.
4. Weather: wind shear, rain dimming and going out, heat discs for snow, shown on the bench.
5. The bench and the torch and candle sheet rows.

## Proposed: the flame's motion from a baked simulation (D493)

Status: Leaning (D493), waiting on the developers' answer on the review desk (item `r-fire-sim`); no kind plays the loop before it. The baker's step is built (`node tools/fire/bake/bake.ts campfire-sim`, `assets/textures/fire/campfire-sim.dds`, fire round g24), and so is the game's body (`Body::Baked`, `fire/sim.rs`, fire round g25), which no kind chooses yet; the colour comes from the flame, not the temperature, which stands at 0.82-0.97 wherever the flame is over 0.1, and the shear of a wind stronger than the bake's is not added yet. It is the hybrid of the table above, which D466 left as the upgrade path if a measured gap asked for one.

**The gap.** Five rounds (g17-g21) tuned the field's lobe clocks, pinch and tear; its tongues still split 1.65 times a second or more against the filmed campfires' 1.2 at most, and live 0.16 s at dusk against 0.04-0.12 (research/2026-10-25-fire-motion.md). Lobes rising past each other read as forks; noise has no way to tear one body into tongues. A Mantaflow campfire, filmed in the same view (research/2026-10-25-fire-fluid-sim.md), round g23's F (96 cells, vorticity 0.8, flame vorticity 1.5, a gusting side wind of 1.8 m/s mean, the clock in real seconds), meets eight of motion.ts's nine targets: flicker 2.50 Hz, area sd/mean 0.19, 13.1 tongues born a second, life 0.05 s, splits 0.85, lean 9.5°, r at no lag 0.05, top red 11%. It misses sheds (0.27 against 1.0 or more; see the open item). Shipped games take fire's motion from offline simulations the same way.

**What changes.** A baked kind's field reads its shape and motion from a looping 3D volume in place of its lobes and noise (`field_gas`). The marcher, its box, its steps, the emission and absorption, the threshold and feather that make the edge a torn sheet, the film mask and the depth it writes are unchanged. The volume holds two channels a cell: the sim's flame (the field's density, cut by the existing threshold; g22 found the edge as flame³ matters as much as the sim) and its temperature (the ramp's heat, in place of depth and height). Velocity is not stored: the field needs none to draw, and wind is not replayed from it. Frames are blended linearly in time; if a 60 fps film shows the blend doubling tongues, the bake goes to 60 fps (twice the memory) before motion vectors, which made our one-sim flipbook worse than none (D466's table). A kind chooses it as it chose the field, by its body (`Body` in `fire/hearth.rs`).

**What stays.** The light's flicker (`light.rs`, the sum of sines round the puff rate), the shared colour ramp and its rows, each kind's settings (size, brightness, ramp row, LOD distances; its rise speed becomes its playback speed), sparks, embers, smoke and the flare, and every gate below. Today the field's puffs and the light share one clock; a baked flame and the light would not, so the baker also writes the loop's light a frame, and the light follows it only if a film shows the two out of step.

**The loop.**
- **Seamless** by the baker's existing offline crossfade (`tools/fire/bake/sets`' `crossfade`): the sim runs a second past the loop's end and that second is blended into its start. One fetch a step at play time, and soft for under a second in eight. Two copies half a loop apart would cost two fetches and soften every frame, as two uncorrelated flames averaged lose their torn edges.
- **8 s at 30 fps** (240 frames): long enough to hold several of the bake's gusts (periods 2.7-11 s) so one fire watched does not visibly repeat; the puffs (2.2 Hz) and tongues (0.05 s) need 30 fps at least.
- **At the sim's own 2.3 cm cells**, in the box the flame reaches: 40 x 36 x 52 for the campfire, 0.92 x 0.82 x 1.19 m, its gusts pushing it along the wind (a 32-wide box cut flame over 0.3 in 11 of 480 frames). Finer cells hold no more detail (the sim has none; g22's 160-cell bake was calmer, not sharper); the edge's sharpness is the threshold's.
- **Memory**: 73 KB a frame as BC5 (two BC4 channels, half a byte a texel each), 17.1 MiB for the loop; 34 MiB uncompressed. Block-compressed 3D textures need wgpu's `TEXTURE_COMPRESSION_BC_SLICED_3D` (the pinned Bevy enables it, D493); without it, the BC5 decoded at load to Rg8, so the repository carries one file.
- **Shared by every fire of that bake**, loaded once. Each fire plays it at its own time offset (seeded, any of 240 frames), mirrored or not across the wind's plane, turned so its baked wind lies downwind (see wind), scaled ±15% with its speed as the root of its scale, as a fire's puffing goes (about 1.5/√D Hz).
- **Variants a player tells apart (D182)**: one per bake. Offsets, mirroring and scale keep neighbouring fires from moving in step, so a village never reads as copies, but a player would not call two of them different fires. A different fuel layout or stage is a different bake: for the campfire, at most three (a teepee, logs laid flat in a star, a low fire over embers), told apart by height, width and how many tongues stand; about 15 MB each.

**Other kinds.**
- **Torch**: its own bake, a single head about 0.15 m across, puffing near 4 Hz, a box about 0.25 x 0.25 x 0.6 m (under 5 MB). It is carried, so its swing is wind on top (below). It keeps the procedural field until its bake passes its own gates.
- **Brazier**: the campfire's loop, scaled.
- **House blaze**: many sites (`Blaze`); each site plays the campfire's loop scaled up two to three times, at its own offset, overlapping, as shipped games layer flipbooks; its own large bake only if a panel asks.
- **Candle**: keeps the procedural field. A candle's flame is laminar; there is no turbulence to bake.
- **Breath and fireball**: keep their own gas (D430).

**Wind and weather.** The bake's own wind (1.8 m/s gusting, veering ±20°) is in its lean, so each fire turns its loop so the baked wind blows the game's way. Wind stronger than the bake's adds Thomas's shear (Weather and wind, above) for the difference, sampled on the same coordinates; the game's gusts move that shear, on top of the loop's own. In calm air the baked lean stays, about 10°, inside the filmed fires' 7.6-24.5°; a calm bake is the remedy if a panel finds it wrong. Rain, going out and heat shrink the field and raise its threshold as now; snow and heat discs are unchanged. The limit, known in baked fire: the loop's gusts are fixed and the fire does not react to a body passing it.

**The bake pipeline** is the fire baker, `node tools/fire/bake/bake.ts` (TypeScript, Blender headless through `tools/blender/run.sh`), given a set kind that writes the volume instead of rendering a flipbook: the recipe a JSON in `tools/fire/bake/sets/` (today the sim's cache and the box; F's settings, its wind and its fuel are g23's sim.py until the baker simulates it), the packed loop in `assets/textures/fire/` with a sidecar (size, frames, fps, cell, the loop's light a frame), a `--draft` at 48 cells in minutes. The sim's cache (1.6-1.8 GB, 31-40 min on 24 cores) stays in `~/.cache/terra-review`, never in the repository and never in `/tmp`. Against the public repository: each committed bake adds about 15 MB to its history for good (`assets/textures/fire/` is 27 MB today), so only a bake whose loop has passed the baker's seam and motion checks is committed; tries stay in `shots/`. The recipe is the source; no `art/` file is needed.

**How it is checked.**
- **Motion**: `node tools/fire/motion.ts` on the game's dusk film (clipm.ts's view, 20 s) against the nine targets, within the tracker's ±0.3; and a film across the loop's seam, whose second of crossfade measures as the rest.
- **The fire sheet** twice, held to `held.json` (D433), and D472's campfire and torch panels not worse (interval not wholly below -0.1); motion is the gate, as the strips cannot see it.
- **Cost**: `cargo xtask bench` on the 3080 for one near campfire against the 0.3 ms budget and the burning village against 1.5 ms, beside main's field. One trilinear fetch a step should cost less than seven lobes and noise; unmeasured until benched. A seam test in the baker's own tests.

**Open: sheds** (0.27 against 1.0 or more). Mantaflow does not shed, whatever the lever (research/2026-10-25-fire-fluid-sim.md). Wind that pushes only the gas carries a torn blob along with its tongue (g23). Air moving past the fire did no better, at 0.19-0.33 (g34). Finer cells with more vorticity calm it, to 0.21; Blender's noise upres powders the flame into sparkle (g35). Houdini tears flames with a solver term Mantaflow lacks (shredding); EmberGen or Houdini would be a purchase, the developers' call. Without one, the loop ships at eight of nine, or gets its sheds as shipped games do, as pieces over it (below).

**Sketch, not built: shed wisps over the loop.** Shipped real-time fire is built from short-lived particles, each a flame card that spawns at the fuel, rises and fades (Epic's Niagara fire: a SubUV flipbook on velocity-aligned sprites), and artists add pieces over a baked flame where it lacks them ("secondary flame licks", Afonseca's EmberGen campfire). A shed is such a piece: it leaves a tongue's tip, rises faster than the tongue and is gone in a tenth of a second or two. Offline solvers make them by tearing the flame (Houdini's shredding, from the heat field's gradient); our Mantaflow bakes do not (g23, g34, g35). So the loop would carry its own wisps, drawn by the same marcher, if the developers want sheds without a new solver:
- **What**: up to four wisps a fire, each a small soft ellipsoid of flame (about 4 cm across, a tongue's least area in motion.ts) added to the baked flame in `field_gas` (the greater of the two), so the threshold, feather, ramp and depth that draw the body draw it too. Not cards: ours read as haze beside the marched body (D466).
- **Where and when**: the baker writes, for each loop frame, up to three tongue tips (the flame's local tops above three quarters of its height) and the gas's velocity there into the sidecar; no read-back at play time. A fire spawns a wisp at a tip at the target rate, about one per tongue a second, seeded per fire.
- **How it moves**: from the tip at the baked gas velocity (about 3.3 m/s up, the loop's wind in it) plus the game's wind beyond the bake's; the tongue it left falls back, so it comes free. It shrinks and its flame falls through the threshold in 0.04-0.12 s, a filmed tongue's life. The marcher's box grows upward by the farthest a wisp rises, about 0.4 m (3.3 m/s for 0.12 s).
- **What it costs**: four ellipsoids a step in the march, uniforms a fire; benched with the body against the near campfire's 0.3 ms.
- **Checked** as the loop is (motion's nine targets on the game's film, sheds 1.0 or more with the other eight held, the fire sheet, D472's panels, the bench). The light's flicker ignores them.

**Rejected.**
- **Tuning the noise field further**: five rounds, splits stuck at 1.65 or more and dusk life at 0.16; the fault is the method.
- **A live grid** (32 x 32 x 64, about 2.7 MB): about 0.3-0.5 ms a fire on the 3080 (estimate, about 50 dispatches a step), so ten fires cost 3-5 ms against the village's 1.5 ms; its gain, fire reacting to wind and the player, is what Epic keeps 3D grids for in hero effects and cinematics.
- **A better flipbook on cards**: a card is flat from above, and ours read as haze and failed our gate (D466).
