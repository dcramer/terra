# v0 characters: log and next steps

The people of v0: the player's body, what it wears, how it stands and moves, as [v0-goals.md](v0-goals.md) asks of them (the creator of a few sliders so two players tell each other apart, D111; equipping gear; the boss's mark shown on the arm, D590; locomotion and fight sets earning the developers' yes). How characters are made is [characters.md](characters.md); this page is where the work stands and what comes next, kept short, newest last. Pick-up point for whoever continues it.

## Decided

- **D599**: Terra's people are built on Quaternius's base bodies and modular outfits, in our own paint. The developers, 2026-11-02: "the quarternius stuff is clearly good, and its way better than our stuff. so you're going to stick with it. but we might have an issue w/ clothing/etc".
- **Their parts stay as made**: meshes, UVs, normals, weights, the split into pieces and the bones are never changed by our tools (no clipping pushes, stretch, scale, retopology or decimation). We add paint (new maps on their UVs) and new pieces they lack; a new piece that clips is fixed, never theirs. A check proves every imported piece matches its source vertex for vertex. The developers: "just dont fuck up their parts/etc".
- **D588**: moves no source does well are keyed from a generated pose sheet ([keyed-moves.md](keyed-moves.md)). The idle in the game today is one, on the old body.
- **One game copy a machine**, a switch allowing exactly one more for a two-player test (amending D319, being built).

## Licences

The developers bought the Source editions, 2026-11-02, from quaternius.itch.io. Each zip's own licence file governs:

| Pack | Licence | In the public repo |
|---|---|---|
| Universal Base Characters [Source] (2025-08) | CC0 1.0 | Yes, with its License_Source.txt verbatim beside it |
| Modular Character Outfits - Fantasy [Source] (v2.1) | CC0 1.0 | Yes, likewise |
| Bestiary - Dungeon Monsters Kit [Source] | Quaternius Asset License v1.0 (2026-08-28): use in our game, no redistribution of the assets, modified or not | **No.** Kept out of git; a public repo is redistribution |

The animation libraries we already use (UAL1, UAL2) are CC0.

## Log

- **2026-11-01 to 11-02, on the old Tripo body.** Arms and legs stretched to real proportions (D593, now Leaning: it goes with the Tripo body). The idle remade through the pose sheet, its arms posed from an upright-CT study of people at ease (Yoshida et al. 2022), and landed with the carry poses rebuilt (881de686). Clothing faults traced to our clipping fixer piling pushes up to 9 cm (crotch spike, straps standing off) and to the body's 55 cm shoulders; not fixed, parked as moot.
- **2026-11-02, the base question.** Research ([character-base](../research/2026-11-02-character-base.md)) found every ripple came from building gear and clips on a body that was then changed, with clips made for another body. Ready-made sets compared; only Quaternius fits licence, skeleton and style. A trial in our renderer: crisp, detailed shapes; smooth painted colour, a step more stylised than our art style; the same 65 joints as the mannequin, all 33 clips play. A first repaint of their Ranger in our materials (weave reads a little like a grid, wear faint).
- **2026-11-02, stopped here** at the developers' request: the review desk wiped (old items saved in `~/.cache/terra-review/desk-archive-2026-11-02/`), the two running jobs told to land what is checked and hand off.

## Next, in v0's order

The switch as bounded jobs (the research note's "The switch in jobs" has the detail). One game run at a time.

1. **Their files in, and the clothing answer**: bodies and outfits under `art/` with their licences; the intact-parts check; how their outfits split, how they map to our slots and v0's starting kit (small_pack, leather_coat, trousers, boots, sword and shield), what they lack, whether pieces of different outfits mix, what the separate woman costs. In progress: `~/.cache/terra-handoff/char-switch-j2.md` when it stops.
2. **The man through our pipeline**: their body as the base, our clips retargeted once onto it (`retarget.py`).
3. **The woman**, with a woman's proportion bands in `proportions.py` (it has only a man's).
4. **The paint as a tool**: our materials on their UVs, the Ranger's first pass refined.
5. **The starting kit as outfit data**: their pieces where they have them, new pieces made on their body where not.
6. **The fight on the new bodies**: `cargo xtask combat` and the carry poses against their baselines.
7. **Into the game**, one body a sex, the arm left clear for the mark; the creator's few sliders.
8. **Delete the Tripo base** and the scripts it alone needed (`stretch.py`, the clipping fixer's use on bought pieces).

## Open with the developers

- Their man's fingertips hang 2.2 cm higher than the 99th percentile allows. Recommended: one stretch of about 3 cm at import, before anything is built on him. Until they say, he stays as made, which the rule above also asks.
- Is the repainted Ranger the right direction for "our paint"?

## Later, past v0

Body build morphs, the other outfits, levels of detail for crowds, the Bestiary's monsters (licence above), the spear's own attack (it goes into the spine in Sword_Attack today), skin-aware hip keeping (D535).
