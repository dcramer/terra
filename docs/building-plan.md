# Building: the technology plan

How we build the construction system that [proposals/construction-001.md](proposals/construction-001.md) settles (D192 to D194), in the code we have, by AGENTS.md's rules. [building.md](building.md) is the design as the player meets it, [building-tech.md](building-tech.md) the technical design, [building-kits.md](building-kits.md) how each kind looks. This is the plan of work: where each part lives, what it stores, how it is drawn, checked and measured, and in what order. Technical choices are mine (D160, D195); it is current thinking, changed as we learn.

## What it must meet

| From | It asks |
|---|---|
| D192 to D194 | Cubes are the truth, each planned or built to a fill level; nothing leaves its cubes; any arrangement drawn; pieces chosen by their neighbours; plans as ghosts, walked through; doors and windows fitted props; nothing floats, and nothing taken while something rests on it; lost support collapses |
| D28, D21, D51, D52 | Shared state in plain crates with no Bevy; every change an operation in the server's order; a thousand players on one server |
| D58, D127, AGENTS.md | ECS, a plugin per feature, small systems; many of everything; work only where something changed; a new kind of thing is data |
| D69, D71 | Every step checkable by an agent: tests, the kit sheet, scripted and live runs; tools for the work we repeat |
| D25, D185 | Measured with `cargo xtask bench`, on the fast GPU and an integrated one, in the loop a player runs |
| D14, D177 | Every piece a model from the art pipeline, textured, one grade a kit |

## Where it lives

**`terra_build`**, shared, with no Bevy (D28). Today it is `lib.rs` (the base, operations, rooms), `logs.rs` (1,800 lines), `masonry.rs`, `mesh.rs` and `walk.rs`. It is split before it grows further, a module a concern:

| Module | Holds |
|---|---|
| `cubes` | The cube store: sections of 16 cubes a side (8 m), each a dense array of packed cubes, kept in a sorted map, with a dirty mark per section |
| `ops` | The operations, what each needs to be valid, and each one's inverse for undo |
| `ground` | The ground's height under each column, sampled into a plan when it is laid |
| `support` | What holds what: joined to the ground, and what rests on a cube |
| `rooms` | Rooms and indoors, worked out per building (today's, moved) |
| `cost` | What each cube costs and what went into it, in exact fractions |
| `kit` | A kit's data: its pattern, sizes, pieces by role, textures; checked as it is read |
| `layout/runs` | Long members: logs, beams, planks, wattle, laid along runs (today's `logs.rs`, split) |
| `layout/blocks` | Block kinds on the dual grid, from eighth-cube parts (or today's sections, if the spike says so) |
| `layout/trims` | Jambs, sills and lintels round every opening, and caps where thicknesses differ |
| `layout/roofs` | Roof pieces from the height field: slope, eave, verge, ridge, hip, valley, flashing |
| `layout/ground` | Footings down to the ground, and door steps |
| `walk` | Walking: each cube to its fill, as its kind's slab, and the shapes' table |
| `save` | The snapshot format, and conversion of older saves |

A layout is a pure function of a section of cubes, its neighbours' edges, the kits and the base's seed. Its output is **piece instances in grid units**: a role (a log of 3 m, an outer corner's eighth, a ridge cap), a version and a place. No model sizes, so the server needs no art, and nothing floating that machines could disagree on. Versions come from a fixed hash of the base's seed and the cube's place, held to known values by a test.

**`terra_game`**, the Bevy side, as plugins under `build/`, each owning its components and systems (`build.rs`, 1,330 lines today, split the same way):

| Plugin | Does |
|---|---|
| `store` | A base is an entity; its sections are child entities with a `Section` component; applying an operation marks sections dirty |
| `layout` | Lays dirty sections on the async compute pool, at most a few a frame, nearest first |
| `draw` | Merges each section's built pieces into one mesh per material; draws planned pieces as instanced ghosts and the next part lit; far off, a mesh per section of the pieces' lowest detail, with `VisibilityRange` |
| `models` | Loads each kit's models and their levels of detail, keyed by role and version |
| `props` | Doors and windows: entities fitted into holes, opening, shutting, lit within, and later broken and mended |
| `act` | Laying plans, building by fill (hold the key), taking apart with the reason it is refused, undo, fitting props; driven by the hammer's existing aim and act |
| `light` | Rooms dim the sky's light from inside (D155) |
| `sheet` | The kit sheet: a row for every case in construction-001's table, in every state |

**Networking** (track D, `terra_net`, not started) carries the operations as they are. Nothing here waits for it, and nothing here may assume a client is also the server.

## What is stored

| | Stored as | Size |
|---|---|---|
| A cube | Packed in 32 bits: kind (12), shape (4), fill (4), planned or built (1), a change pending (3), the part it was laid in (8) | A section is 16 KB dense, a cabin two or three sections |
| A roof | Its sections (as now), and each roof cell's stage: frame, battens, covering | Bytes a cell |
| The ground under a plan | A height per column, in centimetres, sampled when it is laid | 2 bytes a column |
| A fitted prop | Its kind, its hole, and whether it is open | An operation |
| A base | A snapshot of its sections and roofs, plus the operations since, folded in now and then | |
| A village | A seed and the generator that makes its plan; only what players change is stored (D108) | |

Iteration is always in sorted order, so every machine lays and floods the same way.

## Operations

`LayPlan` (a part's cubes, with ground samples), `Fill` (a cube a step), `Empty`, `SetShape`, `Replace` (a change of kind, pending), `Carve`, `AddRoof`, `RoofStage`, `FitProp`, `RemoveProp`, `SetOpen`. Each is checked by one pure function against the state when it arrives: in the claim, within the plan budget, joined to the ground, nothing resting on what is taken, no body in the way, materials there. Undo is the operation's inverse, checked the same way. The part a player is building is held for them for a few seconds, so two players do not take the same one. A refusal says why, the same words the ghost shows.

## Drawing

1. **Laying** a dirty section gives its piece instances. Changing one cube marks its section, and its neighbours when it is on an edge.
2. **Merging**: built instances become one mesh per section and material, on a worker thread, uploaded a few a frame. A kit's textures are an array per material, so a section is a few draws whatever its kinds.
3. **Ghosts**: planned instances are drawn instanced in the ghost material, Bevy batching the same mesh and material; the next part lit. Plans are sent only to the claim's tribe, near (D193).
4. **Far off**: each section's lowest-detail pieces merged, faded in with `VisibilityRange`; further, a box per section in its kits' colours, later an impostor.
5. **Light**: the sky's light dimmed indoors, from rooms; a probe per room later.

Long members come in a few lengths with their texture repeating along them, cut faces showing end grain, so nothing is stretched. Block kinds draw from about six eighth-cube parts per kit, a few versions each, which cover every arrangement.

## Collision

First, as now: arithmetic on the cubes, each to its fill and its kind's slab, stairs and slopes from the shapes' table, props by their boxes. When collision is designed (D53, a doc first, as engine.md says), each section also gives boxes merged greedily from its built cubes, rebuilt when it changes; nothing else changes.

## The art pipeline

- **Kits are data**: a file per kind in `assets/data/kits/`, its pattern, roles and their models, textures and sizes (as now).
- **Long members**: Tripo models per role, logs in lengths, refined along their length (as now).
- **Blocks**: Tripo models conformed by `refine`: cut to their cell, edges feathered to the kit's shared profile, the seam set in a joint, every side evened to its front's grade (built) and squared (built).
- **Levels of detail**: `refine` writes each model at three budgets, for near, middle and far.
- **A kit checker**, `cargo xtask kits`: every role a kit's pattern needs has a model; edges match the shared profile; sizes and budgets within the style guide's; textures present. Run by `cargo xtask check` on the kit files, and on the models when they change.

## Checking it

- **Tests** in `terra_build`, no GPU: each operation's rules; layouts against known cases; seeds against known values; support and rooms on L, T, U and courtyard plans.
- **Property tests over random operations**, as `terra_items` has: after every step, nothing floats, nothing taken while held, refunds exact, layouts the same laid twice, undo returns the state. A failure names its seed and step.
- **The kit sheet** shows every case in construction-001's table in each state, a vantage per row, for screenshots.
- **Live**, over the remote (D69): commands to lay a plan, fill, take apart, fit a prop and read a base's state, so a script builds a cabin from plan to door and walks in, as `cargo xtask loop` plays the loop.
- **Measured**: a `village` scene in `cargo xtask bench`, fifty cabins of mixed kinds in view, on the fast GPU and the integrated one (D25, D185). The bars: no frame above the yard's for the same view; a section laid and merged in under 2 ms on a worker; an operation applied and checked in under 0.1 ms.

## Order

Each step ends checked by script before the next (D69), and is committed as it goes.

0. **Spike: block modules.** Can Tripo's stone, conformed to shared edges, look as good as the stone sections, with no seams? A straight wall, a carved cross, a stone and log join, a wall part-built, on the kit sheet beside today's stone house. The developers judge. If not, block kinds keep sections; nothing else changes.
1. **The model.** The cube store in sections; fill, shape and state per cube; operations with their checks and inverses; ground sampled into plans; support; snapshots; older saves converted. Property tests. The game unchanged on top.
2. **Layouts.** `logs.rs` split into runs, trims and ground; block layout from the spike's answer; roof pieces; all in grid units; the kit checker. The kit sheet covers every case.
3. **Drawing.** Sections as entities, layouts on the worker pool, merged meshes, instanced ghosts, far meshes, indoor light. The entity per piece goes. The `village` bench, measured on both GPUs.
4. **Plans and building** (D139). Laying plans with the tools there are; building by fill; taking apart and undo with their refusals; the plan budget; doors and windows fitted, opening and shutting. The remote commands, and a scripted cabin from plan to door.
5. **Shapes and more parts**: stairs, slopes and halves (D193); floors drawn alone; posts.
6. **Loss and repair**: collapse into debris with attacks (D90), repair.
7. **The rest of D139 to D141**: carrying big materials, building from storage, recruits building.
8. **Networking**, when `terra_net` comes: the operations as they are, snapshots for late joins.

## Where the risk is

| Risk | What we do |
|---|---|
| Conformed modules show their seams | The spike first; sections are the fallback |
| A castle's section rebuilds too slowly | Section size tuned by the bench; the merge on workers |
| The ghost of a large plan costs too much to draw | Instancing, plans drawn only near and to the tribe, a budget per claim |
| Collision is undesigned (D53) | Cubes' arithmetic serves until its doc; sections give boxes when it comes |
| Bevy's render API moves under us (D184) | Merging and instancing in our own module, on Bevy's public mesh API |
| `terra_build` grows into a monolith again | The modules above, each with its own tests |
