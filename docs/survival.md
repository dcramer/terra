# Survival

Status: the developers said on 2026-10-10 that there are survival-coupled attributes, that they may vary per world, and which ones to start with. How each works is a proposal below. Nothing is built.

## The attributes

From the developers: **health, stamina, oxygen, hunger, thirst**, and possibly more later.

| Attribute | Goes down when | Comes back with | When it runs out |
|---|---|---|---|
| Health | You are hurt: falls, cold, heat, poison, creatures | Rest, food, medicine | You are down (see D18: no fail state) |
| Stamina | You sprint, jump, swing a tool, swim | Resting a moment | You cannot sprint, swim hard or work hard |
| Oxygen | Always, where the air cannot be breathed: under water, and on planets with thin or no air | Breathable air, the space suit's tank, a base with air | Health falls fast |
| Hunger | Slowly, all the time; faster when working | Food | Health falls slowly |
| Thirst | A little faster than hunger | Water | Health falls |

The rates and refills are placeholders, written to have something to react to.

## Oxygen

From the developers: being under water uses up oxygen, in the same way as being on a planet with little or no atmosphere (D74). So oxygen is one rule, "is the air here breathable, and how much", that water, thin air and vacuum all answer. A space suit, made of parts such as a suit and an oxygen tank (D64), is how you carry air with you; a bigger tank lasts longer.

## How they vary by world

An attribute is one rule plus numbers, and a planet's theme sets the numbers ([worlds.md](worlds.md)). On the starter planet the air is good, so oxygen never falls outdoors. On an airless planet it falls everywhere outside a base. A hot planet might make thirst fall faster; a heavy one might make stamina fall faster. Adding an attribute later should be adding one rule, which every planet then sets numbers for.

## How it fits what was already written

[gameplay.md](gameplay.md) has every survival need follow one arc: carried by hand, then solved by a machine at your base, then, on a planet you terraform, solved by the planet itself. The first two steps still hold; the last is parked with terraforming (D60). The older list of needs there (oxygen, water, food, temperature, radiation) maps onto the attributes: water is thirst, food is hunger, and temperature and radiation are things about a place that wear down health.

## In the game's model

Each attribute is a component on whoever has it: the player, and later creatures. What changes them is a system that reads where you are (the air, the temperature) and what you are doing. The server decides them, since a thousand players cannot be trusted to report their own health (D51, D52).

## Open questions

1. **What happens at zero?** D18 says no fail state. Is "down" a respawn at base, a loss of what you carry, or something gentler?
2. **How hard is it?** D3 sets the tone between Planet Crafter and Vintage Story. On the starter planet, does hunger matter at all?
3. **Is stamina a survival attribute or a movement one?** It refills in seconds, unlike the others.
4. **Which come next?** Temperature and radiation as attributes of their own, or as things that drain health?
5. **Are they shown as meters**, as in Subnautica, or worked into the world?
