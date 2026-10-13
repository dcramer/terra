# Gameplay: a living world

The design of the game as a whole: what kind of game it is, what goes on in its realms, and what the player does there. The developers adopted it on 2026-10-12 as the source of truth for design (D89): after [decisions.md](decisions.md), this page and the system pages it links win over any other doc. It began as a numbered proposal of 2026-10-11 and replaced this page's earlier draft (terraforming loops, in git history). What the game is made of, first and foremost, is an engine of deep, general systems across procedural worlds (D187, [vision.md](vision.md#what-we-are-building)); this page is what those systems make together. "A living world" is a working name.

Status: the direction is Decided (D89). Within it, each item is as settled as the decision it cites; the rest is proposal, with questions at the end. The foundational lore every system rests on is in [lore.md](lore.md).

## What the developers said

From 2026-10-11:

- **Still a sandbox, with some survival**, but focused on **a really living world that you can participate in**: NPCs, monsters, wildlife, other things going on.
- **Portals and other ways to reach other worlds** (D72).
- **No terraforming** (D89), and **no oxygen tanks or science fiction**, at least for now.
- **Closer to Enshrouded**: a sandbox with RPG in it (D85).
- **He Who Fights With Monsters as a core basis** for the mechanics and for how the world works (D87, D88).
- The atmosphere, what is going on in the world, and the design around it are still to be worked out.

## Based on

**The foundational lore comes from *He Who Fights With Monsters*, as world-building, not as content** (D125, restated by the developers on 2026-10-12). We use the books' fundamental systems of how the universe works: magic everywhere that pools and makes monsters, essences that give powers, ranks that everything climbs, realms joined by ways between them. We do not reproduce the books item for item: their specific essences, abilities, peoples, places, gods, names and story are theirs, and ours are invented, drawing heavily on literature and game design. [lore.md](lore.md) writes the fundamentals out in our own words, as the basis for every system here.

Open. The developers want each design based on a game they know they enjoy, so its core mechanics are known to be fun. A comparison of candidates is in [research/reference-games.md](../research/reference-games.md); its suggestion is **Enshrouded for how it plays, Soulmask for what you are doing**: Enshrouded's movement, fighting, building and rescued survivors, with Soulmask's recruits who work and defend, outposts at resources, and attacks provoked by your activity. Kenshi is the reference for a world that goes on without you.

## The idea in one line

Proposed: **the world goes on whether or not you are there, and you join in.** Monsters gather, animals roam and hunt, settlements trade, grow and get attacked. Nothing waits for the player to start it, and what players do changes where it goes next.

That replaces the old pitch's engine, a planet you terraform, with a new one: a world that is busy without you.

## How it all works

This overview gives the idea, what drives the world, and the principles. Each system has its own proposal:

| System | Proposal | In short |
|---|---|---|
| Combat | [combat.md](combat.md) | A fast-paced action RPG with Enshrouded as the baseline (D180), on stamina; weapons as skills by doing; abilities from essences; rank decides more than skill |
| Creatures | [creatures.md](creatures.md) | Wildlife with routines and a food chain; monsters that form where magic pools, kinds made by rule |
| Magic | [magic.md](magic.md) | Open (D163): three essences and a confluence, awakening as a weighted draw of generated powers, attributes (D164), mana (D102), cores (D165), combat styles and crafts from skill books, old-school spellcraft |
| Progression | [progression.md](progression.md) | Three ways to grow (D124): skills by doing, techniques by study, powers from essences; skills on the rank ladder (D159); crossing a rank (D126); research and unlocks (D68) |
| Resources, tools and crafting | [crafting.md](crafting.md) | Materials by region and rank, one tool to a job, stations in claims run by recruits, wear and repair (open) |
| People | [people.md](people.md) | Settlements with days, quests as what people say, recruits, trade |
| Your tribe | [tribe.md](tribe.md) | A tribe of players and recruits, its claims its outposts (D166); recruits work on their own however the simulation makes sense (D161, D162): not a colony sim; carrying to builds, taking their own gear, defending the claim |
| Survival | [survival.md](survival.md) | Needs that reward more than punish, day and night, death and coming back at a hearth (D171) |
| Climate | [climate.md](climate.md) | Leaning: seasons (D149) and weather as swings in temperature, daylight and precipitation that everything responds to; body temperature (D153) |
| Biomes | [biomes.md](biomes.md) | How a world fills with life, from grass to canopy (D148) |
| Your character | [player.md](player.md) | No class, creation (D111), moving and getting around big round worlds, the map, what is on screen and what is not |
| Lore | [lore.md](lore.md) | How the universe works, from the books' world-building in our own words (D125); threats and an organisation across realms (D172) |
| The arc | [arc.md](arc.md) | The fiction, the first hour without a tutorial, the long game by rank and the spread, playing together |
| Settings | [settings.md](settings.md) | Leaning (D116): every realm's theme fills the same gameplay roles, as the art's palette roles do; rank is the only power axis; a setting is a way of using magic |
| A realm of machines | [settings.md](settings.md#a-worked-example-the-techno-realm) | Open, one setting worked through: magitech whose machines run on cores, cannot grow, and are best at holding a place; no guns |
| Building and claims | [building.md](building.md) | Walls, floors and roofs of half-metre cubes on the base's grid (D152, D181), placed as The Sims does (D154); claims, outside which the world heals (D94), starting at a size that can be upgraded (D104) |
| The MVP | [prototypes.md](prototypes.md#mvp-the-first-playable) | Leaning (D105): the build order for a first playable that tests the core loop, not the game's scope |
| Items | [items.md](items.md) | The backpack, equipment and action bar (D84) |

## What makes it go: magic as the world's weather

From the lore ([lore.md](lore.md), after the books' world-building): magic is everywhere and gathers in places, and where too much gathers, monsters form. Left alone, they multiply and spill out, and people whose work it is keep them down. Monsters leave behind what those people live on, including the essences that give powers.

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

## Factions and settlements, after Kenshi

From the developers, 2026-10-11 (D95): Kenshi has many of the concepts we like for how the world exists: villages with things going on, and factions. We will need systems like these. Kenshi also lets you capture people; the developers ruled that out for players, along with keeping prisoners: it sets the wrong tone. Hostile settlements may hold prisoners of their own, though, and rescuing them is one way to recruit (D90), alongside helping settlements.

What Kenshi does, from memory and unchecked: the world is shared among factions, each with its towns, people and attitudes to the others and to you. Towns have shops, bars, guards and residents. Caravans, patrols, raiders and wanderers travel between them on their own errands. Raids come to towns and to the player's outposts, more as an outpost grows. What happens sticks: a town can be overrun, abandoned or taken over, and stays that way.

Proposed for us:

- **Factions are data**, like everything that varies by world (D59): a culture's peoples (D86), raiders, orders, monster-worshippers, merchants. Each has places, aims and attitudes to the others and to you, and those attitudes move with what you do. From the developers: **no fixed set or number**; more are added as we go, and, as in the great procedural games, **players discover them**, learning about each only as they meet it.
- **Settlements are places with needs**: food, safety, trade, people. Magic pooling nearby (see above), raids and failed harvests press on them; help, trade and defence relieve them. A settlement can grow, shrink, fall or be rebuilt, and stays as it is left. From the developers: **settlements can be lost for good**, which is fine with many worlds and realms to go to, and systems can repopulate land if needed.
- **People travel**: traders between settlements, patrols, raiders, refugees from a fallen village. Roads are worn by them, and they are what you meet on the way.
- **What happens sticks**, as in Kenshi: the world remembers. A settlement you saved is there next time; one you did not may be ruins.
- **Your standing** with each faction is earned and lost by what you do. How much of it is on screen follows D93.
- **The tone is gentler than Kenshi's** (D3): its world is bleak, with slavery and starvation at its heart. Ours can be dangerous without that.

Technical (mine): Kenshi is single-player and runs mostly what is near you. With a thousand players (D52), factions and settlements run as a cheap summary on the server, a few numbers per settlement and route, with full detail only where players are. That is the same detail-by-distance as for monsters and wildlife (see What it costs).

## Where the player fits

Proposed loops:

| Loop | What the player does |
|---|---|
| 30 seconds | Move, fight, gather, place a piece. Feels good with nothing else around it. |
| 10 minutes | Go out for something (materials, a monster nest, a request from someone), come back, build or make something with it. |
| Session | Answer something the world is doing: clear a pooling place before it surges, defend a settlement, follow a migration, reach a new portal. Come back stronger. |
| Long | Climb the ranks; hold a region; reach other worlds, each harder than the last. |

**Why build?** From the developers, 2026-10-11 (D90, Leaning): part of survival is establishing bases and outposts; recruited NPCs might help defend them, and built defences might come later. In Enshrouded you build a home, and the people you rescue come to live and work in it. Proposed for us: a base is a **hold** on the world. It is where you make things, where recruited NPCs live and work, and a place the magic around you is kept down. Bigger holds take on bigger stretches of wild.

**Progression in three layers** (D124, Leaning): skills by doing (D88), many and small; techniques by study, from skill books and teachers; and powers from essences, rare and large, changing how you play. See [progression.md](progression.md#three-layers) and [magic.md](magic.md).

**Ranks as the spine** (D159): iron, bronze, silver, gold and diamond rank monsters, places, portals, materials and people alike, so a bronze-rank portal leads somewhere bronze-rank things live. That gives worlds an order without a story to justify it.

**Recruiting.** Mostly by rescuing people, or as something close to a quest reward for helping their settlement (D90). Recruits live in your tribe, work on their own and help defend it: see [people.md](people.md#recruits) and [tribe.md](tribe.md).

## No hand-holding

From the developers, 2026-10-11 (D92): a principle of the game is that it does not hold your hand. There might be quests, but not a tracked quest log that points you where to go.

Proposed, for how quests and finding things then work:

- **Quests are what people say.** Someone tells you their village lost its smith to something in the hills to the east, past the split rock. You remember it, or write it down, and go.
- **Directions are given by the world.** Landmarks, rivers, ridges, the sun, roads, other settlements. Technical: procedural worlds need to know their own landmarks to name them in directions; that is generation work, mine to plan.
- **The map is yours.** It shows what you have seen and what you mark on it, not where a quest wants you (D76).
- **What the world is doing is noticed, not announced.** Smoke on the horizon, animals fleeing, a glow where magic pools, a settlement's bell.
- **Help is in the world, if anywhere.** A recruit who knows the region, a guide, a sign by a road.
- **No journal** (D92). What you were told is yours to remember.

**Realistic but fun** (D122, from the developers): the core tenet. Things work as they would in the world, so players can reason about them; where realism would only be a chore, fun wins.

**Hundreds of variants, not millions** (D182, from the developers): a core tenet. Almost everything is modular and procedural, but thematic, as Borderlands' guns and No Man's Sky's planets are: made from parts by rules, with a theme (a realm, a maker, a people) shaping each. Hundreds or thousands of variants of a kind that each matter, not infinite low-value ones made to be numerous.

**Rich systems, little on screen** (D93, from the developers): the game is rich with systems but somewhat realistic; you do not get a magic display of every piece of information, though some compromises keep it fun. What your body would feel is on screen, and your own skills, ranks and powers read on a character screen; the world's information you have not learned, such as a monster's rank or what an unknown plant does, is learned by looking, and skills sharpen it. What is shown and what is not is in [player.md](player.md#what-is-on-screen).

## Why explore, and why settle in more than one place

From the developers, 2026-10-11 (D91): the player must have reasons to explore, and to need to settle in more locations. Resources and fast travel were floated; the mechanism is open.

Proposed, as levers that work together. The first three are the core; the rest add to them.

| Lever | How it works | Why it makes you go |
|---|---|---|
| **Resources by region** | Materials belong to places: a kind of rock to one terrain, a wood to one forest, the best to deep caverns, far regions and higher ranks. Each world has its own (D59). | What you need next is somewhere you have not been. |
| **Held ground** | You can build, craft at stations and rest safely only on ground you hold, around a base or outpost, as within the build area of Enshrouded's altars (160 metres on a side once upgraded). Held ground is where the magic is kept down. | To work somewhere, you have to settle there. |
| **Travel is earned** | There is no fast travel between places you hold (D72). Quick travel comes from magic (astral magic's markers, recall and gates, [magic.md](magic.md)), portals players build, and mounts. | Distance stays real; going far is a choice and a skill. |
| **Carrying is limited** | The backpack is small and nothing stacks (D84), so hauling far is slow. An outpost by a resource lets you work it where it is. | A reason to settle near what you need, not haul it home. |
| **People live in places** | Those you can rescue or help are in settlements spread through the wild, and cultures differ by region and world (D86). | The recruits you want are far away. |
| **Magic sites** | Places where magic pools are dangerous, because monsters form there, and valuable, for what can be made or opened there. Holding one keeps it from surging and lets you use it. | Risk and reward drawn on the map. |
| **Each realm's pressures, and an organisation across them** (D172) | Every realm has its own threats: its conditions, monsters and events. A dangerous organisation works across realms, feeding pools and binding monsters ([lore.md](lore.md#threats)). | Holding ground matters, and their trail leads from realm to realm. |

**There is no fast travel between claims** (D72): quick travel comes from magic, built portals and mounts, and what you carry goes with you. So outposts are for working and holding places; the small backpack (D84) is what makes it worth working a resource where it is.

What it asks of the engine, technical and mine: held ground is a few numbers per outpost, cheap to store and send; resources by region are a function of the seed and the place, like everything else (D47). With a thousand players (D52), who may hold what, and whether held ground is shared, are the ownership questions in [multiplayer.md](multiplayer.md).

## Atmosphere

Answered on 2026-10-12 (D172): **the frontier as the everyday**, settled lands at the edge of a wild full of magic, with **threats that vary by realm and a dangerous organisation across them** in place of a single spreading threat ([lore.md](lore.md#threats)). Arriving from another world, as the books' hero does, was ruled out.

## What it changed

Brought in line when it was adopted, on 2026-10-12:

- **Tone (D3):** the automation half was replaced by D90, bases and outposts defended by recruits.
- **Survival (D61):** lighter. Health, stamina, hunger, thirst and body temperature (D153); oxygen only as breath under water (D74). [survival.md](survival.md) is now this design's.
- **Progression:** research stays (D68); how it works is proposed as learning by finding rather than a tree of technology ([crafting.md](crafting.md#learning-what-to-make), [progression.md](progression.md)).
- **The pitch and pillars** ([vision.md](vision.md)).
- **Hostile life:** driven by magic, not terraforming ([creatures.md](creatures.md)). The terraforming phases moved to [planet-sim.md](planet-sim.md), parked with it (D89).
- **Worlds are realms** (D17, D87), with no fixed number, not a dozen planets.
- **Subnautica** (D85) stays a model for building and the inventory, not for the setting.
- **Tools** (D96): no science-fiction toolset and no single harvesting tool; one tool to a job ([crafting.md](crafting.md#tools)).
- **Equipment** (D64): armour, clothing and packs, not a space suit.

## What it costs

Creatures and people are the most expensive thing in the design: they walk on round ground players can dig, find their way, animate and act, and the server has to run them for a thousand players (D52). A living world needs **detail by distance** for life as the terrain has it for ground: full behaviour near players, a cheap summary far away ("this pooling place has grown", "the herd has moved"), and nothing at all where nothing is changing. That is technical and mine to design. The order of work is the MVP (D105, Leaning, [prototypes.md](prototypes.md#mvp-the-first-playable)): one world, one kind of wildlife, one monster, one settlement, judged for whether it feels alive before more is added. It is a build order, not the game's scope (D187).

## Questions for the developers

1. Answered: **the premise.** The frontier, with threats by realm and an organisation across them (D172).
2. Answered: **monsters form where magic pools**, and there is ordinary wildlife besides (D103). Raids on claims are not a concern for now.
3. **How much story?** Hand-written quests and characters, or only what comes out of the world's systems?
4. Answered: **combat** is important (D97), and Enshrouded is its baseline, a fast-paced action RPG (D180).
5. Answered: **essences.** The books' system is the baseline, tweaked where the game needs (D98); proposed in [magic.md](magic.md) (D163, Open).
6. Answered: **ranks** are for everything: people, monsters, places, portals, materials (D159).
7. **Settlements.** Can players found them? Settlements can be lost for good (D95); is an outpost with recruits living in it a settlement?
8. **Who are you?** A newcomer to the frontier, or someone born there? (Not someone from another world.)
9. Answered: **death.** No fail state (D18): it is a persistent sandbox. What you carry drops where you fell, and dropped items decay and disappear after a while (D84).
10. **A thousand players.** In a living world, what do many players do together that a few cannot: defend a city, hold a frontier, answer a surge?
11. Answered: **fast travel with materials.** What you carry goes with you; but there is no fast travel between claims, only travel by magic, built portals and mounts (D72).
12. **Held ground.** Is building and crafting allowed only on ground you hold, as in Enshrouded?
13. **Can held ground be lost** when an outpost falls, and does the wild take it back?
14. Answered: **no journal.** What to remember is up to the player (D92).
15. **The map.** May players mark it freely? Does it show anything you have not seen?
16. Answered in part: **knowing your progress.** There will be some on-screen display and character stats (D93). Open: whether other things' ranks, a monster's for one, are shown or must be judged.
17. Answered in part: **the base game.** Enshrouded for how it plays (D85, D180) and Kenshi for how the world exists (D95); Medieval Dynasty for the tribe (D161). Open: whether Soulmask is the reference for what you are doing.
18. Answered in part: **factions.** No fixed set or number; added as we go and discovered by players (D95). Open: can a player found or lead one?
19. Answered in part: **settlements that fall** can be lost for good, with repopulation if needed (D95). Open, to decide later: can players take one over, for instance by wiping it out and claiming the land?
20. Answered: **players do not capture or keep prisoners**; it sets the wrong tone. Hostile settlements may hold prisoners you can rescue (D95).
21. Answered: **how many worlds.** No fixed limit: realms, not a dozen planets (D17), each theme filling the same gameplay roles so any number fit one lore (D116).
