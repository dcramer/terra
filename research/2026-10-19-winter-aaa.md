# Winter that reads: how shipped games draw a snowy forest

2026-10-19. The question: Terra's snowfall in the lush forest (D418, weather rounds 2 to 5) scores 4/10 against Sons of the Forest's winter, and the developer calls it "unimpressive". Five rounds tuned exposure, haze and flakes. Stepping back: what makes winter read as winter in Red Dead Redemption 2, The Witcher 3, Sons of the Forest, Horizon (Frozen Wilds and Forbidden West), Ghost of Tsushima and Yōtei, Assassin's Creed (Valhalla and Shadows), God of War Ragnarök and Unreal Engine 5's snow scenes, what do we have against what they do, and which few changes buy the most? It follows [2026-10-18-storms-and-winter.md](2026-10-18-storms-and-winter.md), which covers falling snow, lying snow and seasons in general, and [docs/weather.md](../docs/weather.md). Reference frames are in `~/.cache/terra-review/weather/refs-aaa` (sheet.jpg, CREDITS.txt), out of the repo for copyright, beside the Sons of the Forest frames in `refs`.

## Answer

Winter reads when the whole frame is **three values: white snow, dark evergreens and bark, grey air**, with almost no colour. Every reference does this. Ours is a green forest with a white floor. The five rounds tuned the air and the flakes. The real gaps are the plants and the snow's value:

1. **The plants are in winter state.** Broadleaves stand bare, herbs and flowers are gone, and only evergreens keep needles, which carry a white load on top over dark undersides. Ours shows sage-green broadleaf crowns, bushes and flowers in January. That one fact makes the frame read as "spring with snow".
2. **Snow is the brightest thing in the frame**, close to white, and it lights everything above it from below. Ours is darker than the haze, and 24% of our pixels are near black, against 0 to 5% in every reference.
3. **The air is cold by its light, not by a filter.** Shade is lit by the sky, and nothing green bounces. Ours carries a green-yellow cast from the summer canopy's bounce.
4. **Falling snow is layered:** big soft flakes near, many small ones mid, and a veil far off. Ours is a few grey quads.
5. **The snow carries marks:** trails, a trodden path, things poking through, and snow on every up-facing surface.

## Measured

ImageMagick on the stills (480 px wide, sRGB). "Black" is the share of pixels under 0.1 luma, "white" the share over 0.7.

| Frame | Mean luma | Luma sd | Black | White | Mean R, G, B |
|---|---|---|---|---|---|
| Terra r5/s1/a.png | 0.28 | 0.18 | 0.243 | 0.010 | 71, 72, 67 |
| Terra r5/frame10.png | 0.26 | 0.16 | 0.243 | 0.008 | 65, 66, 57 |
| Sons of the Forest 1 to 3 | 0.38 to 0.46 | 0.08 | 0.000 | 0.000 | 96 to 119, B 3 to 4 under R |
| RDR2 snow trail, overcast | 0.54 | 0.22 | 0.024 | 0.286 | 132, 139, 152 |
| Horizon Frozen Wilds snowfall | 0.38 | 0.20 | 0.046 | 0.088 | 85, 100, 112 |
| UE5 pine forest (RealBiomes) | 0.37 | 0.19 | 0.029 | 0.076 | 96, 92, 98 |

What the numbers say:
- **Blacks.** Ours has five to ten times the references' near-black pixels. These are the trunks (luma 14 to 23 in the r5 review) and the dark undersides of the green foliage.
- **Whites.** Ours has a tenth of their bright pixels. Sons of the Forest has none over 0.7 either, but only because its milky veil lifts everything to one grey (sd 0.08). That is the other way to make winter read: low contrast rather than bright snow.
- **Colour.** Ours is the only frame where blue is under green and red by 5 to 9 levels: a warm green cast. The others are neutral (Sons of the Forest, UE5) or blue (RDR2, Horizon, both graded).

## What shipped games do

**Red Dead Redemption 2.**
- Snow deforms through tessellation and keeps trails; the PC release raised snow-trail quality (Rockstar via GameSpot, in the earlier note).
- Snow builds up in layers on clothing and horses (Digital Foundry, via vgtimes).
- The atmosphere is one integrated system of sky, cloud and volumetric fog (Bauer, SIGGRAPH 2019).
- The Grizzlies are snowy all year, so their spruces are authored with snow on them. In refs-aaa/game-rdr2-snow-trail-1 (observed), the conifers are near black with white on every upper bough. The trail is a deep trough whose walls are blue-grey in shade, and nothing is green.

**The Witcher 3.** Skellige's snow is regional and authored; no talk covers it. The next-gen update reworked vegetation (Digital Foundry preview). No usable snow frame was found.

**Sons of the Forest.**
- Its seasons run over a 20-day cycle; in winter the whole island is covered and the lakes freeze (Sons of the Forest wiki).
- In our refs (observed), deciduous trees are bare, the conifers carry snow, and the frame is nearly monochrome: saturation 0.03 against our 0.12 to 0.22.
- A milky in-forest veil keeps trunks mid-grey.

**Horizon Zero Dawn: The Frozen Wilds** (Decima).
- Dynamic deformation compresses snow into trails that persist; grass shows where the snow is pushed aside; the feet kick up snow (Eurogamer, via GameBanshee).
- In the refs (observed), the trees are bare and the brush is frosted with rime. Snowfall reads as a soft veil in the gorge, not as distinct flakes.

**Ghost of Tsushima and Ghost of Yōtei.**
- Yōtei's terrain is tessellated and deforms at runtime: particles and geometry render into a displacement buffer as characters walk, roll and fight.
- The same system knocks snow off trees and bushes, spawning snow particles. So trees carry a snow layer that is state, not paint.
- Snow sparkles through "stable screen-space noise".
- Characters take on snow through a directional grid, the one that also makes them wet and muddy.
- Source: PlayStation Blog, Ghost of Yōtei tech deep dive.

**God of War Ragnarök.**
- Snow is real displacement through hardware tessellation, carved each frame on the GPU by capsules on physics bodies: each pushes the snow down to where its shape meets the hard ground beneath.
- A detail normal map reads a top-down alpha mask that the VFX write as characters move.
- Particles collide with the displacement itself, not with the depth buffer.
- GPU-spawned chunks decorate the displaced edges.
- Each terrain chunk takes two draws, one tessellated and one plain, with a compute pass choosing which chunks are inside the displaced area.
- Source: Surricchio, "Reinventing the Wheel for Snow Rendering", GDC 2023, via 80.lv.

**Batman: Arkham Origins and Rise of the Tomb Raider** (the earlier note).
- Batman renders what touches the snow from below, in an ankle-high orthographic view, into a height map, and displaces with tessellation or relief mapping (Barré-Brisebois, GDC 2014).
- Rise of the Tomb Raider decouples deformation from geometry ("deferred deformation", GPU Pro 7), because statically tessellated snow meshes cost too much.

**Assassin's Creed.**
- Valhalla's snow tech is unpublished. Assassin's Creed III tessellated its footprints (Ubisoft's PC technology video, via Geeks3D).
- Shadows has four seasons on Anvil's weather (Atmos). Winter "strips bare" bushes and grass, which removes cover for stealth (producer Karl Onnée, TechRadar; PCGamesN). Snow builds up and buries paths, rocks and trees in a storm, and water freezes (Push Square, on Ubisoft's video series).
- So the shipped answer to "bushes in January" is no.

**Unreal Engine 5.**
- Snow on meshes is a blend by the world normal's up component (VertexNormalWS, WorldAlignedBlend), with one falloff control and an amount driven over time (The Rookies, UE5 open-world part 3).
- Deep snow uses Nanite tessellation and displacement (5.4, experimental).
- Trails are written to a render target or runtime virtual texture that the landscape reads (community practice, no Epic doc found).
- In the RealBiomes demo (observed), the pine trunks are mid-brown and lit from below by the snow, never black.

**Physical facts the looks rest on.**
- Fresh snow reflects about 90% of visible light (0.98 at 370 nm, 0.82 at 900 nm for moderately aged fresh snow, Annals of Glaciology 1994). Its visible albedo falls only slowly with age; soot at 1 ppm takes 5 to 15% off (Warren and Wiscombe).
- A camera's reflected meter makes snow grey: lit snow is 1.5 to 2 stops over 18% grey, so photographers add +1 to +2 EV (Canon, Park Cameras, Luminous Landscape).
- Conifers intercept 0 to 60% of a year's snowfall (Moeser, Western Snow Conference 2017). A branch takes more snow until its load beats its strength, then sheds it (Hedstrom and Pomeroy 1997, 1998). So crowns are loaded unevenly, thickest on the upper, horizontal boughs.

## What we have against what they do

| Element | Terra today (code) | Shipped games |
|---|---|---|
| Broadleaves in winter | Oak, birch and hazel cards drop by their leaf year (plant_years.ron, `bare()` in foliage_fragment.wesl:297) | Bare (Sons of the Forest, Horizon, AC Shadows) |
| Herbs, flowers, bramble | No leaf year (herb, daisy, buttercup, campion, bramble; holly is rightly evergreen), so they are green all year; flora `choose()` (flora_kinds.rs:204) takes no season | Gone or withered in winter (AC Shadows) |
| Snow on boughs | A colour mix toward SNOW (0.68, 0.72, 0.80) by lying depth × an up mask 0.3 to 0.75 × patches (foliage_fragment.wesl:345-349) | White loads on the upper faces over dark needles; authored snow variants (RDR2, Witcher) or a removable layer (Yōtei); UE: world-normal blend |
| Snow lying | Albedo 0.70 to 0.75 (ground.wesl:120), ±6% noise called sparkle, roughness 0.8, soft swell normals, litter sinks under it | Albedo about 0.9, glints, real displacement, blue-grey shade in troughs |
| Snow bounce | SNOW_BOUNCE (0.45, 0.47, 0.52) in the sky volume (skylight.rs:65); the canopy's hidden share bounces LEAF_GLOW green | Snow lights trunks and undersides from below (observed everywhere) |
| Exposure | The eye takes back 0.8 of the stops lost to cloud (D418), metered on the light | Snow scenes held 1 to 2 stops over a grey meter |
| Grade | Neutral, D65 (D370, the developers') | RDR2 and Horizon grade cold (observed); Sons of the Forest near neutral |
| Falling snow | Two drift kinds, near and far (drift.ron), drawn before volumetric fog, which darkens them | Near soft discs, mid flakes, far veil; flakes lit like the air |
| Trails | None | Displacement from a top-down or capsule stamp (Batman, GoW, Yōtei, Horizon, RDR2) |
| Other surfaces | Rocks, logs and trunk bases carry no snow (weather.md stage 3) | Snow on every up-facing surface (AC Shadows, all refs) |

## Ranked changes

Ranked by how much of "it reads as winter" each buys for its cost. Verify each with a still at the r3 path vantage and `r3/rgb.sh`, `r3/flakediff.mjs` and `look.ts measure --against sotf-forest-snow`, plus the measures above (`black`, `white`, B against R). Change several levers per still (the GPU queue costs 20 to 35 minutes a still).

1. **Winter plant state: no green but evergreens.**
   - **What:** give herb, flower and bramble kinds a year in plant_years.ron (herbs die back, flowers bloom in their season and are gone outside it), so the existing `bare()` discard takes them. First identify the pale-green crowns in frame10's right half (`r snap` on one). If they are a kind with no leaf layer, add one; if they are pine cards, the pine's card needs work, not its year.
   - **Source:** AC Shadows, Sons of the Forest and Horizon observed; phenology.
   - **Verify:** a unit test that a January sample gives the flowers' and herbs' share 0. On the still, the green-hue share outside conifers is under 2%, and mean B is within 3 levels of R.
   - **Cost:** data and a small `choose()` or shader change, an hour or two.
   - **Flagged below as design.**
2. **Snow loads on the conifers, white on top and dark under.**
   - **What:** in `seasoned()`, snow at albedo about 0.9 and neutral (as the ground's rule), not the blue (0.68, 0.72, 0.8). Weight it by the crown normal's up component and by height in the crown, so upper boughs carry most. Keep the undersides' needles dark. Load by interception, so it is patchy and heaviest on horizontal boughs.
   - **Next:** a removable snow layer of clumps on top of boughs, as Yōtei knocks off. It could reuse the blown-snow cards (D345).
   - **Source:** Yōtei, UE world-normal blend, RDR2 observed, Hedstrom and Pomeroy.
   - **Verify:** in a crown crop, the upper third's luma is at least 0.8 of the ground snow's and the undersides are at most 0.35 of it. White-over-0.7 pixels in the crowns are 10 to 30%, as in the RDR2 frame.
   - **Cost:** the mask is one shader edit; the clumps are a day.
3. **Snow as the brightest thing, and its light from below.**
   - **What:**
     - Ground SNOW to about 0.85 to 0.9 linear, neutral.
     - SNOW_BOUNCE to follow the snow's albedo (about 0.85), not 0.45.
     - The sky volume's hidden canopy share bounces snow's colour, not LEAF_GLOW green, where the crowns are bare or loaded.
     - Then check exposure keeps snow 1.5 to 2 stops over grey: sRGB about 200 to 230 on open snow under the deck.
   - **Source:** Warren and Wiscombe; photography's +1.5 to 2 EV; the UE5 and RDR2 trunks observed.
   - **Verify:**
     - near-black pixels from 24% to under 6%;
     - trunks at 5 to 10 m at 35 to 60 luma (handoff target);
     - snow over haze from 0.6 to at least 0.9;
     - B against R within ±3.
     - `cargo xtask looks` on tundra and mountain, so sunlit snow does not clip.
   - **Cost:** constants, one still and a looks run.
4. **Falling snow in three layers, lit like the air.**
   - **What:** near flakes as large soft discs within 0.3 to 3 m, streaked by the shutter (the r3 patch). Mid flakes as many small ones. Far as a veil scrolled on a camera-centred cylinder, as Remember Me's rain layers (Lagarde, in the earlier note). Draw or composite the flakes so the volumetric fog does not darken them by the depth behind; it currently runs after the transparent pass with a fixed ambient (handoff).
   - **Source:** Horizon's gorge and the Sons of the Forest frames observed; Lagarde.
   - **Verify:** `flakediff.mjs` gives 300 to 600 flakes of 2 to 12 px, at least 200 luma over dark ground. A 2 s film shows the slant with the wind.
   - **Cost:** about a day, mostly the fog-order fix.
5. **Snow on every up-facing surface, and what pokes through.**
   - **What:** rocks, logs, trunk bases and bush tops read the climate map's snow as the ground does (weather.md stage 3). Short grass tufts and fern tips poke through as frosted dark marks, instead of the dark scratch strokes of buried litter and grass blades.
   - **Source:** AC Shadows, Sons of the Forest, Horizon (rime on brush) observed.
   - **Verify:** on the still, no up-facing rock or log top is darker than 0.5 of the ground snow's luma, and dark strokes in open snow are under 1% of its pixels.
   - **Cost:** a shader edit in each of three or four materials.
6. **Trails and the trodden path.**
   - **What:**
     - First, the path as a compacted channel: snow a little greyer and smoother, with raised banks by the normal.
     - Then Batman's camera-centred height map, stamped from below by walkers' capsules as GoW stamps them. It is read as a normal and parallax first, with displacement later. Trails persist and fill in again as snow falls.
   - **Source:** Batman (GDC 2014), GoW Ragnarök (GDC 2023), Yōtei, Horizon.
   - **Verify:** a unit test that a stamp lowers the height and fresh snow raises it again. On the still, the path's luma is 0.8 to 0.9 of the untrodden snow's, with a visible edge in the normal.
   - **Cost:** a channel from the path's data is small; trails are several days, and in multiplayer the walkers' stamps must be the same on every machine or local only (a technical choice).
7. **Glints on snow in clear weather.**
   - **What:** a procedural sparkle on a jittered 3D grid of seeds, filtered against aliasing (Wang and Bowles, EGSR 2016), or Yōtei's stable screen-space noise. It barely shows under overcast, so it is for clear winter days, not this film.
   - **Verify:** on a sunlit tundra still, 0.1 to 1% of snow pixels over 0.95, and frame to frame the change at a still camera is under 2% (no shimmer).
   - **Cost:** half a day.

Not on the list: a cold grade. The reviewer asked for "saturation about 0.4, slightly cool white balance", and RDR2 and Horizon do grade cold. But D370 holds the grade neutral, and that is the developers' decision. Changes 1 and 3 remove the cast that the grade would hide.

## For the developers (design, Open)

1. **Do deciduous trees and bushes stand bare in winter?** The oak's, birch's and hazel's years already drop their leaves (D327), and every reference with broadleaves shows them bare. Recommended: yes for all deciduous kinds, with holly, ivy and the conifers evergreen and bramble keeping a few dark leaves, as in life.
2. **Do flowers and green herbs appear in January?** Today daisies, buttercups and campion are placed and drawn all year. Recommended: no; each blooms in its season and is gone or a dry stalk outside it, as AC Shadows strips cover in winter. That changes play too: less cover and fewer forage plants in winter.
3. **Does the grade stay neutral in snow (D370)?** Recommended: keep it neutral and let the light and the plants carry the cold; revisit only if changes 1 to 3 still read warm.
4. **Do trails persist and are they shared?** Footprints that last a few minutes and that every player sees are a tracking mechanic (hunting, being followed). Recommended: yes, rare and short-lived, if the server can afford it.

## Limits

- Rockstar, CD Projekt, Endnight, Guerrilla (for snow) and Ubisoft (for Valhalla) have published no snow talks. Their rows rest on press, wikis and our reference frames, marked observed.
- The reference frames are promotional or captured with different exposure and grading, and two are sunny. They bound the values; they are not targets.
- The measures are coarse (thresholds on downscaled sRGB), and nothing here was rendered in Terra.
- No snowy Witcher 3, Ghost of Tsushima forest or overcast Valhalla frame was found.

## Sources

- PlayStation Blog, "Ghost of Yōtei: tech deep dive", 2025. https://blog.playstation.com/?p=411201
- 80.lv, "Santa Monica's Senior Programmer on How God of War Ragnarök's Snow System Was Made" (Paolo Surricchio; GDC 2023, "Advanced Graphics Summit: Reinventing the Wheel for Snow Rendering"). https://80.lv/articles/santa-monica-s-senior-programmer-on-how-god-of-war-ragnar-k-s-snow-system-was-made/
- Colin Barré-Brisebois, "Deformable Snow Rendering in Batman: Arkham Origins", GDC 2014. https://gdcvault.com/play/1020177/Deformable-Snow-Rendering-in-Batman
- Fabian Bauer, "Creating the Atmospheric World of Red Dead Redemption 2", Advances in Real-Time Rendering, SIGGRAPH 2019 Courses. https://doi.org/10.1145/3450623.3464676 (as Crossref lists it)
- vgtimes.ru on Digital Foundry's history of snow in games. https://vgtimes.ru/tech-and-hardware/141250-kak-sneg-v-igrah-stal-realnym-digital-foundry-prosledili-put-ot-beloy-tekstury-do-obemnogo-sveta.html
- GameBanshee, "Horizon Zero Dawn: The Frozen Wilds is an Unmissable Tech Showcase". https://gamebanshee.com/k4ah6
- TechRadar, "Assassin's Creed Shadows' open-world will feature seasons that impact combat and traversal". https://www.techradar.com/gaming/consoles-pc/assassins-creed-shadows-open-world-will-feature-seasons-that-impact-combat-and-traversal
- PCGamesN, "Assassin's Creed Shadows seasons explained". https://www.pcgamesn.com/assassins-creed-shadows/seasons
- Push Square, "Assassin's Creed Shadows devs explain one of the best open world weather systems in gaming", 2026. https://www.pushsquare.com/news/2026/04/assassins-creed-shadows-devs-explain-one-of-the-best-open-world-weather-systems-in-gaming
- Geeks3D, "Assassin's Creed III PC Technology Video", 2012. https://geeks3d.com/20121123/assassins-creed-iii-pc-technology-video
- The Rookies, "Creating Open World Environments in Unreal Engine 5: Part 3". https://blog.therookies.co/2024/01/25/creating-open-world-environments-in-unreal-engine-5-part-3/
- Puget Systems, "Exploring Unreal Engine 5.4" (Nanite tessellation and displacement). https://www.pugetsystems.com/blog/2024/04/16/exploring-unreal-engine-5-4-whats-new-and-whats-next/
- Beibei Wang and Huw Bowles, "A Robust and Flexible Real-Time Sparkle Effect", EGSR 2016. https://diglib.eg.org/bitstream/handle/10.2312/sre20161209/049-054.pdf
- Jakob et al., "Discrete Stochastic Microfacet Models", SIGGRAPH 2014. https://research.cs.cornell.edu/stochastic-sg14/stochastic.pdf
- Warren and Wiscombe, the spectral albedo of snow (Western Snow Conference summary). https://westernsnowconference.org/node/893 ; Annals of Glaciology 20, 1994. https://dx.doi.org/10.3189/1994AoG20-1-1-5
- Hedstrom and Pomeroy, snow interception (Western Snow Conference, 1997). https://westernsnowconference.org/node/265 ; Moeser, Western Snow Conference 2017. https://westernsnowconference.org/node/1825
- Canon, "Photographing snow". https://www.usa.canon.com/learning/training-articles/training-articles-list/photographing-snow ; Luminous Landscape, "Exposing snow". https://luminous-landscape.com/exposing-snow
- Sons of the Forest wiki, Seasons. https://sonsoftheforest.wiki.gg/wiki/Seasons
- Digital Foundry's Witcher 3 next-gen preview, via Pure Xbox. https://www.purexbox.com/news/2022/12/the-witcher-3-next-gen-impresses-hugely-for-xbox-series-x-in-digital-foundry-preview
