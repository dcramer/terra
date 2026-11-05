# Character: gear

## 5. Gear

The game has many items and many bodies (D127), so gear is data: every worn item has one look, `assets/models/gear/<item>.glb`, made once on the base body and carrying the base's morphs, so it fits every body (D128). The item is named as in the catalogue in lower case with `_` for spaces (`leather_coat`). The game finds it by that name and puts it on as the file says: a skinned look is bound to the body's skeleton and follows its morphs; a rigid one names its socket in its glTF extras. No game code names an item, so adding gear is adding files.

A piece is its data, `art/gear/<item>/fit.json` (its kind, how it is made and from what; a garment's `fit` is `shell` or `hung`, with its `role`, `coverage`, dressed `model`, and `over`, `under`, `plain`, `shape`; `gear_list.ts` reads them all), so adding one edits no script. `tools/characters/build_item.sh <item>` makes it alone on a full build's bodies: on the base, carried onto every morph, pushed clear, checked and written as the game's file (the quiver's binary chunk identical to the full build's). `build_bodies.sh` makes every piece from the same data (section 7).

**Clothing and armour that bend with the body** (chest, legs, gloves, feet) are made on the base body, so nothing is set per character:

1. Draw that base's own sheet wearing it (free): `--ref` the base's approved sheet, and say "the same person, in exactly the same T-pose ... now also wearing <the piece>". Check it kept the person and the pose.
2. Make the dressed model (a `model`'s credits): crop the front view with `tools/characters/front_view.sh SHEET FRONT`, then `tripo.ts model --faces 30000`. Keep the sheet, front view and model as source art in `art/gear/dressed-<body>-<item>/` (`sheet.png`, `front.png`, `model.glb`) and commit them.
3. Build the piece from the base body:

```bash
tools/blender/run.sh tools/characters/shell_garment.py tools/characters/out/base-male-game.glb art/gear/dressed-male-outfit/model.glb leather_coat Leather chest_long_sleeves
```

The last word is its coverage, one of `COVERAGE` in the script (`chest_sleeveless`, `chest_long_sleeves`, `legs_long`, `gloves`, `feet`...): which parts of the body it covers and where its hems fall. The piece is those faces of the base body, pushed out (and as far as the dressed model stands off, for bulk), keeping the body's own weights, so it moves as the skin under it does; its colour is baked from the dressed model. Do not try to cut the piece out of the dressed model by colour or distance: Tripo paints its dressed body a shade off ours and makes it a little thinner, and every such cut came out full of holes.

**Worn things from their own models** (a pack, a quiver) are fitted to the base: a pack's bag skinned whole to the upper spine, a quiver rigid on its belt socket, each as its kind in [docs/attachment.md](../../../../docs/attachment.md) says (a cap is made on the head as clothing is):

```bash
tools/blender/run.sh tools/characters/fit_gear.py tools/characters/out/base-male-game.glb small_pack large_pack quiver
```

Each piece's turn, size (a share of the body's height) and fit are data beside its model, `art/gear/<item>/fit.json` (the schema is in the script's head), so a new piece never edits the script. A pack keeps only its bag from the model: its straps are made from the body's own skin (`bands.py`), along a route of rays cast from the body's joints, so they bend as the shoulders do and fit any body. Keep to the heights off the skin in `docs/characters.md`: a chest piece within 1.8 cm above the waist, straps solid from the skin to 2.2 cm, so a pack and a jerkin from different makers still stack. Tripo turns each model its own way and adds parts that stand out, such as a pack's stiff strap loops, so render a new model from four sides first and set the turn that puts its front to the body's front. Read each `FIT` line: it says how much of the piece rests on the body, and `FLOATING` when too little does. Given a figure dressed in Quaternius's pieces (`out/quaternius-male-ranger-game.glb`), `fit_gear.py` and `gear_check.py` fit and check over its outside (`look.worn_over`: its meshes joined in the run, the faces under a layer left out, every face kept for rays from outside) and lay the straps on as ribbons (`bands.ribbon`, facing turned away from the bones: a belt's inner side faces in), in plain leather until the paint (D599).

**Check every piece on every body**, by numbers first:

```bash
tools/blender/run.sh tools/characters/gear_check.py tools/characters/out/base-male-game.glb leather_coat trousers small_pack quiver
```

It plays our clips and reports, per clip, the share of skin near each piece that shows through it (the piece just beneath the skin, and nothing of it covering the skin from outside), and `where` on the body. Then look, live, with the body hidden if need be ([tools/sandbox](../../../../tools/sandbox/README.md)): a picture through the body hid a coat full of holes. Each piece also lists the patches of skin it covers, which the game does not draw under it (`patches.py`, `body_mask.wesl`).

Then add a row for it to `THIRD_PARTY.md`.

What is held is the same idea, without a body: one look per item, `models/held/<item>.glb`, with the grip at the origin, the handle along +Y toward the working end, a one-sided head toward -Z, as its kind in [docs/attachment.md](../../../../docs/attachment.md) says, carried in the carry pose its data names (`carry_hold`: a one-handed tool's `Carry_Tool`; attachment.md's "Carried"; a pose marked `upright` in `fight.json` is held in the body's frame, so refit its looks when that changes); `node tools/characters/attach_contract.ts` checks every look against it. `tools/characters/fit_held.py` makes them; each item's source, length and grip are its data, `art/held/<item>.json`, so a new item is a file. Check a held look in the workbench with `--hold <item>` through a carried clip and a swing. The fingers close on it by touch, not by a keyed hand pose (D562, `terra_walker::grip`, `tools/characters/grip.ts`): no item needs a grip pose, but its handle must be what crosses its grip's plane (y = 0) near its line, at a real thickness, and the move check's "fist" row holds the fingers within 1 cm of it and 80 degrees round it on every template. Every look in the fist names one (the spear `Carry_Spear`, the bow `Carry_Bow`; `held.rs` tests it), nothing tips it after the clip: judge a carried look by `moves.ts --carry`, its `held_cm` (nothing in the body), its `held_up_deg` and its wrap, not by its fingers' distance alone.

