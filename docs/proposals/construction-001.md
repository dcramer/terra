# Construction 001: a voxel core, drawn by its neighbours

Status: proposal, 2026-10-12, for the developers to read and correct before more is built. It sets the rules every construction piece follows, and walks through every case they must handle. It builds on what is decided (D48, D139, D146, D152, D154, D179, D181) and changes none of it except where it says so. The research behind it is [research/adjacency-pieces.md](../../research/adjacency-pieces.md).

## What it answers

The developers, 2026-10-12:

- "we probably need to set some guard rails and rules for how we're going to do the core construction pieces. similar to how enshrouded does"
- "our core needs to be voxels"; Enshrouded's half-metre voxels "change the rendering of the voxel based on its adjacent ones … roof tiles/walls/etc render edges/etc diff … that seems like a really good system if we can adopt it and keep our build ghosts/etc"
- "the log cabin might have too much going on with the logs protruding out … hard to fit in"
- "they can walk through the ghosts, but not the built stuff. it should behave like normal collision"
- "for foundations … we're just gonna force you to sort of build into the ground … enshroudeds solution sucks because it destroys the ground"
- "we use models not geometry thats one of our core art rules"; doors are not part of a plan
- "think this through. all the cases we have to handle then actually design it fully and correctly"

## The rules

1. **The cube is the truth.** A structure is half-metre cubes on its base's grid (D181), each air or a kind of construction, solid, glass or open, and planned or built. Everything else is worked out from them: what is drawn, what collides, what is indoors, what it costs, what goes in next. Only roofs (a few sections, D160) and each part's progress are stored besides.
2. **Nothing leaves its cubes.** Every piece lies inside the cubes it is drawn for, give or take a few centimetres of rough edge. So anything can be built against anything, and what is seen is what collides. The two exceptions reach outward on purpose: a roof's eaves, and footings into the ground.
3. **Any arrangement of cubes is drawn.** A kit cannot refuse a shape: every neighbourhood its cubes can be in has a piece. A player who carves a cross into a wall, stacks a wall two cubes thick or builds an L of floors sees it made, not a gap.
4. **A piece is chosen by its neighbourhood only.** Which piece, which version and which turn come from the cubes round it and a hash of where it is, so the same cubes look the same on every machine, and changing one cube redraws only the pieces touching it (D127).
5. **Each kind draws its own cubes.** Where kinds meet, each ends at the boundary between them, and the heavier one's corner runs through (D179). A kind treats another's cubes as solid for hiding faces, and as empty for choosing its own shape.
6. **Plans are the same cubes, not yet built.** A plan, a building going up and a finished one are one set of cubes in three states, drawn by the same rule: what is in, solid; what is not, as the ghost; the next part lit. Only what is built collides, holds a walker up or encloses a room.
7. **Openings are cubes left open.** A door or window is an item fitted into an opening of its size once the wall is up, never part of a plan (2026-10-12). The kit frames every opening along its edges, whatever its shape.
8. **Roofs are the one surface.** A roof is drawn from its height field, not its cubes, and its pieces are chosen by the field's shape round each point: slope, eave, verge, ridge, hip, valley, where it meets a wall. Walls under it end at its underside.
9. **The ground is not changed by building.** Footings fill the space between the floor and the ground and stop where they meet it, set a few centimetres in so no gap shows; nothing is flattened or filled. Only building below the ground's level, a cellar or a terrace into a slope, digs it, as D131 says (an [open question](#open-for-the-developers)).
10. **Pieces are models** (D177): made by the art pipeline, one concept to a kit, one grade, several versions of every common piece, every side alike (building-kits.md).

## Drawing by neighbours

Two ways, by what a kind is made of, both worked out from the cubes alone:

| | Blocks: stone, brick, plaster, earth, carved rock | Long members: logs, beams, planks, wattle |
|---|---|---|
| How | **The dual grid**: a module at every corner of the grid, chosen by which of the eight cubes round it are full, as Townscaper's | **Runs**: long pieces along each straight run of cubes, with joint pieces chosen at ends, corners, T's and crosses, as the log kit now |
| Gives | Outer and inner corners round or crisp as the kit is made, flat faces where cubes sit side by side: "corners become rounded on the outside and inside, but when two segments are next to each other they're flat" (building.md) | A log as long as the wall it spans, which is what makes a cabin read as logs |
| A kit makes | About a dozen module shapes for walls, posts and floors, of the 53 there are; the rest fall back to the nearest | A run piece in a few lengths and versions, an end, a corner, a frame |
| Carving a cube | The modules round it change | The run splits, its new ends cut |

A building can mix both: logs over a stone plinth is a run kind over a block kind, each drawing its own cubes (rule 5).

**How the modules are made.** A module's edges must meet its neighbours', which generated models never do. So a block kit's modules are clean shapes to shared edge profiles, made by script in Blender and carrying the material's texture and relief, with generated models for what does not tile (doors, windows, trims, corner stones as details). That is how modular kits are made in shipped games, and it is the risk to try first ([Next](#next)).

## Plans, going up, built

| | Plan | Going up | Built |
|---|---|---|---|
| Cubes | All planned | Some built | All built |
| Drawn | The kit's pieces in the ghost material | Built pieces solid, the rest ghost, the next lit | Solid |
| Collides | No | What is built (built 2026-10-12) | Yes |
| Encloses a room | No | No | Yes |
| Doors and windows | Openings only | Openings only | Fitted when made |

**What goes in** is a piece (D160), as now: a log, a stone section, a step. The cubes it fills become built when it goes in, which is what collision reads. A block kit's dual-grid modules straddle cubes, so for them the unit of building is the cube, or a run of a metre: the modules round it redraw as it goes from ghost to solid. Taking apart runs the same in reverse, last in first out.

## The cases

Each row is a case the system must draw, build and collide correctly. Status is today's.

| Case | How the rules handle it | Status |
|---|---|---|
| **Walls** | | |
| A straight run | Runs: pieces along it; blocks: flat-face modules | Built (logs, stone sections) |
| A free end | Runs: cut flush; blocks: end modules | Built |
| An L corner, outside and inside | Runs: courses interlock, ends at the corner cube's face; blocks: outer and inner corner modules | Built for logs (within the grid since 2026-10-12) |
| A T and a cross | Runs: the through wall runs on, the stem ends at its face; blocks: modules | Built for logs |
| A post: a column alone | Runs: an upright; blocks: a column of end modules | Built for logs |
| A wall two or more cubes thick | Blocks: drawn like any solid; runs: one run per row, faces hidden between | Not built |
| Walls of different heights side by side | Each run or module sees its own column; tops drawn where nothing is above | Partly |
| A wall's top with nothing on it | A cap: the top course, or top modules | Built for logs |
| A wall meeting a roof | Ends at the roof's underside: logs shortened (gable logs, trimmed ends), blocks cut by the roof's plane | Built for logs; stone gables not |
| Diagonal and curved walls | Deferred (D146) | |
| **Openings** | | |
| A door or window of the default size | Cubes open or glazed; the kit frames it | Built |
| Any carved shape, at an edge, a notch in the top | The same: framed along its own edges | Built for logs |
| An opening in stone | Lintel over it; blocks frame it with jamb and sill modules | Lintel built; modules not |
| Doors and glass fitted | Items fitted after (rule 7) | Shown only when built |
| **Floors** | | |
| An inside floor | A layer of cubes; boards on top, joists below | Built (log kit's floor) |
| A floor's exposed edge, outside the walls | Its edge pieces, or the footing below covers it | Wrong: the plain edge shows |
| A floor over a wall below, a balcony, a hole for stairs | Modules round each, or edge pieces | Not built |
| **Roofs** | | |
| Gable, hip, lean-to, flat | Sections over the height field (D160) | Built |
| Eaves, verges, ridges, hips | Pieces chosen by the field's shape | Covering built; ridges and hips are plain |
| Valleys, a roof against a taller wall | The field's shape; flashing pieces where it meets a wall | Not built |
| Logs or stone through a roof | Never: cut to its underside | Built for logs |
| **Mixing kinds** | | |
| Logs on a stone plinth | Each kind its own cubes | Built |
| Stone and logs in an L, a T, in line | The heavier's corner runs through; the other ends at it | Built for logs ending at stone |
| A floor or roof of another kind than its walls | Each by its own kit | Roofs take the walls' kit |
| **Ground** | | |
| Level ground | Footing pieces a little into it | Plain blocks stand in |
| A slope | Footings fill down to the ground and stop there, taller where it falls away; the floor at the highest ground (rule 9) | Plain blocks stand in |
| Ground higher than the floor | Open: dig it (D131) or refuse | D131: dug |
| A cellar, a terrace into a hill | Built below ground level: the ground dug there (D131) | Built |
| Ground dug away under a building later | Open: the footing shows, or the building is undermined | Footing shows |
| Door steps | Steps of the kit down to the ground | Plain blocks stand in |
| **Walking** | | |
| Walls, glass, openings | Cubes block, openings pass | Built |
| Ghosts | Walked through (rule 6) | Built |
| Floors, roofs, steps | Stood on | Built |
| Stairs and ladders | Not built (building.md step 4) | |
| **Many** | | |
| Fifty cabins in view | One mesh per building and material, rebuilt only where cubes changed; ghosts instanced; a stand-in far off (D160) | Not measured |

## Player experiences

How the pieces work together, walked through as players will meet them (the developers, 2026-10-12: "just thinking about how all the pieces work together and the different player experiences"). Each says what the player does and sees, and what it asks of the rules; **gaps** are what nothing above answers yet.

**1. The first night: a shelter, fast.** Early on, with wattle and poles. The player draws three short walls and a lean-to roof over them, open on one side. The plan shows its stakes and thatch; they put them in from the pack in a minute or two. Under it they are out of the rain, though it is not indoors (D155), which a closed room would be. *Asks:* small plans must be quick to lay and build, and shelter must count before a room is closed. **Gaps:** whether a roof alone shelters from rain; whether early kinds go up in big parts, not stake by stake.

**2. A log cabin on a slope.** The player draws a 5 by 4 m room with the room shortcut. The plan shows a footing, taller on the downhill side, a floor, four walls log by log with a door and a window cut, and a gable roof. Building goes from the bottom: the footing's stones, then the floor, then the logs, carried one at a time from where the trees fell (D140), then the roof in stages. The door, made at the workbench, is fitted last. *Asks:* the footing laid by the game (question 6); a plan's order the physical one; doors as items. **Gaps:** how a roof goes up in stages; how the next part to build is shown from inside and outside.

**3. Adding on: a stone room against the cabin.** The player draws stone walls meeting the built log wall at a T and an L. The log wall is built, and a built wall never changes under the player (building-tech.md), so the stone ends against the logs here, though the heavier would normally run through. The new roof meets the cabin's wall or roof in a valley. *Asks:* rule 5 gives way to what is built. **Gaps:** a shared wall between two rooms of different kinds; roofs meeting walls and other roofs (D160's hard part).

**4. Changing their mind.** A window in the wrong place: carved out of the built wall a cube at a time, the logs cut and framed round it, the cut logs given back. A whole wall taken down: it comes down last part first, giving back its logs. *Asks:* carving and taking apart drawn by the same rules as building. **Gaps:** what happens to a roof whose wall is taken away, with no stability yet (D142): recommend a wall under a roof cannot be taken down until the roof is.

**5. A cellar.** The player draws walls a storey down, inside the claim. The ground there is dug as the plan is built (D131), and the cubes it no longer fills show the walls; no footing is laid below ground. *Asks:* rule 9's exception. **Gaps:** stairs down (building.md step 4); whether the digging is the player's work or the plan's.

**6. Building together.** One player fells and carries logs; another sets them and hammers them in; a recruit fetches stone (D90). The plan is the same for all; each builds the next part nearest them (building-tech.md), and the claim's list says what is still needed. *Asks:* parts put in by anyone, in any order the physical order allows. **Gaps:** two players reaching for the same part.

**7. A village.** Houses put up by the world, from the same parts and operations (D108): plans built by its people over days, so a village grows as a base does. *Asks:* every rule holds for buildings nobody drew by hand. **Gaps:** the far stand-in for many houses (D160), not measured.

## Open for the developers

1. **Foundations.** Leaning, the developers, 2026-10-12: "ideally foundations just stop at the ground? but im not really sure". So footings of the kit's stone fill the space between the floor and the ground and stop where they meet it, taller where the ground falls away; the ground is left as it is, and the floor sits at the highest ground under the building. Only building below ground level digs (D131). Still to say: whether a tall footing costs more, and what happens when the ground under a building is dug later.
2. **The unit of building.** Recommend a piece for long members (a log), and a cube for blocks, or a metre of them, so a stone wall goes up in about the steps a real one does.
3. **Round or crisp corners**, per kind: the developers asked for rounded outside and inside corners. Recommend crisp for cut stone, round for plaster and earth, as real ones are.
4. **Walls thicker than a cube**, for castles and retaining walls. Recommend yes for block kinds; it costs nothing on the dual grid.
5. **Which kinds are blocks and which are long members.** Recommend stone, brick, plaster and earth as blocks; logs, timber framing, planks and wattle as long members.
6. **Is a footing its own part?** The developers, 2026-10-12: "the footings might need to be a specific kind of building block tho? like a footing or floor? idk". A footing needs no special block: it is cubes of a block kind, stone by default, in the layers under the floor, and one rule makes it stop at the ground: **a cube the ground passes through is drawn down to the ground and no further**, which also closes a floor's edge. What is open is who lays it. Placed by hand first, as Valheim, Rust and Enshrouded do: explicit, but a step to forget, and gaps on slopes until it is done. Laid by the game: The Sims' way (D154). Recommend the game lays it into the plan under every ground-floor wall and floor edge, shown as its own part with its own cost, built first (D139), and changed if wanted: timber posts or stilts instead, or none over a cellar.

## What changes from what is built

- **Logs stay on runs**, now inside the grid. Their footing becomes stone pieces into the ground, and the plain foundation slab and concrete steps go.
- **The stone kit moves to the dual grid**: its metre sections and cornerstones become a dozen modules to shared edge profiles. The Tripo sections made today become the source of its texture and of details that do not tile.
- **Collision by state** stays as built.
- **Floors get edges**, and roofs ridges, hips and valleys, as pieces.

## Next

One question to answer first, as a spike: **can a dual-grid stone kit of a dozen modules, made by script to shared edge profiles and textured from the Tripo stone, look as good as the Tripo sections, with no seams?** Built on the yard's kit sheet, beside today's stone house, and judged by the developers. If it can, block kinds go to the dual grid; if not, they stay on runs and sections, and the rules above still hold.
