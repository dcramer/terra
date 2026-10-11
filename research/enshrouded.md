# Enshrouded's voxels and building

2026-10-10. Question: what do Enshrouded's voxel terrain and building do, now that the developers have named it a role model for both, and where does it meet what we have decided?

## What it does

From reviews, press and community guides. Keen Games has published no account of its terrain technology that I could find, so this is second-hand, and the game changes with its updates.

- **Its own engine.** Keen built the Holistic engine in-house for Enshrouded, on Vulkan. Its only public technical talk found so far is on global illumination over voxels.
- **The ground is material.** Players dig it, add to it and reshape it: flatten a slope, cut into a cliff, dig out below the surface. A rake levels dirt and grass for paths and pads.
- **Buildings are made of voxels too.** You can build block by block, as in Minecraft, from small pieces rather than large prefabricated parts, and there are also blueprints for whole floors, walls and stairs. Because both are voxels, pieces join seamlessly, blocks of different materials blend where they meet, and cutting an opening in a wall gets contextual styling.
- **Whether placement snaps to a grid** is contradicted between guides.
- **Sizes**, added 2026-10-11 from notes the developers shared (source not given): each voxel is a half-metre cube, so eight make a Minecraft-sized metre block. The construction hammer, with Alt held, steps through shapes and sizes such as 1, 2 and 4 metre foundations and walls. An altar's build area, upgraded, is 160 metres on a side, 320 blocks from edge to edge. This corrects a community guide's figure of a metre for the smallest block.
- **Water** was added later and is said to respond to terraforming.
- **Persistence.** Built structures do not decay. One server guide mentions terrain "regeneration"; how it works was not found.

## What the developers like in it

From the developers, 2026-10-10: the voxels are really well done; it looks beautiful and the art is good; the building is top notch; and its combat and movement are great. Probably it and Subnautica are our two biggest inspirations (D85).

On movement and combat, from memory and not checked: you sprint and climb on stamina, dodge, glide with a glider, and swing with a grappling hook; combat is melee, bows and magic, with blocking, parrying and dodging, fought in real time.

## Where it meets our decisions

Inferred, for the developers; nothing here changes a decision.

- **Digging and filling ground** is already what we have: smooth voxels, fully diggable (D9), and dig and fill operations in `terra_core`. A flattening tool would be another operation.
- **Building from voxels conflicts with D9**, which says structures are separate modular pieces, not terrain. Enshrouded's seamless joins and blended materials come precisely from building in the same voxels as the ground.
- **It pulls against D48**, whose inspiration is Subnautica 2's rooms that are shaped and resized. Enshrouded's is piece by piece and blueprint by blueprint. The two can coexist (rooms as blueprints), but which leads is design.
- **Combat by reflexes pulls against D3**, which answers threats with automation rather than reflexes.
- **Its look is not Forever Ago's** (D14): textured and painterly where ours is flat colour.
- **Water that responds to digging** is what D24 and D44 leave open.
- **Our terrain is smooth on a sphere** (D9: blocky voxels do not tile a sphere). Enshrouded's world is flat and its building blocks are cubes. Cube-shaped building voxels on a round planet need a local frame per base, which the base grid already has.

## Limits

Nothing here was checked in the game or against Keen's own material. No voxel size, chunk resolution or level-of-detail scheme was found.

Sources: [behind-the-scenes video, via RPG Site](https://rpgsite.net/news/15193-enshrouded-creating-world-enshrouded-behind-scenes-video), [PC Games Hardware tech test](https://www.pcgameshardware.de/Enshrouded-Spiel-74267/Specials/Test-Release-Steam-Gameplay-1438724/), [building trailer write-up](https://eip.gg/news/enshrouded-trailer-shows-off-building-mechanics/), [Guild Order community wiki](https://guildorder.com/games/enshrouded/wiki/base-building-and-terraforming), [Supercraft building guide](https://supercraft.host/wiki/enshrouded/base_building_guide/), [Khronos Graphics Programming Conference](https://www.khronos.org/events/graphics-programming-conference), [Wccftech](https://wccftech.com/enshrouded-is-a-zelda-ft-valheim-type-of-game-from-keen/amp/).
