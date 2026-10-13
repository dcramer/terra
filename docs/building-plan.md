# Building: the technology plan

How we build the construction system that [proposals/construction-001.md](proposals/construction-001.md) settles (D192 to D194), in the code we have, by AGENTS.md's rules. [building.md](building.md) is the design as the player meets it, [building-tech.md](building-tech.md) the technical design, [building-kits.md](building-kits.md) how each kind looks. This is the plan of work: where each part lives, what it stores, how it is drawn, checked and measured, and in what order. Technical choices are mine (D160, D195); it is current thinking, changed as we learn.

## The bar: on par with Enshrouded (D211)

The developers' agenda, above everything below: building takes **an approach to voxels like Enshrouded's** and is on par with it in how buildings blend together. Not a copy of its look: the style is ours (D14), as are the kits and materials. Cubes stay the truth and stay optimised cubes. Cubes that touch blend into one structure, never a stack of boxes; and each cube's look changes with what it touches and where it sits: a corner, an edge, a top, a base, rough or dressed, against another material, against the ground. Every kind is judged side by side with Enshrouded against this, and whatever falls short is changed, however it is drawn today. Finding the technology is ours: research ([research/voxel-building-look.md](../research/voxel-building-look.md)), then spikes compared with Enshrouded, then the chosen way built, merged per section and measured.

## What it must meet

| From | It asks |
|---|---|
| D211 | On par with Enshrouded: touching cubes blend into one; each cube's look by its neighbours and place; still optimised cubes |
| D192 to D194 | Cubes are the truth, each planned or built to a fill level; nothing leaves its cubes but eaves, footings and drawn-only trims (up to an eighth of a cube); any arrangement drawn; pieces chosen by their neighbours; plans as ghosts, walked through; doors and windows fitted props; nothing floats, and nothing taken while something rests on it; lost support collapses |
| D28, D21, D51, D52 | Shared state in plain crates with no Bevy; every change an operation in the server's order; a thousand players on one server |
| D58, D127, AGENTS.md | ECS, a plugin per feature, small systems; many of everything; work only where something changed; a new kind of thing is data |
| D69, D71 | Every step checkable by an agent: tests, the kit sheet, scripted and live runs; tools for the work we repeat |
| D25, D185 | Measured with `cargo xtask bench`, on the fast GPU and an integrated one, in the loop a player runs |
| D14, D177, D197, D200 | Props and trims models from the art pipeline; block kinds their cubes' surface and timber generated long members, both from real materials; one grade a kit |

## Where it lives

**`terra_build`**, shared, with no Bevy (D28), a module a concern. `cubes`, `ops`, `ground`, `support`, `rooms`, `cost`, `walk` and `save` are built (step 1, D199); `kit`, `layout/runs`, `layout/trims`, `layout/ground`, `layout/floors`, `layout/blocks` and `layout/roofs`, `timber`, `surface` and `roofing` are built (step 2, under way); `sections` is built (step 3, D213); `layout/roles`, `layout/dressing`, `layout/stairs` and `dress` are built (step 2a, D215); `layout/shapes` is built (step 5, D220):

| Module | Holds |
|---|---|
| `cubes` | The cube store: sections of 16 cubes a side (8 m), each a dense array of packed cubes, kept in a sorted map, with a dirty mark per section |
| `ops` | The operations, what each needs to be valid, and each one's inverse for undo |
| `ground` | The ground's height under each column, sampled into a plan when it is laid; the footings under a building, down to it (D206) |
| `support` | What holds what: joined to the ground, and what rests on a cube |
| `rooms` | Rooms and indoors, worked out per building (today's, moved) |
| `cost` | What each cube costs and what went into it, in exact fractions |
| `kit` | A kit's data: its pattern, sizes, timber, materials, props and cost; checked as it is read |
| `layout/runs` | Long members: logs, beams, planks, wattle, laid along runs |
| `timber` | A long member made from its piece: a profile (round, hewn) swept along it, shaped by the kit and its seed, displaced by its material's height (D200) |
| `layout/blocks` | Block kinds drawn as their cubes' surface, rounded and displaced by their material (`surface`, D197) |
| `layout/trims` | A timber kind's frames round its openings, posts up their sides, the doors and windows set in them, and end boards where it ends against another kind |
| `layout/roles` | Every exposed edge's role, from the cubes and the ground's samples, never stored: arris, inside corner, jamb, crest, nose, sill, head, soffit, foot, border, meet (D215) |
| `layout/dressing` | Each role's piece as the kit dresses it: a profile along a cube's edge, corner stones up it |
| `layout/stairs` | A timber kind's flights laid as steps: logs across, a log along each nose, treads, stringers; a column topped by a stair two treads, by a slope a ramp of boards (D220) |
| `layout/shapes` | A timber kind's halves as low decks, and its posts as uprights on a pad under a bolster (D220) |
| `dress` | A kit's dress drawn: profiles swept, mitred, jointed and capped; corner stones placed; the masks of dirt and wear |
| `layout/floors` | A timber kind's floors and flat roofs as decks: boards, rim beams, joists (D206) |
| `layout/roofs` | Roof pieces from the height field: slope, eave, verge, ridge, hip, valley, flashing |
| `layout/ground` | Door steps down to the ground (D206); footings are cubes, laid by `ground` |
| `walk` | Walking: each cube to its fill, and to its shape's top under the walker's feet: a stair's steps, a slope's ramp, a half (D220) |
| `save` | The snapshot format, and conversion of older saves |
| `sections` | A section laid alone and drawn: its pieces and how far each is in, merged by kind and material near, baked into one coarse surface far off and coarser further, the plan's pieces as ghosts; which sections a change reaches |

A layout is a pure function of a section of cubes and what reaches it (the whole of a row of logs, a roof or a deck that crosses into it), the kits and the base's seed (`layout::lay_section`). Every piece belongs to one section, the one the cube its middle is in (`Piece::anchor`), so a log crossing a section's edge is laid by one section only, and the sections laid one by one are exactly the base laid whole, held by a test. Its output is **piece instances in grid units**: a role (a log of 3 m, an outer corner's eighth, a ridge cap), a version and a place. No model sizes, so the server needs no art, and nothing floating that machines could disagree on. Versions come from a fixed hash of the base's seed and the cube's place, held to known values by a test.

**`terra_game`**, the Bevy side, as plugins under `build/`, each owning its components and systems (`build.rs` split the same way; `store`, `layout`, `draw`, `maps` and `light` built, step 3):

| Plugin | Does |
|---|---|
| `store` | A base is an entity; its sections are child entities with a `Section` component; a change sets the sections it reaches waiting |
| `layout` | Lays and merges waiting sections on the async compute pool, as many at once as it has threads, nearest the camera first |
| `draw` | Puts each section on screen a few a frame: a mesh per kind and material near, the plan's pieces as ghosts and the next part lit, far off one baked mesh and further a coarser one, with `VisibilityRange`; near detail only near the camera |
| `models`, `maps` | Load each kit's door and window models (`pieces`), and the height and colour maps its pieces are made with on the CPU (`maps`) |
| `props` | Doors and windows: entities fitted into holes, opening, shutting, lit within, and later broken and mended |
| `act` | Laying plans, building by fill (hold the key), taking apart with the reason it is refused, undo, fitting props; driven by the hammer's existing aim and act |
| `light` | Rooms dim the sky's light from inside (D155) |
| `sheet` | The kit sheet: a row for every case in construction-001's table, in every state |
| `village` | Fifty buildings of every kind, roof and plan, for the `village` bench |

**Networking** (track D, `terra_net`, not started) carries the operations as they are. Nothing here waits for it, and nothing here may assume a client is also the server.

## What is stored

| | Stored as | Size |
|---|---|---|
| A cube | Packed in 32 bits: kind (12), shape (4), fill (4), planned or built (1), a change pending (3), the part it was laid in (8) | A section is 16 KB dense, a cabin two or three sections |
| A roof | Its sections (as now), and each roof cell's stage: frame, battens, covering | Bytes a cell |
| The ground under a plan | A height per column, in centimetres, sampled when it is laid | 2 bytes a column |
| A fitted prop | Its kind, its hole, and whether it is open | An operation |
| A base | A snapshot of its sections and roofs, plus the operations since, folded in now and then | |
| A village | A seed and the generator that makes its plan (`terra_build::village`, D229); only what players change is stored (D108) | |

Iteration is always in sorted order, so every machine lays and floods the same way.

## Operations

`LayPlan` (a part's cubes, with ground samples), `Fill` (a cube a step), `Empty`, `SetShape`, `Replace` (a change of kind, pending), `Carve`, `AddRoof`, `RoofStage`, `FitProp`, `RemoveProp`, `SetOpen`. Each is checked by one pure function against the state when it arrives: in the claim, within the plan budget, joined to the ground, nothing resting on what is taken, no body in the way, materials there. Undo is the operation's inverse, checked the same way. The part a player is building is held for them for a few seconds, so two players do not take the same one. A refusal says why, the same words the ghost shows.

## Drawing

As built in step 3 (D213):

1. **Laying**: a change marks its sections (a cube's own, and its neighbours' when it is on an edge), and every section of the building they are in waits to be laid again, as a layout reaches across a building: a log's joins along its wall, a roof's kind from the walls under it. Sections are laid and merged on the workers, nearest the camera first; one that lays as it was, its pieces and their states the same (`Laid::key`), is left as it is drawn, so a change merges again only what it changed. A test checks a change never alters a section it did not set waiting.
2. **Merging**: built pieces become a mesh per section, kind and material, their long members' and block surfaces' corners shared, uploaded a few a frame. A section's block surface is worked out with the cubes a step past its edge, so it meets its neighbour's with no seam. Texture arrays per material were not needed: near, a section is ten or so draws, and far off one.
3. **Ghosts**: a plan's pieces are a unit box or eight-sided prism each, stretched over the piece, all in two ghost materials (`shaders/ghost.wesl`: a faint blue, each face's edges drawn as fine lines, its outline brighter), so Bevy batches them and a plan reads as its pieces, not a pale sheet; a block kind's plan is a plain shell lined at its cubes. The next part is drawn as it will be, lit. Ghosts are seen to 90 m.
4. **Detail by distance**: every piece as it is made to 25 m; to 60 m, still textured, members of eight sides with rings 40 cm apart, the stone's surface in four steps a face and half displaced, no battens; past 60 m one mesh a section: members of six sides with a ring at each end only, the stone in two steps, roofs a plain covering, no battens or rafters, its textures baked into its corners' colours from each material's colour and occlusion (as Unreal's HLOD bakes them); past 180 m coarser again, walls, floors and roofs only, to 600 m; each fading into the next with `VisibilityRange`. Finer detail is made only for sections near the camera (all of it within 40 m, kept to 55 m; textured within 95 m, kept to 140 m), so the far majority of a village costs the workers and the GPU little.
5. **Light**: a room closed all round and covered holds a light probe of the sky's light dimmed (D155), which Bevy uses in place of the sky inside it; a probe lit from the room itself later.

Long members are made to their length (D200), their texture repeating along them and cut faces showing end grain, so nothing is stretched. Block kinds are no pieces: their cubes' surface is meshed per section, rounded and displaced by their material (D197), finer near the camera and coarser far off.

## Collision

First, as now: arithmetic on the cubes, each to its fill and its kind's slab, stairs and slopes from the shapes' table, props by their boxes. When collision is designed (D53, a doc first, as engine.md says), each section also gives boxes merged greedily from its built cubes, rebuilt when it changes; nothing else changes.

## The art pipeline

- **Kits are data**: a file per kind in `assets/data/kits/`, its pattern, roles and their models, textures and sizes (as now).
- **Long members** (D200): generated from the kit file, a profile swept along each piece; their materials (bark, end grain, daub) from the library like the blocks', the bark's height read on the CPU to displace it.
- **Blocks**: a material each, from a library or generated (D191): colour, normal, roughness and the height the surface is displaced by (D197).
- **Levels of detail**: `refine` writes each model at three budgets, for near, middle and far.
- **A kit checker**, `cargo xtask kits`: every role a kit's pattern needs has a model; edges match the shared profile; sizes and budgets within the style guide's; textures present. Run by `cargo xtask check` on the kit files, and on the models when they change.

## Checking it

- **Tests** in `terra_build`, no GPU: each operation's rules; layouts against known cases; seeds against known values; support and rooms on L, T, U and courtyard plans.
- **Property tests over random operations**, as `terra_items` has: after every step, nothing floats, nothing taken while held, refunds exact, layouts the same laid twice, undo returns the state. A failure names its seed and step.
- **The kit sheet** shows every case in construction-001's table in each state, a vantage per row, for screenshots.
- **Live**, over the remote (D69): commands to lay a plan, fill, take apart, fit a prop and read a base's state, so a script builds a cabin from plan to door and walks in, as `cargo xtask loop` plays the loop.
- **Measured**: a `village` scene in `cargo xtask bench`, fifty buildings of mixed kinds, roofs and plans in view, against `village-bare`, the same view with nothing built, on the fast GPU and the integrated one (D25, D185). The bars: no frame above the yard's for the same view; a section laid and merged in under 2 ms on a worker; an operation applied and checked in under 0.1 ms.

## Order

Each step ends checked by script before the next (D69), and is committed as it goes.

0. **Spike: block modules** (done, 2026-10-12, [research/block-modules.md](../research/block-modules.md)). Conformed Tripo pieces read as a grid and break at carved shapes; the cubes' surface, bevelled and displaced by a real stone material, reads as one natural wall and carves cleanly. Decided (D197) and built in `terra_build::surface`: a mesher for that surface, per section on workers, as the terrain's: outer faces, bevels, subdivision near the camera, the material's height displaced on the CPU, triplanar texturing.
1. **The model** (done, 2026-10-12, D199). The cube store in sections; fill, shape and state per cube; operations with their checks and inverses; ground sampled into plans; support; snapshots; older saves converted. Property tests. The game unchanged on top: it still builds with the shortcuts, instantly, and saves them, until step 4.
2. **Layouts** (under way, 2026-10-13). Done: `logs.rs` split into `layout/runs`, `trims`, `ground`, `floors` and `blocks`; timber generated from the kit (D200, `timber`), A as the log kit and B a second profile, the Tripo logs gone; footings, floors and door steps (D206): footings as stone cubes down to the ground, a block kind's floors as its surface with flagstone tops, a timber kind's as decks of boards, rims and joists, stone steps from each door; the grey slab gone. Pieces are placed in the grid, in ticks of a 256th of a cube (D208), not metres. Roofs are a kit's pieces (D209, `layout/roofs`, `roofing`), with gables of logs or stone up to them, and a third kit, hewn timber under thatch. Trims round openings and where kinds meet, and the kit checker, `cargo xtask kits` (D210).
2a. **The look, on par with Enshrouded (D211).** Research how Enshrouded and the best grid builders blend touching cubes and change each cube's look by its neighbourhood (done, 2026-10-13, [research/voxel-building-look.md](../research/voxel-building-look.md): world-space face fill, plus a role for every exposed face, edge and corner choosing material layers, shader masks, swept profiles and corner pieces); spike the candidate ways on stone, logs and roofs, shown side by side with Enshrouded; build the one that meets the bar for every kind, in place of today's drawing where it falls short. Steps 3 and 5 take whatever it gives. Then, from the parity audit ([research/enshrouded-parity.md](../research/enshrouded-parity.md)) and D226 to D228: a material picker, more shapes and replace in place; brushes, carving and taking out the cube aimed at; roofs as cubes built by hand, ridges and eaves from neighbours, with "a roof of this material" laying them; terrain fill and flatten; earth and wattle and daub kits. Spiked (2026-10-13, [research/role-pieces.md](../research/role-pieces.md)) and built (D215): every exposed edge dressed by its role, worked out from the cubes and the ground when a section is laid; the stone kit with profiles and corner stones made in Blender (`tools/kits`), a plinth stepping with the ground and the dirt rising from it; the timber kits with plates, sills, posts and stairs by role; Keen's gallery piece in each kind on the kit sheet; roofs take the masks too, greened toward the eaves, greyed toward the ridge and worn along their edges (D220). Next: a refined kit's module fill.
3. **Drawing** (built, 2026-10-13, D213). Sections as entities, laid and merged on the worker pool, merged meshes, ghosts as blueprints, detail by distance with baked far meshes, indoor light probes; the entity per piece and whole-base relayout gone. The `village` bench, measured on both GPUs ([research/building-drawing.md](../research/building-drawing.md)): operations meet their bar; sections with buildings in them take longer than 2 ms to lay and merge, and the integrated GPU is held up by the buildings' triangles, both left to do.
4. **Plans and building** (built, 2026-10-13, D201). Laying plans with the tools there are; building by fill; taking apart and undo with their refusals; the plan budget; doors and windows fitted, opening and shutting. The remote commands (`build.*`), and a scripted cabin from plan to door, `cargo xtask cabin`. Since: footings laid into plans and built first (D206); openings cut into walls already built, under the wall over them, and doors whose leaf swings in a fixed frame (D207).
5. **Shapes and more parts** (built, 2026-10-13, D220): stairs, slopes, halves and posts (D193), placed with the hammer (a flight of stairs or a ramp up a storey, a post, a block of any shape, the wheel turning which way it rises), walked up, and drawn in every kit: stone cut to the shape with a nose on each tread and copings on halves and slopes, pillars of drums; timber as flights of logs and treads or ramps of boards, low decks and posts under bolsters. A timber kind's solid mass is a framed deck, not a pile of log ends. Floors drawn alone, with the hammer's floor, their edges finished. The kit sheet's rows of every shape in each kit, built, going up and planned, and `cargo xtask cabin` building a deck on posts and walking up its stairs.
6. **Loss and repair**: collapse into debris with attacks (D90), repair.
7. **The rest of D139 to D141**: carrying big materials, building from storage, recruits building.
8. **Networking**, when `terra_net` comes: the operations as they are, snapshots for late joins.

## Where the risk is

| Risk | What we do |
|---|---|
| A block kind's surface costs too many triangles near and far | Finer near the camera, coarser far off, per section; measured on the `village` bench |
| A castle's section rebuilds too slowly | Section size tuned by the bench; the merge on workers |
| The ghost of a large plan costs too much to draw | Instancing, plans drawn only near and to the tribe, a budget per claim |
| Collision is undesigned (D53) | Cubes' arithmetic serves until its doc; sections give boxes when it comes |
| Bevy's render API moves under us (D184) | Merging and instancing in our own module, on Bevy's public mesh API |
| `terra_build` grows into a monolith again | The modules above, each with its own tests |
