# A forest as rich as big games make them

2026-10-12. Question: can the world feel like a dense, rich forest, as The Witcher 3 or Sons of the Forest do, within our frame budget (D25)? Asked by the developers as a test, outside the art direction of the time (D14, then flat colour; it moved to textured, lightly stylised realism the same day, which this forest fits). The first version looked right and ran at about 7 frames a second; they asked for it to be made fast, with a draw distance, its trees cut as the game's are, and grass, bushes and paths in place of ferns.

## Answer

Yes, on the machine measured. A forest of about 2,700 oaks, grown and young, 1,500 bushes, grass and paths, 440 m across, runs at a median 5 ms a frame at 1920x1080 on an Apple M4 Max, about 1.6 ms more than the empty yard. The first version, 399 oaks in a third of the area, took 99 ms. Nearly all of that went on one thing: every leaf card drawn into the shadow maps.

Not yet measured on Windows or on an integrated GPU, which D25 asks for.

## What it is

The sandbox's `lush` experiment ([lush.rs](../crates/terra_sandbox/src/experiments/lush.rs)), on the yard:

- **Trees grown in code.** A trunk splits into limbs, branches and twigs, each a tube with bark on it; clumps of leaf cards, each a picture of a spray of oak leaves, sit at the twigs. Normals lean out of the crown so it lights as one, leaves deep and low in it are darker, light passes through them, and everything moves in the wind ([foliage.wesl](../assets/shaders/foliage.wesl)). Six mature kinds of 16 to 25 m and three young of 7 to 11 m, each drawn many times.
- **The game's trees.** Each tree is an entity with the game's `Growth`, so it stops the player and is cut as the flora file's leafy trees are: young ones with a stone axe, grown ones only with an iron axe (D96). Checked by script: a young oak gave 3 wood to a stone axe; a grown one refused the stone axe and gave 8 to an iron one. Bushes cut for fibre.
- **Undergrowth.** Grass in tufts of solid blades, thinner under trees and none on the paths; bushes along the paths and in glades; paths as strips of earth over the ground: a ring through the forest and two ways out.

```bash
cargo run --release -p terra_sandbox -- --experiment play --experiment lush --level yard --time 13
```

At 13:00 the sun comes through in flecks. Add `--give iron_axe:1` to fell the grown oaks.

## Where the time went

Measured with switches in the code that left parts out, then with [`cargo xtask bench`](../docs/development.md#measuring-performance) once it existed. Release build, 1920x1080 offscreen, 17:00, M4 Max, unless marked.

| Step | Median frame |
|---|---|
| First version: 399 oaks, every card in the shadow maps, no levels of detail | 99.5 ms |
| Debug build, on screen: the same with leaves casting no shadow | 4.9 ms (from 101) |
| Levels of detail, a 180 m draw distance, the forest six times larger, crowns casting from sparse card stand-ins out to 25 m and solid clumps to 80 m | 7.4 ms |
| Leaf cards cut to an octagon round the leaves | 6.8 ms |
| Mipmaps, keeping the leaves' coverage, and anisotropic filtering | 4.5 ms |
| Recorded baseline, three runs (yard alone: 3.4 ms; the game's own world: 7.3 ms) | 5.1 ms |

Walking down a path, the 99th percentile was 26 ms until benchmarks stopped waiting for the ground (below); it is now 8.8 ms, with a median of 4.8 ms.

Parts, in the version before trimming: the card stand-ins' shadows 2.1 ms, the rest of the trees 1.5 ms, grass 0.2 ms, bushes 0.1 ms. The solid clumps' shadows cost 0.16 ms for every crown from 25 to 80 m.

## What made it fast

1. **Find where the time goes before changing anything.** The first guess, too many triangles, was wrong: with leaf shadows off, the frame went from 101 ms to 4.9. A low sun looks sideways through the whole forest, so each shadow map texel is covered by dozens of overlapping cards, each read and tested against its transparency. A 2048 shadow map instead of 4096 more than halved the time, confirming the cost was pixels, not shapes.
2. **Shadows come from stand-ins.** Crowns cast no shadow themselves. Near the camera a sparse copy of the crown, two larger cards per clump, keeps the light coming through in flecks; farther out, a solid rounded lump per branch casts it; past 80 m, nothing. The stand-ins are on a render layer only the sun sees. This is CryEngine's shadow proxy, where a 3,969-triangle tree casts from 640 ([docs](https://cryengine.com/docs/static/engines/cryengine-3/categories/1114113/pages/21268752)), and Horizon Zero Dawn kept vegetation in only the near shadow cascades. Unreal culls foliage shadows nearer than the foliage.
3. **Solid wherever it can be.** A GPU skips what is hidden before shading it, unless the shader may throw a pixel away, as cut-out cards do; on tile-based GPUs like Apple's and PowerVR's, that switches hidden surface removal off ([PowerVR](https://docs.imgtec.com/starter-guides/powervr-architecture/html/topics/rules/do-not-use-discard.html)). Grass is solid blades, not pictures; the far shadow lumps are solid.
4. **Levels of detail and a draw distance.** Each tree is drawn three ways: in full to 30 m, then without twigs and with a third of the cards to 75 m, then trunk, great limbs and one card a clump to 180 m, dithering from one to the next over 6 m (Bevy's `VisibilityRange`). Shadows use the same level as the camera. Drawing 2,700 trees came to under a millisecond.
5. **Cards cut to their leaves.** A square card whose picture is 43% leaf shades every empty pixel. An octagon fitted to the picture covers 65% of the square, for six triangles instead of two: Emil Persson's particle trimming, who found about eight corners the point past which more do not pay ([slides](https://www.humus.name/Articles/Persson_GraphicsGemsForGames.pdf), [tool](https://humus.name/index.php?page=Cool)).
6. **Mipmaps that keep the leaves.** Bevy makes no mipmaps for a PNG, so far bark glittered and every distant leaf read the full picture. Averaging alone thins cut-out leaves until crowns go see-through at a distance, so each smaller copy's transparency is scaled to keep as much above the cut as the full picture has ([Castaño](https://ludicon.com/castano/blog/articles/computing-alpha-mipmaps/); Unity's `mipMapsPreserveCoverage`). With anisotropic filtering, this alone took a third off the frame.
7. **Undergrowth near the camera only, built off the main thread.** Grass exists in 8 m squares within 45 m of the camera, built on other threads as the camera moves, at most two put in a frame, and dropped behind it.

## Measuring it

What the numbers above needed, now kept (D185): `cargo xtask bench` and its scenes, recorded in [benchmarks.csv](benchmarks.csv); the sandbox's `--offscreen`, which draws into a 1920x1080 picture with one frame in flight, because macOS draws nothing of a covered window or a locked screen; and each run naming its slowest frames, which found the next problem.

**Walking stuttered, in the yard too.** Slow frames came exactly every 56 frames, every 4 m walked: a scripted run waits for each chunk of ground to be meshed so screenshots see the same ground every time, and the ground chooses its chunks again every 4 m. Benchmarks no longer wait. Without the wait, the same step costs a frame about 4 ms in the yard; that is the ground's to improve, not the forest's.

Along the way: a scripted screenshot counted an old file at its path as taken; Bevy's GPU timestamps lose the device on Metal and are now off on macOS; Bevy can hang while closing, so a finished scripted run that does is ended after five seconds.

## Open

- **Rare single slow frames**, 15 to 30 ms about once in ten seconds, in the forest standing still and in the game's world scene too; not tied to walking or grass. Next: a GPU capture in Xcode, or Tracy.
- **A low sun puts the whole floor in shade.** At 17:00 the solid lumps block it everywhere, so the flecks are gone. Smaller lumps, or cards farther out, would let some through at some cost.
- **The edge of the draw distance.** Trees vanish at 180 m. In a dense forest the trees in front hide it; from a clearing or a hill it would show. Impostors, pictures of each tree from several sides, would carry the forest to the horizon.
- **One generator, one species.** More species need their own branching rules and leaf pictures.
- **Grown or modelled.** D14 now asks for real materials, which these trees have: real bark, real leaves. Whether the game's trees are grown in code like these or modelled through Tripo is open; flora.ron cannot yet sway a textured model's leaves apart from its trunk.
