# Fire

Status: Leaning (D415), the agents' technical design. The design questions are marked Open and listed at the end for the developers. The findings behind it are in [research/2026-10-19-how-games-draw-fire.md](../research/2026-10-19-how-games-draw-fire.md). A step back after about 30 rounds, with a professional flipbook put through our path and a ranked plan, is in [research/2026-10-19-fire-step-back.md](../research/2026-10-19-fire-step-back.md).

## Intent

The developer, 2026-10-18: "We are going to focus on CRUSHING the fire module, we wanna do it properly, well, and in the way that would make an AAA developer proud. we do it properly, and once, every time this entire simulation needs fire ever again, we have it properly done!"

And: "BIG theme here is whether we can capture the rules in some static systems/modules that we could reuse so we don't have to always fix those problems." Earlier: "idk how to tell you how to do fire, i gave you so much feedback and it all looks like fucking joke all the time."

So:
- **One Fire module draws every fire in the game**: a candle, a torch, a campfire, a brazier, a house, a castle, a forest, a dragon's breath, an explosion.
- **The rules shipped games follow are written into it once.** A new fire is a preset of data (D136), never new shader code or a hand lift.
- **A machine check measures every fire against photographs of real fire**, so a bad one fails before a person sees it. The look aimed at is lightly stylised (D423): AAA game frames are the target and win where their classes differ from the photographs', which stay a physical sanity check.

What went wrong in twelve rounds, from the research:
- the flame's emission and cover came from one luma channel;
- the tonemapper turned orange into peach;
- the fire was fully exempt from exposure;
- the lights had no shadows and were not in the fog;
- the smoke was unlit or missing;
- the breath was one smooth volume.

Each is a rule below.

## The parts

Who does what, by layer (D198):

| Part | Where | What it does |
|---|---|---|
| What burns | the sim of whatever burns: `terra_build::fire` for cubes (D377); a wildfire sim for plants (Open); gameplay for torches, campfires and spells | Says where fire is, how hot, how much fuel, and how far each burnable has charred. Engine-free (D28). |
| **Fire** (`terra_world::fire`) | the world layer, a plugin of submodules: `preset`, `ramp`, `flame`, `smoke`, `sparks`, `light`, `haze`, `fog`, `lod`, `breath`, `burst`, `check` | Draws fires from presets. Owns its buffers, materials and lights. Reads the eye's adaptation (`adaptation`), the wind and the clock. |
| Burnables' looks | the materials of what burns (building's scorch, D377; plants; props) | Fresh, alight and charred, driven by the char level the sim says and the fire's flicker. |
| The baker | `tools/fire/` (new: TypeScript driving Blender's Mantaflow and Cycles headless) | Bakes flipbooks in the channel format below and checks them. |
| The check | `tools/fire/measure.ts`, behind a new xtask task, `fire` (stage 0) | Measures stills of every preset against the real-fire references. Gates a push that touches fire. |

The layer above hands Fire **fires**. A fire is a kind (a preset's name), a place (a point, a segment for a breath, or a set of sites on a surface), a size, a heat from 0 to 1, a fuel left and an age. Fire works out everything else from the preset. As today, every fire of a layer is a slot in one GPU buffer, so all flames are one draw, all smoke one, and all sparks one, however many burn (D127).

## The kinds

Every kind is a preset in `assets/data/fire.ron`. The layers each kind uses:

| Kind | Fuel glow | Body | Licks | Rod | Smoke | Sparks | Flare | Haze | Light | Ground |
|---|---|---|---|---|---|---|---|---|---|---|
| candle | wick | one small card | - | - | thin wisp | - | soft | - | 1, unshadowed | - |
| torch | head | 1-2 | 2 | - | thin plume | few | yes | small | 1, shadowed when held | - |
| campfire | logs, char | 2-3 | 4-6 | - | plume, lit | many | yes | yes | 1, shadowed | scorch ring |
| brazier | coals | 2 | 3-4 | - | plume | some | yes | yes | 1 | - |
| structure fire | char on cubes | a set per site | per site | - | a column per building, lit, shadowing | cinders that spread | per cluster | per cluster | 1 per cluster | falling brands |
| wildfire (Open) | char on plants | sites along a front | per site | - | a wall of columns | cinders | - | per cluster | 1 per stretch of front | burnt ground |
| breath | - | flame sprites along the stream | yes | a hot rod at the jaw | soot off the edges and the end | embers, burning drops | at the mouth | along the stream | 1 moving at the front, 1 at the hit | blast, lingering fire, scorch |
| explosion and fireball | - | a one-shot fireball | - | - | a soot ball, then a column | debris sparks | a flash | a shock ring | a 0.1-0.3 s flash light, then the fire's | lingering fire, scorch |
| embers alone | glowing bed | - | - | - | wisps | rising sparks | - | - | 1, dim | - |

## The layers

Each layer is a renderer with its own clock: it starts, stops, loops and hides on its own, as The Witcher 3's effect tracks do.

1. **Fuel glow.** The burnable's own material lit from inside: cracks in char, coals, a torch head. It pulses with the fire's flicker. It is a material state, not a particle (see Burnables). A campfire's `Fire` part is its coals and burning faces, cut from its logs (156 of its 237 vertices are the logs'), not a flame: it glows with the fire (`props::Embers`), and only a `Fire` part with no map gives way to the drawn flame (fire round 10). Its coal bed is its model's too, added after refine by [tools/props/coals.ts](../tools/props/coals.ts) (material `Coals`, so it flickers as its `Embers`): a low ash mound inside the logs and 48 rounded coals heaped on it, highest under the apex, each a smooth lump of 80 triangles (its small ones were 20-sided and read as boxy dice, fire round 25: alone against main, blind, 6/10 against 5/10), dark crust with soft glowing seams, glowing undersides and grey ash on their tops and between, in 4,002 triangles. Their heat follows the heap: the densest middle runs hottest, past 1,440 K toward yellow, the rim's coals cool to red under ash, and a grey-white ash skirt, broken at its edge, rings the bed (fire round 19); every glow stays under the tonemapper's shoulder. So a hearth draws no coal card, only a torch (its flat salmon slab was the bed, fire round 17). Its glow follows the camera's exposure physically (`EMBER_WEIGHT` 0), so by day it is a seventh of its night's and the logs read charred, not red-spotted; its map's cracks are fine threads, dull red to orange-yellow at the hottest, the coal bed under the tonemapper's shoulder so it stays orange, not pink (fire round 11). Its char runs along the grain, blocks three times longer along each log than across with fine streaks along it, not round spots, and its cracks glow a quarter as much on faces turned up, where by night they read as orange speckle on the log tops (fire round 15: alone against main, blind, 5/10 against 4/10). The hero campfire's logs ([tools/props/hearth.ts](../tools/props/hearth.ts)) char over every underside, inner top and foot, and are scorched and charred in patches over their upper outer faces too, which the camera sees (fire round 25: their raw split faces read as pale planks; with the rounder coals, blind, 6/10 against 5/10), in fissures along the grain with staggered cross-checks and a ragged edge, their cracks glowing in its `Wood` emissive map; split faces weathered grey-brown, bark in irregular plates (fire round 21: alone against main, blind, 5/10 against 3.5/10). Its ring stones differ in size, outline, turn and tone, grey-brown from blue-grey granite to brown sandstone, sooted toward the fire and smoke-stained on top (fire round 25: alone against main, blind, 6/10 against 5/10, main's "same-sized pale beige ovals, evenly spaced like a row of bread rolls").
2. **Body.** Large, slow, looping flipbook cards on the fuel. They are camera-facing about the vertical, leaning into the wind and the fire's motion. Playback rate follows the fire's size: big fires roll slower.
3. **Licks.** Smaller, faster flipbook cards at the body's edges and top. They break off and rise, each with its own age, so tongues alternate with dark at every scale (MEASURED observation 6).
   - A hearth's last card is a soft additive glow round its flames' core, 1.2% of their brightness at its middle (fire round 23, 5.5/10 against 5; at 5% a ball of orange light).
   - A hearth (fire round 12, research/2026-10-19-fire-step-back.md): its body stands about as high again over its logs as they are (`HEARTH_BODY` 0.95 of `tall`, its foot a little up among them; at 0.8 it read short, about as high as its logs), its cover eaten away toward its tips by a threshold rising with height through a climbing noise (alpha erosion, Unity's BonFire and Unreal's fire materials), its upper part let through and its foot hotter. Its body is four overlapping columns round the stack (`HEARTH_COLUMNS`), each a torn slice of the baked frame at its own moment, the middle one tallest, the others out among the logs and lower, so dark shows between them; their erosion has a floor and a soft feather (0.2; the tongues' 0.14, the columns' sides thinning from a quarter in), so the crown tears into wisps with no orange halo (fire round 13, blind review 6/10 against 3) and its edges are wispy where they meet the dark, not cut out (fire round 16, 6/10 against 5 at 0.07). Nine tongues (`hearth_tongue`), each a slice of the campfire's baked set inside a teardrop (as a procedural teardrop alone, a smooth pale petal: fire round 23, 5/10 against 4), white-hot at the root and deep red at the tip, rise from the logs' tops where they stand, on their slopes and off the apex the middle column holds (`TONGUE_IN`), leaning in along them, each living 0.5-1 s: a swaying teardrop whose tip erodes first, hot at its foot and orange up it, born again elsewhere when it has pinched off. Its cover is a third of its drawn shape (`TONGUE_THICK`), as thick as the baked body's flame, since both are lit at the hearth's brightness: drawn opaque and stacked on the apex, the tongues were the white spike in the flame's middle (fire round 16: gone with the tongues hidden, kept with the columns hidden; blind review 5/10 against 4).
4. **Rod.** For jets only: a hot, opaque core mesh or ribbon with panning noise, Monster Hunter World's TubeLight. The breath's raymarch survives only as this thin rod near the jaw, if it earns its place.
5. **Smoke.** Alpha-blended cards that are six-way lit, backlit and dark. They pale as they climb and thin, and cast shadows. It extends today's smoke (D325) with the format below.
6. **Sparks and embers.** GPU points, velocity-stretched and curl-noise driven, riding the wind with their own influence (God of War). Their colour comes by age through the ramp, and their glow from the fire's flicker.
7. **Flare.** One soft additive glow sprite at the hottest point, faded by occlusion and distance. It stands in for the camera's glare that bloom alone does not give.
8. **Heat haze.** A full-screen pass bending the HDR picture in a column over each near fire (see Heat haze).
9. **Light.** Point lights by the light rules.
10. **Fog.** A fog volume round the fire: the glow of the air and the absorption of its smoke, lit by the fire's light (see Light).
11. **Ground.** A scorch decal, and lingering fire sites where a breath or burst lands on something that burns (not water).

## The flipbook format

Two textures a flipbook set, with motion vectors for every frame, cooked without mips by `cargo xtask cook` (D188): the flame map BC7, the motion BC5.

- **Flame map**, per texel, 0 to 1 each:
  - **R heat**: the index into the ramp, the fire's temperature as the baker renders it.
  - **G cover**: how much the flame hides what is behind it. It is the sim's density or flame mask, never derived from heat.
  - **B soot**: dark matter inside the flame (pockets, char, smoke wisps that occlude).
  - **A distortion mask**: how much this texel bends what is behind it. The tips move, the base stays (Back 4 Blood).
- **Smoke maps**, Unity's six-way layout:
  - map 0 RGB is lit from right, top and back, with density in its A;
  - map 1 RGB is lit from left, bottom and front, with the emission from the fire inside in its A.
- **Motion vectors**: RG8 at half the colour resolution, sampled unfiltered (SideFX Labs' advice), one per frame to the next. The shader blends frames along the flow, as `flames.ts`'s do today.
- **Sets**: each kind names its sets. A set is a loop (body, licks, smoke), a one-shot (ignition, burst, fireball, die-down) or a strip (the breath's stream).
  - Hero sets are 8x8 frames of 256 texels.
  - Far sets are 8x8 frames of 128.
  - Sets are shared between kinds: a campfire and a house fire use the same licks.
  - How many sets in all: about 12 to start. Kinds differ mostly by preset, not by texture.
- **Why this format:** emission and cover are separate quantities in every shipped pipeline (Battlefront II, Back 4 Blood, Monster Hunter World, film compositing). Tying them is the root of our lime, khaki and pastel rounds.

## The baker

We bake our own flipbooks with free tools, and never pay for a fire library or tool. The developer: "we are in the era of LLMs, and i do want to be able to beat it without having to pay for fire library."

Flipbooks are simulated, not filmed. Footage has no heat or cover of its own and cannot be relit. It stays only for far, additive fires, if at all.

**Leaning: Blender's Mantaflow fire and smoke sim, rendered in Cycles, as an offline baker.** It runs headless through `tools/blender` (Blender 5.2).

*Why Mantaflow over a sim of our own:*
- It simulates what a pyro solver must: combustion of fuel into flame, temperature, smoke density and velocity, with buoyancy, vorticity, noise upres and dissolve.
- **Cycles renders the volume it makes.** It gives true self-shadowed six-way lighting and an emission pass from the same voxels.
- Extending `flames.ts` into a 3D pyro sim would also need a volume renderer for the six-way maps. That is two hard things to own instead of none.
- Blender is a program we call, so its GPL does not reach our repository. Its renders are ours.
- The scripts are our own Python, run inside Blender (D121). We never copy Blender's or anyone's GPL code, or Unity's six-way add-on, into the repository; we write the six-way render ourselves from its published idea.

*What a bake is.* `node tools/fire/bake/bake.ts <set>... | --all [--draft] [--pack]` reads a set's recipe in `tools/fire/bake/sets/<set>.json`: the domain, fuel sources, sim settings, length, loop and camera. It runs `tools/fire/bake/bake.py` in Blender, which simulates and renders each frame three times on the CPU (so a bake takes no GPU memory), each an emission-and-absorption volume with no scattering, ray marched (`volume_biased`), so a pixel is an exact line integral through the grids:
- **heat**: the flame's emission-weighted mean flame, seen through flame and smoke;
- **motion**: the velocity in m/s (Mantaflow's is cells a second), weighted the same, projected onto the camera. It gives exact motion vectors, with no optical-flow guessing;
- **depth**: the flame's and the smoke outside it's optical depths, so cover and soot are `1 - e^-depth`.

A smoke set (`"six_way": true`, `campfire-smoke` and `house-smoke`) is one puff, a one-shot, rendered three times a frame. Single scatter is linear in light, so three suns of three colours light three sides in one render (the idea is Unity VFXToolbox's, the code ours):

- **light0**: lit red from the right, green from the top, blue from behind; **light1** the opposite three;
- **glow**: emits the smoke's heat and its density and absorbs its density, so G is exactly its alpha and R/G its mean heat.

`smoke.ts` packs them as the smoke maps above, each the light over alpha (straight, for straight blending), times a `light_scale` in the sidecar. Its unit is a white Lambert surface facing the light; thin smoke lit from one side is about a quarter. Its line says the mean response from each side; `maps.png` shows the six maps, alpha and glow of four frames. Smoke has no motion vectors: its frames crossfade.

`pack.ts` then:
- scales heat so the set's 99th percentile is 0.97;
- crossfades a loop's last frames into its first, premultiplied;
- writes `assets/textures/fire/<set>.png` (the flame map), `<set>.motion.png` (half size) and `<set>.json` (the sidecar the engine reads: grid, rate, motion scale).

PNG in the repository, a set about 4.3 MB (2048² flame map 3.5 MB, motion 0.8 MB); the game reads the cooked copies where they are there. Far sets are the hero's mips, not files of their own.

It prints one line per set:
- frames;
- how much of the frame the flame covers;
- the heat by quarter;
- the soot share;
- the loop's seam against an ordinary step;
- how high the fire reaches;
- the flow's check: the warp error with the vectors against none, and the best scale against the exact one.

`sheet.ts` draws each set's raw passes as a contact sheet beside the heat-to-ramp preview (`tools/fire/ramp.ts`, blackbody), and measures eight frames with `measure.ts` against the set's class, over the background most of its photographs have, at the exposure that fits best, through Hable and KhronosPbrNeutral.

A bake takes 10-20 minutes on the CPU and is run when a recipe changes, not by the gates. Its outputs are committed.

**If Mantaflow cannot reach the references** (too coarse, or too slow at the needed resolution), the fallback is our own offline pyro sim in TypeScript, `flames.ts` grown into one. It would add combustion, temperature, density and velocity on a 3D grid, and a simple volume renderer for the same passes. That is a step back to take only with a measured reason (stage 3).

`tools/textures/flames.ts`'s footage is retired once the first baked set passes the check.

## The colour ramp

One ramp texture for every fire in the game, after Skull and Bones: `assets/textures/fire/ramp.png`, 256 wide, one row per fuel.
- The rows are wood, oil and pitch, gas, and a spare row each realm may add (D14: a realm changes materials and light, never the rules).
- A row's RGB is the emission at that heat, in linear HDR relative units.
- Its A is how far that heat covers what is behind it rather than adds to it: 1 from the orange up, 0.4 at the dull red rims (2026-10-19). A flame is alpha-blended, as Unity's BonFire and NVIDIA's Vulcan demo draw theirs: added over the blue-grey world, its hot end took that world's blue (the pro flipbook's own B/R 0.03-0.10 came out 0.64; research/2026-10-19-fire-step-back.md).
- Its hottest has the blue a camera's blown core takes, a quarter of red (linear), rising as the fourth power of green's exposure so the orange keeps almost none (`BLOWN_BLUE`): the night photographs' core is 253,224,133, where a 1,670 K blackbody has no blue in sRGB.
- It is drawn premultiplied (`ONE, ONE_MINUS_SRC_ALPHA`):
  - `rgb = ramp(heat).rgb × cover × brightness + soot_colour × soot`;
  - `a = cover × ramp(heat).a + soot`.
- It starts from blackbody glow (D377's 950-1670 K) and is then tuned once, by machine, against the reference bands (the check below). It is never tuned per fire.
- **Saturated as it dims.** Below the knee a row's chroma does not fall with its brightness (Back 4 Blood's squared tint), so a thin flame goes deep red, never grey.
- It is generated by `tools/fire/ramp.ts` from a few keys, so it is data with a source, not a painted file. `node tools/fire/ramp.ts` writes it: 8-bit RGB over a scale (the brightest channel, as a blackbody's red passes 1) kept in `ramp.json`, sRGB-encoded so the dim red end keeps its precision.
- **Filmed, not raw** (2026-10-19): a row is the blackbody as a photograph records it, each channel 1 - e^(-film x light), so a brightening flame fills its red first and its green catches up: red, orange, yellow, its hottest near white (`film` 4.5 for wood). The photographs it is measured against are of fire so exposed; KhronosPbrNeutral keeps hue, so the raw blackbody (950-1670 K, deep red) drew the baked sets salmon pink and 90-99% red. On the baker's sheet under Neutral the tongue then holds 15/15, the torch 14/15, the campfire 12/15 at x6.
- **A card stands by its set's reach**: each sidecar says how high up its frame the flame's tips reach (`reach`), and a hearth's or blaze's card stands `tall / reach`, so a fire's `tall` is how high its flames lick (a campfire's 1.3 m, over its metre of logs); a hearth's body stands at 0.95 of it, its tallest tips reaching it only now and then (fire round 12).

## The exposure rule

A preset gives a fire's luminance in absolute units (a candle about 10^4 cd/m², a bonfire's body about 10^5). The flame is drawn at

`drawn = L × (E / E_ref)^w`

- `E` is the eye's adaptation this frame (`adaptation`, a frame late, as Unreal's EyeAdaptation).
- `E_ref` is the adaptation of a dusk scene.
- `w` is the **exposure weight**, one number per realm:
  - w = 0 is physical (Frostbite): the fire is weak by day.
  - w = 1 is the screen's scale, ours today: the fire is the same at noon and night, and reads as pasted on.
  - **Leaning: w = 0.4**, to be measured.
  - It is data: `FireExposure` (crates/terra_world/src/fire/exposure.rs), a resource a realm sets, its weight and the dusk exposure (EV100 9) a fire is drawn as tuned at. Absent, the weight is 1, today's look. The shader's `on_screen` applies it to everything drawn on the screen's scale (flames, walls, tongues, breaths); embers, coals and drops (`soot`) are already physical.
  - The rule's own check is that the drawn fire's noon-to-night ratio is about 3 (The Witcher 3's 15x by day against 5x at night, against an exposure swing of many stops).
  - **How much a fire should dominate at dawn against noon is Open for the developers** (art).
  - Meanwhile a campfire's flames go pale by day (fire round 20): from 1 to 3.5 stops under dusk's exposure its flames' (not its coals' or ground's) light washes 40% toward a pale yellow of its own luminance and halves, and they cover 30% of what they did, so at noon they read pale and see-through, as in daylight footage, and by night as before. A working look for that Open question, not its answer.
- **Lights stay physical** (weight 0). By day a fire lights almost nothing, and at dusk it lights everything near it (MEASURED observation 10).
- Bloom comes only where the drawn fire saturates the curve. There is no fire-only bloom.

## The tonemapper

The camera's tonemapper is chosen by measurement, for the whole camera. Fire is exposed and graded with the world (Battlefield 3's lesson: particles lit and tonemapped apart are "the wrong color" against it), and only a hearth's or torch's flame past the white takes a film's shoulder of its own (2026-10-19, `fire::film`):

- **Why**: KhronosPbrNeutral mixes whatever is past its white toward white in every channel alike, so a flame with no blue (the ramp's hottest is B/R 0) gains blue there: a hot campfire texel came out 254,233,208 (B/R 0.64), salmon. The grade's `post_saturation` 0.82 after it floors blue at a fifth of brightness. Night photographs of campfires go yellow at their brightest (254,233,141; B/R 0.03-0.15).
- **The mask**: the hearth flames are drawn a second time (`FIRE_MASK`), their colour untouched, adding `min(1, red)` to the HDR picture's alpha past the 1 the scene holds. TAA, bloom and the volumetric fog keep the alpha, and smoke blended over a flame takes its share back. Bevy's depth of field wrote an alpha of 1, so a film shot with an aperture lost the mask and its campfire read milky pink; its passes are patched to keep the alpha they are given (`film.rs`, D184).
- **The shoulder**: in Bevy's `tone_mapping` (patched, D184; the tonemapping pass only, not a material tonemapped in its own shader), the masked share of a pixel goes through the Hable curve (`hable.rs`) per channel, with 2% of each channel caught by the other two as a film's layers catch each other's (AgX's inset does the same). So red runs out first, then green, then blue: orange, yellow, white. That share skips `post_saturation`. Below about 0.7 Hable and Neutral nearly agree for a colour with no blue, so a flame's dim edge and its mask's soft edge meet the world without a seam.
- **Not the breath or a blaze**: their sprites' brightness is their heat and they are tuned through Neutral; they keep it until measured otherwise.

- **Candidates**:
  - Bevy 0.20's own: AcesFitted (per-channel, hue-shifting, the nearest Bevy has to Hable), AgX, TonyMcMapface, BlenderFilmic and KhronosPbrNeutral (today's).
  - A Hable (Uncharted 2) curve, which Bevy does not have, ported from Godot's Filmic tonemapper (MIT, D196): `crates/terra_world/src/hable.rs` swaps Bevy's unused Reinhard for it (`shader_patch`), so a camera given `hable::HABLE` draws with it. Its white is Hable's own 11.2 (5.6 before Godot's doubling).
- **Test**:
  - Render the fire sheet (below) through each.
  - Measure each preset's bands against its class.
  - Measure the existing scene references (`tools/sandbox/looks/`: the forest by day, the lake at dusk) through each, so a curve that fixes fire but breaks the rest is seen.
  - Keep the candidate that brings the yellow and white cores back within range without moving a scene look out of its own.
- **Prediction:** a per-channel curve (Hable or AcesFitted) wins on fire, and AgX is close to Neutral. The test decides.
- **Decided by the developers (D423): the camera keeps KhronosPbrNeutral.** Fire is fixed at its source; the fire-only mask stays allowed. The bake-off above is history.

## The light rules

- **One light per cluster.** Fires within a few metres merge into one light at their heat-weighted centre, its power the sum of theirs. A burning village is a dozen lights, not hundreds (D127).
- **A wood flame's colour.** A hearth's light is wood flame's orange as the games white-balance it, about 2,300 K, its blue under the blackbody's, and 2 stops brighter by night: at 1,900 K and 3 stops the lit stones passed the tonemapper's shoulder and read salmon pink (sRGB 232,126,88); now warm brown (192,106,61, fire round 13). At that light the pool, stones and ash still read pink-salmon and lavender, so it is (1.0, 0.66, 0.16), and the stones are mid-albedo warm grey-brown, sooted black on their inner faces and over their tops from the fire's side, the ash skirt a darker warm grey, soft and patchy at its edge; alone against main in a blind review, 5/10 against 3.5/10 (fire round 21). Its light is a disc as wide as its flames, 50,000 lm a metre of flame reaching 7 m, and three tenths of it a shadowless fill hung high over the fire (1.6 times its flames' height, 1.5 times its share): the stones are lit inside the ring and shadow the ground outside it, and the pool falls to dark within 4-5 m. A disc three times as wide with half its light a fill four times its share left the fall-off level to some 4 m, an even tint and no pool; alone against main in a blind review, 5/10 against 4/10 (fire round 19).
- **One flicker per fire.**
  - The signal is a product of three incommensurate sines (Valheim's 1, 0.564 and 0.758) plus a low noise, with a random phase per fire.
  - Its depth and rate come from the preset:
    - a torch: depth 0.3-0.4 over 0.3-0.4 s (The Witcher 3);
    - a campfire: about ±15%;
    - a house fire: slower and shallower, since a big fire puffs slowly.
  - **The same signal drives** the light's power, the fuel glow, the sparks' glow and the flame's brightness (Sons of the Forest's linked flickers). Nothing on a fire pulses on its own.
- **Jitter.**
  - The light's position moves on all three axes by the same signal: about 5 mm for a candle or torch, 0.1 m for a campfire, scaled with the flame's size.
  - Its `radius` is the flame's half-width, so the shadows soften as a real flame's do.
  - Moving shadows are the strongest cue that a flame is alive.
- **Shadows.**
  - The nearest two fire lights within 20 m cast shadows; four on the high setting.
  - A held torch always does.
  - Bevy's point light has no shadow strength. A light's shadows switch on and off with hysteresis while its power eases. Porting a shadow-fade (D184) is a later stage, if the switch shows.
- **By night.** A fire's light is 2.5 stops brighter by full night than by day (`Sky::night`), as The Witcher 3 keys its fire to the hour: our night is drawn some stops brighter than a real one (a 700 lux moon at EV 9), so a fire as bright as by day lit the ground 3 m out about as much as the moon and stood grey; as bright by day as by night, sunlit stone went salmon. The night grade fades only the dim (D302), so the ground a fire lights keeps its orange.
- **Fog.**
  - Bevy's volumetric fog scattered a point light with no phase function (`terra_world::scatter` patches it with the directional light's Henyey-Greenstein).
  - A blaze's light stays out of the fog: in the haze its tens of millions of lumens lit a ball of air brighter than the house (fire sheet, rounds 5 and 6).
  - Each of the nearest kept fires with a shadowed light (four at most) puts a small `FogVolume` round its light (`fire::fog`): a ball of air densest at the flames, a few metres across, thickening as night comes, which of the fire lights only kept fires' shine in. It is a faint warm halo, not a ball: its air thin enough that the tonemapper keeps it orange, and a fire light's in-scatter held to its radius as a sphere light's is (Frostbite's), so there is no hot white dot at the light. The haze is mist-thick by the ground and scatters forward, so a fire light's in-scatter there is taken at 0.15 (`scatter::LOCAL_SCATTER`): at full it was the salmon ball over every campfire, past the tonemapper's white, whatever the fire's own volume held (A/B 2026-10-19: the ball stayed with the fire's volumes off and went with its light out of the fog).
  - Its smoke's absorption, darkening the air above, and Enshrouded's emissive fog volume are later stages (D184).
- **LOD.**
  - Light to 40 m, shadows to 20 m (Valheim), eased, never popped.
  - Past 40 m a fire keeps its flare and an emissive glow card, with no light.
- **Bounce** from fire into the scene is Open (technical): it waits on Terra's GI.
- **Flashing** follows a settings option for flashing lights (Valheim's accessibility setting).

## The smoke

- **Lit six ways**, from the smoke maps, by everything that lights a surface: the sun through the atmosphere and its shadows, every fire light in the cluster (held at its radius, as the fog's are), and the open sky from all six sides (`OpenSky` times the environment's brightness, the 2/3 so thin smoke gives back the sky itself). The card's frame comes from its uv's derivatives, so a turned card is lit from where its light is. A fire light under a young puff lights only its underside.
- **Backlit**: a transmission term, so smoke between the camera and the sun or the fire glows at its rim (Cyberpunk 2077's backlighting).
- **Lit from inside**: the glow channel times the fire's heat, strongest just above the flames, so the column's foot glows orange (MEASURED observation 5).
- **Dark**: its albedo is soot's, about 0.03-0.08, paling to grey only as it thins and climbs (D325's rule): a blaze's 0.03 to 0.15, a hearth's 0.14 to 0.42, never the pale 0.5 the fire's light turned salmon. A hearth's is a thin column from its flames' tips, lit warm from below by its light: born at 0.55 of its flames' height, in by 7% of its life and thinning as (1 - t)^1.5; at 0.6 of the baked puff, born at 0.7 and gone as (1 - t)^2.2 it showed by night not at all and by day only as a smudge metres up (fire round 20). By the sun and sky a hearth's smoke is wood smoke's pale blue-grey (albedo 0.5 to 0.82, its firelight still taking the grey), and by day (camera EV100 10.5 to 12.5 and up) it gives back six times what its single-scattering bake does, for the light its near-white wisp scatters again and again (Wrenninge 2013's octaves, Hillaire 2016): measured, at single scattering it gave back a quarter of the sky behind it and read as dark beads; by night as before, where the gain turned it milky lavender (fire round 21: alone against main, blind, 5/10 against 4.5/10).
- **Puffs**: each card plays frames 4 to 34 of a baked puff over its life, born a little open and done before the puff outgrows its frame; a hearth's wisp from `campfire-smoke`, a blaze's column and a breath's soot from `house-smoke`.
- **A hearth's wisp is one thread** (fire round 22): 72 puffs over a life of about five seconds, each drawn out twice as tall as wide, its bake's alpha opened out (to the power 0.6) and mixed 30% with a soft round fall-off over its card, at 0.4 of their cover, so each reaches the next ones'. The bake's alpha alone is a small ball in its card: 24 puffs read as beads with sky between them though their cards overlapped. Blind 5/10 against main's 4. It frays as it climbs (fire round 24): the soft fall-off gives way to the bake's broken alpha, at 0.32 of their cover, narrower and fading sooner, and the hearth's light, whose fill hangs in the smoke 1.6 flame heights up, reaches only its first metre; the old column was a dense tan tube lit orange to the top of the frame. Blind 5.5/10 against 5.
- **Shadowing**: dense smoke casts into the sun's shadow map as a dithered caster at its density (Leaning). A burning building's column shades the ground downwind.
- **Stable**: a smoke card's lighting eases over about a second, so dappled sun does not make it flicker (Monster Hunter World's Temporal Shadow).
- **Soft**: faded where it meets geometry (depth fade) and near the camera.

## Heat haze

- Its own full-screen pass over the HDR picture, before the tonemapper (Bevy's `FullscreenMaterial`, `fire::haze`, `fire_haze.wesl`), as Doom applies it in its tonemap pass: flames are bent with what is behind them, never greyed.
  - Up to four of the nearest kept fires, each a column over its flames projected to the screen every frame, from 0.15 of their height to twice it; the pass is removed while there is none.
  - The column bends what is behind by a rising noise, at most 0.15 of its half width (`BEND`): at 0.05 a blind judge saw nothing; 0.15 reads as heat shimmer (5/10 against 4.5, fire round 23); 0.25 read as marbled water.
  - Gone by 40 m. No depth test yet: a column draws over anything in front of the fire.
- One column per fire, never many rising cards, which read as smoke. Its fault left: coarse whorls more than fine upward ripples by day.

## Composites

- **Breath** (Monster Hunter World's Zeno Jiva, Elden Ring):
  - its main stream, drawn as games draw a flamethrower's (a beam of a few ribbons lying along the stream's line, each turned to the eye only about it and gone as the eye looks down it, panning noise flying with the gas; a card turned wholly to the eye leaned toward it and spanned only the stream's middle, a flat banded oval), half a metre wide at the jaw and over a metre at the face: opaque, white-yellow on its line at the jaw, orange-red down its length and at its torn edge, grown out of its core when the breath starts and eroded back to it when it stops (`breath_beam`). It replaced the marched rod, which read as a smooth cream tube;
  - 36 flame puffs in all (26 along the stream, 10 in the fireball), dissolved over their life by a hard-edged threshold on their cover (erosion), never faded by alpha, and coloured by heat alone through `breath_ramp` (white, yellow, orange, red, brown, black, never a dimmed yellow, which reads khaki); what is behind them fades them by erosion too, and before the struck face by their distance off its plane, so the wall's top never shows through as a level line;
  - its sprites write their depth (`FireExtension::depth`), what is eroded away discarded: left out of it, the volumetric fog, which runs after the transparent pass, marched each ray on to the wall or the sky behind the flame, a level band across it at the wall's top and the horizon;
  - flame sprites along the stream, each with its own age and size, growing and cooling with distance;
  - soot only above the hit: 10 puffs a second off where it strikes, warm brown-black, and a landed puff's own soot once it has burnt a while on the face; a puff cools through red and brown to black as it fades and as soot takes it, never a thinned yellow, which reads olive;
  - white-yellow and opaque where it strikes, as every reference is;
  - embers and, if the breath is liquid, burning drops falling from its underside;
  - a light riding its front.
  - A ray cast finds where it hits. A hit spawns a blast, lingering fire sites on what burns, and a scorch.
  - The jet carries the struck face's distance and normal (`Jet::face`, `Jet::normal`), so its landed puffs spread over that face and climb it, laid half onto it, rather than flying on into the wall, and the fireball stands off it.
  - Its scorch is a soot stain laid on the struck face: a plume over the hit and a char spot at it, darkening over `STAIN_S` of the stream on it, kept to the face by depth as a decal is (`breath_stain`), streaked up the face. It grows only while the stream plays on it and stays once the jet is off or its breather gone: each jet slot is drawn from where it last burned (`kept`), until that slot breathes again; a scorch kept by the face itself is still to come.
  - Its fireball is many puffs a few times smaller than the ball, rolling out along the struck face and up it, each from the fireball set's mushrooming frames on, not one ball of seven times its size showing the set's round first frames.
  - A grid blaze's tongue cards have their level, white-hot foot and upright sides eaten back raggedly by noise: standing on a wall's top, that foot was a straight cut through the breath's flame.
  - Its light is under its flame (D415's brightness order): a jet's three lights at 4e6 lm and a wood flame's orange, where 5e7 and a pale yellow lit the ground, the wall and the dragon brighter than the fire by night.
  - **Whether a dragon breathes a gas jet or liquid fire (napalm: it falls, clings and burns on) is Open for the developers.**
- **Explosion and fireball**:
  - a flash light for 0.1-0.3 s;
  - a one-shot fireball set, premultiplied, turning from fire to smoke over its life by the ramp's A column;
  - a soot ball rising into a column;
  - debris sparks;
  - a haze ring;
  - lingering fire and a scorch where it burns.
- **Structure fire.** The cellular fire (D377) stays the truth.
  - Fire draws it as **sites**: one body, licks and haze per burning cluster, sized to its fuel and anchored about 0.1 m out along the face's normal.
  - Flames leave openings and rise a storey above the roof (MEASURED observation 8).
  - Each building gets one smoke column, and cinders spread fire by the sim's rules.
- **Wildfire** is the same sites on plants along a front. **Whether forests burn and spread in play is Open for the developers** (D122).

## Burnables

Everything that burns has three looks, named by its kind's data:
- **Fresh**: its own material.
- **Alight**: charring, with glowing cracks whose emission is the fire's flicker times the char's heat.
- **Charred**: black, cracked, unlit, then ash or gone if its fuel is spent.

The sim gives each burnable a char level and a heat. The material blends between the three looks by them. Building already blackens cubes by scorch level (D377). This adds the alight state's glow and the link to the flicker, and the same for plants and props.

## A fire as data

`assets/data/fire.ron` holds one preset per kind. A sketch:

```ron
"campfire": (
    fuel: "wood",                       // the ramp's row
    luminance: 8.0e4,                   // cd/m², before the exposure rule
    body: (set: "body-a", cards: 3, size: 0.9, rate: 1.0),
    licks: (set: "licks-a", cards: 5, size: 0.4, rate: 1.8, rise: 0.6),
    smoke: (set: "smoke-a", rate: 2.0, rise: 1.2, albedo: 0.05, shadows: false),
    sparks: (rate: 12.0, life: (1.0, 2.5), wind: 0.7),
    flare: (size: 1.4, strength: 0.4),
    haze: (size: 1.5, strength: 0.6),
    light: (power: 1600.0, range: 9.0, flicker: (depth: 0.15, period: 0.35), jitter: 0.1, shadows: Near),
    fog: (radius: 3.0, density: 0.4),
    ground: Some("scorch-ring"),
    lod: (licks: 25.0, haze: 40.0, light: 40.0, card: 120.0),
)
```

- A new kind is a new entry. No code names a kind (D136).
- How many presets differ: a few dozen kinds, each a few hundred variants by size, heat and fuel (D182). The variants a player tells apart are size, fuel colour and smoke.

## LOD and budgets

All fires of a layer share one buffer and one draw, as today. LOD is by distance and by birth rate (The Witcher 3):

| Distance | What draws |
|---|---|
| near (under 25 m) | everything |
| mid (25-40 m) | body, fewer licks, smoke, sparks at half rate, light without shadow, no haze |
| far (40-120 m) | one body card, smoke, flare and glow card, no light |
| beyond | smoke column and glow card: a burning village seen from a hill reads as its smoke |

Budgets, Leaning until measured with `cargo xtask bench` on the RTX 3080 at 1920x1080 and the integrated GPU (D185):

| | Budget |
|---|---|
| fire's GPU time, a burning village (50 sites, 10 campfires, 20 torches) | 1.5 ms |
| fire's GPU time, one campfire near | 0.3 ms |
| shadowed fire lights | 2 (4 on high) |
| unshadowed fire lights | 16 |
| volumetric fire lights | the shadowed ones |
| flipbook memory | 64 MB |

A bench scene, the burning village, holds the budget (stage 10).

**The cinematic tier.** The developer: "for cinematic shot, we may wanna cook that scene differently where the distances are a bit better, we can burn the budget there a little bit."

A film may spend more than play does. It does so through data, not special code (D249): a fourth quality preset, `cinematic`, beside low, medium and high (D239), which a film's run asks for with `--quality cinematic`. Each preset's `lod` and the light rules read it:
- **LOD distances** are about three times play's, so a film's mid and far fires keep their licks, haze and light;
- **flipbooks** are the hero sets at 512-texel frames;
- **shadowed fire lights**: eight;
- **haze** is drawn at half resolution, not a quarter;
- **sparks and licks** are at twice the birth rate;
- **smoke** lighting is per pixel, not per card.

Its budget is whatever holds the film's capture rate (30 frames a second at 1080p, Leaning). A film's camera is placed for the fire, so its distances suit it. The tier changes how much is drawn, never how it is coloured, so a fire that passes the check in play passes it in a film.

## The automatic check

Every change to fire is measured against photographs of real fire before a person looks at it.

- **The references.** `refs/fire-real` holds photographs by class: campfire, building, pool fire, gas jet, flamethrower, napalm. They stay outside the repository (their licences), with their credits.
  - Their measured numbers are committed as `tools/fire/references.json`: band medians and ranges per class, as `tools/sandbox/looks/` commits look references.
  - Classes have only 2-4 photos today. Stage 0 brings each to at least eight.
- **The metrics**, from MEASURED.md's method, ported into `tools/fire/measure.ts` with tests that know their answers:
  - the flame's band shares (white, pale, yellow, orange, red, and red plus orange);
  - B/R and G/R;
  - the dim share;
  - soot over flame, and the soot at the flame's edge;
  - dark pockets in the flame;
  - detail by octave (fine, mid, coarse) relative to the flame's height;
  - at night, how much the ground round the fire is lit, against the same view by day.
- **The fire sheet.** The xtask task `fire` (`tools/fire/sheet.ts`) runs one scripted sandbox scene an hour on the yard, noon, dusk and night, each fire from a fixed camera: today a campfire, a log cabin alight, and a dragon's breath on a stone wall with its fireball. Once presets exist, every preset in a row, against stone and against sky.
  - Each fire's crop comes from its projected bounds (`fire.seen`, `terra_world::fire::seen`), never from hand-drawn boxes.
  - It writes `shots/fire/sheet.jpg` and a table: each fire's metrics against its class's range, with ✗ where one falls out.
- **The gate.** `cargo xtask fire --gate` runs as a continuous check on main (`cargo xtask continuous`, [tools/continuous](../tools/continuous/README.md), D37), on the GPU machine whenever main moves over what the fire sheet reads; a regression is reported with the commit range it came in. It is not run in a hook or before landing: hooks are for quick checks, and a GPU render is not one (D398). What it reads: fire's code and shaders, its textures, `fire.ron`, the building fire, the tonemapping patches and the check's tools (`FIRE` in `xtask/src/scope.rs`, D398). Until a stage brings a fire into its class, the gate fails only where a fire is worse than held: a number out that `tools/fire/held.json` holds in, or a fire not seen. Each stage that brings numbers in writes the file again (`cargo xtask fire --hold`), so what it fixed stays fixed; once every fire holds, the file is empty and a fire out of its class's range fails the gate.
  - Ranges are the class's min to max, widened by a tolerance. They are not the median: real fires vary.
- **Film probes.** Any still from a film or probe run that has a fire crop beside it is measured the same way (`node tools/fire/measure.ts <still> --class building`), so the castle film's rounds are judged by the same numbers.
- **Numbers do not replace eyes.** The adversarial review against the references still judges a milestone. The check only stops the obvious failures reaching it.

## The plan

Each stage lands on main, passes the check, and shows a visible improvement on the fire sheet or the castle film, except stage 0, which is the check itself.

| Stage | What lands | How it is checked |
|---|---|---|
| 0. The check | `tools/fire/measure.ts` and its tests, `references.json` with at least eight photos a class, the xtask task `fire` with today's fires as presets | Today's fires measured. They are expected to fail on yellow and on red, soot and dim shares, and the table says where. |
| 1. Curve and exposure | the tonemapper bake-off (Bevy's five and a Hable port), then the exposure weight | Bake-off table of bands per candidate and scene looks per candidate. The developers see the sheet before a camera-wide change. |
| 2. Light | clustered fire lights; flicker with three-axis jitter; shadows near; brighter by night; a fog volume per kept fire, blazes out of the fog; the flicker linked to fuel glow and sparks | The night sheet's lit-ground metric; a film of a campfire's swimming shadows; bench. |
| 3. Channels and ramp | the flame map format, `ramp.ts` and the ramp, premultiplied drawing; `tools/fire/bake/` and the first Mantaflow sets (body, licks) | Band shares within range for campfire and building. |
| 4. Smoke | the six-way smoke maps baked; lit, backlit, from-inside smoke; shadow caster | Soot over flame and edge soot within range; smoke reads dark by day on the sheet. |
| 5. Small fires as presets | `fire.ron`; candle, torch, campfire, brazier, embers; flare and sparks | Every small kind passes the check at three hours. |
| 6. Structure fire | sites from the cellular sim, openings, columns, cinders; alight and charred looks linked to the flicker | The castle film's wall crops pass the building class. |
| 7. Breath | the composite: rod, sprites, soot, embers, hit, lingering fire, scorch, moving light | Breath crops pass the flamethrower class. Review of the castle film. |
| 8. Explosion | fireball one-shot, flash, soot ball, debris, lingering fire | Probe stills pass the napalm and fireball rows. |
| 9. Haze | the distortion node and haze cards | Haze visible on the sheet's sky row, gone by 40 m; under its cost budget. |
| 10. Many fires | LOD bands, birth-rate LOD, the cinematic tier, the burning-village bench | Within budget on both GPUs; the castle film at cinematic holds its capture rate. |
| 11. Wildfire | only if the developers decide forests burn | Its own class of references. |

The change expected to matter most is stage 1, the tonemapper, since no tuning undoes a curve that turns orange peach. The second is stage 3, the channel split, which removes the lime and pastel failures at their root. Stage 2's light is the cheapest large gain at dusk.

## Open for the developers

- **How much fire dominates at dawn against noon** (the exposure weight).
- **The dragon's breath**: a gas jet or liquid fire.
- **Wildfire**: do forests burn and spread in play?
- **Smoke as physics** (it pools under roofs and smothers fires, as Valheim's does): a gameplay question.
