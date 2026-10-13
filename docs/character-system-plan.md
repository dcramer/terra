# Character system: the plan

How bodies, customisation and wearable gear are built on Bevy, following the techniques shipped games and public tools use. It covers what each part is, where the technique comes from, how it maps onto Bevy, the order of work, and how each step is proven done. Current thinking (D111, D112, D115, D128). For the developers to read before the remaining steps are built.

- **The contracts** are in [characters.md](characters.md).
- **The techniques**, with sources, are in [research/character-systems-industry.md](../research/character-systems-industry.md).
- **What Bevy gives and costs** is in [research/bevy-character-capabilities.md](../research/bevy-character-capabilities.md).
- **How deformation was fixed** is in [research/skin-deformation.md](../research/skin-deformation.md).

## Goals

- **A creator of a few sliders** (D111): sex, build (heavier, thinner, stronger), height (widely, for other races later) and skin colour; hairstyles and hair colour. NPCs drawn from the same sliders by seed.
- **Gear in every slot fits every body** (D64, D128): head, chest with the arms, gloves, legs, feet, back, belt. One item is made once, with no work per item per body (D127).
- **Looks good** in the art style ([art-style.md](art-style.md), D14), at life size (D110).
- **Looks right in motion:** no twisting, squeezing or poke-through in any clip, on any body, checked by machine (D69).
- **Many characters** on screen, toward a thousand players (D127).
- **One command** to add a body parameter, an item or a clip, and one command to check everything.

## The system

Each part names its technique (T1 to T9 in the industry note) and its state.

### Skeleton and motion

| Part | Technique | State |
|---|---|---|
| One skeleton, Unreal's names; Quaternius's animation libraries, current versions; Tripo's V3 rig with weighted twist bones | Standard humanoid skeleton | **Built** (D115) |
| Twist step: a limb's roll moved to the hand and foot and spread over the twist bones, baked into each clip | Unreal's twist bones, Unity's arm-twist setting | **Built** |
| Weight-blended skinning: dual quaternions but round the hips | Maya's weight-blended skinning | **Built** (our shader) |
| Feet on the ground for every body size; hands on what they hold | Two-bone IK after animation | To build: phase 4 |
| Hip offset by body size, so a heavy body's roll stays out of the floor | Root and pelvis adjustment, measured per body | To build: phase 4 |
| Animation at a lower rate for far characters | Unreal's Animation Budget Allocator | To build: phase 6 |

### The body: parameters on one base

| Parameter | Technique | Bevy | State |
|---|---|---|---|
| **Build and sex** (heavy, thin, strong, female) | T2: morph targets on the base mesh, made from designs of the base person | `MorphWeights`; the game mixes joint offsets per morph | **Built** |
| **Height** (widely, for other races later) | T1 at its simplest: a uniform scale of the whole character. Gear, sockets and clips scale with it, so nothing is checked per height; what is held keeps its own size | The character's root scale; our skinning keeps a uniform scale | To build: phase 2 |
| **Other proportions** (limb length, shoulders, head) | T1: bone scales per parameter, on leaf or helper bones so nothing shears | A system writes joint rest transforms, as joint offsets are written now | Not yet (D111) |
| **Skin colour** | T8: tint baked out of the albedo, a skin mask and a palette (melanin-like) in the shader | The body's material extension | To build: phase 2 |
| **Head and face variety** | T2: blends of head morphs | Its own weight set, never animated by clips | Later: design open |
| **Hair** | Hair as a part bound to the head; hide flags and hat-hair variants under helmets | A worn look, by name | Phase 5 |

- **A character is a few slider values** (D111): sex and build as morph weights, height as a scale, skin colour as a tint, each clamped to what every item passes at. The named templates stay as presets for people and tests. An NPC is slider values from its seed, within its people's ranges.
- **One crate turns appearance into numbers.** `terra_body`, without Bevy, turns appearance data into joint transforms and morph weights. It is tested without a GPU and shared by the tools and the game.

### Gear

| Kind | Technique | State |
|---|---|---|
| Garments cut from the body (coat, cap) | Faces of the base pushed out, keeping its weights | **Built**; edges ragged (phase 3) |
| Garments from their own models (a boot, plate, a hood) | T4: weights copied from the body by Epic's inpainting method (MIT reference code); T2/T3: morphs by surface binding with rigid clusters | To build: phase 3 |
| Every garment on every body | T2: the base's morphs carried onto each item | **Built** by nearest vertex; phase 3 moves to surface binding (Outfit Studio's averaging, Mutable's Reshape) |
| Clipping per body shape | BodySlide's clipping fixer, through our clips, a round kept only while it helps | **Built** |
| Rigid looks on sockets (packs, quivers, helmets) | Made in socket space; per-morph shifts to keep clearance | **Built** |
| Loose parts (coat skirts, cloaks, long hair) | T7: helper bones turned by the thighs, then spring bones with capsule colliders (the VRM spring-bone model) | To build: phase 5. Pose-only clipping, which no push can fix |
| Skin hidden under gear | T6: per-triangle masks, conservative, unioned across what is worn, from a sweep of poses with margins at openings | Built by 1,024 patches; per triangle in phase 3 |
| Layering (a shirt under a jacket) | T5: a compressed shape per item and a slot order | **Not planned:** D64 has no layers |

### In the game

| What | How | State |
|---|---|---|
| One mesh per item, shared by every wearer, re-bound to the body's joints | — | **Built** |
| Each look's copied skeleton removed after re-binding | — | Phase 1 |
| Each garment's skin cut down to the joints it uses | Joint matrices are per skinned mesh | Phase 1 |
| The body material made bindless | So bodies batch | Phase 1 |
| Morph weights shared by reference, garments listing morphs in the body's order; body shape never animated by clips | — | Phase 1 check, phase 2 build |
| Characters merged into one skinned mesh with a texture atlas, at a distance or always | T9: UMA's combiner, Mutable's packing; our own merge, since Bevy's doesn't do skins or morphs | Phase 6, if the crowd measurement asks for it |
| Bevy 0.20 | Shaders move to WESL | Phase 7: after `bevy-inspector-egui` supports it (D31); spike the skinning override first |

## Order of work, and how each step is proven

Each phase ends when its checks pass in the build, and a person has looked at the workbench sheets.

| Phase | Builds | Done when |
|---|---|---|
| **0. Deformation and gear on morphs** | Everything marked built above | **Done** (2026-10-12): the build passes; the forearm squeeze through the libraries' 20 hardest clips is down from 70% to 0–13%; every item passes on every body |
| **1. Scale foundations** | Copied skeletons removed; garment skins trimmed; a crowd benchmark (`--experiment crowd`). Still to do: `BodyMask` bindless | **Mostly done** (2026-10-12): entities per character 324 → 105, joint matrices 365 → 108; 1,000 characters on the RTX 5090 25.7 → 13.3 ms. The integrated GPU costs 2.5–4 ms per dressed character, and the world alone is 29.6 ms there, which brings LOD forward (phase 6). Numbers in [research/bevy-character-capabilities.md](../research/bevy-character-capabilities.md#measured-2026-10-12) |
| **2. Body parameters** | `terra_body`; height and proportions in the bones; skin tint; templates as data; the parameter grid in the build (each limit, and mixes such as half heavy and half strong) | Rig and gear checks pass at every limit and mix; joints in the game match the tools to the millimetre (`--joints`); workbench sheets of each template |
| **3. Gear from any model** | Weight inpainting; morphs by surface binding with rigid clusters; per-triangle hiding from a pose sweep; garment edges (hem where the dressed model's garment ends, a turned-in rim, the bake masked); every clip in the rig and gear checks | A boot and a hood made from their own models pass on every template; a rendered hole check finds no holes; no ragged edges on the sheets |
| **4. Contact** | Foot IK; pelvis offset by body size; a held tool's off hand | Feet within 1 cm of the ground while standing on every template; no body below the floor by more than 2 cm in any clip |
| **5. Loose parts and hair** | Thigh-driven helper bones; spring bones with capsules on the thighs; hair as parts; hide flags and hat-hair variants | A long coat and long hair pass the gear check through every clip on every template; no hair through a helmet |
| **6. Crowds** (LOD may come forward, as the integrated GPU needs it) | LOD meshes for bodies and gear, and fewer shadow passes for far characters, for the integrated GPU; animation rate by importance for the CPU; per-character merge and atlas if still needed; NPC looks from a seed | The crowd budget is met on the integrated GPU |
| **7. Bevy 0.20** | Shaders in WESL; skinning override re-done | Every check passes and the sheets match the 0.19 ones |

## Changing Bevy

We are not bound by Bevy's limits (D184):
- **Extend first:** our own systems, components, material extensions and render steps.
- **Replace** a plugin or shader when a measurement says Bevy's won't do. We already replaced the skinning shader.
- **Fork** a crate only as a last resort, kept small and written down.

The likeliest replacement is computing joint matrices once per skeleton rather than per skinned mesh, if phase 1's crowd measurement asks for it. Before that come trimming garment skins and merging each character's parts.

## What could go wrong

| Risk | How we would know | If it happens |
|---|---|---|
| Morph-blended shapes clip between the extremes | The parameter grid in the build | A corrective morph for that pair, or clamp the slider |
| Bone scales shear or squash gear | Rig and gear checks at the limits | Scale on helper or leaf bones only (T1) |
| The crowd costs too much | The phase 1 benchmark | Trim skins, then merge per character, then rate LOD, in that order |
| Weight inpainting is slow or unstable on generated meshes | The rig check on the transferred weights | Its reference implementation is MIT; fall back to nearest weights with smoothing for the parts it fails on |
| A third-party clip misbehaves | The clip check | Check the author's current version first; replace the clip |
| Bevy 0.20 breaks the skinning override | The spike in phase 7 | Stay on 0.19 (D31) until it works |

## Design questions for the developers

Settled on 2026-10-12 (D111): sliders only; height varies widely, for other races later; no other proportions yet. Still open:

1. **Height range for humans,** and later per race.
2. **Head and face variety:** blends of a few heads, or fixed faces?
3. **Hair:** how many styles to start; long hair with physics, or short only at first?
4. **Loose garments:** are long coats, robes and cloaks wanted early? They need phase 5.

## Why not something ready-made

| Option | Why not |
|---|---|
| Reallusion Character Creator | Its licence forbids use in a character generator and with AI |
| Daz | A game licence per item |
| MetaHuman | Made only in Unreal; realistic, not our style |
| Unreal Mutable, Unity UMA | Tied to their engines. This plan rebuilds their techniques on Bevy |
| Synty Sidekick | Its licence forbids AI work |
