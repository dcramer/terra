# Construction

Status: a draft for the developers to correct. It collects what they said on 2026-10-10 while playing the sandbox's `play` experiment, and proposes a shape for the system that would deliver it. The sandbox already does some of this; where it does not is listed under "The gap".

## What the developers want

In their words, shortened.

- **The inspiration is Subnautica 2**, blended with Rust, Ark and other advanced building games (D48).
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
- **Building costs things you carry.** "I think it's likely what we'll want is for you to have the raw items, which allow you to build anything that you might have the capabilities to build." With an inventory, and an action bar you put items on to use them (D55).

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
- **No cost.** Nothing is carried.
- **Round rooms do not fit the model as it stands.** A base is square tiles. See the first open question.

## Open questions

1. **The grid.** Rooms are on three-metre square tiles. A round room, or a second building at an angle to the first, does not fit that. Is everything on one grid, with round rooms as a rounded look on square tiles? Or are buildings free shapes joined by connectors, as in Subnautica?
2. **What gets rounded?** The upright corners of walls only, or the roof's edge and the foot of the wall too? How round?
3. **What is a material a property of**: a whole room, a wall, or a tile?
4. **What does "capable of building" mean?** Found blueprints, scanned things, a research tree, tools carried?
5. **What does taking something apart give back?** Everything, as in Subnautica, or less?
6. **Do bases have needs**, as in Subnautica: power, air, hull strength? Rooms being plain boxes suggests lights at least need power from somewhere.
7. **Upper storeys.** Stairs or ladders as buildables, and whether rooms can be stacked from the dial.
8. **Who may change what** in co-op (also question 16 in [player.md](player.md)).

## What is built

The sandbox's `play` experiment: see its [README](../crates/terra_sandbox/README.md). What Subnautica 2 does, as far as it could be confirmed, and how the sandbox's controls map to it, is in [research/subnautica-2-building.md](../research/subnautica-2-building.md).
