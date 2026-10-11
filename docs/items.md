# Items and containers

Status: a draft for the developers to react to. Nothing here is built, and nothing should be until the questions at the end are answered. Each section says whether it comes from the developers, is a proposal, or is open.

## What the developers have said

From 2026-10-10:

- **An equipment grid, a character sheet** (D64): slots for what you wear and carry as gear, including the parts of a space suit such as a suit and an oxygen tank.
- **The inventory is the backpack** (D65, Leaning): probably very small at the start, made bigger by a better backpack in the backpack slot.
- **An action bar** (D66): you put items on it to use them; whatever is in the picked slot is in your hand.
- **You carry raw items, and they let you build what you are able to build** (D55, Leaning).
- **The build menu comes from a building tool** (D67), and **research unlocks what can be made** (D68).

## The idea

Proposed. One rule for everything that holds items: **a container is a row of slots, and each slot holds one stack of one kind of item.** The backpack, the equipment grid, the action bar, a storage locker in a base and a dropped bag on the ground are all containers. They differ only in how many slots they have and what each slot accepts.

| Container | Slots | What a slot accepts |
|---|---|---|
| Equipment | One per kind of gear: backpack, suit, helmet, oxygen tank, and so on | Only gear of that kind, one at a time |
| Backpack | Set by the backpack worn in the equipment slot | Anything |
| Action bar | A few | Anything usable; it holds the item, not a link to one in the backpack |
| Storage in a base | Set by what was built | Anything |

Putting something in a slot that does not accept it is refused. Taking off a backpack that is not empty is refused, or spills, which is a question below.

## Items

Proposed.

- **A kind of item is an entry in a list**, as buildables are (D56): its name, how many fit in one stack, which equipment slot it goes in if any, and what it does when used. Adding an item is adding an entry. Planets do not get their own items in code; a theme names which kinds are found there (D59).
- **Most items are just a kind and a count.** Twenty of a metal is one stack.
- **Some items are one of a kind, with state of their own**: a tool that wears down, an oxygen tank that is half full. These stack to one and carry a few numbers. Whether tools wear at all is still open ([player.md](player.md#tools), question 9); the model allows it either way.
- **Where raw items come from**: harvesting returns a yield in cubic metres, objects or square metres (D45). A table in data turns a yield into items: a cubic metre of a rock is so many stones. What the raw items are is design and is open (D43, D55).

## In the game's model

Technical; mine to decide, and recorded as D83.

- **The rules live in an engine-free crate, `terra_items`** (D28): kinds, stacks, containers, and every way an item can move. Tests run with no window.
- **An item in a container is a value, not an entity.** A backpack's contents are state of the player, as health is. Each container is a component on whoever holds it (D58): the player, a locker, a dropped bag. An item lying loose in the world is an entity of its own.
- **Every change is an action the server checks** (D51): move, split, swap, equip, use, drop, pick up, take from a harvest, spend on a build. The client asks; the server applies it with the same function and says what happened. One function, used by both, means the client can show the result at once and is rarely wrong.
- **Nothing is made or lost by moving it.** Only harvesting creates items and only using, building or crafting destroys them. That is a property the tests check over random sequences of actions.
- **Each player is sent only their own containers**, and what others can see of their equipment. Inventories are small: a thousand players carrying a hundred stacks each is a few megabytes on the server (D52).
- **Agents can drive it** (D69, D73): every action is a plain call, so the sandbox's scripted runs and later an agent client can do anything a player can.

## What it replaces

The sandbox's `play` experiment has a stand-in ([inventory.rs](../crates/terra_sandbox/src/experiments/play/inventory.rs)): five raw items as counts, two tools, a six-slot bar, and no equipment or backpack. The first build would replace it with the crate and change no behaviour a player would notice, apart from adding an equipment grid with a backpack slot.

Not covered: crafting, research, what the raw items are, and how the inventory looks on screen beyond the stand-in.

## Open questions

Numbered to follow [player.md](player.md). My recommendation is given for each; none is decided.

12. **Stacks in slots, or a grid where items take up shape, as in Subnautica?** Recommended: stacks in slots. A shape grid adds a packing puzzle, a harder interface, and harder rules for agent players, for a feeling of weight that a small backpack already gives.
13. **Weight or volume limits besides the number of slots?** Recommended: none for now. Slots and stack sizes are limit enough, and the tone is gentle (D3).
14. **Is anything shared between players?** Recommended: each carries their own; sharing is storage built in a base, which anyone may use (a base belongs to everyone, [player.md](player.md#co-op)). Handing things over is putting them in a locker or dropping them.
15. **What happens to what you carry when you are down?** Depends on D18 (no fail state, Leaning). Recommended: the backpack's contents drop where you fell as a bag you can go back for; equipment and the action bar stay with you.
21. **How big are stacks?** Recommended: large for raw items (fifty or more), one for tools and gear.
22. **Does the action bar hold items or point at them?** Recommended: it holds them, so the bar is part of what you carry. The other way, a bar of shortcuts into the backpack, is common too and costs no carrying space.
