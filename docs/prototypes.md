# Prototypes

The MVP, the first playable (D105), and how we judge it. Where the engine under it stands is in [engine.md](engine.md#tracks).

## Before

The first builds were on Godot: a tech spike that chose the engine and the terrain (D9, D12), a simulation toy for terraforming, now parked (D60), and a tech demo joining them. They were deleted on 2026-10-12; what they found, and the numbers this build has to beat, are in [research/godot-spike.md](../research/godot-spike.md), and their code is in git history. The outpost scene, a first picture of the game in Rust ([research/outpost-scene.md](../research/outpost-scene.md)), grew into the game that the MVP is built in.

## MVP: the first playable

Leaning (D105), for the developers to edit. Being built in `terra_game`: `cargo xtask play` plays it, `cargo xtask loop` plays its loop by script and checks every step. Where it stands is below.

**Question:** is the core loop of [the game's design](gameplay.md) fun, with almost nothing else around it? It is a prototype of that design (D89), built to answer this question before anything is spent on scale.

If the loop is not fun with one monster and one settlement, more content will not fix it.

### Where it stands

| Part | State |
|---|---|
| One small world, day and night | Voxel terrain, dug with the pickaxe for stone and ore, saved. Day and night: the sun crosses the sky in half an hour, then moonlight and stars; the time is saved, and `--time` sets it for a script. Night is only dark so far (D134). A torch, as the developers asked: one to start with, more from a campfire; in hand it lights the ground round you |
| Gathering with separate tools (D96) | Built |
| Crafting at stations, timed and queued (D109) | Built: workbench, smelter, forge, campfire |
| A claim, upgradable (D94, D104) | Built |
| Building on the voxel grid (D48) | Walls, floors and roofs of half-metre cubes (D181), and items; what is built cuts into the ground it stands on, once trees and rocks are cleared (D131). Plans constructed part by part (D139) are missing |
| Backpack, equipment, action bar (D84); a belt quiver (D118) | Built; the bar has eight slots. Wear and repair (D99): tools, weapons and leather wear with use and stop working when broken; a line under each icon shows it; opening the station that makes a thing, in your claim, mends it free |
| One wildlife, hunted for meat and hide | Built: deer, with the bow |
| One monster, forming where magic pools (D103), holding a prisoner | Built: spiders at a glowing hollow |
| Combat: melee, a bow, block, dodge, stamina (D97) | Built, with hit zones (D119): head, body and legs from each creature's joints, the head worth twice a body; a blow it did not see coming is a critical |
| One settlement, a request, the prisoner to rescue, who runs the forge (D90, D95) | Built, the village from building parts (D108) |
| Skills by doing (D88) | Built, archery among them (D117) |
| Hunger and food, death and the dropped pack (D101) | Built |
| The map, only where you have been (D76) | Built |
| A local server (D51) | Missing |
| Saving and loading | Built |
| Feel: swing animations for the axe and pickaxe, a better harvesting experience, a small notice of what you gained when you harvest or pick something up, hit effects when damage lands, and the like | Built: what you gained, listed at the right as it comes in; a blow sprays blood or ichor and jolts what it struck; being hurt reddens the edges of the view and shakes it. Swing animations are under way separately |
| Camera: zoom in and out with the mouse wheel, later all the way to first person | Built: the wheel, or = and -, from 1.4 to 9 m behind; while the hammer has a part out that the wheel turns, it turns that instead. First person is to come |
| Building: move what is being placed up and down, not only turn it, except what must stand on the ground | Built: Shift and the wheel |
| Done when: a developer plays it, an agent drives it, save and reload, frame times | The agent's run passes; the developers' play-test, frame times on the integrated GPU, and a save-and-reload check in the script are to do |

### In it

- **One small world**, smooth voxel terrain (D9), one biome, day and night. Everything in reach within a few hundred metres; the developers want the MVP small in scale, not a large world.
- **Gathering** with separate tools (D96): an axe for trees, a pickaxe for rock and ore, a knife for fibre and hide.
- **Crafting** at a workbench, a smelter and a forge, taking time and queued (D109), from a handful of materials: wood, stone, fibre, hide and one ore. Tools made of better material work more (crafting.md).
- **A claim** you place, 40 metres on a side, upgradable to 80 and 160 (D94); walls, floors and roofs on the voxel grid (D48, D181); the world heals outside it.
- **The backpack, equipment and action bar** (D84), with wear and repair (D99).
- **One kind of wildlife** that grazes, flees and can be hunted for meat and hide.
- **One kind of monster**, forming at one place where magic pools (D103), dangerous to go near, holding a prisoner.
- **Combat**, third person (D97): one melee weapon and a bow; attack, block, dodge; stamina.
- **One settlement**, built from the same parts as the player's buildings (D108), with a few people, a request in words, and the prisoner to rescue (D95). Rescued, they come home with you and run the forge (D90).
- **Skills by doing** (D88): a few, such as chopping, mining, a weapon, crafting.
- **Hunger** and food (D101); **death** drops the pack, costs a little skill progress, and you wake at your bed.
- **The map**: only where you have been, and where you are (D76).
- **A local server**: the game talks to a server inside it (D51).
- **Saving** and loading.

### Not in it

Essences and abilities, raids, factions beyond the one settlement, portals and other worlds, taming and mounts, trade, many players, the spread. Essences are the first addition after it.

### Done when

- **A developer plays it from a fresh start**, with nothing explained beyond the controls, and finds their way through: gather, craft, claim, build a shelter, eat, find the settlement, rescue the prisoner, bring them home, have them make a better tool. Both developers say whether it was fun, which is the question.
- **An agent can drive the same loop** (D69) and see that each step worked.
- **Save and reload** restores the claim, what is built, what is carried and the recruit.
- **Frame times** meet D25 in the play loop on the RTX 5090, and are reported on the integrated GPU.

### What it needs first

Engine work: the voxel terrain and building on the voxel grid are built; collision and a character controller on voxels (D53) and the local server (D51) are not. Placeholder content (materials, recipes, weapons, numbers) is chosen as it is built, as the inventory's was.
