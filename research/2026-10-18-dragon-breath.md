# Dragon breath and a burning castle: what makes them read as real

2026-10-18. The question: what do the best games and films do for a dragon's breath and a building on fire, and what does real flamethrower and napalm footage show, so that the castle film's fire stops reading as orange paint? It builds on [2026-10-16-real-time-fire-rendering.md](2026-10-16-real-time-fire-rendering.md) (colour, flame shape, smoke optics, haze, firelight numbers) and [2026-10-15-dragons-and-fire.md](2026-10-15-dragons-and-fire.md) (spread, flight). Tags: [S] shipped or filmed practice, [P] physics or measurement, [I] our inference.

## Answer

- **A breath reads as fire from contrast inside it, not from its colour.** Every reference has a near-white rod at the mouth, a rolling front of yellow cores with orange-red rims, and black soot wrapping the outside. A stream of one orange, however bright, reads as paint. Ours spans 1230 to 1480 K, so it is one orange.
- **Soot is half the picture.** In flamethrower and napalm footage, black smoke rolls off the top of the stream within a few metres of the nozzle and keeps billowing where it lands, darker than anything else in frame. Ours has no smoke at all on the breath.
- **Liquid fire falls.** Thickened fuel rains burning drops from the stream's underside and splashes as it hits, then clings and burns on. A gas jet (Skyrim, most games) does not, which is why it reads as a game effect.
- **Light sells the heat.** Every game reference throws a strong, moving orange light on the ground and walls under the breath. That counts for more than the flame's own detail at film distances.

## 1. How the best do it

- **[S] Game of Thrones** (seasons 6 to 8). Dragonfire was real: a flamethrower on a motion-control head, shot as plates, then comped and scaled. The VFX team has said a real burn was the way to get its weight, its heat ripple and how it clings. CG fire was simulated (Houdini) and matched to the plates. It reads by its near-white throat, a thick rolling body, black smoke torn back by the dragon's speed, and fire left burning on the ground.
- **[S] How to Train Your Dragon** (DreamWorks, 2010 onward; the 2025 live action). Stylised colour but physical shape: a bright, narrow jet that blooms into a rolling ball at its end, with strong rim light on the dragon's head and the ground.
- **[S] Elden Ring** (Agheel, Lansseax, Placidusax). A dense volume of flipbook fire particles, wide at the end, laid on the ground as a sweep that keeps burning for seconds, with a big moving point light. No smoke to speak of: the strength is the lingering ground fire and the light.
- **[S] Skyrim.** A cone of additive orange sprites with a point light: the dated look. It reads as a game effect because it is all one colour, adds up to a glow and leaves no smoke.
- **[S] Monster Hunter World and Rise (Rathalos, Rathian).** The fireball is a hot yellow core with an orange rim and black smoke trailing it, ending in a blast with a lingering burning patch and scorch.
- **[S] Horizon Forbidden West.** The machines' fire (Fireclaw, Scorcher) is dense flipbook streams with heat distortion and a smoke trail, and fire stays on the ground where it lands.
- **[S] Battlefield (1, V, 2042) flamethrowers.** A liquid-fuel stream: narrow and bright near the nozzle, turbulent and widening out, ending in a burst that spreads across what it hits, with a dark smoke column above, burning ground left behind, and flickering light. These are the closest to real footage.

## 2. Real flamethrower and napalm footage

- **[P] Near the nozzle**, a thickened-fuel rod 1 to 3 m long, its flame thin and very bright (overexposed white-yellow).
- **[P] The body** unravels into a rolling, turbulent front. In it are bright yellow cores (the overexposed parts), orange around them, dull red-brown at the rims, and pockets of black soot where fuel burns rich.
- **[P] Soot** rolls up and out of the front's upper side within 3 to 5 m and is near black (albedo 0.03 to 0.05, as §4 of the earlier note). It hangs behind as a column long after the stream stops.
- **[P] Drops** of burning fuel fall from the stream's underside in arcs, and splash and burn where they land.
- **[P] Where it hits**, the stream spreads across the surface in a rolling ball 2 to 3 times the stream's width, then clings and keeps burning.
- **[P] Light.** A big burn lights the ground orange for tens of metres, even by day.

## 3. What this means for ours

- **[I] The jet's colour** should run 1100 K at its rims to 1750 K in its cores. It should be white-yellow at the mouth for the first fifth of its length, and it should cool down the stream. Emission is weighted by thickness, not by a flat alpha, so a hot core glows past what it covers. The olive cast came from hot yellow drawn at half cover over smoke.
- **[I] Soot pockets** go inside the flame: where the flipbook's heat is low inside the body, it covers without glowing.
- **[I] The breath gets its own smoke**: lit, near-black cards born along the outer half of the stream and at its end, rising and swelling, carried along by the stream and downwind, lasting a few seconds after it stops.
- **[I] Some puffs become drops**: they leave the stream and fall under gravity as small bright streaks.
- **[I] A burning building**: fewer, larger flames (one card set per 2 to 4 m of front, Heskestad heights) over the rows of alike small tongues, plus thick dark smoke wrapping them.

## 4. Real fire measured (castle film round 9)

[P] 19 photographs from Wikimedia Commons (public domain, CC0, CC BY and BY-SA; credits with the sheet, outside the repository at `~/.cache/terra-review/refs/fire-real/CREDITS.txt`): fuel and pool fires, an airshow's napalm wall, an M2 in colour and M7, M132 and M113 flamethrowers on 1960s slides, burning timber buildings, a bonfire and a campfire, gas and oil-well jets. Each was measured on a crop round its fire by `measure.mjs` (kept beside them, it runs on renders too). Flame pixels are those whose brightest channel is at least 110 and red, or near-white. The table gives each class's median, a share of the flame pixels. Red is a hue under 25, which is what reads as deep orange in a photograph. Fine, mid and coarse are the luma energy in octaves of 2-4, 8-16 and 32-64 px. Each class has only two to four images, so the medians are a direction, not a tolerance.

| Class | White | Yellow | Orange | Red | B/R | Soot/flame | Dim | Fine/mid/coarse |
|---|---|---|---|---|---|---|---|---|
| Flamethrower | 0 (43 in one clipped core) | 25 | 28 | 13 | 0.59 | 0.05 | 4 | 5/20/75 |
| Napalm wall | 0 | 3 | 28 | 68 | 0.28 | 0.52 | 22 | 12/27/61 |
| Pool fire | 6 | 23 | 34 | 35 | 0.27 | 0.14 | 26 | 7/20/74 |
| Burning building | 1 | 9 | 29 | 53 | 0.32 | 0.04 | 39 | 7/25/67 |
| Gas jet | 1 | 24 | 25 | 28 | 0.40 | 0.00 | 11 | 4/18/77 |
| Campfire | 0 | 13 | 31 | 53 | 0.18 | 0.28 | 39 | 14/28/58 |

What they show:
- **White is rare.** Daylight wood and building fires are 0-4% white. White shows only at a jet's nozzle, in a pool fire by night (6-12%) and at an oil well's base. Thin flame is never white; on film white is the camera clipping.
- **Colours run yellow inside, then orange, then deep red-orange outward.** Wood, building and napalm fires are more than half deep red-orange and only 3-13% yellow. Pool fires and jets are about a quarter yellow. B/R stays at 0.16-0.43, except in the hottest jets.
- **The rim is broad, dim and see-through, not an outline.** 25-48% of a pool, building or campfire flame is dim. Only jets and gas columns are solid.
- **A fuel jet is a clean rope.** Its edge meets the sky, not soot (0-3%). Soot comes only where it ends in a fireball: overlapping orange balls with black caps.
- **Wood and pool fires interleave with black at every scale.** There are 169-214 dark pockets inside a bonfire's flame against a median of 38, and the soot column dwarfs a pool fire's flame, which shows through holes in it.
- **The detail is mostly coarse but never smooth.** In every class 58-77% of the energy is coarse and 4-14% fine.
- **By day a fire lights almost nothing round it.** At dusk the ground and every silhouette's rim go orange.

[I] What that means for ours, measured on the same crops of round 8's and round 9's probes:
- A building's flames: the gradient's stops spread so the lower half of a flame's heat is deep red-orange and only its top fifth yellow and pale. Its dim rims are see-through at a third of their cover. Its soot comes from the simulation (§5).
- The breath: a rope that is solid, yellow and pale in its core, orange through its body, deep red only at its outer puffs, and soot only on the outside puffs once it opens. A hot yellow must never be laid at part cover or under soot, since a darkened yellow is olive.

## 5. The flipbook, round 9

[I] `tools/textures/flames.ts` now simulates one fire on a grid of 160 by 320 cells, 2.8 times as many as before, and keeps 96 frames. Each texel carries four values:
- its temperature;
- its soot, made where the body burns rich, burnt away where the gas is hot and carried on as black wisps where it has cooled;
- and its motion to the next frame, across and up, so the shader moves each frame along the flow into the next. This is how Unreal and EmberGen play motion-vector flipbooks. It replaces a crossfade, which doubles every edge.

The sheet is 1920 by 2560 RGBA8 (19.7 MB on the GPU). The game's three channel-fires go, and cards vary by their moment and mirror instead.

[P] The smoke streaks at 18-21 s were not the anti-aliasing. They were the same with TAA off, and r8 found them the same with no motion blur. They were the sky's clouds: drawn over the atmosphere, the sky sphere is premultiplied and so sorted among the see-through things by its middle, which is the camera, so it came last and laid its clouds over the smoke in front, since smoke writes no depth to stop it. The sky is now drawn first.

## 6. Real footage and a volume, rounds 11 and 12

[I] A wall's flames are frames of a public-domain wood fire filmed at night (Wikimedia Commons "File:Fire.ogv"), keyed by luma in `tools/textures/flames.ts`. Each frame is a whole fire of many tongues, so three cards to a blaze on a two-metre grid made a hedge; one card a blaze, with a third of the blazes left as embers and smoke, shows dark timber between separate fires. The key keeps dull yellow glow at partial cover, which read lime and olive over stone and sky. Holding a dim texel's green under its red, by how bright it is, takes that out, since a dim flame is red.

[I] The breath is marched as a volume (128 steps at most, empty space skipped). Three things made it a flat salmon ball (r11) and then a flat orange one (r12's first probes):
- **Too thin.** At 5 a metre the sky showed through. Now the core is 18 a metre and the rims 5.
- **Graded from the axis.** In an opaque gas the eye sees only the outer layer, so the yellow core was hidden. Now the temperature is even through the body and falls off only in a thin rim.
- **Past the knee.** The cards' gain of 1.7 lifted the gas past the tonemapper's knee, which desaturated orange to salmon (sRGB 230,142,82).

[I] Centred on the struck wall, half the fireball was behind the stone. Rays ended after a sliver of its thin edge, so windows showed through it. It now stands 0.85 of its radius back from the wall. Its face takes wide noise in temperature, which gives bright lumps, dark red folds and soot pockets.

[P] Still open after r12: inside the fireball, a lighter patch with straight edges. It is not the stream: cutting the stream out of the ball left it unchanged. Suspects are the heat haze, which reads the opaque scene copy without the breath, and a smoke card's edge. The rope is also smoother than a real jet.

## Limits

- The game and film breakdowns are from talks, interviews and footage the author knows, not frame measurements. No reference sheet was measured with `tools/sandbox/look.ts`.
