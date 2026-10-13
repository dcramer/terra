# Generating buildings from a drawn plan

2026-10-12. Question: how do shipped games and established techniques turn a drawn plan (walls, floors, storeys, openings, a roof's style and pitch) into a detailed building, and how hard is each part for us? Asked after the developers agreed The Sims' way of placing (D154) and wondered how hard it is technically with many variants. It covers roofs over any footprint, walls laid from a kit, drawing many pieces, building piece by piece, and telling what is indoors (D155).

## How this was found

Web research on the day, plus the pinned Bevy 0.19.1 source and crates.io. Nothing was built or measured. Few studios publish how their buildings are made: Townscaper, Clockwork Empires and Foundation's modding wiki are the only developer accounts found. Manor Lords' Unreal interview was behind a bot check and could not be read, and nothing else from its developer on building generation was found.

**Labels on each claim:**
- **developer**: the developer's own talk, blog, docs or modding wiki;
- **paper**: a peer-reviewed paper or library documentation;
- **second-hand**: press, wikis, guides, forum threads;
- **inferred**: my reasoning, not checked in anything running.

## Answer

- **Roofs:** a hip roof over any footprint is the straight skeleton of the footprint. For footprints whose walls run only in four directions, as ours do, that is much simpler (inferred, on published geometry). The roof's height is the pitch times the distance to the outside measured along the grid's axes. It needs no general solver, and gables, lean-tos and flats are a small step from there. Shipped games either let the player place overlapping rectangular sections (The Sims) or ran a general weighted skeleton and fought its edge cases (Clockwork Empires).
- **Walls:** shipped games use solvers (wave function collapse) where the shape is open and the look is free, as in Townscaper and Bad North. They use parameters and neighbour rules where the pattern is known, as in Foundation's walls and The Sims' corners. A running bond or a log wall is a pattern, not a search, so rule tables fit us, and WFC does not.
- **Drawing:** every game with player-built pieces at scale ends up merging static pieces. Assassin's Creed Unity did it on the GPU. Unreal's HLOD and Valheim's performance mods do it on the CPU. Bevy has the parts for both: automatic GPU instancing, `Mesh::merge`, and `VisibilityRange` cross-fades.
- **Building piece by piece:** the shipped models are a ghost that fills in (Sons of the Forest), ordered sections (Medieval Dynasty), and authored stages per part (Foundation). Our plan is already a list of parts, so progress can be one bit per part.
- **Indoors:** a flood fill over the grid (RimWorld, Space Engineers, The Sims' rooms) is exact and explainable. Valheim's ray counting needs no structure but gives odd answers, and players notice. Our grid makes the flood fill cheap.

## 1. Roofs over any footprint

### What games do

**The Sims 2 and 3** (second-hand). A roof is made of sections, each a rectangle in a style: gable, hip, mansard, shed, cone and more. The player places them one by one, and an auto-roof covers a room in one click. A Sims 2 builder's guide says that same-pitch hip sections "blend" where they overlap, that gable sections leave gaps unless drawn into one another, and that a hip pulled too far pokes through its neighbour. The roof is the overlap of rectangular sections. The game does not solve the footprint as a whole. **The Sims 4 dropped auto-roof**; players report that roofs clip unless there is a closed room under them.

**Clockwork Empires** (Gaslamp Games, 2013; developer). A designer marks a footprint and a style, and the game generates the building with Kelly and Wonka's procedural extrusions: roofs come from a weighted straight skeleton. Their post is titled "How to Debug an Exploding Building". Buildings exploded on missed edge cases, one of them newly collinear edges, and it took about seven rounds of fixes found by dumping every step to SVG.

**Townscaper** (developer, via [Game Developer](https://www.gamedeveloper.com/game-platforms/how-townscaper-works-a-story-four-games-in-the-making)). Roofs are tiles like everything else, chosen per cell. That works because every block is the same size; it has no pitch to choose.

**Foundation, Medieval Dynasty, Valheim, Enshrouded, Grounded** (second-hand). Their roofs are pieces in fixed shapes (Medieval Dynasty's and Foundation's from a building's own model). Enshrouded's players complain that roofs are hard to line up ([building-systems-prior-art.md](building-systems-prior-art.md)). Manor Lords' houses are generated to fit a plot of four corners the player drags out, and grow with its size. How its roofs are made was not found.

**City tools** (paper and documentation). CityEngine's `roofHip` and `roofGable` work on any polygon. CGAL (since 5.6) has weighted straight skeletons and extrudes them into closed roof meshes. Blender's OSM importer uses bpypolyskel. Its author reports success on 99.99% of about 320,000 OpenStreetMap hip roofs, but only after "several cleaning and merging algorithms". Even there, its output is not a true straight skeleton.

### The technique

- **A hip roof is the straight skeleton** (Aichholzer et al., 1995). Every eave rises at the same pitch, and the roof's height at a point is the pitch times the time the shrinking outline takes to reach it. Ridges, hips and valleys are where faces meet.
- **Gables, and roofs with mixed pitches, are a weighted skeleton.** Each edge moves at its own speed; an edge that does not move is a gable (Kelly and Wonka 2011; Held and Palfrader 2017).
- **It is fiddly to make robust.** Events fire at the same instant and edges become collinear. Footprints on a grid are exactly the degenerate case: a square's four hips meet in one point, and every rectangle's ridge ends where two events coincide.
- **For footprints with walls in four directions, it is a distance.** For such ("rectilinear") footprints, the straight skeleton coincides with the Voronoi diagram under the L∞ distance, the distance measured as the larger of the two axis offsets. This is stated in Aichholzer et al. 1995 and in Huber, Aichholzer, Hackl and Vogtenhuber 2014; their condition is general position, which our grid breaks (read in excerpt).
  - **The roof surface is simpler still** (inferred). Shrinking a four-direction outline at the same speed everywhere is the same as eroding it by an axis-aligned square. So the roof's height at any point is the pitch times the L∞ distance from that point to the outside. This is a plain function, so collinear walls do not matter.
  - **What follows from it** (inferred): every face is flat. On our 0.5 m grid, every ridge, hip and valley runs along a line of a 0.25 m lattice or along its diagonals. Holes such as a courtyard work the same way.
- **Gables over L, T and U shapes** have two shipped answers:
  - **Sections that overlap, the highest wins** (The Sims). Split the area into rectangles. Give each its own gable, hip or lean-to at the shared pitch, and take the highest roof at every point. Where two meet, this makes valleys by itself. A narrower wing's ridge runs into the main roof's slope, as on real houses. The pitfalls: a wider wing's gable end can stick up above a lower roof, needing a gable wall there; and which way to split is a choice that changes the look.
  - **A weighted skeleton** with the gable edges held still. It is general, but it is the code that exploded for Clockwork Empires.

### Rust crates (crates.io, 2026-10-12)

| Crate | Version, updated | Licence | What it does | Fit |
|---|---|---|---|---|
| [straight-skeleton](https://github.com/lizelive/straight-skeleton) | 0.2.1, 2026-07 | GPL-2.0+ | Integer grid input, exact predicates, holes, roof faces with hip, mansard and gable styles | GPL, so not for our MIT game; three months old, no users |
| [surfer2](https://github.com/atsyplenkov/surfer2-rs) | 0.1.0, 2026-08 | GPL-3.0+ | Port of the surfer2 research code | GPL; one release |
| [geo-buffer](https://github.com/1011-git/geo-buffer) / [geo-buf](https://github.com/njwitthoeft/geo-buf) | 0.2.0 2023 / 0.1.0 2025 | Apache-2.0 | Polygon offsetting by straight skeleton; the skeleton only as line strings, no faces or heights | Abandoned (the fork says so); would need faces built on top |
| [symbios-shape](https://github.com/TheJanusStream/symbios-shape), [bevy_symbios_shape](https://crates.io/crates/bevy_symbios_shape) | 0.4.1 / 0.10.0 (Bevy 0.19), 2026-10 | MIT | CGA shape grammar with 15 roof types over rectangular scopes, L and U shapes cut from them | One author, one star; rectangles only; a reference, not a dependency |
| CGAL Straight_skeleton_2 (C++) | 6.1 | GPL or commercial | Weighted skeletons, extrusion to roofs | Reference only |

No permissively licensed, maintained Rust crate computes roof faces over a general footprint. For ours, none is needed (above).

## 2. Laying a wall from a kit

### What games do

- **Townscaper** (developer, via [Game Developer](https://www.gamedeveloper.com/game-platforms/how-townscaper-works-a-story-four-games-in-the-making)):
  - **Tiles are corner pieces:** marching cubes places them at the corners of an irregular grid, and wave function collapse picks which modules fit the shape.
  - **Details in passes:** windows, chimneys and benches come in a later pass, and windows are cut with the stencil buffer rather than modelled into tiles, so a window never has to fit a corner.
  - **Recipes over search:** "recipes" such as gardens and stairwells are high-priority patterns, so players can find them again.
  - **It may fail:** it "is actually allowed to fail silently". A reskin would take "somewhere in the region of 500 tiles"; Bad North has over 300.
- **Marian Kleineberg's infinite city** ([developer blog](https://marian42.de/article/wfc/)):
  - **Sockets:** about 100 blocks, each face carrying a connector number and a symmetry; neighbours are allowed where the connectors match, plus exclusion rules.
  - **Failure:** it needs backtracking, which sometimes undoes a lot. The author thinks that makes it a poor fit for commercial games.
- **Foundation** (Polymorph; [developer modding wiki](https://www.polymorph.games/foundation/modding/)):
  - **City walls are a generator with parameters**, drawn as a line: crenellation spacing, width, height, UV tiling (`WALL_CONFIG`).
  - **Monuments are parts with categories:** a core, extensions, doors, with the counts required as data.
  - **Material variants are data:** a material set swaps a list of materials on the same model. One building constructor picks a random part from a list.
- **The Sims 4** (second-hand): walls sit on a grid, with separate corner meshes per wall height. Modders and a developer on Polycount infer the piece is chosen by its neighbours. Patterns misdraw at corners of walls one tile long.
- **Valheim, Grounded, Medieval Dynasty** (second-hand): fixed pieces with snap points, or fixed sections of a blueprint; nothing generated. Medieval Dynasty lets each section choose its material and whether it is plain, a window or a door.

### The technique

There are three ways to choose a piece:

| | How | Suits | Weak at |
|---|---|---|---|
| Rule table | A piece per situation: course parity, a wall's end, a corner (L, T, X), an opening's side, sill or lintel | Known patterns: bonds, stacked logs, posts and panels | Nothing to choose between, so little surprise; variety comes from versions |
| Corner tiles (marching squares or cubes, dual grid) | Look at the corners of a cell, take the tile for that pattern; 16 cases for two values, 6 with turning and mirroring ([Boris the Brave](https://www.boristhebrave.com/2021/11/14/classification-of-tilesets/)) | Shapes made of equal cells, where every tile is a cell | Pieces bigger than a cell, such as a metre block in a half-metre grid |
| Wave function collapse, sockets | Sockets must match; a solver fills in the rest | Open shapes where many looks are valid | Can fail or backtrack; hard to predict; a fixed pattern is wasted on it |

## 3. Drawing many pieces

- **Assassin's Creed Unity** (Ubisoft; [SIGGRAPH 2015 slides](https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf); developer):
  - **Its Paris is built from modular pieces**, "partially automated", with about ten times the instances of earlier games.
  - **The answer was GPU-driven rendering:** meshes cut into clusters, culled and drawn on the GPU, with one or two orders of magnitude fewer draw calls.
- **Unreal's HLOD** ([Epic docs](https://dev.epicgames.com/documentation/en-us/unreal-engine/hierarchical-level-of-detail-overview-in-unreal-engine); developer):
  - **Static actors are combined into proxy meshes** with atlased textures, one draw per proxy.
  - **Each layer is built a chosen way:** instancing, merging, simplifying or approximating.
- **Fortnite** (second-hand): uses octahedral impostors, a baked set of views, for distant objects.
- **Valheim** (second-hand, community):
  - **Every placed piece is an object.** Guides put stutter from about 8,000 to 10,000 objects in an area.
  - **The popular performance mods merge pieces.** They put static pieces to sleep and merge their meshes, on a 32 m grid or around a base the player marks; one reports doubling frame rate on large bases.
- **Our own forest test** ([forest-scale-test.md](forest-scale-test.md)):
  - **A million choppable trees:** each 65 m cell is baked into one mesh, rebuilt when one tree changes; the trees themselves are bits, not objects.

**Bevy 0.19** (checked in the pinned source and the [release notes](https://bevy.org/news/bevy-0-19/)):
- **Automatic instancing:** entities that share a mesh and material are drawn by GPU-driven multi-draw on Vulkan and DX12, as [procedural-vegetation.md](procedural-vegetation.md#bevy) describes; `MeshTag` passes a number per instance to the shader.
- **Per-entity cost stays on the CPU.** The release's own `bevy_city` benchmark draws 55,000 entities in 11.8 ms when static. Many of its gains are about doing less per unchanged entity: sparse uniform uploads, change lists, cheaper dirty-transform marking. So entity count still costs.
- **`Mesh::merge` and `Mesh::transformed_by`** combine meshes on the CPU (same topology and attributes).
- **`VisibilityRange`** cross-fades levels of detail with a dither, checked on the GPU since 0.19. Its docs describe replacing many meshes with one far away (hierarchical LOD) and a billboard impostor last.
- **Not found:** an impostor baker, or a crate for merging meshes or batching statics, for 0.19.

## 4. Going up piece by piece

| Game | What is placed | How progress shows | Source |
|---|---|---|---|
| Sons of the Forest | A ghost of the whole structure, or logs placed freely | Each log fills its place in the ghost; the companion can finish it; whole logs are cut to the lengths needed | Second-hand |
| The Forest | A ghost from the book | Logs and sticks added one by one; a counter of what is still needed | Second-hand ([prior art](building-systems-prior-art.md)) |
| Medieval Dynasty | A ghost of a fixed building | Built with the hammer in order: foundation, walls (the gable triangles count as walls), roof; each section's material and kind chosen first | Second-hand |
| Green Hell | A ghost | Materials put on in a set order | Second-hand (wiki) |
| Foundation | A building or a monument's parts | Each part has a construction model whose nodes are named `step_0`, `step_1`; they appear in order; nodes sharing a number appear in random order among themselves | Developer (modding wiki) |
| Manor Lords | A building or a plot | Ground levelled, materials hauled, frame then infill in stages; players say beams and stone "pop in" in phases | Second-hand |
| Valheim, Rust, Enshrouded, Grounded | A piece | Whole at once, paid on placing | Second-hand |

**Two representations:** a fixed list of stages authored per model (Foundation, Manor Lords), or each piece a slot to fill (Sons of the Forest). The first is cheaper to draw and author, and the second is what our "each stone is a part" means.

## 5. What is indoors

| Game | How | Strengths | Weaknesses | Source |
|---|---|---|---|---|
| RimWorld | Map split into 12 by 12 regions, cut by walls and doors; a room is regions enclosed by walls, doors, rock. Separate thresholds for temperature (25% unroofed, or open to the map edge), work and mood. Rooms over 36 regions are not rooms | Exact, cheap, incremental; tells the player why | Thresholds confuse; size limit | Second-hand ([wiki](https://rimworldwiki.com/wiki/Room)) |
| The Sims 4 | A room is an area enclosed by walls, or one with a floor; walls of any height count | Simple; floors and ceilings follow from it | Roofs clip over spaces that are not rooms | Second-hand |
| Space Engineers | Airtightness flood-filled over grid cells; each block says which faces are airtight; doors toggle theirs | Exact on a grid | Odd blocks leak; big rooms are slow | Second-hand ([wiki](https://spaceengineers.wiki.gg/wiki/Airtightness)) |
| Valheim | 17 rays from the player or station: one up, eight level, eight diagonal, to 30 m; sheltered needs 80% blocked and a roof (a non-leaky material straight up); stations need 70% | No knowledge of buildings needed; works in caves and under cliffs | Same house turned gives a different answer; big halls fail; trees and terrain count; players add a bar to pass | Second-hand ([community wiki](https://valheim-wiki.mave.me/Cover)) |
| Enshrouded | A Sheltered check for the Rested buff, with walls and a roof | Unknown | How it is worked out not found | Second-hand |

## What it means for us

Inferred, for [building.md](../docs/building.md); it decides nothing. The difficulty is my estimate for one developer with agents; the developers' art time is separate.

### Roofs

- **Recommended:** one height field per roof, on the grid.
  - **Sections:** the player's roof is one or more rectangular sections, each a style (hip, gable, lean-to, flat) at the roof's one pitch, and the roof is the highest section at each point. That is The Sims' model, and it gives valleys and dying ridges for free.
  - **Auto-roof** over an enclosed area has two options:
    - an L∞-distance hip over the whole footprint, any four-direction shape, holes included;
    - or a split into rectangles, longest first, for gables.

  The roof mesh, the gable walls and the frame are all read off the height field: every face is flat and every line falls on a 0.25 m lattice or its diagonals. No skeleton solver and no epsilons. That needs confirming with a spike first (below).
- **The detail:** cover the roof with a surface per face in the material (thatch, shingle, slate), plus trim pieces along eaves, ridges, hips and valleys.
  - **The frame:** common rafters on the 0.5 m grid lines, hip and valley rafters on the diagonals. Each rafter's cut length is known from where it meets a hip or valley.
  - **Not single shingles:** individual shingles as parts would be thousands per roof.
- **Pitfalls to design for:**
  - a gable end standing above a lower roof needs a gable wall there;
  - walls under one roof at different heights;
  - a roof over part of a lower storey;
  - eaves overhanging walls, and how they meet a neighbour's wall.
- **Difficulty:**
  - **Rectangles in four styles:** routine, days.
  - **The L∞ hip over any footprint:** small code, a week with tests; the risk is that the inference above is wrong somewhere, which a spike can show in a day.
  - **Several sections meeting:** where the time goes, weeks of edge cases, mostly where roofs meet walls.
- **Cut or defer:** mixed pitches in one roof, which break the lattice; dormers, curved and conical roofs; mansard and gambrel. Splitting complex footprints into gables automatically: start with the hip auto-roof and with sections the player draws, as The Sims 2 did.

### Walls

- **Recommended:** a rule table per pattern, not WFC. A wall's layout is a pure function of its plan, its neighbours' joins, its material kit and a seed:
  1. Split the wall at openings.
  2. Take each join's type (end, L, T, X) at each end.
  3. Lay courses by the pattern, choosing pieces by role: stretcher, half, corner by course parity, jamb, sill, lintel, cap, raked top under a gable.
  4. Pick a version of each by a hash of its position.

  This is the same on every machine, in shared code, so the server sends only plans and which parts are built.
- **Patterns are code, kits are data.** Four patterns cover the table in building.md: coursed bond, stacked logs, posts and panels, stakes and panels. A new material on an existing pattern is a data file: piece models with sizes and roles, and the pattern's parameters. A new pattern is code, rarely.
- **Pitfalls to design for:**
  - **Course heights:** a log's diameter or a course's height must divide a storey, or a top plate takes up the rest.
  - **Gable walls have sloped tops**, so pieces must be cut by a plane. Cut them at bake time in code, not by modelling a piece per pitch.
  - **Walls meeting at different heights.**
  - **The kit changes after building starts:** if a kit changes after a wall is half built, its layout must not reflow under it. Store which kit version a wall was laid with.
- **Difficulty:**
  - **Straight walls in a bond:** routine, days.
  - **Joins, openings and raked tops across four patterns:** a few weeks.
  - **The kits:** the bulk is art, about 10 to 15 pieces a material with a few versions each. Each kit, and the rule for its pieces' joins, is where looks are won or lost.
- **Cut or defer:** diagonal and round walls (already later, D146); carving finer than the grid.

### Drawing

- **Recommended:** a finished building is one merged mesh per material, rebuilt in a background task when a part changes. That is the forest test's approach, Valheim's mods' and Unreal's HLOD.
  - **Parts being built** are drawn separately, as instanced ghosts and pieces, until the wall or section is done.
  - **Far off,** a proxy made from the plan itself: walls as boxes, the roof's height field as a few planes, in the material's average colour. It needs no baking, and `VisibilityRange` fades between the two.
  - **Never an entity per stone** for a finished building.
- **Difficulty:** routine to moderate, a week or two. Measure a village of 50 cabins on both GPUs before committing.
- **Cut or defer:** impostors (the small planet hides most distance, as in the forest test); meshlets.

### Building piece by piece

- **Recommended:** the layout is the list of parts, in order: courses bottom up, corners first. A wall's progress is one bit per part (a 6 m stone wall 2.5 m high is about 30), sent and saved like the forest's chop mask. The ghost is what is not yet set.
  - **Roofs go in a handful of stages per face**, frame then battens then covering, as Foundation's steps do, not tile by tile.
  - **"Next part"** is the lowest unbuilt one nearest the builder, so holding the key builds a wall up course by course, and two players can work from both ends.
- **Difficulty:** routine once the layout exists, a week. The subtle part is keeping a part's identity stable when a plan is edited, which is the same reflow problem as a kit changing.

### Indoors

- **Recommended:** the flood fill D155 already proposes, RimWorld's and Space Engineers' way.
  - **Per storey,** over cells, with walls on the edges between cells. Open doorways and windows are edges weather crosses; a shut door is not.
  - **Covered** is a cell under a roof's height field or a floor above.
  - **Recomputed per building** when a part is finished or removed, never per frame.
  - **The rule is strict,** as D155 says: closed all round. It names what is missing, which RimWorld's thresholds and Valheim's rays cannot.
- **Natural shelter, such as a cave or an overhang,** is not a building. A Valheim-style ray count is a possible fallback for those, but that is a design question.
- **Difficulty:** routine, days. A large building at 0.5 m cells is tens of thousands of cells, nothing for a flood fill.

### Overall

| Area | Routine | Hard | Defer |
|---|---|---|---|
| Roofs | Rectangles; hip over any four-direction footprint (if the spike confirms) | Sections meeting, gable walls above lower roofs, roofs meeting walls | Mixed pitch, dormers, curves, automatic gable splitting |
| Walls | Straight bonded walls; versions by hash | Joins, openings and raked tops across patterns; the art of each kit | Diagonal and round walls, fine carving |
| Drawing | Merged mesh per building | Rebuild cost while building; integrated GPU | Impostors, meshlets |
| Piece by piece | Bits per part; ordered "next part" | Stable part identity when plans or kits change | Per-shingle roofs |
| Indoors | Flood fill per building | None found | Natural shelter |

**The first thing to build is a spike that answers one question:** is the L∞ height field the same roof as the straight skeleton for our footprints? The spike would take L, T, U and courtyard plans and draw them in the workbench. If it is, roofs are not the risk; the risk is the art of the kits and where roofs meet walls.

## Limits

- **Nothing was built or measured.** The roof shortcut is my reading of the geometry, checked by hand on rectangles and an L, not in code.
- **The L∞ result was read in excerpts.** The papers' condition is general position, which our grid breaks; I argue the height function does not care, but have not proved it.
- **No developer account was found for** The Sims' roofs or walls, Manor Lords' buildings, Medieval Dynasty, Enshrouded's shelter check, or Grounded's construction. Those rest on guides and forums.
- **Townscaper is known only through one article's retelling** of its talks. The talks themselves ("Organic Towns from Square Tiles", IndieCade Europe 2019; Bad North at EPC 2018) were not watched.
- **Valheim's ray layout is from community wikis**, which say Iron Gate do not publish it.
- **Bevy's per-entity cost is the release notes' benchmark,** not ours; how many separate pieces 0.19 can carry was not measured.

## Sources

Roofs:
- [Aichholzer et al. 1995, A novel type of skeleton for polygons](https://www.sthu.org/misc/SKRW14/papers/Aichholzer_1995_ANovelTypeOfSkeletonsForPolygons.pdf)
- [Eppstein and Erickson 1999, Raising roofs, crashing cycles, playing pool](https://www.sthu.org/misc/SKRW14/papers/Eppstein_1999_RaisingRoofsCrashingCyclesPlayingPool.pdf)
- [Huber, Aichholzer, Hackl, Vogtenhuber 2014, straight skeletons and Voronoi diagrams under polyhedral distances](https://publications.ist.tugraz.at/files/publications/geometry/ahhv-ssmvd-14.pdf)
- [Kelly and Wonka 2011, Interactive architectural modeling with procedural extrusions](https://peterwonka.net/Publications/pdfs/2011.TOG.Kelly.ProceduralExtrusions.TechreportVersion.final.pdf)
- [Held and Palfrader, straight skeletons with additive and multiplicative weights for roofs and terrains](https://arxiv.org/pdf/1604.03362)
- [CGAL: weighted straight skeletons and extrusion](https://www.cgal.org/2023/05/09/improved_straight_skeleton/)
- [bpypolyskel](https://github.com/prochitecture/bpypolyskel), [its announcement](https://community.osarch.org/discussion/423/)
- [Gaslamp Games: How to Debug an Exploding Building (2013)](https://archive-gaslamp.dredmor.com/?p=6161)
- [CityEngine roofHip](https://doc.arcgis.com/en/cityengine/2022.0/cga/cga-roof-hip.htm), [roofGable](https://doc.arcgis.com/en/cityengine/latest/cga/cga-roof-gable.htm)
- [Mod The Sims: Sims 2 roofing guide](https://modthesims.info/t/611828)
- [Sims 4: no auto-roof](https://simscommunity.info/2022/03/04/ultimate-guide-to-building-roofs-in-the-sims-4/), [EA forum on roof clipping](https://forums.ea.com/discussions/the-sims-4-mods-and-custom-content-en/roof-goes-through-vallroom-sims-4/9170626)
- Crates: [straight-skeleton](https://crates.io/crates/straight-skeleton), [surfer2](https://crates.io/crates/surfer2), [geo-buffer](https://crates.io/crates/geo-buffer), [geo-buf](https://crates.io/crates/geo-buf), [symbios-shape](https://crates.io/crates/symbios-shape)

Walls:
- [Game Developer: How Townscaper works (Tommy Thompson, 2022)](https://www.gamedeveloper.com/game-platforms/how-townscaper-works-a-story-four-games-in-the-making)
- [Sylves: the Townscaper grid](https://boristhebrave.com/docs/sylves/1/articles/tutorials/townscaper.html)
- [Boris the Brave: classification of tilesets](https://www.boristhebrave.com/2021/11/14/classification-of-tilesets/)
- [Marian Kleineberg: infinite city by wave function collapse](https://marian42.de/article/wfc/)
- Foundation modding wiki: [construction steps](https://www.polymorph.games/foundation/modding/_export/raw/construction-steps), [walls](https://www.polymorph.games/foundation/modding/_export/raw/walls), [monuments](https://www.polymorph.games/foundation/modding/_export/raw/monuments), [material sets](https://www.polymorph.games/foundation/modding/_export/raw/material-sets), [building asset processor](https://polymorph.games/foundation/modding/building-asset-processor)
- [Sims 4 Studio: corner walls](https://sims4studio.com/thread/598), [EA forum: patterns at corners](https://forums.ea.com/discussions/the-sims-4-feedback-en/corners-of-wall-patterns-not-applied-correctly/13548729)

Drawing:
- [Haar and Aaltonen, GPU-Driven Rendering Pipelines, SIGGRAPH 2015](https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf)
- [Unreal: HLOD overview](https://dev.epicgames.com/documentation/en-us/unreal-engine/hierarchical-level-of-detail-overview-in-unreal-engine)
- [80.lv: impostor baker for UE4](https://80.lv/articles/impostor-baker-for-ue4/)
- [Valheim Performance Overhaul](https://thunderstore.io/c/valheim/p/Skarif/ValheimPerformanceOverhaul/v/6.3.0/), [eip.gg on instance counts](https://eip.gg/valheim/guides/performance-optimization-tips/)
- [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/)

Building piece by piece:
- [eip.gg: Sons of the Forest construction](https://eip.gg/sotf/guides/construction/), [TechRaptor: Sons of the Forest building](https://techraptor.net/gaming/guides/sons-of-forest-building-guide)
- [Steam: Medieval Dynasty building](https://steamcommunity.com/app/1129580/discussions/0/3464974615681407892)
- [Steam: Manor Lords building process](https://steamcommunity.com/app/1363080/discussions/0/4358998116397481606), [Hooded Horse wiki: burgage plot](https://wiki.hoodedhorse.com/Manor_Lords/Burgage_plot/en)

Indoors:
- [RimWorld wiki: Room](https://rimworldwiki.com/wiki/Room)
- [Carl's Sims 4 guide: rooms](https://www.carls-sims-4-guide.com/tutorials/building/houses.php)
- [Space Engineers wiki: Airtightness](https://spaceengineers.wiki.gg/wiki/Airtightness)
- [Valheim wiki: Cover](https://valheim-wiki.mave.me/Cover), [gamever.io: the 17 rays](https://gamever.io/knowledge-base/the-17-rays-how-valheim-decides-whether-you-are-sheltered)
- [Twinfinite: Enshrouded comfort and shelter](https://twinfinite.net/guides/enshrouded-comfort-system-explained-how-it-works-what-it-does/)
