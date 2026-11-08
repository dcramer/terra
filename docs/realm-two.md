# Realm two: the industrial realm

Status: **a first slice built, 2026-11-08; its theme is the developers' (D642, Leaning)**: the realm is a row of `assets/data/realms.ron` (`terra_world::realm`): its light, sky, ground colours, the share of plants that grow (one in ten) and the turn of their hue (toward ochre), and five built parts scattered by the seed round the start (`terra_game::play::realm_parts`); the first realm's gate leads to it. Its sign shines teal and lights the ground round it by night (a `Light` part and a catalogue `Lamp`). Not built: its own ground materials, lights on its other parts, parts beyond 500 m of the start, wildlife, monsters, sound; no still has been shown to the developers. The realm beyond the first world's gate (D641): the proof that a realm is data ([v0-tracks.md](v0-tracks.md), Realm two; [art-style.md](art-style.md), "What every realm shares"). It needs enough of each part to show a stranger another world of the same game, not a full realm's worth.

## What it is

A worked-out, built-over land: concrete and rusting steel under a hazed sky, a few lights still burning (D642). Industrial, a little futuristic, cyberpunk-like; hand-made things are replaced by fabricated ones, timber and thatch by "panels and cables".

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
