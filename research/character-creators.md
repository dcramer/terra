# How character creators work, and what fits Terra

2026-10-11. Question: how do the best character creators and equipment systems work, with Enshrouded as the model the developers named, and how would we build one that fits our goals? Those goals: generated assets, many realms with cultures, NPCs made by rule, visible equipment, the painted look of the mood board (D14), life size (D110), and many players on one server.

Read, not tried, except where it says otherwise. The one thing tried is fitting generated gear to a rigged body ([spike/character_gear](../spike/character_gear/README.md)).

## Answer

**Enshrouded's creator is simple, and that is the lesson.** It has no sliders. You pick a preset, which fixes face, body and skin together, then hair, beard, colours and voice ([GameWatcher](https://www.gamewatcher.com/enshrouded-character-creator-customization-options), [update notes](https://enshrouded.com/en-US/news/enshrouded-back-to-the-shroud-update)). Armour is one piece per slot (head, upper body, arms, lower body, feet), with a cosmetic slot for each, any piece can be hidden, and a tinting station recolours most pieces in three channels from crafted palettes ([wiki](https://enshrouded.wiki.gg/wiki/Armor), [update notes](https://enshrouded.com/en-US/news/enshrouded-thralls-of-twilight-update)). Clipping is fixed by hand, piece by piece, in its patch notes. Players call its characters the weak part: "undetailed" beside the terrain ([Steam](https://steamcommunity.com/app/1203620/discussions/0/591772052728335891/)). What the developers like in Enshrouded is its world and light, not character tech. Its NPCs are about 27 hand-made people; it has no system for generating them.

So Terra can match Enshrouded with a small system, and needs one thing Enshrouded lacks: NPCs generated from culture rules.

## How the others do it

| Game | Technique | What to take |
|---|---|---|
| WoW | The body is split into regions ("geosets") that armour hides or replaces ([wiki](https://warcraft.wiki.gg/wiki/Character_model)) | Hide-masks: no clipping without hand-fitting every pair |
| Skyrim, Fallout 4 | Armour carries the body parts it covers, which are then not drawn ([CK wiki](https://ck.uesp.net/wiki/Skyrim_bodyparts_number)); BodySlide copies the body's shape sliders onto outfits | Hiding by slot; copying body shapes onto gear offline |
| Cyberpunk 2077 | A shape key that shrinks an inner garment when another is worn over it ([wiki](https://wiki.redmodding.org/cyberpunk-2077-modding/for-mod-creators/3d-modelling/garment-support-how-does-it-work)) | Layering without fitting every combination |
| The Sims 4 | Morphs and bone deformations, part pairing, randomised townsfolk with tags for what random NPCs may wear ([GDC](https://gdcvault.com/play/1022085/Innovations-in-The-Sims-4)) | Tags that control what generated NPCs use |
| Baldur's Gate 3 | Fixed face presets per race, four body types, no sliders ([Steam](https://steamcommunity.com/app/1086940/discussions/0/3811784760602395694/?ctp=3)) | Presets are enough for a top RPG |
| Bannerlord | An NPC is a face key plus age, weight and build within a range; culture templates limit hair and beards by tag ([docs](https://docs.bannerlordmodding.com/_xmldocs/npccharacters/npccharacter/face/bodyproperties)) | A culture is parameter ranges plus allowed parts |
| Kenshi | NPCs are drawn from all the same sliders as the player | One system for players and NPCs |
| Soulmask | Tattoos, body paint and dyes, on recruits too ([Steam](https://steamcommunity.com/app/2646460/discussions/0/4330853607900375094)) | Culture shown through paint layers |
| Synty Sidekick | A bought stylised modular kit: parts, body blend shapes, colours, baked to one mesh ([store](https://syntystore.com/products/sidekick-character-creator)) | Same architecture; its licence forbids AI work, so not for us |

Engine systems: Unreal's [Mutable](https://dev.epicgames.com/documentation/unreal-engine/mutable-overview-in-unreal-engine?lang=en-US) removes hidden faces, merges a character's parts into one mesh, bakes morphs and textures in the background, and sends only the parameters over the network. Unity's UMA does much the same with "DNA" and recipes. We would not need their node graphs, only the same few steps.

## Tools for us

| Need | Best option | Notes |
|---|---|---|
| Skeleton and animations | Quaternius Universal Animation Library, already in the sandbox | CC0, 250+ clips. Reported, not yet checked: the same skeleton as Quaternius's base characters, outfits and hair ([itch](https://quaternius.itch.io/universal-animation-library), [write-up](https://app.cinevva.com/de/blog/2026-05-10-open-world-browser-part-25-universal-characters)) |
| A base body | Quaternius Universal Base Characters (CC0) to start; restyled toward the mood board | Six bodies, 20 hairstyles, rigged to that skeleton ([itch](https://quaternius.itch.io/universal-base-characters)) |
| Body-shape sliders, if wanted | MakeHuman / MPFB2's targets, CC0 ([licence](https://static.makehumancommunity.org/about/license.html)) | Realistic proportions; moving them onto a stylised body is our work |
| Ruled out | Reallusion Character Creator (its licence forbids use in any character generation system and AI work, [EULA](https://cdn-www.reallusion.com/Content/EULA/AP/EULA_AP.htm)); Daz (per-item game licences); Ready Player Me (shut down January 2026) | |
| Gear made to fit | Our Blender script: fit, push out of the body, copy the body's weights, hide what is covered | Works for single pieces ([spike](../spike/character_gear/README.md)); pairs should be generated one at a time |
| Gear following body sliders | Blender's Surface Deform: each body slider saved as a matching shape key on each piece | Scriptable; untried |
| Generated heads as slider variants | Non-rigid registration (trimesh's `nricp_amberg`, open source) onto the base head, with landmarks | Possible by script; likely to fail on extreme stylised heads, so each needs a look |

Bevy 0.19, read from its source: any number of skinned meshes can share one skeleton's joints; a part's morph weights can point at the body's, so body, clothes and hair move with one set of sliders (`MeshMorphWeights::Reference`, `bevy_mesh-0.19.1/src/morph.rs`); up to 256 morph targets and 256 joints a mesh; animations play on any skeleton with the same bone names. `Mesh::merge` does not merge morphs or remap joints, so merging a character into one mesh is ours to write if it is needed.

## Appearance as data

A character is a small record, not a mesh: a body preset, a few bone scales, a part per slot, a palette colour per role. A player's fits in under 64 bytes, saved and sent as is. An NPC needs only a seed: the realm, the culture and the person's number, expanded the same way on every machine (GOALS does this, per [GPC 2025](https://graphicsprogrammingconference.com/2025/)). A culture is a template, as in Bannerlord: which parts and hairstyles it uses, ranges for the scales, its colours for each palette role, and its paint patterns.

## Many players

Drawing many customised characters is solved by merging each character's parts into one mesh after their gear changes, simpler versions at a distance, and a player setting for how many get full models (Guild Wars 2, [notes](https://www.guildwars2.com/en/news/world-vs-worldthe-end-of-culling/)). Unreal's own comparison puts merged characters at one draw call each ([docs](https://docs.unrealengine.com/4.27/AnimatingObjects/SkeletalMeshAnimation/WorkingwithModularCharacters)).

## A proposed shape, for the developers

Inferred, and a design question that is theirs:

1. **Like Enshrouded, plus.** Two to four base bodies with a few bone scales (height, shoulders, build, head). Preset faces, swappable hair and beards. One skinned piece per equipment slot, fitted to each base body by script, hiding the body regions it covers, with three tint channels taken from palette roles. A cosmetic slot per slot, and any piece hideable.
2. **Cultures on top.** Templates that pick parts, scales and colours for generated NPCs, and paint layers (tattoos, body paint) that mark a culture, as Soulmask does.
3. **Later, if wanted:** body-shape sliders, copied onto every piece of gear offline.

## Limits

- Nothing found on how Enshrouded shades or textures its characters, and nothing technical on Black Desert, Elden Ring or Conan Exiles.
- Enshrouded's 1.0 is due 15 October 2026; this describes Early Access.
- That Quaternius's packs share one skeleton is reported, not checked. Whether MakeHuman's sliders survive the move onto a stylised body is untried.
- The only thing tried is fitting three generated pieces to the sandbox's mannequin. A great base body, the hardest part, has not been attempted.
