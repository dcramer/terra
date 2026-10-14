# Carrying a forest to the horizon

2026-10-14. Question, from review #3 of the lush forest: from the overlook the horizon was a dark slab (sky 0.42 above the horizon, 0.24 under it, a hard line) and the forest an island on a flat brown plane. How do shipped open worlds draw the forest past the trees they really draw, and what was the slab?

## What the slab was

Measured with each effect taken out in turn (`--quality low` drops the volumetric haze; the rest over the remote protocol):

- **The planet is small.** On the 2 km stand-in the ground curves away 2.4° under the horizontal from eye height and 12° from the overlook, 45 m up; the air is Earth's seen through a lens (`air`, D212), whose own horizon dips 0.6° and 1.1°. Between the two, every view showed Bevy's atmosphere's *ground*: haze over a dark albedo, a flat band that ended at a hard line. Nothing standing on our ground can fill it: past about 420 m from the overlook the ground is under its horizon, so a 30 m tree there shows only to 770 m.
- **The mist rose with the eye.** The volumetric haze's box followed the camera, its foot 10 m under it, so its mist (seven times the haze, its lowest dozen metres) hung at eye height from the overlook. Looked through edge-on it darkened the band further, most in the middle of the view, since Bevy's volumetric fog gives it little light from away from the sun.
- **The forest's haze.** The distance fog (D224) is 240 m of shaded green air, right under the canopy and wrong in the open: everything past the forest faded to 0.2.

## How shipped games do it

- **Billboards, then less.** SpeedTree crossfades a tree into a 360° billboard lit as the tree was ([SDK docs](https://docs.unity3d.com/speedtree-runtime-sdk/manual/level-of-detail.html)); Unity's terrain trees do the same past a distance ([manual](https://docs.unity3d.com/Manual/terrain-Tree-LOD.html)). Horizon Zero Dawn's last levels are crossed planes ([Sanders, GDC 2018](https://gdcvault.com/play/1025066/Between-Tech-and-Art-The), from a secondhand summary). Octahedral impostors (Brucks, [shaderbits](https://www.shaderbits.com/blog/octahedral-impostors)) bake a tree from many angles into one atlas, for views from above too. Far trees are batched: drawn merged or instanced by the thousand, never one draw each, and they cast no shadows.
- **Past the trees, terrain and backdrop.** Open worlds draw their farthest land as low-detail terrain with its forest baked into its colour (Far Cry's terrain, [Moore, GDC 2018](https://gdcvault.com/play/1025261/Terrain-Rendering-in-Far-Cry)), and past the map's edge as backdrop: hills painted into the sky or set as a ring of silhouettes, paler with distance.
- **Aerial perspective does the fading.** Far land takes the sky's colour at the horizon, so it fades into the sky rather than into a band of fog's own colour (D224's note on The Witcher 3).

## What we took (D273)

- **Far trees as merged billboards** (`lush/far.rs`, `far_forest.wesl`): 48,000 cards from the meadow's edge out to 1 km, placed by the near forest's own grove and glade rules, merged 160 m a square into 144 draws, no shadows. Each card turns to the eye about its upright, and the shader draws its tree, an oak's lobed crown, a pine's high ragged one or a spruce's cone of whorls, with a belt of shrubs at its foot, rounded and lit by Bevy's lighting, coloured from the near crowns' own leaf pictures. Drawn from the shape rather than baked from the tree, so a new kind is a few lines of shader; an octahedral bake of the real trees would match them closer and is the next step if they are looked at from nearer.
- **Far land in the sky** (`sky`): below the horizon, ridges one behind another between the planet's horizon and the air's, each paler in the air's own colour just over the horizon, which the sky shader reads from a small cube of the atmosphere Bevy works out each frame. It stands for land the stand-in planet is too small to have; on a world of D13's size the ground's horizon and the air's nearly meet at eye height.
- **The haze follows how open the sky is over the eye** (`sky_occlusion`, `daylight`): under the canopy the forest's 240 m of green air, out of it 2.5 km of air in the horizon's colour, so the far forest and land show and fade toward the sky.
- **The mist lies on the ground** under the eye, wherever the eye is.

Measured from the vantage sheet's overlook at 13:00 (rows' grey over the middle of the view): above the horizon 0.42 and under it 0.24 before; 0.67 and 0.62 after, the ridges stepping down to 0.52 at the forest's edge; at 17:00, 0.67 and 0.64. The sky above is brighter too, as the mist no longer stands between the eye and it. The far trees add 0.06 million triangles to the walk bench's frame (13.54 to 13.60) and nothing measurable to its GPU time on a contended RTX 3080.

Gentle hills: the lush world's ground could be given swells larger than D242's 6 m, but on the 2 km planet ground past 420 m is under the overlook's horizon, so hills there would have to stand a hundred metres to show at all; they are the far land's ridges for now, and real terrain once the world is its full size.
