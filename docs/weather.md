# Weather

Status: **proposal, 2026-10-19**. How the weather is drawn and checked is technical and mine to decide (D418); what the weather does to play, and how much of it a world gets, is the developers', Open at the end. It builds on [climate.md](climate.md), which owns the quantities (D312 to D315, D327), and follows [scene-rules.md](scene-rules.md): every rule has a system that owns it and a check that fails when it breaks. What shipped games do, with sources, is [research/2026-10-18-storms-and-winter.md](../research/2026-10-18-storms-and-winter.md).

## Why

The developer, 2026-10-18: "spin up a new session working explicitly on weather and season conditions (i.e. storms, and winter)", with films that read as The Witcher 3's storms, Red Dead Redemption 2's weather, Sons of the Forest's winter and Ghost of Tsushima's wind.

The climate already knows the weather: fronts drift over the world, cloud builds ahead of them, rain or snow falls as the temperature says, the wind rises and turns (D315). Snow lies, the ground wets and leaves turn from what fell before (D327). But nothing in the picture follows the weather of the moment: the sky's clouds are a constant, the sun shines at full strength under a full deck, nothing falls, and the haze is the hour's. A storm and a still day are drawn alike.

## Constraints

- **Quantities, never a weather flag** (climate.md's rule). Nothing asks "is it a storm"; each part reads cloud, what is falling and how fast, how much of it is snow, the wind, the air's wetness, and lightning's strikes, where it is. "Storm" and "blizzard" are labels for tools and people (below), never read by a system.
- **One weather at the eye.** A `Weather` resource holds the climate where the camera is, eased over seconds so a front's edge passes rather than snaps. Every drawn part reads it, as everything reads the one wind (D255). Shared state keeps its own sample (D312): what changes play is worked out by the shared crate, the same on every machine.
- **Each rule in the system that makes the thing** (Systems apart, D58): the sky owns the deck, the light owns how cloud dims the sun, the air owns visibility, drift owns what falls, the ground and the plants own wetness and snow. A scene never tunes a rule; a film only chooses when it is shot.
- **Physical numbers, cited.** Illuminance under cloud, visibility in rain and snow, fall speeds, a flash's flicker are measured quantities (the research note's), each a named constant with its source, so a tuning round adjusts a fact, not a fudge.
- **Port, don't invent** (D196): techniques from the talks (Frostbite, Decima, RDR2, Ghost of Tsushima), code only from MIT, BSD, Apache or zlib sources.
- **The grade stays neutral** (D370): a storm is darker and greyer because its light is, never because a grade says so.

## What each part does

| Part | Rule | Owner | Check |
|---|---|---|---|
| Sky | Cloud cover is the deck: broken cumulus to a full grey overcast, darker and lower bellied as rain grows, lit from above and seen from below; the sun's disc and glow fade behind it | `sky`, `sky.wesl` | Overcast still: no sun disc, sky band luma within the references' range |
| Light | Cloud's thickness takes the sun's direct light away and leaves the sky's: under a full deck about a tenth of clear-sky light reaches the ground, almost none of it direct, so shadows soften and go; the shade's colour is the deck's grey | `daylight` | Unit test of the ratios; still: shadow contrast falls with cloud |
| Exposure | The eye opens for the dimmer day, but only part of the way (about half the stops lost), so a storm reads dark, as a photograph's does | `daylight` | Overcast still's mean luma under the clear day's, over the night's |
| Air | Visibility shortens with what falls (snow far more than rain of the same water), the haze takes the deck's colour, and the far land flattens and greys | `daylight`, `air` | Unit test: visibility per mm/h against the research note's |
| Falling | Rain and snow are drift kinds (D379) seen where it falls, as many as the rate says, carried by the wind: snow as soft flakes drifting at about a metre a second, rain as streaks stretched along their fall by the shutter | `drift`, `drift.ron` | Unit test: none fall at none, more fall at more, snow only when it is snow |
| Wind | The one wind (D255) rises with the weather and gusts travel across grass and crowns as fronts; what falls is carried by it | `wind`, plant shaders | Existing sway tests; film |
| Surfaces | Wet ground, bark and stone darken and gloss with wetness (D327), puddles stand in hollows, rain rings the water, snow lies on what faces up: ground, needles and limbs today, rocks, logs, roofs and the far forest next | ground, plants, `lake`, building materials | Look-away and colour bands (scene-rules) on the winter and storm stills |
| Lightning | Strikes are events of the storm cells, from the seed and the clock (shared, D312): a branched bolt, a flash of three or four return strokes over half a second lighting the deck from inside and the land from the bolt, thunder at 343 m/s | `weather::lightning` | Unit test: flash timing; frame ledger: a flash's luma steps |
| Cold | Breath shows below about 5 °C; ice lies on still water after frost (D315's integrated ice), snow on the ice | bodies, `lakes` | Stills |

## The weather as data

- **A world's weather is its climate's numbers** (`climate.ron`): how often and how hard it rains, the wind, the fronts' size and spells. A monsoon world and a dry one differ only there.
- **Named weathers are queries** (`assets/data/weather.ron`): a name and the conditions it means ("snowfall: snow falling at 0.8 mm/h of water or more"; "storm: rain at 4 mm/h or more and wind at 9 m/s or more"). Tools use them to find a moment, people use them to talk, a forecast can name one; no system reads them to change behaviour.
- **What falls looks as `drift.ron` says** (D136): the flakes' size, colour, fall and how many to a millimetre an hour.
- **Picking the weather for a run** (D418): `--weather <name>` moves the clock from `--date` on, a day at a time, to the first day whose `--time` hour has that weather at the start. The weather is the climate's own, so the snow on the ground, the wet and the light all agree with the falling snow: nothing is forced.

## What the climate lacks

The fronts make at most the theme's heaviest rain (2.5 mm/h on the start, steady frontal rain) and winds to about 10 m/s. Real storms are convective: cells a few kilometres across, living an hour, with rain of 10 to 50 mm/h, gust fronts of 15 to 25 m/s ahead of them and lightning. Proposed for stage 2: a storm field in `terra_climate`, cells drawn from the seed and the clock where the air is warm, humid and a front is passing, adding their rain, gusts and strike rate to the fronts', so a summer afternoon builds thunder and a winter one does not.

## Order of work

| Stage | Builds | Done when |
|---|---|---|
| 1. Overcast and snowfall | `Weather` at the eye; the deck, the light and exposure under cloud, visibility in what falls; snowfall as drift kinds; `weather.ron` and `--weather` | A 20 s film in the lush forest in January snowfall, reviewed against the references at 7.5 or more |
| 2. The storm | Storm cells in the climate; rain streaks and splashes; rings on the water; wet surfaces and puddles; lightning and its light; gust fronts in the grass | A storm film at the lake, reviewed the same way |
| 3. Winter whole | Snow on rocks, logs, roofs, the far forest and far land; ice on the lake with snow on it; breath; blowing snow in a gale; colder haze | A winter film at the lake, frozen |
| 4. Change | A front arriving within a film; a season's time-lapse | A clear-to-storm film with no step a person sees |
| 5. Play | What the weather does to bodies and work, as the developers decide below | Their tests |

## Open questions for the developers

1. **How stormy the start is.** Recommended: as a temperate place, some 20 to 30 thunder days a year, mostly summer afternoons; winter brings snowfall and the occasional blizzard. Open.
2. **Lightning that strikes.** May a strike set a tree or a roof alight, or hit someone on a hilltop? Recommended: yes, rare and shared, a reason to shelter. Open.
3. **Rain on the body.** Should rain soak clothing and so chill (D153), and a storm put out an open fire? Recommended: yes, through wetness, as the quantities already allow. Open.
4. **How dark a storm may be.** Recommended: about two stops under an overcast day, never as dark as a moonlit night, so play stays readable. Open.
5. **Whiteouts.** May a blizzard cut sight to some 50 m? Recommended: yes, rarely, the hardest weather of the start. Open.
6. **Weather at the trailer.** Should the trailer's runs keep their clear mornings, or does one become a storm or a snowfall? Recommended: one snowfall run and one storm run, once each passes review. Open.
