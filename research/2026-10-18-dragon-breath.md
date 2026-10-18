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

## Limits

- The game and film breakdowns are from talks, interviews and footage the author knows, not frame measurements. No reference sheet was measured with `tools/sandbox/look.ts`.
