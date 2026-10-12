# Construction

Status: the baseline building system, as a draft for the developers to read and correct before more of it is built. Asked for on 2026-10-12: "a baseline building system that implements the whole of it… so I'm not constantly saying you forgot this". It gathers everything the developers have said about building, everything players of the games we draw on will reach for ([research/building-systems-prior-art.md](../research/building-systems-prior-art.md)), and what is built. Each item says whether it is decided (with its D number), leaning, proposed by me, or open. Nothing proposed here is built on until the developers have read it.

## What the developers want

In their words, shortened.

- **The basis is a voxel grid, as Enshrouded's** (D48, 2026-10-11), with Subnautica 2's shaped rooms as the easier way in. Earlier: Subnautica 2, blended with Rust, Ark and other advanced building games.
- **Enshrouded is a role model** for building and voxels: "a ton of stuff we really might like to model off of". Building in voxels is fine; the terrain stays smooth voxels. See [research/enshrouded.md](../research/enshrouded.md).
- **The Forest's building is an inspiration** (D85, 2026-10-12): "a nice building system, the way you get logs/etc is nice for larger projects (vs sticks or something akin to that)".
- **Keep the prior art in mind for the controls too**: Enshrouded, Rust, Ark, Subnautica 2 (2026-10-12).
- **You pick the part you want to build, and can cycle its variants**, from "the little dial building menus we see in things like Rust and Subnautica". **A build menu like Subnautica's** (D67): you equip the building tool and it brings up what you have unlocked (D68).
- **Rooms are plain boxes**, with no lights or fittings. **A room placed across another extends it** to the combined shape.
- **Walls are a simple part**, indoors only. **Doors and windows are their own parts**, cut into any wall.
- **Furniture and kit are buildable items.** Everything built in the world, a village's houses included, is made of the same parts (D108); stations are the only prefabs.
- **Everything has a collision box**, used for placing as well as walking (D53).
- **A grid things snap to**, which can be switched off. New rooms snap to rooms already there. One square grid for everything, "a little flexible": in some games each cell is huge, "so you have no freedom to build". Half a metre for now (D57).
- **What is placed can be moved up and down**, not only turned, except what must stand on the ground (2026-10-11).
- **Only a foundation may go into the ground** ("I think"); since voxel terrain, a room set into the ground cuts it away instead (D131, Leaning). **Trees and rocks are cleared first** (D131, Leaning).
- **Details morph.** "Corners become rounded on the outside and inside, but when two segments are next to each other they're flat."
- **It must be really easy to extend**: "We might let you build a round room, or connectors, or different materials for rooms, and they should be able to change their look based on what they're connected to."
- **Some buildables are a fixed size**: a round room probably cannot be freely resized, and is reached through a connector, "which maybe auto-builds if you connect it to another room".
- **Building, taking apart and cutting take time**: click and hold, animated in and out.
- **Building costs things you carry**: "you have the raw items, which allow you to build anything that you might have the capabilities to build" (D55).
- **What does not fit in the inventory drops to the ground** (2026-10-12).
- **You build only inside a claim** (D94, Leaning), which starts small and grows (D104).
- **Structures are planned, then constructed** (D139, Decided, 2026-10-12): "for structures, you place a blueprint/plan of it, and then you have to actually construct it somehow… it makes it more realistic. So each wall you'd have to do that, and then doors are like carve outs so it can be slightly diff? Same with windows? We'll wanna think this through."
- **Constructing should feel immersive** (D139, 2026-10-12): "you'll actually go up and hit the thing with a hammer or similar? Or maybe it's like, you bring the resource and place it? Like you carry it, like The Forest / Grounded 2? We want it to feel a little immersive when you build up these structures."
- **The controls and plans proposed here "feel good"** (D144, 2026-10-12).
- **A blueprint shows its parts, and they can be small** (D139): "ideally a blueprint shows its 'parts' if you will, kind of like Forest shows the logs, and that can vary based on the type of construction".
- **Large structures take large materials, carried** (D140, Leaning): "you probably need large materials and can carry them? And later in the game maybe you unlock ways to transport them more efficiently."
- **Building draws on storage** (D141): "building from storage absolutely, but large items probably will need a special storage? And that probably would be unlocked later game (some of this)."
- **No stability in the MVP, but wanted later** (D142).
- **Rooms can be underground** (D143): "yes you can have underground rooms".

## The baseline

Everything a player arriving from Enshrouded, Rust, Ark, Subnautica 2 or The Forest will reach for, and where ours stands. This is the list to check the system against, so nothing is forgotten.

| Capability | Prior art | Ours |
|---|---|---|
| A tool that builds, and a menu of what you can build | All | Built: the hammer and the build menu (D67) |
| A variant wheel for the thing in hand | Rust, Subnautica | Built: hold R |
| A ghost before you commit, allowed or refused, saying why | All | Built; what blocks it is outlined, but not through walls, as Subnautica 2's is |
| Rotate | All | Built: wheel, or Z and C |
| Snap to a grid, with a switch to go free | Enshrouded, Ark, Valheim | Built: X, and hold Alt |
| Raise and lower, and nudge | Enshrouded, Ark Ascended | Raise and lower built; rooms by the storey, down too |
| Place in a line or an area: drawn walls, floors, rooms | The Forest, Sons of the Forest, Subnautica 2 | Built: rooms drawn as rectangles, walls as lines inside or out |
| Pick another of what you look at | 7 Days to Die | Built: middle mouse |
| Move what is placed | Subnautica 2 | Built for items, not rooms or walls |
| Take apart, giving back everything | Enshrouded, Subnautica, Valheim | Built; what does not fit drops at your feet |
| Undo for a short while | Enshrouded, Rust | Built: Y, within a minute |
| Repair | Rust, Valheim, The Forest | Missing: nothing built takes damage yet |
| Change a piece's material in place | Rust, 7 Days to Die | Missing: no materials yet |
| Plans that cost nothing, then constructed part by part | The Forest, Ark Ascended | Missing; decided (D139) |
| Building from storage nearby, not only the pack | Subnautica 2, Valheim | Missing; decided (D141) |
| Carrying big materials by hand, better transport later | The Forest | Missing; the direction (D140, Leaning) |
| A base level with the ground where it is begun | Ark Ascended's heights | Built: the base's grid is set where its claim stone goes |
| Building on slopes | All, differently | Built: rooms cut into the ground (D131) and stand on foundations below |
| Floors above and below, and stairs | All | Rooms on storeys above and below the base's floor built; stairs missing |
| Roofs | All | Generated with the hull; nothing to place by hand |
| Doors that open, close and lock | All | Doorways only |
| Fences, gates, palisades | Ark, The Forest, Rust | Missing |
| Stability | Rust, Valheim, Sons of the Forest | None in the MVP (D142); wanted later |
| A claim with an area, an owner and who else may build | Enshrouded, Rust, Valheim | Area and owner built; nobody else can be allowed yet |
| Upkeep or decay | Rust, Ark | None; open |
| Saved layouts placed again | Ark Ascended (2026), mods elsewhere | Missing; the village is one (D108) |
| A camera for building | Enshrouded (2025) | The zoom only |
| Stations and furniture placed as items | All | Built |

## Controls

Agreed by the developers on 2026-10-12 (D144: "generally speaking your plans feel good"), after asking that they be locked down; how a part is put into a plan is the one part still being worked through (D139). One scheme for everything in this document, on mouse and keyboard, after what the prior art agrees on, keeping what is built where it already works. Every key can be changed in the options (D116) but the wheel and the numbers. A gamepad scheme comes later and follows the same states.

**The rule underneath: a click lays, work builds.** A click puts down a plan (D139), which costs nothing and can be undone at once. Spending materials takes work: setting a carried part, swinging the hammer at a part, holding to build an item, take apart or repair. So a misclick never costs anything.

### Always, with the hammer in hand

| Input | Does | Built |
|---|---|---|
| B, or tap right mouse | Open or close the build menu. In it: click a tile, or 1 to 7 for a tab, the arrows and Enter; Escape or right mouse closes | Yes |
| Hold R | The wheel of the picked part's variants: move the mouse to one and let go. Tap R for the next, Shift and R for the one before | Yes |
| Backspace | Put the part away: the hammer in hand with nothing picked | Yes |
| X | Grid on or off | Yes |
| Hold Alt | The other of the grid's settings, while held | Yes |
| Middle mouse, or V | Pick another of what the view is on: its part, variant, turn, height and material | Part only |
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
| Shift and wheel, or Page Up and Page Down | Raise and lower: half a metre a notch on the grid, a tenth off it, up to three metres; rooms a storey at a time, two down to two up (D143). Stations and the claim stone stay on the ground | Yes |
| Ctrl and wheel | The kind of construction (D139) or material. Ctrl also rolls (D97), so while a part is picked it should not; to settle when there are kinds | No |
| Click left mouse | Lay it: a plan, for a structure; for an item, hold instead | For rooms and walls, laid as a ghost that stays; holding on it builds it whole, as there are no plans yet |
| Press, drag and let go | Draw it: a wall or fence along the line dragged; a room or floor over the rectangle dragged. A plain click lays the variant's own size | For rooms and walls |
| Hold left mouse (items) | Build the item there, spending its cost | Yes |
| Tap right mouse | Put the part away (or, carrying something in move mode, put it back) | Yes |

After a placement the same part, variant, turn and height stay in hand, so the next goes down at once.

### At a plan

| Input | Does | Built |
|---|---|---|
| Aim at a plan | Its next part is outlined, and the prompt says what it is, what it needs and how many are within reach: "Log: 3 of 12. 9 more needed, 4 in reach" | No |
| Carrying a big part, left mouse at its outline | Set it in place (see [how a part goes in](#planned-then-constructed)) | No |
| Swing the hammer (left mouse) at a set part, or at a plan's next small part | Fix it, or put it in: a few swings each, the fixings from the pack or storage in the claim (D141) | No |
| Q, then hold left mouse | Take the plan away, and everything put into it comes back | No |

### Taking apart (Q)

| Input | Does | Built |
|---|---|---|
| Aim | What would come away is outlined: an item, a part of a structure (the last put in, so it comes down in the reverse order), or a whole plan | Items and rooms |
| Hold left mouse | Take it apart, giving back what it cost; what does not fit drops at your feet | Yes |
| Shift and hold left mouse | Take apart the whole of it: a wall, or a room, part after part | No |

### Moving (G)

| Input | Does | Built |
|---|---|---|
| Click an item | Pick it up; it follows the aim like a part being placed, and turns and rises the same way | Yes, but rising |
| Click | Put it down | Yes |
| Tap right mouse | Put it back where it was | Yes |
| Click a plan | Pick the whole plan up and move it, while nothing has been put into it | No |

What is constructed stays where it is built; to move a wall, take it apart and lay it again.

### Repairing

With the hammer in hand and nothing picked, aim at something damaged: its damage shows, and holding left mouse mends it for a little of its cost (D90; not built, since nothing takes damage yet).

### To lock down

The calls for the developers, each recommended as written above:

1. **Click lays, hold builds**: a click puts down a plan for free; holding spends materials (putting in parts, building an item, taking apart, repairing).
2. **Drawing**: press, drag and let go to lay a wall or fence along a line, or a room or floor over a rectangle, in the grid's four directions (D146).
3. **Height on Shift and the wheel**, with Page Up and Page Down; **the kind of construction on Ctrl and the wheel**.
4. **Undo on Y**, within a minute (Ctrl and Z would roll, so Enshrouded's key instead; built).
5. **Right mouse stays the build menu** and **Q stays take-apart**, as Subnautica, not Enshrouded.
6. **F opens and closes doors**, as it uses everything else.
7. **Constructed things are not moved**, only plans and items; a wall is taken apart and laid again.

### Where ours differs from the prior art

Right mouse is our build menu, as Subnautica's (D67), where Enshrouded's right mouse takes apart; Q is our take-apart mode, where Enshrouded's Q nudges. Keeping ours is recommended: they follow the game the developers named for the menu, and they are built.

## Snapping and feel

What makes placing feel good is mostly that the ghost goes where you meant without a fight. Proposed:

1. **What it snaps to, in order.** First the joins of what is already built or planned, within reach of the aim: the end of a wall, the edge of a floor or room, a frame in a wall, a socket on a fixed buildable. Then the base's grid (D57). Then, for items, the surface under the aim: floor, ground, a table's top, a wall. With the grid off, or Alt held, only surfaces.
2. **It sticks.** Once snapped, the ghost stays put until the aim has moved well off that spot, so it never flickers between two places. Moving on to the next spot along a wall is a short, clean step. (Built for rooms.)
3. **It glides.** The ghost slides to each new place in a few hundredths of a second instead of jumping, but the outline of where it will land is always exactly where it will land.
4. **Structures aim on a level.** A room or wall is aimed on the base's floor at the height chosen, not on the bumps of the ground under the aim, so it does not jump as the view crosses grass and stones. Items aim at what is under the view. (Built for rooms.)
5. **Out of reach, it waits.** Aimed beyond reach, the ghost stays at the edge of reach along the aim, dimmed, instead of vanishing. (Built for rooms, not yet dimmed.)
6. **It faces sensibly.** A wall, door or window faces the player; an item faces back toward them the first time, and remembers its turn after.
7. **It shows what it is doing.** The grid shows only around the ghost, fading out a few metres off. The join it snapped to lights up. Lines appear when it lines up with an edge already built. A small arrow marks its front. Off the floor, a line drops to what is under it and the height is shown.
8. **It says no plainly.** Red, one line saying why, and what is in the way outlined, behind walls too. Blue otherwise, with its cost and what is in reach.
9. **It sounds right.** A soft tick as it snaps to a new place, a different one when it turns, and the thud of laying a plan or putting in a part.
10. **The hand is steady.** Every value above, how sticky, how fast the glide, how far the grid shows, is tuned by playing, and kept as data so it can be changed without code.

## Planned, then constructed

The developers' direction (D139, Decided): a structure is first laid out as a plan, and then actually constructed, part by part. What follows is my working-through of it, to react to; nothing here is settled.

**Laying a plan.** Everything structural that the build menu places, a room, a wall, a floor, a fence, goes down as a plan: a pale ghost in the claim that costs nothing and blocks nothing yet. Lay as many as you like; move them, resize them, take them away for free. A plan shows, part by part, what it needs. This is also how a whole base is tried out, and its full cost seen, before anything is gathered (The Forest, Ark Ascended's planning mode).

**Constructing it.** A plan shows its parts, the way The Forest's blueprint shows each log (D139): every piece that will make it is outlined in the ghost, and each is put in one at a time. Stand at the plan with the hammer and what the next part needs, in your pack or in storage nearby (D141), and hold the build key: that part goes in, its material is spent, and the outline fills. Parts can be small, a single log or a single stone.

**What the parts are depends on the kind of construction.** Each kind of construction says how a wall, floor or roof of it is made, as data, like everything else built (D56, D136):

| Construction | A wall is | A floor is | A roof is |
|---|---|---|---|
| Logs | Logs laid one on another, each as long as the wall or a log can be, notched at the corners | Logs side by side on beams | Poles, then a covering |
| Planks | Posts at the corners and every so far, then planks across them | Beams, then planks | Rafters, then planks or shingles |
| Stone | Courses of cut stone, each block a part | Flagstones on a bed | Beams, then slates |
| Wattle and daub (early) | Stakes, woven sticks, then a coat of mud | Packed earth | Thatch on poles |

So a log cabin's wall of 4.5 metres, a storey high, is about a dozen logs, put in from the bottom up; a stone wall is many more, smaller parts. The order is the physical one: what a part rests on goes in first. That is not a stability check (D142), only which outline is next.

**How a part goes in: carried, set, hammered.** The developers want it to feel immersive, and named both ways: bringing the material and placing it, as The Forest and Grounded 2, and going up and hitting it with a hammer. Then, settled for large construction (2026-10-12): "it'd originally be placed as a blueprint, and you'd have to then bring the materials to it to build it (kind of like the forest or grounded 2)". The rest of this is a proposal, by the size of the material, which is how building is really done:

- **Big parts are carried and set.** A log, a beam or a cut stone is brought to the plan, on the shoulder (D140) or picked up from a pile within reach, and set into the outline it fits: you see it go in, and the outline fills.
- **Then the hammer fixes it.** A few swings at what was just set peg, nail or mortar it in, the small fixings coming from the pack or storage (D141). Until it is fixed, the part is in place but loose: it can be lifted out again freely.
- **Small parts are only hammered.** Wattle, planks, shingles and thatch come from the pack or storage; swinging the hammer at the plan puts the next one in with each few strikes.
- **Recruits do the same**, visibly, when the materials are near (D90).

Open: how many swings a part takes, and whether fixing is a separate step or the last swing of setting it.

**Drawing a plan.** Asked about on 2026-10-12: "how would dragging to draw work exactly? … say it's a stone wall, we'll need the stone to look modular". Proposed:

- **A wall or fence is drawn as a line.** Press where it starts, drag, let go where it ends. The start snaps to a join (the end of a wall already there) or the grid; the line runs along the base's grid, in its four directions only for now (D146), a grid step (half a metre) at a time, so every wall is a whole number of steps long. While dragging, the plan is shown already split into its parts, with its length and what it needs: "4.5 m stone wall: 54 stones, 12 in reach". Shift and the wheel, while dragging, set how high, a course or a storey at a time.
- **A room or floor is drawn as a rectangle**: press at one corner, drag to the other. A room's plan is its floor, its four walls and its roof, each split into parts (and, by D152, laid as those, not as a room).
- **Drawing on from a wall's end continues it**, and closing a loop of walls offers to make it a room.
- **A plain click lays the variant's own size**, as now: a 4.5 metre wall, a 3 by 3 metre room.

**Walls, not rooms** (D152, Decided). The developers, 2026-10-12: "maybe we should get rid of 'rooms' and only allow walls? … a room should be N walls anyways, it's a shortcut … it's weird to place blueprints of rooms". Then: "blueprints need to be fundamentally walls/floors/roofs, and you build up piece by piece instead of prebuilt. it just feels better". How placing them works is [the next question](#enshrouded-or-subnautica-how-placing-works) (D154). Proposed:

- **The parts are walls, floors and roofs**, and later foundations and stairs. A room is not a thing that is built; it is what walls, a floor and a roof enclose.
- **The room in the build menu is a shortcut**: a rectangle drawn with it lays its walls and its floor as plans, each its own, as if drawn one by one. Closing a loop of walls drawn by hand offers the same floor.
- **A roof is laid over what is enclosed**, offered when the walls close, or drawn as a rectangle like a floor.
- **Indoors is worked out**: inside walls and under a roof, by the grid. It is what shelters from weather and lets interior things go in.
- **Into the ground**: a wall or floor laid where the ground is high cuts it away, as rooms do now (D131); a cellar is the ground dug out, then walled.

**What a building is** (D155). The developers, 2026-10-12: station expansions count "anywhere in the building … buildings will need to know if they're 'complete' (aka walls/roof) anyways because otherwise you have natural elements like rain/cold that should have effect". So the game keeps track of buildings:

- **A building** is the walls, floors and roofs that touch one another. Two separate cabins in a claim are two buildings.
- **Its rooms** are the spaces enclosed by its walls, floor and roof. A room that is closed all round is **indoors**: sheltered from rain, wind and cold (D78), and where interior items go. A room missing its roof, or with a gap in its walls, is not, and says what is missing ("No roof over this room").
- **Doors and windows**: an open doorway lets weather in; a fitted door, shut, keeps it out, and an open window lets in a little (D139).
- **Stations** belong to the building they stand in: their expansions count anywhere in it (D151).

Technically (mine): worked out from the grid when a part is built or taken away, not every frame (D127): the parts that touch, flood-filled into buildings; the cells inside each, flood-filled from its floor, are a room unless they reach the open air. Each building is then an entity with its rooms, which weather, warmth and stations read.

The technical design is [structures.md](structures.md) (D160). Built, 2026-10-12: a base is walls on the edges between cells and floors and roofs on cells, with rooms worked out from the walls; the room tool is the shortcut, and walls are drawn inside or out.

**How a material is split into parts, so it looks built and not tiled.** Each kind of construction is a small kit of pieces sized to the grid, and a rule for laying them, as real masonry and carpentry have:

| Kind | Pieces | Laid |
|---|---|---|
| Cut stone | A block a metre long and half a metre high and deep; a half block; a corner block; a sill and a lintel for openings; a capstone | In courses half a metre high, each course offset half a block from the one under it (a running bond), half blocks at the ends of every other course, corner blocks interlocking where two walls meet |
| Logs | Logs as long as the wall, up to the longest a tree gives (say four metres), notched at the ends | Stacked, a log a course; longer walls splice at a post; corners interlock, the logs of one wall alternating with the other's |
| Planks | Posts, rails, planks | Posts at the ends and as evenly as fits within a metre and a half; planks between them |
| Wattle and daub | Stakes, woven panels, daub | Stakes every half metre, panels between, daub over |

Because every wall is a whole number of half metres, the kit always fits: a stone wall of 4.5 metres is four whole blocks and a half in one course, a half and four whole in the next. Corners and openings choose their own pieces by what they are next to, which is the morphing the developers asked for (D56). Each piece gets a little variety, a slightly different shape, turn or shade from a few made versions, so a long wall does not repeat.

**What the pieces are made of** (D146, decided 2026-10-12). Each piece is a model from its kind's kit, made once by script (as the pickaxe and torch are) with a few versions each, set on the base's grid. The pieces are parts of the blueprint picked in the build menu, never things made first at a station and then placed: the developers do not want to "build walls then place them". The grid is still the skeleton (D48, D57): it says where each piece is, what is inside and out, what blocks a walker, and what an opening cuts. The pieces are what is seen and what goes in one at a time.

**Carving, later.** The developers may want to carve what is built, and agreed it can come later. Carving by the grid's half metre needs nothing new: it takes cells out of the skeleton and the kit lays itself again around the gap, a cut block becoming a half block, as doors and windows are cut now. Carving finer than the grid, chiselled or smoothed, needs the material drawn as voxels, Enshrouded's way; our ground already is (D48), so it would come as a kind of construction of its own, such as carved rock, where the pieces sit on the voxel volume where they meet it. Nothing about the kit stops either.

**Building in bulk.** Putting in parts one by one is the realistic way and must not become a chore: holding the build key keeps putting in the next part while you have the materials; a skilled builder (D88) puts them in faster; recruits put them in for you (D90). The Forest's later option to skip each part's animation is the lesson: show each part going in, quickly.

**Doors and windows are carved out** (the developers' "carve outs, so it can be slightly different"):

- **On a plan**, an opening is planned into the wall: the parts it crosses are shortened or left out, so the wall needs less, and a frame is added to build.
- **In a finished wall**, an opening is cut: it takes work with a tool, gives back the material of what was cut away, and leaves a frame to build.
- **The door or the glass is its own item**, made at a station and fitted into the frame. A doorway without a door is just a gap; a window frame without glass is open to the weather.

**Small things are not planned.** Furniture, storage, lights and stations are placed whole, as now: they are made, then set down. Open whether stations should be constructed too, being large.

**Taking down.** A constructed part comes down to its plan, giving back its materials (or dropping them at your feet if they do not fit). A plan with nothing built is removed for free. Undo works on both.

**Who constructs.** Anyone allowed in the claim (D94), and recruits who work at the base (D90), as Sons of the Forest's companion finishes blueprints, but never a draft still being designed (D170, Leaning; [tribe.md](tribe.md#plan-and-build-handing-work-to-the-tribe)). A claim-wide list of what all its plans still need, as The Forest's counter, tells a group what to go and get.

**On the voxel grid** (D48): constructing a part writes its blocks; a plan is blocks not yet written. Carving an opening is removing blocks. A kind of construction's parts are how the brush is sized for structures.

## Large materials and carrying

The developers' direction (D140, Leaning; D141):

- **Big structures take big materials.** Logs for walls and posts, cut stone blocks for stone walls, beams for roofs. Small materials (fibre, nails, hide) still go in the pack.
- **Big materials are carried, not packed.** A felled tree gives logs that lie where it fell. You carry one on your shoulder, slowly, or two with effort, with nothing else in your hands, as The Forest does. A big building becomes a project you can see: trees down, logs dragged home, walls rising. A second player has a job.
- **Later, better ways to move them** are unlocked (D68): a sled, then a cart, then perhaps a beast of burden (D100's mounts), each carrying more.
- **Building draws on storage** (D141, decided): within the claim, constructing a part takes what it needs from storage nearby, not only from the pack.
- **Big materials need their own storage**, unlocked later (D141, Leaning): a log pile or a stone yard, which holds what a chest cannot. Before it, big materials lie on the ground where you drop them, near the build, and building takes from what lies within reach.

Open: which materials are big; how many a player carries; whether a log is cut into planks or beams first (Sons of the Forest cuts them with the axe; a sawpit or sawbench is the station version).

## Placing

- **The grid** is half a metre (D57; see [The grid](#the-grid)). Things snap to it and turn in eighths; off it, they turn freely.
- **A base stands level where its claim stone is put** (built, 2026-10-12): its grid is set there, level with the ground at the stone. Every room of a base shares its floor height and stands on a foundation where the ground falls away (built). **Rooms can be underground** (D143): a room may be placed a storey or more down, cutting the ground away, for cellars and for terraces down a slope.
- **Into the ground**: a room cuts the ground away where it stands, no deeper than its own height (D131, Leaning). Trees and rocks must be cleared first (D131, Leaning); small plants clear themselves.
- **Up and down**: anything placed can be raised and lowered (asked for), on the grid in half-metre steps. What must stand on something says so: a chair on a floor, a station on the ground.
- **What may not overlap**: an item may not go into another item or a wall (built); a room may not go over a solid thing set down outdoors, such as the claim stone, which is outlined (built).
- **What the ghost says**: blue when it can go, red when not, with one line saying why and what it costs; what is in the way outlined, through walls too (proposed, after Subnautica 2's first update).

## The parts

The baseline catalogue. Everything is an entry in a list (D56), drawn by what it is next to.

| Part | Status |
|---|---|
| Room: a box of any size on the grid, grown across others, walls, floor and roof generated | Built |
| Interior wall, window, door, pushing a wall out or in | Built; doors do not open |
| A storey up, and stairs or a ladder to it | Missing; the model has storeys |
| Roof shapes beyond flat | Missing; open |
| Foundations, and posts for building out over a drop | Foundations generated; posts missing |
| Fence, wall, gate and palisade, drawn as a line | Missing; needed once bases are attacked (D90) |
| Door that opens, closes and locks | Missing |
| Stations: workbench, smelter, forge, campfire | Built |
| Furniture, storage, lights | Built |
| Claim stone | Built |
| Fixed buildables, ports and connectors (round rooms) | Missing; see [The grid](#the-grid) |
| Blocks and brushes on the voxel grid (D48) | Missing; the engine work below |

## Changing what is built

- **Taking apart** gives back everything it cost (built); what does not fit drops at your feet (built).
- **Undo**: the last thing built or taken apart, within a minute (built, on Y).
- **Moving**: items now (built); rooms and walls as a whole later (open).
- **Repair**: once bases take damage (D90), the hammer mends what it built, for a little of its cost (proposed). Tools and gear already wear and are mended at stations (D99).
- **Material**: changing a part's material in place, paying the difference (proposed; material is a property, not a different part, below).

## Stability

None in the MVP, and wanted later (D142). Rust, Valheim and Sons of the Forest make structures hold themselves up; Enshrouded and Subnautica do not; Ark does but hides its rules, which players complain about. When it comes: shown only while building, as Valheim's colours; a part that holds others up cannot be taken out from under them, as Sons of the Forest's; and constructing in order (floor, walls, roof) already reads as physical without it.

## Enshrouded or Subnautica: how placing works

Decided (D154): The Sims' way, below. The developers, 2026-10-12, with walls, floors and roofs settled as the parts (D152): "the only question we have is how it should work. like enshrouded style, or subnautica style. i do like enshroudeds flexibility to design something truly interesting/unique/beautiful but i will admit its also a little overwhelming".

Earlier, on 2026-10-11: Enshrouded's building might be the better base concept than Subnautica 2's, and the basis was settled (D48): **a voxel grid, as Enshrouded's**. "Subnautica 2 is just easier, but as long as the basis is almost like this voxel grid we could have flexibility in time." What Enshrouded's does, from [research/enshrouded.md](../research/enshrouded.md): buildings of small voxel blocks in a material, placed one at a time or as ready-made pieces; pieces join seamlessly and materials blend where they meet; cutting an opening styles its edges; the ground can be dug, filled and flattened; you build only within an altar's radius.

| | Subnautica 2 | Enshrouded |
|---|---|---|
| What you place | Fixed modules that snap together | Blocks and ready-made pieces, in any shape |
| Freedom | Little: the modules' shapes | Any shape, down to one block |
| Speed for big things | Fast | Slower, unless ready-made pieces are used |
| What makes it look good | The modules' design | The player's skill and time |
| Learning it | Minutes | Long; many shapes, and roofs by hand |

**Why Enshrouded's overwhelms**, as far as we can tell: the player makes every detail by hand, from many shapes; roofs are built a piece at a time; nothing helps a beginner's house look good. Its freedom is in the shape of the building; its effort is in the detail.

**Agreed: The Sims' way, with our materials.** You draw the shape of the building; the game makes the details. The developers find The Sims "not bad" (2026-10-12). Building a log cabin would go:

1. **Walls.** Pick *Wall*, log. Press and drag 6 m: a wall's blueprint, shown log by log. Drag on from its end, round three more sides; when the loop closes, the game offers a floor inside, a click to accept.
2. **Roof.** Pick *Roof* and click inside the walls: a roof's blueprint covers what they enclose. The wheel picks its style (gable, hip, flat, lean-to), Shift and the wheel its pitch. Its rafters, ridge and gable ends are worked out.
3. **Openings.** Pick *Door* or *Window* and click a wall: the opening is planned into it, and the wall's logs shorten round it.
4. **Construct.** Bring logs, set them, hammer them in (D139). The cabin goes up log by log.

| You decide | The game decides |
|---|---|
| Where every wall goes, how long and how high, in which material | How the logs or stones of each wall are laid, and how corners interlock |
| Floors and storeys | Corner posts, trims, sills and lintels |
| Where doors and windows go, and their size | How the wall is cut round them |
| The roof's style and pitch, over what you enclosed | Its rafters, ridge, eaves and gable ends |
| Stairs, foundations, what goes inside | What is indoors, sheltered from rain and cold (D155) |

**Against the other two**, for the same cabin:

- **Subnautica's way**: you place a ready-made room module of a fixed size and join modules with corridors. Quick, but every cabin is the same.
- **Enshrouded's way**: you place each wall section and each block, then build the roof slope by slope, with ridge pieces, gable ends and trims by hand. Any shape is possible; that is the freedom the developers like, and the overwhelm.

Ours gives the same freedom of shape as Enshrouded's, any footprint, any length, any height, many storeys, without making every detail by hand. **Later, with progress**: arches, round and diagonal walls, carving (D146), more roof styles and materials are learned (D68), as Terraria's are (D85); a builder who wants a detail of their own can place it, such as a piece of roof or trim by hand. And a building can be saved as a design and laid again.

**Where we improve on it**, agreed by the developers on 2026-10-11: shape rooms, then carve them; bases that matter, held, defended and lived in (D90); built on a round world, each base on its own square grid (D57); building together (D52); strength, perhaps.

**Technically** (mine; D9): structures stay separate from the terrain's smooth ground, but a base can be a voxel volume of its own: blocks on the base's grid, meshed by their own rules, blended with the ground where they meet. Only the chunks with something built in them are kept, and every block placed or carved is an operation (D21). The room tiles in `terra_build` become one way of writing blocks. This gets its own technical design before it is built.

## Claims

From the developers (D94, Leaning; D104): you build within areas you claim, so the world is not randomly destroyed everywhere, nor managed by anyone. Built: a claim stone, 40 metres across, grown to 80 and 160 for a price; nothing can be built outside one; the base is begun at it.

Proposed, to react to:

- **Outside claims, the world heals.** You can dig and gather anywhere (D43), but what you change outside a claim grows back after a while, as Enshrouded's does after about half an hour.
- **Claims belong to a tribe**, its players and recruits (D166, [tribe.md](tribe.md#tribes-and-outposts)); others build there only if allowed, as Rust's tool cupboard lists who may. There is no player against player (D2).
- **Doors and storage obey the claim**: its owner and those allowed open them, as Valheim's ward does.
- **Claims are limited**: how many a player holds and how big they grow. Ark has none, and its shared servers fill with pillars put up only to block land: the warning for a thousand players.
- **Claims need keeping up**: a claim long abandoned fades and its ground heals, so a persistent world does not fill with dead bases. Rust's daily upkeep tax is disliked; a lit hearth, as Soulmask's bonfire, is gentler.
- **Claims are what the world attacks** (D90).

**Technically** (mine): claims keep the world's stored state in proportion to what players hold, not to everything they ever touched (D47, D52).

## The shape proposed

Four ideas (D56, Leaning).

1. **A buildable is one entry in a list**: its name, place in the menu, variants, cost, where it may go and how it is drawn. Adding a couch, a connector or a round room is adding an entry.
2. **Where a thing may go is a rule the entry names**: on open ground or across a room; on a wall; on the edge between two indoor tiles; on a floor, the ground, or another item. When a rule refuses, the thing shows red and the game says why.
3. **A thing's look comes from what it is next to**: walls in a line are flat with a seam; outside corners round outward, inside corners inward; a wall ending against nothing is capped. Written once for walls, it applies to anything that is a wall.
4. **Material is a property, not a different buildable.** A room in stone is the same room with a different skin and cost. Changing it in place is changing that property.

Underneath, the base stays a set of tiles, the walls and openings on them, and the list of operations that made it, so it can be saved and sent (D21). A saved layout is that list, placed again; a village is one (D108).

## The test of "really easy to extend"

| To add | It should take |
|---|---|
| A piece of furniture | One entry and a model file. True today. |
| A window shape | One entry. True today. |
| A material for rooms | One skin entry |
| A new structural part on the grid, such as a connector | One entry, its rule, and its shapes |
| A new kind of morphing, such as a roof edge | One rule, applied to everything of that kind |

## The gap

What is built, against all of the above: see [The baseline](#the-baseline). In short, the parts and placing are there; missing are height, undo, plans and constructing part by part, big carried materials, building up and down, doors that work, defences, repair, materials, permissions, and the voxel blocks themselves.

## The grid

**One step for everything: half a metre** (D57). A wall moves by it, a room is sized in it, a window or door is a run of its cells, an item snaps to it: "the smallest placeable kind of thing". The developers also want the grid to show on a wall when something is placed there; nothing hangs on a wall yet.

**Three sizes, from the player's side** (with the voxel grid as the basis, D48): the **block**, half a metre, as Enshrouded's; the **brush**, how much one action places or carves, in several sizes as Enshrouded's 1, 2 and 4 metre pieces; and **what is seen**, where blending, bevels and rounded corners stop it looking like a grid.

**Two kinds of structural buildable**: free-form ones, of any size, resized by pushing walls and extended by building across them; and fixed ones, one footprint as designed (a round room, a dock), joined to others only by **ports**. Two ports facing each other join; a port facing a free-form wall cuts its own doorway; one facing something further off is joined by a **connector**, which the game builds itself when a fixed buildable is placed in line with something. Not built.

**You see the grid only near what you are placing**, as now.

## Sizes

What each part should measure (D175, Leaning: proposed 2026-10-12, when the developers asked for the expectation to be written down). Realistic but fun (D122): the sizes real buildings have, rounded to the half-metre grid where a part is placed by it, so a player who knows a house can reason about ours. Everything is scaled to a person 1.8 metres tall, whose body is 0.6 metres across (a walker's radius of 0.3). Items have their own measured sizes and checked heights, in [`builds.ron`](../assets/data/builds.ron).

| Part | Now | Real buildings | Expected |
|---|---|---|---|
| Grid cell | 0.5 m | | 0.5 m (D57) |
| Storey, floor to floor | 3.0 m | 2.7 to 3.2 m; rooms 2.4 to 2.7 m high inside | 3.0 m |
| Wall, thick | 0.10 m, every wall | Wattle and daub 0.1 to 0.15; planks on a frame 0.1 to 0.15; logs 0.2 to 0.3; stone 0.4 to 0.6 | By kind of construction, from its kit (D146): wattle and planks 0.15, logs 0.3, cut stone 0.5 (a block deep). Centred on the grid line, so a thick wall takes from the rooms either side |
| Post at a wall's end or corner | 0.16 m square | 0.1 to 0.2 m | As the wall's kind: a squared timber, a log end, a corner block |
| Door, the opening | 1.5 m wide, 2.2 m high | About 0.8 to 1.0 m by 2.0 to 2.1 m | **1.0 m by 2.1 m**: two cells, wide enough for a person with a margin. Wider doors, 1.5 and 2 m, for barns and gates |
| Window | 0.5, 1.5 and 3 m wide; sill 0.9 m, head 2.5 m; a tall one 0.2 to 2.8 m | Sill 0.8 to 1.0 m; head level with the doors' | Widths by the cell; sill 0.9 m, **head 2.1 m, level with the doors**; tall windows to the floor |
| Floor, thick | Drawn flat | 0.1 to 0.3 m with its joists | 0.2 m, so a floor between storeys reads as one |
| Roof | Pitch 0.7 rise a metre (35°), a lean-to 0.4 (22°); 0.1 m thick; no overhang | Thatch 45° and up, shingles and slates 30 to 45°, a lean-to 15 to 25° | Pitch by covering: thatch steep, slate and shingles 35°; **eaves overhanging the walls by 0.3 to 0.5 m**, with a fascia |
| Ceiling, inside | 3.0 m to the roof or floor above | 2.4 to 2.7 m | 2.8 m: the storey less the floor above |
| Steps | 0.17 m a step | 0.15 to 0.2 m a step, 0.25 to 0.3 m a tread | As now; stairs between storeys (step 4) the same |
| Foundation | 0.14 m proud of the walls, into the ground | | As now |
| Room drawn, smallest | 1 m | A corridor 0.9 to 1.2 m; a small room 2 to 3 m | 1 m, as now |

Changing a size later is changing a number in the kit or the grid's constants, not the code; the doors and windows that do not match yet are the first change when kits come (step 4 of [structures.md](structures.md)).

## The build menu

As the developers want it (D67, D68): with the hammer in hand, tap the build menu key, and a panel of tabs shows what you can build, each tile with a picture, its name and its cost, greyed if you cannot pay and absent if you have not learned it. Click one, or use the arrows and Enter. The variant wheel (hold R) chooses among the variants of the thing in hand. Taking apart and moving are modes on their own keys, not entries. The last few things built sit at the top. The world never pauses. Built, with pictures drawn from each thing's model.

Tabs: Claim; Structure; Openings; Crafting; Furniture; Storage; Light and fire. Proposed: Defence, once there are fences and gates; Blocks and brushes, with the voxel grid (D48).

## One system for everything built

D108: everything built in the world, by anyone, is made of the same parts as what a player builds. A village's houses are rooms, walls, roofs and doors from the build catalogue, put up by the same operations. A well is a buildable thing, a station rather than a room. The test of a building system is that a settlement can be described in it; if something in a village cannot be, the catalogue grows.

## Proposed order

For the developers to reorder. Each step is small enough to play and judge.

1. **The controls, locked, and placing finished**: the keys above, raise and lower, undo, drawing, the room-over-a-placed-thing check, what blocks a ghost shown through walls, and snapping as [Snapping and feel](#snapping-and-feel) says. These hold whatever comes after.
2. **Plans, then constructing** (D139), with walls, floors and roofs as the parts rather than rooms (D152): rooms and walls laid as plans, built part by part from the pack and from storage in the claim (D141); doors and windows carved out and fitted.
3. **Large materials** (D140): logs that lie where trees fall, carried on the shoulder; a log pile to keep them, later unlocked.
4. **Downward and upward**: rooms underground (D143), a storey up, stairs and ladders.
5. **Doors and defences**: doors that open and lock by the claim; fences, walls and gates drawn as a line.
6. **Damage and repair**, with the first attacks on a base (D90).
7. **Materials**, and changing them in place; better transport, unlocked (D140).
8. **Voxel blocks and brushes** (D48), with plans and parts written as blocks: the engine track, with its own technical design.
9. **Stability**, after the MVP (D142).

## Open questions

For the developers. Each has my recommendation.

Answered on 2026-10-12: blueprints are walls, floors and roofs, built up piece by piece, not rooms (D152). Placing The Sims' way: the player draws, the game makes the details ([above](#enshrouded-or-subnautica-how-placing-works), D154).

Answered on 2026-10-12: structures are planned, then constructed, a blueprint showing its parts, which can be small and depend on the kind of construction (D139); big materials are carried, with better transport later (D140); building draws on storage, with special storage for big materials later (D141); no stability in the MVP, but later (D142); rooms can be underground (D143). A kind's pieces are kit models, parts of the blueprint, not made first and placed; walls in four directions for now; carving later (D146).

1. **The kinds of construction**: which there are, and when each is learned (D68). Recommend wattle and daub, then logs, then planks, then stone for the MVP's first two.
2. **Must parts go up in order**, what rests on something after it? Recommend yes.
3. **Are stations constructed too**, being large, or placed whole like furniture? Recommend placed whole for now.
4. **Cutting an opening in a finished wall**: with which tool, and does it give back what was cut? Recommend the axe for wood, the pickaxe for stone, giving it back.
5. **Which materials are big**, and how many does a player carry? Recommend logs and cut stone; one comfortably, two at a walk.
6. **Is a log cut into beams or planks first**, by the axe or at a station? Recommend at a sawbench, so a wall panel takes planks and a post takes a log.
7. **Taking apart**: always a full refund? Recommend yes, as now; Rust's window exists because of raiding, which we do not have (D2).
8. **What gets rounded**: the upright corners of walls only, or the roof's edge and the foot of the wall too?
9. **What is a material a property of**: a whole room, a wall panel, or a block?
10. **What "capable of building" means** (D68): blueprints found, things scanned, a research tree, skill? The developers named Terraria (2026-10-12): going to other zones, finding new materials, which unlock new things to craft and build, many of them only for their look. Realms (D87) would be our zones: each realm's materials open its own kinds of construction and decoration.
11. **Do bases have needs**, as Subnautica's: power, light, warmth?
12. **Who may change what** in co-op? Recommend the claim's owner and whom they allow (D94).
13. **Claims**: how many per player, what keeps one alive, how fast the world heals outside them?
14. **Upkeep**: does a base decay if left? Recommend only an abandoned claim, slowly, never one in use.

## What is built

The `play` experiment: see the game's [README](../crates/terra_game/README.md). Subnautica 2's controls and how ours map to them are in [research/subnautica-2-building.md](../research/subnautica-2-building.md); all five games are compared in [research/building-systems-prior-art.md](../research/building-systems-prior-art.md).
