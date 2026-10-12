# Character system: the plan from scratch

How bodies and what they wear would be built if we started over, for humans first, following what modern games with deep creators and varied populations ship. The intent and contracts are in [characters.md](characters.md) (D111, D112, D128). The evidence is [research/modern-character-variety.md](../research/modern-character-variety.md), with methods in [research/one-mesh-for-every-body.md](../research/one-mesh-for-every-body.md). Current thinking, for the developers to read before it is built.

## Goals

- **Many bodies, many items, no item × body work** (D127). A new item is made once; a new body works with every item.
- **A small creator** (D111): male and female, up to twenty templates, skin and hair colours, hairstyles; sliders possible later. NPCs drawn from the same system by seed, varied enough that the world feels alive (D85, Red Dead Redemption 2).
- **No artist**: generated art for looks, scripts for everything else, every step checked by a machine (D69).
- **Looks good** in the painted style of the mood board (D14), at life size (D110).
- **Performs** with many characters on screen and a thousand players (D127).
- **Easy to work on**: adding a body or an item is one command and a check.

## What modern games do

Red Dead Redemption 2, Starfield, Final Fantasy XIV, Bannerlord and Dragon's Dogma 2 share one pattern, read from their own files and talks:

- **One skeleton per family of bodies, with shape mostly in the bones.** Height, limb length, shoulders, build and weight are bone scales or helper bones moved by parameters. Gear skinned to the skeleton follows with no work per item. Final Fantasy XIV carries gear between about 16 body types with one matrix per bone; Bannerlord's weight and build are bone scales.
- **A few body morphs for soft volume, copied onto every garment by a tool.** Starfield ships three on every outfit: Heavy, Muscular and Thin, the corners of its body triangle.
- **The body hides what is covered.**
- **Variety from texture and data:**
  - tint masks and palettes;
  - dirt, wetness and wear as per-character shader parameters;
  - curated looks per kind of person, picked from at random.
  
  This is what makes Red Dead Redemption 2's crowds read as people.
- **Hand work only where it pays:** a few alternate states for conflicts players see constantly (Red Dead Redemption 2's tucked shirts and boots over trousers), and a model of its own for a body where deforming fails (Final Fantasy XIV's per-race flag).

Games that instead make gear per body type (Baldur's Gate 3, Cyberpunk, Watch Dogs: Legion) pay for each item on each type, which is why they keep types few.

## How our first attempts went wrong

- **One generated model per body**, each with its own rig: gear had to be made per body.
- **Then the shared mesh with free shape:** each body fitted vertex by vertex to a generated model, with weights and joints borrowed from that model's rig. It copied the model's clothing creases, broke the face, and failed the rig checks. Each fix uncovered the next.

Neither is what shipped games do. Their bodies are a small number of parameters on one asset, not arbitrary meshes.

## 1. The base body, made once

| Part | What it is | From |
|---|---|---|
| Mesh | One human body in our T-pose, its seams welded | Our own: the base male Tripo made from its approved sheet (`art/characters/base-male/`), cleaned once (its topology rebuilt evenly if morphs or deformation need it) |
| Skeleton | Ours (the Quaternius mannequin, forearm twist bones), so all our clips play | `figure.gltf` |
| Skin weights | Made for this mesh, on our bones | Tripo's rig converted onto our bones (as now; it passes the rig checks), fixed on the base if a check fails |
| Patches | 1,024 patches a garment can hide | Cut once |
| Sockets | Hands, head, back, belt | Rules on the skeleton and skin, as now |

The base is checked with the rig checks until it passes, once.

## 2. A body is a few parameters

| Parameter | How it works | Example |
|---|---|---|
| **Sex** | Either a morph on one base or two bases (male, female); decided by the spike | Starfield's body type 1 or 2 |
| **Proportions** | Bone scales and offsets per bone: height, leg and arm length, shoulder and hip width, neck, head size | Bannerlord, Final Fantasy XIV, ARK |
| **Build** | A few soft morphs: heavy, muscular, thin, plus chest and belly | Starfield's triangle |
| **Head** | Blends of a few head and face shapes | GTA's parent heads, Watch Dogs: Legion's blended heads |
| **Skin** | A painted texture on the base's layout, tinted for skin colour | Red Dead Redemption 2's tints |

- **A template (D111) is a preset of these numbers and a skin.** Sliders later are the same numbers, clamped to the range every item passes its check at.
- **An NPC is a template plus small random offsets** within its culture's ranges, from its seed.
- **Making a template from a design** (`body new NAME SHEET`): the design's sheet and Tripo model, as now. Then:
  - estimate the parameters that bring the base closest to the model: its pose matched, then proportions, build and head solved by least squares;
  - Tripo paints the base's layout in those proportions from the design's front view.
  
  A design is never copied vertex by vertex into a body, so no crease of the generated model comes through. Tried on 2026-10-12: Tripo's paint on a mesh we gave it, from a design's front view, looked like the design, face included.
- **The parameters themselves are made once each, from designs of the base person**: the same sheet redrawn heavier, thinner, stronger, as a woman, with another head. Tripo makes each into a model, and the base's mesh is registered onto it (its pose matched, then drawn smoothly onto the model's skin, the hands kept as the base's). What moved is kept as a morph, so every morph is the same base mesh. Bone proportions are measured from the skeletons. A new morph is a new parameter for every body and is copied onto every item by the tool.

## 3. Gear, made once on the base

| Kind | Examples | Made as | On any body |
|---|---|---|---|
| **Skin garments** | Shirts, jerkins, trousers, gloves, boots, caps | The base's faces it covers (by a coverage rule), pushed out by its stand-off, with its own texture | Skinned to the skeleton, so proportions come free; carries the body's build morphs, copied from the base by vertex |
| **Bound pieces** | Collars, hoods, plates | A model of its own on the base, each vertex bound to the nearest base surface | Weights and build morphs copied through the binding; parts marked rigid ride one bone |
| **Loose pieces** | Skirts, coat tails, cloaks | Bound to a smooth helper between the legs, not the legs | Follows the helper; spring bones later |
| **Rigid things** | Packs, quivers, helmets, tools | Made once in a socket's space, as now | On the socket. Under a helmet the head gives way (a hat morph on the head, as in Starfield), not the helmet |

- **One mesh per item, shared by every wearer.** Only each character's parameter values differ.
- **Colour, once per item:** painted by Tripo on the garment's shape on the base, or baked from one model of the base wearing it.
- **No clipping by construction:**
  - covered patches are not drawn;
  - each slot keeps its height off the skin;
  - an item that fails at a template gets that template's own model by exception, as in Final Fantasy XIV.
- **Checks:** every item on every template and at each parameter's limits (`gear_check`), and its icon.
- **Later, from Red Dead Redemption 2:** a few alternate states (tucked, rolled, under boots) when a conflict shows up constantly; dirt, wetness and wear as shader parameters; tint masks and palettes on everything.

## 4. In the game

- **Appearance is data:** sex, proportions, build, head, skin and tints, hairstyle, outfit. A few bytes, saved and sent; an NPC's from a seed.
- **Bevy does the work it already supports:**
  - Skinned meshes share one skeleton.
  - Proportions are each body's own joint rest positions and scales, from its parameters, applied after animation. Clips carry rotations; the hips' height is scaled.
  - Build morphs are morph weights; every worn piece points at the body's weights (`MeshMorphWeights::Reference`), so one set moves all.
  - The patch mask shader and tints stay as built.
- **No per-character meshes**, so a crowd in the same jerkin draws the same mesh. For crowds later: merging each character's parts, and lighter bodies at a distance.
- **A small Rust crate without Bevy**, `terra_body`, turns appearance data into joint rest positions, scales and morph weights, and holds the binding used by the tools. It is tested without a GPU.

## 5. Tools and workflow

| Command | Does | Cost |
|---|---|---|
| `body base` | Builds the base from its design and our skeleton; rig checks | free, once |
| `body morph NAME SHEET` | A new parameter from a design of the base person: model, registration, morph, checks on every item | 25 credits |
| `body new NAME SHEET` | Design model, parameters estimated, skin painted, checks | 35 to 45 credits |
| `gear new ITEM --kind skin --coverage chest_sleeveless --concept IMAGE` | Piece made once on the base, morphs copied, painted, checked on every template | 10 to 25 credits |
| `gear new ITEM --kind rigid --socket back --concept IMAGE` | Model fitted to its socket, checked | 25 credits |
| `body check`, `gear check` | Every check on everything | free |
| Workbench | Any template in any outfit through any clip | free |

They live in `tools/characters/` (D133): TypeScript to drive, Blender for imports, posing and baking, Rust for what the game shares.

## Tried, 2026-10-12

The spike's first question, answered yes ([research/body-morphs.md](../research/body-morphs.md)):
- Heavy, thin and strong morphs were made from designs of the base man.
- The female body was made a morph of the male base, on her own proportions with her own painted skin.
- The jerkin, cap and pack, made once on the base, were carried onto all four by a tool. Of 12 clipping checks, 11 pass. The one failure is the pack on the heavy body, by skin hidden inside the bag; the fix is to hide the patches under it.

So sex is a morph on one base, and gear is made once for both.

## 6. Order of work

Steps 1 to 3 are built (2026-10-12; [research/body-morphs.md](../research/body-morphs.md#in-the-game-2026-10-12)). Still open from them: the packs' straps on the biggest builds, and the jerkin's ragged edges.

1. **Spike**, one question: does a base body with a few parameters reproduce our two bodies well enough to look right, and does a jerkin made once pass its clipping check on both and at each parameter's limits?
   - build the base and pass the rig checks;
   - estimate our two bodies' parameters and paint them;
   - make one jerkin once, copy the morphs, and check it everywhere.
   
   It also settles sex as one base or two.
2. **The game**: appearance data, per-body joints and morph weights, one mesh per item; the per-body gear files deleted.
3. **The tools and the playbook**; the old per-body scripts deleted.
4. **Then**: hair, the remaining slots, bound and loose pieces, NPC looks per culture, tints and wear on everything, crowds.

## 7. Risks

| Risk | How we would know | If it happens |
|---|---|---|
| The base's weights deform badly at some morph | The rig checks at each morph's limit | Fix on the base, once; corrective morphs at elbows and knees |
| Registering the base onto a design fails somewhere (hands, face) | The fit's error and a look | Keep the base's own there; a design closer to the base's pose and proportions |
| A few parameters cannot reach a design | The parameter fit's error and a look | Add a parameter to everyone (a new bone scale or morph), never a one-off mesh |
| One base for both sexes makes gear sit wrong on one | `gear_check` on both | Two bases, gear moved from one to the other by the binding, with a per-item exception flag |
| Rigid parts bend with build morphs | `gear_check` and a look | Mark rigid parts to ride one bone |
| Loose garments tear between the legs | `gear_check` | The skirt helper; spring bones |
| 27,000 triangles a body is heavy for crowds | GPU measurements, integrated and fast | Lighter bodies at a distance |

## 8. Commercial options

None does this for us in Bevy:

| Option | Why not |
|---|---|
| Reallusion Character Creator | Its licence forbids use in a character generator and with AI |
| Daz | A game licence per item |
| MetaHuman | Made only in Unreal; realistic, not our style |
| Synty Sidekick | Its licence forbids AI work |
| Unreal Mutable, Unity UMA | Tied to their engines; this plan is the same design |
| Meshcapade (SMPL) | A realistic body model only, with no clothing |
| Faceform Wrap | Fits a base mesh onto a generated body, with a person placing landmarks; not needed if parameters are estimated |
