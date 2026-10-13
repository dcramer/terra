# Magic: essences, books and old-school spellcraft

Part of the game's design ([gameplay.md](gameplay.md), D89). Status: Open, a proposal for the developers (D163), 2026-10-12, partly answered the same day: attributes and monster cores are kept (D164, D165), there is no fast travel between claims (D72), and the old-school touches follow the books. Each item is as settled as the decision it cites. It builds on the lore ([lore.md](lore.md)) and gives the examples each system needs. This page owns essences, powers, attributes, cores, mana and studied crafts; skills, the rank ladder and crossing a rank are in [progression.md](progression.md). How the books do it is in [research/hwfwm-magic.md](../research/hwfwm-magic.md). We take the mechanics, never the content (D125): every essence, ability, style and working here is ours.

## What the developers asked for

From 2026-10-12:
- **Nostalgia from old-school magic,** within our world design and lore.
- **Examples to build from,** for each system:
  - combat styles learned from skill books;
  - ritual magic and the like, learned from skill books;
  - essences, a handful of core ones, inspired by the books;
  - the books' two kinds of thing: essences, and the stones that awaken their powers.
- **Learning powers from essences is procedural and random,** as in the books: a weighted draw from a list, the list shaped by what you have learned before. There is a lot to generate.
- **What other magic is there** that this list leaves out?

## The shape of it

Four ways to have magic, matching the lore's three ways to grow (D124) plus things that hold magic:

| | Where it comes from | How you get it | Random? | Old-school feel |
|---|---|---|---|---|
| **Essence powers** | Essences you absorb, woken by awakening stones | Found from monsters and pools, then absorbed and awakened | Yes: a weighted draw, shaped by you | A character no one else has; spells on a hotbar |
| **Studied crafts** | Skill books, teachers, practice | Read a book and its first workings are yours; practice deepens them | No: a book teaches what it says | Spellbooks, reagents, words of power, rituals in chalk circles |
| **Combat styles** | Skill books, teachers, practice | Read a form, practise it | No | Martial arts in five forms |
| **Things that hold magic** | Making, finding, gods | Crafted, looted, bought, granted | Loot rolls; made things don't | Scrolls, wands, potions, enchanted gear |

Essence powers are fast, personal and mostly for the moment of a fight. Crafts are slow, deliberate and anyone's to learn. Things are magic you carry and spend. Each has a reason to exist that the others don't cover.

## The old-school feel

What made magic in Ultima Online, EverQuest, RuneScape, Morrowind and tabletop D&D memorable, and where each lands here:

| Old school | Ours |
|---|---|
| A spellbook you fill page by page | Your book: the character screen's pages of powers and workings, filled as you learn (D93) |
| Reagents in your pack: pearls, roots, ash | Components, used up, as in the books: monster cores, fragments of essences, and other remnants and herbs a realm's role names ([settings.md](settings.md)) |
| Words of power, spoken as you cast | Chanted spells, as in the books: some essence powers are chanted, a cast that can be broken off, stronger for it, and never quicker with rank. And the diagrams of crafts are written in the symbols of magic, whose structure is the same in every realm |
| Spell circles, first to eighth | Workings by rank: iron workings to diamond |
| Mark and Recall; Gate Travel | Astral magic, below |
| Scrolls and wands | Inscription, below |
| Identify | Ritual magic's first working |
| A wizard's familiar | Bindcraft, below |
| Meditating to get mana back | Rest: mana returns faster sitting still, and Meditation is a skill by doing (D88) |
| Fizzling | Backlash, as in the books: a working a rank above you risks exhaustion, your attributes dropping a rank for a while, injury or death; two ranks above kills; and in thin magic a working is unreliable |
| Schools of magic | The ideas of essences, and the crafts |

## Essences

From the lore and the books' mechanics (D98), with our own essences:

- **Three essences and a confluence.** You absorb up to three; the fourth forms from their combination and is yours alone. The combination is your identity, chosen by what you absorb, with no class picked at the start. Confluence is the books' name for it (D125).
- **Five powers to an essence.** The first wakes when you absorb it; each of the other four needs an awakening stone. Twenty powers in all, which is a whole character.
- **Rarity:** common, uncommon, rare, epic, legendary. As in the books, rarity is relative to a place: a frozen pool mostly gives Frost and rarely something stranger. Where you hunt decides what you can become (D91).
- **Where essences form:** in pools and in surges, and in monsters made there, so each realm's leaning decides what its pools give ([lore.md](lore.md#magic)).
- **Absorbing one hurts,** and can only be done at rest, in safety. Proposed: also possible as a ritual (ritual magic), which is gentler.
- **Each essence binds to one of your four attributes** when absorbed ([below](#attributes)).
- **No essence is better than another.** They differ in how they play.
- **Read on the character screen** (D93): your essences, powers, attributes and ranks. Others' ranks are judged by looking, and by their aura.

### Core essences

Our own list, proposed (D125: the lists are ours, never the books'). Each has an idea, its tags (which the confluence and awakening read), where it forms, and what its powers lean to.

| Essence | Rarity | Tags | Forms in | Its powers lean to |
|---|---|---|---|---|
| **Blade** | Common | weapon, edge | Battlefields, fallen armouries | Strikes, parries, conjured edges |
| **Arrow** | Common | weapon, reach | Hunting grounds | Aimed shots, marking prey |
| **Bulwark** | Common | weapon, guard | Old forts | Blocks, guarding others, walls |
| **Maul** | Common | weapon, force | Quarries, forges | Staggers, breaking, shockwaves |
| **Ember** | Common | element, heat | Burnt and volcanic ground | Bolts, burning, light, warmth |
| **Brook** | Common | element, flow | Streams, springs | Mending, pushing, breathing under water |
| **Stone** | Common | element, earth | Caverns, cliffs | Hardening, walls, tremors |
| **Gale** | Common | element, air | Ridges, high places | Speed, leaps, pushing, carrying sound |
| **Hound** | Common | beast, pack | Forest and plain | Tracking, calling a pack, keen senses |
| **Boar** | Common | beast, charge | Thickets | Charging, toughness, goring |
| **Hawk** | Common | beast, sight | Cliffs, open sky | Far sight, diving strikes |
| **Root** | Common | growth, earth | Old woods | Snaring, holding fast, mending |
| **Stride** | Common | body, speed | Anywhere things flee | Movement, dodges, quick strikes |
| **Brawn** | Common | body, strength | Anywhere things fight | Power, carrying, breaking |
| **Frost** | Uncommon | element, cold | Ice and high ground | Slowing, freezing, cold armour |
| **Storm** | Uncommon | element, sky | Peaks, open sea | Lightning, chaining, thunder |
| **Mist** | Uncommon | element, veil | Marshes, dawn valleys | Hiding, confusing, slipping away |
| **Thorn** | Uncommon | growth, harm | Bramble and briar | Barbs, bleeding, hedges |
| **Marrow** | Uncommon | body, death | Graves, old kills | Bone armour, spikes, the dead |
| **Pulse** | Uncommon | body, life, cost | Battlefields | Powers paid in health, draining, quickening |
| **Dawn** | Uncommon | force, holy | Sunlit holy places | Healing, revealing, burning the unholy |
| **Dusk** | Uncommon | force, shadow | Deep caverns, long nights | Hiding, fear, blinding |
| **Whisper** | Uncommon | mind, sound | Ruins, where people gather | Boons for allies, lulling, voices |
| **Raven** | Uncommon | beast, omen | Battlefields, gallows | Scouting, ill luck, carrion |
| **Hearth** | Uncommon | home, warmth | Old homes, long-held claims | Shelter, warmth (D153), guarding a place |
| **Ash** | Rare | death, heat, renewal | Burnt forest after the fire | Rising again, burning out, regrowth |
| **Tide** | Rare | flow, time | Coasts, the edges of pools | Ebb and flow: building up, then releasing |
| **Leech** | Rare | body, take | Places that drain magic | Taking what others have: health, mana, power |
| **Dream** | Rare | mind, sleep | Where many have slept | Sleep, visions, walking in others' minds |
| **Grave** | Rare | death, rest | Old burial grounds | Raising the dead, ending things, wards against them |
| **Glyph** | Epic | knowledge, mark | Ruins of studied peoples | Marks that work later, traps, empowering crafts |
| **Rot** | Epic | death, growth | Blighted ground | Decay that spreads, fungus, plague |
| **Ward** | Epic | guard, magic | Old held places | Protecting places and others, turning magic aside |
| **Oath** | Epic | bond, will | Where vows were kept or broken | Binding promises, strength while keeping them |
| **Plume** | Epic | beast, sky | The highest places | Gliding, then flight at high rank |
| **Abyss** | Legendary | astral, void | Where realms wear thin | Pulling, swallowing, stepping through the astral |
| **Crown** | Legendary | rule, aura | Where the mighty died | Commanding, aura, followers |
| **Ford** | Legendary | crossing, astral | Natural portals | Moving between places: blinks, doors, gates |

More come per realm as data: a gothic realm adds Lantern and Shroud, a machine realm Gear and Spark. Each is a row like these.

### Confluences: from tags, by rules

A confluence is worked out from the three essences' tags by a few rules, so players can learn the patterns and trade recipes:

- **Kindred:** three that share a family (say three elements) give that family's greater form. Ember, Stone and Gale give **Primal**.
- **Opposed and balanced:** two opposites plus a balancing one give a union of the two. Dawn, Dusk and Whisper give **Gloaming**.
- **Opposed and escalated:** two opposites plus a strength give a violent one. Ember, Frost and Brawn give **Shatter**.
- **Beast and element:** a beast with elements gives a creature of that element. Hawk, Storm and Gale give **Stormwing**.
- **Body and cost:** two body essences with Pulse or Leech give something that spends itself. Pulse, Brawn and Leech give **Frenzy**.
- **Weapon and guard:** Blade, Bulwark and Stride give **Duellist**; Arrow, Hawk and Gale give **Far Shot**.
- **Death and growth:** Marrow, Root and Rot give **Barrow**; Ash with any growth gives **Rekindle**, rarely.

The usual result is the likeliest, not certain, as in the books' later volumes: a common recipe gives its confluence most of the time and sometimes a neighbour. Recipes become knowledge players share and argue about, which is the old-school feel of a game with secrets.

## Attributes

Kept, as in the books (D164): **Power, Speed, Spirit and Recovery.**

- **Each essence binds to one of them** when you absorb it, usually the one its idea suits: Brawn to Power, Stride to Speed, Dawn to Spirit, Brook to Recovery.
- **Each attribute has its own rank and steps,** and **your rank is a function of your attributes.** You are silver when they are.
- **They rise as the powers of the essences bound to them rise,** so how you use your powers shapes your body. Skills (D88) don't raise them; skills are what your hands know.
- **What each does, proposed and tunable:**

| Attribute | Raises |
|---|---|
| **Power** | Blows and throws, carrying (D55), breaking, the stamina pool |
| **Speed** | Moving, striking, dodging, reacting |
| **Spirit** | The mana pool (D102), the strength of powers, resisting magic, aura |
| **Recovery** | The health pool, and how fast health, stamina and mana come back; resisting poison, sickness and cold or heat (D153) |

- **A rank's gap shows in them:** a higher-rank attacker's attributes beat a lower-rank defender's resistance, which is why two ranks up is death ([combat.md](combat.md)).
- **On the character screen** (D93), as the books show them.

## Mana

The books' fighters spend mana on powers. **There is mana as well as stamina** (D102): a pool of its own beside health and stamina (D61), shown with them, in blue, as the developers said on 2026-10-11. Spirit sets how large it is and Recovery how fast it comes back (D164). Proposed: it returns faster at rest, sitting still, and with food ([survival.md](survival.md)).

## Awakening stones: drawing a power

The books' second kind of thing, kept with its name (D125); the kinds of stone are ours. A stone is found as essences are, in pools, surges and the remains of strong monsters. Each has a kind that pulls the draw its way:

| Stone, examples | Rarity | Pulls toward |
|---|---|---|
| Of the Grip, of the Watch, of the Step | Common | Strikes; perception; movement |
| Of Kindling, of the Well, of the Hill | Common | That element's effects |
| Of the Rampart, of Mending | Uncommon | Guarding; healing |
| Of the Blow, of Patience | Uncommon | Big single blows; slow builds that release |
| Of the Pack, of the Bond | Rare | Summons and familiars |
| Of the Flood, of the Throng | Rare | Areas; many targets |
| Of Crossing, of Ages | Legendary | Movement through the astral; powers that grow with time |

### How a draw works

When you use a stone on an essence, the server draws a power from that essence's list. The list is long and generated (below). Every candidate has a weight, which is multiplied by:

1. **The stone's kind:** strongly toward what it pulls. A rarer stone pulls harder, but anything is possible.
2. **Your other essences:** candidates sharing tags with them are likelier, so a character comes out coherent. Absorbing all three before awakening any gives the best fit, as in the books, which is a choice players can make.
3. **The powers you already have** (decided, 2026-10-12): the developers' answer to how a character's history shapes its draws.
   - Candidates that combine with your powers are likelier: sharing their tags, or building on their mechanics. A power that marks makes powers that hit the marked likelier, as Hades' boons open the way to their duos.
   - Kinds you lack are likelier, so no one ends with five passives;
   - kinds you have many of are less likely;
   - some kinds are one to a person (an aura, a perception) or one to an essence (a summon, a familiar).
4. **Your skills and what you know** (decided, 2026-10-12): someone who knows ritual magic draws more ritual-like powers, and a practised swordsman more blade strikes. Your skills (D88) and books shape your powers, which is the lore's "who you are".
5. **Where you are:** a little toward the leaning of the realm you awaken in. Awakening a stone in a death realm leans a little toward death.

### How many outcomes a stone has

From the developers, 2026-10-12: not fifty things it could roll. Proposed: **a stone on an essence has a short list of outcomes, about five to ten,** and the variety comes from what each outcome becomes in you:

1. **The outcome list.** Each pairing of essence and stone kind has a short list of outcomes. An outcome is a shape of power: its mechanics, as this essence does them. Ember with the stone of the Step might have seven: bursting into flame and reappearing, a dash that leaves fire, a leap that lands in a burst, running on heat, and three more. This is the loot machine's base.
2. **The draw picks one outcome,** weighted by everything above.
3. **The outcome is then made yours:** its twists, numbers and ladder are rolled by the loot machine's affixes and tiers. Two people who both drew "bursting into flame and reappearing" get recognisably the same power, different in what it leaves behind, how far, and what it gains at silver.

Why a short list:
- what a stone gives is learnable;
- the common outcomes can be known (below);
- powers stay recognisable, as people in the books know what an essence tends to give.

The books keep records of what combinations give, and nothing on the fan wiki suggests long lists.

**Where the lists come from:**
- **Core essences:** generated by our tools and kept as data, so we can read and tune them, and what players learn holds for everyone.
- **Generated essences:** a realm's essences get theirs when the realm is generated.

**Proposed sizes:**

| Pairing | Outcomes |
|---|---|
| Common essence, common stone | About eight, familiar and broad |
| Rarer essence or stone | Fewer, about five, and stranger |

The numbers multiply: 38 core essences, a few dozen kinds of stone, and five to ten outcomes each give thousands of shapes across the game, each made individual by its rolls.

The draw is made once, by the server (D51), and kept: it is a change to you, stored like any other. **There is no choosing among offers** (2026-10-12): you get the one power the draw gives, as in the books. Your only control is what you bring to it: which stone, on which essence, when, and what you already have and know.

### Before you awaken: what is known

From the developers, 2026-10-12: awakening is a ritual, and while doing it you see what is known. Simplified the same day: **the most common effects are known.**

- **How much is known follows rarity** (from the developers, 2026-10-12). A common stone on a common essence has been done countless times, so its whole outcome list is known. A rare stone on a legendary essence has been done by few, so little or nothing is.

| Pairing | Known at the ritual |
|---|---|
| Common and common | Every outcome on the list, with which are usual and which unusual |
| Uncommon in either | Most of the list |
| Rare in either | The commonest two or three |
| Epic in either | The commonest one, or rumours |
| Legendary in either | Nothing but rumours |

- **Outcomes are shown in words.** "Ember with the stone of the Step commonly gives: bursting into flame and reappearing; a dash that leaves fire behind; a leap that lands in a burst." What most people got is what the world knows.
- **Knowledge grows in the world** (decided, 2026-10-12): when anyone in a world awakens an outcome that wasn't known, it becomes known there. Rare pairings fill in as players discover them, and the first to find one is the first to know. **People in the world count too:** some areas may have essence users among their people, or special kinds of people who hold essences, and what they have is known locally and can be learned by asking them (D173).
- **What can't happen is shown too:** "you already have an aura", "this essence has its summon". The limits are common knowledge.
- **Nothing else:**
  - no numbers;
  - none of the rarer outcomes;
  - never the power you will get, its twist, its ladder or its name.

  The rare tail stays a surprise.
- **The order is for you:** which known outcomes are usual is worked out with your tilt included ([How a draw works](#how-a-draw-works)), so two people reading the same stone may see different common effects. The reasons aren't shown.
- **Knowing ritual magic shows one or two more outcomes than the world knows** (decided, 2026-10-12), the better the ritualist the more, so progress in one discipline pays in another. Books and scholars in settlements tell the known outcomes too, before you hold the stone.
- **Nothing to game by waiting.** The draw is seeded from the world, you, the essence's slot and the stone, so putting the ritual off gives the same result. Choosing a different stone or a different essence is the real choice, as in the books.

### Hundreds of powers, from shared mechanics

From the developers, 2026-10-12: procedural, and **hundreds of powers, not dozens.** Characters may overlap a little. The mechanics behind powers overlap a lot, as they do in the books, where many people can fly or teleport, each in their own way. Some powers combine several mechanics, and nearly every power is different.

So powers aren't written as lists. They are built from three layers:

1. **Mechanics: what a power does.** A shared library of a few dozen, the same for every essence. This is where powers overlap.
2. **Flavour: how this essence does it.** Each essence gives a mechanic its look, its sound and, above all, a side effect. This is where powers differ.
3. **Composition: what goes with it.** One main mechanic, up to two more combined with it, a delivery, a twist, a cost and a ladder of ranks:

| Part | Choices, for example |
|---|---|
| **Kind** | strike (stamina), spell (mana), conjuration, summon, familiar, aura, perception, passive, movement, zone |
| **Delivery** | touch, weapon, bolt, cone, area, self, an ally, a place, a mark that triggers later |
| **Twist** | lingers, chains, stacks, builds up then releases, costs health instead of mana, stronger at night, stronger near water |
| **Cost and wait** | mana, stamina or health; a cooldown |
| **Ladder** | something added at each rank, iron to diamond |

#### The generator, after loot

From the developers, 2026-10-12: the basis is how shipped games generate loot, with much from the books, applied to powers. One machine serves powers and loot alike ([settings.md](settings.md#loot), D127). The survey is [research/procedural-loot-and-spells.md](../research/procedural-loot-and-spells.md).

| Loot's way | Ours |
|---|---|
| **A base decides the behaviour:** Diablo's base item, Path of Exile's skill gem, Noita's projectile | **The mechanic** is the base: a verb with tags and a declared type |
| **A maker gives a signature:** a Borderlands manufacturer, a Hades god | **The essence** gives its flavour and side effect, and multiplies the pools' weights by tag: ×0 forbids, more favours, as Path of Exile's fossils do |
| **Affixes in families, one of each, in tiers gated by level** | **Twists and added effects** in families, one of each to a power, their tiers opened by rank |
| **Slot caps by rarity** | **How many twists a power may have,** set by its rarity |
| **Support gems and duo boons:** a modifier declares what it can join | **Combinations:** a mechanic declares which others it can carry. Tags are the first filter and the declared rule is the test, since Path of Exile found tags alone weren't enough |
| **Prerequisites:** Hades' duo boons | **Powers open others:** what you have makes some candidates eligible and likelier, never certain |
| **Player tilting:** fossils, keepsakes, class-biased loot | **Your stone, powers and skills** tilt the weights ([How a draw works](#how-a-draw-works)) |
| **A budget:** Noita's wand cost, Diablo IV's item power | **The rank's budget,** spent by mechanics, twists and numbers |
| **Hand-written uniques,** with their own route to the player | **Anchors:** the classics, written as data with rolled numbers |
| **Bad-luck protection:** Slay the Spire's rising rare chance | **No one goes too long without a kind they lack:** the weight of a missing kind grows with each draw that passes it by |
| **Seeded draws,** safe from reloading | **Draws seeded from the world** and made by the server (D51) |

**Rarity of a power:** as loot has rarity, so does a drawn power: common, uncommon, rare, epic, legendary.
- Rarity gives more twist slots and higher tiers, within the rank's budget.
- A rarer stone, and a rarer essence, raise the odds, as magic find does.

**Diablo IV's lesson,** taken on: fewer, bigger, unconditional effects read better than many small conditional ones. A power does one or two clear things, and its twist is something you notice.

#### The mechanics library

Proposed, to grow over time. Each is a small piece of game logic written once (D127):

| Family | Mechanics |
|---|---|
| **Moving** | blink, dash, leap, glide, flight, wall-walking, water-walking, phasing through, swapping places, pulling yourself to a point, returning to a marked point |
| **Harming** | strike, bolt, beam, cone, burst around you, chain, harm over time, delayed burst, finishing blow, harm paid back |
| **Defending** | shield on yourself, shield on others, barrier, harder skin, reflecting, absorbing into mana, untouchable for a moment, cleansing |
| **Controlling** | slow, root, stun, knock back, pull, fear, sleep, blind, silence, drawing attacks to you |
| **Supporting** | heal, heal over time, boon, quickening, raising the fallen, sharing harm, sharing senses |
| **Making** | summoning a creature, conjuring a weapon, conjuring a wall or ground, a construct, a familiar, a decoy or double |
| **Knowing** | seeing through walls, tracking, sensing magic, foresight, marking, reading auras |
| **Changing** | changing form, changing size, an elemental body, hiding in plain sight |
| **Paying** | costing health for mana, refunding on a kill, stacks that build and then spend, borrowing from later |
| **Places** | an aura, a zone on the ground, a totem, a trap |

#### One mechanic, many essences

The same mechanic through different essences, as the books do with flight and teleporting:

| Essence | Blink, as this essence does it | Side effect |
|---|---|---|
| Ember | Burst into flame and reappear | Burning ground where you left |
| Mist | Dissolve into fog and form again | Unseen for a moment after |
| Raven | Scatter into birds that fly to the spot | Can't be hit while scattered |
| Root | Sink into the ground and rise from a tree's roots | Only between living wood, but far |
| Ford | Open a small door and step through | An ally can follow you through |
| Storm | Become a bolt of lightning | Harms what lies between |
| Dream | Wake where you dreamed of being | Must have seen the place before |

| Essence | Flight, as this essence does it | Side effect |
|---|---|---|
| Plume | Wings | Fast, and tires you |
| Gale | Ride the wind | Only with the wind, or slowly against it |
| Ember | Fire beneath you | Lights you to everything, and burns what's below |
| Dusk | Shadow that carries you | Only out of the sun |
| Abyss | Fall upward | Unaffected by blows that push |

#### Combinations

Some powers stack mechanics, which is where the most distinctive ones come from:

- **Riptide** (Tide): pulls everything in an area toward its centre, slows them, then bursts. Pull, slow and delayed burst.
- **Carrion Watch** (Raven): a familiar that sees for you, marks what it sees, and pecks what you mark. Familiar, sharing senses, marking and harm over time.
- **Hearthguard** (Hearth): a zone around your fire where allies heal, warm (D153) and take less harm. Zone, heal over time and shield on others.
- **Oathbound Strike** (Oath): mark a foe and swear to fell it. You are faster and stronger toward it, and weaker if you turn away. Mark, boon and paying.

#### Why it gives hundreds each

With a few dozen kinds of stone and five to ten outcomes each, one essence has a few hundred shapes of power ([How many outcomes a stone has](#how-many-outcomes-a-stone-has)). The library of about 70 mechanics, a dozen deliveries and a few dozen twists builds those shapes and makes each drawn power individual. With 38 core essences and more per realm, the whole space is far beyond what any player will see.

- **Two characters with the same essence will usually share a mechanic or two,** such as both having a blink. They rarely share a power, because the flavour, the combination and the ladder differ.
- **A rank-up adds one of:**
  - a mechanic combined in;
  - a twist;
  - a widening (more targets, more area, longer);
  - a stronger side effect.

  All of it is generated with the power, so two blinks that start alike can end far apart.

#### Rules that keep it sound

- **Fit:** a mechanic must suit the essence's tags. Ember harms and moves; it can't heal, except through Ash. The confluence can bridge what its three can't.
- **Budget:** a rank sets how much a power may do. Combining mechanics costs budget, so a power that does three things does each less. A drawn power is never stronger than a hand-made one; it is different.
- **Limits, as in the books:**
  - one aura and one perception to a person;
  - one summon and one familiar to an essence;
  - no second lasting boost of a kind you already have.
- **Hand-written anchors** sit in every essence's pool with high weight: the classics a player expects of an idea, such as a ball of fire for Ember, a healing touch for Brook, a howl that calls the pack for Hound. They are the nostalgia, and the generated powers are the surprise.

#### Names

From [naming.md](naming.md): the essence's word, or its flavour, joined to what the power does. Emberstep, Mistwalk, Ravenflight, Rootpassage, Riptide, Cinder Bolt. A generated name must pass naming's checks, and a few hand-written names cover the anchors.

#### Essences are generated too

The 38 core essences are the hand-made anchors. Realms add essences of their own, generated the same way: an idea word from the realm's leaning, by naming's rules, with tags, a rarity and a flavour for each mechanic family. A gothic realm's Lantern blinks by guttering out and lighting again elsewhere. Every essence, made or generated, draws from the one library.

#### Checking it without a person (D69)

A tool, built with the generator: draw thousands of characters and report
- how often two characters share a whole power (aimed low, a few in a hundred);
- how often they share a mechanic (aimed high);
- how the mechanics spread across essences;
- any power over its rank's budget.

The numbers tell us when the library needs more mechanics or an essence needs more flavour.

### Three worked examples

**Cinder Bolt** (Ember; spell; a bolt):

| Rank | What it does |
|---|---|
| Iron | Throws a bolt of fire that burns for a few seconds |
| Bronze | The burn spreads to anything close to the target |
| Silver | Burning targets take more from all fire |
| Gold | Hold the cast to throw three |
| Diamond | A target burned to death leaves a burning patch that keeps burning |

**Pack Call** (Hound; summon; a place):

| Rank | What it does |
|---|---|
| Iron | Calls a hound of your rank to fight beside you for a while |
| Bronze | Two hounds |
| Silver | The hounds share your marks: what you mark, they hunt |
| Gold | Hounds that fall rise once more |
| Diamond | A pack leader joins them, and you hear what they hear |

**Bedrock** (Stone; passive):

| Rank | What it does |
|---|---|
| Iron | Standing still, you take less from blows |
| Bronze | And you can't be pushed |
| Silver | Allies behind you take less too |
| Gold | A blow that would knock you down stuns its giver instead |
| Diamond | Standing still for a few seconds, you are as stone: nearly untouchable, but unable to act |

### Raising powers

- **By use, under real pressure:** fighting, or using a power where it matters (D88). A power used only for practice barely grows.
- **Capped at the rank of the attribute it is bound to.** Your rank comes from your four attributes, which rise as the powers bound to them do (D164, [above](#attributes)).
- **Crossing a rank may need a key** (D126).
- **Monster cores can hurry it** (D165), at the books' price:
  - absorbing cores raises your least-advanced powers first;
  - each core used makes practice count for less, until only cores work;
  - leaning on them makes crossing past gold harder.

  Hurry now, pay later. Cores are also what crafts burn as components, what calms a pool, and a trade good, so every monster killed is worth something.

## Skill books

A skill book is a found thing (D68, D92). Reading one teaches what it holds at once. Its requirements are a rank, a skill, or another book read first. Books come from ruins, from monsters that were once people or that were clever, from settlements' teachers and traders, and from other players. In the books only some people can use skill books; proposed: anyone can.

### Combat styles

A style is a way to use a kind of weapon. **A style has five forms, iron to diamond, one book each,** so crossing a rank in a style means finding its next book or a teacher (D126). The first form teaches stances and a few moves; the rest come by practice ([combat.md](combat.md)).

| Style | Weapon | Plays as | First form teaches |
|---|---|---|---|
| **Stillwater Guard** | Sword and shield | Wait, turn the blow, answer | A parry that opens the attacker; a shield bash |
| **The Wolf's Run** | Two short blades | Hit, move, hit from the side | A dash through an enemy's flank; strikes from behind count double |
| **Long Reach** | Spear or staff | Keep them at the point | A sweep that pushes back; a vault on the haft |
| **Breaker** | Two-handed hammer or axe | Break guard, break shields, break bones | A blow that breaks a block; a stagger on a full swing |
| **Hunter's Patience** | Long bow | Wait for the shot | A held draw that doubles damage; quieter movement while drawn |
| **Running Shot** | Short bow | Shoot on the move | Shooting while dodging; quick nocks |
| **Open Hand** | Unarmed | Throw, hold, disarm | A throw; taking a weapon from a hand |
| **Knife and Cloak** | Dagger and thrown knives | Feint and vanish | A feint that draws a block; a knife from hiding |
| **Shieldwall** | Shield, with others | Hold a line together | Shields beside each other guard each other; better with recruits (D90) |

Realms add their own as data ([settings.md](settings.md#weapons-different-ways-to-play)): a gothic scythe style, a machine realm's bolt-thrower drill.

### Crafts: studied magic

The old-school spellbook lives here. A craft is a discipline of working with magic without powers of one's own (D124, [lore.md](lore.md#studied-magic)). Its workings are slow, take components and often a place, and are learned from books in ranks, iron workings to diamond.

- **The first book of a craft teaches its iron workings.** Later books teach higher ones, and practice makes them surer and quicker.
- **A working above your rank risks backlash:** a rank above hurts and may fail, and two above can kill, as in the books.
- **Workings draw on the magic of the place,** so in a thin realm or a drained spot they are unreliable, and near a pool they are strong. Where you work matters.
- **Written in the symbols of magic:** a diagram's symbols have a structure that holds in every realm, as in the books. A book from another realm's people teaches the same structure in their hand.

**Ritual magic.** Diagrams drawn in chalk, salt or ash, with candles and components, over minutes.

| Rank | Workings |
|---|---|
| Iron | **Identify** (what a thing is and does); **Kindle** (a fire that lasts a night without fuel); **Sweeten Water** (make water safe); **Sense Pool** (how strong the magic is here, and which way it gathers) |
| Bronze | **Calm** (lower a pool for days, so fewer monsters form); **Seat Essence** (absorb an essence gently, without the pain); **Far Sight** (see a place you have marked) |
| Silver | **Call Rain** (or call it off); **Hallow** (a claim's ground resists monsters forming) |
| Gold | **Drain** (empty a pool, taking its magic as components); **Raise** (make a pool, deliberately) |
| Diamond | **Wake a Ford** (open a natural portal where the astral runs thin) |

**Wardcraft: protecting places.** Glyphs on doors, posts and ground.

| Rank | Workings |
|---|---|
| Iron | **Alarm** (know when something crosses); **Lock** (a door only you and yours open) |
| Bronze | **Glyph of Fire** and other traps, by component; **Quiet** (no sound passes) |
| Silver | **Barrier** (a wall of force across a gap); **Turn** (magic aimed through it weakens) |
| Gold | **Standing Ward** (a ring of stones around a claim that weakens monsters' rank inside) |
| Diamond | **Sanctuary** (no monster forms or enters, while it is kept fed) |

**Astral magic: the astral between places.** The old-school travel magic.

| Rank | Workings |
|---|---|
| Iron | **Sense Ford** (feel which way the nearest crossing lies) |
| Bronze | **Mark** (inscribe a marker stone at a place); **Recall** (return to your marker, alone, after a long cast) |
| Silver | **Fold** (a pocket of space: a bag that holds more than it should, weighing less) |
| Gold | **Gate** (open a short-lived portal to a marker, for a party) |
| Diamond | **Bridge** (a lasting portal between two of your claims) |

There is no fast travel between claims (D72, 2026-10-12): getting around quickly is the work of systems like this one, so astral magic is the travel network. A marker can be anywhere, claim or not. Recall and Gate cost components and take a long cast that can be broken, so travel is earned and planned. Portals players build (D72) and mounts (D100) are the other ways.

**Bindcraft: familiars and summons.**

| Rank | Workings |
|---|---|
| Iron | **Bind Familiar** (a small animal bound to you, such as an owl, cat, toad or crow: it scouts, fetches and warns) |
| Bronze | **Summon** (a creature from a monster's remnant, for a fight) |
| Silver | **Bind Construct** (a remnant in a body of wood, stone or metal that works for you) |
| Gold | **Command** (turn a monster of a lower rank for a while) |
| Diamond | **Bind Guardian** (a great creature bound to a place you hold) |

**Making with magic** (the books' artifice), crafts done at stations ([crafting.md](crafting.md)):

- **Alchemy:** potions and oils.
  - Iron: healing, mana, cure poison, night eyes.
  - Bronze: breathe water, warmth (D153), blade oils of fire, frost or venom.
  - Silver and up: rank-crossing draughts, and stranger things.
  - Drinking above your rank gives backlash, as eating does.
- **Inscription:** scrolls and wands, the old-school pocket magic.
  - A **scroll** holds one working, or one charge of a power its writer has, ready to release in a moment. A fighter can carry a scroll of Barrier or Recall.
  - A **wand** holds a few charges of one, and is refilled with components.
  - This is how slow crafts and others' powers reach a fight.
- **Enchanting:** a lasting property on gear, from a remnant and a working: a flaming edge, a feather-light pack, a cloak that keeps warmth, a blade that never dulls. Capped by the gear's rank.

## Other magic, beyond the list

What the developers asked about: magic-like things the list above leaves out.

- **Gods' gifts** (D156), kept simple for now: blessings at shrines (boons for a while), and thematic touches such as shrines, priests and holy places. Gods granting essences or stones of their choosing, as in the books, is for later.
- **Aura** ([lore.md](lore.md#aura)). Trained by doing: holding it in, pressing others with it, reading others'. It is a skill, and the one magic everyone has.
- **Sensing magic.** Seeing pools, remnants and the leaning of a place: a perception power for some, a skill for anyone over time (D93).
- **Food.** Food from magic ingredients gives boons (D101), and food above your rank gives backlash.
- **Remnants and cores as fuel.** What monsters leave is the reagent, the fuel and the trade good of every craft above, and of loot.
- **Loot with properties** ([settings.md](settings.md#loot)): found gear that rolled its own magic.
- **Taming** (D100). Not magic, but in the same family as bindcraft: wild animals by patience, monsters by binding.
- **Reading old tongues.** A skill for reading ruins' writing and old books, so higher books can be found and read. Proposed.

## What it asks of the engine

Technical, mine:
- A power is data: its parts and its ladder, kept on the character.
- The lists of candidates per essence are generated from the parts, the essence's tags and its anchors.
- A draw is a weighted choice the server makes and stores.
- Nothing about powers is written per power, which is D127's rule.

## Money that is magic

Decided (D167): money works as in the books. The currency is magic itself, condensed, so it can be spent or burned.

- **What it is:** **spirit coins**, as the books name them (D167): magic condensed into small hard coins with a rank, iron, bronze, silver, gold or diamond.
- **Where it comes from:** monsters leave it, and it forms where magic pools thickly. Culling is a living ([people.md](people.md#trade)).
- **What it buys:** everything settlements and traders sell. One rank's worth is many of the rank below.
- **What it burns for:**
  - **Fuel:** for workings, for magic items, and for a standing ward or a bridge kept running ([crafting.md](crafting.md)).
  - **A surge of strength:** absorbing one above your rank lifts your attributes to its rank for a short while, enough to strike or survive what you couldn't. Then it leaves you exhausted, and the backlash is worse the higher the coin.
  - **Components:** what some workings need, alongside cores and quintessence.
- **The same in every realm,** because it is the same magic and the same ranks everywhere ([lore.md](lore.md)). This answers [people.md](people.md)'s question of one currency or many: one, by the lore.
- **Why it suits us:** every purse is also a store of power, so spending is always a choice between buying, building and burning.

## Fragments of essences

Decided (D168): kept as components, and named as the books name them: **quintessence**, as in "Ember quintessence".

- **What they are:** a small fragment of one essence's kind. Not an essence: they can't be absorbed, and many don't make one.
- **Where they come from:** monsters and pools of that kind, more often than essences.
- **What they are for:**
  - workings that need an idea, such as a fire ward needing Ember quintessence;
  - enchanting gear with a property of that kind;
  - fuel for items of that kind.

  They are the reagents of the old school, by essence.

## Essences in the starting realm

Procedural, like every realm. There is no fixed list: the starting realm's essences come from its generated magic like any other's. One rule for it, proposed: the generator makes sure that near the start, every family is reachable among the commons (weapons, elements, beasts and body), so a first character can go any way. Its stronger pools hold the rarer essences of its own leaning.

## Open questions

0. Answered: **no picking among offers.** One draw, as in the books, with the most common effects shown at the ritual, and never numbers ([above](#before-you-awaken-what-is-known)).

1. Answered: **attributes** are kept (D164). **Monster cores** are kept (D165). **No fast travel between claims** (D72). **The starting realm's essences** come from its generation, with the rule above. **Old-school touches** follow the books: chanted spells, components used up, backlash.
2. Answered: **no separate tally of habits.** What tilts a draw is what you already have, your powers and your skills (2026-10-12).
3. Answered: **money is magic,** as in the books (D167).
4. Answered: **fragments of essences** are components (D168).
5. Answered: **their names** are the books': spirit coins and quintessence (2026-10-12).
6. **Can an essence be removed** or replaced? Maybe; the developers expect it to be complex and are not sure yet (2026-10-11).
