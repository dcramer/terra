# How good-looking voxel builders draw their blocks

2026-10-13. Question: how does Enshrouded draw its player building, so that cubes that touch blend into one structure and each cube's look changes with what it touches and where it sits, and how do other good-looking grid builders do it? What technique is common to them, and what path takes Terra to the bar D211 sets?

The developers: "so enshrouded has cubes right, and then when two cubes touch, it somehow blends it. thats really what we're after … they change visually depending on what they're touching or where they're located"; and "this doesnt mean we're copying enshrouded, but we want to take an approach to voxels similar to how they've done it". So this note studies the technique, not the look; ours stays D14's.

Each claim about Enshrouded is marked **Keen** (Keen Games said it: talks, slides, patch notes, developer answers), **players** (community reports, not checked) or **seen** (my reading of in-game images: the block galleries on the official wiki, which show each block material built into the same test piece, and Steam store screenshots). Keen has given no talk on how building blocks are meshed or textured; the method below is pieced together from all three.

## Enshrouded

### The grid and what a voxel stores

- **Half-metre voxels, everything voxels.** "Voxel world: everything can be destroyed and built. Voxel size of 0.5³ m³" (**Keen**, [water talk, GPC 2025](https://graphicsprogrammingconference.com/2025/), slide 2); a developer on Reddit: "they're half a meter a side, like our blocks" (**Keen**, [AMA via devtrackers](https://devtrackers.gg/enshrouded/p/f634f585-enshrouded-ama-come-ask-the-developers-your-questions-about-the-world-of-embervale)). The rest of the world's geometry is a sparse voxel octree: "using full voxels like with the rest of our geometry … updating a sparse voxel octree" (**Keen**, the same talk's notes).
- **A voxel stores a material, and more.** Asked where GI rays get material properties, the speaker said they are "encoded in a voxel world … in some very optimized way" (**Keen**, [GI talk, GPC 2024](https://www.youtube.com/watch?v=57F1ezwH7Mk)). Building voxels carry decorative states: a decay tool and an overgrowth tool paint them per block ([Pact of the Flame](https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/1789580505469416)), and "voxel buildings can now have a withered decorative appearance and plant overgrowth on the same voxel" ([Wake of the Water](https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/1815580768392038)) (**Keen**). Fill is coarse: water is "quantized to only 16 fill states per voxel" (**Keen**, water talk).
- **Terrain and building are different classes.** GI hits are sorted into "terrain hits, sky hits and building hits because these are three different shaders" (**Keen**, GI talk). Terrain is meshed smooth and displaced ("Geometric differences: terrain displacement maps", water talk slide 49); **building voxels are not**: the art director, asked about curved walls, said "we've recently made experiments to have voxels that look like building materials but are voxeled the same way as terrain materials (so they can be more organic) but so far it has not been that easy" (**Keen**, [AMA recap](https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/1786409468884723)). So shipped building blocks are angular cubes with a building material's look, not a smoothed isosurface.
- **Props are not voxels.** Doors and the like are "blocking props … thinner than a voxel, placed freely" (**Keen**, water talk slide 49); double doors were realigned "to better fit into the voxel grid" ([Hollow Halls](https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/5686430301736314574)). Blocks turn only in quarter turns; decorative props turn freely ([Pact of the Flame](https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/1789580505469416)) (**Keen**).

### How a material becomes geometry

Keen's own words: blueprints "are simply a collection of voxels that can be placed at once, but as they are voxels they will blend together seamlessly, however you stick them together" (**Keen**, [Building & Terraforming video](https://www.youtube.com/watch?v=ot0YWj848jo)). Level designers stamp any mesh into the voxels and "no matter what geometry we add or how the objects overlap the result will always be a clean voxel mesh" ([world-building video](https://www.youtube.com/watch?v=N0qrbfkcLEg)).

What the blocks look like (**seen**, the wiki galleries of [half-timbered](https://enshrouded.wiki.gg/images/Half-TimberedGallery.png), [regular stone](https://enshrouded.wiki.gg/images/RegularStoneGallery.png), [fancy stone](https://enshrouded.wiki.gg/images/FancyStoneGallery.png), [conifer](https://enshrouded.wiki.gg/images/ConniferGallery.png), [rough wood](https://enshrouded.wiki.gg/images/RoughWoodGallery.png), [plaited](https://enshrouded.wiki.gg/images/PlaitedGallery.png), [brick](https://enshrouded.wiki.gg/images/BrickGallery.png), [half-brick](https://enshrouded.wiki.gg/images/Half-BrickGallery.png), [castle wall](https://enshrouded.wiki.gg/images/CastleWall.png)): **each block material is a small architectural kit, and which part of it a cube shows is decided by the cube's neighbours.**

| Where a cube sits | What it shows (seen) |
|---|---|
| A face inside a mass of the same material | Nothing: faces between cubes are gone, and the face fill runs on across cubes with no seam |
| An outside face | The material's face fill: plaster, ashlar, brick, wattle, poles, laid in world space so the courses and joints continue from cube to cube; stones and bricks stand proud at silhouettes, so the fill has real relief |
| A vertical outside edge | A member down it: a timber post (half-timbered, conifer, plaited), a pole with lashed ends (rough wood), quoin-like stones (stone) |
| A horizontal outside edge at the top | A cap that overhangs: a cornice with corbels (half-timbered, fancy stone), a coping (stone, brick), a top plate with log ends (rough wood) |
| A horizontal outside edge at the foot | A plinth or sill beam, stepping out |
| The top row of a wall, the bottom row | Their own band: fancy stone's foot row is a panelled wainscot, its top row a frieze over plaster |
| A top face | The material's floor: planks, flags, woven mat |
| An underside | The material's ceiling: coffers, joists, a vaulted pattern |
| The edges of a hole | The same edge rule, so a carved window gets jamb posts, a lintel and a sill; in stone, the stones round its head make it read as an arch |
| A stair step | Each step a small slab with the material's edge trims |

Two more things show in every gallery: **trims leave the cube** (cornices, plinths and log ends stand several centimetres past the voxel, with no collision there), and **faces in panels vary by place**: diagonal braces in some half-timbered and conifer panels and not others, posts at intervals along a plaited wall, probably decided by a hash of the place.

Players describe the same rule from the outside (**players**): "every block has different versions depending on how its built and what it connects to"; textured blocks lose "the nice edge texture" once they touch another; some blocks carry "3d ornaments" without collision ([Steam](https://steamcommunity.com/app/1203620/discussions/0/4302697419062883699/)); a window comes out with "those stones on the top that will make the edges somewhat round" or square depending on where it sits on the grid ([Steam](https://steamcommunity.com/app/1203620/discussions/0/4339851902697489714)); the game blends in six directions and diagonal blending "would require new artwork for each direction" ([feature board](https://enshrouded.featureupvote.com/suggestions/524025/more-building-blocks-of-different-shapes-and-angles), as quoted by search, the page being behind a bot check); a doorway's lintel looks different facing north or east than south or west, and a floor slab's underside arch runs one way only ([feature board](https://enshrouded.featureupvote.com/suggestions/612370/block-texture-rotation)); wood blocks' edges look "rough" ([Steam](https://steamcommunity.com/app/1203620/discussions/0/568165241142344164/)).

Put together (inference): **a building voxel's mesh is its material's face fill on its exposed faces, plus authored pieces on its exposed edges and corners and per-row bands, chosen from its six face neighbours (and the edge and corner cases they imply), with a few variants hashed from its position.** The lintel that differs by facing and the one-way arch suggest the rule looks at the neighbours in a fixed axis order rather than symmetrically. Whether the face relief is a displaced mesh or modelled tiles cannot be told from images; the silhouettes have it, so it is geometry, not only a normal map.

**Rough and refined are separate materials** (**Keen**, the [wiki's block list](https://enshrouded.wiki.gg/wiki/Blocks)): Rough Stone, Roughly Cut Stone, Refined Stone, Polished Granite, Rough Wood, Refined Wood and so on are different items, each with its own fill and edge set (**seen**: rough ones have irregular stones and ragged edges, refined ones crisp ashlar and mouldings). "Rough" is a kit, not a state; decay and overgrowth are the per-voxel states.

### Blending between materials, against terrain, carved and part-built

- **Between materials** each keeps its own fill and the edge between them is treated as joined for at least some materials (players: the edge texture goes when another textured block touches). Mixed walls in the store shots (a log wall on a fieldstone plinth, [screenshot](https://store.steampowered.com/app/1203620/)) read as one building because both sides' trims meet at the same line (**seen**).
- **Against terrain**: Keen worked on it: "Improved the visuals at locations where terrain voxels and building voxels intersect" (**Keen**, [Souls of the Frozen Frontier](https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/6146943657174062772)). In screenshots the foot of a wall sits in the terrain's grass, which covers the joint (**seen**).
- **Carved**: a hole's edges are outside edges, so they get the edge set: "cut a hole out of a wall, and you'll see a frame appear around your new window" ([EIP on the building trailer](https://eip.gg/news/enshrouded-trailer-shows-off-building-mechanics/)).
- **Part-built** does not exist: a block is there or not, and every state looks finished, because the look is recomputed from whatever is there. Adding a cube moves the coping up a row.
- **Roofs** are their own materials (eight or so: tiles, shingles, straw, slate) placed with slope shapes (**Keen**, [wiki](https://enshrouded.wiki.gg/wiki/Roof_Materials)). A single roof block draws as a rounded mound of tiles and a slope's hips and valleys are smooth with the tiles following them; eaves get gutters and brackets, ridges caps or crests, verges boards ([tiles](https://enshrouded.wiki.gg/images/RoofTilesGallery.png), [castle roof](https://enshrouded.wiki.gg/images/CastleRoof.png)) (**seen**). So roofs appear to be the one smooth surface in building, with tile relief and edge pieces, as D181 makes ours the one exception too. Players complain walls poke through roofs ([Steam](https://steamcommunity.com/app/1203620/discussions/0/4206994473254518044/)).

### Optimisation

- A GPU-driven renderer on Vulkan, bindless only for streamed material textures and "SDF chunk data, which is the geometry of the game" (**Keen**, [Vulkan in Enshrouded, GPC 2024](https://www.youtube.com/watch?v=2LLXC9xCST4)); GI traces a simplified SDF of the voxels on every GPU rather than hardware ray tracing (GI talk).
- Building has levels of detail: "Reduced visual popping on building materials. Improved the visual details at voxel buildings on 'medium'" (**Keen**, [Forging the Path](https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/1830163047268427)); voxels are compressed on disc ([Souls of the Frozen Frontier](https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/6146943657174062772)). How the building mesh is merged per chunk is not public.

## Other games

| Game | The cell | How touching cells blend and change by context |
|---|---|---|
| **Space Engineers** (Keen Software House, no relation) | 2.5 m or 0.5 m blocks | Armour blocks are a panel model per side, the panels' UVs shifted so one large texture runs across many blocks (`PatternWidth`, `PatternHeight`); faces covered by another armour block are culled; "edge models automatically placed on some edges"; construction stages are models of their own ([deformable armour](https://spaceengineers.wiki.gg/wiki/Modding/Reference/Deformable_Armor)). Players ask to turn the edges off per block ([Keen support](https://support.keenswh.com/spaceengineers/pc/topic/let-us-toggle-edges-for-armor-blocks)). The closest documented twin of what Enshrouded seems to do. |
| **Townscaper** (Oskar Stålberg) | Irregular quads, stacked | Marching cubes with authored modules: a piece per corner configuration, made in Blender, deformed to the cell ([Game Developer](https://www.gamedeveloper.com/game-platforms/how-townscaper-works-a-story-four-games-in-the-making), [a reimplementation](https://threadreaderapp.com/thread/1261950965189672961.html)). Wave function collapse picks among candidates by priority, then decoration is added; windows are cut by stencil rather than built into modules; a new theme would take "somewhere in the region of 500 tiles". |
| **Minecraft, connected textures** (OptiFine CTM, Chisel, Create's casings) | 1 m cubes | Geometry stays a cube; the texture of each face is picked from its eight neighbours in the face's plane, 47 tiles for the full set, 5 for the compact one ([OptiFine](https://optifine.readthedocs.io/ctm.html), [MCPatcher](https://bitbucket.org/prupe/mcpatcher/wiki/CTM_Methods)). Blending by texture alone. |
| **7 Days to Die** | 1 m blocks, smooth terrain | A block is a shape (chosen from a radial menu of dozens) in a material, with a texture painted on each face from one atlas ([wiki](https://7daystodie.wiki.gg/wiki/Block), [Steam](https://steamcommunity.com/app/251570/discussions/0/3037102935230092117)). No neighbour blending: the player chooses every shape and texture. Looks blocky beside Enshrouded. |
| **Valheim** | No grid, snap points | Modelled pieces that overlap so seams hide; nothing changes by neighbour. The modular baseline: good pieces, no blending. |
| **Teardown** | 10 cm voxels per object | Not meshed: each object's volume is ray-marched, a byte a voxel indexing a palette of up to 255 materials ([Voxagon](https://blog.voxagon.se/2020/12/03/spraycan.html), [interview](https://softwareengineeringdaily.com/2025/01/02/teardown-and-voxel-based-rendering-with-dennis-gustafsson/)). Detail comes from tiny voxels, not from a cell's neighbours. Not our problem's answer at half a metre. |

Dragon Quest Builders 2 and Medieval Engineers were candidates; no technical account of their block drawing was found.

## The technique, and its names

Every builder that looks made rather than tiled does the same three things:

1. **Cull and continue.** Faces between filled cells are not drawn, and the face material is laid in world space (or UV-shifted per cell, as Space Engineers does) so it runs across cells with no seam. Hidden-face culling is what every voxel mesher does; continuity is world-space or triplanar mapping.
2. **Choose by neighbourhood.** What a cell shows is a function of which neighbours are filled, and with what. In 2D this is **auto-tiling**: 16 tiles on a **dual grid** or 47 "blob" tiles from eight neighbours, the same set Minecraft calls **connected textures** ([Boris the Brave's classification](https://www.boristhebrave.com/2021/11/14/classification-of-tilesets/)). In 3D it is the **marching cubes tile set**: a piece per configuration of the eight cells round a corner, 256 cases, 53 shapes once turned and mirrored (our [adjacency-pieces](adjacency-pieces.md)). Enshrouded and Space Engineers do a cheaper version on the cell itself: its six faces, its twelve edges and eight corners, each exposed or not. **Wave function collapse** ([Gumin](https://github.com/mxgmn/WaveFunctionCollapse)) only picks among valid candidates; with one valid piece per case, a hash of the place does it without failing.
3. **Author the pieces to blend.** The pieces are made by artists to shared profiles: every edge piece meets the face at the same inset and stands out by the same amount, so any two meet. Townscaper's ~500 are hand-modelled; Enshrouded's trims are clearly modelled per material. Fine detail that does not tile (windows, doors, furniture) is kept out of the set, as props, decals or stencils.

**Smooth voxels with material blending** (marching cubes or [Transvoxel](https://transvoxel.org/) meshes, triplanar texturing, height-based blending between materials) is the other family. It is how Enshrouded's terrain is drawn, and its roofs appear to be, but Keen says its building blocks are not, and it is what we tried as timber C and rejected.

## What it means for Terra

### What D192 got right

Its core is Enshrouded's: the cube is the truth; a piece is chosen by its neighbourhood; long members on runs; doors and windows are props fitted into holes; plans are the same cubes drawn as ghosts. Half a metre is Keen's own size. Nothing in the data model needs to change to reach the bar; the drawing does.

### Why the block-modules spike failed

The spike ([block-modules](block-modules.md)) cut Tripo's whole stone sections into eighth-cube pieces and conformed their edges. That is neither of the jobs above done right. The face was cut up, so the face fill repeated at module scale and broke wherever a shape interrupted it; and the pieces were not authored to blend, so their edges did not share a profile. The other way, B, the displaced surface, is in fact Enshrouded's first job (cull and continue) done well, which is why it won. What the spike did not try is the second and third jobs: pieces only at edges, corners and bands, authored to a profile.

### Where we are against the bar

| Kind | Cull and continue | Look by neighbourhood |
|---|---|---|
| Stone (D197) | Done: one surface, world-space stone, bevelled, displaced | Only round openings (D210's dressed stone) and caps where kinds meet. No quoins on outside corners, no plinth at the foot, no coping on top, no band at floor lines, no ceiling underneath, no wear at edges or ground |
| Logs (D200) | Done along runs | Most of the way: corners stack log ends, openings get board frames, gables cut to the roof. No top plate, no sill log on a footing, no posts at free ends or jambs as a kit choice, no weathering at the foot |
| Roofs (D209) | Done: one coursed surface over any plan | Eaves, verges, ridges, hips, valleys and flashing are already pieces chosen by the shape: Enshrouded's own method |
| Kinds meeting (D179, D210) | Each kind its own mesh | Caps and end boards; no rule for which kind's edge set wins at every joint |
| Against the ground (D206) | Footings down into it | Nothing: the wall meets the grass with a hard line |

So the gap is mostly stone and other block kinds, plus the ground and per-cube wear. Timber and roofs already work the way Enshrouded's do.

### A role for every exposed part of a cube

The concrete step is a **role resolver** in `terra_build`, a pure function of a section's cubes and its neighbours' edges, run where layouts already run. For every exposed face, edge and corner of a cube it gives a role from its neighbourhood (the 26 cells round it, with their kinds), and each kit maps roles to what to draw:

| Role, from the neighbourhood | Selects | Stone | Logs | Roof |
|---|---|---|---|---|
| Face exposed to air | Face fill, world-space, with relief | Rubble or ashlar surface (D197) | Logs along the run (D200) | Coursed covering (D209) |
| Face against another kind | Nothing drawn; the heavier kind's edge set runs past (D179's order) | Runs through | Ends flush, end board (D210) | Flashing (D209) |
| Top face open to sky | A material layer | Coping stones, flags (`Floor`) | Top plate log | Ridge cap |
| Underside over air | A material layer | Lintel stones or a vault pattern | Joists and boards (D206) | Soffit boards |
| Vertical outside edge | Module geometry, a corner piece per course | Quoins, long and short by turns | Stacked log ends (D200) or a post | Verge board |
| Horizontal outside edge at the top | A profile swept along the run, mitred at corners | Coping or cornice overhang | Plate overhang | Eave board, gutter |
| Horizontal outside edge at the foot, or on another kind | A profile swept along the run | Plinth step | Sill log | |
| Hole edges (carved or opening) | As outside edges, by side | Jambs, sill, lintel (D210) | Board frame (D200) | |
| Within a cube of the ground surface | A shader mask | Dirt and damp band, height-blended into the ground's material | The same, darker wood | |
| Outside edges, all | A shader mask from distance to the edge | Worn, lighter arrises | Worn bark | Mossy eaves |
| Place hash | A variant | Which quoin, which stone | Taper, bend (D200) | Course shift (D209) |

The four kinds of output are cheap in different ways: **material layers** and **shader masks** cost no geometry (a texture-array index and a weight per vertex, which the merged mesh carries), **swept profiles** reuse the timber generator's sweep along runs, and **corner pieces** are the only true modules, a handful per kit.

### How the pieces are authored

- **Profiles, not cut models.** A coping, cornice, plinth, sill or plate is a 2D section authored in Blender (D121's Blender exception), the kit file naming it, swept along each run the resolver gives and mitred at corners, as logs are; its material a trim texture from the library, its relief the same height displacement as the surface. A profile is a few points, so a kit can have many and a new one is data (D136).
- **Corner pieces to a shared profile.** Quoins, a cornice's corner block, a post's foot: modelled in Blender to the kit's profiles so they meet the swept runs exactly, or made by Tripo and refined (the `models` skill), then cut to the profile's box. Tripo makes the surface detail; the box and profile come from the kit. This is what the block-modules spike lacked.
- **Variants by data** (D182): per kind, a face fill, a finish (rough fieldstone, squared rubble, ashlar: Enshrouded's rough and refined, as kinds), an edge set (plain, quoined, corniced) and wear. Eight stones, three finishes and three edge sets is about 70 stone kinds a player can tell apart by colour, coursing and how their corners are dressed.

### Blending, carving and part-built

- **Between kinds**: the resolver sees kinds, so the rule D179 states (the heavier runs through) decides whose edge set is drawn at every joint, and the lighter kind's face simply stops against it, as logs already do.
- **Against the ground**: no change to the ground (D192); a dirt band masked in from each foot cube's height above the ground sample (`layout::ground` has it), and the grass system allowed to grow against the foot. Enshrouded did this late; it is the cheapest win.
- **Carving**: carved edges are outside edges, so any hole gets jambs, sill and lintel by the same rule, not only door and window holes.
- **Part-built**: the built cubes are resolved as if they were the whole building, so every stage looks finished (a half wall wears its coping until the next course goes on), as Enshrouded's do; ghosts are resolved from the plan, so they show the finished look. Freezing a layout once a piece is in (building-tech.md) still holds for timber runs; trims are cheap to redraw.

### Making it fast

The same as the plan's step 3 already says, with the roles folded in: the resolver runs per dirty section on workers; one mesh per section and material, kits' textures as arrays, so roles add no draws. Levels of detail per section: near, full relief and every piece; middle, the surface's relief flattened to its normal map and corner pieces dropped below a few pixels; far, the faces with their layers and masks only, then a box. Measured on the `village` bench on both GPUs (D185) before anything is tuned.

### The spike: candidates side by side

Step 2a's spike, on the yard, in the game's renderer and light, each candidate built into **Keen's own test piece** (the one every wiki gallery uses: a stair up to a platform, a wall with a window and a door hole, a slab overhanging it) plus our cases: an L, a T, a cross carved out, a wall half built, stone meeting logs, a wall on a slope. Each judged beside the Enshrouded gallery shot of the nearest material, for how cubes blend and how their look changes by place, not for style:

| | What | Settles |
|---|---|---|
| A | Today: the displaced surface with D210's trims | The baseline |
| B | A plus material layers and shader masks from roles: tops, undersides, foot band, edge wear, ground band | How far no new geometry gets |
| C | B plus swept profiles and corner pieces from roles: coping, plinth, quoins, string course; for logs, plate, sill, posts | Whether this reaches the bar |
| D | Dual-grid modules authored in Blender to shared profiles, not cut from Tripo, for one stone | Whether whole modules beat C, now done right |

My expectation is C for block kinds and C's log rows for timber, with D only worth building if C's corners look assembled. Enshrouded reference images were gathered for this from the wiki galleries and store screenshots; they are Keen's, so they stay out of the repository.

### D197 and D200

- **D197 (block kinds as their displaced surface): keep it, but it is half the answer.** It is Enshrouded's face fill and should stay; its "as Enshrouded seems to" should be read as that, since Keen says its building blocks are not meshed like terrain. What it lacks is the role layer above. I would recommend the developers amend it to "the surface plus pieces chosen by role", rather than replace it.
- **D200 (generated logs): keep it.** Generated logs on runs are already pieces chosen by their neighbourhood; the roles add a plate, a sill and optional posts.
- **One thing to raise, not change:** D192 says nothing leaves its cubes but eaves and footings, and log ends were pulled flush on 2026-10-12. Every Enshrouded material's trims stand a few centimetres past the cube, which is much of why they read as built. Letting trims overhang by up to an eighth of a cube, drawn only and never colliding, is a design question for the developers.
- A small technical note: Enshrouded keeps wear and overgrowth per voxel. Our 32-bit cube has no spare bits; a sparse side table per section would hold them when they come.

## Limits

- Keen has not described how building blocks are meshed. The method above is inferred from images and players' reports, consistent with what Keen has said; the face relief's nature (displaced mesh or modelled tiles) and how variants are hashed are unknown.
- The galleries show each material alone in one lighting; mixed materials and ground contact were seen only in a few store screenshots. No footage was examined frame by frame.
- Nothing here was built or measured; the spike decides.
