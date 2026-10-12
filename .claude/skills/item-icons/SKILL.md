---
name: item-icons
description: Make, check or fix item icons (what the action bar, backpack, equipment slots, storage and station screens show for an item) in Terra's house style. Use when adding an item or changing its look (a held tool or weapon, a worn thing, or an item's own look such as a material), when the icons test fails, when asked about item icons, or when an icon looks wrong or is missing.
---

# Item icons

Icons follow the industry's usual pipeline (D129): image files baked during development, committed with the other art, loaded by the game. Nobody draws one. `cargo xtask icons` renders every item in the catalogue from its own look, in one house style (D130), so every icon matches the rest and matches the thing itself.

Consistency comes from three things:

1. **One rig, in code**: `crates/terra_sandbox/src/icons.rs`. Nothing in it names an item; never tune it for one.
2. **Looks that follow the contracts** in [docs/characters.md](../../../docs/characters.md#the-contracts). An icon that looks wrong almost always means a look that breaks one.
3. **The staleness test.** `assets/icons/manifest.txt` records the look and style each icon came from; `every_icon_is_baked_from_its_current_look` (run by `cargo xtask check`) fails when a look or the style has changed since. A stale icon cannot be committed.

## Bake

After adding an item or changing a look, from the repository root:

```bash
cargo xtask icons --sheet shots/icons/all
```

It writes `crates/terra_sandbox/assets/icons/<item>.png` for every item whose look exists, sized to its cells (96 pixels a cell), rewrites the manifest, removes icons of items no longer in the catalogue, lists the items with no look yet, and exits; about a minute on the CPU under WSL. Look at the sheet, then commit `assets/icons` with the look.

## Which look an item's icon comes from

| The item | Its look |
|---|---|
| A tool or weapon | `models/held/<item>.glb` |
| Gear | `models/gear/base-male/<item>.glb` (the player's body) |
| Anything else, such as a material | `models/items/<item>.glb`: the thing on its own, as it lies, life size, facing +Z |

`<item>` is the catalogue name in lower case with `_` for spaces (`character::item_id`): "Cooked meat" is `cooked_meat`. An item with no look has no icon and shows its name; the baker lists them. To give it an icon, add the look and bake; no code changes.

## The house style

- **Held things** lie along the icon's diagonal, square on, broad side to the front, working end up and to the right.
- **Everything else** faces the front, seen from 15 degrees above; worn things as worn, with no body in them.
- **One rig**: the play scene's warm sun from above and to the left, its blue shade, a cool light from behind on the edges, the play camera's grading. No outline, no background, no perspective.
- **Framed** by the look's bounds and the item's cells, with a little room.

The style is the developers' (D130). To change it: ask them, change the constants at the top of `icons.rs`, raise `STYLE` there so every icon is stale, bake, redraw the docs' sheet (below), and record it in `docs/decisions.md`.

## Check an icon

Read the sheet at half size (`shots/icons/all-half.png`, 48 pixels a cell, a backpack cell on a plain display). A good icon:

- **Reads at 48 pixels**: you can say what it is without its name.
- **Is not mistaken for its neighbours**, such as a stone axe for an iron one.
- **Sits like the rest**: a held thing's working end up and to the right, showing its broad side; anything else from the front.
- Is not cut off, and is not a sliver in an empty slot.

To preview a look that is not an item yet, without touching the assets:

```bash
cargo xtask icons --model models/items/<item>.glb --sheet shots/icons/<item>
```

To see icons in play (slow on the CPU; a few minutes):

```bash
cargo run -p terra_sandbox -- --experiment play --fresh --give <item>:1 --press i@150 --settle 260 --clean --shot shots/icons/play.png
```

## When it looks wrong, fix the look

| What you see | What is wrong |
|---|---|
| The item's name instead of an icon | It has no look, or it was not baked: bake, and the list says which looks are missing |
| The icons test fails | A look or the style changed since the icons were baked: bake and commit `assets/icons` |
| A held thing's working end down or to the left | Its handle does not run along +Y from the grip at the origin toward the working end |
| A worn thing or an item seen from behind or the side | It does not face +Z |
| Much darker or duller than the rest | Its materials' colours or texture, beside the other looks' |
| Two items look alike | Their shapes or colours, which is design: say so, don't patch the icon |

If a look truly needs to sit differently in its slot, don't add a number for one item in code (D127). Propose an entry in the look's glTF extras, as its carry tilt is, read by the rig for every look, and record it.

## Keep the docs' sheet current

When icons change, redraw the sheet [docs/items.md](../../../docs/items.md#item-icons) shows:

```bash
cargo xtask icons --sheet docs/images/item-icons
```

## Not built yet

Icons for colour variants (a mask baked with each icon, saying which parts take which colour, for the screen to tint) and for an item's state, such as broken; both wait until items carry them. Icons in storage and the station screens. Looks for materials.
