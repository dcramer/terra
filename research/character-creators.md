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

## What shipped systems confirm, 2026-10-12

Looked up again after generated gear, fitted one piece at a time, clipped and sat wrong on our bodies:

- **Hide the body under gear.** Mutable clips body geometry with a clothing mesh ([node reference](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/CustomizableObjectNodeModifierClipWithMesh)); MetaHuman outfits carry body masks; Cyberpunk modders toggle body parts off under clothing that fully covers them, and warn against it where skin is only partly covered ([ArchiveXL body toggles](https://www.nexusmods.com/cyberpunk2077/mods/26627)). Asset makers ship a full body and hide the parts that clip ([Ida Faber](https://docs.idafaber3d.com/unreal-engine/modular)).
- **Author gear on reference bodies.** MetaHuman's resizable outfits keep several source sizes and pick the one nearest the body's measurements ([MetaHuman 5.6](https://metahuman.com/news/metahuman-leaves-early-access-with-a-feature-packed-new-release), [parametric clothing](https://dev.epicgames.com/documentation/metahuman/creating-parametric-clothing-for-metahuman), [forum](https://forums.unrealengine.com/t/new-outfit-asset-for-metahumans-in-unreal-5-6/2520056)); BodySlide builds every outfit with the body's own sliders, "so armour conforms to the body preset without clipping", and outfits made for one body don't fit another with different sliders without conversion ([BodySlide](https://www.nexusmods.com/fallout4/mods/25)). One poster suggests a few skeletons (small, male, female, large) with morphs in between, since one chest piece across a dwarf and a tall elf distorts badly ([Epic forum](https://forums.unrealengine.com/t/modular-equipment-system-for-multiple-characters-best-practices/247805)).
- **Same named points on every skeleton; the item carries its grip.** Every skeleton has the same attachment points; clothing is merged into the skinned mesh and rigid items attach to sockets ([Epic forum](https://forums.unrealengine.com/t/modular-equipment-system-for-multiple-characters-best-practices/247805)). A weapon's own pivot is set to line up with the hand socket, since meshes differ in where their handles are ([Epic forum](https://forums.unrealengine.com/t/weapon-socket/19413), [UE sockets](https://docs.unrealengine.com/4.26/WorkingWithContent/Types/SkeletalMeshes/Sockets)); the off hand reaches a second grip by IK ([IK Rig weapon goals](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/IKRetargetWeaponGoalsOpSettings)).
- **Enshrouded** shows only slots (head, upper body, arms, legs, feet) and a cosmetic layer that can hide pieces; nothing public on how its armour fits bodies ([wiki](https://enshrouded.fandom.com/wiki/Armor), [vanity system](https://metaforge.app/enshrouded/enshrouded-the-new-vanity-system)).

What this means for us is in [docs/characters.md](../docs/characters.md).

## Tried, 2026-10-12: making clothing on the body

From a model of the base body wearing the piece (its own sheet, redrawn wearing it, made by Tripo):

- **Cutting the piece out of the dressed model failed.** Tripo's dressed model is one outer surface: no skin under the jerkin. Kept by standing off our base's skin, a close jerkin came out patchy, since Tripo's dressed body is a little thinner than ours. Kept by colour as well, against our base's colours or the dressed model's own skin and shorts, it came out full of holes and strips: Tripo paints the dressed body a shade off ours, with its own light and shade. Its segmentation splits by anatomy (head, torso, limbs), not by clothing.
- **Building the piece from our own body works.** Mark the body's faces the piece covers, by a coverage said once per item (the parts it covers and where its hems fall), copy them, push them out (as far as the dressed model stands off, for bulk), keep each vertex's weights, and bake the dressed model's colour onto it. It moves as the skin under it does. A sleeveless jerkin on both bodies: under 1% of nearby skin through it in idle, walk, jog and the sword swing on the male, under 2.5% on the female; about 4-5% in a crouch or a roll, where the thigh rises into the hem and the arm presses the armhole. Cutting the armholes back to skin the arm moves less than 15% took the female's idle from 6% to under 2%.
- What a check measures matters as much as the piece: the first two clipping checks reported nonsense (nearest-surface sides on single-sided generated meshes, rays that miss when arms come down). Skin with the piece's surface just beneath it is the measure that held.

## Limits

- Nothing found on how Enshrouded shades or textures its characters, and nothing technical on Black Desert, Elden Ring or Conan Exiles.
- Enshrouded's 1.0 is due 15 October 2026; this describes Early Access.
- That Quaternius's packs share one skeleton is reported, not checked. Whether MakeHuman's sliders survive the move onto a stylised body is untried.
- The only thing tried is fitting three generated pieces to the sandbox's mannequin. A great base body, the hardest part, has not been attempted.
