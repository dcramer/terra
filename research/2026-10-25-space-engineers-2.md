# Space Engineers 2's terrain, mining and building, against ours

2026-10-25. Question: the developers name Space Engineers 2 as closer than Enshrouded to the voxel terrain and building they want, "fully procedural" being the main difference. What does it do, and where does it differ from ours?

## What it does

From Keen's announcements and the community wiki; Keen has published no technical account of how VRAGE3 stores or meshes voxels, and the game is in early access (since 2025-01-27), so this changes with its updates. Marked *SE1* where the behaviour is the first game's and SE2's notes only name it.

**Terrain.** Planets are voxels on VRAGE3: a planetary heightmap sets the base shape, a generator places small voxel pieces of overhangs and boulders where they fit, five kinds of cave (modelled outside the game) are subtracted, and landmarks are shapes projected onto the voxels and eroded. Full 3D surfaces (overhangs, arches, caves), where SE1 was a heightmap. Small rocks are hand-modelled meshes, not voxels, and fracture; trees and bushes fracture too. Enshrouded's world, by contrast, is made by hand.

**Hardness.** Each ground material has a hardness: snow and sand give deep, soft deformation, rock resists, in impacts, collisions, landings and mining alike.

**Mining.** Hand drills of four tiers, each faster, held to remove ground continuously and take what comes out into the backpack by volume (*SE1*'s drill a sphere ahead of the tool); they also detect ore nearby, as an ore detector block does for a ship. What a planet's ground gives follows its kind: soil-like gives iron, rock-like nickel, sand-like silicon. Keen says resources will be spread more unevenly across sectors later. Survival cannot put ground back (*SE1*).

**Building.** One 25 cm grid for every block: SE1's small (0.5 m) and large (2.5 m) grids became one compound grid, blocks of many sizes on it. A base starts from a block set in the ground or on another block (a static grid); a vehicle is a grid of its own.

- **Projections are the plans.** Blocks go down as holograms, free, without physics, walked through; a whole blueprint goes down as one, with mirroring, copy and paste, undo, and support blocks added automatically.
- **Welding builds them.** A hand welder (four tiers, each faster) turns a projected block real from what the backpack holds, components or, for basic blocks, raw ore processed on the spot. An area welder (VS2.2) welds many blocks at once; welder blocks do it on a ship or base. In *SE1* a block goes up through construction stages, a frame, then partly clad, then whole, as its components go in; it works only above a share of its integrity; ship welders draw through conveyors from storage.
- **Grinding takes it back.** A grinder (four tiers) takes a block down and returns its materials to the backpack (*SE1*: through the stages in reverse). Damage lowers a block's integrity the same way.
- **The look.** Blocks are their own models; the render gives armour edges and the joins between colours of block parallax and subtle rounded bevels. No blending of material between blocks as Enshrouded's.

## Where it meets ours

| | Space Engineers 2 | Enshrouded | Ours now |
|---|---|---|---|
| The world | Procedural planets | Made by hand | Procedural (D8), a 3D density field (D9), caverns possible (worlds.md) |
| Digging | Held drill, continuous, rate by tier and hardness | A tool-shaped chunk at a time | A 0.7 m scoop every 0.9 s (`terra_survival::dig`) |
| What it gives | Every ground kind something, by volume | Items by material | Items by volume per material (`assets/data/ground.ron`); soil and sand nothing |
| Ore | Deposits in the ground, uneven, found with a detector | Veins and nodes | A voxel material in veins through all stone (`GeneratedPlanet`, 9 m noise); ore rocks stained (D383) |
| Putting ground back | No | Yes | Digging then filling restores (terrain.md); not a player act |
| Plans | Projections, free | Blueprints of floors and walls | Plans, free, ghosts walked through (D139, D192) |
| Constructing | Weld: per block, staged, from pack or network | Placed at once from the pack and chests | Hold the hammer at a plan, part by part, from the pack (D201); storage built (D295), paused (D333) |
| Taking down | Grind, stage by stage, all back | Taken out, all back | Take apart, last in first out, all back |
| A block going up | Construction stages drawn | None | Drawn as far as filled (D358) |
| Damage | Integrity per block | Blocks destroyed | Damage, collapse and fragments (D305) |
| Grid | 25 cm, blocks of many sizes | Voxels of 0.5 m | 0.5 m cubes per base (D181) |
| Look | Modelled blocks, bevels | Material blending by neighbour | Enshrouded's bar (D211, D215, D290) |

So the construction loop is SE's already in outline (plans, built from what is delivered, taken back whole); what is Enshrouded's in ours is the look and the hand brushes. Mining is where SE2 differs most: continuous drilling by hardness, every ground giving something, and ore as deposits to find.

## Sources

- Keen, [VS2: Planets & Survival Foundations](https://2.spaceengineersgame.com/space-engineers-2-vs2-planets-survival-foundations-live-now/) and its [patch notes](https://support.keenswh.com/spaceengineers2/pc/announcement/space-engineers-2-alpha-vs-2-planets-survival-foundations)
- Keen, [VS2.2: Mechanical Blocks & Weapons](https://support.keenswh.com/spaceengineers2/pc/announcement/space-engineers-2-alpha-vs2-2-mechanical-blocks-weapons)
- Space Engineers 2 wiki: [Building](https://spaceengineers2.wiki.gg/wiki/Building), [Projection Building](https://spaceengineers2.wiki.gg/wiki/Projection_Building), [Backpack Building](https://spaceengineers2.wiki.gg/wiki/Backpack_Building)
