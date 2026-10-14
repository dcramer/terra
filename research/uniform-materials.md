# One way to draw every construction material

2026-10-14. Question: how do shipped games draw every building material in one uniform way, and swap a placed piece's material fast? And, for us, does a material drawn as nothing but the cubes' surface and its textures (logs, shakes and thatch included) read as well as a modelled piece, as the developers asked of the modelled log ("tho it seems its mostly a texture thing?")? It informs [construction-002](../docs/proposals/construction-002.md); it decides nothing.

Each claim about a game is marked **official** (the studio's talk, notes or modding docs), **community** (wiki, players, modders, press) or **inferred** (mine).

## How shipped games do it

There are two families, and the games that look best use both.

**One cell, one material index, one mesher.** The cell (a voxel, a block face, an instance) stores a small number; re-skinning writes that number and the mesh is rebuilt or a shader reads it.

| Game | What a material is | What changes on a swap |
|---|---|---|
| Teardown | A byte a voxel indexing a palette of 256 materials per object; all palettes in one texture on the GPU ([Voxagon](https://blog.voxagon.se/2020/12/03/spraycan.html), official) | The index; paint rewrites indices through a table to shade variants packed in spare slots (official) |
| Minecraft | A block id per cell; a blockstate maps each state to a model with texture variables, so a stair's shape is shared by every material ([wiki](https://minecraft.wiki/w/Tutorial:Models), community) | The block; its 16³ section is meshed again, in the same frame with "Semi-Blocking" ([MC-248871](https://bugs.mojang.com/browse/MC-248871), official) |
| Connected textures (OptiFine CTM, Continuity) | The face's texture picked from its eight neighbours in its plane at mesh time: 47 tiles for the full "blob" set, 5 for the compact one ([OptiFine](https://optifine.readthedocs.io/ctm.html), official) | Texture choice only; geometry never changes |
| 7 Days to Die | A shape (2,000 and more) times a material, with a paint per face from one atlas, 255 paints in a fixed bit field ([devtracker](https://devtrackers.gg/7-days-to-die/p/623cbf0c-color-and-texture-question), community) | Paint: an index per face. Material upgrade: the block |
| Space Engineers | Armour is a model per side whose UVs span several blocks (`PatternWidth`, `PatternHeight`), with separate **edge models** laid along exposed edges (`ShowEdges`, `EdgeType`), and a model per construction stage (`BuildProgressModels`) ([Mod API](https://github.com/KeenSoftwareHouse/SpaceEngineersModAPI), official) | Skins replace texture sets per material slot, painted on; geometry unchanged ([wiki](https://spaceengineers.wiki.gg/wiki/Modding/Reference/SBC/AssetModifier_Definition), community) |
| The Sims 4 | Swatches: texture variants on one shared mesh; roofs a generated shape in a pattern texture, trims a separate piece along its edges ([Sims 4 Studio](https://sims4studio.com/thread/163/sims-studio-recolor-options-explained), community) | The swatch, on a placed object |
| Dual Universe | A material per voxel, meshed by dual contouring (official); how it is textured is not public | Not public |

**A modelled piece per material and shape.** The piece is the unit; a swap is a different actor.

| Game | What a material is | What changes on a swap |
|---|---|---|
| Valheim | A prefab per material and shape (wood, core wood, stone, black marble), roofs as 26° and 45° panels with separate ridge pieces (community) | Nothing in place: taken down and built again. Mods merge meshes because many draws choke it ([ValheimBuildOptimization](https://thunderstore.io/c/valheim/p/RockBreaker/ValheimBuildOptimization/v/0.5.5/), community) |
| Fortnite | Modular meshes per material, 350 to 400 pieces a building, HLOD merging regions ([Epic via 80.lv](https://80.lv/articles/guide-proxy-lod-tool-in-unreal-4-20), official) | The piece. Placing: 0.15 s to the first, 0.05 s to each next ([patch notes](https://www.pcgamesn.com/fortnite/turbo-build), official) |
| V Rising | Walls by tier; looks by **wallpaper** per wall face, free since 1.0 and painted by holding the button ([1.0 notes](https://blog.stunlock.com/v-rising-1-0-patch-notes/), official); roofs made automatically over closed rooms (community) | Wallpaper: a texture set on the same wall |
| Satisfactory | Foundations in FICSIT, concrete, asphalt and more: each its own buildable, swapped by the Customizer with its cost refunded ([wiki](https://satisfactory.wiki.gg/wiki/Customizer), community), so a mesh swap (inferred); colours and patterns are per-instance data the material reads ([modding docs](https://docs.ficsit.app/satisfactory-modding/latest/Development/Satisfactory/Paintable.html), official) | Material: the piece. Colour, pattern: a parameter |
| Rust | Building skins (adobe, brick, brutalist) in place of a grade's look ([Rustafied](https://www.rustafied.com/updates/2023/4/27/building-skins-and-more), community) | The piece, through the hammer |
| Townscaper | Corner modules by marching cubes on an irregular grid; a new theme would need "about 500 tiles" ([Game Developer](https://www.gamedeveloper.com/game-platforms/how-townscaper-works-a-story-four-games-in-the-making), community) | |

**Enshrouded**, the bar (D211), has said almost nothing about how its building is meshed. What is known: half-metre voxels; an octree of 6³ SDF "bricks" in a 3D texture atlas, with "terrain displacement maps" and "LOD differences to terrain/building voxels" named as open problems ([water talk, GPC 2025](https://graphicsprogrammingconference.com/2025/), official); a new "Brick Renderer" ships with 1.0 on 15 October 2026 with no talk yet ([Wccftech](https://wccftech.com/enshrouded-director-keen-games-bet-studio-future-5-million-players-paid-off-gamble/), official). Its galleries show each block material as a face fill running across cubes plus modelled pieces at edges, corners and bands, chosen by neighbours ([voxel-building-look.md](voxel-building-look.md), inferred from images); a cut hole gets a frame ([EIP](https://eip.gg/news/enshrouded-trailer-shows-off-building-mechanics/), community); roofs are 13 materials in slope shapes with ridge, verge and eave pieces (wiki, community). It has **no material swap**: replacing means breaking and building again, a request open since January 2024 ([feature board](https://enshrouded.featureupvote.com/suggestions/526075/upgrade-replace-change-or-swap-blocks-building-material), community).

**What they share.** No game found draws ridges, eaves, verges, quoins or log ends as texture alone: every one that has them uses a modelled piece or a neighbour-chosen variant (Space Engineers' edge models, Enshrouded's trims, The Sims' roof trim, Valheim's ridge pieces). No game found draws log walls as a textured surface. The fast swaps (Teardown, Space Engineers skins, The Sims, V Rising wallpaper, 7 Days to Die paint) are all swaps of a **texture set on unchanged geometry**; a change of **material class** (Satisfactory's concrete for asphalt, Rust's skins, Fortnite's upgrade) swaps pieces. No studio publishes a swap's latency beyond Fortnite's placing times.

**Terms.** A *texture array* is same-sized layers indexed by an integer, so one draw picks a layer per vertex or voxel ([OpenGL wiki](https://www.khronos.org/opengl/wiki/Array_Texture)); a *material ID* is that integer, per voxel or vertex; *triplanar* mapping samples along three axes and blends by the normal ([GPU Gems 3, ch. 1](https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-1-generating-complex-procedural-terrains-using-gpu)); a *trim sheet* is one texture of reusable strips many meshes map onto; a *modular kit* is grid-snapped pieces that combine ([Skyrim, GDC 2013](https://www.gamedeveloper.com/design/skyrim-s-modular-approach-to-level-design)); *mesh decals* are thin geometry blended over a surface ([Epic](https://dev.epicgames.com/documentation/en-us/unreal-engine/using-mesh-decals-in-unreal-engine)); *POM*, parallax occlusion mapping, ray-marches a height map per pixel (Tatarchuk, I3D 2006); *vertex colours as masks* blend layers such as dirt and moss; *HLOD* merges clusters of meshes into one far proxy ([Epic](https://dev.epicgames.com/documentation/en-us/unreal-engine/hierarchical-level-of-detail-in-unreal-engine)); a *spline mesh* bends a modelled length along a line, as our modules do ([Epic](https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Engine/USplineMeshComponent)).

## The spike

`spike/uniform-materials`, deleted with this note (D133). The same house (5 by 4 m, a door, two windows, a gable roof, on its footings) built through the remote's checked operations on the yard, in each candidate's kit, every kit a data file and nothing else, and pictured on the Windows GPU (`--time 15.5`, at `ac53df0`) from the same places: the street at 12 m, a corner at 5 m, close at 2 m, and the gable from below.

- **Logs today**: generated (D200).
- **Logs B**: the Tripo log (`art/kits/logs/log`), laid as each whole log by the kit's `timber.module`, otherwise today's kit.
- **Logs A**: a block kind drawn as its cubes' surface, its only change a texture set **baked from that same Tripo log** in Blender (four courses 0.25 m apart, colour, normal, occlusion and height, 2 by 1 m, the groove shaded and warmed toward the generated logs' colour), relief by parallax 4.5 cm. So A and B have the same log; only the geometry differs, which is the developers' question.
- **Stone A**: the stone kit with no role pieces, only its surface and masks.
- **Roofs A**: shakes, slate and thatch with no ridge, hip, valley, eave, verge or flashing pieces: the covering alone, its frame kept.

![Logs today, B and A at 12, 5 and 2 m; stone B and A; thatch, slate and shakes, B and A](images/uniform-candidates-2026-10-14.jpg)

**What it showed.**

- **It is not mostly a texture.** At 12 m A's log wall passes. At 5 m it reads as a striped box: no log ends at the corners, its courses wrapping round the corner as a rounded bevel, its top edge and reveals flat, the window's reveals showing the texture across. At 2 m the silhouette is flat and the parallax swims. The same texture on the modelled log reads as logs at every distance. The texture is why B's logs beat today's along a wall (bark lost in patches, silvered wood, a log's own end); the geometry is why both beat A at a corner. As timber C found in 2026-10-12 ([timber-walls.md](timber-walls.md)): a surface cannot end a log.
- **The look can be baked onto geometry the rules lay.** A second test the same day, by the agent working on logs, put the same Tripo log's bark on the generated logs: baked onto a cylinder, tiled along each log, its normal steepened, its height from the model's lumps (settings in that agent's scratchpad, not kept). On the village's cabin at the street, a corner and close, the generated logs in the modelled bark get most of the way to the modelled log and keep their sawn ends and corners; what only the model has is a lumpier outline, branch stubs, and fatter logs that show less daub. So a hand-made reference piece, its look baked onto what the rules lay, is a third way between generating and placing models: the industry's high-to-low bake, used here for a material rather than one asset.

![Generated logs as now, generated logs in the modelled log's bark, and the modelled log, at the street, a corner and close](images/logs-texture-test-2026-10-14.jpg)

- **A log kind drawn as A is also a cube thick**, 0.5 m against the logs' 0.3, and goes up a cube at a time, not a log at a time, and its ghost cannot show its logs, as D139 asks a blueprint to.
- **Stone without its pieces** is one seamless rubble wall at 12 m and a pile of rubble cubes at 5 m: no lintel over the openings, no quoins, plinth or coping, as role-pieces' candidate 2 found ("about a third of the way").
- **Roof coverings already are A**, and hold up. Without their edge pieces, slate and shakes read thin at the verge but clean; thatch reads cleaner than today's modelled verge and eave, which stand out as golden rolls and lumps (the third goals review's finding 8). A ridge with no capping is a crease, which real thatch and slate never are.
- **Swap speed is not set by how a material is drawn.** A 1 m box of each house changed to another material through `build.replace`, built step by step, on a base of the seven houses: from each operation to its change on screen (`Build: change on screen`) 110 to 145 ms for every candidate, 250 to 300 ms for the modelled log. Of that, laying the base again took 64 to 90 ms and the slowest section 37 to 72 ms; the draw is the least of it. A dev build (optimised, with debug information), the machine at load 7 to 18. The modelled log's extra time was not traced.

## What it means

- Surface-only materials (A) suit what has no direction and no edge: stone's, earth's and plaster's faces, floors, roof coverings. They fail every material whose look is its members or its edges: logs, frames, quoins, lintels, ridges.
- Pieces for everything (C) fail what the cubes promise: carving anywhere, any shape, any thickness, with no grid showing ([block-modules.md](block-modules.md)).
- A hand-made reference baked onto rule-laid geometry gets a log most of the way with nothing placed, and keeps what the rules give (ends, corners, any length); it gives up only what is in the outline.
- Every good-looking builder found does both: a surface or panel fill from the material, and modelled pieces at its edges and members, chosen by neighbours. That is today's block and roof drawing, and D251.
- A fast swap is a texture-set change on geometry that stays. For us the cost of a swap is laying the base and its sections again, which a swap of material, where no cube's solidity changes, need not do in full.

## Limits

One light, one log, one house, judged on stills by an agent. A's log texture was baked once, not refined; a better bake would help at 12 m and change nothing at a corner. The edit times are on a shared machine, a dev build, and a base of seven houses in a row (an edit there reaches 40 sections); they are comparisons, not budgets. Enshrouded's own method is still inferred. C was not built: the block-modules spike and the games above stand for it.
