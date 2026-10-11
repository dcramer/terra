# Items and containers

Status: the developers set Subnautica as the baseline on 2026-10-10 (D84), "for now", and may revisit limits such as weight later. How it works in code is mine to decide (D83). What Subnautica does is in [research/subnautica-inventory.md](../research/subnautica-inventory.md).

## What the developers have said

From 2026-10-10:

- **An equipment grid, a character sheet** (D64): slots for what you wear and carry as gear, including the parts of a space suit such as a suit and an oxygen tank.
- **The inventory is the backpack** (D65, Leaning): probably very small at the start, made bigger by a better backpack in the backpack slot.
- **An action bar** (D66): you put items on it to use them; whatever is in the picked slot is in your hand.
- **You carry raw items, and they let you build what you are able to build** (D55, Leaning).
- **Subnautica is the baseline** (D84), and the game must stay modelled in ECS (D58).

## How it works

Subnautica's rules, with our backpack added:

| | What it is |
|---|---|
| Backpack | A grid. Each item fills a rectangle of cells, most of them one. Nothing stacks. Its size is set by the backpack worn. |
| Equipment | One slot per kind of gear: backpack, suit, helmet, oxygen tank, and so on. A slot takes only gear of its kind. |
| Action bar | A few slots that point at items in the backpack. The item stays in the backpack; if it leaves, the slot empties. |
| Storage | More grids, built in a base. Anyone may use them. |
| Limits | Room in the grid only. No weight. |
| Going down | What you carry drops where you fell, as something you can go back for, until it decays and disappears after a while (D84). Equipment stays on you. |

Picking something up puts it in the first place it fits, scanning from the top left. Items are not turned.

## Items

- **A kind of item is an entry in a list**, as buildables are (D56): its name, its size in cells, and which equipment slot it goes in, if any. Adding an item is adding an entry. A planet's theme names which kinds are found there (D59); planets get no item code of their own.
- **Items wear** (D99): tools, weapons and armour carry their condition, as one-of-a-kind items with state of their own; a broken one stops working but is kept, and is repaired at a station ([crafting.md](proposals/game-design-001/crafting.md#wear-and-repair)). Not built yet in `terra_items`.
- **Every item has an identity**, so the action bar can point at it and the server can say which one moved.
- **Where raw items come from**: harvesting returns a yield in cubic metres, objects or square metres (D45). A table in data turns a yield into items. What the raw items are is design and still open (D43, D55).

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

The sandbox's `play` experiment carries on it ([README](../crates/terra_sandbox/README.md)): the backpack, the worn pack and the bar are components on the player, building spends from the backpack, taking apart is refused when what it gives back would not fit, and cutting into a full backpack leaves the rest behind. It has a character sheet with a slot each for a pack, a helmet, a suit and an oxygen tank; clicking gear wears it and clicking a slot takes it off. Placeholders: those four slots; a bare backpack of 5 by 4; a starting small pack that makes it 6 by 5; suit parts that do nothing yet; six bar slots; tools two cells wide.

Not covered: crafting, research, what the raw items are, item sizes, and how the inventory looks on screen beyond the stand-in.

## Still open

- **Weight, volume or other limits**: none for now; the developers may revisit (D84).
- **Is what you carry lost for good if you never go back for it?** And does it drop all of it, or only what you gathered since you were last at a base, as in the first Subnautica?
- **How big is the first backpack**, and how many slots does the bar have? Subnautica 2 starts with about 20 cells; the first game's bar has five.
