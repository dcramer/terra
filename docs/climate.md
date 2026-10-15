# Climate: seasons, weather and what they do

Status: Leaning, 2026-10-12; how it works decided 2026-10-14 (D312 to D314). Seasons exist, change play and are fundamentals rather than flags (D149, Decided); weather differs by world (D78); body temperature is its own attribute (D153). Grew out of [biomes.md](biomes.md), which keeps what seasons do to plants' looks.

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

## How it works

Decided with the developers on 2026-10-14 (D312, D313); the code's shape is the agent's (D314). Nothing anywhere asks what season it is: it asks the quantities, at a place and a time.

### The quantities are fields

Temperature, daylight, precipitation, wind and the air's wetness are functions of a place and a time, in a crate that does not depend on Bevy (D28), so every machine works out the same answer from the world's seed and its clock and the server sends only the time (D52):

- **Temperature** is the theme's base, less about 6.5 °C a kilometre of height, plus the year's swing (lagging the sun by about a month, as real seasons do), the day's (warmest mid-afternoon), and the weather's.
- **Daylight** is the sun's path for the date: the sun's declination swings with the year, so winter days are short and its sun low.
- **Weather** is fronts of noise drifting across the world with the clock; whether what falls is rain or snow is the temperature's doing.
- A world's theme is the numbers: its base climate, and how far each quantity swings and how fast. A temperate world swings in temperature, a monsoon world in rain; no code differs.
- **In code (D315):** `terra_climate`. The clock is days; the year's slow terms move on once a day, at midnight. The start's climate is `assets/data/climate.ron`, and its clock starts in late August, under the sun the art was lit by (10° north), with the summer's leaves.

### What the past leaves is integrated

Snow, wet ground, ice, and the warmth and chill plants count are what recent weather left behind. Each is folded over the days before, on a coarse grid (tens of metres a cell), worked out when asked and kept by the day, never stored in the save:

- snow: what fell below freezing, less what warmth and sun melted;
- wetness: rain, less what dried;
- ice on water: thicker after frost, thinner after thaw;
- warmth (degree-days above a base since midwinter) and chill (cold days, and the day's length), the clocks plants keep.

In code (D315): cells are 32 m; a fold starts at the first midsummer at least half a year back, so warmth (above 5 °C) and chill (below 10 °C since midsummer) are exact and snow and ice start from none. A species' year is said in those counts.

Players' fires, digging and building are edits on top, like any other.

### Shared and seen (D312)

What changes play is the same on every machine, worked out by the shared crate: the temperature where a body stands, snow depth, ice that holds a walker, wet ground, what a plant offers to gather. What only changes the look is each machine's own: exactly how far a leaf has turned, frost on a branch, breath in the air.

### What grows where, and when (D313)

- **A species carries its niche and its year, as data:** the climate it tolerates (the coldest winter it survives, the warmth it needs to grow, wetness, shade, soil) and when it leafs out, flowers, fruits, turns and drops its leaves, by the warmth and chill counted and the day's length. An oak turns once the day is under so many hours and chill passes so much; a pine barely changes.
- **The world decides what grows where** by setting the climate fields against those niches (as [biomes.md](biomes.md#biomes-from-rules-not-a-list) proposes): where an oak's niche fits, oaks can grow, the best-fitting kinds most. A biome is what emerges, named afterwards; a world's theme says which families of species exist there at all. When a place's climate is not one a kind tolerates, that kind does not regrow there (D150), and another that fits does.
- Each plant is its kind's year, shifted a little by its own seed and by height, so a stand turns over days, not all at once.

### In ECS

- **Things that care carry a sample** (`ClimateSample`: temperature, wind, wetness, sun, snow), refreshed by a system often near a player and rarely far off, so the cost follows what is near players, not the world's size (D127).
- **The ground bends it** (D323): a place carries how its ground faces the sun and how much drifting snow it catches, worked out by the world from the terrain, so sunny slopes are warmer and lee hollows hold snow longest, and the climate crate never reads terrain. A place's latitude is where it is.
- **Local things bend it**, as components: a fire's heat, a shelter's walls and roof (D261), the canopy's shade (the sky occlusion volume, D270), clothing's warmth.
- **Small systems respond, each reading samples:** a body's temperature drifts toward its surroundings (D153); a creature acts within its own band, so sleeping through the cold emerges; snow and mud slow a walker; ice holds one once thick enough.
- **Plants are not entities that tick:** there are far too many. A plant's state is a function of its cell's counted warmth and chill, its kind's year and its own seed, worked out when it is drawn or gathered.
- **In code (D327):** a `ClimateSample` is taken again every half second within 48 m of a player, every 3 s within 200 m, every 20 s beyond, at most 0.6 ms of a frame. `Heat` is a fire's radiant watts, spread over a sphere round it; `Sheltered` (0 to 1) takes the wind to a tenth, keeps off what falls and the sun, and holds in a fire's heat; under the canopy the sun is cut by the sky hidden, and the rain and wind to two fifths where it is closed. The past at a place goes from its day's start to the next day's through the day, so snow builds and melts by the hour. A kind's year (`terra_climate::LeafYear`) is when its leaves come out (warmth), turn and fall (chill, once the day is shorter than its length), and how much of them turn and drop; a plant's seed makes it count up to a quarter faster or slower.

### Drawn from the same fields

The world layer bakes small climate maps round the camera, as it bakes the cover map: how far each kind's leaves have turned and fallen, snow, wetness, frost. The shaders read them:

- leaves take their kind's colour for the moment, and drop away card by card as they fall, as cards already thin with distance (D223), until a tree stands bare;
- snow lies on surfaces facing up: the ground, the tops of branches and rocks, its edge broken by noise; wet ground and bark darken;
- litter builds after the fall and grass browns, through the floor's own rules (D246);
- the sun's path follows the date, the haze the air's wetness, breath shows in the cold.

On the GPU this is a few texture reads; on the CPU a few cheap functions and a fold per cell per day.

In code (D327): two maps a kilometre across round the camera, a 32 m cell a texel, painted on another thread when the camera has gone 128 m or the clock a quarter of an hour: the ground's (snow depth, its wetness, rain falling now, frost), and the leaves', a layer a kind in the years file's order, how many leaves it has and how far they have turned, each for a plant early and late in its kind's year, so a plant takes its place between by its seed.

### Order of work

1. **The fields:** temperature, daylight by the date, precipitation and fronts, in the shared crate, with tests; the sun's declination following the date. Built (D315).
2. **The integrated state:** snow, warmth and chill per cell, kept by the day. Built (D315).
3. **The samples and the climate maps**, and a scripted way to set the date as `--time` sets the hour, so a year can be looked at. Built (D327): `terra_world::climate`, kinds' years in `assets/data/plant_years.ron`, and `--date 10-15` (a day of an Earth calendar, as that share of the world's year) or `--date 40` (a day of the clock); the log says what the climate is at the camera once its maps are painted.
4. **The proof in the lush forest:** the oak's year as data, turning and dropping its leaves; snow on the ground and branches; litter after the fall. Judged as the forest was: a reference of the look in each season, films at each, and the checks every change passes (the `environment` skill).
5. **Body temperature** (D153), and snow and mud under a walker.

## Open questions

Answered on 2026-10-12:

- **Body temperature:** probably its own attribute (D153).
- **The starter world's winter:** challenging, with goals (D62).
- **Forecasting:** yes, mostly by looking, with technology and magic adding to it.
- **Which effects of cold and heat come first:** left to me; the order is under [body temperature](#body-temperature).

Nothing is open here now beyond the numbers, which are tuned by playing.
