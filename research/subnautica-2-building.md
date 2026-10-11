# Subnautica 2's base building, and a first try at it

2026-10-10. Question: how does Subnautica 2's construction system work, which the developers named as their inspiration, and what would the same idea look like here?

## What Subnautica 2 does

From press coverage and community guides. I could not read the developers' own video or a primary description of the controls, so treat the details as second-hand.

- **No fixed modules.** The first game snapped predefined rooms together. The second moves to what its design lead called "a new procedural system, where basically you don't have to be constrained to a specific shape", and "a sculptural base-building system that provides far more flexibility than anything we've done before".
- **A few primitives that can be resized.** Room, corridor, nook, half-round room, moonpool. Guides describe growing and shrinking rooms and corridors along their axes.
- **Windows are shaped, not chosen.** Instead of picking a porthole from a list, you shape a window on a wall. Coverage shows squares, portholes, rounded corners and larger shapes.
- **Interior walls, arches and doors** can be placed inside rooms.
- **The hull has a strength budget.** Each addition changes a structural rating; when it falls too low the base leaks. The exact numbers differ between sources and are unverified.
- **Co-op.** Up to four players can build together.
- **Still changing.** The game is in early access. The developers have said buildings are fairly blocky for now and that round rooms are planned.

## What we tried

A plain crate, `terra_build`, and a sandbox experiment that draws it:

```bash
cargo run -p terra_sandbox -- --experiment look --experiment build
```

![An example base: two rooms, a corridor, an upper storey, a run of joined windows and a door](../docs/images/build-test-2026-10-10.png)

The model:

- **A base is the set of tiles inside it.** Each tile remembers which room it was added as. There are no room parts.
- **The hull is generated.** Wherever inside meets outside there is a wall, floor or roof. Rooms that touch open into each other.
- **You resize a room by pushing or pulling one of its walls** by a tile.
- **Windows and doors are a property of a wall tile.** A window is described by its four edges, so it can be any size, and windows on neighbouring tiles with no gap between them read as one long window.
- **Everything is an operation**, so a base can be sent to other players and saved as the list of operations that made it.

It has 13 tests and works through real key presses in the sandbox. One flaw found that way and fixed: at first a wall was "every hull face in the same plane", so pushing one room's wall dragged its neighbour's along. Tiles now belong to rooms.

## The developers' verdict after playing it

2026-10-10, once the experiment had a player and a builder tool (the sandbox's `play` experiment): a good start, but clearly not Subnautica 2's building system yet. What they named as missing:

- **You select the part you want to build.** Ours has no parts to choose from. Its tool has three modes that act on whatever shape is already there.
- **You can cycle through variants of a part.** Ours cycles one thing, a wall tile, through four window sizes and a door.
- **"Similar and other things."** They did not list the rest. The account of the controls above is second-hand, so the list of what is missing should come from them or from playing the game, not from this note.

So the model here, tiles and a generated hull, may survive, but the way a player builds with it has to be redesigned around choosing a part first. That is a design question and has no doc yet.

## What it does not do

- **It is not built the way Subnautica 2 is built.** See the verdict above.
- **It is boxy.** Flat walls on a square grid, which is the look the developers said they do not want. Rounding, chamfered edges and curved rooms are the obvious next question.
- **Windows are rectangles**, cycled through four presets. No free shaping, portholes or rounded corners.
- **Resizing loses windows.** A window lives on a wall tile, so when the wall moves the window is gone.
- **No interiors**: no inner walls, and no furniture or machines a player can place.
- **No rules**: no cost, power, air, or hull strength.
- **One base, on a formula planet**, standing on a foundation clear of the ground, with no relationship to dug terrain.

Since this note was first written the base has gained trim, lamps, furniture placed by hand, and a player who can walk through it. None of that changes the list above.

## Sources

- [PC Gamer on the developer video](https://www.pcgamer.com/games/survival-crafting/subnautica-2-developer-video-reveals-a-brand-new-base-building-system-with-some-pretty-fabulous-windows/)
- [The Escapist on the modular system](https://www.escapistmagazine.com/news-subnautica-2-modular-base-building-system/)
- [PCGamesN base building guide](https://www.pcgamesn.com/subnautica-2/base-building)
- [Massively Overpowered on the system](https://massivelyop.com/2026/03/15/subnautica-2-shows-off-its-highly-modular-and-customizable-base-building-system/)
- [Subnautica 2 wiki: Window](https://wiki.subnautica.com/sn2/Window)
- [Mobalytics building recipes](https://mobalytics.gg/news/guides/subnautica-2-building-recipes)
