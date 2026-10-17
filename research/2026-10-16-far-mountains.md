# Far mountains: what made the wall behind the castle regular (2026-10-16)

Question: the developers, watching the castle-and-dragon film: "the mountain is tooo regular in the back, it needs to be fixed up". What draws the huge quilted zigzag across the range's wall north of the valley, its uniform ridges and its even snow band, and how do shipped open worlds give far mountains their structure?

## How shipped games do far mountains

- **The shape is eroded, offline, at many scales.** Open worlds' mountains are heightfields run through hydraulic (flow) and thermal erosion in World Machine, Gaea or Houdini: flow cuts branching gullies and valleys at every scale and carries the spoil down; thermal erosion breaks cliffs and piles talus under them ([World Machine's features](https://www.world-machine.com/features.php)). Their flow maps then mask the materials: rock strata and cracks on the cut faces, sediment in the beds ([a Terragen and World Machine workflow](https://planetside.co.uk/forums/index.php?msg=148438)). Sucker Punch's Tsushima was built with in-engine tools whose texture and placement rules artists write in a small language run on the GPU (Pohlmann, [Samurai Landscapes](https://gdcvault.com/play/1027352/Samurai-Landscapes-Building-and-Rendering), GDC 2021); its distant ranges were sculpted vistas. How Horizon's and RDR2's far ranges were built is not public in detail.
- **Far off, the detail is in the shading, not the mesh.** A far LOD's mesh is coarse; its erosion detail is a normal map baked from the full-resolution heightfield, and its materials come from slope, height, facing and flow, so a wall kilometres off still shows ribs, gullies and snow lying in them.
- **The air layers the ranges.** Aerial perspective (Bevy's Hillaire atmosphere here, D212, D334) pales and blues each farther range.

Our ground is generated per point (D322): the ranges' ridged multifractal, damped by its own slope (Quilez), with Runevision's per-point erosion filter cutting gullies down the fall line in three sets 140, 59 and 25 m apart, and couloirs on the walls. Far chunks are meshed 16 to 32 m a cell at 3 to 6 km, so only the widest gullies stand in the mesh; the ground shader adds the rest in the light (D322's ribs).

## What was wrong

1. **The quilt is the far shader's ribs** (`ground.wesl`). Their phase was a sine of `dot(world, across_face)`: the distance across the fall measured from the planet's centre, 30 km off, along a direction taken from each fragment's normal. A degree's turn of the normal between two triangles moved it by 520 m, six to fourteen of the ribs' wavelengths, and the noise that made them wander was read at that distance too, so every triangle of the far mesh drew its own set: chevrons in a lattice of the mesh across every wall. Fixed by laying the ribs as the ground's own gullies are laid, cell by cell (`fall_stripes`, a port of `Range::stripes`): each cell's point is near, so a turn of the fall moves a stripe by a little. Stills before and after below.
2. **The ridges' warp stepped** (`mountains.rs`). Its three noises were shifted 101, 203 and 307 cells by adding to the point, which a single then holds only to 3e-5 of a cell: the warp stood still for 0.6 m and jumped, stepping the ground by up to 0.15 m at each, and the gullies' depths, which follow the slope measured over 40 m, made some steps half a metre. Whole cells are now shifted apart from the point (`Noise::sample_shifted`); the ground is otherwise the same (the lake's site, level and size unchanged).
3. **The wall is one octave's flank.** The field maps (`cargo xtask fields --at 2.2,4.6 --size 4000 --cell 8`) show the range north of the valley as a single wall 10 km long, 2 km high, at 45 to 60 degrees, its contours smooth parallel arcs: the first octave's ridge, its finer octaves damped away where it is steep and its upper part stood forward by the walls (D322), with only the 140 m gullies' comb on it.

## Tried on the ground, and set aside

Each measured on the field maps, the lake's site query (`--site lake`) and the town's:

- **Rock bands as terraces** (a smooth step through every 70 m of height on the walls): terraced the low spurs in rings like a contour map, and the ledges with gullies cut across them held ponds, 2.3% of the wall's region under water against 0.5%.
- **Side valleys** cut down the flanks by the same stripes at 1.9 km, 800 m and 340 m, 200 m deep: the flanks took spurs and valleys, but every rule that cut deep enough left pits that filled as tarns, and the walls and valleys changed which lake fits the lake's query best (it scores the walls looming over a lake): the lake's site moved from -15.34 km east, 1.68 km north (1,317 m, 0.55 km²) to other lakes 4 to 5 km off, which would take the lake's trailer run with it.
- **Spurs** (the range's edge shifted out along its ridges and in along its valleys): lowered the lake's outlet by 10 m.

The town's site stayed put under all three. Carving the ranges' flanks at the scales between the main ridges and the gullies is the change that would most make the walls real mountains, as the eroded heightfields of shipped games are; it needs the demo places pinned to where they are (or found again and their runs moved), which is the developers' and the demos' call, not a side effect of the far look.

## Results

Rendered on 2026-10-17 with main as it was (2b746903) and with the patch, the castle valley at 06:48 on 10 June from the meadow, up the wall and from 40 m over the forest, and the lake at 10:06: stills and a before-and-after sheet in `~/.cache/terra-review/mountain4/` (`sheet-before-after.jpg`, `crop-v490.jpg`; not in the repository). An adversarial review against eight Tatra and Alps photographs scored the far wall 3/10 before and 3.5/10 after:

- **Gone:** the chevron quilt over the whole face, the crenellated sawtooth crest and the straight snow stripe across the wall.
- **Still wrong:** the snow cap reads as an egg-crate of even, rounded blobs, snow on their lit tops rather than lying in the couloirs; the three stripe sets read as even bumps above and a fine streaking that fades to haze below, not as a few deep gullies running long down the face; dark bands at one height still cross the mid-wall; the lake's far slopes are nearly unchanged.
- **The main cause now is the shape:** one smooth curtain of a wall with no peaks, cols, buttresses, cliff bands or scree aprons.

**Front rules.** The patch also has the spurs and crest peaks as theme numbers, left at 0. Turned on (spurs 1, peaks 0.35), `cargo xtask fields --site` found no place within 200 m of three of the four pins: the town and settlement (2.22, 2.12 and 2.07, 2.56) and the lake (-15.34, 1.68). Only the tundra (-15.54, 10.52) stayed. They stay off until the front rules are faded round the pinned sites.

## Second round (D395)

- **Why the egg-crate.** A vantage still with the shader's couloirs painted (and one with its ribs off) showed the shader self-consistent, snow in its own gullies, but those were a second set 150 m apart from other cells than the mesh's 140 m gullies; both broke off within a gully's width (a cell a gully), so they made round bumps, and with the ribs off the mesh held only gentle relief under an even cap. Fixed by cells spanning 2.5 gullies (`gully_stretch`), the shader drawing a set only past where the far mesh holds it, and the vertices' snow on steep ground lying in the mesh's gully beds.
- **Why the bands.** The granite's streaks were a noise of `dot(world, across_face)` and `dot(world, up)`: the first is 0 (across is level, the point is along up), the second the distance from the centre, so a noise of height alone. Now laid down the fall by `fall_stripes`.
- **Front rules, round the pins.** With the rules faded out within 700 m to 1.8 km of each place in `places.ts`, peaks at 0.35 keep all four pins, but they change only a range's outer front (0.9% of the field map round the castle's wall) and lowered the dome over the lake, so they stay off. Spurs lose the lake even faded out 3 to 5 km round it: they dam its valley far below, and it floods into a larger lake.
- **Review:** 3.5/10 before, 4.5/10 after (stills in `~/.cache/terra-review/mountain5/`). Left: the snow's tongues an even comb under a solid band; no peaks or cols on the crest, which lies inside the range where the front rules do not reach; an even corrugation of ribs; one flat grey tone; the wall's foot lost in haze.
