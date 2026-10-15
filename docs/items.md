# Items and containers

Status: the developers set Subnautica as the baseline on 2026-10-10 (D84), "for now", and may revisit limits such as weight later. How it works in code is mine to decide (D83). What Subnautica does is in [research/subnautica-inventory.md](../research/subnautica-inventory.md).

## What the developers have said

From 2026-10-10:

- **An equipment grid, a character sheet** (D64): slots for what you wear and carry as gear: head, chest (covering the arms), gloves, legs, feet, back and belt, with more later. Not every slot shows on the body. One piece per slot. Jewellery is open. There is no space suit or oxygen tank (D89): breath is only under water (D74).
- **What you carry is in your pack** (D106): a container worn in the pack slot, and a bigger pack holds more. See [Packs](#packs).
- **An action bar** (D66): you put items on it to use them; whatever is in the picked slot is in your hand.
- **Building takes materials brought to a plan**, from what you carry and from storage nearby (D139, D141).
- **Subnautica is the baseline** (D84), and the game must stay modelled in ECS (D58).

## Packs

Decided (D106), by the developers on 2026-10-12: "a pack IS a container that holds things, a bigger pack holds more things". Without a pack you carry nothing but what is in your hands.

- **A pack is an item worn in the pack slot**, and its grid is part of it. Take it off and the things stay in it; put on another and you carry what is in that one.
- **Bigger packs hold more** because they are bigger, not because they unlock slots.
- **Death drops your pack** with everything in it, where you fell (D84, D171).
- **Without a pack** you carry what is in your hands, perhaps two hand slots (Leaning). Proposed: the belt holds a few tools for the action bar (D66), and a quiver (D118), which is already a container the same way.
- **Technically** (mine): a container's contents belong to the item that is the container, kept by its identity, so moving the pack moves what is in it. `terra_items` gains nested containers; nothing about a grid changes.

Not built: the game still grows one backpack.

## How it works

Subnautica's rules, with our backpack added:

| | What it is |
|---|---|
| Backpack | A grid. Each item fills a rectangle of cells, most of them one. Nothing stacks. Its size is set by the backpack worn. |
| Equipment | One slot per kind of gear (D64): head, chest, gloves, legs, feet, back and belt. A slot takes only gear of its kind. |
| Action bar | A few slots that point at items in the backpack. The item stays in the backpack; if it leaves, the slot empties. |
| Storage | More grids, built in a base: a chest, 8 by 6, opened beside the backpack. Building in the claim draws on it after the pack (D141, D295). Anyone may use them. |
| Limits | Room in the grid only. No weight. |
| Going down | What you carry drops where you fell, as something you can go back for, until it decays and disappears after a while (D84). Equipment stays on you. |

Picking something up puts it in the first place it fits, scanning from the top left. Items are not turned.

## Items

- **A kind of item is an entry in a list**, as buildables are (D127): its name, its size in cells, and which equipment slot it goes in, if any. Adding an item is adding an entry, in [`assets/data/items.ron`](../assets/data/items.ron) (D136), which also says what it does in hand or worn; no code names an item. A realm's theme names which kinds are found there (D59); realms get no item code of their own.
- **Items wear** (D99): tools, weapons and armour carry their condition, as one-of-a-kind items with state of their own; a broken one stops working but is kept, and is repaired at a station ([crafting.md](crafting.md#wear-and-repair)). In `terra_items`, an item carries how worn it is and a kind how much it lasts; wearing and mending are changes like any other, so the server can check them.
- **Every item has an identity**, so the action bar can point at it and the server can say which one moved.
- **Where raw items come from**: harvesting returns a yield in cubic metres, objects or square metres (D45). A table in data turns a yield into items. What the raw items are is design and still open (D43).

## Item icons

Every slot that holds an item shows its icon. Icons are image files in `assets/icons`, loaded by the game; an item with none shows its name.

- **How they are made** (D129, Decided): `cargo xtask icons` renders every item from its own look in one house style, sized to its cells; `tools/icons/paint.ts` repaints the render in the art style; a manifest records what each came from, and a test fails when one is stale. Variants (a dyed item, a quality frame) are to be shown on top of an icon, not redrawn.
- **What they look like** (D130, Leaning): in the art style ([art-style.md](art-style.md)), a bold silhouette, warm light from the upper left, no background. Every icon is still painted in the earlier direction C, to be repainted in direction D.

How to make, check and fix them is in [the item-icons playbook](../.agents/skills/item-icons/SKILL.md); how the approach was chosen is in [research/item-icons.md](../research/item-icons.md).

![Every item's render and painted icon, at 96 pixels a cell](images/item-icons.png)

![In play: icons in the equipment slots, the backpack and the bar](images/item-icons-in-play.png)

## In the game's model

Technical, recorded as D83.

- **The rules live in an engine-free crate, `terra_items`** (D28): kinds, grids, equipment, the bar, and every way an item can move. Tests run with no window.
- **In ECS, a container is a component** on the entity that has it: the player, a locker, a bag dropped where someone went down. The items inside are data in that component, not entities, in the same way health is a number and not an entity. An item lying loose in the world is an entity of its own.
- **Every change is an action the server checks** (D51): move, equip, use, drop, pick up, take from a harvest, spend on a build. The client asks; the server applies it with the same function and says what happened, so the client can show the result at once and is rarely wrong.
- **Nothing is made or lost by moving it.** Only harvesting creates items and only using, building or crafting destroys them. Tests check this over random sequences of moves.
- **Each player is sent only their own containers**, and what others can see of their equipment. Inventories are small: a thousand players with fifty items each is well under ten megabytes on the server (D52).
- **Agents can drive it** (D69, D73): every action is a plain call.

## What is built

The rules, in [`terra_items`](../crates/terra_items/src/lib.rs): the catalogue of kinds, the grid, equipment that resizes the backpack, and the action bar, with a test that random moves never make or lose an item.

The sandbox's `play` experiment carries on it ([README](../crates/terra_sandbox/README.md)): the backpack, the worn pack and the bar are components on the player, building spends from the backpack, and whatever does not fit in the backpack, from cutting, digging, butchering or taking apart, drops in a pile at the player's feet to be picked up (asked for by the developers on 2026-10-12). It has a character sheet with five slots, by the names the items file wears things in: Pack, Head, Body, Legs and Belt (a quiver, D118), of D64's standard slots; a player starts in a leather coat and trousers, with the small pack; clicking gear wears it and clicking a slot takes it off. Placeholders: those five slots, short of D64's seven; a bare backpack of 5 by 4; a starting small pack that makes it 6 by 5, and a large one 7 by 6; a leather coat and trousers that take part of each blow; eight bar slots; tools two cells wide.

Not covered: crafting, research, what the raw items are, item sizes, and how the inventory looks on screen beyond the stand-in.

## Still open

- **Weight, volume or other limits**: none for now; the developers may revisit (D84).
- Settled for ammunition (D118): **a quiver is a container, holding a stack of arrows, worn on the belt in a slot of its own.** Arrows stack inside a quiver, which is one item. How many it holds, and whether other ammunition works the same way, are open. It fits a pack that holds things (D106).
- **Is what you carry lost for good if you never go back for it?** And does it drop all of it, or only what you gathered since you were last at a base, as in the first Subnautica?
- **How big is the first backpack**, and how many slots does the bar have? Subnautica 2 starts with about 20 cells; the first game's bar has five.
