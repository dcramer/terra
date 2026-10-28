# Assembled items: can our engine and tools carry D565, as one simple pipeline with no per-item fitting

2026-10-28, characters and items. Question: can Bevy at our pin, our build and our checks carry D565's design ([docs/assembled-items.md](../docs/assembled-items.md), [the research](2026-10-28-assembled-equipment.md)) reliably, as a pipeline in which a new item is a choice of parts and never a model adjusted to fit a hand? The developer: "i want the absolute best, simplest pipeline we can create here to make it work correctly. i dont want multiple days of you looping, showing me screenshots, trying to micro adjust models to fit in a hand".

Evidence is our code (path:line), Bevy 0.20.0's source as pinned (`~/.cargo/registry/src/index.crates.io-*/bevy_*-0.20.0`, written `bevy_x:path:line`), our commits and one run timed for this note. Nothing was built.

## Answer

Yes, with one step unproven. The engine side is already there: a held look is a rigid scene hung on a socket node, posed in the same frame as the body, and Bevy batches many distinct part meshes and materials into few draws. The pipeline can be made per kind, never per item, if **one Rust assembler** (pure, no Bevy) turns a recipe into part placements and the item's points, and every consumer reads its output: the game spawns the parts at those placements, and a command writes the same placements as a glb in today's held-look contract, so every check we have (contract, move check, grip, workbench, icons) reads an assembled item unchanged. What is missing is small to medium tooling (a part checker, the envelope rule, a batch mode for the move check, a sheet differ and approval store) and one medium-large unknown: whether generated clips hold the whole envelope, which is D565's step 3 (the item track) and must be proved before anything else is built.

## Verdicts

| # | Question | Verdict | Confidence | Evidence |
|---|---|---|---|---|
| 1a | Rigid assembled items on sockets | **Yes, as today.** An item is a parent entity under the socket with 2–4 part children; nothing new in the engine | high | `terra_walker/src/character.rs:350-359` spawns the look `ChildOf(hand)`; animation runs `.before(TransformSystems::Propagate)` (`bevy_animation:src/lib.rs:1322-1340`), propagation in PostUpdate (`bevy_transform:src/plugins.rs:37-47`), skins extracted after it (`bevy_pbr:src/render/skin.rs:286`): body and item share a frame, no lag. The grip already cuts every mesh under the held root (`terra_walker/src/grip.rs:417-474`), so several parts work |
| 1b | Merge parts at bake time, or an entity a part | **An entity a part, sharing part meshes.** Merging makes every item its own mesh: no instancing, memory per item, a bake per recipe. `Mesh::merge` exists for a later crowd LOD | high | `bevy_mesh:src/mesh.rs:2208` (`merge`: same topology, matching attribute formats; an attribute only one side has is a trap, :2224); `transformed_by` :2288; the glTF loader merges nothing (`bevy_gltf:src/loader/mod.rs:188-224`) |
| 1c | Batching many distinct items for many players | **Yes on desktop GPUs, unmeasured at crowd scale.** Parts that share a vertex layout, pipeline and material slab go in one multi-draw-indirect call, one bin per distinct mesh; StandardMaterial is bindless, so material instances share a slab. Lost on GL/WebGL (no indirect culling) | medium-high | batch-set key `bevy_core_pipeline:src/core_3d/mod.rs:197-220`, bin key the mesh id :232-238; multidraw only under `Culling` (`bevy_render:src/render_phase/mod.rs:2300-2309`); slabs by vertex layout (`bevy_render:src/mesh/allocator.rs:458-469`); `#[bindless]` StandardMaterial (`bevy_pbr:src/pbr_material.rs:22-27`), 2048 resources a slab (`bevy_render:src/render_resource/bindless.rs:25`). Features in `StandardMaterialKey` (alpha mode, which maps) split pipelines, so parts should share one map set. Needs a bench scene before a crowd relies on it |
| 1d | Materials by role | **Yes, as today.** Each glTF primitive is an entity with its material (`bevy_gltf:src/loader/mod.rs:1637-1684`); our `Tint` already swaps materials by the role in `GltfMaterialName` for shared handles (`terra_walker/src/character.rs:84-93, 374-414`). A material a (role, finish) shared by every item; `MeshTag` with an extended material if a per-instance tint is wanted, outside the batch key (`bevy_mesh:src/components.rs:182-208`; we use it in `terra_world/src/parting.rs:244`) | high | as cited. Glbs never share material handles (labelled per file), so the swap to shared handles is required, not optional |
| 2 | The recipe model | **Fits, deterministic to the byte if the assembler is Rust and never Blender.** Recipe = kind, part ids, material, seed; hash = SHA-256 of its canonical bytes with each part's `source_sha` (D552) and the assembler's version | high (determinism), medium (saves, server) | pure f32 maths with `libm` (D30) gives the same bits on one binary; `terra_bake/src/glb.rs:26,49` already reads and writes glb from Rust; `build.ts --again` (`build.ts:311-342`) proves a step's bytes. Blender's glTF round trip is not stable (D563), so Blender only makes a part, once. Saves hold a kind name and wear (`terra_game/src/play/save.rs:112-122`), `Item { id, kind, wear }` (`terra_items/src/lib.rs:33-43`): a recipe field and a save version bump (D353 refuses old saves, no converter). No server crate exists; `terra_items` is already pure functions "so the server can check a player's request" (lib.rs:8-11), and has no dependencies, the right home for recipe checking |
| 3a | Part checker | **Missing; small to medium.** Half exists as measures in `attach_contract.ts` (handle at y=0 `handleAt` :286, head reach :305) and the kit checker's shape (`tools/kits`, D349) | high | — |
| 3b | Envelope refusal | **Missing; small code, numbers unmeasured.** D459's tempo `sqrt(I_ref/I)` is not in code (no hit for an inertia or tempo rule in `crates/`); `items.ron`'s `length`, `mass`, `balance` are typed by hand (`assets/data/items.ron:32-38, 136`). Parts need a mass and centre of mass in their data, written by the part checker | high (code), medium (numbers) | — |
| 3c | The grid | **Exists for one set of clips and two fixed looks; extremes, the edge row, stowing and batching missing.** See "The grid" below | medium | `moves.ts` (814 lines): one move set on 4 templates with a fixed `--hold` list (moves.ts:75-81) |
| 3d | Kind sheets and approval | **Sheets exist in the game's renderer; the differ and the approval store do not.** | high | the workbench lays clips, carry poses and the walker's post-clip layers (58405b33, 3d8a2b6a; `terra_walker::pose::plugin`), one PNG a clip, sides × moments (`terra_sandbox` workbench `sheet.rs:1-23`), about a minute on the Windows GPU (character skill). No tile diff, fault flag or approved-set store anywhere in `tools/` or the sandbox |

## The grid

Measured for this note: `node tools/characters/moves.ts` on main (sword.shielded's 9 moves and 10 other clips, 4 bodies, a sword and a shield) took **28.4 s on one core** (104% CPU, 320 MB at most), as 59445d9d's 27 s; most of it is the bodies' skin-depth search (59445d9d), not the items. The machine has 32 cores, so runs side by side (D406's every-core sweep) are bounded by memory, not time.

Counting today's 13 in-hand classes (`tools/characters/attach_kinds.json`), each checked on its reference and its envelope's two extremes, 4 bodies, about 20 clips a class, held and carried:

| Grid | Cases (item × body × clip) | Runs of 28 s | Serial | 16 side by side (5 GB) |
|---|---|---|---|---|
| Envelope only (the design) | 13 × 3 × 4 × 20 × 2 ≈ 6,200 | 78 | 36 min | 2.5 min |
| One class changed | 3 × 4 × 20 × 2 = 480 | 6 | 2.8 min | 28 s |
| Every pairing (30 a kind), literally | ≈ 62,000 | 780 | 6 h | 23 min |
| Every head on the longest handle, added | + 10 heads a class | + 130 | + 1 h | + 4 min |

So the design's choice holds: motion is checked on the envelope's extremes (and the widest head), the pairings only statically (joins, sizes, envelope), which is milliseconds each. A batch mode in `moves.ts`, many looks measured against one posed body a frame, would make a class's run cost about one run, since the item work is small beside the body's. The grid is a build step a class (D563), stamped by the class's clips, bodies, kind row and envelope parts, so only what a change touches runs.

## Today's tools

| Tool | Under D565 |
|---|---|
| `fit.ts` (224) | **Reused** as the one command for uniques; gains an assembled path that skips detection and Blender |
| `detect.ts` (323) | **Kept for uniques only**: parts carry their points, nothing to detect |
| `fit_held.py` (286) | **Replaced for assembled items** by the assembler, which writes the same contract (grip at origin, +Y up the handle, head to -Z, extras); kept for uniques |
| `haft.py` (59) | **Deleted from the item path** once handles are made by code; kept only while a unique or a world prop needs a source haft thinned |
| `attach_contract.ts` (632) + `attach_kinds.json` | **Reused** on every assembled look; its mesh-measured handle becomes a cross-check of the declared points. `attach_kinds.json` gains `"fits": "parts"`; the spear's known fault (`attach_faults.json`) goes once its second grip is a handle point |
| `grip.ts` (337) and `terra_walker::grip` | **Reused**, taking the handle's line and radius from the item's extras when given: today the line is assumed +Y and the radius the median of a mesh cut within 5 cm of the axis (`grip.rs:112-131`), the one step a code-made handle makes exact |
| `carry_hand.ts`, `carry_arm.ts` | **Reused unchanged**: they read the carry tilt, not the look |
| `shield_place.ts`, `shield_arm.ts`, `shield_path.ts` | **Reused unchanged** (shields' targets are per pose, D554) |
| `held.ts`, `clash.ts` | **Reused**; tip, edge and guard from the parts' points instead of width bands |
| `moves.ts` | **Reused, extended**: extremes as looks, the edge-leads row, stowed, batch mode |
| `glb.ts`, `sockets.py`, `hold.py`, `clear.ts`, `build_item.sh` | Unaffected |

## The simplest pipeline

Three commands and one look a kind. Every step is one command, a line a stage, failing with the number, the bar and the field to change (D388, D556).

| Step | Command (new unless named) | In | Out | Passes when |
|---|---|---|---|---|
| 1. A part | the models skill as today (concept, Tripo, Blender clean-up), once a part; handles from numbers, no model | a part's data: slot, template, points, material roles | `assets/models/parts/<part>.glb`, its data with mass and centre measured | — |
| 2. Check the part | `node tools/items/part.ts <part>` | the part and its kind row | ✓, or the field to change | points where the template says (grip on the centreline within 1 mm), size and budgets in range, roles named, every allowed pairing joins with no gap, burial or shared plane |
| 3. Check the kind | `node tools/items/kind.ts <kind>` (a build step a class, cached) | kinds table, parts, clips, bodies | the extremes and reference assembled, the grid's rows, the kind sheet's changed tiles | every pairing inside the envelope or refused; on the extremes and widest head, every body and clip: fist within 1 cm and 80° (today's row), edge leads (-Z ≥ 0.7 of the motion), held, carried and stowed clear of the body |
| 4. The developers' eye | the desk, the kind sheet's changed tiles beside the art-style checklist | step 3's sheet | an approval stored by the sheet's tile hashes | once a kind, and again only when a part or rule changes |
| In play | none | a recipe from the station, loot or a theme's lists and a seed | the assembler's placements, part scenes spawned on the socket; the icon rendered from the recipe | the assembler refuses what step 3 would |

A new item of an approved kind is a recipe: no command, no picture. A new part runs steps 1 to 3 and shows only the tiles it changed. A new kind is its row, its reference item, its clips (the combat skill's `clip.ts`), then steps 2 to 4.

## Missing or risky

In order of risk; sizes are an agent's jobs, about a day's context each.

1. **The envelope through generated clips (D565 step 3), unproven; medium-large.** No shipped game bakes an item track for clips made without a prop. The proof is the design's: one class, three clips, three handles by three heads, every body. If the extremes fail, the envelope shrinks or the class splits, which is cheap; if the track itself fails, two-handers need a prop bone. 2 to 4 jobs.
2. **Clips per class; large, but per kind, not per item.** Only `sword.shielded.ron` exists (`assets/data/moves/`). Every other class needs its set before its grid means anything. This is the combat work's cost, not this pipeline's.
3. **Checks and game on two pose paths; medium.** The design asks for one (`terra_walker::pose`); `moves.ts` poses in TypeScript twins (`arms.ts`, `grip.ts`, `carry_hand.ts`, `shield_arm.ts`), held to Rust only where a twin test shares the Rust test's case (`arms.test.ts`, `carry_hand.test.ts`). A drift is a check passing what the game draws wrong. Either a twin test for each, or the grid's pose from a Rust command. 1 to 2 jobs.
4. **Icons a recipe; a design question.** Painting an icon per item (`tools/icons/paint.ts`) is free to 100 images a month, then 10 credits each (`tools/icons/README.md:21`): per-item work and cost at thousands of items. A rendered icon from the recipe (`cargo xtask icons`) scales; painting a part or a kind's looks only is the developers' call (art style).
5. **Code-made handles beside modelled heads; small, judged by eye once.** A handle from numbers needs a tileable wood or wrap material in the art style; the kind sheet is where it is seen.
6. **Two-handers' second grip not read in Rust; small.** No Rust reads `second` today (only `attach_kinds.json` names it). A handle point makes it exact; the off hand's IK onto it is D461's work.
7. **The assembler, recipe in data and saves; small to medium.** `terra_items` recipe type and check, the assembler (placements and points), its glb writer on `terra_bake::glb`, the save's version bump, the game spawning parts and sharing materials by role. 2 jobs.
8. **The part checker, envelope rule, `moves.ts` batch and rows, sheet differ and approval store; small each.** 3 to 4 jobs together. The envelope's numbers are measured on the first class, not found anywhere.
9. **Crowds not benched; small.** A bench scene of many players with assembled items, to confirm batching (1c) with real numbers (D185).

## Limits

- The grid's case counts assume 13 classes, about 20 clips a class and 30 pairings a kind; only the sword's clips exist to count.
- Batching was read in Bevy's source, not measured on our scenes.
- The step sizes are estimates from comparable jobs, not measured.
- Uniques stay on today's fitter path (D552) and are not costed here.
