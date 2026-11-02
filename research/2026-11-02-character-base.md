# A character base where body, clothes and animations are made together

2026-11-02. The developer, after a day of fixes that each broke something else: "what if we redid the character/model/animations entirely would that help?", then "im not sure if we can find a ready made thing but we could explore that...". Question: what should Terra's character base be, so that the body, the clothes and the animations fit each other from the start? Research only: nothing here changes the game, and nothing was bought. The choice is the developers' (a design question, on the desk).

## In plain words

- **Half the diagnosis holds.** Our skin-tight clothes are already made the studio way: cut from the base body's own surface, keeping its weights, so they follow it. What ripples is that **the body underneath was wrong and was changed after everything was built on it** (short arms and legs, stretched afterwards, D593), and that **every clip was made on another body**, Quaternius's mannequin, so each one is carried across and corrected. Loose pieces (skirts, aprons) have no bones of their own and fold when the thighs lift.
- **One ready-made set fits our licence and our skeleton: Quaternius's.** Its six base bodies, its fantasy outfits and the two animation libraries we already bought are all on one skeleton, the very one our clips come from (the mannequin's 65 joints; ours adds 8 twist bones). It is CC0, so it can sit in our public repository. It costs about $40 for the source editions; the free editions are enough to try it. It is a step more cartoon than our style, and its bodies are about 13k triangles against our 2.5k to 5k budget (ours is 28.9k today).
- **Every other ready-made set is ruled out** by its licence (Reallusion forbids in-game character creation and AI work; store packs can't sit in a public repository), by its style (Synty is low-poly cartoon), or by both (MetaHuman, which since June 2025 may be used outside Unreal, is photoreal and made in Unreal).
- **Making our own properly** means one body made once at the right proportions, never stretched afterwards, with clothes made on it and every clip carried onto it once. MakeHuman's free Blender add-on (MPFB, CC0 bodies) gives that body with true proportions and sliders built in, but it looks realistic and plain and would need our skin painted on.
- **The games we follow do what our contracts already say**: one skeleton per body, armour weighted from the body it is made on, the body hidden under it. Valheim and Kenshi make men and women as two bodies with their armour weighted to each. Nothing public says how Enshrouded or Soulmask do it.
- **Recommended:** try Quaternius's free bodies and outfits in the game beside ours, from the same views, measured by our own checks, before choosing. If they pass and the developer likes the look, rebuild the base on them; if the look is too cartoon, on MPFB; either way the body is fixed once before the clothes and clips are rebuilt on it.

## Testing the diagnosis

The coordinator's brief: the body came from Tripo and was stretched, the clothes are made by scripts and fitted afterwards, and the animations come from four sources made for other bodies. Against the repository:

| Claim | What the repository shows | Holds? |
|---|---|---|
| The body came from Tripo and was stretched | Tripo's model and auto-rig, converted onto our skeleton (`convert_rig.py` moves our joints to Tripo's), then arms and legs stretched and scaled back to 1.80 m (D593). Every gear build after the stretch failed on 7 looks | Yes |
| The clothes are fitted afterwards | Skin garments are the base's own faces pushed out with its weights kept (`shell_garment.py`, docs/characters.md): made on the body, as studios do. Hung pieces (skirts, aprons, `hung_garment.py`) ride a helper between the legs with no bones of their own; the apron's chest folds onto its lap (D593's faults) | Half: tight clothes no, loose ones yes |
| Clips come from four sources made for other bodies | Quaternius UAL1 and UAL2 on the mannequin (65 joints, 13.7k triangles, measured from `assets/models/character/mannequin.glb`); Uthana and Kimodo retargeted onto it; keyed clips on our skeleton; then every clip carried onto the base by `retarget.py` and `aim.py`, with carry aims, arm keeping (D535) and wrist limits laid over | Yes |
| The body is in budget | The base is **28,916 triangles** (measured from `assets/models/character/base.glb`, 73 joints, 4 morphs) against the art style's 2,500 to 5,000 for a character body | No: a fault nobody listed |

So the ripples come from the body being changed under finished work and from the clips being made on a different body. A matched set fixes both only if the body's proportions are right before anything is built on it, and if the clips are made on, or carried once onto, that same body.

## The ready-made sets

Prices and licences read at their sources on 2026-11-02.

| Set | Bodies | Outfits | Animations | Skeleton | Licence: public repo / creator in game / AI | Price | Style |
|---|---|---|---|---|---|---|---|
| **Quaternius Universal Base Characters** + **Modular Character Outfits: Fantasy** + **UAL1, UAL2** | 6: male and female in "Superhero", "Regular" and "Teen" proportions; 20 hairstyles (5 free); about 13k triangles each; eye and skin colour shaders | 12 outfits from 62 parts (82 by one caption), 3 textures each, `All_Male` and `All_Female` versions; v2.1, 2026-05-07 | 120+ (UAL1) and 130+ (UAL2), already bought | One humanoid skeleton across all four, by Quaternius "compatible"; one developer's write-up says the same 65 joints, names and bind poses in every file. Our mannequin has 65 joints, our base those plus 8 twist bones | CC0: yes / yes / yes | Free editions: 2 bodies, 5 hairstyles, 2 outfits. Source editions $19.99 and $20 minimum | Painted stylised, between ours and a cartoon: strong jaws, smooth simplified shapes |
| **Reallusion Character Creator 5** + ActorCore | Any, from sliders; CC4's game base is 10k polygons, CC5's HD base 70k | Many, bought per pack | ActorCore's motion store | Its own CC skeleton | No: files "contained in proprietary formats" (§6.3), no making available to third parties (§2.1B) / **no**: "online or in-software character generation" prohibited (§2.1C2) / **no**: "AI-generated output" prohibited (§2.1C5) | $299 (CC5, August 2025) | Realistic |
| **MetaHuman** (Unreal 5.6+) | Any, from its creator | Its own outfits, sellable on Fab | Unreal's | Unreal's MetaHuman skeleton | Not in source form, on our reading of Unreal's EULA for its content (not checked clause by clause) / made in Unreal with cloud services / may be used with AI, not to train it | Free under $1M revenue a year, then a $1,850 seat | Photoreal |
| **Synty Sidekick** | Body and face blend shapes | Modular packs, mostly $199.99 | Synty's packs | Synty's own | Store EULA: no redistribution / its creator tool is Unity and Unreal only / generative AI needs a custom licence | $199.99 a pack | Low-poly, faceted, cartoon |
| **Mixamo** | Stock characters, each its own mesh | None as a system | A large mocap library | Mixamo's | Use in games embedded; raw files not to be redistributed / n/a / n/a | Free with an Adobe login | Plain realistic |
| **Fab and Unity store sets** (e.g. Stylized Warrior Pack, Undead Knights) | Per pack, usually 1 to 3 | Per pack, made on its own body | A few each, or none | Usually Unreal's mannequin | Fab's Standard License allows any engine but limits fully open-source projects; no source files in public / per pack / per pack | Per pack | Varies by maker |
| **MakeHuman / MPFB 2** (Blender add-on) | One base mesh; sex, build, height, proportions and face as sliders on it | Community clothes (`.mhclo`), each vertex bound to the base's surface with a list of base vertices it hides | None | Several of its own, including a game-engine rig; custom rigs supported since 2026 | Assets CC0, add-on GPLv3: yes / yes / yes | Free | Realistic, untextured |

Daz (a game licence per item) and Ready Player Me (closed January 2026) stay ruled out as in [character-creators.md](character-creators.md).

**MetaHuman is no longer Unreal-only.** Since 4 June 2025 it may be used "in any game engine or DCC application", though it is still made in Unreal. It stays out for its photoreal look (art style: "not photoreal"), its making inside Unreal with cloud services, and its files, which as Unreal content we read as unable to sit in a public repository.

**The repository rule decides most of it.** Terra is a public MIT repository that a clean checkout builds (D38). A store pack's files, or anything built from them, cannot be committed, so the game could not be built without each developer buying a copy, which is why D532 already rules out store clips. Only CC0 sets and our own work pass. That leaves Quaternius and MakeHuman.

### Quaternius more closely

- **It is the set our clips were made on.** `art/README.md`: our mannequin is cut from UAL1 and UAL2, and its skeleton is the one our bone names come from (D115). A Quaternius base body would play those clips without a retarget, its hands where Quaternius put them: the corrections laid over the clips today (carry aims, arm keeping) exist because our body differs from the mannequin.
- **What is not known yet**, for the trial to measure: whether the bodies' joints really match the mannequin's (the shared skeleton is one developer's report, Quaternius says only "compatible"); whether the male and female bodies share one mesh layout, so that the woman can be a morph of the man as D112 has it, or are separate meshes as the `All_Male` and `All_Female` outfits suggest; and the "Regular" bodies' proportions against ANSUR II (`proportions.py`), since the "Superhero" ones are exaggerated by name.
- **What it does not bring**: our eight twist bones (`twist.py` adds them), the 1,024 skin patches, the sockets, the build morphs (heavy, thin, strong), and a look in our style; its outfits are 12, where D182 wants hundreds of variants made by our tools.

## Making our own, the studio way

How studios make one matched set, from Don't Nod's account of Banishers ([80.lv](https://80.lv/articles/don-t-nod-s-banishers-character-art-workflows-digital-outfits-creation)), Epic's modular character docs and MakeHuman's format:

1. **The body first, once.** Bodies from scan data in a few sizes per sex, sharing topology, mesh layout and landmarks, then rigged; body shape changes "without moving the position of its joints". Proportions are settled before any clothing is made.
2. **Clothes on that body.** Marvelous Designer for the pattern and folds only, the shape finished in a sculpting tool, retopologised with "a square topology underneath the triangulated folds for riggers", weighted from the body. A garment is reused on another body by deforming it, not remaking it.
3. **Clips on that skeleton,** each source carried onto it once, from one reference pose, then checked through a range of motion.

For us that is:

| Step | Ours today | Made properly | Jobs |
|---|---|---|---|
| Body | Tripo from a sheet that drew short arms, then stretched | One body checked by `proportions.py` before any gear exists: Quaternius's "Regular", MPFB's at its sliders' defaults, or a Tripo remake from a proportion-checked sheet (about 250 credits, D593). Brought to budget (2.5k to 5k triangles, LODs below) | 2 to 3 |
| Skeleton and weights | Tripo's auto-rig converted, joints moved to Tripo's | Our 73 joints at the new body's own joints, weights from its source (Quaternius's or MPFB's), the twist bones added, `rig_check.py` against the mannequin | 1 |
| Templates | Morphs from Tripo designs, registered on the base | The same `make_morph.sh` on the new base; MPFB's own sliders could give heavy, thin, strong and female directly | 1 to 2 |
| Tight clothes | Cut from the base's faces | The same tools, unchanged, on the new base | rebuild only |
| Loose clothes | A helper between the legs | Skirt bones driven by the thighs (D593's named fix), or Marvelous Designer drapes ($39 a month or $280 a year) weighted from the body | 1 to 2 |
| Clips | Four sources, each carried onto the base and corrected | Carried once onto the new body from one reference pose; keyed clips keyed on it; the corrections removed one by one as the checks allow | 2 |

About 8 to 11 jobs for any of the three bodies, with the comparison first. **Carries over unchanged:** `build_bodies.sh`, the gear tools (`shell_garment.py`, `hung_garment.py`, `bands.py`, `fit_held.py`), the skin patches and body hiding, the attachment contract and sockets, `retarget.py`, `twist.py`, `export_character.py`, `make_morph.sh`, `paint_skin.sh`, every check (`rig_check`, `gear_check`, `proportions.py`, `gait_check`, `attach_contract.ts`, `moves.ts`, the combat scorecard) and the workbench. **Goes:** the Tripo body conversion in `convert_rig.py` and the stretch (`stretch.py`, D593). Marvelous Designer is optional, worth it only once loose garments (coats, robes) are wanted (docs/characters.md, open question 4).

## How the games we follow do it

Public detail is thin; Enshrouded has no mod kit and Soulmask's is barely documented.

| Game | Bodies | Skeleton and clips | Clothing and armour | Source |
|---|---|---|---|---|
| Enshrouded | Presets per sex that fix face, build and skin (8 male, 7 female at launch); no sliders | Not public | Five armour slots and a cosmetic slot each; how it fits bodies is not public. A Keen talk on its character pipeline (Tinko Wiezorrek, 2024) left no slides or video | [Enshrouded wiki](https://enshrouded.wiki.gg/wiki/Armor), [the talk's listing](https://ag.mediencampus.h-da.de/termin/industrytalk-tinko-wiezorrek/) |
| Valheim | Two bodies, male (model 0) and female (model 1); presets for hair, beard, skin | One bone list of 53 shared by body and gear; gear on the wrong bone order distorts | Helmets and capes are skinned meshes on the player's bones; chest and leg armour partly a texture layer on the body; mod tools hide body parts under an item | [Blacksmith's Tools](https://thunderstore.io/c/valheim/p/GoldenJude/Blacksmiths_tools), [Jotunn: bone reordering](https://valheim-modding.github.io/Jotunn/tutorials/bonereorder.html), [TextureCustomizer](https://thunderstore.io/c/valheim/p/thedefside/TextureCustomizer/) |
| Kenshi | Male and female body meshes per race; sliders are shape keys on each | Bone names fixed across races; clips play only on a skeleton of the same names and structure, one per sex | Armour made on a copy of the body so it inherits the weights, once per sex; three bones a vertex; colour part maps hide the body under it; early shirts were textures on the body | [Steam modding thread](https://steamcommunity.com/app/233860/discussions/3/3314107199318270995), [kenshi_io_blender](https://lucius64.github.io/kenshi_io_blender/Option_description.html) |
| Soulmask | Face presets, few body sliders, fixed after creation | Unreal 4.27 (its mod kit); skeleton not public | Not public; armour mods are made per female body replacer | [Steam](https://steamcommunity.com/app/2646460/discussions/0/4330853607900375094) |

What they share: one skeleton per kind of body, which never changes after gear exists; armour weighted from the body it was made on; the body hidden under what covers it. Men and women are two bodies with gear made for each (Valheim, Kenshi), not one body with a morph as ours is. Our contracts in docs/characters.md already say all of this; what we lacked was a body that was right before the gear was built.

## How each option scales

| | Body templates (D593's proportions, women) | Hundreds of variants (D182) | Many on screen |
|---|---|---|---|
| **Quaternius base** | Six bodies; heavy, thin and strong made as morphs by our tools. If male and female share a layout, women are a morph and every item fits both; if not, every worn look is made twice, as in Valheim and Kenshi | 12 outfits to start, recoloured by role; our tools make more on the same body, and CC0 lets every part be cut and recombined | 13k triangles needs a LOD below it for crowds; 65 joints plus 8 twist |
| **MPFB base** | Best: sex, build, height and proportions are all morphs of one mesh, so every item fits every body by construction, as Starfield and Bannerlord do | Nothing ready in our style; every item made by our tools | Its full mesh is above budget; needs reducing as ours does |
| **Our own, remade with Tripo** | As today, from a sheet checked for proportions first | As today | As today, brought to budget |
| **Today's, fixed fault by fault** | Each body change rebuilds and re-breaks gear and clips, as on 2026-11-02 | As today | 28.9k triangles, five to ten times the budget |

## Recommendation

Rebuild the base on a body that is right before anything is built on it, and choose that body by trying it in the game rather than from the vendor's pictures:

1. **First, a comparison job, free.** Quaternius's free "Regular" male and female and its free Ranger and Peasant outfits, put in the workbench and the game beside our base, from the same views, through the same clips (the idle, the walk, the sword guard, a crouch), measured by `proportions.py`, `rig_check.py`, the gear check and the triangle count, and their joints compared with the mannequin's, name by name and at rest. It also answers whether the two bodies share a layout.
2. **If it passes and the developer likes the look:** buy the source editions (about $40, D526) and rebuild the base on the "Regular" bodies with our tools, about 8 to 10 jobs. Its clips need no retarget, and its outfits are a matched starting wardrobe.
3. **If the look is too cartoon:** MPFB's body at its defaults, with our skin painted on (`paint_skin.sh`), its sliders as the templates. Free, more jobs on the look.

Quaternius first because it is the only set where body, clothes and clips were made together and the licence fits, and its skeleton is already ours. Not Reallusion or a store pack: they cannot sit in the repository, and Reallusion forbids a character creator in the game. Not today's base, fixed fault by fault: every fix to the body breaks what was built on it, and it is far over its triangle budget.

## The trial: Quaternius's set beside ours

2026-11-02, char-base j3. The developer downloaded the **Source** editions of Universal Base Characters, Modular Character Outfits: Fantasy (v2.1) and Bestiary: Dungeon Monsters Kit; all three CC0. Nothing from them is committed. Measured with our tools (`proportions.py`, `rig_check.py`, `glb_clips.ts`) and the workbench, in the game's renderer and light, on the Windows GPU.

**In plain words.** They are not low-poly cartoon: 13k to 33k triangles, smooth, with folds, seams and stitching sculpted into their normal maps and buckles and straps modelled. They are a step more stylised than our style in their materials: the cloth and skin are smooth painted colour with no weave, grain or wear, so at play distance a tunic reads as a clean field of green. The faces are lightly stylised (strong jaws, large eyes) and the hair is solid sculpted locks. Beside ours they look cleaner and more finished. Ours look more realistic but blotchy. Their skeleton is the mannequin's, joint for joint, and the mannequin's clips play on them without a fault. Their arms are short, as ours were before D593. The man and the woman are two different meshes, so the woman cannot be a morph of the man (D112).

### What the packs hold

| Pack | What | Formats | Textures | Triangles | Joints |
|---|---|---|---|---|---|
| Universal Base Characters | 6 bodies (Regular, Superhero, Teen; man and woman), each also cut to head only and upper body; 20 hairstyles, beards and brows on the head bone; Godot, Unity and Unreal projects | .blend, glTF (external PNGs), FBX | Painted, smooth: 2048² colour (light and dark skin), normal, roughness; eyes 256² | Regular man 13,880 (body 12,128), woman 14,932; Superhero 14,318 and 15,060; Teen 13,992 and 15,136; a head alone 4,840 and 5,532 | 65, the mannequin's names and parents |
| Modular Character Outfits: Fantasy | Knight, Noble, Peasant, Ranger, Wizard (and Knight with cloth), each for man and woman, 62 parts; `All_Male` and `All_Female` | .blend, glTF, FBX | Painted, smooth: 4096² colour, normal, ORM, three colourways each | As exported, without the head: Ranger 26,982 (man) and 26,966 (woman), Knight 25,187, Noble 24,097, Wizard 14,430, Peasant 12,894 and 13,568 | 65, the same |
| Bestiary: Dungeon Monsters | Hellwarden, Imp, Lycan, Puglin, two skeletons, Tidebreaker; no clips | .blend, GLB, FBX | Painted: 2048² colour, normal, ORM, emissive, three colourways | Hellwarden 19,840, Skeleton A 18,310, B 16,205, Tidebreaker 15,176, Imp 15,232, Lycan 14,145, Puglin 7,528 | 65 (skeletons, Hellwarden, Tidebreaker), 71 (Lycan, plus 6 tail), 55 (Imp, Puglin) |

As built for the trial: their man as the Ranger, with a head and a beard, is 32,856 triangles; their woman as the Peasant, with a head and buns, 22,384. Ours is 28,916 naked. The art style's budget for a body is 2,500 to 5,000 and for a creature 1,500 to 4,000, so both sets need levels of detail cut to budget either way.

### Against the art style's checklist

| Item | Theirs | Ours today |
|---|---|---|
| Silhouette | Clear: hood, belts, bracers, boots | Clear |
| Size | Life size: man 1.81 m, woman 1.77 m | 1.80 m |
| Shapes | Big, medium and small, with modelled detail where it tells (buckles, straps, rivets) | Big shapes; the clothes' detail is in the colour only |
| Material | Leather and metal read, but cloth and skin are smooth colour with no grain or wear | Real cloth and leather grain, blotchy at play distance |
| Shading | Painted and simplified, closer to hand-painted than "a little shaded" | A little shaded |
| One grade | One grade across the set; cleaner and brighter than our props (the barrel) | Matches the props |
| Values | Parts differ in lightness | Parts often the same muddy brown |
| Normals | Smooth, split at creases | Smooth |
| Budget | 5 to 7 times over | 6 times over |
| Contracts | No sockets, no twist bones | Pass |

### Fit, measured

| Measure | Their man | Their woman | Ours |
|---|---|---|---|
| Joints against the mannequin's | The same 65 names and parents | The same | The same, plus 8 twist bones |
| Rest joints from the mannequin's | Mean 6.0 cm, at most 8.4 cm (fingertips); bones within 11° | Mean 10.8 cm, at most 15.7 cm; within 14° | Mean 10.3 cm, at most 14.1 cm; clavicles 34° |
| Mesh | Body 7,047 vertices | Body 7,308 vertices: another mesh | One mesh, the woman a morph |
| `proportions.py` | Fails: fingertip 0.754 m, over the 99th percentile's 0.732 (arms short); reach 1.80 m and crotch 0.90 m in band | Fails: reach 1.66 m under 1.73, fingertip 0.742 over 0.714 (the bands are a man's; there are none for a woman yet) | Passes |
| `rig_check.py`, the mannequin's 33 clips | Motion passes every clip; weights, seams, reach and symmetry pass; rest pose fails (clavicle 12°, limit 3°: its bones do not point at their child, which `retarget.py` reads as the limb) | The same; hand 7° | Passes |
| The clips as they are in a glb (`glb_clips.ts`) | The mannequin's clips carry every joint's position, so played unchanged they pull the joints up to 84 mm to the mannequin's (longer arms) | Up to 157 mm | Carried by `retarget.py` |

So the clips play on their bodies without retargeting in Blender's sense (by bone turns) and pass every motion check; in the game they need either the joints' positions dropped from the clips (all but the root and pelvis, the simplest retarget) or `retarget.py`, as ours do.

Stills: `~/.cache/terra-review/char-base-trial/` (naked row, dressed rows, close views, monsters, textures).

**Not done** (char-base j3 stopped at its context bound): their bodies in play with the idle, walk, sword guard and crouch beside ours, and films of them. Copies of their four models with the mannequin's clips, with and without the joints' positions, are rebuilt in a minute by the job's scripts (the handoff `char-base-j3.md`).

## Limits

- The trial above supersedes this bullet for Quaternius: Nothing was downloaded or tried: the shared skeleton, the layouts and the proportions of Quaternius's bodies are claims until the comparison job measures them. Its free editions need itch.io's download page, which a script could not reach.
- The Synty licence page refused to load (HTTP 429); its terms here come from Synty's product page and a search excerpt of its 2024 purchase licence. Mixamo's FAQ refused to load (HTTP 403); its terms come from Adobe's community answers.
- Enshrouded's and Soulmask's pipelines are not public; the Valheim and Kenshi rows rest on mod tools and mod authors' threads.

## Sources

- Quaternius: [Universal Base Characters](https://quaternius.itch.io/universal-base-characters), [its page](https://quaternius.com/packs/universalbasecharacters.html) (August 2025), [Modular Character Outfits: Fantasy](https://quaternius.itch.io/modular-character-outfits-fantasy), [the shared-skeleton write-up](https://app.cinevva.com/blog/2026-05-10-open-world-browser-part-25-universal-characters)
- Reallusion: [Content EULA](https://reallusion.com/Content/EULA/EULA.htm) (updated 1 August 2025), [CC5's release](https://www.cgchannel.com/2025/08/reallusion-releases-character-creator-5/)
- MetaHuman: [CG Channel, 4 June 2025](https://www.cgchannel.com/2025/06/you-can-now-sell-metahumans-or-use-them-in-unity-or-godot/)
- Synty: [Sidekick](https://syntystore.com/products/sidekick-character-creator), [one-time purchase licence](https://syntystore.com/pages/one-time-purchase-licence)
- Mixamo: [Adobe's FAQ](https://helpx.adobe.com/en/creative-cloud/faq/mixamo-faq.html), [licensing thread](https://community.adobe.com/questions-696/mixamo-faq-licensing-royalties-ownership-eula-and-tos-589400)
- Fab: [the Standard License in other engines](https://forums.unrealengine.com/t/can-i-use-assets-from-fab-for-godot-engine/2451166), [CG Channel on its open-source limit](https://www.cgchannel.com/2024/11/epic-games-to-share-new-free-assets-on-fab-every-two-weeks)
- MPFB: [Blender extension](https://extensions.blender.org/add-ons/mpfb/) (2.0.18, 10 October 2026), [the `.mhclo` format](https://github.com/makehumancommunity/mpfb2/blob/master/docs/fileformats/mhclo.md), [CG Channel on its licences](https://www.cgchannel.com/2025/03/check-out-open-source-blender-character-generation-plugin-mpfb-2)
- Marvelous Designer: [pricing, 2026](https://www.cgchannel.com/2026/08/clo-virtual-fashion-releases-marvelous-designer-2026-1/)
- Studio method: [Don't Nod's Banishers outfits](https://80.lv/articles/don-t-nod-s-banishers-character-art-workflows-digital-outfits-creation)
