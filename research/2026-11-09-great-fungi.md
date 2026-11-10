# Great fungi: what a cap, its gills and its stalk are, and how ours was flat

2026-11-09, for the far realm's cap tree (docs/realm-two.md, D642; `terra_world::grown::caps`, `assets/shaders/cap_skin.wesl`). The developers on the realm: "its not lush, it doesnt look that good", "mostly just a shitty version of a scifi realm right now". Its forest is the cap tree, and on a still it read as a placeholder: a lathe's disc of one lilac, ruled stripes for gills, a stalk in tree bark. This note is what was read before changing it, and where ours measured flat.

Grades: [P] primary, read now; [S] secondary, read now; [C] community; [M] from memory, nothing rests on it.

## What a real one is

- A cap's shape is its age: convex caps "continue to expand as they mature until they become flat"; campanulate is a bell, infundibuliform a funnel, an umbo a "knobby protrusion at the center" ([Pileus](https://en.wikipedia.org/wiki/Pileus_(mycology)) [S]). The fly agaric goes "globose to hemispherical and finally to plate-like", flat or slightly concave when old ([Amanita muscaria](https://en.wikipedia.org/wiki/Amanita_muscaria) [S]; [First Nature](https://www.first-nature.com/fungi/amanita-muscaria.php) [S]). The parasol is egg-shaped, then flat with a smooth dark umbo, its stalk at full length before its cap opens ([Macrolepiota procera](https://en.wikipedia.org/wiki/Macrolepiota_procera) [S]).
- Warts are the scraps of the veil the button broke through: they stay the size they were as the cap widens, so they spread apart, and rain and age shed them and fade the cap (Amanita muscaria, as above [S]). The parasol's scales lie thickest by its darker middle ([First Nature](https://www.first-nature.com/fungi/macrolepiota-procera.php) [S]).
- A margin is striate, "minute radiating lines or ridges" ([Glossary of mycology](https://en.wikipedia.org/wiki/Glossary_of_mycology) [S]), "sometimes somewhat lined" on the fly agaric ([MushroomExpert, A. muscaria var. flavivolvata](https://www.mushroomexpert.com/amanita_muscaria_flavivolvata.html) [P], the author's own collections). Hygrophanous caps change colour as they dry, often two-toned; viscid is thinly slimy; zonate is in concentric zones ([MushroomExpert glossary](https://www.mushroomexpert.com/glossary.html) [P]).
- Gills are not ruled lines of one length. Short gills (lamellulae) are set between the long toward the rim: one between each pair, or one reaching halfway and two shorter either side of it ([Mushroom, the Journal: lamellulae](https://mushroomthejournal.com/greatlakesdata/Terms/21391.html) [S]); on the fly agaric "short-gills frequent", the gills close or crowded and free of the stalk (MushroomExpert, as above [P]).
- A stalk is fibrous and "ragged"; the parasol's is banded in brown scales, a snakeskin, torn apart as it lengthened; a ring is a skirt, pendant, grooved and flaccid with age, and the foot is a bulb (First Nature, both pages [S]; [Annulus](https://en.wikipedia.org/wiki/Annulus_(mycology)) [S]).

## How games and engines draw one

- No developer's account of a shipped game's giant mushrooms was reached (Morrowind, Zangarmarsh, Grounded, Avatar, No Man's Sky: searches only; ArtStation and UESP refused). What follows is engine practice, not a game's.
- Thin surfaces lit through: Unreal's Two Sided Foliage model transmits light through a thin surface by a subsurface colour, and its plain Subsurface is "not as accurate for thin surfaces" ([Shading models](https://dev.epicgames.com/documentation/en-us/unreal-engine/shading-models-in-unreal-engine) [P]). Not done here: a cap's rim lit through from above is the next thing a picture would gain.
- Relief without a normal map: a procedural height is turned into a normal from its and the position's screen derivatives; Unity's Normal From Height node is this, its generated code read ([Shader Graph: Normal From Height](https://docs.unity3d.com/Packages/com.unity.shadergraph@17.0/manual/Normal-From-Height-Node.html) [P]); it is Mikkelsen's "Bump Mapping Unparametrized Surfaces on the GPU", 2010 [M]. To be ported as the skin's relief.
- Warts as cellular noise: one jittered point a grid cell, the nine cells round searched ([The Book of Shaders, 12](https://thebookofshaders.com/12/) [S]; the technique only, no code taken).
- A hobby generator builds caps, stems and gills from Bezier profiles in a few size presets, clustered by Poisson-disk placement and an attractor ([procedural fungi](https://benjamin.ahlbrand.me/post/2021-11-27-procedural-fungi-generation/) [P], its author's). Ours keeps a profile by rule and adds age to it.

## Where ours was flat (measured)

`target/job` stills of the nearest cap tree of seed 3 at 13:00, high preset, 1920x1080; crops by hand; sRGB 0 to 1. `sd` is the standard deviation of luma in the crop, `fine` the mean difference of pixels two apart, `sat sd` the spread of saturation.

| Crop | luma sd | fine | sat | sat sd |
|---|---|---|---|---|
| Gills from 4 m | 0.059 | 0.012 | 0.75 | 0.053 |
| Gills from 26 m | 0.058 | 0.048 | 0.72 | 0.046 |
| Top from 26 m | 0.139 | 0.040 | 0.61 | 0.140 |
| Top from 100 m | 0.055 | 0.029 | 0.37 | 0.034 |
| A young cap's top from 5 m | 0.130 | 0.011 | 0.44 | 0.121 |
| Stalk from 4 m (tree bark) | 0.080 | 0.054 | 0.43 | 0.203 |

The gills were the flattest thing in the picture and the most saturated (0.75, a fill no real material is: the guide's rule 6); a top's spread was its pale blobs against one fill, with nothing fine in it near (0.011); the stalk was textured, but as an oak. By night the whole underside shone as a lamp's shade does. And every cap was one outline: a circle, the same dome at every age.

## What ours is to depart in

A cap tree is 8 to 25 m, forty times a parasol: its warts would be a hand to a forearm across, not a fingernail, and its gills a hand apart at the rim, so both read from the ground. Its colours are its realm's (plum over blue gills that shine), not any species'. Its gills are to be drawn in the skin with relief, not as blades of mesh: 96 long gills doubling twice would be 400 cards a cap. None of this is built on main yet (far-caps j1 ended with the new shape unlanded).
