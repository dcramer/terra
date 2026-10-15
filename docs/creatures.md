# Creatures: wildlife and monsters

Part of the game's design ([gameplay.md](gameplay.md), adopted as the source of truth on 2026-10-12, D89). Each item is as settled as the decision it cites; the rest is a proposal for the developers to react to.

## What the developers have said

- A living world with monsters and wildlife (D89).
- Factions and settlements, after Kenshi (D95); the world goes on without you.

## Based on

**Kenshi** and **Valheim** for animals that live their own lives and react to you. The lore, after *He Who Fights With Monsters*'s world-building, for monsters that form where magic pools ([lore.md](lore.md#monsters)).

## Wildlife

- **Animals with routines**: graze, drink, rest, flee, hunt, gather in herds, move with the seasons.
- **A food chain**: grazers, the predators that eat them, scavengers. Monsters hunt animals too.
- **Mostly not hostile.** Most flee; some defend themselves or their young; predators hunt what is weak, including a lone player.
- **What they give**: meat, hide, bone, feathers. Hunting is a skill by doing (D88).
- **Some can be tamed**, probably (D100): ridden as mounts, perhaps put to work or kept for what they give.
- **Populations live in regions** and recover when left alone, as the ground heals outside claims (D94). Over-hunting empties a region for a while, not forever.

## Monsters

- **They form where magic pools.** The place decides the kind: a marsh makes marsh things, a ruin its own. The more it pools, the higher their rank.
- **Left alone they grow**, spread, and surge: they spill into the land around, onto roads, settlements and claims.
- **They fight**: they are what combat is mostly against ([combat.md](combat.md)).
- **What they leave**: materials found nowhere else; cores (D165), which hurry powers, run machines and calm pools; spirit coins, the money that is magic (D167); quintessence, fragments of their kind for workings and items (D168); and sometimes essences ([magic.md](magic.md#essences)). Culling them is a living ([people.md](people.md#trade)).
- **Pooling places can be cleared**, and stay quieter for a while; holding one in a claim keeps it down (D94).

## Kinds, by rule

A small set of body plans (a four-legged runner, a flier, a crawler, a big lumbering thing, a swarm), each varied by size, look, behaviour and rank by the world's theme (D59). A new world brings new kinds without new code. Players learn each kind by meeting it, as with factions (D95): no bestiary is filled in for you (D92).

## Designed kinds

Monsters the developers have designed, each its own kind (D250). Their models are in `assets/models/creatures`, made from concepts in `art/props`; none is rigged, animated or in play yet.

| Kind | What it is | Where |
|---|---|---|
| Worman | A giant earthworm standing as tall as a person, disguised as a traveller: a blue cap, round dark glasses, red lips, two thin arms and a knife. It attacks anyone who comes close | Wherever travellers go |
| Drillhog | A groundhog the size of a boar, with drills growing from its back and head, which it digs and attacks with | Dungeons and places underground |
| Seawish | A fish as tall as a person, standing on its tail, with a thick mop of seaweed growing on its head; it fights with a rusty sword and its seaweed. In the water it passes for a floating clump of seaweed | Beaches |

## What it asks of the engine

Technical, mine. Creatures are the most expensive thing in the design: they walk round, diggable ground, find their way, animate and act. So creatures are simulated by how near the nearest player is (D145): every tick close by, thinking less often further out, and waiting where they are beyond; a region's animals and a pooling place's strength are a few numbers when nobody is near; and small behaviours combine, rather than one large brain per creature.

## Questions

1. Answered in part: **taming and mounts**, probably yes (D100). How taming works, and which animals, is open.
2. **Monsters among themselves.** Do they fight each other and the wildlife where they meet?
3. **Bosses.** Are there great monsters at the heart of the worst pooling places? The organisation across realms may be behind some (D172).
4. **Night.** Are monsters more active, or different, after dark ([survival.md](survival.md))?
