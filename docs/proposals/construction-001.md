# Construction 001: a voxel core, drawn by its neighbours

Status: adopted on 2026-10-12 (D192, D193, D194). Block kinds are drawn as their cubes' own displaced surface rather than on the dual grid (D197, [Next](#next)); where this page says dual grid or modules for blocks, read that. It sets the rules every construction piece follows, and walks through every case they must handle. It builds on what is decided (D48, D139, D146, D152, D154, D179, D181) and changes none of it except where it says so. The research behind it is [research/adjacency-pieces.md](../../research/adjacency-pieces.md).

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
9. **The ground is not changed by building.** Footings fill the space between the floor and the ground and stop where they meet it, set a few centimetres in so no gap shows; nothing is flattened or filled outside a building. Inside its footprint, ground above the floor is cut down to it, and building below the ground's level, a cellar or a terrace into a slope, digs it (D131, narrowed by D193).
10. **Pieces are models** (D177): made by the art pipeline, one concept to a kit, one grade, several versions of every common piece, every side alike (building-kits.md).
11. **Nothing floats** (the developers, 2026-10-12: "we also want the structure system so you cant just build floating buildings"). A built cube must be joined, through built cubes, to the ground: a cube the ground passes through, or a footing that reaches it. A roof stands on the walls under its edges. A plan may float, as a ghost, but a part is built only once what it rests on is, and nothing is taken out, or the ground dug from under it, while anything rests on it or is attached to it (D193). One flood fill over a building's built cubes when one changes (D127). This is not load or span, which stay for later (D142): a ten-storey tower on one post is allowed, a house in the air is not.

## Drawing by neighbours

Two ways, by what a kind is made of, both worked out from the cubes alone:

| | Blocks: stone, brick, plaster, earth, carved rock | Long members: logs, beams, planks, wattle |
|---|---|---|
| How | **The dual grid**: a module at every corner of the grid, chosen by which of the eight cubes round it are full, as Townscaper's | **Runs**: long pieces along each straight run of cubes, with joint pieces chosen at ends, corners, T's and crosses, as the log kit now |
| Gives | Outer and inner corners round or crisp as the kit is made, flat faces where cubes sit side by side: "corners become rounded on the outside and inside, but when two segments are next to each other they're flat" (building.md) | A log as long as the wall it spans, which is what makes a cabin read as logs |
| A kit makes | About a dozen module shapes for walls, posts and floors, of the 53 there are; the rest fall back to the nearest | A run piece in a few lengths and versions, an end, a corner, a frame |
| Carving a cube | The modules round it change | The run splits, its new ends cut |

A building can mix both: logs over a stone plinth is a run kind over a block kind, each drawing its own cubes (rule 5).

**How the modules are made.** A module's edges must meet its neighbours', which generated models never do on their own. So each is a Tripo model conformed in the clean-up: cut to its cell, its edges feathered to a shared profile, the seam set in a mortar joint, which is how modular kits hide their seams ([gap 30](#gaps-and-how-to-close-them)). It is the risk to try first ([Next](#next)).

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
| Any carved shape, at an edge, a notch in the top | The same: framed along its own edges | Built for logs; cut into a wall already built, under the wall over it (D207) |
| An opening in stone | Lintel over it; blocks frame it with jamb and sill modules | Lintel built; modules not |
| Doors and glass fitted | Items fitted after (rule 7) | Shown only when built |
| **Floors** | | |
| An inside floor | A layer of cubes; boards on top, joists below | Built (D206): timber decks, stone flagstones |
| A floor's exposed edge, outside the walls | Its edge pieces, or the footing below covers it | Built (D206): a timber deck's rim beam; stone's own surface |
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
| Level ground | Footing pieces a little into it | Built (D206): a course of stone cubes |
| A slope | Footings fill down to the ground and stop there, taller where it falls away; the floor at the highest ground (rule 9) | Built (D206) |
| Ground higher than the floor | Open: dig it (D131) or refuse | D131: dug |
| A cellar, a terrace into a hill | Built below ground level: the ground dug there (D131) | Built |
| Ground dug away under a building later | Open: the footing shows, or the building is undermined | Footing shows |
| Door steps | Steps of the kit down to the ground | Built (D206): stone slabs |
| **Support** (rule 11) | | |
| A wall, floor or post on the ground | Joined to the ground: built | Not checked |
| A floor or wall built off something not yet built | Refused until what it rests on is in; the plan shows what is first | Not checked |
| A balcony, a cantilever | Allowed while joined; how far it may reach waits for load (D142) | |
| Taking out a part that holds others up | Refused, saying which parts rest on it | Not checked |
| Digging the ground from under a building | Refused in a claim while something rests on it | Not checked |
| A roof whose walls are taken away | The walls cannot be taken until the roof is | Not checked |
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

**4. Changing their mind.** A window in the wrong place: carved out of the built wall a cube at a time, the logs cut and framed round it, the cut logs given back. A whole wall taken down: it comes down last part first, giving back its logs; if the roof rests on it, the roof must come down first (rule 11). *Asks:* carving and taking apart drawn by the same rules as building.

**5. A cellar.** The player draws walls a storey down, inside the claim. The ground there is dug as the plan is built (D131), and the cubes it no longer fills show the walls; no footing is laid below ground. *Asks:* rule 9's exception. **Gaps:** stairs down (building.md step 4); whether the digging is the player's work or the plan's.

**6. Building together.** One player fells and carries logs; another sets them and hammers them in; a recruit fetches stone (D90). The plan is the same for all; each builds the next part nearest them (building-tech.md), and the claim's list says what is still needed. *Asks:* parts put in by anyone, in any order the physical order allows. **Gaps:** two players reaching for the same part.

**7. A village.** Houses put up by the world, from the same parts and operations (D108): plans built by its people over days, so a village grows as a base does. *Asks:* every rule holds for buildings nobody drew by hand. **Gaps:** the far stand-in for many houses (D160), not measured.

## Gaps, and how to close them

Found by reading this proposal against the building docs, the decisions and the code, with a second, adversarial review (2026-10-12). Each gap has the most obvious or best answer. **Mine** marks a technical answer, decided here (D160 updated); the design calls were the questions below, all now decided.

**The model**

1. **What is stored was said two ways.** Rule 1 stores cubes; D160 stores parts, frozen layouts and a bit per piece, and the late corner needs history the cubes do not keep. *Answer:* each cube stores its kind, its shape and its state: air, planned, or built to a **fill level** (logs fill a cube in two courses, a block in one), with at most one change pending on it (replace, carve, take away). Cost and progress are per cube; a part is only how the interface groups cubes. Layouts always follow the current kit, so a kit tuned later redraws what is built with nothing lost, and a corner looks the same whichever wall went up first (D122: fun over history). D160's frozen layouts and kit versions go. *Mine.*
2. **Pieces and cubes did not line up**: one log spans eight cubes but half a cube's height, so its first course made eight cubes solid to full height. *Answer:* the fill level: a cube collides from its foot to its fill, so a wall with one course in is a quarter of a metre high to a walker. *Mine.*
3. **Where the ground's height comes from.** Footings and support need it the same on every machine, and the ground is smooth, edited, and round. *Answer:* sampled when a plan is laid, stored per column in the plan's operation, rounded, and sampled again only when a terrain operation touches that column. A kit caps how tall a footing grows; beyond it, the game offers posts or stilts. *Mine.*
4. **The operation log grows forever**, and a late join replays months of it. *Answer:* a base is saved and sent as a snapshot of its cubes and roofs, plus the operations since, as D21 already says for terrain. *Mine.*
5. **The server would need the art** if layouts used models' sizes. *Answer:* layouts, costs, the next part and support are worked out in grid units from kit data only; models are the client's. *Mine.*
6. **The same seed makes identical villages** (against D182), and a hash that changes between Rust versions would change every building. *Answer:* the base's seed with each cube's place, through a fixed hash checked by tests of known values. *Mine.*

**Drawing**

7. **Rule 3 and "a dozen modules, the rest fall back" disagree**: a fallback cracks or overlaps. *Answer:* modules made of eighth-cube parts, as Townscaper's corners and RPG Maker's quarter tiles: about six shapes cover all 256 cases. *Mine.*
8. **Openings on the dual grid**: a module cannot tell a framed opening from a wall's end, and every state of a cube multiplies the cases. *Answer:* the dual grid sees only solid or not; jambs, sills and lintels are a second pass of trims along each opening's edges, as the log kit frames them now. *Mine.*
9. **Kinds of different thickness leave holes**: logs (0.3 m) on stone (0.5 m) open a 10 cm ledge into the stone. *Answer:* a face is hidden only where the neighbour's profile covers it; otherwise it gets a cap or sill, as a real plinth's top. *Mine.*
10. **One entity per piece, and one mesh per building**, neither scales: the game places an entity per log today, and a castle would rebuild its mesh for every stone. *Answer:* built pieces merged into meshes per fixed section of cubes, rebuilt only where cubes changed; ghosts instanced; far off, a simplified mesh per section with its texture baked (as Unreal's HLOD). *Mine.*
11. **Stretching textured models smears them.** *Answer:* logs in a few lengths, their texture repeating along them, cut faces showing end grain; footings and anything drawn down to the ground textured by world position. *Mine.*
12. **The sky lights rooms as brightly as outdoors**, since the sky light has no walls. *Answer:* rooms (D155) dim it: from inside a room, the sky's light is scaled by how much sky its openings see, and later a light probe per room. *Mine.*
13. **Eaves reach into neighbouring cubes** (rule 2's exception), through a lean-to's wall or the next house. *Answer:* eaves are cut back against any cube within reach, as logs are cut at roofs. *Mine.*
14. **Chimneys and towers cannot rise through a roof**, since every wall is cut to its underside. *Answer:* only cubes that end below the roof's surface are cut; a column that rises through it opens a hole in the roof, with flashing round it. *Mine.*

**Doors, windows and what stands in a room**

15. **Glass was a cube in the plan**, though a window is fitted later. *Answer, the developers' own (2026-10-12):* doors and windows are props, so "they can actually look nice and be interactive": a wall only has a hole, cut by the door or window tool or carved; a prop fits a hole it sits inside, the kit filling any rest with boards or a transom, and the game says plainly when none fits. Glass leaves the cubes. A prop opens and shuts, has shutters, shows light within, and later breaks and is mended; a shut one closes its room. A door is a flag on the edge between two rooms, so opening it does not work the rooms out again. *Theirs, written in.*
16. **Items rest on cubes**: take a floor away and the chest floats. *Answer:* every item records the cube it stands on or hangs from, and nothing is taken away while anything is attached to it or rests on it, items, parts and props alike, so nothing has to drop. *Decided (D193).*
17. **Building into where someone stands** traps them. *Answer:* a part whose cubes hold a body, a creature or a dropped item is refused, and what is in the way outlined (Rust, Valheim). *Mine.*
18. **Stairs, ramps and ladders** cannot be cubes that are only solid or not, and a cube is taller than a step. *Answer:* a cube has a shape as well as a kind (whole, stair, slope, half), each with its collision from a table, as Minecraft's and Enshrouded's shapes. *Decided (D193): whole, stair, slope, half.*

**Support (rule 11)**

19. **"Joined" was not defined.** *Answer:* face to face, between built cubes with any fill; a roof section is held when each of its eaves rests on a built cube's top or a post; roofs are in the graph. *Mine.*
20. **Support lost by other means**: attacks (D90), ground dug at a claim's edge, old saves. *Answer:* what is no longer joined collapses into debris that gives back part of its materials (Valheim, 7 Days to Die); it never floats. Only what does not ask, an attack or the ground giving way, makes a collapse; a player's taking apart is refused instead (16). *Decided (D193).*
21. **A cellar under a built house is a deadlock**: the ground cannot be dug from under the footing, and the cellar cannot be built before it is dug. *Answer:* the cellar's plan digs, a column at a time, each column dug only once the cubes below it are built from the bottom and take the load; one operation, checked as one. *Mine.*
22. **Ground above the floor inside a building**: rule 9 leaves the ground alone, D131 cuts it, and the floor sits on a grid layer, so a slope can poke through it. *Answer:* D131 narrowed: ground is cut only inside the building's footprint, down to the floor; outside, nothing is touched. *Decided (D193).*

**Many players**

23. **Undo and two people at once.** Operations are applied in the server's order (D21), so an undo or a second builder can find the state changed. *Answer:* the server checks every operation, undo included as its inverse, against the state then; undo is refused if it would break support or someone has changed those cubes since; the next part is held for the player taking it for a few seconds. *Mine.*
24. **Plans cost nothing, so one player can plan a million cubes** that everyone nearby is sent and draws. *Answer:* a claim has a budget of planned cubes, and plans are seen only by its tribe, with the hammer in hand, nearby. *Decided (D193).*
25. **Refunds can be farmed**, as cost is set apart from pieces and carving gives back. *Answer:* cost is per cube in exact fractions, and taking apart gives back exactly what went in, no more; what is given back goes to the claim's storage when the taker did not pay for it, and who may take apart is by rank. *Decided (D193).*
26. **Two bases meeting**, each on its own grid and turn. *Answer:* claims do not overlap and keep a margin; every derived piece, eaves and footings included, stays inside its claim. *Decided (D193).*
27. **Villages built by stored operations** cost too much across whole worlds (D52). *Answer:* a village is a seed and the generator that makes its plan; only what players change is stored (D108 stands). *Mine.*

**The docs and the art**

28. **Changing a kind** is "pay the difference" in building.md and "take it down" in building-kits.md. *Answer:* a replacement pending on each cube, built like any part, neighbours redrawn. *Mine.*
29. **Stale text**: D178 said log ends run 0.25 m past; building-kits.md said there are no textures. *Fixed in this change.*
30. **Modules against D177.** Modules made by script in Blender may read as the code-drawn geometry the developers rejected. *Answer, the developers' (2026-10-12):* the modules are Tripo's models, conformed: cut to their cell, their edges feathered to a shared profile, the seam set in a mortar joint, so they stay pipeline models. The spike covers a straight wall, a carved cross, a stone and log join, and a part-built state. *Mine to try, theirs to judge.*

## Open for the developers

1. **Foundations.** **Decided** (D194): footings fill the space between the floor and the ground and stop where they meet it, taller where it falls away, costed by the cube like anything else; the floor sits at the highest ground under the building; the ground outside is not changed.
2. **The unit of building.** **Decided** (D194): a cube's fill level (gap 1). A log fills its course in every cube it spans; a block kind goes in a cube at a time; holding the key keeps going while there are materials (building.md, building in bulk).
3. **Round or crisp corners.** **Decided** (D194): each kit's own, as its material really is: crisp for cut stone and brick, round for plaster and earth.
4. **Walls thicker than a cube.** **Decided** (D194): allowed for every kind; a block kind draws them as one mass, a long-member kind as walls side by side.
5. **Blocks and long members.** **Decided** (D194): stone, brick, plaster and earth are blocks; logs, timber framing, planks and wattle are long members.
6. **Footings.** **Decided** (D194): the game lays them into the plan under every ground-floor wall and floor edge, as their own part with their own cost, built first (D139), in stone by default; posts or stilts instead, or none over a cellar, by choice.
7. **Nothing floats.** **Decided** (D194): rule 11 is in the MVP, refining D142; load and span stay for later.
8. **Taking away what holds something** (gap 16): **Decided** (D193): refused, for anything attached to it or resting on it, items, parts and props alike: "we should refuse to remove things with other things attach. otherwise we have to add another system to 'drop' those things".
9. **Cube shapes** (gap 18): **Decided** (D193): whole, stair, slope and half, more as kinds need them.
10. **Lost support** (gap 20): **Decided** (D193): what is no longer joined collapses into debris, giving back part of its materials.
11. **Ground inside a building** (gap 22): **Decided** (D193, D131 narrowed): cut only inside the footprint, down to the floor.
12. **Plans** (gap 24): **Decided** (D193): a budget of planned cubes per claim, seen only by its tribe, with the hammer in hand, nearby.
13. **Refunds** (gap 25): **Decided** (D193): exactly what went in, to the claim's storage when the taker did not pay; taking apart by rank.
14. **Claims meeting** (gap 26): **Decided** (D193): no overlap, with a margin, and everything built inside its claim.

## What changes from what is built

- **Logs stay on runs**, now inside the grid. Their footing becomes stone pieces into the ground, and the plain foundation slab and concrete steps go.
- **The stone kit is drawn as its cubes' surface** (D197, built): its Tripo sections are gone, its stone a displaced material.
- **Collision by state** stays as built.
- **Floors get edges**, and roofs ridges, hips and valleys, as pieces.

## Next

Spiked on 2026-10-12 ([research/block-modules.md](../../research/block-modules.md)): conformed Tripo pieces read as a grid of metre blocks and break where a shape interrupts their tiling; the cubes' own surface, bevelled and displaced by a real stone material, reads as one natural wall, with no seams, and carves cleanly. Recommended: block kinds drawn as that surface, long members as modelled pieces, doors, windows and details as props. Decided by the developers and built the same day (D197).
