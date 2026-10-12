# Recruits who live and work at your base

2026-10-12. Question: how do shipped games handle recruited NPCs who live and work at the player's base, and which of them get it working without the player managing every person?

For [tribe.md](../docs/tribe.md). The developers want recruits who work stations, haul, build and defend, and grow by doing, but not a colony sim: no micromanaging, recruits functional on their own (D161).

Sources are mostly wikis, guides and Steam threads, which disagree in places and describe versions that change. The Medieval Dynasty and Kenshi wikis would not load, so those lean on Steam threads and guides. None of the games was played for this note. Points marked (unverified) are from memory.

## The games

### Medieval Dynasty

The developers' named inspiration.

- **Recruiting** costs dynasty reputation, earned mostly from quests. The cost rises with every adult villager you already have, into the tens of thousands late on, which is the only brake on numbers. Children born in the village are free. A house, food, water and firewood must be ready first.
- **Jobs are assigned by the player**, villager to workplace, in a management screen usable from anywhere. Each workplace splits its output across products by percentage. The developers' own words: "If you want your villagers to work, you have to tell them what you want them to do."
- **Skills by doing**, 1 to 10 per family (extraction, hunting, farming, husbandry, crafting, trade). Output rises about linearly with level. A villager is happier working their best skill. Changing a villager's job mid-season costs mood.
- **Production is abstracted.** Work is done whether or not the villager is seen doing it, and the output appears in storage.
- **Two global stores**, food and resources, linked across the whole map. Villagers take tools and inputs from them and put output into them. Nobody hauls.
- **Needs**: a house, prepared food, water, firewood. Mood is raised by a better house, decorations, working their best skill, marriage and children. At the bottom, a villager leaves; they do not starve.
- **Families**: villagers marry, have children, and the children grow into workers.
- **They do not** build, defend or follow you (unverified, but no guide says otherwise).
- **Liked**: a village that grows into a dynasty, and work that goes on calmly.
- **Disliked**: items in the "wrong" chest are invisible to villagers. Keeping storage stocked with axes, knives and hoes as they wear out is a standing chore. How much each job makes is hard to tell.

### Kenshi

- **Recruits** are hired in bars, some for a fee, freed from slavery, or taken from prisons.
- **Jobs are given by the player and then kept**: a permanent job is set by a modified click. Each character has a job list you reorder by dragging. Jobs include mining (which hauls to the nearest store of the right kind), working a machine, farming, hauling, building and repairing.
- **Skills purely by use.** Hauling trains labouring, being overloaded trains strength, and being beaten trains toughness. This is the canonical design for skills by doing.
- **Equipment** is entirely manual.
- **Raids**: factions raid outposts, sometimes early and several at once.
- **Death**: a character is knocked out first. They can bleed out, lose limbs or be taken, and death is permanent.
- **Disliked**: around 30 people it becomes "a micromanagement mess". Players put the injured to bed by hand. The per-character toggle "sleep when wounded" does not always work. Workers leave a station to eat and never return, and get stuck in dead spots in the navigation mesh.

### Soulmask

- **Recruiting**: you beat a person down, keep them alive while their recognition of you builds, then recruit them.
- **Jobs are set per tribesman** through a menu that shows what they suit: a station, gathering at a map marker, hauling, a farm, camp upkeep, a mine, or a patrol route. Tasks are reordered for priority.
- **Proficiencies**: about 14 for work and 8 for weapons. Each tribesman has a cap set by aptitude, and guides agree aptitude matters more than current level. Milestones grant buffs, and defects can be removed as they level.
- **Needs**: food, drink and comforts. Low mood and idleness can end in desertion.
- **Equipment and followers** are set up by hand: gear, food, bandages.
- **Disliked**:
  - Tribesmen sit idle for want of one missing link: a tool, a misplaced marker, an input, or a full store.
  - Every tribesman paths on the server, so lag grows with their number, and guides advise compact bases and lower caps.
  - Some players lost whole bases of workers to wildlife.

### Enshrouded

- **Survivors** are rescued from vaults and summoned home: a blacksmith, a hunter, an alchemist, a carpenter, a farmer, and later others.
- **A survivor is the station menu**: they open stations and recipes. They do not work, haul, build, fight, die or improve.
- **Each needs a bed in shelter**. A placed bed goes to the nearest survivor without one. Some recipes need the survivor sheltered.
- Patches have made bases with many villagers run better. Version 1.0 is due on 15 October 2026, so its notes are worth checking.

### Palworld

The key reference for automatic work.

- **Work suitability**: each kind of creature has fixed levels in a dozen kinds of work, such as kindling, planting, handiwork, mining, lumbering and transporting. These levels are innate, not earned by doing.
- **Work is picked automatically**: every creature at a base does any work it is suited to inside the base, nearest first.
  - The order is one fixed table: defence first, then building, then most work, with transporting last.
  - When a raid comes, everything that can fight drops its work.
  - Overrides are light. Throwing a creature at a station pins it there. A stand can switch kinds of work off for one creature.
- **Building**: you place a plan, and creatures with handiwork build it.
- **Hauling** is the transporters' work. Being last in the table, output lies on the floor in a busy base.
- **Needs**: food from a feed box, a bed each, and sanity. Overwork drains sanity, and low sanity slows and then stops work.
- **Caps**: about 15 to 20 workers a base by default, raised by servers.
- **Liked**: catch the right specialist and the base runs. One strong specialist beats several generalists.
- **Disliked**: pathing above all. Creatures get stuck on stairs and storeys, and starve beside a full feed box. The folk fix is to recall them and send them out again. Pathing has been patched several times, so old complaints may be out of date.

### Bellwright

- **Recruits** come from nearby villages, gated by that village's trust, and cost renown that rises with each one.
- **Roles**: worker, companion who follows and fights, or guard.
- **Jobs use a priority grid**, 1 to 9 per job per villager, with building priorities, production orders and delivery as a job. One guide advises setting delivery low for most workers, because haulers make wasteful trips.
- **Villagers do the building**. The player's own hammering is very slow.
- **Equipment**: gear presets that equip the best item of each kind come later. Before then, everything is equipped by hand, and players wanted the presets sooner.
- **Raids**: a bar fills as you fight and expand, and a call to arms turns workers into soldiers.
- **Verdict**: the closest of the survival games here to a colony sim.

### Dragon Quest Builders 2

- **Residents move in** as you solve each island's problems.
- **Building**: you lay down a blueprint and put materials in a chest, and residents build it from the chest.
- **Farming**: a scarecrow marks a field and is labelled with a crop. Residents then till, plant and water it.
- **Food and comfort**: residents cook and eat from shared kitchens and chests. Rooms (a bedroom, a kitchen, a toilet) meet their needs, and happy residents produce the currency.
- **Defence**: residents fight in defence battles and can join your party.
- **Why it matters**: the clearest case of work without micromanagement. The player says what they want through things in the world, a blueprint, a labelled scarecrow, a chest, rather than through orders to each person.

### Briefly

- **Conan Exiles**: a crafting thrall placed in a station opens recipes and speeds it, and never improves. A fighting thrall guards where it is placed or follows you. Caps per clan were added to cut server lag.
- **V Rising**: servants are sent off screen on timed hunts. Their success follows their power, and they can come back hurt.
- **Sons of the Forest**: Kelvin is given orders from a notepad: fetch logs, sticks or stones, finish a structure (completes your blueprints), clear an area, follow, stay. Downed by enemies, he can be revived; killed by the player, he is dead for good.
- **Fallout 4**: settlers are assigned one at a time to a crop, a guard post or a shop. Settlement-defence quests nag the player endlessly, the standard warning about raids that demand you.
- **Mount & Blade: Bannerlord**: companions fill party roles (quartermaster, scout, surgeon) whose effects follow their skills, which rise by use.
- **RimWorld and Going Medieval**: a grid of colonists by kinds of work with priorities. It is micromanagement because the player is the scheduler: tuning the grid, reacting to shortages, ordering each fighter.

## What they have in common

1. **Recruiting costs something that grows with the headcount**: reputation, renown, recognition. Where recruits come only by rescue or story (Enshrouded, Dragon Quest Builders 2), the supply is the limit.
2. **A bed comes first**, and a new bed goes to the nearest person without one.
3. **Work is chosen in one of three ways**:
   - Automatically, from an innate aptitude (Palworld).
   - By the player, with a reward for using the best skill (Medieval Dynasty).
   - By a priority grid (Bellwright, RimWorld).

   Kenshi and Soulmask sit between: the player sets a job list, and the person works it alone.
4. **Logistics without hauling works best.** Medieval Dynasty's linked stores and Dragon Quest Builders 2's chest take hauling out of play. Where hauling is a job, it is the least reliable work in the game.
5. **Building help is the same everywhere**: the player places the plan, and the people build it from storage.
6. **Defence is one switch**: when an attack comes, everyone who can fight drops their work, and the rest keep away or are ignored.
7. **Needs scale how much gets done**: unhappy people work slowly or leave. They do not starve to death.

## What works without micromanagement

Dragon Quest Builders 2 and Palworld. Both take the player's intent from things placed in the world rather than from menus per person. Both make the sensible choice by default: suitability, the nearest job, everyone fighting when attacked. Both keep one light override: pin this one here.

Medieval Dynasty is calm but not automatic: one assignment per villager, after which they work for seasons. Abstracting the output is what keeps it calm.

What the developers describe is **Palworld's automatic picking, with skills grown by doing in place of fixed aptitude**. None of these games ships exactly that. Its risk is that automatic picking plus growth by doing locks a recruit into whatever job was nearest at the start. Something must pull each person toward what they are best at, as Medieval Dynasty's reward for using the best skill does.

## How it goes wrong

- **Pathing.** The top complaint in Palworld and present in Kenshi: storeys, stairs and clutter, and people starving beside food. In Soulmask, each person's pathing costs the server.
- **Idle for an unseen reason.** A tool in the wrong chest, a full store, one missing input. The guides for Bellwright, Soulmask and Medieval Dynasty are troubleshooting checklists, because the game does not say why someone is idle.
- **Storage.** Items in the "wrong" container are ignored, or the player's best food and tools are used up from shared stores.
- **Hauling and priorities.** Wasteful trips (Bellwright). Haulers who cross the whole map (RimWorld). Output never collected (Palworld). A building plan that pulls every miner off the rock (Palworld).
- **Gear chores.** Restocking tools as they wear (Medieval Dynasty), and equipping each follower by hand (Kenshi, Soulmask, early Bellwright).
- **Scale.** Management breaks around 30 people (Kenshi). Caps per base get added afterwards for server load (Conan Exiles, Soulmask).
- **Unfair deaths.** People lost to the game's own failings: wildlife wiping out workers, a recruit bleeding out while you watch another squad. Players accept a loss more readily when they caused it, and when being downed comes before death.
- **Nagging.** Defence that demands the player's presence again and again (Fallout 4).

## Sources

- Medieval Dynasty:
  - Recruiting: [Steam thread](https://steamcommunity.com/app/1129580/discussions/0/2962768718561041418/), [GameRant guide](https://gamerant.com/medieval-dynasty-villager-recruitment-guide-job-mood/)
  - Assigning work: [Steam thread](https://steamcommunity.com/app/1129580/discussions/0/2993171056497188614)
  - Skills: [Steam thread](https://steamcommunity.com/app/1129580/discussions/0/2950411088488084855)
  - Mood: [Steam thread](https://steamcommunity.com/app/1129580/discussions/0/3373782431179138822)
  - Storage: [Steam thread](https://steamcommunity.com/app/1129580/discussions/0/3842178984960868204/)
  - Families: [Steam thread](https://steamcommunity.com/app/1129580/discussions/0/2952663522484651584)
  - The "wrong chest" problem: [GamingTrend review](https://gamingtrend.com/feature/reviews/medieval-dynasty-review-a-tale-of-many-buckets)
- Kenshi:
  - Recruits: [pindrop](https://pindrop.gg/kenshi/recruits)
  - Jobs: [Steam thread](https://steamcommunity.com/app/233860/discussions/0/3454730619125473279), [hamy.xyz](https://hamy.xyz/blog/2023-08-kenshi-early-game-money-farm)
  - Raids: [Steam thread](https://steamcommunity.com/app/233860/discussions/0/1835685838064323917)
  - Squad size: [Steam thread](https://steamcommunity.com/app/233860/discussions/0/1696043263509997664)
- Soulmask:
  - Tribesmen: [Whisper of the House guide](https://www.whisperofthehouse.com/soulmask/tribesmen-guide)
  - Assigning work: [gamever](https://gamever.io/knowledge-base/how-to-assign-tasks-to-tribesmen-in-soulmask)
  - Tribe management: [low.ms](https://low.ms/blog/soulmask-tribe-management-guide-dedicated-server)
  - Defending the base: [Steam thread](https://steamcommunity.com/app/2646460/discussions/0/4336481387155767770)
- Enshrouded:
  - [wiki.gg NPC page](https://enshrouded.wiki.gg/wiki/NPC)
  - [Carpenter needs shelter](https://enshrouded.wiki.gg/wiki/Quests/Carpenter_Needs_Shelter)
- Palworld:
  - Pals not working: [playerecho](https://playerecho.com/palworld/base-pals-not-working)
  - Pathing: [palworldcompanion](https://palworldcompanion.com/bases/pathing-ai)
  - Sanity: [xgamingserver](https://xgamingserver.com/blog/palworld-sanity-guide/)
  - Bases: [wiki.gg Base page](https://palworld.wiki.gg/wiki/Base)
  - Pathing complaints: [Steam thread](https://steamcommunity.com/app/1623730/discussions/0/592885762520848744)
- Bellwright:
  - Settlements: [bellwrightmap](https://bellwrightmap.xyz/settlement)
  - Priorities: [Steam guide, old build](https://steamcommunity.com/sharedfiles/filedetails/?id=3380034073)
- Dragon Quest Builders 2:
  - Automatic building: [Gematsu](https://gematsu.com/2018/11/dragon-quest-builders-2-introduces-automatic-construction-latest-weapons)
  - Review: [God is a Geek](https://godisageek.com/reviews/dragon-quest-builders-2-review/)
- Others:
  - Conan Exiles: [thrall caps](https://conanexiles.com/?p=9720)
  - V Rising: [servants](https://www.gamerguides.com/v-rising/guide/dominance/servants)
  - Sons of the Forest: [Kelvin](https://sonsoftheforest.wiki.gg/wiki/Kelvin)
  - Fallout 4: [settlements](https://fallout.wiki/wiki/Fallout_4_Settlements)
  - RimWorld: [work](https://rimworldwiki.com/wiki/Work)
