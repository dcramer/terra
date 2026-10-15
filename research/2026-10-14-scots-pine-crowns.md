# Scots pine crowns that read from the ground, and the lush forest as a tall conifer stand

2026-10-14. Questions: the developers chose (D301) a tall, open conifer stand as Sons of the Forest's for the lush forest, Scots pines and spruces with crowns far overhead and oaks the exception. Now that pines are most of the forest, the trees pass's open item matters most: "the pine crowns are still broadleaf-looking clumps on spoke whorls" (review of D309), and a needle-tuft card tried then "read as lobed broadleaf leaves from 15 m". How do shipped games build a Scots pine's crown, and what makes ours read as one from the ground at 15 to 30 m?

## Answer

A Scots pine's crown reads as a pine from below by its **masses**, not its needles: dark, domed pads of foliage at the ends of a few crooked limbs, with sky between the pads, their edges a fringe of needle tufts. At 15 to 30 m a card a metre across is 40 to 80 pixels, so no needle resolves; what reads is the silhouette of each mass and of the cards at its edge. Ours read as broadleaves for three causes, each fixed (D318):

1. **Spokes in tiers.** The crown was whorls of four to six straight branches at one height, a year apart, as a sapling's are. An old pine has lost its whorls: a dozen or more crooked limbs leave the bole at heights of their own. Now 14 to 23 limbs a crown, spiralled, low ones near level and long, high ones climbing, each forking toward its end.
2. **Clumps strung along spokes.** Clumps every 0.7 m along each branch's outer two thirds made a row of lobes on a bare stick. Now each limb end, fork and a point on a long limb's outer half carries a **pad**: six to nine short shoots crowded about the end, fanned ahead and to the sides and turned up, some steeply, a tuft at each tip, so the tufts lie as one domed cushion a metre or two across. A first try with four to seven long shoots fanned flat read as a palmate leaf, lobes on stalks; crowded short shoots read as one mass.
3. **A card whose outline is a leaf.** The old card (a generated picture of a bough laid flat) and the feather-shaped needle cards first tried here are long fronds of side shoots: their outline at 50 pixels is a notched leaf, and several of them a lobed broadleaf. The card is now a tuft drawn needle by needle (`textures.ts cards`, `scots.leaves.json`): a short stalk with twelve shoots fanned from its top, bold needles 12 to 17% of the card long and leaning 0.3 to 1.0 radians off their shoot, as wide as it is long, so its outline is a spiky round fringe. Three or four cards a tuft are turned round the shoot a third of a turn apart, as SpeedTree's needle clusters are planes rotated about their shoot, so a tuft has body from below as from the side.

Colour and light follow the masses: needles dark grey-green (the card's leaf average linear 0.015, 0.031, 0.018, half a spruce's darkness above it), each pad shaded from its own middle as an oak's masses are (its top lit, its underside dark), and the needles lit through at a third of a broadleaf's strength, as needles are thick and waxy: lit through as leaves, a crown against the sun glowed lime.

## How shipped games build them

- **SpeedTree** builds conifers as a branch hierarchy with needles placed as *clusters* (a custom mesh of several planes rotated about the shoot, with normals edited to read as a volume) or as *fronds* (cards along the branch ends); the Longleaf pine breakdown on SpeedTree's site builds its needle bush from several rotated planes, "duplicated, rotated, and with vertex noise", its pivot in the bush (C, via search; the page refused a fetch). Polycount threads on conifers in UE and CryEngine agree that single billboards fail for needles, that a Scots pine is harder than a spruce, and that its masses must bend up toward their ends (C).
- **Drawing practice** says the same for a different reason: draw a pine's masses' dark undersides and let their lit tops read as sunlit, never single needles (C).
- **Sons of the Forest**: nothing public on how its trees are authored (see [2026-10-14-sotf-forest-look.md](2026-10-14-sotf-forest-look.md)); its frames show tall straight boles in rows, crowns far overhead in dark masses with sky in slits.

## Measured

2026-10-15, at 1920 by 1080 on the RTX 3080, against the oak wood with pine groves it replaces (main before D318, its frames measured the same way).

**The look at 13:00**, the vantage sheet's four forest views against Sons of the Forest's daytime frames (`sotf-forest-day`), the oak wood, then the stand:

| | Sons of the Forest | oak wood | pine stand |
|---|---|---|---|
| mean | 0.148 | 0.120 | 0.181 |
| below 0.08 | 46% | 54% | 30% |
| contrast | 0.426 | 0.464 | 0.471 |
| shade blue over green | 0.377 | 0.296 | 0.396 |
| shade red over green | 1.261 | 0.796 | 1.039 |
| lime at the top | 2.0% | 7.4% | 2.1% |
| trunks across the middle | 48.7% | 12.6% | 54.1% |

The stand reads as one: trunks across the middle and the lime of backlit leaves at the top are now Sons of the Forest's, and its shade's colour near it. It is brighter than the reference (0.181 against 0.148, a third fewer pixels near black): the high crowns let light down, as D301 asked, but further than a reference frame's.

**Sky seen looking up** from eye height at twelve places: a mean of 13.6% (median 13.9%, places from 0 to 42%), under D245's 15%; the oak wood showed 2.8% (median 1.3%). A pine's pads hide 72 to 85% of the sky straight up through them, projected from below.

**The undergrowth** hides 19.8% of the horizon band within 20 m at eye height (median 19.0%), under D268's aim of 30 to 60%: fewer broadleaf saplings on needles thin the middle layer.

**Its crowns**: a grown pine's near crown is 9,400 to 17,500 triangles (its whorls were 4,900 to 10,000), one card a tuft far off 2,700 to 5,000; its shadow stand-in about twice the whorls'.

**Frame time**, the walk bench (`cargo xtask bench walk`'s scene), two runs of each alternated on a machine other agents were building on (load 14 to 36 on 24 cores):

| | median | 99th percentile | worst | hitches | GPU, median |
|---|---|---|---|---|---|
| oak wood | 13.0 to 13.7 ms | 37 to 49 ms | 50 to 78 ms | 59 to 521 | 9.9 ms |
| pine stand | 12.8 to 12.9 ms | 15.4 to 15.5 ms | 17 to 29 ms | 0 to 2 | 10.1 ms |

On the GPU the stand costs 0.2 ms more: volumetric fog 0.27 ms more (2.02 against 1.75), its shadow lookups spread along the stand's longer sightlines between bare boles; the main pass the same and the prepass 0.17 ms less, as a conifer's cards drop out by 70 m (`NEEDLES_DROP_M`). Drawn whole out to 110 m as a broadleaf's, the pads' cards were 8.37 million triangles a pass against 7.10 and half a millisecond more, for no difference the look measures (mean 0.184 against 0.183). Neither holds D25's 13.3 ms at the 99th percentile: the frame is the GPU's 10 ms and some 2.5 ms more of render work in series, the same in both.

**The hitches** first seen with the stand (30 to 60 ms frames, 6 in 2,400) were the machine's, not the trees': the slow frames are the game's own work on the CPU rising from 4 ms to 20 to 35 ms while the GPU holds at 11, every system in the frame slower at once (a chrome trace of a slow stretch), at different frames in each run of the same build; on a quiet machine the stand ran 2,400 frames with none (worst 16.4 ms), and the oak wood, with sixteen busy threads beside it, had 337. A busy machine also outlasted the loading screen's 60 s in the play experiment, which then started with the pines' bark not yet ready and drew their crowns over no trunks, so a film made then is not of the stand.

## Limits

- Judged at 1920 by 1080 on the vantage views, films and still views of single pines at 15 to 30 m; near views of a crown are rare in a stand this tall.
- The card's needles are 12 to 17% of a tuft's card, two to three times a Scots pine's at a tuft's size; at the distance a crown is seen from, no needle resolves.

## Sources

- SpeedTree, "Building Pine Flatwoods in SpeedTree": <https://store.speedtree.com/?p=744562>
- 80.lv, "Studying Pine Creation with SpeedTree and Unreal Engine 5": <https://80.lv/articles/studying-pine-creation-with-speedtree-unreal-engine-5>
- 80.lv, "Setting up a pine branch card in Blender geometry nodes for SpeedTree": <https://origin.80.lv/articles/setting-up-a-pine-branch-card-in-blender-geometry-nodes-for-speedtree>
- Polycount, foliage card normals and conifer needles: <https://polycount.com/discussion/comment/1694927>, <https://polycount.com/discussion/comment/2762820>
