# How magic works in He Who Fights With Monsters

2026-10-12. Question: how do the books' magic mechanics work (essences, awakening stones, abilities, ranks, skill books, magic beyond essences), so we can design our own version? Asked by the developers for the magic system ([docs/magic.md](../docs/magic.md)). We take the fundamentals, never the content (D125).

**Sources:**
- The fan wiki, read through its MediaWiki API, since its pages refused direct fetches: `https://he-who-fights-with-monsters.fandom.com/wiki/` plus a page name below.
- The most useful page was "Shirt's Question Corner", the author's own answers (cited as QC).
- The wiki is thin, and several pages don't exist: Skill Books, Ritual Magic, Astral Magic, Aura.

## Essences

- **What they are:** pure magic that shapes the soul's latent power ([Essences](https://he-who-fights-with-monsters.fandom.com/wiki/Essences)).
- **Where they form:** where magic is saturated, and in numbers during monster surges.
- **Rarity:** five tiers, Common, Uncommon, Rare, Epic and Legendary.
  - A rarity is the chance of an essence appearing *in an environment suited to it*, not how common it is overall. In an unstable place a common essence of that kind is far likelier than a legendary one (QC).
  - No essence is stronger than another.
- **About 147 are named:**

  | Rarity | Count |
  |---|---|
  | Common | 89, mostly weapons, tools and animals |
  | Uncommon | 24 |
  | Rare | 11 |
  | Epic | 12 |
  | Legendary | 6 |

  More than 100 confluences are listed.
- **Absorbing one:**
  - It is a consumable, and absorbing it hurts.
  - Each binds to one of four attributes: Power, Speed, Spirit, Recovery.
  - A person holds four: three chosen, plus the confluence. The same essence can't be taken twice.
- **The confluence:**
  - It forms on its own from the three, and only its owner can claim it.
  - Rejecting it loses it for good.
  - One combination usually gives the same confluence. From the eleventh book it is likely rather than certain.
  - Many recipes lead to one confluence.
  - The author's rules of thumb: opposing forces plus a balancing essence tend one way, opposing forces plus an escalating essence another, and some pairs almost always give one result (QC).
- **Banned essences:** some are banned by the books' adventurers' society because they harm innocents, not because they are strong.

## Awakening stones

- **Five abilities to an essence:** the first awakens when the essence is absorbed, and each of the other four takes one stone. That is twenty abilities in all ([Awakening Stones](https://he-who-fights-with-monsters.fandom.com/wiki/Awakening_Stones)).
- **What you get is random, weighted by:**
  - the stone's kind (rarer stones lean more specialised, but can give anything);
  - the essence;
  - the person's race;
  - what the person knows, such as ritual magic;
  - which kinds of ability the person already has;
  - perhaps personality, which the wiki marks as theorised.
- **Further rules:**
  - No second permanent boost of a kind already held.
  - Synergy is best if all essences are absorbed before any are awakened.
  - An ability's effects at every rank are fixed when it awakens (QC).
- **Kinds of stone:** about 70 are named, by rarity, such as the hunt, wrath, ruin or the gate. Stones form as essences and monsters do; gods and great beings also make them.
- **Divine stones:** a god's stone always gives the ability the god chooses, and the god can take it back.

## Abilities

- **Kinds:**
  - special ability (passive or utility, the most common);
  - special attack (usually stamina);
  - spell (usually mana, sometimes chanted);
  - conjuration (makes an item or weapon);
  - summoning;
  - familiar;
  - aura;
  - perception;
  - zone.
- **Limits:** one aura and one perception per person, and one summon and one familiar per essence.
- **Effects carry tags,** such as holy, curse, poison, drain, boon and affliction. Abilities have costs (mana, stamina or health) and cooldowns.
- **Advancing:**
  - **By use under real pressure,** fastest in combat.
  - **By absorbing monster cores,** which raises the least advanced abilities first but makes training less effective after, until only cores work.
  - **A cap:** abilities stop at the person's rank.
  - **Steps within a rank,** shown as a percentage.
  - **Each rank adds a new effect** to an ability.

## Ranks

Normal, then Iron, Bronze, Silver, Gold, Diamond, then Transcendent (gods and the like) ([Ranks](https://he-who-fights-with-monsters.fandom.com/wiki/Ranks)).

| Rank | What it takes or brings |
|---|---|
| Iron | All four essences |
| Bronze | All twenty abilities at bronze |
| Silver | Beyond human; an ordinary killing blow no longer kills |
| Gold | No need to breathe; going further needs a "revelation" about one's essences |
| Diamond | Ageless |

- **Lifespans lengthen with rank:** about 75 years at Iron, up to about 500 at Gold.
- **Attributes have ranks of their own,** and rank is a function of them.
- **A higher rank resists a lower one's harm.**

## Skill books

- **Absorbed, and the skill is known at once,** if you meet the book's requirements. Using them may need a gift; the wiki suggests it but doesn't confirm it for everyone.
- **Known books:** basic ritual magic; basic artifice (putting magic into objects); and a martial art in five forms, one book per form.
- **Where they come from:** looted from dead essence users, or from clever monsters that had the skill (QC).

## Magic not from essences

- **Ritual magic:**
  - Learned by study or from a book, from Iron rank.
  - Channelled through drawn diagrams of symbols, with monster cores, coins and quintessence as components.
  - A ritual a rank above you risks backlash; two above is death.
  - Low ambient magic makes rituals unreliable.
- **Astral magic:** a specialised branch of ritual magic. Little more is documented.
- **Artifice:** everything that makes magic items: alchemy, forging, armour, lasting spell effects. Ritual magic with artifice gives lasting arrays.
- **Alchemy:** potions, made by a profession of its own. Drinking or eating above your rank causes backlash.
- **Divine:** gods give essences and stones to followers and can take them back.
- **Familiars:** summoned by a ritual that uses materials.
- **Racial gifts:** six innate gifts per race, which can change with events.

## Magic as material and money

- **Spirit coins** are both currency and raw magic: they fuel items and rituals. Consuming one above your rank lifts your attributes to its rank briefly, then exhausts you.
- **Monster cores** are common drops, used in rituals and crafting, and to rank abilities faster at a cost.
- **Quintessence** is a fragment of an essence's kind, used in rituals and items.
- **Ambient magic** has two measures: saturation, how much (how many things form), and density, its quality (the rank of what forms).

## Auras

- **An aura ability:** one to a person. A passive field: boons for allies inside, afflictions for enemies.
- **Every person also has a presence of their own,** which can be trained. Stronger auras suppress weaker; spreading one over more people or more ground weakens it; while yours holds, some powers can't move you.

## Limits

- **Fan wiki only,** read through its API: no book text was checked.
- **Not confirmed:**
  - the ritual for absorbing an essence;
  - that steps run 0 to 9;
  - whether everyone can use skill books;
  - the details of astral magic;
  - how loot works without a power to loot.
