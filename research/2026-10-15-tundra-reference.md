# The tundra's reference: photographs of open tundra in autumn, measured

2026-10-15. Question: what should the tundra demo ([docs/demos.md](../docs/demos.md#2-a-tundra), D321) look like, measured, before anything is built: the whole frame and its bands, the colours of the ground's communities the ground and cards will be tuned against, how big things are, and what games show of composition and wind? Step 1 of the demo's order of work.

Tags as in [2026-10-15-biome-demos.md](2026-10-15-biome-demos.md): **[P]** seen in the photographs or frames themselves, or from a primary source, **[I]** inferred.

## What was collected

Everything is kept outside the repository, under `~/.cache/terra-review/refs/tundra/` (someone else's work; the repository is public).

- **Photographs, the look** (`photos/`, `sheet-photos.jpg`, credits in `photos/sources.tsv`): twenty-nine photographs of open low tundra by day in late summer and early autumn, each with a horizon: Alaska (Noatak, the Brooks Range, Kesugi Ridge, the Yukon Delta, Barrow), Svalbard, Greenland, Nunavut, the forest-tundra at Great Slave Lake, Hardangervidda and Dagalifjell, Kola, the Bolshezemelskaya tundra, Taymyr, Taz and Bennett Island. From Wikimedia Commons' Category:Tundra and its subcategories (1,391 files looked through, 1,024 kept by licence and size, 93 shortlisted at full size), as 1,920-px thumbnails (the nearest step Commons serves at or above 1,600). The reference `tools/sandbox/looks/tundra-autumn-day.json` (`look.ts reference --open`).
  - Dropped: winter, night, aerial and helicopter views, close-ups, figures, towns and machines, old scans, maps and museum dioramas.
  - Seven more were dropped after `--draw-bands`: with no sky in frame, or with snowy peaks, glacier fronts or hazy far water read as sky.
  - Credits, each licence checked: APL (CC BY-SA 3.0), Billy Lindblom (CC BY 2.0, two), Sergey Ashmarin (CC BY-SA 3.0), dvs (CC BY 2.0), Sergey.vlad.popov (CC BY-SA 4.0), Ninaras (CC BY-SA 4.0), Shishaev Kirill (CC BY 3.0), Grichanov and Khruleva (CC BY 4.0), Clemens Ratschan (CC BY 3.0), SerBor (CC BY 3.0), Jerzy Strzelecki (CC BY-SA 3.0), NTNU Vitenskapsmuseet (CC BY 2.0), Ansgar Walk (CC BY-SA 3.0), Paxson Woelber (CC BY 2.0), MichaelKirsh (CC BY-SA 4.0), MarmotChaser (CC BY 2.0), Western Arctic National Parklands (CC BY 2.0), ADialla (CC BY 2.0), Buiobuione (CC BY-SA 4.0), Bjoertvedt (CC BY-SA 4.0), and public domain from the US National Park Service (four), Fish and Wildlife Service (two), Geological Survey and Paul Gierszewski.
- **Crop sources** (`crops/`): the same photographs plus five close views: a bilberry slope in Padjelanta (Ökologix, CC BY-SA 3.0), cotton grass at Arviat (Mike Beauregard, CC BY 2.0), a lichen carpet (Education Specialist, CC BY 2.0), frost polygons in the Beartooths (James St. John, CC BY 2.0), a thermokarst pool (NOAA, public domain).
- **Game frames, composition and wind** (`games/`, `sheet-games.jpg`): Steam store screenshots, app IDs checked against the names the API returns. The Long Dark (305620): five of 23, open snow by day. Skyrim Special Edition (489830): one of eight, the tundra plain round Whiterun. Valheim (892970): none of 20; they are forests, sea, interiors, night, and one snowy mountain with a fight and the interface.

## Measured

`look.ts reference --open` over the twenty-nine. Band edges checked by eye on every frame (`bands/`). The mountain lake's photographs are beside them for scale ([2026-10-15-mountain-lake-reference.md](2026-10-15-mountain-lake-reference.md)).

| | Tundra, autumn | Morskie Oko, summer |
|---|---|---|
| Whole frame mean, p5 | 0.487, 0.123 | 0.497, 0.210 |
| Below 0.08 | 3% | 1% |
| Red / green, blue / green | 1.05, 0.95 | 0.92, 0.93 |
| Local contrast | 0.24 | 0.17 |
| Lower third lit orange ("lit litter") | 27% | 3% |
| Shade (below 0.08) red / green | 2.29 | 0.86 |
| Sky: share, mean, blue / green, saturation | 35%, 0.78, 1.09, 0.18 | 25%, 0.76, 1.06, 0.11 |
| Far land: mean, blue / green, saturation, contrast | 0.42, 0.87, 0.30, 0.20 | 0.51, 1.00, 0.17, 0.13 |
| Middle: mean, blue / green, saturation | 0.37, 0.75, 0.46 | 0.42, 0.87, 0.26 |
| Near: mean, blue / green, saturation, contrast | 0.29, 0.67, 0.53, 0.40 | 0.38, 0.79, 0.37, 0.15 |
| Far land under the sky | 0.45 | 0.34 |
| Far bluer than middle | 0.12 | 0.13 |

- **Warm, dark, saturated ground under a big pale sky** [P]. Red outweighs green over the whole frame (1.05). The near ground is darker than the lake's (0.29 against 0.38) but half again as saturated (0.53 against 0.37), and stays saturated far out (0.30 in the far band). Its shadows are red-brown, not blue: shade red/green is 2.3.
- **A third of the frame is sky** (35%), pale and only a little blue. Most of the photographs are under broken cloud.
- **Far land stands darker under the sky** than at the lake (0.45 against 0.34): low rolling country 5 to 20 km off, not ranges 50 km off. It still shifts blue by the same 0.12 over the middle distance. Frame by frame it runs 0.23 to 0.68.
- **Contrast is a speckle** [P]: 0.40 in the near band against the lake's 0.15. Lichen, red leaves and dark gaps alternate at a hand's scale.
- `look.ts`'s forest numbers read oddly here and should not be tuned to: "lit litter" counts the orange heath, and the shade's colour is the dark gaps between shrubs.

## The communities' colours

Mean sRGB (0 to 255) of crops (`crops/`). Spread is the standard deviation of brightness inside the crop: speckle, against a smooth fill. The ground layers and cards are tuned to these, as seen at the distance given.

| Community | Crop | sRGB | Spread |
|---|---|---|---|
| Lichen heath, a lit mat at 10–30 m with shrubs through it | Kola, c0054 | 187, 169, 139 and 158, 138, 108 | 0.26–0.29 |
| Lichen alone (its pale pixels) | Kola, c0054 | 226, 214, 189 | |
| Lichen and crowberry carpet, close | lichen carpet | 98, 94, 70 | 0.25 |
| Dwarf birch heath in autumn, near | Dagalifjell, c0964 | 140, 104, 70; its red patches 126, 79, 60 | 0.23–0.26 |
| Dwarf shrub heath at 20–100 m | Bolshezemelskaya c0001, Noatak c0455 | 88, 61, 37 and 87, 52, 30 | 0.13–0.15 |
| Bilberry slope, crimson and orange | Padjelanta | 127, 79, 18 | 0.19 |
| Dwarf birch turned yellow | Kola, c0054 | 141, 100, 32 | 0.24 |
| Wet sedge, straw and rust, near | Bennett Island, c0373 | 118, 84, 49 | 0.12 |
| Sedge flat at a distance | Brooks Range plain, c0212 | 71, 54, 38 | 0.08 |
| Cotton grass heads (pale pixels) | Arviat | 199, 191, 177 | |
| Moss cushions by pools | Bennett Island, c0373 | 132, 101, 12 and 129, 101, 14 | 0.12 |
| Brown moss and sedge round them | Bennett Island, c0373 | 93, 61, 20 | 0.12 |
| Snow patch, lit | Rocky Mountain NP, c0419 | 231, 231, 234 (221, 217, 217 in thin shade) | 0.05 |

- **The moss is gold, not green** [P]: blue near zero (12 to 14 of 255), the most saturated thing on the ground. Green moss is in the summer photographs only.
- **Lichen is cream, not white** [P] (226, 214, 189), and greys to about 160, 140, 110 once shrubs and shadow are mixed in at 10 to 30 m. Lichen heath is the brightest ground after snow, and the most speckled.
- **Shrub heath darkens with distance** [P]: the same red-brown quilt is 140, 104, 70 near and about 88, 58, 34 at a hundred metres, as gaps and shadow fill the view. Sedge is smooth at every distance (spread 0.08 to 0.12). So the ground's tint past 60 m must be the dark value, not the near leaf colour [I].

## How big things are on screen

- **The cover is low** [P]: on Noatak's heath (c0457) a walker's boots are hidden and nothing more, under 15 cm. Dwarf birch stands (c0054, c0964) reach the knee or above, under a metre [I].
- **Sizes from the literature** [I]: crowberry and lichen mats 5 to 15 cm; cotton grass (*Eriophorum vaginatum*) tussocks 20 to 30 cm high and wide, a stride apart; ice-wedge polygons about 30 m across with under 50 cm of relief ([biome-demos](2026-10-15-biome-demos.md#tundra), [P] there).
- **The quilt's patch size** [P]: near the eye, patches of lichen, red leaves and sedge are a metre or two across; in the middle distance they merge into bands following the ground's lie (rises pale, hollows red or rust, wet flats straw). There are pools from a metre to a hundred metres, many of them, and snow patches as long strips in lee hollows and gullies (c0019, c0419).
- **Few verticals** [P]: a boulder, a lone larch or spruce (c0049, c0060), spruce islands at the edge (c0352). The horizon is low rolling land, often with a range behind.

## What the game frames show

- **Composition** [P]: the horizon sits at about the middle of the frame or higher. Long sightlines run to a few thin verticals (bent spruce, poles, a cabin), and a path or stream leads in (Skyrim's plain).
- **Wind** [P]:
  - The Long Dark shows it with snow streaming low across the ground (frames 10 and 16) and smoke leaning off a chimney (22).
  - Stiff grass tufts stand through the snow (19).
  - Skyrim's plain has wind only in its grass clumps and haze.
  - Moving images are needed to judge the motion; the trailers are left for the film reference.
- **Not the look** [P]: their bands misread overcast game skies (four of six frames find under 6% sky), and their colour is far off the photographs.
  - The Long Dark is bluer (blue/green 1.01 to 1.14 against 0.95) and flatter (contrast 0.06 to 0.24).
  - Skyrim is warmer (red/green 1.32).
  - As D14 and the demo say, they stay a reference for composition and motion only.

## What it leaves open

- `tundra-winter-day`, and a film reference for wind (the trailers, sampled as the mountain lake's were).
- Shrub heights here are inferred; a photograph with a person among dwarf birch would measure them.
- The band split trusts a sky above the land: snowy peaks, glacier fronts, a hazy lake under the skyline or a lone tree's crown (c0060) can take sky. The frames where it failed are not in the reference.
