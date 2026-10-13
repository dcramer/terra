# Distant trees without pops, the mood of a forest, and how The Forest and ARK make theirs

2026-10-13. Questions, from the developers after watching the lush forest walk: how does The Witcher 3 change its trees' detail in the distance without it showing; what makes a forest feel like being inside one, beyond darker bark; and, after hearing that SpeedTree is "kinda meh", what do The Forest, Sons of the Forest and ARK do instead?

Sources are marked **P** (the developers' own talks, notes, documentation, job posts) or **C** (press, community, reverse engineering, inference). Much of the primary material (CDPR's GDC 2014 and SIGGRAPH 2015 talks, Guerrilla's 2018 vegetation talk, Rockstar's 2019 atmosphere slides) is behind GDC Vault or too large to fetch; abstracts were used.

## The Witcher 3's distant trees

- **SpeedTree, deeply integrated** (P, [SpeedTree](https://store.speedtree.com/?p=2530)): about 20 tree and bush species with several models each, 40 grasses, about 200 plant kinds in all.
- **SpeedTree's LOD is continuous, not swapped** (P, [SDK docs](https://docs.unity3d.com/speedtree-runtime-sdk/manual/level-of-detail.html)): outer branches shrink to their centre line, leaf cards drop out by shrinking while the cards that stay grow, wind fades, and at the end the tree crossfades into a 360° billboard lit like it. This is what makes a SpeedTree game's trees thin rather than pop.
- **Distances per asset, times global scales** (P, [NVIDIA's guide](https://www.nvidia.com/en-us/geforce/news/the-witcher-3-wild-hunt-graphics-performance-and-tweaking-guide/)): `FoliageDistanceScale`, `GrassDistanceScale`, `FoliageShadowDistanceScale`.
- **Distance flattened before detail changes** (C, reverse engineered, [astralcode](https://astralcode.blogspot.com/2019/06/reverse-engineering-rendering-of.html)): its fog tints the far scene toward the aerial colour by luminance, desaturating it while keeping its shading, and fades ambient occlusion with distance; a far tree changes detail in air that has already flattened it.
- **Darker, moodier image** (C, reverse engineered): a Hable tone curve, auto exposure clamped so dark forests are not brightened back to mid-grey, low-influence colour LUTs, and a near-black vignette.

## What makes being in a forest feel real

- **Enclosure:** little sky (under a closed canopy, 7 to 9% of full sun at eye height), sightlines that end in trunks and haze within tens of metres, and a layer at eye height (saplings, shrubs 1 to 3 m) so the frame is never a lawn under columns.
- **Light:** dark understorey, sun flecks a few stops brighter, backlit leaves glowing, green-tinted fill where light has come through leaves. Ghost of Tsushima lets artists push lighting from physically correct (P, [Patry, SIGGRAPH 2021](https://www.advances.realtimerendering.com/s2021/jpatry_advances2021/index.html)).
- **Air:** haze that varies with height and drifts, motes and falling leaves in the shafts. Ghost of Tsushima's froxel haze falls off with height; RDR2's sky, fog and volumetrics are one system (P, [2019](https://advances.realtimerendering.com/s2019/index.htm)).
- **Motion:** one wind for everything, gusts travelling across the grass (P, [Ghost of Tsushima](https://gdcvault.com/play/1027124/Blowing-from-the-West-Simulating)).
- **Sound:** placed in the world rather than panned; for a small team likely the strongest cue of being inside for its cost.

## The Forest, Sons of the Forest, ARK

- **Endnight** has not said what it authors trees with. Both games use mesh LODs then billboards per species, and patch notes keep fixing billboard and tree popping; Sons of the Forest added "far distance stippling", a dithered fade (P, patch notes). A felled tree is swapped for cut pieces matched to it (P).
- **ARK** uses SpeedTree with ZBrush and Maya (P, artist portfolio and job ad); its trees are foliage instances carrying a harvest component, swapped for a falling physics actor when cut; distant trees are culled, and pop-in is one of its known faults (C). ARK: Survival Ascended moves foliage to Nanite (P, interview).
- **Why SpeedTree is called meh:** studios ship its library and presets unchanged so trees look alike, and untuned LODs pop (its own docs say so). The tool itself does continuous LOD well; the Witcher 3 is its showcase.

## What we took

- **Cards that drop out** (D223): our crowns now thin card by card with distance, survivors growing, as SpeedTree's do, instead of swapping between versions.
- **Haze between the trees** (D224): a forest-scale distance haze, its colour following the hour.
- **A darker, more muted scene** (D216).

Still to take, roughly in order of effect per cost: sound in the world; an eye-height layer of saplings and shrubs; haze that varies with height and drifts; one travelling wind; the same continuous thinning for trunks and branches; billboards or impostors for the far forest; shadows chosen per cascade (Horizon Zero Dawn).
