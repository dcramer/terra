---
name: item-icons
description: Check or fix an item's icon (what the action bar, backpack, equipment slots, storage and station screens show for it) in Terra's house style, or give an item one. Use when adding or changing an item's look (a held tool or weapon, a worn thing, or an item's own look such as a material), when asked about item icons, or when an icon looks wrong or is missing.
---

# Item icons

Nobody draws an item's icon, by hand or with an image model. The game renders it from the item's own look (D129) in one house style (D130), the first time a screen shows it, and caches it for the session. Every item, colour and procedural variant gets an icon that matches the rest and matches the thing itself, and changing a look changes its icon on the next run: there is nothing to regenerate or commit.

Consistency comes from two things only:

1. **One rig, in code**: `crates/terra_sandbox/src/icons.rs`. Nothing in it names an item; never tune it for one.
2. **Looks that follow the contracts** in [docs/characters.md](../../../docs/characters.md#the-contracts). An icon that looks wrong almost always means a look that breaks one.

## Which look an item's icon comes from

| The item | Its look |
|---|---|
| A tool or weapon | `models/held/<item>.glb` |
| Gear | `models/gear/<body>/<item>.glb`, the wearer's body |
| Anything else, such as a material | `models/items/<item>.glb`: the thing on its own, as it lies, life size |

`<item>` is the catalogue name in lower case with `_` for spaces (`character::item_id`): "Cooked meat" is `cooked_meat`. An item whose look does not exist yet shows its name, and the log says which file was missing (`icons: models/items/stone.glb did not load`). To give it an icon, add the look; no code changes.

## The house style

- **Held things** lie along the icon's diagonal, square on, broad side to the front, working end up and to the right.
- **Everything else** faces the front (+Z), seen from 15 degrees above, worn things as worn with no body in them.
- **One rig**: the play scene's warm sun from above and to the left, its blue shade, a cool light from behind on the edges, the play camera's grading. No outline, no background, no perspective.
- **Framed** by the look's bounds and the item's cells, 96 pixels a cell, with a little room.

The style is the developers' (D130). To change it, ask them, change the constants at the top of `icons.rs`, redraw the docs' sheet (below), and record it in `docs/decisions.md`.

## Check a look's icon

```bash
cargo run -p terra_sandbox -- --experiment icons --model models/held/<item>.glb --sheet shots/icons/<item>
```

It renders the icon one cell square with the game's renderer, saves `<item>.png` at 96 pixels and `<item>-half.png` at 48 (a backpack cell on a plain display) on an empty slot's colour, and exits, in seconds on the CPU under WSL. Read the half-size one. Then render every look and see the new one beside the rest:

```bash
cargo run -p terra_sandbox -- --experiment icons --sheet shots/icons/all
```

To see it in play, give it and open the inventory (slow on the CPU; a few minutes):

```bash
cargo run -p terra_sandbox -- --experiment play --fresh --give <item>:1 --press i@150 --settle 260 --clean --shot shots/icons/play.png
```

A good icon:

- **Reads at 48 pixels**: you can say what it is without its name.
- **Is not mistaken for its neighbours**, such as a stone axe for an iron one.
- **Sits like the rest**: a held thing's working end up and to the right, showing its broad side; anything else from the front.
- **Has its colours by role**, so a tint recolours it as it does in play.
- Is not cut off, and is not a sliver in an empty slot.

## When it looks wrong, fix the look

| What you see | What is wrong with the look |
|---|---|
| The item's name instead of an icon | Its look is missing or failed to load: the log says which file |
| A held thing's working end down or to the left | Its handle does not run along +Y from the grip at the origin toward the working end |
| A worn thing or an item seen from behind or the side | It does not face +Z |
| A tint changes nothing | Its materials are not named by role (Leather, Metal, Wood...) |
| Much darker or duller than the rest | Its materials' colours or texture, beside the other looks' |
| Two items look alike | Their shapes or colours, which is design: say so, don't patch the icon |

If a look truly needs to sit differently in its slot, don't add a number for one item in code (D127). Propose an entry in the look's glTF extras, as its carry tilt is, read by the rig for every look, and record it.

## Keep the docs' sheet current

When looks are added or the style changes, redraw the sheet [docs/items.md](../../../docs/items.md#item-icons) shows:

```bash
cargo run -p terra_sandbox -- --experiment icons --sheet docs/images/item-icons
```

## Not built yet

Icons for an item's state (broken, how full) beyond the count written on a container; icons in storage and the station screens; keeping icons on disk between runs, if rendering them at start ever costs too much.
