# Do vaults and arched niches lift the crypt demo's score?

2026-10-25. The question: does the crypt dungeon (`--experiment dungeon`) score higher with three harsh judges when its corridors and rooms are vaulted and its corridor walls carry arched burial niches in a rhythm, built from building's existing cubes, kits and pieces? The judges' top fault in rounds 20 and 23 was "flat-ceilinged boxes, no vaults, arches, piers or niches". How each theme is built is the developers' design call (open on the review desk, item r-dungeons), so this was a spike on a branch, `spike/dungeon-vaults`, and none of its code is on main.

## Answer

No. With the vaults the crypt scored 3 / 3.5 / 3.5 = **3.33**. Round 23's crypt, filmed the same way without them, scored 3 / 4 / 4 = **3.67**. The difference is within one judge's point, so the vaults made no measurable difference. All three judges saw the new shapes, and all three said they don't read as vaults. The niches look like "flat black triangles" or decals, and the pointed vault looks like "two wedges stuck on the ceiling" or "dark planks crossing the top of the frame". Half-metre cubes cut only to 45° can make polygonal vaults, but at a 2 m corridor's scale those read as wedges, not as curved masonry. A real vault needs a piece of its own, not more cubes.

## What was built (spike/dungeon-vaults, `crates/terra_dungeon/src/stamp/vault.rs`)

The vaults are switched on by a theme flag, `vaults: true`, which only `crypt.ron` sets. They are laid after the dressing and only change the stamp's cubes and props.

- **Corridor vault**: under the ceiling slab, two upside-down slopes (`Shape::InvertedSlope`) rise from each wall, starting at 2.5 m and meeting at the crown at 3.5 m. This makes a pointed tunnel vault over the 2 m corridor. The first try used one slope a side and a 1 m flat crown (nearly half a regular octagon). In stills it read as a flat ceiling with a ledge, so it was replaced before filming.
- **Room cove**: a chamfer two cubes deep runs along every wall that isn't open. It is one cube deep in front of a stair's way, so the way's 3 m opening stays clear.
- **Transverse arches**: in a straight run of corridor, a pointed arch of trim (the `Stone` kit) stands where each cell meets the next, one cube inside the vault. Each side has two haunches, rising from 2 m at the wall to 3 m at the crown.
- **Arched niches**: each wall of each straight corridor bay gets one niche, centred between the arches. The opening is 1 m wide and 1 m high, with its sill at 0.5 m. Its head is two upside-down slopes meeting in a point (a mitred arch, up to 2 m), and a cube of the wall's stone forms its back. A niche is cut only where the cube behind the wall is air or another cell's wall. One niche in three holds the existing `VasePair` prop at 0.6 scale. The rooms' existing loculi get the same mitred head.
- **A change in `Frame::fill`**: it now turns every shape a quarter clockwise with the frame (`Shape::turned`). Before, it turned only stairs and slopes.
- **Tests**: `cargo test -p terra_dungeon` passes 24 of 24 on the branch. Those include air staying inside its space, the walker reaching every object, and every model existing. No new test was written: this is a spike.

## What building could and couldn't do

- **Could**: cubes cut to 45° slopes in the `Greystone` and `Stone` block kits. They are drawn with the stone surface and seat correctly against the slabs and walls. The spike needed no change to building.
- **Couldn't**:
  - **Only 45° faces.** A vault's curve became one or two straight facets a side. A 1 m rise over a 2 m span allows two facets at most.
  - **No upside-down corner slope.** Where two chamfers meet in a corner, the cube is left whole, so a room's cove steps at every corner and a corridor's vault ends in a notch.
  - **No room to rise.** Levels are 4 m apart (a slab and 3.5 m clear), so a vault can't rise above the old ceiling. It is cut down into the space instead, which lowers the springing to 2.5 m.
  - **No dressed pieces for these shapes.** There are no voussoirs, archivolts, imposts or springers. An arch is a 0.5 m trim cube, and a niche's head is the wall's own texture cut on the diagonal.
  - **No depth or light in the niches.** A niche is half a metre deep with no light, so the auto-exposed frame crushes it to black.
  - **No props for remains.** There are no bones, skulls or shrouded bodies to fill the niches, only vases.

## Film and reviews

- **Film**: the crypt tour as in round 23. The shot came from the spike's own still log (`r20/tourshot.ts`): 47.5 s, 95 m, a peak turn of 46°/s, and no bitten-for lines. Worker job 20261025-220246-2122 filmed it at the game's pace (`--cine …@110`, frames 300-2959). Still jobs: 20261025-215750-d97a (first try) and -220128-3be3 (pointed).
- **Reviews**: three fresh harsh judges used round 17's brief (the same CC photo anchors) and strips chosen as in round 23: 8 frames spread evenly, plus an 8-frame run 45% in (19.9 s). Differences from round 23: the judges saw the crypt alone, not beside the mine and keep. Judges vary by about a point (D472).
- **Scores**: crypt 3 / 3.5 / 3.5 = 3.33, against round 23's 3.67.
- **Top faults**, the three judges agreeing:
  1. **The vaults and niches don't read.** The niches are black triangles with no depth, sill, contents or edge light. The vault is wedges at the top of the frame, with no springing line, piers or a curved surface catching light.
  2. **One cobble texture at one scale on every surface**, with sharp edges and no mortar, quoins or damp band.
  3. **Flat fill light.** Torches throw no pool, and the goal room is one orange wash.
  4. **Sparse stand-in dressing**: a lone boulder, a low-poly spider, bones as sticks, empty niches.
  5. **The camera brushes a door leaf**, then holds about 1.2 s on a blank wall (run 20.2-21.7 s). This is round 23's camera fault, unchanged.

## What a full build would need

- **Vault and arch pieces in building**: a dressed piece laid over a run of cubes, with a curved intrados, voussoir courses and a springer or impost at the wall. It would work like a roof covering over roof cubes, or a timber member over a frame, rather than shapes of single cubes. An upside-down corner slope (or a groin piece) would close the corners.
- **Taller levels for vaulted themes** (for example 5 m), or the springing on piers in rooms. Then a vault rises above a 2.5 m wall instead of being cut down into the space, and its curve sits in frame at eye height.
- **Niches 0.5-0.6 m deep** with a dressed head and sill, a little fill light or bounce inside, and contents: bones, skulls, shrouded bundles, which are new props.
- **A second stone material for arches and piers** (ashlar against the walls' rubble). Without it no shape reads: every judge put the single material in their top three.

## Limits

- One seed (7), one theme, one film, and three judges, whose scores spread by about a point. A difference of 0.33 is noise.
- The judges saw the crypt alone, while round 23's judges saw all three themes. That may shift a score a little either way.
- The note doesn't test vaults made as a dressed piece, which is what the reviews ask for. It only tests the nearest that cubes make.
- Film (desk copy) and stills: `~/.cache/terra-review/final/dungeons/vaults/desk/`. Reviews and the spike's patches: `~/.cache/terra-review/final/dungeons/vaults/`.
