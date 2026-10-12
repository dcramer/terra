# People: NPCs, quests, recruits and trade

Part of the game's design ([gameplay.md](gameplay.md), adopted as the source of truth on 2026-10-12, D89). Each item is as settled as the decision it cites; the rest is a proposal for the developers to react to.

## What the developers have said

- There are NPCs (D70). Villages have things going on, and there are factions, discovered as you go (D95).
- Recruits come mostly by rescue, or as something like a reward for helping their settlement (D90). Hostile settlements may hold prisoners you can rescue; players do not capture or keep prisoners (D95).
- Quests may exist, but no tracked quest log and no journal (D92).
- **Recruits have their own progression** (D123, 2026-10-11): at least those who join you grow, with a progression system of their own. Whether other people do is open.
- **Recruits live in your tribe and work on their own** (D161, D162, 2026-10-12): stations, hauling and carrying to builds, however the simulation makes sense, without micromanaging; not a colony sim. They can die. A tribe is the people of a claim, players included (D166). What they do once they have joined is in [tribe.md](tribe.md).

## Based on

**Kenshi** for people with their own lives in a world that goes on. **Enshrouded** for rescued people who come to live and work at your base. **Soulmask** for recruits who work and defend. **Medieval Dynasty** for a village that grows around you (D161).

## Life in a settlement

- **People have days**: work, eat, sleep, gather, keep watch. A settlement's needs ([README](gameplay.md#factions-and-settlements-after-kenshi)) show in what they do and say.
- **Travellers** go between settlements: traders, patrols, pilgrims, refugees. You meet them on roads.

## Talking, and quests

- **People say what is going on**, drawn from the world itself: a pooling place in the hills, a missing hunter, a raid last week, the price of iron. Quests are requests among those words.
- **Requests come from needs**: clear something, find someone, bring something, defend a place, rescue prisoners. Directions are given by landmarks (D92).
- **Nothing is tracked.** You remember, or you mark your own map (D76).
- **Your standing** with a settlement and its faction rises and falls with what you do for and against them (D95).

## Recruits

- **Who**: people you rescued, or who chose to follow you for what you did for their settlement. You bring them back to your claim safely (D161).
- **Where**: they live in your claims (D94), and need a bed, food and safety.
- **What they do**, by what they are good at: run a station ([crafting.md](crafting.md)), gather, farm, haul, guard (D90). They take work on their own, without being managed (D161, D162); how is in [tribe.md](tribe.md). They will travel and fight beside you, later (D161).
- **What they become** (D123): they improve at their work by doing it, as you do (D88). Proposed: on the same ladder as your skills ([essences.md](essences.md#skills-on-the-rank-ladder)), so a recruit has a rank in what they do, such as Bren a bronze-rank smith, and what they can make follows it. A recruit you bring along grows; one you rescue arrives with what they had already learned. Their skills read on their own sheet, like yours (D93).
- **How many**: limited by what your claims can house and feed.

## Trade

- **Shops and traders** in settlements; travelling traders on the roads.
- **Coin and barter.** In the books, as I recall (unchecked), monsters leave behind coin-like spirit coins that adventurers live on. Proposed: a currency that comes from monsters, so culling them is also a living ([creatures.md](creatures.md)).
- **Prices follow need**: iron is dear where there is none.

## What it asks of the engine

Technical, mine. People speak from what the world knows, so their lines are made from the world's facts and the settlement's state, not written one by one. Settlements and travellers far from players are a cheap summary on the server; full lives only near players.

## Questions

1. Answered: **companions.** Recruits can travel and fight beside you, as a future to-do (D161).
2. Answered: **recruits can die** (D161). How is proposed in [tribe.md](tribe.md#when-a-recruit-dies).
3. **How deep is talk?** Short lines and requests, or real conversations?
4. **Currency**: one coin across all worlds, or each world its own?
