# Realm two: the far realm

Status: **a first slice built, 2026-11-08; its theme is the developers' (D642, Leaning)**: the realm is a row of `assets/data/realms.ron` (`terra_world::realm`): its light, sky, ground colours, the share of plants that grow (one in ten) and the turn of their hue (toward ochre), and five built parts scattered by the seed, standing round the player as they walk (`terra_game::play::realm_parts`); the first realm's gate leads to it. Only the animals its row names live in it (foxes and hares, a stand-in; nothing keeps its den). Its sign shines teal and lights the ground round it by night (a `Light` part and a catalogue `Lamp`). Its ground has its own layers (`assets/materials/ground_industrial`: gravel, asphalt and cracked concrete for the first realm's grass and leaf litter). Not built: lights on its other parts, wildlife, monsters, sound; no still has been shown to the developers. The realm beyond the first world's gate (D641): the proof that a realm is data ([v0-tracks.md](v0-tracks.md), Realm two; [art-style.md](art-style.md), "What every realm shares"). It needs enough of each part to show a stranger another world of the same game, not a full realm's worth.

## What it is

**A fantasy and science-fiction world, where the first realm is real and medieval** (D642, the developers, 2026-11-08: "great, and different than the first world"). Another world at a glance: a violet sky and a cool rose sun, a forest of teal leaves over rose and plum grass, plants of its own that glow at night, a land of its own shape, and the standing remains of an older, stranger making. Seen on 2026-11-08 it was the first realm's forest in another colour: its trees, its land's shapes and its grass were the first realm's. That is what is to change, in this order: its look by data (built: sky, sun, leaves, grass, soil), its own plants (grown kinds with their own habits, bark and leaf cards, D222), its night light, its land recipe, its parts (crystal, stone, machines: models, which cost credits), its small life, monsters and sound.

## What grows in it (proposed, 2026-11-08; the developers: "we might not want regular looking trees", the trees' way of making being how to get whatever is chosen detailed)

Not the first realm's trees in another colour. What stands where trees would is its own, each grown by rule as the forest's trees are (a skeleton by its kind, cards on it, a scanned or made surface, levels of detail and a far form: `terra_sandbox::experiments::lush`), so it is as detailed near and as cheap far:

| Kind | What it is | Made as |
|---|---|---|
| Cap trees | Great fungi 8 to 25 m tall: a ribbed stalk, one or several wide caps in tiers, gills that glow at night | A grown skeleton (stalk and cap ribs) with cap and gill surfaces; the modelled FarGiantFungus is its small form |
| Frond spires | A tall bare stem with whorls of long drooping fronds, lit at their tips | The conifer's skeleton with its whorls kept and its needles replaced by frond cards |
| Fan trees | A low, wide, forking tree whose branches end in flat translucent fans | The oak's forking skeleton with fan cards at its tips |
| Undergrowth | Plum and magenta bushes, cyan ferns, glowing tufts | The first realm's grown bushes, rosettes and tufts on the realm's own leaf atlas (`foliage_far`, built) |

Which of these, and how strange, is the developers' to say; the first to make is the cap tree, as it is the least like a tree.

**What it costs** (a reading, not a bench: the high preset on the Radeon 8060S, a full-size world, standing): the first realm at its start 41 frames a second; this realm 39 at its gate and 35 two hundred metres on. The crystal's and the monolith's lamps are the likely tenth: some forty point lights within reach.

## Its parts, each by an existing pipeline

| Part | In the first realm | In this one | By |
|---|---|---|---|
| The theme itself | `kind: "world"` | `kind: "industrial"`, a row of `assets/data/realms.ron`: its light, sky, ground and what stands on it | data (D136); `play::portal::gates::beyond` names it |
| Light and sky | clear northern day | amber-grey overcast, a low orange sun, a long dusk; the night lit teal and magenta by what glows | the sky's and the scene's tweaks, as the placeholder realm sets them |
| Ground | soil, grass, rock | hardpan and cracked slab, rust-stained gravel, slag; almost no grass | the ground's colours first; its own materials (`tools/textures`) after |
| Plants | old-growth forest | few and hardy: scrub, lichen, bare trees; open ground | the flora's kinds and the woods' density, turned down |
| Built parts | none in the wild | large props scattered by the seed: a pylon with cables, a pipe stack and vent, a tank, a derelict machine, a glowing sign | the model pipeline (`tools/props`, the `models` skill), each a kind placed by rule |
| Wildlife, monsters, sound | forest animals, birdsong | after: their tracks |

## Order of work

1. The realm as data: `realms.ron`, read on entering a world of its kind; the light, sky and ground colours from it (replacing the placeholder realm's code). Check: a scripted crossing reads the realm's name and its sun and ground colours from the game.
2. The first built parts: four or five props, concept then model, each checked against the style guide and the model contract.
3. Their scattering by the seed, clear of the gate and the start, with their solids.
4. Fewer plants. Stills beside the first realm for the developers.

## Open, the developers'

- The theme's details: how lit, how ruined, whether anyone lives there.
- Its land recipe's size and relief; v0 differs from the first realm only by climate in survival's rules (v0-tracks).
