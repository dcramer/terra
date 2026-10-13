---
name: item-icons
description: Make, check or fix item icons (what the action bar, backpack, equipment slots, storage and station screens show for an item) in Terra's painted style. Use when adding an item or changing its look (a held tool or weapon, a worn thing, or a material), when the icons test fails, when asked about item icons, or when an icon looks wrong or is missing.
---

# Item icons

Icons are image files in `assets/icons`, committed with the other art and loaded by the game; an item with none shows its name. They are made in two steps (D129, D130):

1. **Rendered** from the item's look by `cargo xtask icons`, in one fixed house style. The render is the fallback icon and the underdrawing for painting: it fixes the shape, angle and framing so every icon is composed alike.
2. **Painted** by `tools/icons/paint.ts`, which has the image model repaint the render in the art style, with a style reference and approved icons as references so icons keep to one style. A person approves the result on a review sheet.

This follows how games with many generated items do it: an authored icon per kind, with variation (colour, quality, state) shown on top of it rather than redrawn.

## Adding an item, or changing its look

From the repository root:

1. **Make its look**, if it has one (below), following the contracts in [docs/characters.md](../../../docs/characters.md#the-contracts).
2. **Describe it** in `tools/icons/icons.txt`: one line, `<item>: <what it is>`, saying what the look shows and nothing more. An item with no look is painted from this line alone.
3. **Paint it**: `node tools/icons/paint.ts <item>`. It bakes the renders first, paints, writes the icon, records it in `assets/icons/manifest.txt` as painted, and saves `shots/icons/review.png` (renders above, paintings below) and `review-half.png` (a backpack cell's size). About a minute for a few items; up to 100 images a month are free.
4. **Review** both sheets. Paint again any that fail the checks below; each run is a new roll.
5. **Commit** `assets/icons` with the look and the description. `cargo xtask check` must pass.

`node tools/icons/paint.ts --unpainted` paints every described item that has no painted icon yet.

## Which look an item is rendered from

| The item | Its look |
|---|---|
| A tool or weapon | `models/held/<item>.glb` |
| Gear | `models/gear/<item>.glb` (one look for every body) |
| Anything else, such as a material | `models/items/<item>.glb`: the thing on its own, as it lies, life size, facing +Z |

`<item>` is the catalogue name in lower case with `_` for spaces (`character::item_id`): "Cooked meat" is `cooked_meat`. Items with no look yet are listed when baking.

## Checks

**Automatic.** The manifest records the look and render style each icon came from. `every_icon_is_made_from_its_current_look` (run by `cargo xtask check`) fails when a look has changed since its icon was rendered or painted, or the render style since it was rendered: bake (`cargo xtask icons`) or paint it again, and commit.

When an item's file changes but its look does not (gear rebuilt with its morphs by `tools/characters/build_bodies.sh`), compare its render with its painting (`cargo xtask icons --renders DIR`), and if they match keep the painting: `cargo xtask icons --keep <item>` records the file as it is now.

**By eye, on the review sheet.** A good icon:

- **Reads on the half-size sheet**: you can say what it is without its name.
- **Matches its render**: the same thing, with nothing added. The painter is told not to add detail, but it can (an early stone axe gained a cord binding); paint again if so.
- **Keeps to the others' style**: the same painted finish, light from the upper left, a bold silhouette.
- **Is not mistaken for its neighbours**, such as a stone axe for an iron one. If two items look alike, it is their looks or descriptions, which is design: say so.
- Is cleanly cut out: no magenta fringe, nothing missing at the edges.

## The render style

- **Held things** lie along the icon's diagonal, square on, broad side to the front, working end up and to the right.
- **Everything else** faces the front, seen from 15 degrees above; worn things as worn, with no body in them.
- **One rig**: the play scene's warm sun from above and to the left, its blue shade, a cool light from behind on the edges, the play camera's grading. No outline, no background, no perspective. Framed by the look's bounds and the item's cells, 96 pixels a cell.

It is in `crates/terra_game/src/icons.rs`; nothing in it names an item, so never tune it for one. A look that renders wrong breaks a contract:

| What you see | What is wrong |
|---|---|
| A held thing's working end down or to the left | Its handle does not run along +Y from the grip at the origin toward the working end |
| A worn thing or an item seen from behind or the side | It does not face +Z |
| Much darker or duller than the rest | Its materials' colours or texture, beside the other looks' |

If a look truly needs to sit differently in its slot, don't add a number for one item in code (D127): propose an entry in the look's glTF extras, as its carry tilt is, read by the rig for every look, and record it.

## The painted style

Icons follow the art style, [docs/art-style.md](../../../docs/art-style.md) (D14, direction D). Every icon so far is painted in the earlier direction C, and stays so until a D style reference is approved and everything is repainted; until then, keep new icons matching the existing ones. The style reference is `tools/icons/style/painted-workbench.jpg`, from the earlier mood board; the anchors (`@anchors` in `icons.txt`) are approved icons given as references too. The prompt is in `paint.ts`. Changing the style is the developers' call (D130): ask, change the reference, anchors or prompt, paint everything again (name every item), and record it in `docs/decisions.md`. Changing the render style also means raising `STYLE` in `icons.rs`.

## Keep the docs' sheet current

When icons change, redraw the sheet [docs/items.md](../../../docs/items.md#item-icons) shows: `cargo xtask icons --renders <dir>`, then `node tools/icons/paint.ts --review <every item> --renders <dir>`, and copy `shots/icons/review*.png` into `docs/images` as `item-icons*.png`.

## Not built yet

Variants shown on top of an icon (a palette shift for a dyed item, a frame or badge for quality or state), which wait until items carry them; icons in storage and the station screens.
