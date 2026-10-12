# Survival: needs, rhythms and death

Part of [game design 001](README.md). A proposal for the developers to react to; nothing here is decided unless it cites a decision. The general survival doc is [docs/survival.md](../../survival.md); under this proposal its oxygen tanks and terraforming give way to what is here.

## What the developers have said

- Survival attributes: health, stamina, oxygen, hunger, thirst (D61). Oxygen is used up under water (D74). No oxygen tanks or science fiction for now (D89).
- A sandbox with some survival in it; part of survival is establishing and holding bases and outposts (D90).
- No fail state (D18). What you carry drops when you die, and dropped items decay after a while (D84).

## Based on

**Valheim** and **Enshrouded** for survival that rewards rather than punishes: food that makes you stronger, rest that restores you, cold and dark that you prepare for.

## Needs

| Need | Met by | When it is low |
|---|---|---|
| Hunger | Food: hunted, fished, gathered, farmed, cooked | Health and stamina recover slowly and their limits shrink; in the end, starving kills (D101) |
| Thirst | Water: springs, rivers, wells, a waterskin | The same, faster |
| Breath | Air: only under water (D74) | Health falls fast |
| Warmth | Fire, shelter, clothing, work | Body temperature (D153): too cold raises hunger and stamina costs, too hot raises thirst and stamina costs, and far out of bounds either wears health down ([climate](../../climate.md#body-temperature)) |
| Rest | A bed in a claim | Proposed: rested, you recover faster for a while |

**Food is essential** (D101): it is what keeps you from dying; going without makes you weaker, and then kills you. **Good food may also give buffs**, as in Valheim and Enshrouded; whether, and what they are, the developers will decide later.

## Rhythms

- **Day and night.** Night is darker, colder and more dangerous: more monsters, and stranger ones ([creatures.md](creatures.md)). Light matters: torches, fires, lit claims. Proposed (D134): a whole day lasts half an hour of play, as Valheim's and Enshrouded's do, a little more day than night; night dark enough that a fire or torch matters, never so dark you cannot see the ground. In the tech demo, night is only dark for now.
- **Weather** (D78): rain soaks and chills, storms ground you, heat drains thirst. Each world its own.
- **Seasons** (D149): temperature, daylight and precipitation swing over a year and everything responds; winter on the starter world is a real challenge to prepare for ([climate](../../climate.md)).

## Death

- **No fail state** (D18). You come back.
- **Where**: proposed, at the bed you last slept in, in a claim; failing that, the nearest friendly settlement.
- **What you lose**: your pack drops where you fell and decays after a while (D84); what you wear stays on you.
- **A little progress**, from the developers (D101): death should have some consequence, perhaps skill or experience lost. Proposed, after Valheim: some progress toward the next step of your skills is lost, never a step already reached, a rank, an essence or an ability. It stings without undoing what you built, and keeps "always getting better" true over time.

## Questions

1. Answered: **starving can kill**; food is essential (D101).
2. Later: **food buffs** may exist; whether and what is for later (D101).
3. Answered: **seasons**, yes (D149).
4. Answered in part: **death's cost** is the pack and a little skill or experience (D101). How much, and from which skills, is open.
5. Answered: **body temperature** is probably its own attribute (D153).
