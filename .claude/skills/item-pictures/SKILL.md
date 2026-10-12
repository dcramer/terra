---
name: item-pictures
description: Check or fix an item's picture, its icon in the action bar, backpack, equipment slots, storage and station screens, in Terra's house style. Use when adding or changing an item's look (a held tool or weapon, or a worn thing), when asked about item icons or pictures, or when a picture looks wrong.
---

# Item pictures

Nobody draws an item's picture, by hand or with an image model. It is drawn from the item's own look (D129) in one house style (D130), so every item, colour and procedural variant gets a picture that matches the rest and matches the thing itself. Consistency comes from two things only:

1. **One rig, in code**, in `crates/terra_sandbox/src/experiments/pictures.rs`, until the game draws pictures itself. Never tune it for one item.
2. **Looks that follow the contracts** in [docs/characters.md](../../../docs/characters.md#the-contracts). A picture that looks wrong almost always means a look that breaks one.

## The house style

- **Held things** lie corner to corner, square on, broad side to the front, working end up and to the right.
- **Worn things** face the front, seen from 15 degrees above, as worn, with no body in them.
- **One rig**: the play scene's warm sun from above and to the left, its blue shade, a cool light from behind on the edges, the play camera's grading. No outline, no background, no perspective. Framed by the look's bounds with a little room.

The style is the developers' (D130). To change it, ask them, change the constants at the top of `pictures.rs`, redraw the sheet in the docs (below), and record it in `docs/decisions.md`.

## Check a look's picture

After making or changing a look (`models/held/<item>.glb`, or `models/gear/<body>/<item>.glb`):

```bash
cargo run -p terra_sandbox -- --experiment pictures --model models/held/<item>.glb --sheet shots/pictures/<item>
```

It saves `<item>.png` at 128 pixels and `<item>-64.png` at 64, the size of a slot, on an empty slot's colour, then exits. It runs on the CPU under WSL in seconds. Read the 64-pixel one. Then draw every look and see the new one beside the others:

```bash
cargo run -p terra_sandbox -- --experiment pictures --sheet shots/pictures/all
```

A good picture:

- **Reads at 64 pixels**: you can say what it is without its name.
- **Is not mistaken for its neighbours**, such as a stone axe for an iron one.
- **Sits like the rest**: a held thing's working end up and to the right, showing its broad side; a worn thing from the front.
- **Has its colours by role**, so a tint recolours it as it does in play.
- Is not cut off, and is not a sliver in an empty slot.

## When it looks wrong, fix the look

| What you see | What is wrong with the look |
|---|---|
| A held thing's working end down or to the left | Its handle does not run along +Y from the grip at the origin toward the working end |
| A worn thing seen from behind or the side | It does not face +Z, as worn |
| A tint changes nothing | Its materials are not named by role (Leather, Metal, Wood...) |
| Much darker or duller than the rest | Its materials' colours or texture, beside the other looks' |
| Two items look alike | Their shapes or colours, which is design: say so, don't patch the picture |
| Long and thin, small in its slot | Expected in a square; the game frames each picture to its item's cells |

If a look truly needs to sit differently in its slot, don't add a number for one item in code (D127). Propose an entry in the look's glTF extras, as its carry tilt is, read by the rig for every look, and record it.

## Keep the docs' sheet current

When looks are added or the style changes, redraw the sheet [docs/items.md](../../../docs/items.md#item-pictures) shows:

```bash
cargo run -p terra_sandbox -- --experiment pictures --sheet docs/images/item-pictures
```

## Not built yet

The game's slots still show names. Drawing pictures in play, cached by look, tint and state and framed to each item's cells, is D129's next step; it uses this rig.
