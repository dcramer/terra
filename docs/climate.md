# Climate: seasons, weather and what they do

Status: Leaning, 2026-10-12. Seasons exist, change play and are fundamentals rather than flags (D149, Decided); weather differs by world (D78); body temperature is its own attribute (D153). The rest proposes how they work. Grew out of [biomes.md](biomes.md), which keeps what seasons do to plants' looks.

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

- **Bodies** have a body temperature ([below](#body-temperature), D153). Cold wears you down; so does heat, and a desert world's summer can kill.
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

## Body temperature

Probably an attribute of its own (D153): "if you're too cold it could increase stamina usage (same with too hot) or increase thirst or something else". Proposed, after what a real body does:

- **What moves it:**
  - It drifts toward what the surroundings push it to: the air's temperature, the wind, being wet, the sun, shelter, a fire nearby.
  - Clothing slows the drift.
  - Work warms you.
- **Comfortable within a band**, and nothing happens there. Beyond the band, effects grow with how far out you are:

| | Cold | Hot |
|---|---|---|
| A little | Shivering burns food: hunger rises faster | Sweating: thirst rises faster |
| More | Work and sprinting cost more stamina, and it comes back slower | The same |
| Far | Hands are clumsy: tools work slower | Dizzy: stamina runs out sooner |
| Too far | Health falls (hypothermia) | Health falls (heatstroke) |

- **Seen on the body before any meter** (D93): breath in cold air, shivering, sweat, a flushed face. Whether there is also a meter is survival's open question 5.
- **Creatures have it too,** by their own band. That is why some sleep through the cold.
- **The order to build the effects in,** decided as the developers asked (2026-10-12):
  1. The body temperature itself, and health falling when too far out. These are the stakes.
  2. Hunger and thirst rising. These are the everyday cost that makes preparing worth it.
  3. Stamina costs.
  4. Clumsy hands and dizziness, which need tuning against work and combat.

## Winter on the starter world

The starter world is challenging, not a freebie (2026-10-12, D62). Winter poses real problems, harder than Valheim's or Enshrouded's and gentler than Vintage Story's brutal ones (D3). The point is goals, not punishment. Proposed:

- **What autumn asks for:**
  - food laid in and kept from spoiling;
  - firewood;
  - warm clothing, from hides and wool;
  - a shelter that holds heat.
- **What winter gives:**
  - tracks in the snow to hunt by;
  - frozen lakes that reach islands and far shores;
  - what is found or met only in the cold.
- **The first winter is survivable with a fire, a shelter and clothing** made in the first days. A good autumn makes it comfortable; a poor one makes it a struggle, never a wall (D18).

## Reading the weather

Players can forecast it (2026-10-12), mostly by looking, as D93 asks:

- clouds building and the sky's colour;
- the wind turning;
- animals sheltering or moving.

Technology and magic can add to it, such as an instrument or a rite that tells further ahead, but they don't replace looking.

Since weather is worked out from the seed and the time, what is coming is already known to the game, so a forecast can be exactly right.

## How it could work

Technical, Leaning, and nothing is built until the design is agreed:

- **Weather is a function of the world's seed and its time.** Fronts are noise moving across the realm's world. Every client works out the same weather from the clock, so the server sends nothing but the time. Shared by a thousand players (D52), it costs nothing.
- **Ground state is integrated from recent weather** at coarse resolution, again with nothing stored:
  - snow depth;
  - wetness;
  - ice on water.

  Players' fires and digging are edits on top, like any other.
- **Each quantity is a field** that systems ask at a place and time, like the ground query. Survival, plants, creatures and the renderer all ask the same fields.

## Open questions

Answered on 2026-10-12:

- **Body temperature:** probably its own attribute (D153).
- **The starter world's winter:** challenging, with goals (D62).
- **Forecasting:** yes, mostly by looking, with technology and magic adding to it.
- **Which effects of cold and heat come first:** left to me; the order is under [body temperature](#body-temperature).

Nothing is open here now beyond the numbers, which are tuned by playing.
