# Icon painter

Paints item icons in the art style (D130). Each icon is painted by the image model from the item's render (`cargo xtask icons`), which fixes its shape, angle and framing, with a style reference and approved icons as references so every icon keeps to one style. The painter cuts it out of a flat magenta background, fits it to the item's cells, writes `assets/icons/<item>.png`, records it in the manifest as painted, and saves a sheet of renders above paintings to review. The baker never replaces a painted icon; the sandbox's icons test fails when an item's look changes after its icon was painted, until it is painted again.

The playbook is [.agents/skills/item-icons](../../.agents/skills/item-icons/SKILL.md).

```bash
node tools/icons/paint.ts stone_axe knife     # paint these
node tools/icons/paint.ts --unpainted         # paint every described item with no painted icon
node tools/icons/paint.ts --review stone_axe --renders DIR   # only the review sheet again
```

It bakes the renders first unless given `--renders DIR` (from `cargo xtask icons --renders DIR`). The review sheet is `shots/icons/review.png`, with a half-size copy at a backpack cell's size. Look at both, paint again any that are wrong, and commit `assets/icons`.

| File | What it is |
|---|---|
| `paint.ts` | The painter |
| `icons.txt` | What each item is, one line each, plus the style reference and the anchor icons. An item must be described here to be painted; items with no look yet, such as materials, are painted from this alone |
| `style/painted-workbench.jpg` | The style reference: the painted workbench from the mood board ([Terra Style Directions](https://claude.ai/artifact/AWgMLGJmS5GR7hvjJMbKq4), direction C) |

Needs Node 22.18 or later, ImageMagick (`convert`), and the Tripo CLI logged in ([tools/tripo](../tripo/README.md)). Images are free on the Studio subscription up to 100 a month, then 10 credits each. Tests: `cd tools/icons && node --test`.
