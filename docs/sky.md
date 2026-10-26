# Sky

Status: **proposal, 2026-10-26**. How the sky is drawn and checked is technical (D515, Leaning until the developers have read this); how it looks within the art style, which clouds and effects a world gets, how far one sees the weather coming and the moon's cycle are the developers', Open at the end (D516 to D522). It is the sky's half of [weather.md](weather.md), which owns the light under cloud, the air, what falls and the surfaces; [climate.md](climate.md) owns the quantities. What shipped games and engines do, with sources, is [research/2026-10-26-volumetric-skies.md](../research/2026-10-26-volumetric-skies.md).

## Why

The developer, 2026-10-26: "we may need a little bit more of a skybox going on... we'd like to have things like volumetric clouds and realism for weather, but in general to have it look 'real'"; "clouds would have diff shades, there could be rain, wind, sun, moon"; "clouds wouldn't be in the skybox though they'd be in.. like in the sky itself". Clouds are volumes in the air that the sun lights and that shade the land, not a painted backdrop.

Today Bevy's physical atmosphere draws the sky's colour and the sun (D212); over it `sky.wesl` draws clouds as noise on a flat layer, an overcast deck as one grey sheet (D418), stars and a moon that stands opposite the sun at every phase (D310). The clouds have no thickness, cast no shadow, do not follow the climate's wind or where the fronts are, and drift by each machine's own run time, so two players see different skies.

## Constraints

- **The sky is the climate's picture** (D315, D418). Cloud, its kind, its base and where it moves come from the climate's quantities (cover, what falls, humidity, temperature, wind), as Microsoft Flight Simulator's clouds are its weather model's; never a sky preset or a "storm" flag.
- **Clouds are volumes, lit by physics.** Their shades come from how light scatters in them (dark bases, bright tops, the silver lining toward the sun, bright interiors from light bouncing many times), not from painted colours, so they are right at every hour and under every sun without tuning.
- **Clouds are a light.** The clouds drawn are the clouds that shadow the ground, dim the sun, cut shafts in the haze (D217) and light the shade through the sky light (D490, D492). The shade and the sky never disagree.
- **Clouds stand at their real heights** above the ground, so a 2.5 km peak pierces a cumulus field and a valley fills with stratus, but spread in the air's own frame (D212, D334), so a layer recedes to the horizon as it does on Earth rather than closing over a 30 km world as a ceiling.
- **The same sky for everyone** (D52, D312). Clouds, their motion and lightning are functions of the seed and the world's clock, which is all the server sends of the weather (D315); nothing in the sky reads a machine's own run time.
- **Port, don't invent** (D196): the method every shipped game uses (Horizon's Nubis, Frostbite, Unreal, RDR2), code from the open implementation on Bevy's own atmosphere (D515).
- **The grade stays neutral** (D370): a sky is darker because its light is.

## Clouds

The climate decides how much cloud there is and of what kind; the sky draws it. Kinds are a continuum on one model (a height profile and a density per kind, Nubis's and the World Meteorological Organization's cloud atlas), not separate effects:

| Kind | Height | When the climate makes it | How it looks |
|---|---|---|---|
| Cumulus | base 0.5 to 2 km (lower in humid air), tops 1 to 3 km | sun on the ground, unstable air, low to middling cover; afternoons | separate heaps, bright tops, flat grey bases, silver edges toward the sun, born and dying over 10 to 30 minutes |
| Stratocumulus | 0.5 to 2 km | stable humid air under an inversion, middling to high cover | rolls and patches joined in a sheet, gaps of blue |
| Stratus and overcast (nimbostratus in rain) | from the ground to 2 km; deep in rain | high cover, a front's steady rain or snow | an even grey deck, darker and lower bellied as rain grows, scud under it (today's deck, made a volume) |
| Fog and low cloud | on the ground, filling valleys, wrapping peaks | humid still air cooling: night, morning, over water (D244, D371) | cloud the eye stands in, lit as the rest |
| Altocumulus and altostratus | 2 to 7 km | ahead of a front | rippled fields; a pale veil the sun shows through as a disc |
| Cirrus and cirrostratus | 5 to 13 km, ice | a day ahead of a warm front | thin streaks and veils, lit pink long after sunset; halos |
| Cumulonimbus | base under 1.5 km, tops 10 km and more, an anvil | the storm cells weather.md proposes for its stage 2 | a tower with a dark base, rain shafts beneath, a shelf cloud on the gust front, lightning inside |

- **The base follows the air**: a cloud forms where rising air saturates, about 125 m up for each degree between the temperature and the dew point, so humid days have low bases and dry ones high (Espy's rule from the climate's temperature and humidity).
- **The weather map is the climate's cloud over the sky's reach**, kilometres round the eye, not the one value at the eye: a front's edge is a line of cloud one can see coming (how far is D518).
- **Rain darkens and thickens** cloud, as D418's deck does today; snow cloud is flatter and paler.

## Light and shade

- The sun lights a cloud through the air (its colour and dimming from the atmosphere at the cloud's height), the sky lights it from all round, and the ground lights its base from below: paler and warmer over snow, a burning town or a hearth's glow.
- At dusk cloud tops stay sunlit after the ground's sunset and turn rose, then grey from below as the earth's shadow rises (D432); by night clouds are lit by the moon and hide the stars, and a thin veil shows the moon's halo.
- **On the ground**: a cloud's shadow moves across the land at the wind's speed; under a broken sky the light comes and goes as shadows pass, and under a full deck the sun's light is gone and the shade's colour is the deck's (D418's rule, now by place rather than by the cover's average).
- **In the air**: the haze takes the cloud shadows, so shafts fall through gaps (crepuscular rays) and the haze under a deck is dim.
- **In the sky light**: the painted sky the shade is lit by (D190, D490, D492) follows the clouds: by cover, from the CIE's clear sky (type 12, today's) toward its overcast (type 1, brightest overhead), with the sunward glow dimmed by the cloud between, so a shaded wall under a passing cloud dims and greys with the sky.

## Wind, rain, snow and lightning in the sky

- **Clouds ride the wind at their height**, about twice the ground's at 1.5 km and turned a little from it, so they cross the sky faster than the grass moves; towers lean with the shear; the climate's wind sets both (D255 is the ground's).
- **Rain and snow show in the sky** as grey shafts under distant cells and the deck's lowering; near the eye they are drift (D379, weather.md). Virga, rain that dries before it lands, hangs from high bases in dry air.
- **Lightning lights the cloud from inside**: most flashes are glows in the deck, a few are bolts to the ground (weather.md's rules: three or four strokes over a third of a second, thunder at 343 m/s).

## Sun, moon and stars

- **The sun** is Bevy's atmosphere's disc and light (D212), dimmed by the cloud on its line of sight; a veil of cirrostratus shows it as a pale disc with a 22° halo.
- **The moon** is a body with a phase: lit by the sun as the real moon is (brightest full by a surge, a quarter moon a tenth as bright), earthshine on a crescent's dark part, rising and setting on its own path through a month, up by day half its time (Krisciunas and Schaefer 1991; Jensen and others 2001). Its light by night is its phase's (D310 sets it as if always high and full; D520 asks).
- **Stars** from a real catalogue's positions, brightnesses and colours, turning with the sky, the Milky Way on moonless clear nights, hidden by cloud and drowned by moonlight and haze.
- **The sky's optics**, each from its geometry: a rainbow 42° round the point opposite the sun when the sun is behind and rain is lit in front (a secondary at 51°); halos and sundogs in ice cloud; crepuscular rays through gaps; which of these a world gets is D521.

## Realms

A realm changes parts, materials and light, never the rules (D14): its sun's colour and size, how many moons and of what (size, colour, period), the air's make-up (Bevy's scattering medium), what its clouds are made of (water, ice, ash that absorbs, dust that reddens) and which kinds its climate makes, its stars. All of it data on the realm's theme; none of it a painted backdrop.

## Multiplayer

The server sends the clock (D315); every client works out the same clouds from the seed and it: the climate's fields, the weather map, the noise's drift and every lightning stroke. What a cloud does to play (the sunlight that warms a body, D312) is the shared crate's, from the climate's cover, never from a client's drawn shadow. What clients draw differently is only quality: steps, resolution, how far.

## Budget

The world must hold D25's frame (13.3 ms at the 99th percentile on an RTX 3080-class GPU at the high preset; 30 fps on the integrated GPU at the low one). Shipped games spend about 2 ms (Horizon Zero Dawn on a PS4 at 1080p); the cost is held down by tracing at a quarter or half resolution and rebuilding the rest from previous frames (temporal reprojection), by leaping through empty air on a coarse distance field, and by tracing the cloud shadow map a sixteenth a frame.

- **High preset**: clouds, their shadow map and their shafts within 1.5 ms on the RTX 3080-class target at 1080p (so well under 1 ms on the bench's RTX 5090).
- **Low preset** (integrated GPU): within 2 ms with fewer steps at quarter resolution; if it cannot, the low preset keeps a 2D layer driven by the same weather map and lighting, so the sky and the shade still agree.
- Measured with `cargo xtask bench` (D185) on the bench scenes and a cloudy and an overcast variant of `walk`, both GPUs reported.

## Checks

Each rule fails a check an agent runs (D69), as weather.md's table does; references are the WMO cloud atlas's photographs per kind, sky photographs at the hours filmed and frames of Horizon Forbidden West, RDR2 and Microsoft Flight Simulator.

| Rule | Check |
|---|---|
| The same sky everywhere | Two runs at one world clock started minutes apart draw the same sky still, pixel for pixel within the reprojection's noise |
| Cover is the climate's | An upward fisheye still (as a total-sky imager) measures the share of sky covered within 0.1 of the climate's cloud, at three covers |
| Shades | Stills of cumulus by day: bases darker than sunlit tops by the references' ratio; a cloud before the sun has an edge brighter than the sky beside it; no part of a cloud black |
| Cloud shadows | Under a broken sky the share of sunlit ground is within 0.15 of one minus the cover; a film's shadow edges move at the wind at cloud height |
| Light under cloud | The shade's light falls with the cover by Kasten and Czeplak's ratios (D418's tests), and a wall under a passing shadow dims within a second of it |
| Base height | Unit test: the base from temperature and dew point by Espy's rule; still: a humid morning's base below a dry afternoon's |
| Motion and stability | A 20 s film at noon and dusk: no shimmer, ghosting or crawl on cloud edges in the frame ledger (scene-rules); clouds keep their shapes as they drift |
| Moon | Unit test: lit fraction and brightness by phase against Krisciunas and Schaefer; stills at new, quarter and full |
| Optics | Still: a rainbow's radius 42° (±1°) from the point opposite the sun, only when the sun is behind and rain is lit |
| Cost | `cargo xtask bench` within the budget above on both GPUs |
| Look | Stills and a film reviewed against the references at 7.5 or more, as weather.md's stages are |

## Order of work

Smallest first; each stage lands only when its check passes. Nothing is built until the developers have read this (D49).

| Stage | Builds | Done when |
|---|---|---|
| 0. Measure | A spike runs the open Bevy cloud implementation (D515) in the sandbox's lake and forest at our Bevy, cumulus and overcast | Its cost on both GPUs and stills reviewed: port it, or port Nubis's method from the talks |
| 1. One sky for all | Today's 2D clouds and deck move by the world's clock and the climate's wind at cloud height, the weather map from the climate's cloud round the eye | Two runs at one clock draw the same sky |
| 2. Volumes | Cumulus, stratocumulus and the overcast deck as one volumetric layer from the weather map, lit by sun, sky and ground, replacing the 2D layer on the high preset | Cover, shades and cost checks; stills reviewed |
| 3. Clouds as light | The cloud shadow map on the ground and in the haze; the sky light by cover (CIE types); the sun's beam by its line through cloud | Shadow, light-under-cloud and shafts checks; a broken-sky film |
| 4. Night | The moon's phase, path and light; earthshine; stars from a catalogue; clouds by moonlight | Moon checks; night stills at three phases reviewed |
| 5. High and thin | Cirrus and altocumulus, halos; the base by humidity; fog and valley cloud as cloud on the ground; peaks in cloud | A front's arrival in a film: cirrus first, then the deck, reviewed |
| 6. Storm sky | Cumulonimbus from weather.md's storm cells, rain shafts, lightning inside cloud, rainbows after | The storm film of weather.md stage 2 with its sky reviewed |

## Open questions for the developers

1. **How real the sky looks** (D516). Recommended: clouds and light physically based, photographic, as the shipped games we look to; art-style's light stylisation stays in materials and the grade. Open.
2. **Which clouds, and in what order** (D517). Recommended: cumulus, stratocumulus and overcast first, cirrus next, cumulonimbus with storms, fog and valley cloud last. Open.
3. **Seeing the weather coming** (D518). Recommended: the sky shows the climate's weather to the horizon, so a front is seen hours ahead, as D78's forecasting by looking asks. Open.
4. **Above the clouds** (D519). Recommended: clouds seen from below and from peaks, wrapping them and filling valleys; flying through cloud waits for something that flies. Open.
5. **The moon's cycle** (D520). Recommended: real phases on a month (29.5 days on the start), its own path, up by day half its time; moonless nights darker but never black. Open.
6. **The sky's effects** (D521). Recommended: rainbows, halos and sundogs, crepuscular rays and lightning in cloud, each by its physics; auroras for cold or magical realms later. Open.
7. **How dramatic a storm's sky is** (D522). Recommended: as real storms build, a summer afternoon's cumulus towering into an anvil over an hour, a shelf cloud ahead of the gust front, rain shafts; drama from timing and seeing it come, not exaggeration. Open.
