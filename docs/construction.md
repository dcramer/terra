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
- **Structures are planned, then constructed** (D139, Leaning, 2026-10-12): "for structures, you place a blueprint/plan of it, and then you have to actually construct it somehow… it makes it more realistic. So each wall you'd have to do that, and then doors are like carve outs so it can be slightly diff? Same with windows? We'll wanna think this through."
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
| Raise and lower, and nudge | Enshrouded, Ark Ascended | Missing; asked for |
| Place in a line or an area: drawn walls, floors, rooms | The Forest, Sons of the Forest, Subnautica 2 | Rooms are boxes, grown across one another; walls and fences are not drawn |
| Pick another of what you look at | 7 Days to Die | Built: middle mouse |
| Move what is placed | Subnautica 2 | Built for items, not rooms or walls |
| Take apart, giving back everything | Enshrouded, Subnautica, Valheim | Built; what does not fit drops at your feet |
| Undo for a short while | Enshrouded, Rust | Missing |
| Repair | Rust, Valheim, The Forest | Missing: nothing built takes damage yet |
| Change a piece's material in place | Rust, 7 Days to Die | Missing: no materials yet |
| Plans that cost nothing, then constructed part by part | The Forest, Ark Ascended | Missing; the direction (D139, Leaning) |
| Building from storage nearby, not only the pack | Subnautica 2, Valheim | Missing; decided (D141) |
| Carrying big materials by hand, better transport later | The Forest | Missing; the direction (D140, Leaning) |
| A base level with the ground where it is begun | Ark Ascended's heights | Built: the base's grid is set where its claim stone goes |
| Building on slopes | All, differently | Built: rooms cut into the ground (D131) and stand on foundations below |
| Floors above, and stairs | All | Storeys exist in the model; stairs and building upward are missing |
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

Proposed, to react to: one keymap for everything above, on mouse and keyboard, after what the prior art agrees on. Every key can be changed in the options (D116) but the wheel and the numbers. Rows in bold are new.

| Action | Ours | Built | Prior art |
|---|---|---|---|
| Take the hammer in hand | Its bar slot (1 by default) | Yes | A tool in every game |
| Build menu | Tap right mouse, or B | Yes | Subnautica's right mouse; Enshrouded's Tab |
| Variant of the thing in hand | Hold R for the wheel; tap R for the next | Yes | Rust's wheel |
| Lay a plan; build a part of one | Click left mouse to lay a plan; hold it, or E, at a part to construct it | Holding builds a whole room at once today | The Forest's E at a blueprint |
| Put the part away; put back what is carried | Tap right mouse | Yes | Subnautica |
| Rotate | Wheel; Z and C | Yes | Valheim's wheel; Enshrouded's R |
| **Raise and lower** | **Shift and the wheel: half a metre a notch on the grid, a tenth off it; Page Up and Page Down** | No | Enshrouded's Q and the wheel; Ark Ascended |
| **Material** | **Ctrl and the wheel** | No | Enshrouded's Ctrl and the wheel |
| Grid on and off | X | Yes | Enshrouded's X |
| Off the grid while held | Hold Alt | Yes | Valheim's Shift |
| Pick another of what you look at | Middle mouse, or V | Yes | 7 Days to Die |
| Take apart | Q for the mode, then hold left mouse | Yes | Enshrouded's right mouse; Valheim's middle mouse |
| Move | G for the mode, then click to pick up and put down | Items only | Subnautica 2's F |
| **Undo the last thing built or taken apart** | **Ctrl and Z, within a minute, giving back what it cost** | No | Enshrouded's Y; Rust's window |
| **Repair** | **With nothing picked, aim at something damaged and hold left mouse** | No | Rust's and Valheim's hammer |
| Camera nearer and further | Wheel when nothing in hand turns; = and - | Yes | Enshrouded's first-person building camera (2025) |

Three conflicts to know about: the wheel turns, zooms, raises (with Shift) and changes material (with Ctrl); Q is our take-apart but Enshrouded's nudge; right mouse is our menu but Enshrouded's remove. I would keep ours, which follow Subnautica (D67), and add only the bold rows.

## Planned, then constructed

The developers' direction (D139, Leaning): a structure is first laid out as a plan, and then actually constructed, part by part. What follows is my working-through of it, to react to; nothing here is settled.

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

**Building in bulk.** Putting in parts one by one is the realistic way and must not become a chore: holding the build key keeps putting in the next part while you have the materials; a skilled builder (D88) puts them in faster; recruits put them in for you (D90). The Forest's later option to skip each part's animation is the lesson: show each part going in, quickly.

**Doors and windows are carved out** (the developers' "carve outs, so it can be slightly different"):

- **On a plan**, an opening is planned into the wall: the parts it crosses are shortened or left out, so the wall needs less, and a frame is added to build.
- **In a finished wall**, an opening is cut: it takes work with a tool, gives back the material of what was cut away, and leaves a frame to build.
- **The door or the glass is its own item**, made at a station and fitted into the frame. A doorway without a door is just a gap; a window frame without glass is open to the weather.

**Small things are not planned.** Furniture, storage, lights and stations are placed whole, as now: they are made, then set down. Open whether stations should be constructed too, being large.

**Taking down.** A constructed part comes down to its plan, giving back its materials (or dropping them at your feet if they do not fit). A plan with nothing built is removed for free. Undo works on both.

**Who constructs.** Anyone allowed in the claim (D94), and recruits who work at the base (D90), as Sons of the Forest's companion finishes blueprints. A claim-wide list of what all its plans still need, as The Forest's counter, tells a group what to go and get.

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
- **What may not overlap**: an item may not go into another item or a wall (built); a room may not go over a placed thing, such as the claim stone (not yet checked).
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
- **Undo**: the last thing built or taken apart, within a minute (proposed).
- **Moving**: items now (built); rooms and walls as a whole later (open).
- **Repair**: once bases take damage (D90), the hammer mends what it built, for a little of its cost (proposed). Tools and gear already wear and are mended at stations (D99).
- **Material**: changing a part's material in place, paying the difference (proposed; material is a property, not a different part, below).

## Stability

None in the MVP, and wanted later (D142). Rust, Valheim and Sons of the Forest make structures hold themselves up; Enshrouded and Subnautica do not; Ark does but hides its rules, which players complain about. When it comes: shown only while building, as Valheim's colours; a part that holds others up cannot be taken out from under them, as Sons of the Forest's; and constructing in order (floor, walls, roof) already reads as physical without it.

## Enshrouded as the base?

From the developers, 2026-10-11: Enshrouded's building might be the better base concept than Subnautica 2's. Later the same day they settled the basis (D48): **a voxel grid, as Enshrouded's**. "Subnautica 2 is just easier, but as long as the basis is almost like this voxel grid we could have flexibility in time." So shaped rooms come first, as the easy way in, written as blocks on the grid.

What Enshrouded's does, from [research/enshrouded.md](../research/enshrouded.md): buildings of small voxel blocks in a material, placed one at a time or as ready-made pieces; pieces join seamlessly and materials blend where they meet; cutting an opening styles its edges; the ground can be dug, filled and flattened; you build only within an altar's radius.

| | Shaped rooms (Subnautica 2) | Voxel blocks (Enshrouded) |
|---|---|---|
| What you place | A room, a wall, an opening | A block, or a ready-made piece of many blocks |
| Freedom | Boxes on a grid, resized | Any shape, down to one block |
| Speed for big things | Fast: a room in one go | Slower, unless ready-made pieces are used |
| Details that suit their neighbours | Written per part | Natural: blocks join and blend |
| A new material or shape | New code per kind of part | A new kind of block, or a new piece |

**Where we improve on it**, agreed by the developers on 2026-10-11: shape rooms, then carve them; bases that matter, held, defended and lived in (D90); built on a round world, each base on its own square grid (D57); building together (D52); strength, perhaps.

**Technically** (mine; D9): structures stay separate from the terrain's smooth ground, but a base can be a voxel volume of its own: blocks on the base's grid, meshed by their own rules, blended with the ground where they meet. Only the chunks with something built in them are kept, and every block placed or carved is an operation (D21). The room tiles in `terra_build` become one way of writing blocks. This gets its own technical design before it is built.

## Claims

From the developers (D94, Leaning; D104): you build within areas you claim, so the world is not randomly destroyed everywhere, nor managed by anyone. Built: a claim stone, 40 metres across, grown to 80 and 160 for a price; nothing can be built outside one; the base is begun at it.

Proposed, to react to:

- **Outside claims, the world heals.** You can dig and gather anywhere (D43), but what you change outside a claim grows back after a while, as Enshrouded's does after about half an hour.
- **Claims belong to someone**, a player or a group; others build there only if allowed, as Rust's tool cupboard lists who may. There is no player against player (D2).
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

## The build menu

As the developers want it (D67, D68): with the hammer in hand, tap the build menu key, and a panel of tabs shows what you can build, each tile with a picture, its name and its cost, greyed if you cannot pay and absent if you have not learned it. Click one, or use the arrows and Enter. The variant wheel (hold R) chooses among the variants of the thing in hand. Taking apart and moving are modes on their own keys, not entries. The last few things built sit at the top. The world never pauses. Built, with pictures drawn from each thing's model.

Tabs: Claim; Structure; Openings; Crafting; Furniture; Storage; Light and fire. Proposed: Defence, once there are fences and gates; Blocks and brushes, with the voxel grid (D48).

## One system for everything built

D108: everything built in the world, by anyone, is made of the same parts as what a player builds. A village's houses are rooms, walls, roofs and doors from the build catalogue, put up by the same operations. A well is a buildable thing, a station rather than a room. The test of a building system is that a settlement can be described in it; if something in a village cannot be, the catalogue grows.

## Proposed order

For the developers to reorder. Each step is small enough to play and judge.

1. **Placing, finished**: raise and lower, undo, the room-over-a-placed-thing check, what blocks a ghost shown through walls, and the controls above. These hold whatever comes after.
2. **Plans, then constructing** (D139): rooms and walls laid as plans, built part by part from the pack and from storage in the claim (D141); doors and windows carved out and fitted.
3. **Large materials** (D140): logs that lie where trees fall, carried on the shoulder; a log pile to keep them, later unlocked.
4. **Downward and upward**: rooms underground (D143), a storey up, stairs and ladders.
5. **Doors and defences**: doors that open and lock by the claim; fences, walls and gates drawn as a line.
6. **Damage and repair**, with the first attacks on a base (D90).
7. **Materials**, and changing them in place; better transport, unlocked (D140).
8. **Voxel blocks and brushes** (D48), with plans and parts written as blocks: the engine track, with its own technical design.
9. **Stability**, after the MVP (D142).

## Open questions

For the developers. Each has my recommendation.

Answered on 2026-10-12: structures are planned, then constructed, a blueprint showing its parts, which can be small and depend on the kind of construction (D139); big materials are carried, with better transport later (D140); building draws on storage, with special storage for big materials later (D141); no stability in the MVP, but later (D142); rooms can be underground (D143).

1. **The kinds of construction**: which there are, and when each is learned (D68). Recommend wattle and daub, then logs, then planks, then stone for the MVP's first two.
2. **Must parts go up in order**, what rests on something after it? Recommend yes.
3. **Are stations constructed too**, being large, or placed whole like furniture? Recommend placed whole for now.
4. **Cutting an opening in a finished wall**: with which tool, and does it give back what was cut? Recommend the axe for wood, the pickaxe for stone, giving it back.
5. **Which materials are big**, and how many does a player carry? Recommend logs and cut stone; one comfortably, two at a walk.
6. **Is a log cut into beams or planks first**, by the axe or at a station? Recommend at a sawbench, so a wall panel takes planks and a post takes a log.
7. **Taking apart**: always a full refund? Recommend yes, as now; Rust's window exists because of raiding, which we do not have (D2).
8. **What gets rounded**: the upright corners of walls only, or the roof's edge and the foot of the wall too?
9. **What is a material a property of**: a whole room, a wall panel, or a block?
10. **What "capable of building" means** (D68): blueprints found, things scanned, a research tree, skill?
11. **Do bases have needs**, as Subnautica's: power, light, warmth?
12. **Who may change what** in co-op? Recommend the claim's owner and whom they allow (D94).
13. **Claims**: how many per player, what keeps one alive, how fast the world heals outside them?
14. **Upkeep**: does a base decay if left? Recommend only an abandoned claim, slowly, never one in use.

## What is built

The `play` experiment: see the game's [README](../crates/terra_game/README.md). Subnautica 2's controls and how ours map to them are in [research/subnautica-2-building.md](../research/subnautica-2-building.md); all five games are compared in [research/building-systems-prior-art.md](../research/building-systems-prior-art.md).
