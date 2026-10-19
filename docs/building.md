# Building

Status: the building system as the player meets it, for the developers to read and correct. It gathers what the developers have said about building, what players of the games we draw on will reach for ([research/building-systems-prior-art.md](../research/building-systems-prior-art.md)), and what is built. Each item says whether it is decided (with its D number), leaning, proposed, or open. How it works in code is [building-tech.md](building-tech.md); how each kind of construction looks is [building-kits.md](building-kits.md). The rules every construction piece follows, and every case they handle, are in [proposals/construction-001.md](proposals/construction-001.md) (D192, decided). What a cube is, who draws each face and edge between two cubes, and how every junction is closed and proved is the voxel contract, [building-voxels.md](building-voxels.md) (D348).

Everything built is half-metre cubes on its base's own grid (D48, D181): walls are rows of cubes a cube thick, posts columns of them, floors layers, openings cubes left open, and roofs cubes of a roof kind cut to slopes (D241); what is wall, what floor and which rooms are indoors is read from the cubes alone (D261, D277). The player builds by hand, roofs too, with brushes, carving and shapes, and the game lays the details from each cube's neighbours; drawing a room, a wall or a roof of a material over what is enclosed stays as a quick way (D152, D154, D226). Building is done with the hammer in hand, standing in the world, not from a separate view above: the build menu comes from the tool (D67) and offers what you have unlocked (D68), and a plan view from above stays [an idea for later](#ideas-for-later) (D176). You see what you would place before you commit, and taking apart gives back what it cost.

**Held to a baseline for now (D333, the developers):** construction work is held to wood (logs, hewn timber, shakes) and stone (stone, slate), built by hand and destroyed, with infinite pretend resources, until that is flawless and fast. Earth, lime-washed earth, wattle and daub and thatch are frozen: they still build and pass their checks, but are not polished, and are out of the settlement, the QA sets and the fault ratchet. Costs, building from storage, carrying large materials and recruits (building-plan.md's step 7) are paused. No new materials or props unless the developers ask.

## What the developers want

In their words, shortened.

- **The basis is a voxel grid, as Enshrouded's** (D48), and everything built is strictly on it (D181).
- **Enshrouded is a role model** for building and voxels: "a ton of stuff we really might like to model off of". Building in voxels is fine; the terrain stays smooth voxels. See [research/enshrouded.md](../research/enshrouded.md).
- **The Forest's building is an inspiration** (D85): "a nice building system, the way you get logs/etc is nice for larger projects (vs sticks or something akin to that)".
- **Keep the prior art in mind for the controls too**: Enshrouded, Rust, Ark, Subnautica 2.
- **You pick the part you want to build, and can cycle its variants**, from "the little dial building menus we see in things like Rust and Subnautica". **A build menu like Subnautica's** (D67): you equip the building tool and it brings up what you have unlocked (D68).
- **Blueprints are walls, floors and roofs**, built up piece by piece; a room is what they enclose (D152). **Doors and windows are openings** cut into any wall.
- **Furniture and kit are buildable items.** Everything built in the world, a village's houses included, is made of the same parts (D108); stations are the only prefabs.
- **Everything has a collision box**, used for placing as well as walking (D53).
- **A grid things snap to**, which can be switched off. One square grid for everything, "a little flexible": in some games each cell is huge, "so you have no freedom to build". Half a metre for now (D57).
- **What is placed can be moved up and down**, not only turned, except what must stand on the ground.
- **What is built into the ground cuts it away** (D131). **Trees and rocks are cleared first** (D131).
- **Details morph.** "Corners become rounded on the outside and inside, but when two segments are next to each other they're flat."
- **It must be really easy to extend**, with pieces that "change their look based on what they're connected to" (D56).
- **Building, taking apart and cutting take time**: click and hold, animated in and out.
- **Building costs things you carry**: "you have the raw items, which allow you to build anything that you might have the capabilities to build" (D55, since replaced by D139 and D141).
- **What does not fit in the inventory drops to the ground**.
- **You build only inside a claim** (D94), which starts small and grows (D104).
- **Structures are planned, then constructed** (D139): "for structures, you place a blueprint/plan of it, and then you have to actually construct it somehow… it makes it more realistic. So each wall you'd have to do that, and then doors are like carve outs so it can be slightly diff? Same with windows? We'll wanna think this through."
- **Constructing should feel immersive** (D139): "you'll actually go up and hit the thing with a hammer or similar? Or maybe it's like, you bring the resource and place it? Like you carry it, like The Forest / Grounded 2? We want it to feel a little immersive when you build up these structures."
- **The controls and plans proposed here "feel good"** (D144).
- **A blueprint shows its parts, and they can be small** (D139): "ideally a blueprint shows its 'parts' if you will, kind of like Forest shows the logs, and that can vary based on the type of construction".
- **Large structures take large materials, carried** (D140): "you probably need large materials and can carry them? And later in the game maybe you unlock ways to transport them more efficiently."
- **Building draws on storage** (D141): "building from storage absolutely, but large items probably will need a special storage? And that probably would be unlocked later game (some of this)."
- **No stability in the MVP, but wanted later** (D142).
- **Rooms can be underground** (D143): "yes you can have underground rooms".
- **Placing is by hand, Enshrouded's way** (D226, amending D154): brushes, carving, shapes and roofs built cube by cube, any shape and any number of storeys; the game makes the details from neighbours, and The Sims' drawing of rooms, walls and "a roof of this material" stays as a convenience.
- **Walls on roofs, the roof whole round them** (D343): walls, chimneys and blocks may stand on a roof's cubes, rise through them and stand beside them, and the roof stays modular round them with no gap: "we should implement it correct, but the issue is we had gaps, because we didnt think this through". A storey goes up over a roof left on as well as with it taken off. How it is drawn is [building-plan.md](building-plan.md#where-roofs-and-walls-meet-d343); only a roof cut to a shape no roof has is refused, "A roof is cut only to slopes, corners and halves."

## The baseline

Everything a player arriving from Enshrouded, Rust, Ark, Subnautica 2 or The Forest will reach for, and where ours stands. This is the list to check the system against, so nothing is forgotten.

| Capability | Prior art | Ours |
|---|---|---|
| A tool that builds, and a menu of what you can build | All | Built: the hammer and the build menu (D67) |
| A variant wheel for the thing in hand | Rust, Subnautica | Built: hold R |
| A ghost before you commit, allowed or refused, saying why | All | Built; what blocks it is outlined, but not through walls, as Subnautica 2's is |
| Rotate | All | Built: wheel, or Z and C |
| Snap to a grid, with a switch to go free | Enshrouded, Ark, Valheim | Built: X, and hold Alt |
| Raise and lower, and nudge | Enshrouded, Ark Ascended | Raise and lower built; structures by the storey, down too |
| Place in a line or an area: drawn walls, floors, rooms | The Forest, Sons of the Forest, Subnautica 2 | Built: the room shortcut drawn as a rectangle, walls as lines inside or out |
| Pick another of what you look at | 7 Days to Die | Built: middle mouse |
| Move what is placed | Subnautica 2 | Built for items, not walls |
| Take apart, giving back everything | Enshrouded, Subnautica, Valheim | Built; what does not fit drops at your feet |
| Undo for a short while | Enshrouded, Rust | Built: Y, within a minute |
| Repair | Rust, Valheim, The Forest | Built: the hammer mends a damaged cube for a share of its cost (D305); attacks to damage it are still to come (D90) |
| Change a piece's material in place | Rust, 7 Days to Die | Built: Change material, a brush's box of built cubes, the old out from under what stays and given back, the new built (D232) |
| Plans that cost nothing, then constructed part by part | The Forest, Ark Ascended | Built (D139, D201) |
| Building from storage nearby, not only the pack | Enshrouded | Built: chests, and what is built in a claim draws on them after the pack (D141, D295) |
| Carrying big materials by hand, better transport later | The Forest | Missing; decided (D140), how is open (D296) |
| A base level with the ground where it is begun | Ark Ascended's heights | Built: the base's grid is set where its claim stone goes |
| Building on slopes | All, differently | Built: what is built cuts into the ground (D131) and stands on a footing below |
| Floors above and below, and stairs | All | Storeys above and below the base's floor built; stairs and ramps up a storey built (D220) |
| Roofs | All | Built (D241): roof cubes of shakes, slate or thatch built by hand, any shape, their ridges, hips, valleys, eaves and verges from their neighbours; "add a roof" (gable, hip, lean-to or flat) over what walls enclose or a rectangle drawn |
| Doors that open, close and lock | All | Doorways only |
| Fences, gates, palisades | Ark, The Forest, Rust | Missing |
| Stability | Rust, Valheim, Sons of the Forest | None in the MVP (D142); wanted later |
| A claim with an area, an owner and who else may build | Enshrouded, Rust, Valheim | Area and owner built; nobody else can be allowed yet |
| Upkeep or decay | Rust, Ark | None; open |
| Saved layouts placed again | Ark Ascended, mods elsewhere | Missing; the village is one (D108) |
| A camera for building | Enshrouded | The zoom only |
| Stations and furniture placed as items | All | Built |

## Controls

Agreed by the developers (D144); how a part is put into a plan is the one part still being worked through (D139). One scheme for everything in this document, on mouse and keyboard, after what the prior art agrees on, keeping what is built where it already works. Every key can be changed in the options (D116) but the wheel and the numbers. A gamepad scheme comes later and follows the same states.

**The rule underneath: a click lays, work builds.** A click puts down a plan (D139), which costs nothing and can be undone at once. Spending materials takes work: setting a carried part, swinging the hammer at a part, holding to build an item, take apart or repair. So a misclick never costs anything.

### Always, with the hammer in hand

| Input | Does | Built |
|---|---|---|
| B, or tap right mouse | Open or close the build menu. In it: click a tile, or 1 to 7 for a tab, the arrows and Enter; Escape or right mouse closes | Yes |
| Hold R | The wheel of the picked part's variants: move the mouse to one and let go. Tap R for the next, Shift and R for the one before | Yes |
| Backspace | Put the part away: the hammer in hand with nothing picked | Yes |
| X | Grid on or off | Yes |
| Hold Alt | The other of the grid's settings, while held | Yes |
| Middle mouse, or V | Pick another of what the view is on: its part, variant, turn, height and material | An item's part; a cube's material (D232) |
| Q | Take-apart mode, and back | Yes |
| G | Move mode, and back | Yes |
| Y | Undo the last thing laid, built or taken apart, within a minute, giving back what it cost; taking apart undone takes back what it gave. Enshrouded's key: Ctrl and Z would roll, Ctrl being the roll | Yes |
| = and - | Camera nearer and further; the wheel does it only when nothing in hand turns | Yes |
| F | Use what the view is on: open a door, work a station, grow the claim | Yes, but doors |

### Placing: a part picked

| Input | Does | Built |
|---|---|---|
| Aim | The ghost follows, snapped (see [Snapping](#snapping-and-feel)) | Yes |
| Wheel, or Z and C | Turn: a quarter turn for structures; an eighth for items on the grid; 15 degrees off it | Yes |
| Shift and wheel, or Page Up and Page Down | Raise and lower: half a metre a notch on the grid, a tenth off it, up to three metres; structures a storey at a time, two down to two up (D143). Stations and the claim stone stay on the ground | Yes |
| Ctrl and wheel | The material every plan is laid in, through every kit (D232), shown with the keys under the aiming mark. Ctrl is the roll's key (D97), which waits while a part is in hand, as Shift and the wheel share the sprint's | Yes |
| Alt and wheel | The brush: half a metre, 1, 2 or 4 m, for blocks, changing material, carving and taking out (D226), on the brush's own grid | Yes |
| Click left mouse | Lay it: a plan, for a structure; for an item, hold instead | For walls, roofs and the room shortcut, laid as a plan (D201) |
| Press, drag and let go | Draw it: a wall or fence along the line dragged; a room or floor over the rectangle dragged. A plain click lays the variant's own size | For walls, roofs and the room shortcut |
| Hold left mouse (items) | Build the item there, spending its cost | Yes |
| Tap right mouse | Put the part away (or, carrying something in move mode, put it back) | Yes |

After a placement the same part, variant, turn and height stay in hand, so the next goes down at once.

### At a plan

| Input | Does | Built |
|---|---|---|
| Aim at a plan | Its next part is outlined, and the prompt says what it is, what it needs and how many are within reach: "Log: 3 of 12. 9 more needed, 4 in reach" | Lit in the plan, the prompt saying what it takes or why it cannot go in; not counts or reach |
| Carrying a big part, left mouse at its outline | Set it in place (see [how a part goes in](#planned-then-constructed)) | No |
| Swing the hammer (left mouse) at a set part, or at a plan's next small part | Fix it, or put it in: a few swings each, the fixings from the pack or storage in the claim (D141) | Held, from the pack: with nothing picked, or held still on a plan, what goes in next goes in, and the next while held; not from storage, no carrying |
| Q, then hold left mouse | Take the plan away, and everything put into it comes back | Yes, part by part, last in first out |

### Taking apart (Q)

| Input | Does | Built |
|---|---|---|
| Aim | What would come away is outlined: an item, a part of a structure (the last put in, so it comes down in the reverse order), or a whole plan | Items, a wall's straight stretch, an opening |
| Hold left mouse | Take it apart, giving back what it cost; what does not fit drops at your feet | Yes |
| Shift and hold left mouse | Take apart the whole of it: a wall, or all a room's walls, part after part | No |

### Moving (G)

| Input | Does | Built |
|---|---|---|
| Click an item | Pick it up; it follows the aim like a part being placed, and turns and rises the same way | Yes, but rising |
| Click | Put it down | Yes |
| Tap right mouse | Put it back where it was | Yes |
| Click a plan | Pick the whole plan up and move it, while nothing has been put into it | No |

What is constructed stays where it is built; to move a wall, take it apart and lay it again.

### Repairing

With the hammer in hand and nothing picked, aim at something damaged: its damage shows, darker and broken the more it has lost, and holding left mouse mends it for a share of its cost, a quarter of that share for each quarter of its health lost (D305). A damaged cube is mended before anything more is built into it or taken out of it.

### Collapse and fragments

What loses its support without being asked, to an attack or the ground giving way, comes down (D193): what is destroyed bursts into fragments of its own material, stone into chunky blocks, logs into lengths and splinters, wattle and roofs into bits, earth crumbling, and what hung from it falls whole a moment later, the higher the later, breaking where it lands. The fragments bounce, roll off ledges and settle, with a puff of dust in the material's colour, the view shaking near a big collapse, then shrink away. A big collapse, from 400 cubes at once, raises a cloud of dust as wide as the mass that fell, rolling out over its foot and up for seconds. Nobody can touch them and nothing is given back from them while building takes no real resources (D305). Nothing takes damage until attacks come (D90); until then the remote and a debug key (F9, on the cube aimed at) do it.

### Fire

What is built of wood or straw burns (D377): logs, squared timber, a timber frame, shakes and thatch catch from flame, thatch at a touch and a log only after a while; each burns for as long as its fuel lasts, chars black and cracked, glowing in its cracks, and burns through, and what it held comes down as any loss does. Fire climbs faster than it creeps, runs downwind, smoulders where no air reaches, and fills a shut room with hot gas until everything round it catches at once, as a room flashes over; burning wreckage that falls sets alight what it lands on. Stone, slate and earth do not burn but blacken with soot, and fall when the timber under them burns away: a tower's floors on bond timbers through its walls, hoardings on their beams. Times are a game's, not a real fire's: a shingle roof goes in seconds, a log wall in half a minute (real logs burn for hours). How a material burns is its file's (`burns`).

### To lock down

The calls for the developers, each recommended as written above:

1. **Click lays, hold builds**: a click puts down a plan for free; holding spends materials (putting in parts, building an item, taking apart, repairing).
2. **Drawing**: press, drag and let go to lay a wall or fence along a line, or a floor or the room shortcut over a rectangle, in the grid's four directions (D146).
3. **Height on Shift and the wheel**, with Page Up and Page Down; **the kind of construction on Ctrl and the wheel** (built, D232, the roll waiting while a part is in hand).
4. **Undo on Y**, within a minute (Ctrl and Z would roll, so Enshrouded's key instead; built).
5. **Right mouse stays the build menu** and **Q stays take-apart**, as Subnautica, not Enshrouded.
6. **F opens and closes doors**, as it uses everything else.
7. **Constructed things are not moved**, only plans and items; a wall is taken apart and laid again.

### Where ours differs from the prior art

Right mouse is our build menu, as Subnautica's (D67), where Enshrouded's right mouse takes apart; Q is our take-apart mode, where Enshrouded's Q nudges. Keeping ours is recommended: they follow the game the developers named for the menu, and they are built.

## Snapping and feel

What makes placing feel good is mostly that the ghost goes where you meant without a fight. Proposed:

1. **What it snaps to, in order.** First the joins of what is already built or planned, within reach of the aim: the end of a wall, the edge of a floor, an opening in a wall. Then the base's grid (D57). Then, for items, the surface under the aim: floor, ground, a table's top, a wall. With the grid off, or Alt held, only surfaces.
2. **It sticks.** Once snapped, the ghost stays put until the aim has moved well off that spot, so it never flickers between two places. Moving on to the next spot along a wall is a short, clean step. (Built for structures.)
3. **It glides.** The ghost slides to each new place in a few hundredths of a second instead of jumping, but the outline of where it will land is always exactly where it will land.
4. **Structures aim on a level.** A wall, floor or roof is aimed on the base's floor at the height chosen, not on the bumps of the ground under the aim, so it does not jump as the view crosses grass and stones. Items aim at what is under the view. (Built for structures.)
5. **Out of reach, it waits.** Aimed beyond reach, the ghost stays at the edge of reach along the aim, dimmed, instead of vanishing. (Built for structures, not yet dimmed.)
6. **It faces sensibly.** A wall, door or window faces the player; an item faces back toward them the first time, and remembers its turn after.
7. **It shows what it is doing.** The grid shows only around the ghost, fading out a few metres off. The join it snapped to lights up. Lines appear when it lines up with an edge already built. A small arrow marks its front. Off the floor, a line drops to what is under it and the height is shown.
8. **It says no plainly.** Red, one line saying why, and what is in the way outlined, behind walls too. Blue otherwise, with its cost and what is in reach.
9. **It sounds right.** A soft tick as it snaps to a new place, a different one when it turns, and the thud of laying a plan or putting in a part.
10. **The hand is steady.** Every value above, how sticky, how fast the glide, how far the grid shows, is tuned by playing, and kept as data so it can be changed without code.

## Planned, then constructed

A structure is first laid out as a plan, and then actually constructed, part by part (D139). What follows is a working-through of it, to react to; beyond the decisions it cites, nothing here is settled.

**Laying a plan.** Everything structural that the build menu places, a wall, a floor, a roof, a fence, goes down as a plan: a pale ghost in the claim that costs nothing and blocks nothing yet. Lay as many as you like; move them, resize them, take them away for free. A plan shows, part by part, what it needs. This is also how a whole base is tried out, and its full cost seen, before anything is gathered (The Forest, Ark Ascended's planning mode).

**Constructing it.** A plan shows its parts, the way The Forest's blueprint shows each log (D139): every piece that will make it is outlined in the ghost, and each is put in one at a time. Stand at the plan with the hammer and what the next part needs, in your pack or in storage nearby (D141), and hold the build key: that part goes in, its material is spent, and the outline fills. Parts can be small, a single log or a single stone.

**What the parts are depends on the kind of construction.** Each kind of construction says how a wall, floor or roof of it is made, as data, like everything else built (D136; [building-kits.md](building-kits.md)):

| Construction | A wall is | A floor is | A roof is |
|---|---|---|---|
| Logs | Logs laid one on another, each as long as the wall or a log can be, notched at the corners | Logs side by side on beams | Poles, then a covering |
| Planks | Posts at the corners and every so far, then planks across them | Beams, then planks | Rafters, then planks or shingles |
| Stone | Courses of cut stone, each block a part | Flagstones on a bed | Beams, then slates |
| Wattle and daub (early) | Stakes, woven sticks, then a coat of mud | Packed earth | Thatch on poles |

So a log cabin's wall of 4.5 metres, a storey high, is about a dozen logs, put in from the bottom up; a stone wall is many more, smaller parts. The order is the physical one: what a part rests on goes in first. That is not a stability check (D142), only which outline is next.

**How a part goes in: carried, set, hammered.** The developers want it to feel immersive, and named both ways: bringing the material and placing it, as The Forest and Grounded 2, and going up and hitting it with a hammer. Then, settled for large construction: "it'd originally be placed as a blueprint, and you'd have to then bring the materials to it to build it (kind of like the forest or grounded 2)". The rest of this is a proposal, by the size of the material, which is how building is really done:

- **Big parts are carried and set.** A log, a beam or a cut stone is brought to the plan, on the shoulder (D140) or picked up from a pile within reach, and set into the outline it fits: you see it go in, and the outline fills.
- **Then the hammer fixes it.** A few swings at what was just set peg, nail or mortar it in, the small fixings coming from the pack or storage (D141). Until it is fixed, the part is in place but loose: it can be lifted out again freely.
- **Small parts are only hammered.** Wattle, planks, shingles and thatch come from the pack or storage; swinging the hammer at the plan puts the next one in with each few strikes.
- **Recruits do the same**, visibly, when the materials are near (D90).

Open: how many swings a part takes, and whether fixing is a separate step or the last swing of setting it.

**Drawing a plan.** The developers asked how dragging to draw would work, "say it's a stone wall, we'll need the stone to look modular". Proposed:

- **A wall or fence is drawn as a line.** Press where it starts, drag, let go where it ends. The start snaps to a join (the end of a wall already there) or the grid; the line runs along the base's grid, in its four directions only for now (D146), a grid step (half a metre) at a time, so every wall is a whole number of steps long. While dragging, the plan is shown already split into its parts, with its length and what it needs: "4.5 m stone wall: 54 stones, 12 in reach". Shift and the wheel, while dragging, set how high, a course or a storey at a time.
- **A floor, or the room shortcut, is drawn as a rectangle**: press at one corner, drag to the other. The room shortcut lays a floor, four walls and a roof, each its own part (D152).
- **Drawing on from a wall's end continues it**, and closing a loop of walls offers to make it a room.
- **A plain click lays the variant's own size**, as now: a 3 metre wall, a 5 by 5 metre room.

**Walls, not rooms** (D152). Blueprints are walls, floors and roofs, built up piece by piece, not prebuilt: "a room should be N walls anyways, it's a shortcut". How placing them works is [below](#enshrouded-or-subnautica-how-placing-works) (D154).

- **The parts are walls, floors and roofs**, and later foundations and stairs. A room is not a thing that is built; it is what walls, a floor and a roof enclose.
- **The room in the build menu is a shortcut**: a rectangle drawn with it lays its walls, its floor and a roof as plans, each its own, as if drawn one by one. Closing a loop of walls drawn by hand offers the same floor.
- **A roof is laid over what is enclosed**, offered when the walls close, or drawn as a rectangle like a floor.
- **Indoors is worked out**: inside walls and under a roof, by the grid. It is what shelters from weather and lets interior things go in.
- **Into the ground**: ground inside a building's footprint above its floor is cut away, and nothing outside it (D131, D193); a cellar is the ground dug out, then walled.

**What a building is** (D155). The developers: station expansions count "anywhere in the building … buildings will need to know if they're 'complete' (aka walls/roof) anyways because otherwise you have natural elements like rain/cold that should have effect". So the game keeps track of buildings:

- **A building** is the walls, floors and roofs that touch one another. Two separate cabins in a claim are two buildings.
- **Its rooms** are the spaces enclosed by its walls, floor and roof. A room that is closed all round is **indoors**: sheltered from rain, wind and cold (D78), and where interior items go. A room missing its roof, or with a gap in its walls, is not, and says what is missing ("No roof over this room").
- **Doors and windows**: an open doorway lets weather in; a fitted door, shut, keeps it out, and an open window lets in a little (D139).
- **Stations** belong to the building they stand in: their expansions count anywhere in it (D151).

Technically: worked out from the cubes when a part is built or taken away, not every frame (D127), as [building-tech.md](building-tech.md) says (D160). Built: a base is half-metre cubes (D181); rooms are the space its built cubes close in all round, of any height, flooded in three dimensions, and each is indoors, dimmed (D261); doors and windows are cubes cut open. Buildings as entities come with weather and stations that read them.

**How a material is split into parts, so it looks built and not tiled.** Each kind of construction is a small kit of pieces sized to the grid, and a rule for laying them, as real masonry and carpentry have:

| Kind | Pieces | Laid |
|---|---|---|
| Stone, earth | Its cubes' surface in its material, cut to their shapes (D197); quoins, plinth, coping, lintels and sills by role (D215) | As one mass over all its cubes, dressed at every exposed edge |
| Logs | Logs as long as the wall, up to the longest a tree gives (say four metres), notched at the ends | Stacked, a log a course; longer walls join end to end, staggered, and walls meeting in line under a post; corners interlock, the logs of one wall alternating with the other's ([building-kits.md](building-kits.md)) |
| Hewn timber | Squared logs | As logs |
| Wattle and daub | Its cubes' surface as daub, a frame of squared timber over it | Posts, plates, rails and braces by the frame's rule (D236, D258) |

Because each piece is chosen by its role from the cubes round its edge, any arrangement fits (construction-001, rule 3): corners and openings choose their own pieces by what they are next to, which is the morphing the developers asked for (D56). Each piece gets a little variety, a slightly different shape, turn or shade from a few made versions, so a long wall does not repeat.

**What the pieces are made of** (D146, D177). Each kind is its material's surface and its pieces, profiles, modules and stones made in Blender, some modelled by hand and the rest by a script from numbers until they are ([building-plan.md](building-plan.md#one-material-system-d251-d258) says which), laid on the base's cubes by role (D251, D258; [building-kits.md](building-kits.md)); logs are made in code (D200). The pieces are parts of the blueprint picked in the build menu, never things made first at a station and then placed: the developers do not want to "build walls then place them". The cubes are the skeleton (D48, D181): they say where each piece is, what is inside and out, what blocks a walker, and what an opening cuts. The pieces are what is seen and what goes in one at a time.

**Carving** (built, D226, D233). On cubes (D181) carving by the grid's half metre is only emptying cubes: the kit lays itself again around the gap, a cut block becoming a half block, a log course ending in cut ends. Doors and windows are cut the same way, at a default size. Carving finer than the grid, chiselled or smoothed, needs the material drawn as voxels, Enshrouded's way; our ground already is (D48), so it would come as a kind of construction of its own, such as carved rock, where the pieces sit on the voxel volume where they meet it. Nothing about the kit stops either.

**Building in bulk.** Putting in parts one by one is the realistic way and must not become a chore: holding the build key keeps putting in the next part while you have the materials; a skilled builder (D88) puts them in faster; recruits put them in for you (D90). The Forest's later option to skip each part's animation is the lesson: show each part going in, quickly.

**Doors and windows are carved out** (the developers' "carve outs, so it can be slightly different"):

- **On a plan**, an opening is planned into the wall: the parts it crosses are shortened or left out, so the wall needs less, and a frame is added to build.
- **In a finished wall**, an opening is cut: it takes work with a tool, gives back the material of what was cut away, and leaves a frame to build.
- **The door or the glass is its own item**, made at a station and fitted into the frame. A doorway without a door is just a gap; a window frame without glass is open to the weather. A plan never shows a door or glass in its openings, only the opening and its frame (2026-10-12). Built (D225): the first door and window models were taken out as not good enough ("we have some of these ugly ass door and window props we can probably just remove for now") and made again in the style: a plank door, a ledged and braced door and a studded door, 1 by 2 m or a pair in a doorway 1.5 or 2 m wide, and board or louvred shutters over a window, a pair over one wider than half a metre. The hammer's *Door* and *Shutters* (Openings tab, the wheel picking the kind) fit one into a hole that fits it once its walls are built, a ghost of it shown first, paid for by the leaf; *Remove* takes it out for what went in; the interact key opens and shuts it, a door swinging into the room and shutters folding back against the wall outside. The village's houses have them, of a kind each house, the hall a pair of doors in a doorway two metres wide. There is no glass: a window is a hole in the wall, its cubes open as a doorway's are, and a window prop is shutters (D225: "just leave windows as a hole in the wall"). Doors are not yet made at a station; each costs its raw materials when fitted.

**Small things are not planned.** Furniture, storage, lights and stations are placed whole, as now: they are made, then set down. Open whether stations should be constructed too, being large.

**Taking down.** A constructed part comes down cube by cube, each going with its plan, nothing left behind (D193: "it should deconstruct and return materials"), giving back its materials (or dropping them at your feet if they do not fit). A plan with nothing built is removed for free. Undo works on both.

**Who constructs.** Anyone allowed in the claim (D94), and recruits who work at the base (D90; not built, D297), as Sons of the Forest's companion finishes blueprints, but never a draft still being designed (D170, Leaning; [tribe.md](tribe.md#plan-and-build-handing-work-to-the-tribe)). A claim-wide list of what all its plans still need, as The Forest's counter, tells a group what to go and get.

**On the voxel grid** (D48, D181): constructing a part fills its cubes; a plan is cubes not yet filled. Carving an opening empties cubes.

## Large materials and carrying

Paused by D333: what follows is built or decided, but not worked on while the baseline is wood and stone with infinite pretend resources.

Decided (D140, D141): big structures take big materials, such as logs, carried to the plan; building takes what it needs from storage nearby, not only the pack. Leaning: better ways to move big materials come later (D68: a sled, a cart, perhaps D100's mounts), and they need a storage of their own, a log pile or a stone yard, unlocked later.

**Building from storage** (D141, built, D295). Storage is chests set down from the build menu's Storage tab, each a grid as the backpack is (D84). Whatever is built in a claim, a plan's parts or an item set down, takes from the builder's pack first, then from the storage standing in that claim, nearest first, as Enshrouded's base storage serves its crafting. A chest is opened with F beside the backpack; a click moves a thing across, Shift and a click every one of its kind. A chest holding anything is not taken apart or moved, as nothing is taken away while something rests on it (D193).

**Carrying big materials** (D140) is not built: how it plays is open (D296). What shipped games do:

| Game | Big materials |
|---|---|
| The Forest, Sons of the Forest | A felled tree's logs lie where it falls; you carry two on the shoulder with nothing else in hand, and add each to the blueprint you look at; a sled carries more. Sons of the Forest cuts logs into halves and planks with the axe, and its companion fetches logs and finishes blueprints |
| Valheim | Nothing carried by hand: everything is in the pack, which slows you past its weight; a cart carries a load for ore |
| Enshrouded, V Rising | Nothing carried by hand: everything is in the pack; Enshrouded's base storage serves crafting |
| RimWorld | Colonists haul materials from stockpiles to a blueprint, which becomes a frame when they arrive; a builder then builds it |

What the developers would decide, each with a recommendation:

1. **Which materials are big.** Recommend logs first, with cut stone blocks the same way once stone is cut at a station; planks, shingles, thatch, wattle, fibre and nails stay in the pack.
2. **Where logs come from.** Recommend a felled tree gives its logs on the ground where it falls, as many as its trunk's length allows, and its branches as Wood in the pack, as The Forest's do.
3. **How many, how slowly.** Recommend one log on the shoulder at a walk, no sprinting, or two at a slower walk, nothing else in hand, dropped with a key, as The Forest carries two.
4. **How a log goes in.** Recommend carried to a plan, use at it sets the log in as the next course's log of that wall, the outline filling, as The Forest adds a log to the blueprint you look at; a log wall takes logs, not Wood. Whether a few swings of the hammer then fix it (D139's carried, set, hammered) waits until setting has been played.
5. **Where logs wait.** Recommend logs dropped near a plan, within a few metres, count for building it, as building draws on storage; a log pile, unlocked later (D141, Leaning), holds them tidily.
6. **Is a log cut first** (open question 6 below). Recommend logs go into log walls whole; a sawbench cuts them into beams and planks for framed kinds and floors.
## Placing

- **The grid** is half a metre (D57; see [The grid](#the-grid)). Things snap to it and turn in eighths; off it, they turn freely.
- **A base stands level where its claim stone is put** (built): its grid is set there, level with the ground at the stone. Everything in a base shares its floor height and stands on a footing where the ground falls away (built). **Rooms can be underground** (D143): walls and floors may be placed a storey or more down, cutting the ground away, for cellars and for terraces down a slope.
- **Into the ground**: what is built cuts the ground away only inside its footprint, down to its floor, no deeper than its own height; outside it, the ground is not changed, and footings stop at it (D131, D193). Trees and rocks must be cleared first (D131); small plants clear themselves.
- **Up and down**: anything placed can be raised and lowered (asked for), on the grid in half-metre steps. What must stand on something says so: a chair on a floor, a station on the ground.
- **What may not overlap**: an item may not go into another item or a wall (built); a structure may not go over a solid thing set down outdoors, such as the claim stone, which is outlined (built).
- **What the ghost says**: blue when it can go, red when not, with one line saying why and what it costs; what is in the way outlined, through walls too (proposed, after Subnautica 2's first update).

## The parts

The baseline catalogue. Everything is an entry in a list, [`builds.ron`](../assets/data/builds.ron), and a kind of construction is a kit file ([building-kits.md](building-kits.md)).

| Part | Status |
|---|---|
| Wall, drawn as a line of cubes, any length, inside or out | Built |
| Room: the shortcut that lays a rectangle's walls, floor and roof | Built |
| Floor, drawn on its own | Built: a rectangle on a storey (D220) |
| Roof: cubes of a roof kind by hand; "add a roof", gable, hip, lean-to or flat | Built (D241, [building-kits.md](building-kits.md#roofs-d241)) |
| Window and doorway, cubes left open | Built |
| Carving cubes by hand, any shape | Built: Carve, a brush's box of built cubes marked and taken out (D226) |
| Taking out the cube aimed at | Built: take-apart takes the brush's box aimed at where nothing rests on it, else what came last of its part (D226) |
| Shapes: whole, half, upper half, stair and slope either way up, outer and inner corner slopes, post | Built (D220, D231) |
| Post, a column of cubes (D179) | Built: a column of post cubes, a storey high (D220) |
| A storey up, and stairs or a ladder to it | Storeys and stairs built (D220); ladders missing |
| Footing under walls on the ground | Built: stone cubes down to the ground, laid first (D206) |
| Fence, wall, gate and palisade, drawn as a line | Missing; needed once bases are attacked (D90) |
| Door that opens, closes and locks | Built (D225): doors and shutters fitted, opened and shut, taken out; no lock |
| Stations: workbench, smelter, forge, campfire | Built |
| Furniture, storage, lights | Built |
| Claim stone | Built |

## Changing what is built

- **Taking apart** gives back everything it cost (built); what does not fit drops at your feet (built).
- **Undo**: the last thing built or taken apart, within a minute (built, on Y).
- **Moving**: items now (built); walls as a whole later (open).
- **Repair**: the hammer mends what is damaged, for a share of its cost (built, D305); what collapses breaks into fragments that fade. Tools and gear already wear and are mended at stations (D99).
- **Material**: Change material, a brush's box of built cubes changed in place, the old out and given back, the new built (built, D232).

## Stability

None in the MVP, and wanted later (D142). Settled already (D193): nothing is taken away while something rests on it or is attached to it, and what loses its support to an attack comes down in fragments that fade. Rust, Valheim and Sons of the Forest make structures hold themselves up; Enshrouded and Subnautica do not; Ark does but hides its rules, which players complain about. When it comes: shown only while building, as Valheim's colours; a part that holds others up cannot be taken out from under them, as Sons of the Forest's; and constructing in order (floor, walls, roof) already reads as physical without it.

## Enshrouded or Subnautica: how placing works

Decided (D154): The Sims' way, below; since amended by D226 to building by hand, roofs too, with the Sims' drawing kept as a convenience. The developers, with walls, floors and roofs settled as the parts (D152): "the only question we have is how it should work. like enshrouded style, or subnautica style. i do like enshroudeds flexibility to design something truly interesting/unique/beautiful but i will admit its also a little overwhelming".

The basis is settled (D48): **a voxel grid, as Enshrouded's**. What Enshrouded's does, from [research/enshrouded.md](../research/enshrouded.md): buildings of small voxel blocks in a material, placed one at a time or as ready-made pieces; pieces join seamlessly and materials blend where they meet; cutting an opening styles its edges; the ground can be dug, filled and flattened; you build only within an altar's radius.

| | Subnautica 2 | Enshrouded |
|---|---|---|
| What you place | Fixed modules that snap together | Blocks and ready-made pieces, in any shape |
| Freedom | Little: the modules' shapes | Any shape, down to one block |
| Speed for big things | Fast | Slower, unless ready-made pieces are used |
| What makes it look good | The modules' design | The player's skill and time |
| Learning it | Minutes | Long; many shapes, and roofs by hand |

**Why Enshrouded's overwhelms**, as far as we can tell: the player makes every detail by hand, from many shapes; roofs are built a piece at a time; nothing helps a beginner's house look good. Its freedom is in the shape of the building; its effort is in the detail.

**The Sims' way, now the quick tools** (D154, amended by D226 and D253). Building by hand, cube by cube, roofs too, comes first; drawing the shape and letting the game make the details stays as a quicker way. A log cabin by the quick tools goes:

1. **Walls.** Pick *Wall*, log. Press and drag 6 m: a wall's blueprint, shown log by log. Drag on from its end, round three more sides; when the loop closes, the game offers a floor inside, a click to accept.
2. **Roof.** Pick *Roof* and click inside the walls: a roof's blueprint, its cubes and its gable ends' walls (D241), covers what they enclose. The next variant picks its style (gable, hip, flat, lean-to). Its rafters, ridge and gable ends are worked out; the roof's cubes can be changed by hand.
3. **Openings.** Pick *Door* or *Window* and click a wall: the opening is planned into it, and the wall's logs shorten round it.
4. **Construct.** Bring logs, set them, hammer them in (D139). The cabin goes up log by log.

| You decide | The game decides |
|---|---|
| Where every wall goes, how long and how high, in which material | How the logs or stones of each wall are laid, and how corners interlock |
| Floors and storeys | Corner posts, trims, sills and lintels |
| Where doors and windows go, and their size | How the wall is cut round them |
| The roof's style, over what you enclosed, or its every cube by hand | Its rafters, ridge, eaves and gable ends |
| Stairs, foundations, what goes inside | What is indoors, sheltered from rain and cold (D155) |

**Against the other two**, for the same cabin:

- **Subnautica's way**: you place a ready-made room module of a fixed size and join modules with corridors. Quick, but every cabin is the same.
- **Enshrouded's way**: you place each wall section and each block, then build the roof slope by slope, with ridge pieces, gable ends and trims by hand. Any shape is possible; that is the freedom the developers like, and the overwhelm.

Ours gives the same freedom of shape as Enshrouded's, any footprint, any length, any height, many storeys, without making every detail by hand. **Later, with progress**: arches, round and diagonal walls, carving (D146), more roof styles and materials are learned (D68), as Terraria's are (D85); a builder who wants a detail of their own can place it, such as a piece of roof or trim by hand. And a building can be saved as a design and laid again.

**Where we improve on it** (D48): shape the building, then carve it; bases that matter, held, defended and lived in (D90); built on a round world, each base on its own square grid (D57); building together (D52); strength, perhaps.

**Technically**: structures stay separate from the terrain's smooth ground (D9). A base is half-metre cubes on its own grid, and every cube filled or emptied is an operation (D21); the design is [building-tech.md](building-tech.md) (D160).

## Claims

From the developers (D94; D104): you build within areas you claim, so the world is not randomly destroyed everywhere, nor managed by anyone. Built: a claim stone, 40 metres across, grown to 80 and 160 for a price; nothing can be built outside one; the base is begun at it.

Proposed, to react to:

- **Outside claims, the world heals.** You can dig and gather anywhere (D43), but what you change outside a claim grows back after a while, as Enshrouded's does after about half an hour.
- **Claims belong to a tribe**, its players and recruits (D166, [tribe.md](tribe.md#tribes-and-outposts)); others build there only if allowed, as Rust's tool cupboard lists who may. There is no player against player (D2).
- **Doors and storage obey the claim**: its owner and those allowed open them, as Valheim's ward does.
- **Claims are limited**: how many a player holds and how big they grow. Ark has none, and its shared servers fill with pillars put up only to block land: the warning for a thousand players.
- **Claims need keeping up**: a claim long abandoned fades and its ground heals, so a persistent world does not fill with dead bases. Rust's daily upkeep tax is disliked; a lit hearth, as Soulmask's bonfire, is gentler.
- **Claims are what the world attacks** (D90).

**Technically**: claims keep the world's stored state in proportion to what players hold, not to everything they ever touched (D47, D52).

## Easy to extend

A new thing to build should be data, not code (D56, D127):

| To add | It takes |
|---|---|
| A piece of furniture | One entry and a model file |
| A window size | One entry |
| A kind of construction on a pattern we have, such as a dark-wood cabin | A kit file and its piece models ([building-kits.md](building-kits.md)) |
| A new pattern of laying, such as dry stone | Code, once ([building-tech.md](building-tech.md#drawing-a-kind-by-role-from-its-kit)) |

A piece's look comes from what it is next to, laid by its kit's rule: corners, ends and openings choose their own pieces. What a cube is made of is its kind, so changing material in place is changing that property. Underneath, a base is its cubes and the list of operations that made them, so it can be saved and sent (D21); a saved layout is that list, placed again, and a village is one (D108).

## The grid

**One step for everything: half a metre** (D57), the cube (D181). A wall is a row of cubes, an opening is cubes left empty, an item snaps to the grid: "the smallest placeable kind of thing". The developers also want the grid to show on a wall when something is placed there; nothing hangs on a wall yet.

**Three sizes, from the player's side** (D48): the **cube**, half a metre, as Enshrouded's; the **brush**, how much one action places or carves, in several sizes as Enshrouded's 1, 2 and 4 metre pieces (built, D233); and **what is seen**, the kit's pieces, which stop it looking like a grid.

**You see the grid only near what you are placing**, as now.

## Sizes

What each part should measure (D175, Leaning). Realistic but fun (D122): the sizes real buildings have, rounded to the half-metre grid where a part is placed by it, so a player who knows a house can reason about ours. Everything is scaled to a person 1.8 metres tall, whose body is 0.6 metres across (a walker's radius of 0.3). Items have their own measured sizes and checked heights, in [`builds.ron`](../assets/data/builds.ron).

| Part | Now | Real buildings | Expected |
|---|---|---|---|
| Grid cell | 0.5 m | | 0.5 m (D57) |
| Storey, floor to floor | 3.0 m | 2.7 to 3.2 m; rooms 2.4 to 2.7 m high inside | 3.0 m, six cubes: the default height of a wall (D176, D181) |
| Wall, thick | A cube, 0.5 m | Wattle and daub 0.1 to 0.15; planks on a frame 0.1 to 0.15; logs 0.2 to 0.3; stone 0.4 to 0.6 | **A cube, 0.5 m** (D181), with the kit's pieces centred in it (D146): wattle and planks 0.15, logs 0.3 to 0.4, cut stone the whole cube |
| Post at a wall's end or corner | 0.16 m square | 0.1 to 0.2 m | As the wall's kind: a squared timber, a log end, a corner block |
| Door, the opening | 1.0 m by 2.0 m | About 0.8 to 1.0 m by 2.0 to 2.1 m | **1.0 m by 2.0 m** by default: two cubes by four, wide enough for a person with a margin. Wider doors, 1.5 and 2 m, for barns and gates |
| Window | 0.5, 1.5 and 3 m wide; sill 1.0 m, head 2.0 m; a tall one 0.5 to 2.5 m | Sill 0.8 to 1.0 m; head level with the doors' | By default **a sill at 1.0 m and a head at 2.0 m, level with the doors**, widths by the cube; any other size or shape carved (D181) |
| Floor, thick | A cube, 0.5 m | 0.1 to 0.3 m with its joists | A cube, 0.5 m (D181): boards on top, joists below |
| Roof | A cube a cube (45°), its cubes' slopes (D241); 0.15 m thick; eaves 0.4 to 0.5 m past its cubes | Thatch 45° and up, shingles and slates 30 to 45°, a lean-to 15 to 25° | 45° for every covering, as roof cubes slope; **eaves overhanging the walls by 0.4 to 0.5 m**, with a fascia; shallower pitches later |
| Ceiling, inside | 2.5 m | 2.4 to 2.7 m | 2.5 m: the storey less its floor cube |
| Steps | 0.17 m a step | 0.15 to 0.2 m a step, 0.25 to 0.3 m a tread | As now; stairs between storeys (step 4) the same |
| Foundation | 0.14 m proud of the walls, into the ground | | As now |
| Room drawn, smallest | 1 m | A corridor 0.9 to 1.2 m; a small room 2 to 3 m | 1 m inside, so 2 m drawn, with walls a cube thick |

Changing a size later is changing a number in the kit or the grid's constants, not the code.

## The build menu

As the developers want it (D67, D68): with the hammer in hand, tap the build menu key, and a panel of tabs shows what you can build, each tile with a picture, its name and its cost, greyed if you cannot pay and absent if you have not learned it. Click one, or use the arrows and Enter. The variant wheel (hold R) chooses among the variants of the thing in hand. Taking apart and moving are modes on their own keys, not entries. The last few things built sit at the top. The world never pauses. Built, with pictures drawn from each thing's model.

Tabs: Structure, first, with *Block* in hand by default (D253); Claim; Openings; Crafting; Furniture; Storage; Light and fire. Proposed: Defence, once there are fences and gates.

## One system for everything built

D108: everything built in the world, by anyone, is made of the same parts as what a player builds. A village's houses are walls, floors, roofs and doors from the build catalogue, put up by the same operations. A well is a buildable thing, a station rather than a structure. The test of a building system is that a settlement can be described in it; if something in a village cannot be, the catalogue grows.

The MVP's village is laid out from its seed by rules (`terra_town::village`, D229): a street village, the houses along both sides of a street with a green and a well, their doors to the street, each of a plan (a rectangle, an L, a T, a U, two storeys with an outside stair, a woodshed, a lookout deck on posts, a low garden wall), a scheme of the kits' kinds (logs, stone, hewn timber, logs on a stone plinth, stone under timber, log walls with stone ends) and a roof (gable, hip, lean-to), each stood on the ground where it goes on its footings, with steps from its door. About three hundred buildings can be told apart by those three and their width; the village has twelve houses, a shed, a deck and eight lengths of wall, every plan and scheme at least once (D229). What grows keeps out of every building and a margin round it, and off the street.

## Proposed order

For the developers to reorder. Each step is small enough to play and judge.

1. **The controls, locked, and placing finished**: the keys above, drawing, what blocks a ghost shown through walls, and snapping as [Snapping and feel](#snapping-and-feel) says. These hold whatever comes after.
2. **Plans, then constructing** (D139, built): walls, floors and roofs laid as plans, built part by part, from the pack and the claim's storage (D141, D295); fitting doors and windows still to come.
3. **Large materials** (D140): logs that lie where trees fall, carried on the shoulder; a log pile to keep them, later unlocked. How is open (D296).
4. **Up and down**: floors drawn on their own, posts, stairs and ramps (built, D220); ladders.
5. **Doors and defences**: doors that open and lock by the claim; fences, walls and gates drawn as a line.
6. **Damage and repair** (damage, collapse into fragments and repair built, D305), with the first attacks on a base (D90).
7. **More kinds of construction** (earth and wattle and daub built, D227; changing them in place built, D232); better transport, unlocked (D140).
8. **Carving and brushes** (built, D226, D233).
9. **Stability**, after the MVP (D142).

## Ideas for later

Not built, and not to be until the developers say.

**A plan view from above** (D176). Survival games build from the character's camera, and where they offer another it is for laying out, bounded by the base or a distance ([research/planning-mode-cameras.md](../research/planning-mode-cameras.md)). The idea: Plan mode (D170) lifts the camera into a view over the claim, panned and turned like The Sims', with walls cut away, for drafting only; the character stays standing in the world, can be hurt, and damage drops the view back. Building, carrying and putting parts in stay in the character's view. Risks: a second, pointer-led set of controls; seeing what the character cannot; being hurt while looking away.

**Heights, not storeys** (D176). With voxels (D48) and rooms of any height there are no fixed storeys, as the developers pointed out. The idea: what is seen and drawn on is a cutting plane, raised and lowered half a metre at a time and stopping at every floor there is, as an architect's section; above it is hidden. A wall has its own height, three metres unless dragged taller or shorter, as The Sims 4's walls do; a floor lies where the plane is; a roof rests on the walls under it. A storey becomes a default height, not a rule, and the same plane serves the character's camera too. In code it already is: height is counted in cubes (D181), and wall, floor and rooms are read from the cubes, a storey only the quick tools' default height (D277).

## Open questions

For the developers. Each has a recommendation.

1. **When each kind of construction is learned** (D68). Which there are is answered: logs and stone first, then hewn timber, earth, and wattle and daub (D227).
2. **Must parts go up in order**, what rests on something after it? Recommend yes.
3. **Are stations constructed too**, being large, or placed whole like furniture? Recommend placed whole for now.
4. **Cutting an opening in a finished wall**: with which tool, and does it give back what was cut? Recommend the axe for wood, the pickaxe for stone, giving it back.
5. **Which materials are big**, and how many does a player carry? D296, with its recommendations in [Large materials and carrying](#large-materials-and-carrying).
6. **Is a log cut into beams or planks first**, by the axe or at a station? Recommend at a sawbench, so a wall panel takes planks and a post takes a log.
7. **Taking apart**: always a full refund? Recommend yes, as now; Rust's window exists because of raiding, which we do not have (D2).
8. **What "capable of building" means** (D68): blueprints found, things scanned, a research tree, skill? The developers named Terraria: going to other zones, finding new materials, which unlock new things to craft and build, many of them only for their look. Realms (D87) would be our zones: each realm's materials open its own kinds of construction and decoration.
9. **Do bases have needs**, as Subnautica's: power, light, warmth?
10. **Who may change what** in a shared world? Recommend the claim's owner and whom they allow (D94).
11. **Claims**: how many per player, what keeps one alive, how fast the world heals outside them?
12. **Upkeep**: does a base decay if left? Recommend only an abandoned claim, slowly, never one in use.

## What is built

The game, and the yard for working on building (`cargo xtask yard`): see the game's [README](../crates/terra_game/README.md). Where each part stands is [The baseline](#the-baseline) and [The parts](#the-parts). Subnautica 2's controls and how ours map to them are in [research/subnautica-2-building.md](../research/subnautica-2-building.md); all five games are compared in [research/building-systems-prior-art.md](../research/building-systems-prior-art.md).
