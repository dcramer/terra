# Survival: needs, rhythms and death

Part of the game's design ([gameplay.md](gameplay.md), adopted as the source of truth on 2026-10-12, D89). Each item is as settled as the decision it cites; the rest is a proposal for the developers to react to. It replaced the earlier survival page, whose oxygen tanks and terraforming are gone (D89).

## What the developers have said

- Survival attributes: health, stamina, hunger, thirst, and breath, used up only under water (D61, D74). No oxygen tanks or science fiction for now (D89).
- A sandbox with some survival in it; part of survival is establishing and holding bases and outposts (D90).
- Mana as well as stamina (D102): a pool of its own for powers ([magic.md](magic.md#mana)).
- No fail state (D18). What you carry drops when you die, and dropped items decay after a while (D84). You come back at a hearth (D171).

## Based on

**Valheim** and **Enshrouded** for survival that rewards rather than punishes: food that makes you stronger, rest that restores you, cold and dark that you prepare for.

## Needs

| Need | Met by | When it is low |
|---|---|---|
| Hunger | Food: hunted, fished, gathered, farmed, cooked | Health and stamina recover slowly and their limits shrink, and you are weaker; once the gauge is empty, health wears away slowly, over a few days, until starving kills: three in-game days, an hour and a half of play (D101) |
| Thirst | Water: springs, rivers, wells, a waterskin | The same, faster: two in-game days (D101) |
| Breath | Air: only under water (D74) | Health falls fast |
| Warmth | Fire, shelter, clothing, work | Body temperature (D153): too cold raises hunger and stamina costs, too hot raises thirst and stamina costs, and far out of bounds either wears health down ([climate](climate.md#body-temperature)) |
| Rest | A bed or a hearth in a claim | Proposed: rested, you recover faster for a while. Resting at a hearth also binds you to it, so it is where you come back (D171) |
| Mana | Rest, sitting still, and food (D102) | Powers cannot be used; how fast it returns is set by Recovery (D164, [magic.md](magic.md#mana)) |

**Food is essential** (D101): it is what keeps you from dying; going without makes you weaker, and then kills you, slowly, over days, so it is a hard mistake to make. Recruits starve the same way ([tribe.md](tribe.md#needs)). **Good food may also give buffs**, as in Valheim and Enshrouded; whether, and what they are, the developers will decide later.

## Rhythms

- **Day and night.** Night is darker, colder and more dangerous: more monsters, and stranger ones ([creatures.md](creatures.md)). Light matters: torches, fires, lit claims. Proposed (D134): a whole day lasts half an hour of play, as Valheim's and Enshrouded's do, a little more day than night; night dark enough that a fire or torch matters, never so dark you cannot see the ground. In the tech demo, night is only dark for now.
- **Weather** (D78): rain soaks and chills, storms ground you, heat drains thirst. Each world its own.
- **Seasons** (D149): temperature, daylight and precipitation swing over a year and everything responds; winter on the starter world is a real challenge to prepare for ([climate](climate.md)).

## Death

- **No fail state** (D18). You come back.
- **Where** (D171): the hearth you last rested at, in a claim; failing that, a settlement's common hearth. Why, in the lore: [lore.md](lore.md#coming-back-from-death).
- **What you lose**: what you carried lies where you fell and decays after a while (D84); what you wore comes back with you (D171).
- **A little progress**, from the developers (D101): death should have some consequence, perhaps skill or experience lost. Proposed, after Valheim: some progress toward the next step of your skills is lost, never a step already reached, a rank, an essence or an ability. It stings without undoing what you built, and keeps "always getting better" true over time.

## Questions

1. Answered: **starving can kill**; food is essential (D101).
2. Later: **food buffs** may exist; whether and what is for later (D101).
3. Answered: **seasons**, yes (D149).
4. Answered in part: **death's cost** is the pack and a little skill or experience (D101). How much, and from which skills, is open.
5. Answered: **body temperature** is probably its own attribute (D153).

## How the attributes work

Health, stamina, hunger, thirst and breath (D61, D74), mana (D102), and body temperature, probably (D153).

- **Breath is the one rule for air:** under water you use it up (D74). There are no oxygen tanks or thin-air worlds for now (D89).
- **Each attribute is one rule plus numbers,** and a realm's theme sets the numbers (D59). A hot realm makes thirst rise faster, and a cold one makes warmth matter more. Adding an attribute is adding one rule, which every realm then sets numbers for.
- **In the game's model,** each attribute is a component on whoever has it: the player, and later creatures. A system that reads where you are and what you are doing changes it. The server decides them, since a thousand players cannot be trusted to report their own health (D51, D52).
- **What weather and seasons do to them** is in [climate.md](climate.md).

