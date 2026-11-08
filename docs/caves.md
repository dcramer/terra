# Natural caves

Status: **designed, 2026-11-08; not read by the developers; step 1 built** (D591, D643): `terra_voxel::caves` carves systems from a generated planet's ground (`GeneratedPlanet::with_caves`); and step 2: every world has a den found from its seed (`terra_land::cave`, `Planet::den`, `den.state`), held by the sweep on seeds 1 to 128. Step 3 in part: a cave's walls and the lip of its mouth are rock, nothing of the land grows or lives in one (`Planet::in_cave`), its mouth is kept clear, and `tools/sandbox/den.ts` walks from the mouth to the chamber; looked at in the game it is a rock passage going down to a room. Not yet: the sky's light is not shut out underground, so it is not dark; the camera in a tight place; a few grass blades still stand on the mouth's walls. Research: [natural-caves](../research/natural-caves.md).

Natural caves are part of how a world is made (D591), and v0's boss, the troll, lives in one that fits a 4 m body and a fight and is reached from the surface on every seed. A cave here is a **system placed by rule and carved from the ground's distance field** (D643): the best fit of what shipped games do (Valheim places its troll caves; Deep Rock Galactic carves authored rooms joined by rule; Minecraft kept placed carvers beside its noise caves), and the way our streaming needs no new rule.

## What a cave is

- **A system**: a mouth on a hillside, passages and chambers. Its parts are capsules (a passage's stretch) and ellipsoids (a chamber), in the planet's space, their walls roughened by a noise of some decimetres.
- **Carved** by subtracting the system's distance from the ground's with a smooth maximum (Voxel Tools' `SdfSmoothSubtract`, MIT, ported): the field stays a distance, so the chunk rule (`lod::may_hold_surface`) meshes a chunk by a cave's wall and skips solid rock, the mover walks it, and digging works in it as anywhere.
- **Placed from the seed**, as the gate is (D641): where the survey finds a hillside a walk from the start reaches, dry, with rock enough over where the chamber goes.
- **By the theme** (worlds.md, "how hollow"): how many systems a square kilometre and how big, a row of the realm's data, when more than the den is built.

## Sizes (dungeons.md's natural caverns)

| Part | Size |
|---|---|
| Passage | 3 to 6 m wide, winding and rising, no steeper than a walk takes |
| Chamber | 15 to 40 m across, 8 to 20 m high |
| The troll's den | a passage a 4 m body passes (5 m and more), one chamber with a clear floor 12 m across, rock at least 3 m thick over it |

## Where it lives (D198)

- `terra_voxel::caves`: the carve: a system's parts, its distance, the set a planet carries (`GeneratedPlanet::with_caves`). Plain Rust, tested alone.
- `terra_land::cave`: where a system goes and what shape it takes on a seed's land: the finder, by the survey, and its faults for the sweep (`cargo xtask worlds`).
- The game: what is in a cave (the troll, D590) and light, camera and sound underground, each its own step.

## Build order

1. The carve: parts, distance, the planet's field with caves; tests that the inside is hollow, the rock round it solid, the field still a distance the chunk rule can trust, and what it costs a chunk far from any cave (measured).
2. The den found from the seed: `terra_land::cave::find`, the world made with it, and the sweep holding one den a seed, its mouth reached by a walk, its chamber fitting the fight, its roof whole.
3. Seen: walked into in the game, stills from the mouth and the chamber; the sun's shadow and the sky's light underground; the camera in a tight place.
4. More than the den: systems by the theme's numbers, and what they hold.

## Variants (D182)

The den: three passage shapes (straight, one bend, a dog-leg down), three chamber proportions, none to two alcoves: 27 layouts, told apart by the way in and the room's shape. Other systems are for step 4 to count.

## Open

- What caves hold besides the troll (worlds.md, question 4): the developers'.
- Whether a den's mouth is marked on the surface (a path, bones, a smell of smoke): the developers'.
