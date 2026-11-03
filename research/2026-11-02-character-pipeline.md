# A character pipeline where bodies, outfits, gear, variety and clips are easy to add

2026-11-02. The developer, after weeks of character work: "I just need a pipeline for all of this that will make things easy. Research how folks have done things with Astra and fable". Astra and Fable are the frontier models (OpenAI's GPT-6 Astra, Anthropic's Claude Fable 5), confirmed by the developer. Questions: how people use such models as agents in 3D character pipelines; how shipped games make equipment and crowd variety easy on modular bodies; how retargeting is made reliable across bodies; and what pipeline we should run. Research only: nothing here changes the game, and every pipeline step below is a recommendation. Binding: D599 (Quaternius's bodies and outfits, their parts never changed), D600 (an empty slot shows a bare piece), D602 (the Ranger as the starting kit), D182 (hundreds of variants), D196 (port the best prior art), D526 (paid when much better).

Builds on, and does not redo: [character-base](2026-11-02-character-base.md) (ready-made sets, the studio method), [character-systems-industry](character-systems-industry.md) and [modern-character-variety](modern-character-variety.md) (how games fit gear to bodies), [agent-driven-animation-deep](2026-10-30-agent-driven-animation-deep.md) (agents making clips), [agents-and-blender](agents-and-blender.md) (agents over Blender).

Grades on sources: **[F]** first-hand account with what was done; **[M]** measured by the author; **[V]** vendor or vendor-adjacent; **[P]** primary documentation; **[S]** secondary.

## In plain words

- **Nobody shows a frontier model making, rigging or dressing a character by itself.** Every first-hand account that ended with a usable character gave the geometry to a generator or a ready-made kit (Meshy, Tripo, Rodin, Quaternius), the rig to an auto-rigger, the motion to a motion model (Kimodo) or a library, and kept the model on what it is good at: writing and running Blender scripts headless, gluing tools, and reading measured reports. The accounts that asked Astra or Fable for the character itself got "rough primitive blocks" and figures that "stray considerably from the original designs". That is our method already.
- **What made the good runs good was the harness, not the model:** a separate checker that reopens what the builder wrote and measures it, renders checked for being blank, outputs never overwritten, and gated phases passed on numbers. We have more of this than anyone found (`intact.ts`, `layers.ts`, `slots.ts`, `gear_check.py`, `rig_check.py`, `markers.ts`).
- **What we lack is not a tool but a catalogue.** Shipped systems (Unreal's Mutable, Synty Sidekick, WoW's geosets, Witcher 3's colour variants) make adding things easy because a body, a piece, a colour and a crowd are rows of data that a builder composes. Ours are scripts and hand-joined files: the man in the game is one pre-joined Ranger file, so equipping draws nothing on him and every man in town is the same Ranger.
- **Crowd variety is cheap where it is done well:** pieces per slot mixed under rules, a few colourways or dye masks per piece, hair, and a population of weighted classes (Mutable's "Populations"). Quaternius's six outfits a sex, three colourways each and twenty hairstyles, mixed per slot where `layers.ts` passes, give hundreds of looks with no new geometry.
- **Retargeting is made reliable by baking each clip once per body and fixing feet by contact**, not by eye: Unreal's retargeter pins a foot while the source foot moves under a speed threshold and springs it free after. Our one open fault on the new man, the Sprint_Loop skate, is exactly that.
- **Recommended:** a wardrobe catalogue as data (bodies, pieces, colourways and dye masks, looks, populations), composed by the game at run time per slot as D600 already decides; one build step a row, with the checks we have run over every pairing; a foot-plant pass in the per-body clip bake; and the developer's eye on contact sheets and films at the desk, never on each fix. Nothing needs buying now.

## 1. Frontier models as agents in character pipelines

### What people report

| Who, when | Model and tools | Steps the model did | Steps given to other tools | Result | Grade |
|---|---|---|---|---|---|
| Three creators, compiled by a Meshy designer, 2026-09-21 ([HackerNoon](https://hackernoon.com/gpt-6-astra-vs-meshy-7-for-3d-creation-what-three-creator-workflows-reveal)) | GPT-6 Astra or Codex, Blender, Meshy | Mesh reduction in Blender (3M to 35k triangles), game integration, rig and clip set-up in the engine | Concept T-poses (GPT Image), image-to-3D, auto-rig and walk/run clips (Meshy) | A 12-player shooter in a day and a 45-character brawler shipped (Rishi). A character modeller's side-by-side: Astra driving Blender gave "a rough primitive-block shape useful only for silhouettes". A third creator: iterating on models in Astra "used a large share of his token budget and went nowhere" | F, V (author at Meshy) |
| grapeot, 2026-09-05 ([blog](https://yage.ai/gpt-6-astra-3d-modeling-en.html)) | GPT-6 Astra, Blender scripts | A shrine diorama in about 30 minutes; a simple rig and keyframes on a cartoon duck; a webcam mocap web app | Video (Grok Imagine) | Scenes and props "charming and faithful"; characters "stray considerably from the original designs" and needed heavy manual correction | F |
| blendermcp.org, 2026-09-05 ([test](https://blendermcp.org/guides/astra-blender)) | Astra in Codex CLI, headless `bpy`, not MCP | A parametric building, two variants | none | Both variants passed an independent checker that reopened each file and re-measured it; the render checked for size and non-blank luminance; outputs never overwritten. Blender crashed under a restricted sandbox before any Python ran | F, M (one run, no characters) |
| Ashizawa Kamome, 2026-07-13 ([note](https://note.com/ashizawakamome/n/n45c6798bf944?hl=en)) | Claude Fable 5 over Blender MCP, Codex for textures | A restaurant scene unattended for 2.5 hours, then a browser game in 1.5 | Images (GPT Image 2, through Codex: Fable makes none) | Usable scene; "it's important to properly set up the modeling harness"; a chef character added, no rig or animation reported | F |
| Pixel Artistry, write-up updated 2026-08-12 ([gachoki](https://gachoki.com/claude-just-ran-an-entire-blender-and-comfyui-pipeline-solo-and-the-results-are-wild/)) | Claude (model not named) over Blender MCP and a ComfyUI MCP | Prop kit layout, retopology with Quad Remesher, baking, scene set-up | Character (Rodin), rig (SkinTokens), clips (Kimodo), retarget (Rokoko add-on) | Five clips on five copies. Arms clip through the body in several clips "mainly because the source character wasn't in a clean T-pose"; the model looped on the eyes' topology and a person fixed it | F |
| StraySpark, 2026-05-14 ([test](https://www.strayspark.studio/blog/ai-auto-rigging-showdown-2026-tripo-meshy-cascadeur-mixamo)) | Auto-riggers, no agent | — | One 30k-triangle humanoid through Tripo, Meshy, Cascadeur, AccuRig, Mixamo to UE 5.7 | Tripo fastest (about 3 min) with over-smoothed shoulders; Meshy's non-standard bone names need a manual remap and its feet slide; AccuRig the tightest free weights; Cascadeur best for editing clips, not rigging. "Faces, secondary motion and gameplay logic still need human work" | M, V (sells tools) |
| MindStudio, 2026-05-06 ([article](https://www.mindstudio.ai/blog/claude-blender-mcp-real-world-performance)) | Claude over Blender MCP | Scene set-up, materials, simple object animation | — | A rigged character with weight painting, constraints and IK "not realistic"; animation curves need heavy correction. Unsourced summary of users | S |
| Tripo, 2026 ([workflow](https://www.tripo3d.ai/blog/gpt-6-astra-3d-character-workflow); page refused a fetch, read from the search excerpt), Meshy ([guide](https://www.meshy.ai/tutorials/blender-mcp-guide)) | Astra over Blender MCP with Tripo or Meshy | Alignment before rigging, material checks, review at each stage | Body, head and hair generated as parts | Marketing; no measurements | V |

Agent-made animation (Kansei, `mixamo-llm-mocap`, Salt Creek and the rest) is in [agent-driven-animation-deep](2026-10-30-agent-driven-animation-deep.md): the same shape, a motion model or library for the motion, the agent for baking, foot locking and measured checks.

Skill packs for Claude Code and Codex now promise character work end to end ([EL4CTEO/blender-skills](https://github.com/el4cteo/blender-skills), 672 stars, 2026-10; [majidmanzarpour/blender-game-skills](https://github.com/majidmanzarpour/blender-game-skills), gated phases "passed on rendered and measured evidence", 2026-09). They are instructions, not results: none publishes a character it made with numbers. Two repositories the search engine named for agent clothing pipelines no longer exist (`wfbcargo/blender-godot-plugins` returns 404; `yverse-studio/claude-setup` has no PR 241).

### What works and what fails, by step

| Step | Handed to a frontier model? | What the accounts show |
|---|---|---|
| Body and outfit geometry | No | Fails from the model; generators or kits do it. Consistency across a set is the kit's, not the generator's ([ai-models-for-art](ai-models-for-art.md)) |
| Rigging and weights | No | Auto-riggers do humanoids acceptably, with shoulders and faces weak; no account of a model painting weights. Weight transfer from a body to a garment is an algorithm (Epic's inpainting method, MIT), not a prompt |
| Clothes fitting | No public account | Nobody found shows a model fitting a garment to a body with measured clipping. Our `fit_gear.py`, `bands.py` and `layers.ts` go further than anything published |
| Retargeting | Scripted by the model, solved by a tool | Rokoko, Unreal's retargeter or a script the agent wrote; failures come from rest poses that differ (arms through the body, offsets baked into every frame) |
| Animation clean-up | Partly | Foot locking, in-place conversion, loop closing and pace matching are scripted and measured by agents ([deep survey](2026-10-30-agent-driven-animation-deep.md)) |
| Checking | Yes, by numbers | The runs that held up used a separate checker and measured reports; looking at the viewport "and deciding it seems fine is not accepted" (majidmanzarpour's skill). No account trusts the model's eye on deformation |
| Glue, data, batch runs | Yes | Where every account says the models shine: scripts, unattended runs of hours, turning a design document into files |

**For us:** no evidence that switching models (Astra for Fable, or the reverse) changes any of this; the steps a model fails at fail for both, and the steps it does well we already hand to it. The gain is in what the agent is given to run: a catalogue to build from and gates to pass.

### Tools named in the brief

| Tool | What it is (2026) | For us |
|---|---|---|
| Blender MCP (Blender Lab, 1.0.x) | Live Blender driven by a model | We have it; headless scripts stay the default ([agents-and-blender](agents-and-blender.md)) |
| Tripo, Meshy | Image-to-3D, auto-rig, preset clips | Tripo (Studio subscription) for rigid gear only (bags, quivers, held things); never a body or a skinned garment now (D599) |
| Cascadeur | Keyframe tool with AutoPosing, physics, retarget by copy and paste, "unbaking" a clip into editable keys. Indie $99 a year at its 2024 launch; 2026 prices disagree between sites | A person's tool: little agent API. Worth it only if a person comes to key hero clips |
| Autodesk Flow Studio (Wonder Studio) | Video-to-mocap and video compositing; FBX export; Lite $10, Standard $45 a month (2025) | Not for us: our motion comes from libraries, Kimodo, Uthana and keyed poses (D588) |
| Armature (FabricatorStudio, 2.1, 2026-08) | Free-for-indies rigging workstation, AI skinning on NVIDIA, USD out, Windows | Not needed: Quaternius's weights are fixed by D599 |
| Robust Weight Transfer add-ons, WeightFlow Pro, SkinKit | Weight transfer onto garments | Port Epic's MIT reference instead (D196), already planned in [characters.md](../docs/characters.md) |

## 2. How shipped systems make equipment and variety easy

How gear fits bodies (bone-driven shape, conformed morphs, surface binding, weight inpainting, hiding by region) is in [character-systems-industry](character-systems-industry.md) and [modern-character-variety](modern-character-variety.md); Skyrim's partitions and WoW's geosets are what D600's swap already follows. What is new here is the part that makes adding things easy: **the data model each system composes from.**

| System | Unit of content | How a look is composed | Colour | Crowds | Grade |
|---|---|---|---|---|---|
| Unreal Mutable (beta in 5.5 to 5.7, production-ready in 5.8 by Epic's news) | A Customizable Object: a base mesh, child objects in groups (one-of or toggle), each a mesh section with removal masks and material layers | Parameters (enums, floats, colours, projectors) pick children and drive layers; Mutable builds one merged skeletal mesh with packed textures, at run time or baked | Colour parameters and texture layers per material | **Populations**: classes tied to an object, each a set of Characteristics (a weighted range for one parameter, by option, fixed value or curve); tags on enum options with allow and deny lists; unconstrained parameters drawn uniformly; "new assets can be easily tagged to automatically enrich the variability of existing populations" ([Populations](https://github.com/anticto/Mutable-Documentation/wiki/Populations-(v5.5))) | P |
| Synty Sidekick | Parts by slot (head, torso, arms, hands, legs, feet, hair, attachments) on one skeleton, packs as themes | Pick a part a slot; mix themes; bake combines every skinned mesh into one | Material colours per part in the editor; merged character gets its own texture | Runtime API for random outfits (Unity) ([changelog](https://syntystore.com/pages/sidekicks-changelog)) | V |
| WoW geosets | A body with numbered alternative regions (sleeves, boots, robe flare) and items as textures over the body's layout | An item names which geoset a region shows and paints its texture regions | Per-item textures | NPC display records pick geosets and textures ([warcraft.wiki.gg](https://warcraft.wiki.gg/wiki/Character_model)) | P (via modders' wiki) |
| The Witcher 3 | Clothing meshes authored in a bright red "rarely used in the game" | Appearances list pieces | Colour variants shift the red to many colours | Varied crowds "with ease" from few meshes ([REDkit wiki](https://cdprojektred.atlassian.net/wiki/spaces/W3REDkit/pages/38436869/HOW-TO+Change+clothing+colors), read through a search excerpt; the page would not render) | P (excerpt) |
| Film crowds (SIGGRAPH 2021 talk, Kaur) | Modular clothing, hair and accessories | Hundreds of outfit combinations a crowd element | A colour and texture palette per clan so groups read apart ([PDF](https://history.siggraph.org/wp-content/uploads/2022/06/2021-Talks-Kaur_Creating-Diversity-and-Variety.pdf), from its abstract) | Groups by palette | S |
| Valheim, Kenshi | Gear made per body on one bone list | Item hides body parts | Chest and leg armour partly a texture layer on the body (Valheim) | — | in [character-base](2026-11-02-character-base.md#how-the-games-we-follow-do-it) |

What they share, and what we should copy:
1. **A piece is data that names its body, its slot, what it hides, and its colour regions.** Nothing is pre-joined; the figure is composed from the rows.
2. **Colour is separate from the mesh.** A neutral or masked albedo and a few channels recoloured per instance (Witcher's red, Mutable's layers, Synty's part colours). This multiplies every piece by its palette at no geometry cost.
3. **A crowd is weighted classes, not hand-made people.** A class constrains what defines a role (the smith's apron, the guard's colours) and leaves the rest random; tags let a new piece join every class it suits without editing them.
4. **A merge for many on screen** (Mutable, Sidekick's bake) is the later step; with the men in ten pieces each, the market vantage's 146 people drew in 7.38 ms of GPU time on the integrated Radeon (char-switch j9, the sandbox's `--bench`, not the release bench), so it can wait.

**What Quaternius's packs give already** (from [character-base](2026-11-02-character-base.md)): six outfits a sex (Knight, Knight Cloth, Noble, Peasant, Ranger, Wizard) in pieces by slot on one skeleton, three colourways each, twenty hairstyles, all bought (Source editions) and CC0. Vendored so far: Ranger and Peasant, hair 1 and 2. A man's figure is a head, body, arms, legs and feet piece (plus hood or hair): six choices a slot before colour, cut down by which pairings `layers.ts` passes (the Ranger tunic over the Peasant trousers fails, the reverse passes), times three colourways, times hair. Hundreds of looks before any dye, as D182 asks.

## 3. Retargeting made reliable across bodies

| Practice | Who | How | Ours today |
|---|---|---|---|
| **One skeleton a family, clips baked per body** | Valheim, Kenshi, Unreal's batch retarget | Each clip carried once onto each body offline and stored; never re-solved at run time | Yes: `retarget.py` re-samples every clip on the man (his joints up to 4.6 cm and 17° from the mannequin's), a `build_bodies.sh` step |
| **The pelvis scaled by leg length, chains copied by rotation** | Unreal IK Retargeter: a Pelvis Motion op and FK Chains op; IK goals moved with the pelvis by Affect IK sliders ([op stack](https://dev.epicgames.com/documentation/unreal-engine/retargeting-operation-stack-in-unreal-engine)) | Root motion transferred in proportion; limbs keep the source's rotations | `retarget.py` and `aim.py` |
| **Feet planted by the source's foot speed** | Unreal's Speed Planting op: while a source foot moves slower than a threshold (default 15, range 0 to 100) the target's IK goal "can't move"; after it unplants, a spring with stiffness and critical damping pulls it back ([settings](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/IKRetargetSpeedPlantingOpSettings?application_version=5.7)) | A per-foot lock, then IK | **Missing**: `markers.ts` finds the man's Sprint_Loop skating 2.7/7.8 cm and the build ends `BUILD fail` on it |
| **Stride warping** | Unreal's IK chain settings | Stride scaled with the body's leg, so pace and stride agree | Pace and stride checked by `stride.ts` and `pace.py` |
| **Contacts kept with the mesh, not the skeleton** | Villegas et al., ICCV 2021, Adobe ([paper](https://arxiv.org/pdf/2109.07431)) | Optimises self-contact, interpenetration and foot contact on the skinned body: "contact for a skinned character requires knowledge of the character's 3D geometry; it cannot be accurately determined from skeleton alone"; foot contact accuracy 0.97 | `layers.ts` measures pokes through every clip; nothing fixes them in the clip |
| **Matched rest poses first** | Every account | A retarget across mismatched rest poses bakes an offset into every frame; Pixel Artistry's arms clipped from a non-T-pose source | `retarget.py` aligns from the T-pose |
| **Checks per body, per clip** | Kansei (plant fraction against Epic's set), mixamo-llm-mocap (no pops, no skate) | Measured gates on every bake | `markers.ts`, `rig_check.py`, `moves.ts`, `cargo xtask combat` and `controller` |

So the method is right and baked per body already; the gap is a foot-plant pass after the retarget, ported from Epic's Speed Planting (its docs and behaviour, not its code, under Epic's licence), the plants found from the source clip (`markers.ts` already marks them), and `markers.ts --check` as its gate.

## Recommendation: the pipeline

Everything here is a recommendation; the design parts (what varies, how the town looks) are the developers'.

### The shape

1. **A wardrobe catalogue as data** (one file or a folder of them, beside `bodies.json`; Mutable's model in our words):
   - **Bodies**: each sex's head, bare pieces (D600) and clips file.
   - **Pieces**: a source file (theirs, vendored, or ours), the body it was made on, its slot, its layer, the regions it hides, its colourways, its dye regions (a mask on its UVs, which D599 allows as paint), and its tags (`work`, `noble`, `warm`, ...).
   - **Pairings**: which pieces may be worn over which, written by `layers.ts` and `slots.ts` runs, never by hand.
   - **Looks**: a piece a slot, a colourway or dyes, hair. The starting kit (D602) is one; a template is one.
   - **Populations**: classes per role, each a weighted choice a slot by tag, a palette, and the parameters it fixes; unconstrained choices drawn evenly (Mutable's Populations). A townsperson's look comes from their seed and their class, as docs/characters.md already says an NPC's appearance does.
2. **The game composes at run time** from the catalogue: one mesh a slot on the body's skeleton, the bare piece where a slot is empty (D600), dyes in the material. The pre-joined `male-ranger.glb` goes; equipping then draws.
3. **One build step a row** in `build_bodies.sh` (it already has steps from `bodies.json`): a piece is vendored or built, `intact.ts` proves theirs untouched, its pairings are measured against every piece of the other slots, its dye mask is painted, its icon made.
4. **Clips baked per body** with the holds and a new foot-plant pass, gated by `markers.ts`, `moves.ts`, `combat` and `controller`.
5. **One contact sheet a change** for the desk: the new piece on every look it joins, or a population's twenty draws, through the four moves. The developer approves the sheet, not each fix.

### Who does each step

| Adding | Agent runs | Person approves |
|---|---|---|
| **An outfit of theirs** (Knight, Noble, Wizard, Knight Cloth, colourways, hair) | Vendor the pieces as shipped; catalogue rows; `intact.ts`; `layers.ts`/`slots.ts` against every piece of the other slots, written as pairings; dye mask painted on their UVs; the sheet | Which outfits and colourways belong in Terra (design), the sheet |
| **A gear piece of ours, rigid** (bag, quiver, pouch, held tool) | Concept and model by the `models` and `tripo` skills; socket; `gear_check.py` over every look it can be worn with | The concept sheet, the sheet in game |
| **A gear piece of ours, skinned** (an apron, a smock: what Quaternius lacks) | Made on their body as a new piece (`fit_gear.py`, `bands.py`), weights by inpainting transfer from the body, `layers.ts` against every piece under it | The concept, the sheet |
| **An NPC look or a role's crowd** | A population class as data; a draw of twenty at the town's vantage; a count of distinct looks in the town | The role's palette and pieces (design), the crowd still |
| **An animation** | Source (UAL, Kimodo, Uthana or keyed, D588), into the mannequin file, baked per body with holds and foot planting, every gate | The film of the clip on each body |
| **A body** (the woman, D599's item 3) | Through `finish_character.sh`, her bands in `proportions.py`, every piece's pairings for her | Her sheet |

### Tools: keep, build, buy

- **Keep** every check named above, `build_bodies.sh` and its steps, `retarget.py`, `fit_gear.py`, `bands.py`, the workbench, the desk. They are the harness the first-hand accounts say decides the result.
- **Build**, in order, each a bounded job:
  1. The catalogue and run-time composition per slot (the man's pre-joined file replaced; equipping draws): 1 to 2 jobs.
  2. The other four outfits, their colourways and the twenty hairstyles vendored, with the pairing matrix measured: 1 job.
  3. Dye masks and palettes (folds into v0 item 4, the paint tool): 1 job.
  4. Populations for the town's roles, and a distinct-looks count at the market vantage as a check: 1 job.
  5. The foot-plant pass in the clip bake (Epic's Speed Planting, its docs ported), the man's Sprint_Loop as its first case: 1 job.
  6. Later: weight inpainting for our skinned pieces (Epic's MIT code), merged meshes and LODs for crowds.
- **Buy**: nothing now (D526). Every Quaternius piece is already bought. Cascadeur Indie (about $99 a year at launch; check the price) only if a person comes to key hero clips; Marvelous Designer ($39 a month) only for loose garments past v0.

### What it would have saved on the log

| In [v0-characters.md](../docs/v0-characters.md)'s log | With the pipeline |
|---|---|
| The town's men all the same Ranger, bald and in green (char-switch j9) | A population draws pieces, colourways and hair by role; the variety check fails before landing |
| Starting kit equipped and not drawn on him (j8) | Pieces composed per slot from the catalogue; nothing made for the old base is looked up for him |
| The dressed man joined by hand, then made a build step (j4, j9) | A look is a catalogue row; no joined file |
| The straps over the layered tunic, two jobs (j6, j7) | Not saved by the catalogue: a new skinned piece over their layered outfit is hard everywhere. Two cheaper routes for the developers below (painted harness, or straps as part of a body piece of ours) |
| Mixing measured one pairing at a time (j2) | The pairing matrix measured once an outfit is added |
| The Sprint_Loop skate found, left for item 6 (j8) | The foot-plant pass in the bake |
| The months on the old Tripo body (before 2026-11-02) | Already saved by D599: every first-hand account that worked started from a kit or generator body too |

## For the developers to decide

- **How the town's men vary** (design). (a) Quaternius's six outfits mixed per slot where pairings pass, three colourways, hair: no new geometry, 2 to 3 jobs. (b) That plus dye masks and a palette per role: hundreds of looks and the roles told apart by colour, as Witcher 3 and the film crowds do, one job more. (c) That plus pieces of ours per role (apron, smock): a job a piece. Recommended: (b), then (c) only for the roles that need a piece of their own.
- **Pack straps** (design). Fitted ribbons over every outfit (today's way, two jobs on one outfit), a harness painted on the body pieces' textures under a rigid bag (Valheim's armour is partly texture on the body; D599 allows paint on their UVs), or straps as part of a body piece of ours worn with a pack. Recommended: painted harness under a rigid bag, the ribbons kept for the starting kit's pack.

## Limits

- Few first-hand accounts reach characters at all; none was found that measures a model fitting clothes or painting weights. The Meshy compilation is by a Meshy employee; Tripo's workflow page refused a fetch (403) and was read through a search excerpt; the StraySpark test is by a seller of tools.
- Reddit and X were not read; earlier notes found they refuse automated reads.
- Epic's documentation pages render in script, so the retarget op stack is read from its Python API and a search summary; Mutable's Populations from the Mutable team's GitHub wiki (v5.5), not 5.8.
- The Witcher 3 REDkit page and the SIGGRAPH talk were read through search excerpts only.
- Variety counts for Quaternius's outfits are by arithmetic on the packs' contents; only two pairings are measured.

## Sources

- Frontier models in 3D: [HackerNoon, three creators with Astra and Meshy](https://hackernoon.com/gpt-6-astra-vs-meshy-7-for-3d-creation-what-three-creator-workflows-reveal) (2026-09-21); [grapeot on Astra's 3D modeling](https://yage.ai/gpt-6-astra-3d-modeling-en.html) (2026-09-05); [blendermcp.org, Astra and Blender reproducible test](https://blendermcp.org/guides/astra-blender) (2026-09-05); [Ashizawa Kamome, Fable 5 and Blender MCP](https://note.com/ashizawakamome/n/n45c6798bf944?hl=en) (2026-07-13); [Pixel Artistry via gachoki](https://gachoki.com/claude-just-ran-an-entire-blender-and-comfyui-pipeline-solo-and-the-results-are-wild/) (updated 2026-08-12); [MindStudio on Claude and Blender MCP](https://www.mindstudio.ai/blog/claude-blender-mcp-real-world-performance) (2026-05-06); [Tripo, Astra character workflow](https://www.tripo3d.ai/blog/gpt-6-astra-3d-character-workflow); [Meshy, Blender MCP guide](https://www.meshy.ai/tutorials/blender-mcp-guide); [Claude Fable 5 launch coverage](https://dev.to/jobinkjames/claude-fable-5-is-here-anthropics-most-powerful-public-model-yet-2d3j) (2026-06)
- Skill packs: [EL4CTEO/blender-skills](https://github.com/el4cteo/blender-skills), [majidmanzarpour/blender-game-skills](https://github.com/majidmanzarpour/blender-game-skills)
- Rigging tools: [StraySpark auto-rigging test](https://www.strayspark.studio/blog/ai-auto-rigging-showdown-2026-tripo-meshy-cascadeur-mixamo) (2026-05-14); [Armature, CG Channel](https://www.cgchannel.com/2026/08/armature-is-a-free-ai-assisted-standalone-character-rigging-tool/) (2026-08-06); [Cascadeur 2024.1 and pricing](https://gameworldobserver.com/2024/03/21/cascadeur-animation-unbaking-autophysics-new-pricing-model); [Flow Studio pricing](https://digitalproduction.com/2025/08/13/flow-studio-prices-drop-tiers-multiply-features-shift) (2025-08-13); [weight transfer add-ons](https://digitalproduction.com/2025/04/23/weight-no-more-streamlining-blenders-weight-painting-with-robust-weight-transfer/)
- Equipment and variety: [Mutable development guides, UE 5.7](https://dev.epicgames.com/documentation/unreal-engine/mutable-development-guides-in-unreal-engine?application_version=5.7); [Mutable Populations (v5.5)](https://github.com/anticto/Mutable-Documentation/wiki/Populations-(v5.5)); [Mutable sample, UE 5.8](https://www.unrealengine.com/news/mutable-sample-for-runtime-character-customization-now-updated-for-ue-5-8); [Synty Sidekick changelog](https://syntystore.com/pages/sidekicks-changelog); [WoW character model](https://warcraft.wiki.gg/wiki/Character_model); [Witcher 3 REDkit, clothing colours](https://cdprojektred.atlassian.net/wiki/spaces/W3REDkit/pages/38436869/HOW-TO+Change+clothing+colors); [Kaur, Creating Diversity and Variety, SIGGRAPH 2021](https://history.siggraph.org/wp-content/uploads/2022/06/2021-Talks-Kaur_Creating-Diversity-and-Variety.pdf)
- Retargeting: [Unreal retarget op stack](https://dev.epicgames.com/documentation/unreal-engine/retargeting-operation-stack-in-unreal-engine); [IK Retargeter Python API, 5.7](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/IKRetargeter?application_version=5.7); [Speed Planting settings](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/IKRetargetSpeedPlantingOpSettings?application_version=5.7); [Villegas et al., Contact-Aware Retargeting of Skinned Motion, ICCV 2021](https://arxiv.org/pdf/2109.07431)
