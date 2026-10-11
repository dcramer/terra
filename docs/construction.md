# Construction

Status: a draft for the developers to correct. It collects what they said on 2026-10-10 while playing the sandbox's `play` experiment, and proposes a shape for the system that would deliver it. The sandbox already does some of this; where it does not is listed under "The gap".

## What the developers want

In their words, shortened.

- **The basis is a voxel grid, as Enshrouded's** (D48, 2026-10-11), with Subnautica 2's shaped rooms as the easier way in. Earlier: Subnautica 2, blended with Rust, Ark and other advanced building games.
- **Enshrouded is a role model** for building and voxels, and for other mechanics (named 2026-10-10): "a ton of stuff we really might like to model off of". Building in voxels is fine; the terrain stays smooth voxels (2026-10-11). See [research/enshrouded.md](../research/enshrouded.md).
- **You pick the part you want to build, and can cycle its variants**, from "the little dial building menus we see in things like Rust and Subnautica".
- **Rooms are plain boxes.** No lights or fittings come with one.
- **A room placed across another extends it** to the combined shape.
- **Walls are a simple part**, indoors only. Not interior rooms, just walls. **Doors and windows are their own parts**, cut into an interior wall the same as an outside one.
- **Furniture and kit are buildable items.**
- **Everything has a collision box**, used for placing things as well as for walking (D53).
- **A grid things snap to**, which can be switched off. New rooms snap to rooms already there.
- **Only a foundation may go into the ground.** Anything else that would clip shows red and cannot be built. (Said with "I think".)
- **Details morph.** "Corners become rounded on the outside and inside, but when two segments are next to each other they're flat."
- **It must be really easy to extend**: new buildables, and properties like that morphing applying to anything related. "We might let you build a round room, or connectors, or different materials for rooms, and they should be able to change their look based on what they're connected to."
- **One square grid for everything**, though you might not see all of it all the time. Buildings join through connectors that can face any way, "more like Subnautica". The grid should be "a little flexible": in some games each cell is huge, "so you have no freedom to build".
- **Some buildables are a fixed size.** A round building probably cannot be freely resized, and is reached through a connector, "which maybe auto-builds if you connect it to another room". It is on the same grid as everything else: "it just wouldn't fill 100% of the cells".
- **A build menu like Subnautica's** (D67): you equip the building tool and it brings up a menu of the buildables you have unlocked. (Earlier the same day this was "a B menu".)
- **Building, taking apart and cutting take time.** You click and hold to complete the task, as other games do. Building animates the thing in, destroying animates it out, and cutting has an animation or a progress bar. How long depends on the task, and there will likely be different cutting tools, such as an axe or a pickaxe.
- **Building costs things you carry.** "I think it's likely what we'll want is for you to have the raw items, which allow you to build anything that you might have the capabilities to build." With an inventory, and an action bar you put items on to use them (D55).

## Enshrouded as the base?

From the developers, 2026-10-11: Enshrouded's building might be the better base concept than Subnautica 2's. It is a little more advanced, and maybe we can improve on it. Later the same day they agreed with the improvements below and settled the basis (D48): **a voxel grid, as Enshrouded's**. "Subnautica 2 is just easier, but as long as the basis is almost like this voxel grid we could have flexibility in time." So shaped rooms can come first, as the easy way in, written as blocks on the grid.

What Enshrouded's does, from [research/enshrouded.md](../research/enshrouded.md) (second-hand, not checked in the game): buildings are made of small voxel blocks in a material, placed one at a time or as ready-made pieces (walls, floors, stairs, roofs); pieces join seamlessly and different materials blend where they meet; cutting an opening styles its edges to suit; the ground can be dug, filled and flattened; and you build only within the radius of an altar you have placed.

Compared with the shaped rooms built so far (D48, the sandbox's `play` experiment):

| | Shaped rooms (Subnautica 2) | Voxel blocks (Enshrouded) |
|---|---|---|
| What you place | A room, a wall, an opening | A block, or a ready-made piece of many blocks |
| Freedom | Boxes on a grid, resized | Any shape, down to one block |
| Speed for big things | Fast: a room in one go | Slower, unless ready-made pieces are used |
| Details that suit their neighbours | Written per part, case by case | Natural: blocks join and blend by what is next to them |
| A new material or shape | New code per kind of part | A new kind of block, or a new ready-made piece |

The table's fourth and fifth rows are two things the developers asked for above: details that morph by what they touch, and a system that is really easy to extend. Voxel building gives both by its nature.

**Where we improve on it.** Agreed by the developers on 2026-10-11:

1. **Shape rooms, then carve them.** Keep Subnautica 2's speed: draw a room and its walls, floor and roof are filled in as blocks. Then work any of it by hand, block by block. The fastest start and the most freedom.
2. **Bases that matter.** As far as I know, Enshrouded's bases are never attacked. Ours are held, defended and lived in (D90): rooms can have a purpose, recruits work and sleep in them, and walls and gates are tested.
3. **Built on a round world.** Each base keeps its own square grid, level to the ground where it stands (D57), so blocks are square to the base, not to the planet.
4. **Building together.** Settlements many players build, if the world calls for them (D52).
5. **Strength, perhaps.** Whether a structure must hold itself up is open; Enshrouded, as far as I know, does not ask it.

**Technically** (mine; D9): structures stay separate from the terrain's smooth ground, but a base can be a voxel volume of its own: blocks on the base's grid, meshed and drawn by their own rules, blended with the ground where they meet. A held area the size of Enshrouded's (160 metres on a side) is 320 blocks across, about 33 million cells if it were all stored, so only the chunks that have something built in them are kept. That keeps the terrain's levels of detail and its edits untouched, and every block placed or carved is still an operation (D21). The sandbox's room tiles in `terra_build` would become one way of writing blocks, or be replaced. With the basis settled, this is the next technical step; it gets its own technical design before it is built.

## Claims

From the developers, 2026-10-11 (D94, Leaning): building will likely need a claim system, because we do not want the world randomly destroyed everywhere, nor to manage it.

Proposed, to react to:

- **You build only inside a claim.** A claim is an area you set up by placing something, as an altar does in Enshrouded (160 metres on a side once upgraded). It is the held ground of [game design 001](proposals/game-design-001.md#why-explore-and-why-settle-in-more-than-one-place): building, crafting stations and a safe rest happen there.
- **Outside claims, the world heals.** You can still dig and gather anywhere, since that is how resources are found (D43), but what you change outside a claim grows back after a while. Enshrouded reportedly rolls back changes outside its build zones after about half an hour (unchecked). Trees regrowing and holes refilling also answers whether a thousand players strip a world bare ([multiplayer.md](multiplayer.md)).
- **Claims belong to someone.** A player or a group; others build there only if allowed. There is no player against player (D2), so nobody can take apart what is in someone else's claim.
- **Claims are limited**: how many a player holds and how big they grow, perhaps by rank or by what is built at the centre.
- **Claims need keeping up.** An abandoned claim, its owner long gone, fades and its ground heals, so a persistent world does not fill with dead bases. Soulmask's lit bonfire, which stops structures decaying, is one model.
- **Claims are what the world attacks** (D90): what pools and forms around a claim is what its defenders hold back.

**Technically** (mine): claims keep the world's stored state in proportion to what players hold, not to everything they ever touched. The world is computed from the seed, and only changes are kept (D47); with edits outside claims expiring, those changes live almost entirely inside claims. That is what makes a persistent world for a thousand players affordable to keep, send and save (D52). A claim is a handful of numbers, and a change outside one carries the time it heals.

## The shape proposed

Four ideas. Each is a proposal until the developers say otherwise.

**1. A buildable is one entry in a list.** It says what it is called, where it sits on the dial, what variants it has, what it costs, where it may go, and how it is drawn. Adding a couch, a connector or a round room is adding an entry. Nothing else in the game has to know the new thing exists.

**2. Where a thing may go is a rule the entry names, not code written for it.** A handful of rules cover everything asked for so far:

| Rule | Used by |
|---|---|
| On open ground or across a room, floor clear of the terrain | Rooms |
| On a wall, outside or interior | Windows, doors |
| On the edge between two indoor tiles | Interior walls |
| On a floor, on the ground, on top of or beside another item | Furniture and kit |

When a rule refuses, the game shows the thing in red and says why. New rules will be needed (on a ceiling, between two buildings) but far fewer than new buildables.

**3. A thing's look comes from what it is next to.** Every structural piece is drawn by first asking what its neighbours are, then choosing shapes to suit. The corner rule the developers described is one case:

| Where two wall segments meet | Drawn as |
|---|---|
| In a straight line | Flat, with a seam |
| At an outside corner | Rounded outward |
| At an inside corner | Rounded inward |
| Where a wall ends against nothing | Capped |

The rule is written once for "walls" and applies to anything that says it is a wall: a room's hull, an interior wall, a connector's side, a wall of another material. A new buildable gets the morphing by saying what it is, not by reimplementing it.

**4. Material is a property, not a different buildable.** A room in another material is the same room with a different skin, and possibly a different cost. Skins are also entries in a list.

Underneath, the base stays what `terra_build` already has: a set of tiles, the walls and openings on their faces and edges, and a list of the operations that made it, so it can be saved and sent to other players (D21). Items would join that list.

## The test of "really easy to extend"

If the system is right, these are each small:

| To add | It should take |
|---|---|
| A piece of furniture | One entry and a model file. True today. |
| A window shape | One entry. True today. |
| A material for rooms | One skin entry |
| A new structural part on the grid, such as a connector | One entry, its rule, and its shapes |
| A new kind of morphing, such as a different roof edge | One rule, applied to everything of that kind |

## The gap

What the sandbox does today, measured against the above.

- **Items are already entries in a list.** Structural parts are not: rooms, walls and openings are written into `terra_build` and into the builder tool case by case.
- **Looks already follow neighbours in three places**: a post where a wall turns and a seam where it continues; a parapet where the roof begins and a line where a storey continues; a foundation ledge only on outside edges. But it is written as special cases, and nothing is rounded.
- **No materials.**
- **Timing is a stand-in.** Jobs are held down and animated, but how long each takes is derived from its cost by one rule, and there is one cutting tool.
- **Cost is a stand-in.** The sandbox has five raw items, a cost for every buildable, a full refund on taking apart, and a cutter tool to gather more. None of the numbers or names were designed, and there is no notion yet of what a player is capable of building.
- **No fixed-size buildables, ports or connectors**, and nothing is placed on a wall. See "The grid".
- **No build menu**, only the dial.

## Open questions

1. **Answered: the grid is half a metre**, changeable later, and it is the smallest thing that can be placed. See "The grid" below.
2. **What gets rounded?** The upright corners of walls only, or the roof's edge and the foot of the wall too? How round?
3. **What is a material a property of**: a whole room, a wall, or a tile?
4. **What does "capable of building" mean?** The build menu shows what you have "unlocked". By finding blueprints, scanning things, a research tree, tools carried?
5. **What does taking something apart give back?** Everything, as in Subnautica, or less?
6. **Do bases have needs**, as in Subnautica: power, air, hull strength? Rooms being plain boxes suggests lights at least need power from somewhere.
7. **Upper storeys.** Stairs or ladders as buildables, and whether rooms can be stacked from the dial.
8. **Who may change what** in co-op (also question 16 in [player.md](player.md)). Proposed: the owner of a claim, and whom they allow (D94).
12. **Claims.** What makes one, how big, how many per player, and what keeps one alive? Does what you dig outside a claim heal, and how fast?
9. Answered: **buildings may be made of voxels.** The developers said on 2026-10-11 they are fine with building in voxels, as long as the terrain stays smooth voxels (D9). Whether Enshrouded's system is the base is still open (D48).
10. Answered: **shaped rooms on top.** Draw a room and it is filled in as blocks, then carve it (D48).
11. **Must structures hold themselves up?** On the agreed list as "perhaps".

## The grid

What the developers said settles the first open question's main fork: one square grid, not free shapes. What follows is a proposal for what that means.

**One step for everything: half a metre.** Decided by the developers, as "something we can change later", and built: it is one constant in `terra_build`. A wall moves by it, a room is sized in it, a window or door is a run of its cells of any width anywhere along a wall, and an item snaps to it. It is "the smallest placeable kind of thing". The developers also want the same grid to appear on a wall when something is placed there, which is not built: nothing hangs on a wall yet.

**Three sizes, from the player's side** (2026-10-11, with the voxel grid as the basis, D48):

1. **The block**: the smallest thing the grid holds, half a metre today. Enshrouded's voxels are half-metre cubes too, from notes the developers shared ([research/enshrouded.md](../research/enshrouded.md)). So ours matches. Halving it means eight times the blocks for the same building, in memory, meshing and what is sent, so it is cheap to change now and dear once saved bases and ready-made pieces are measured in it. Settle it after building with it in the voxel prototype, before content exists.
2. **The brush**: how much one action places or carves. Several brush sizes over one fine grid keep big work fast and detail possible; Enshrouded's hammer steps through 1, 2 and 4 metre foundations and walls over its half-metre voxels.
3. **What is seen**: blending, bevels and rounded corners are what stop voxel building looking like a grid. That is meshing, separate from the block size.

**Two kinds of structural buildable.**

| Kind | Size | Joins others by |
|---|---|---|
| Free-form | Any number of steps, resized by pushing walls, extended by building across it | Sharing a wall, as rooms do now |
| Fixed | One footprint, as designed: a round room, an observatory, a dock. It sits on the same grid and claims a block of cells, but its shape need not fill them: a round room leaves the corners of its block empty. | Its ports only |

**Ports and connectors.** A fixed buildable has ports: places on its outside, each facing one way, including up and down. Two ports that face each other one step apart join directly. A port that faces a free-form room's wall cuts its own doorway there. A port that faces something further off is joined by a connector, a corridor one step wide, and the proposal is that the game builds the shortest straight one itself when you place the fixed buildable in line with something to join. Connectors are also a buildable in their own right.

**You see the grid only near what you are placing**, as now.

Fixed buildables, ports and connectors are not built.

## The build menu

The developers want it like Subnautica's: you equip the building tool, and it brings up a build menu of what you have unlocked (D67, D68). The sandbox is already shaped that way: the builder is a tool on the action bar, and holding the right mouse button with it in hand brings up a dial of parts. What is open is the menu itself. Subnautica's is a tabbed grid (standard elements, interior, exterior, utility, furniture), which holds far more than the dial's fifteen parts. A likely answer is both: the full tabbed menu, and the dial as a quick pick of what you used last.

## What is built

The sandbox's `play` experiment: see its [README](../crates/terra_sandbox/README.md). What Subnautica 2 does, as far as it could be confirmed, and how the sandbox's controls map to it, is in [research/subnautica-2-building.md](../research/subnautica-2-building.md).
