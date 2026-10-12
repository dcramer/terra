# How games generate loot and spells

2026-10-12. Question: how do shipped games generate loot procedurally, and spells, so that our essence powers can be built the same way? Asked by the developers: "there's a lot of systems that do this kind of procedural generation of things like loot, that's where our basis is going to come from", with much from the books, applied to spells. It informs [docs/magic.md](../docs/magic.md#the-generator-after-loot).

A survey of official sites, patch notes and community wikis. The Path of Exile and Hades wikis refused automated reads, so those points rest on mirrors and excerpts. Marked *unverified* where no source could be opened.

## Answer

The industry has one machine:
1. **A base decides the behaviour.**
2. **Modifiers are drawn onto it,** from pools filtered by tags and weighted.
3. **Families keep modifiers from doubling up,** one per family.
4. **Tiers are gated by level.**
5. **Slots and a budget cap the total.**
6. **Prerequisites open combinations.**
7. **Hand-written uniques sit among the random results.**

Diablo II wrote it down, Path of Exile refined it with tags and weights, Borderlands applied it to parts, Noita and Hades applied it to spells and powers. Our powers fit it closely.

## The games

**Diablo II** ([Arreat Summit](https://classic.battle.net/diablo2exp/items/magic.shtml), [data files](https://d2runewizard.com/game-files/guide.html)):
- **Choosing the base and quality:** a drop picks a base item, then rolls quality from best down (unique, set, rare, magic).
- **Which affixes can roll:** an item's level, adjusted by the base's own level, caps which affixes can roll.
- **What each affix has:**
  - a level window;
  - a frequency (its weight);
  - item types allowed and forbidden;
  - a group: "Items cannot spawn with more than 1 item affix with the same group number."
- **How many:** magic items get one prefix and/or one suffix. Rares get two to six, at most three of each.
- **Uniques and sets** are written by hand, with rolled ranges.

**Path of Exile** ([weight](https://www.poewiki.net/wiki/Weight), [affix](https://www.poewiki.net/wiki/Affix), [support](https://www.poewiki.net/wiki/Support)):
- **Tags and spawn weights:**
  - Bases carry tags, and each mod has spawn weights keyed by tag; the first matching tag gives the weight.
  - A weight of 0 forbids a mod.
  - A mod's chance is its weight over the total of the eligible weights.
- **Families and tiers:** mods come in families, one to an item. Tiers are gated by item level, so a low item can't roll the best tier.
- **Crafting tilts the same draw:** fossils multiply weights by tag, ×0 for one kind and ×10 for another. They are the clearest case of a player tilting a table.
- **Skill gems:** an active skill gem is the base mechanic. Support gems linked to it modify it (more projectiles, converted damage, triggers).
- **Which supports apply:** the real test is a declared set of skill types each support accepts or refuses. Tags alone don't decide it.

**Diablo III and IV** ([Loot 2.0](https://diablo.fandom.com/wiki/Loot_2.0), [Season 4](https://www.icy-veins.com/d4/news/diablo-4-season-4-loot-reborn-patch-notes/)):
- **Diablo III:**
  - Skill runes change how a skill behaves, one rune to a skill.
  - Legendary powers rewrite skills.
  - "Smart Loot" biases drops toward the finder's class.
- **Diablo IV:**
  - Aspects (legendary powers) come in five categories, each allowed in certain slots.
  - Season 4 cut affixes, replaced conditional ones with flat ones, and added Greater Affixes at about 1.5 times the normal maximum.
  - **The lesson Blizzard drew: fewer, bigger, unconditional modifiers.**

**Borderlands** ([parts](https://borderlands.fandom.com/wiki/Shotgun_Parts_(Borderlands_2/TPS)), [Borderlands 4](https://borderlands.fandom.com/wiki/Borderlands_4_Parts_System)):
- **Parts:**
  - A gun's body sets its manufacturer, and with it a signature behaviour every gun of that maker shares.
  - Barrel, grip, stock and sight change the rest.
  - Names come from the parts.
- **Rarity:** opens better parts, and legendaries have unique parts.
- **Licensed parts:** Borderlands 4 lets one maker's signature appear on another's gun.
- **Scale:** "Billions of guns" is the product of the part counts.

**Last Epoch and Grim Dawn** ([runes and glyphs](https://support.lastepoch.com/hc/en-us/articles/46361877750043-Runes-and-Glyphs), [Grim Dawn 1.1.7.0](https://forums.crateentertainment.com/t/grim-dawn-version-1-1-7-0/100418)):
- **Last Epoch:**
  - Affix tiers 1 to 7, the top two from drops only.
  - Crafting spends a budget, forging potential.
- **Grim Dawn:** made affixes match the base's damage type, so a fire sceptre rolls fire. The base's tags tilt the draw.

**Noita** ([wand generation](https://noita.wiki.gg/wiki/Wand_Generation)): the strongest model of a mechanic with modifiers.
- **Wands:**
  - A wand's tier gives it a cost budget, spent on its stats in a partly random order.
  - A few wands are rare archetypes with a spell always attached.
- **Spells:** each has a spawn level and a probability per tier.
- **What the player builds:** projectiles, modifiers and multicasts, in order.

**Magicka** ([overview](https://en.wikipedia.org/wiki/Magicka)): eight elements, up to five queued.
- Opposites cancel, and some pairs make new elements.
- Combinations are emergent rules, not generated.

**Caves of Qud** ([mutations](https://wiki.cavesofqud.com/wiki/Mutations)):
- **Each new mutation offers three random choices,** filtered by the kind of character.
- **Mutations list exclusions.**
- **Offers are fixed by the world seed,** so reloading changes nothing.

**Hades** ([duo boons](https://primagames.com/tips/hades-what-are-duo-boons-how-get-duo-boons)): the closest model of "what you have tilts what you get".
- **Pools and offers:** each god has a pool and offers three boons.
- **Duo boons** need a qualifying boon from each of two gods.
- **Legendary boons** need specific boons of one god.
- **Meeting a requirement makes a boon eligible,** not certain.

**Slay the Spire** ([card rewards](https://slay-the-spire.fandom.com/wiki/Card_Rewards)): explicit bad-luck protection.
- The chance of a rare card rises with each common shown, and resets on a rare.
- Runs are seeded.

**Dwarf Fortress, Myth and Magic** (announced August 2026, [PC Gamer](https://www.pcgamer.com/games/sim/holy-crap-its-happening-dwarf-fortress-gets-its-first-myth-and-magic-update-later-this-year-bringing-each-players-world-its-own-procedural-magic-system/)): a magic system generated per world from its cosmology and history. Details not public.

## Patterns

1. **A base, then modifiers.** The base decides the behaviour.
2. **Tags and weights.** A chance is a weight over the eligible total, and 0 forbids. Multipliers tilt the table, as fossils and damage-type affinity do.
3. **Tiers gated by level.**
4. **Families:** one of each.
5. **Prerequisites** open combinations.
6. **Slot caps by rarity or level.**
7. **A budget,** such as Noita's cost or Diablo IV's item power.
8. **Hand-written uniques,** rare and recognisable, with their own route to the player.
9. **Bad-luck protection,** such as Slay the Spire's offset or guaranteed affixes.
10. **Seeded draws,** reproducible and safe from reloading.
11. **Player-directed tilting,** such as fossils, keepsakes or class-biased loot.

## What this suggests for us

Inferred, for magic.md; it decides nothing.

| Loot's term | Ours |
|---|---|
| Manufacturer or god | **The essence:** its flavour, its signature side effect, and tag multipliers over the shared pools |
| Base item or skill gem | **The mechanic:** the verb, with tags and a declared type |
| Affixes in families, tiers by level | **Twists and added effects,** one per family, tiers unlocked by rank |
| Support gems, duo boons | **Combinations,** by declared rules, tags as the first filter |
| Fossils, keepsakes, smart loot | **The stone, your powers and your skills** tilting the weights |
| Uniques, legendary boons | **Hand-written anchors** |
| Noita's budget | **The rank's budget** |
| Seeded offers | **Draws seeded from the world,** made by the server |

## Limits

- Not every rule could be checked at its source. **Unverified:**
  - Diablo II's fallback when a unique fails;
  - Path of Exile's exact prefix and suffix limits;
  - Hades' rarity odds;
  - Grim Dawn's pairing rules;
  - Risk of Rain's chest tags.
- **Myth and Magic** is announced, not released.
- **Nothing was built or measured.**
