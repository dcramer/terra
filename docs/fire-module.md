# The fire module: boundaries and the general flame

Status: Leaning (D466), the agents' technical design; extends [fire.md](fire.md) (D415). The design questions stay Open in fire.md's last section.

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
