# Creatures: wildlife and monsters

Part of [game design 001](README.md). A proposal for the developers to react to; nothing here is decided unless it cites a decision.

## What the developers have said

- A living world with monsters and wildlife (D89).
- Factions and settlements, after Kenshi (D95); the world goes on without you.

## Based on

**Kenshi** and **Valheim** for animals that live their own lives and react to you. **He Who Fights With Monsters**, as I recall it, for monsters that form where magic pools ([README](README.md#what-makes-it-go-magic-as-the-worlds-weather)).

## Wildlife

- **Animals with routines**: graze, drink, rest, flee, hunt, gather in herds, move with the seasons.
- **A food chain**: grazers, the predators that eat them, scavengers. Monsters hunt animals too.
- **Mostly not hostile.** Most flee; some defend themselves or their young; predators hunt what is weak, including a lone player.
- **What they give**: meat, hide, bone, feathers. Hunting is a skill by doing (D88).
- **Populations live in regions** and recover when left alone, as the ground heals outside claims (D94). Over-hunting empties a region for a while, not forever.

## Monsters

- **They form where magic pools.** The place decides the kind: a marsh makes marsh things, a ruin its own. The more it pools, the higher their rank.
- **Left alone they grow**, spread, and surge: they spill into the land around, onto roads, settlements and claims.
- **They fight**: they are what combat is mostly against ([combat.md](combat.md)).
- **What they leave**: materials found nowhere else, sometimes essences ([essences.md](essences.md)), and the means to trade ([people.md](people.md)).
- **Pooling places can be cleared**, and stay quieter for a while; holding one in a claim keeps it down (D94).

## Kinds, by rule

A small set of body plans (a four-legged runner, a flier, a crawler, a big lumbering thing, a swarm), each varied by size, look, behaviour and rank by the world's theme (D59). A new world brings new kinds without new code. Players learn each kind by meeting it, as with factions (D95): no bestiary is filled in for you (D92).

## What it asks of the engine

Technical, mine. Creatures are the most expensive thing in the design: they walk round, diggable ground, find their way, animate and act. So: full behaviour only near players; a region's animals and a pooling place's strength as a few numbers when nobody is near; small behaviours that combine, rather than one large brain per creature.

## Questions

1. **Taming and mounts.** Can animals be tamed, ridden or put to work, as in Kenshi or Soulmask?
2. **Monsters among themselves.** Do they fight each other and the wildlife where they meet?
3. **Bosses.** Are there great monsters at the heart of the worst pooling places?
4. **Night.** Are monsters more active, or different, after dark ([survival.md](survival.md))?
