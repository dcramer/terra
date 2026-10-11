# A living world

Status: a direction the developers are considering (D89, Leaning), written up on 2026-10-11 for them to react to. The first section is theirs; everything after it is my proposal, with questions at the end. Nothing here is built, and nothing should be until they have read it.

## What the developers said

From 2026-10-11:

- **Still a sandbox, with some survival**, but focused on **a really living world that you can participate in**: NPCs, monsters, wildlife, other things going on.
- **Portals and other ways to reach other worlds** (D72).
- **No terraforming** (D60), and **no oxygen tanks or science fiction**, at least for now.
- **Closer to Enshrouded**: a sandbox with RPG in it (D85).
- **He Who Fights With Monsters as a core basis** for the mechanics and for how the world works (D87, D88).
- The atmosphere, what is going on in the world, and the design around it are still to be worked out.

## The idea in one line

Proposed: **the world goes on whether or not you are there, and you join in.** Monsters gather, animals roam and hunt, settlements trade, grow and get attacked. Nothing waits for the player to start it, and what players do changes where it goes next.

That replaces the old pitch's engine, a planet you terraform, with a new one: a world that is busy without you.

## What makes it go: magic as the world's weather

In the books, as I recall them (unchecked): magic is everywhere, gathers in places, and where too much gathers, monsters form. Left alone, they multiply and spill out; adventurers keep them down. Monsters leave behind what adventurers live on, including the essences that give powers.

Proposed: make that the simulation at the centre of the game.

- **Magic gathers by place**, faster in some places than others (deep forest, ruins, caverns, near portals), and differently on each world (D59).
- **Where it pools, monsters form**, of the kind the place makes: a swamp makes swamp things. The more it pools, the stronger they get, and when nobody culls them, a **surge** spills out into the land around.
- **Culling them is how you gain**: loot, materials, and essences.
- **Settlements live with it.** A village near a pooling place needs it culled; one that is not helped may be overrun, move, or call for help.
- **Wildlife is apart from it**: ordinary animals with their own lives, mostly not hostile, food and materials, and prey for monsters too.

Why this suits us:

- **It gives the world a reason to be busy** without a writer scripting every event.
- **It is systemic, so it scales with procedural worlds** (D8, D59): a new world is a new theme for magic, monsters and wildlife, not new content written by hand.
- **It gives building a purpose**: a base is a place you hold. Walls, wards and defenders keep the magic around it down, or keep what forms out.
- **It reuses work.** Technical aside: the parked planet simulation (D6, `terra_sim`) is already a model of local values that spread and add up to global ones. Magic gathering by region is the same shape of problem.

## What lives in the world

| | What it is | Proposed role |
|---|---|---|
| **Wildlife** | Animals with routines: graze, hunt, flee, migrate | Food, hide, bone; a world that feels alive; mostly not hostile unless provoked |
| **Monsters** | Formed from pooled magic, ranked by strength | The main threat and the main reward; surges are the big events |
| **NPCs** | People in settlements, on roads, in the wild | Trade, work, quests that come from what is happening, people to recruit (D86) |
| **Cultures** | The peoples of each world, each its own look within the one art style (D86) | What a world teaches you, and who may join you |
| **Events** | Surges, migrations, raids, seasons, weather (D78) | What you notice and choose to answer |

## Where the player fits

Proposed loops, replacing those in [gameplay.md](gameplay.md):

| Loop | What the player does |
|---|---|
| 30 seconds | Move, fight, gather, place a piece. Feels good with nothing else around it. |
| 10 minutes | Go out for something (materials, a monster nest, a request from someone), come back, build or make something with it. |
| Session | Answer something the world is doing: clear a pooling place before it surges, defend a settlement, follow a migration, reach a new portal. Come back stronger. |
| Long | Climb the ranks; hold a region; reach other worlds, each harder than the last. |

**Why build?** From the developers, 2026-10-11 (D90, Leaning): part of survival is establishing bases and outposts; recruited NPCs might help defend them, and built defences might come later. In Enshrouded you build a home, and the people you rescue come to live and work in it. Proposed for us: a base is a **hold** on the world. It is where you make things, where recruited NPCs live and work, and a place the magic around you is kept down. Bigger holds take on bigger stretches of wild.

**Progression in two layers**, which keeps both what the developers have said (D88) and the books:

- **Skills, by doing** (D88): swimming, climbing, chopping, building, smithing, a weapon. Many, small, always improving.
- **Essences and abilities, by finding**, as in the books: rare, chosen, from monsters and places, giving powers that grow through ranks. A few, large, changing how you play.

**Ranks as the spine.** The books' ranks (iron, bronze, silver, gold, diamond) could rank monsters, places, portals and players alike: a bronze-rank portal leads somewhere bronze-rank things live. That gives worlds an order without a story to justify it.

## Atmosphere: three starting points

What the world is like and what is going on. These can combine; the choice is the developers'.

| | The premise | Feels like | Carries |
|---|---|---|---|
| **A. The frontier** | Settled lands at the edge of a wild full of magic. You are a newcomer who makes a life there. | Enshrouded, Valheim, the early books | The everyday: building, adventuring, helping settlements |
| **B. Something is wrong** | Something is spreading: a blight, an invasion from elsewhere, portals opening where they should not. | Enshrouded's Shroud, the books' larger threats | A long arc and a reason worlds get harder |
| **C. You came from elsewhere** | You arrived from another world, as the books' hero did, and see this one fresh. | The books' start; most LitRPG | Explains the skills and ranks on screen as part of the world, and lets the game be funny |

The developers ruled out C on 2026-10-11 and find A and B interesting. My recommendation stands: **A as the everyday, B as the long arc**. A living world needs a daily life worth joining, and a slow-building threat gives sessions a direction without fixing a story the procedural worlds cannot tell.

## What it changes

If adopted, these need revisiting:

- **Tone (D3).** Done: the automation half is replaced by D90, bases and outposts defended by recruits.
- **Survival (D61).** Lighter: health, stamina, hunger, thirst; oxygen only as breath under water (D74).
- **Equipment (D64).** Armour, clothing and packs, not a space suit. The sandbox's placeholder suit, helmet and tank would change.
- **Hostile life ([gameplay.md](gameplay.md)).** Was driven by terraforming; would be driven by magic.
- **The pitch and pillars** ([vision.md](vision.md)).
- **Subnautica** (D85) stays a model for building and the inventory, not for the setting.

## What it costs

Creatures and people are the most expensive thing in the design: they walk on round ground players can dig, find their way, animate and act, and the server has to run them for a thousand players (D52). A living world needs **detail by distance** for life as the terrain has it for ground: full behaviour near players, a cheap summary far away ("this pooling place has grown", "the herd has moved"), and nothing at all where nothing is changing. That is technical and mine to design; the scope is the developers': my suggestion is to start with one world, one kind of wildlife, one kind of monster and one pooling place, and judge whether it feels alive before adding NPC societies.

## Questions for the developers

1. Answered in part: **the premise.** Not C; A and B are interesting. Both, or one?
2. **Magic as the world's weather.** Is pooled magic making monsters the engine you want, or should monsters simply live in places?
3. **How much story?** Hand-written quests and characters, or only what comes out of the world's systems?
4. **Combat.** As central as Enshrouded's, or one way among several to deal with things?
5. **Essences.** Take the books' system closely (a few essences, a confluence, abilities that rank up), or loosely?
6. **Ranks.** For players only, or for monsters, places and portals too?
7. **Settlements.** Can they be overrun and lost? Can players found them? Bases and outposts are part of survival (D90); is an outpost with recruits living in it a settlement?
8. **Who are you?** A newcomer, a native, someone from another world?
9. **Death.** Still no fail state (D18), with what you carry dropped where you fell (D84)?
10. **A thousand players.** In a living world, what do many players do together that a few cannot: defend a city, hold a frontier, answer a surge?
