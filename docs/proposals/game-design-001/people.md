# People: NPCs, quests, recruits and trade

Part of [game design 001](README.md). A proposal for the developers to react to; nothing here is decided unless it cites a decision.

## What the developers have said

- There are NPCs (D70). Villages have things going on, and there are factions, discovered as you go (D95).
- Recruits come mostly by rescue, or as something like a reward for helping their settlement (D90). Hostile settlements may hold prisoners you can rescue; players do not capture or keep prisoners (D95).
- Quests may exist, but no tracked quest log and no journal (D92).
- **Recruits have their own progression** (D123, 2026-10-11): at least those who join you grow, with a progression system of their own. Whether other people do is open.

## Based on

**Kenshi** for people with their own lives in a world that goes on. **Enshrouded** for rescued people who come to live and work at your base. **Soulmask** for recruits who work and defend.

## Life in a settlement

- **People have days**: work, eat, sleep, gather, keep watch. A settlement's needs ([README](README.md#factions-and-settlements-after-kenshi)) show in what they do and say.
- **Travellers** go between settlements: traders, patrols, pilgrims, refugees. You meet them on roads.

## Talking, and quests

- **People say what is going on**, drawn from the world itself: a pooling place in the hills, a missing hunter, a raid last week, the price of iron. Quests are requests among those words.
- **Requests come from needs**: clear something, find someone, bring something, defend a place, rescue prisoners. Directions are given by landmarks (D92).
- **Nothing is tracked.** You remember, or you mark your own map (D76).
- **Your standing** with a settlement and its faction rises and falls with what you do for and against them (D95).

## Recruits

- **Who**: people you rescued, or who chose to follow you for what you did for their settlement.
- **Where**: they live in your claims (D94), and need a bed, food and safety.
- **What they do**, by what they are good at: run a station ([crafting.md](crafting.md)), gather, farm, haul, guard (D90). Some might travel with you.
- **What they become** (D123): they improve at their work by doing it, as you do (D88). Proposed: on the same ladder as your skills ([essences.md](essences.md#skills-on-the-rank-ladder)), so a recruit has a rank in what they do, such as Bren a bronze-rank smith, and what they can make follows it. A recruit you bring along grows; one you rescue arrives with what they had already learned. Their skills read on their own sheet, like yours (D93).
- **How many**: limited by what your claims can house and feed.

## Trade

- **Shops and traders** in settlements; travelling traders on the roads.
- **Coin and barter.** In the books, as I recall (unchecked), monsters leave behind coin-like spirit coins that adventurers live on. Proposed: a currency that comes from monsters, so culling them is also a living ([creatures.md](creatures.md)).
- **Prices follow need**: iron is dear where there is none.

## What it asks of the engine

Technical, mine. People speak from what the world knows, so their lines are made from the world's facts and the settlement's state, not written one by one. Settlements and travellers far from players are a cheap summary on the server; full lives only near players.

## Questions

1. **Companions**: can recruits travel and fight with you, or do they stay home?
2. **Recruits dying**: can they be lost for good?
3. **How deep is talk?** Short lines and requests, or real conversations?
4. **Currency**: one coin across all worlds, or each world its own?
