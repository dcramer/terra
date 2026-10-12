# Character system: the plan from scratch

How bodies and what they wear would be built if we started over, for humans first. The intent and contracts are in [characters.md](characters.md) (D111, D112, D128); this is the build plan behind them. Research: [one-mesh-for-every-body.md](../research/one-mesh-for-every-body.md), [character-creators.md](../research/character-creators.md). Current thinking, for the developers to read before it is built.

## Goals

- **Many bodies, many items, no item × body work** (D127). A new item is made once; a new body works with every item. Cost grows with items plus bodies.
- **A small creator** (D111): male and female, up to twenty templates, skin and hair colours, hairstyles; sliders possible later. NPCs drawn from the same system by seed.
- **No artist**: generated art (Tripo, the image model) for looks; scripts for everything else; every step checked by a machine (D69) before a person looks.
- **Looks good** in the painted style of the mood board (D14), at life size (D110).
- **Performs** with many characters on screen and a thousand players (D127).
- **Easy to work on**: adding a body or an item is one command and a check.

## How the games that do this well are built

Every system that fits gear to many bodies has the same core:

- **one base body asset** per family, made once with care: mesh, texture layout, skeleton, skin weights, all made for each other;
- **bodies as shapes** of that asset;
- **gear made once on it**, carried onto every shape.

| System | Bodies | Gear |
|---|---|---|
| The Sims 4 | Every body is the base deformed by shape maps | One mesh per frame (male, female) follows every slider |
| Unreal Mutable, Unity UMA | Shapes of the base | Reshaped by the body's morphs, merged per character |
| MetaHuman | Shapes of one topology | Outfits warped to any body |
| BodySlide (Skyrim, Fallout 4) | Body sliders | Copied onto every outfit offline |
| ARK (inferred, not documented) | Sliders as morphs and bone scales | Armour carries matching morphs |
| Enshrouded | Presets only, no sliders | Fitted to each preset; clipping fixed by hand in patches |

Our first approach skipped the base asset. Each body was its own generated model with its own rig, so gear had to be made per body. Then the first attempt at a shared mesh borrowed each body's weights and joints from its Tripo rig, so every body was built from different parts. The plan below puts the base asset first and derives everything else from it.

## 1. The base body asset, made once

The single source of truth for every human. It is built by one tool, kept in `art/body/`, and checked until it passes.

| Part | What it is | Where it comes from |
|---|---|---|
| Mesh | MakeHuman's body (13,380 vertices, all quads) and eyes, in metres, facing -Y, posed once into our T-pose by its own skeleton | MakeHuman's base mesh (CC0), turned bone by bone onto our mannequin's directions |
| Texture layout | The mesh's own layout | MakeHuman (CC0) |
| Skeleton | Ours (the Quaternius mannequin, plus forearm twist bones), so every clip we have plays | `assets/models/character/figure.gltf` |
| Joint rules | Each joint written once as a mix of the skin round it, so any shape finds its own joints | Computed on the base from MakeHuman's joint cubes |
| Skin weights | Made for this mesh, on our bones | MakeHuman's game-rig weights (CC0), renamed to our bones, then fixed on the base until the rig checks pass |
| Patches | 1,024 patches of skin a garment can hide | Cut once on the base |
| Socket rules | Hands, head, back, belt, placed from joints and skin | As `export_character.py` does now, run on each body |
| Shape library | Shapes of the base in our T-pose: MakeHuman's build, sex, height and proportion shapes, its head and face shapes, then every body we make | MakeHuman's targets (CC0), posed the same way |

Checks on the base: the rig checks we have (seams, reach, symmetry, pinch at elbows, wrists and knees through our clips), against the mannequin's scores. The base is not used until it passes. Fixing its weights is one-time asset work, done once for every body.

## 2. Bodies are shapes

A body is a list of vertex positions in the T-pose, nothing more. Its weights and patches are the base's; its joints and sockets come from its own skin by the base's rules. So a taller body has a taller skeleton, and gear follows it.

**Making a body from a design** (one command, `body new NAME SHEET`):

1. **The design.** The sheet is drawn in the mood board's style with the base's T-pose as a reference (free). Tripo makes a model from its front view (25 credits). Both are kept in `art/characters/NAME/`.
2. **Pose.** The base's bones are turned to lie along the model's limbs (articulated fitting: each bone's rotation solved from its skin's nearest points), so pose and shape are not confused.
3. **Shape**, in this order, each step checked by how far the skin lies from the model's:
   - the library's body shapes, mixed by least squares;
   - the head by the library's head shapes only, so eyes, lids and mouth stay as made to move;
   - a smooth residual below the neck, smoothed by distance, never finer than about 1.5 cm, so it copies no crease of the model's clothes;
   - the hands left as the base's;
   - made symmetric.
4. **Back to the T-pose.** The posed result is un-skinned into the base's rest pose, giving the shape.
5. **Skin.** Tripo paints the shape, imported with the base's layout, from the design's front view (10 to 20 credits). The paint is baked onto the base's layout. Skin colours (D111) are tints of it, not new paintings.
6. **Checks:** fit error, the rig checks on the body, a sheet through our clips, and every existing item's clipping check on it.

Tried on 2026-10-12 with our two bodies:
- The shape fit came within about 4 mm on average.
- Tripo's paint on the shared mesh looks like the originals, faces included.
- The face held only when fitted by face shapes, not freely.

The remaining failures all came from borrowing weights and joints from each body's Tripo rig, which the base asset ends.

**Customisation and NPCs** are mixes of shapes: a template plus small offsets. An NPC's mix comes from its culture and seed.

## 3. What is worn, made once on the base

Four kinds. Each is a file per item, never per body.

| Kind | Examples | Made as | On a body |
|---|---|---|---|
| **Skin garments** | Shirts, jerkins, trousers, gloves, boots, caps | The base's faces it covers, chosen by a coverage rule (parts, hems), pushed out by a stand-off per vertex. Its own texture layout. Its colour painted once. | Each vertex follows the body vertex it was made from, along that body's surface; takes its weights |
| **Bound pieces** | Collars, hoods, plates, pauldrons | A model of its own, placed on the base, each vertex bound to the nearest three base vertices plus an offset | Follows those three vertices; weights blended from them |
| **Loose pieces** | Skirts, coat tails, cloaks | Bound to a smooth helper (one tube between the legs) instead of the legs | The helper follows the body; later, spring bones for swing |
| **Rigid things** | Packs, quivers, helmets, held tools | As now: made once in a socket's space | Hung on the body's socket |

- **Colour, once per item.** Paint the garment's shape on the base, from a concept image (Tripo `texture`, 10 to 20 credits), or bake from one Tripo model of the base wearing it (25 credits), the way that works today. A garment's texture is its own, so it is the same image on every body.
- **No clipping by construction:**
  - covered patches are not drawn;
  - every slot keeps to its height off the skin (straps within 2.2 cm, chest pieces within 1.8 cm above the waist);
  - a push-out pass on each fitted piece moves any vertex inside the body back out.
- **Checks:** every item through our clips on every body in the library (`gear_check`), and its icon.

## 4. In the game

- **A Rust crate without Bevy**, `terra_body`, holds the base, the shapes and the garments as data. It builds:
  - a body from a shape mix: positions, normals, joints, sockets;
  - each garment fitted to that body: positions, normals, weights.
  
  It is deterministic, tested without a GPU, and the same code the offline tools use to check and preview.
- **Appearance is data**: a shape mix (a template plus offsets), tints, a hairstyle, an outfit. A few bytes, saved and sent; an NPC's comes from a seed.
- **Built when a character appears or changes clothes**, off the main thread, not every frame. Meshes are cached by (shape, item), so a village in the same jerkin shares one. Distant characters use a lighter body bound to the base like a garment, so lighter versions cost no new mechanism.
- **Bevy only draws and animates.** One skinned mesh per body and per piece, sharing the body's joints. Each body has its own joint rest positions and bind poses from its shape. Clips carry rotations only, the hips' height scaled to the body, so every clip plays on every body. The patch mask shader and tints stay as built.

## 5. Tools and workflow

| Command | Does | Cost |
|---|---|---|
| `body base` | Builds the base asset from MakeHuman's data and our skeleton; runs the rig checks | free, once |
| `body new NAME SHEET` | Design model, fit, paint, checks | 35 to 45 credits |
| `gear new ITEM --kind skin --coverage chest_sleeveless --concept IMAGE` | Makes the piece once, paints it, checks it on every body | 10 to 25 credits |
| `gear new ITEM --kind rigid --socket back --concept IMAGE` | Model, fit to its socket, checks | 25 credits |
| `body check`, `gear check` | All checks on everything in the library | free |
| Workbench | Any body in any outfit through any clip, sheets for review | free |

The pipeline lives in `tools/characters/` (D133): TypeScript to drive it, Blender only for imports, posing and baking, and the fitting itself in the Rust crate, so the tools and the game cannot disagree. The `character` playbook says how to make and fix each.

## 6. Order of work

1. **Spike** (one question: does the base-asset design hold?):
   - build the base asset and pass the rig checks;
   - make our two bodies shapes of it;
   - make one jerkin once and pass its clipping check on both and on a heavier mix.
   
   Offline, in Blender. If it fails, this plan changes before anything else is built.
2. **The crate and the game:** `terra_body`, bodies built in play from shapes, garments fitted at equip, the old per-body files deleted.
3. **The tools**: the commands above, the playbook, the old per-body scripts deleted.
4. **Then**: hair, the remaining slots' first pieces, bound and loose pieces, lighter bodies for crowds, more templates.

## 7. Risks

| Risk | How we would know | If it happens |
|---|---|---|
| MakeHuman's weights deform badly in our clips | The rig checks on the base | Fix on the base (once); corrective shapes at elbows and knees; weights moved from the mannequin |
| A stylised design is outside what the shapes reach | Fit error, a look at the sheet | Keep designs near the base's proportions (the base's silhouette as a reference for the sheet); add the fitted residual as a new library shape |
| Tripo's paint is poor on some bodies | The sheet | Bake from the design's own texture for the body and paint only the face |
| MakeHuman's face reads as generic | The sheet | The face shapes, plus painted features; our own face shapes later |
| 27,000 triangles a body is heavy for crowds | GPU measurements (D-rule: real GPU, integrated too) | Lighter bound bodies at a distance |
| Loose garments tear between the legs | `gear_check` | The skirt helper; spring bones |

## 8. Commercial options

None does this for us in Bevy:

| Option | Why not |
|---|---|
| Reallusion Character Creator | Its licence forbids use in a character generator and with AI |
| Daz | A game licence per item |
| MetaHuman | Made only in Unreal; realistic, not our style |
| Synty Sidekick | Its licence forbids AI work |
| Unreal Mutable, Unity UMA | Tied to their engines; the plan above is the same design, in Rust |
| Meshcapade (SMPL, commercial licence) | A body model only, realistic, no clothing; MakeHuman's free mesh does the same job for us |
| Faceform Wrap | The only one that solves a step: fitting a base mesh onto a generated body. But it needs a person to place landmarks on each, so it would replace our fitting script only if that script fails |
| Marvelous Designer | Cloth for artists; we have none |
