# Climate: seasons, weather and what they do

Status: Open. A draft for the developers, 2026-10-12. Seasons exist and change play (D149), and weather exists and differs by world (D78). This proposes how both work. Grew out of [biomes.md](biomes.md), which keeps what seasons do to plants' looks.

## The rule: fundamentals, not seasons

From the developers, 2026-10-12: "cold is cold, hot is hot, but remember, procedural multiple realms so it's about fundamentals not about hardcoded simplistic concepts", and winter is not snow all the time.

So the game has no season flag that switches on snow, bare trees or hunger. A season is only a slow swing in a few physical quantities. Weather is quicker changes on top. Everything else responds to the quantities, the same way on every world (D59, D122):

| Quantity | Swings with |
|---|---|
| **Temperature** | Latitude, height, day and night, the season, the weather |
| **Daylight** | How long the day is and how high the sun climbs, by the season |
| **Precipitation** | How often and how much falls, by the season and the weather; whether it is rain or snow is the temperature's doing |
| **Wind** | The weather |
| **Wetness of the air** | The season and the weather |

A world's theme sets its base climate and how far, and in which quantities, its seasons swing. So:

- A temperate world swings in temperature, for summers and winters.
- Another world swings in precipitation, for wet and dry seasons with no winter.
- A third barely swings at all.

None of these is code; each is numbers. "Winter" is a label the game can show for the cold part of a temperate year, worked out from the quantities, never read by a system.

## What responds

Each thing reads the quantities where it is:

- **Bodies.** A body loses or gains heat by the air's temperature, the wind, being wet, clothing, shelter and fire. Cold wears you down; so does heat, and a desert world's summer can kill. Which survival attribute this is, is open ([survival](proposals/game-design-001/survival.md), question 5).
- **Ground and water.**
  - Snow lies where precipitation fell below freezing, and melts with warmth and sun. Winter has snowy spells and thaws, lowlands may get rain, and mountaintops keep snow all year.
  - Water freezes after enough cold and thaws after enough warmth, so a lake can be walked on in a hard winter but not a mild one.
  - Rain wets the ground into mud.
  - Snow and mud slow walking, and snow keeps tracks (D93).
- **Plants**, each by its niche ([biomes.md](biomes.md#biomes-from-rules-not-a-list)). Real plants follow warmth accumulated over days and the length of the day, and so do ours:
  - they grow when it is warm, wet and light enough;
  - they flower and fruit after enough warmth;
  - deciduous kinds turn and drop their leaves as days shorten and cold builds;
  - regrowth (D150) follows the same rates, so nothing regrows in a frozen month.

  Spring and autumn emerge from this.
- **Food.** What can be gathered is what plants and creatures offer at the time, so a lean season is lean because nothing is fruiting, not because a rule says so. Storing and preserving food follows from that.
- **Creatures**, each by its own niche: active within their range of temperature, and following their food. Migrating, sleeping through the cold and breeding in the warm emerge per species, as data.
- **Sight.** A wood that has dropped its leaves can be seen through.

## How long a year is

A number per world, in its theme, and so changeable at any time. Since our world is persistent (D52), its year passes in real time, unlike Sons of the Forest's or Don't Starve Together's, which pause when no one plays.

**Starting value: 365 days, about 7.6 real days at the half-hour day (D134).** Chosen to fit the rest:

- **Each season lasts about two real days.** That is one or two evenings for most players: long enough to prepare in autumn and live through winter, short enough to meet every season within a couple of weeks.
- **It drifts against the real week,** so a player who always plays on Tuesday evenings doesn't always find the same season.
- **A cold season is not one long blizzard.** About 90 days of weather, with snowy spells, thaws and clear cold days, make it varied, as a real winter is.
- **Days lengthen and shorten with the season** around the half-hour.
- **Growth is compressed,** as games compress it: a great tree regrows in some fraction of a year, not decades (D122, fun over realism).

## How it could work

Technical, Leaning, and nothing is built until the design is agreed:

- **Weather is a function of the world's seed and its time.** Fronts are noise moving across the planet. Every client works out the same weather from the clock, so the server sends nothing but the time. Shared by a thousand players (D52), it costs nothing.
- **Ground state is integrated from recent weather** at coarse resolution, again with nothing stored:
  - snow depth;
  - wetness;
  - ice on water.

  Players' fires and digging are edits on top, like any other.
- **Each quantity is a field** that systems ask at a place and time, like the ground query. Survival, plants, creatures and the renderer all ask the same fields.

## Open questions

1. **Which survival attribute** cold and heat wear down: their own (body temperature), or health ([survival](proposals/game-design-001/survival.md))?
2. **How hard a winter should be on the starter world.** Proposed: survivable with a fire and clothing from the first days, never a wall.
3. **Can players forecast weather?** Proposed: by looking, at the sky and at animals (D93), not by a readout.
