# Magic: essences, books and old-school spellcraft

Part of the game's design ([gameplay.md](gameplay.md), D89). Status: Open, a proposal for the developers (D163), 2026-10-12. Each item is as settled as the decision it cites. It builds on the lore ([lore.md](lore.md)) and on [essences.md](essences.md), and gives the examples each system needs. How the books do it is in [research/hwfwm-magic.md](../research/hwfwm-magic.md). We take the mechanics, never the content (D125): every essence, ability, style and working here is ours.

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
| Reagents in your pack: pearls, roots, ash | Components: monster remnants, herbs, cores and dusts, named by each realm's role ([settings.md](settings.md)). Workings use them up |
| Words of power, spoken as you cast | Every working has an incantation in the tongue of the people who wrote its book ([naming.md](naming.md)), spoken aloud. Two players' fire wards can sound different |
| Spell circles, first to eighth | Workings by rank: iron workings to diamond |
| Mark and Recall; Gate Travel | Deepcraft, below |
| Scrolls and wands | Inscription, below |
| Identify | Ritecraft's first working |
| A wizard's familiar | Bindcraft, below |
| Meditating to get mana back | Rest: mana returns faster sitting still, and Meditation is a skill by doing (D88) |
| Fizzling | Backlash: a working above your rank, or broken off midway, fails and hurts |
| Schools of magic | The ideas of essences, and the crafts |

## Essences

From the lore and the books' mechanics (D98), with our own essences:

- **Three essences and a confluence.** You absorb up to three; the fourth forms from their combination and is yours alone ([essences.md](essences.md#how-essences-would-work)). The confluence keeps its working name until [naming.md](naming.md) is settled.
- **Five powers to an essence.** The first wakes when you absorb it; each of the other four needs an awakening stone. Twenty powers in all, which is a whole character.
- **Rarity:** common, uncommon, rare, epic, legendary. As in the books, rarity is relative to a place: a frozen pool mostly gives Frost and rarely something stranger. Where you hunt decides what you can become (D91).
- **Where essences form:** in pools and in surges, and in monsters made there, so each realm's leaning decides what its pools give ([lore.md](lore.md#magic)).
- **Absorbing one hurts,** and can only be done at rest, in safety. Proposed: also possible as a ritual (ritecraft), which is gentler.
- **No essence is better than another.** They differ in how they play.

### Core essences

Our own list to start with, proposed. Each has an idea, its tags (which the confluence and awakening read), where it forms, and what its powers lean to. Generic ideas (fire, blood) appear in many games, the books included; the list, its tags and its powers are ours.

| Essence | Rarity | Tags | Forms in | Its powers lean to |
|---|---|---|---|---|
| **Blade** | Common | weapon, edge | Battlefields, armouries' ruins | Strikes, parries, conjured blades |
| **Bow** | Common | weapon, reach | Hunting grounds | Aimed shots, marking prey |
| **Shield** | Common | weapon, guard | Old forts | Blocks, guarding others, walls |
| **Hammer** | Common | weapon, force | Forges, quarries | Staggers, breaking, shock |
| **Fire** | Common | element, heat | Volcanic and burnt ground | Bolts, burning, light, warmth |
| **Water** | Common | element, flow | Springs, lakes | Healing, pushing, breathing under water |
| **Stone** | Common | element, earth | Caverns, cliffs | Hardening, walls, tremors |
| **Wind** | Common | element, air | High places | Speed, leaps, pushing, carrying sound |
| **Wolf** | Common | beast, pack | Forest and plain | Hunting, calling a pack, senses |
| **Bear** | Common | beast, strength | Deep forest | Toughness, mauling, rage |
| **Hawk** | Common | beast, sight | Cliffs, open sky | Far sight, diving strikes |
| **Root** | Common | growth, earth | Old woods | Snaring, rooting in place, mending |
| **Swift** | Common | body, speed | Anywhere things flee | Movement, dodges, quick strikes |
| **Might** | Common | body, strength | Anywhere things fight | Power, carrying, breaking |
| **Blood** | Uncommon | body, life, cost | Battlefields, hunting grounds | Powers paid in health, draining, bleeding |
| **Bone** | Uncommon | body, death | Graves, old kills | Armour, spikes, the dead |
| **Frost** | Uncommon | element, cold | Ice and high ground | Slowing, freezing, cold armour |
| **Storm** | Uncommon | element, sky | Peaks, open sea | Lightning, chaining, thunder |
| **Light** | Uncommon | force, holy | Sunlit holy places | Healing, revealing, burning the unholy |
| **Dark** | Uncommon | force, shadow | Deep caverns | Hiding, fear, blinding |
| **Venom** | Uncommon | beast, poison | Swamps, nests | Poisons, weakening, resistance |
| **Song** | Uncommon | mind, sound | Where people gather | Boons for allies, lulling, echoes |
| **Smoke** | Uncommon | element, veil | Burnt land | Concealment, choking, slipping away |
| **Ash** | Rare | death, heat, renewal | Burnt forest after the fire | Rising again, burning out, regrowth |
| **Tide** | Rare | flow, time | Coasts, the edges of pools | Ebb and flow: building up, then releasing |
| **Moon** | Rare | night, change | Where night is long | Strength at night, change of form |
| **Echo** | Rare | sound, repeat | Caverns, ruins | Repeating what you did, doubles |
| **Hunger** | Rare | body, take | Places that drain magic | Taking what others have: health, mana, power |
| **Rune** | Epic | knowledge, mark | Ruins of studied peoples | Marks that work later, traps, empowering crafts |
| **Wing** | Epic | beast, sky | The highest places | Gliding, then flight at high rank |
| **Omen** | Epic | fate, mind | Where something terrible will happen | Foresight, curses, luck |
| **Rot** | Epic | death, growth | Blighted ground | Decay that spreads, fungus, plague |
| **Ward** | Epic | guard, magic | Old held places | Protecting places and others, turning magic aside |
| **Abyss** | Legendary | the Deep, void | Where realms wear thin | Pulling, swallowing, stepping through the gap |
| **Crown** | Legendary | rule, presence | Where the mighty died | Commanding, presence, followers |
| **Ford** | Legendary | crossing, the Deep | Natural portals | Moving between places: blinks, doors, gates |

More come per realm as data: a gothic realm adds Grave and Mist, a machine realm Gear and Spark. Each is a row like these.

### Confluences: from tags, by rules

A confluence is worked out from the three essences' tags by a few rules, so players can learn the patterns and trade recipes:

- **Kindred:** three that share a family (say three elements) give that family's greater form. Fire, Stone and Wind give **Elemental**.
- **Opposed and balanced:** two opposites plus a balancing one give a union of the two. Light, Dark and Song give **Twilight**.
- **Opposed and escalated:** two opposites plus a strength give a violent one. Fire, Frost and Might give **Shatter**.
- **Beast and element:** a beast with elements gives a creature of that element. Hawk, Storm and Wind give **Thunderbird**.
- **Body and cost:** two body essences with Blood or Hunger give something that spends itself. Blood, Might and Hunger give **Berserk**.
- **Weapon and guard:** Blade, Shield and Swift give **Duellist**; Bow, Hawk and Wind give **Far Shot**.
- **Death and growth:** Bone, Root and Rot give **Barrow**; Ash with any growth gives **Phoenix**, rarely.

The usual result is the likeliest, not certain, as in the books' later volumes: a common recipe gives its confluence most of the time and sometimes a neighbour. Recipes become knowledge players share and argue about, which is the old-school feel of a game with secrets.

## Awakening stones: drawing a power

Proposed: the books' second kind of thing kept as it is, with its name to settle (D158 proposes "quickening stone"). A stone is found as essences are, in pools, surges and the remains of strong monsters. Each has a kind that pulls the draw its way:

| Stone, examples | Rarity | Pulls toward |
|---|---|---|
| Of the Hand, of the Eye, of the Foot | Common | Strikes; perception; movement |
| Of Embers, of Rain, of Stone | Common | That element's effects |
| Of the Wall, of Mending | Uncommon | Guarding; healing |
| Of Wrath, of Patience | Uncommon | Big single blows; slow builds that release |
| Of the Pack, of Calling | Rare | Summons and familiars |
| Of Ruin, of the Crowd | Rare | Areas; many targets |
| Of the Gate, of Ages | Legendary | Movement through the Deep; powers that grow with time |

### How a draw works

When you use a stone on an essence, the server draws a power from that essence's list. The list is long and generated (below). Every candidate has a weight, which is multiplied by:

1. **The stone's kind:** strongly toward what it pulls. A rarer stone pulls harder, but anything is possible.
2. **Your other essences:** candidates sharing tags with them are likelier, so a character comes out coherent. Absorbing all three before awakening any gives the best fit, as in the books, which is a choice players can make.
3. **What you already have:**
   - kinds you lack are likelier, so no one ends with five passives;
   - kinds you have many of are less likely;
   - some kinds are one to a person (an aura, a perception) or one to an essence (a summon, a familiar).
4. **What you know:** a crafter of ritecraft draws more ritual-like powers, and a practised swordsman more blade strikes. Your skills and books shape your powers.
5. **What you have been doing:** proposed, our addition. The game keeps a short record of how you play, such as stalking, healing others, fighting at range or in groups, and leans the draw toward it. Powers follow habits, which is the lore's "who you are".
6. **Where you are:** a little toward the leaning of the realm you awaken in. Awakening a stone in a death realm leans a little toward death.

The draw is made once, by the server (D51), and kept: it is a change to you, stored like any other.

**At the moment you awaken, the power's whole ladder is made:** what it does at iron and what each rank adds up to diamond, as in the books. You can't reroll it.

### What a power is made of

So there is a lot to draw from without writing each power by hand, a power is generated from parts, and its rank sets how much it may do:

| Part | Choices, for example |
|---|---|
| **Kind** | strike (stamina), spell (mana), conjuration, summon, familiar, aura, perception, passive, movement, zone |
| **Delivery** | touch, weapon, bolt, cone, area, self, an ally, a place, a mark that triggers later |
| **Effect** | harm of an idea (fire, cold, poison, holy), heal, shield, push or pull, slow, root, blind, reveal, hide, change form, move |
| **Twist** | lingers, chains, stacks, builds up then releases, costs health instead of mana, stronger at night, stronger near water |
| **Cost and wait** | mana, stamina or health; a cooldown |
| **Ladder** | an effect added at each rank, iron to diamond |

- **The essence's idea limits the parts.** Fire can't heal, except through Ash.
- **The rank sets a budget,** so a power drawn at random is never better than one picked by hand; it differs.
- **Hand-written anchors** sit in every list with high weight: the classics a player expects of an idea, such as a ball of fire for Fire, a healing touch for Water, a howl that calls wolves for Wolf. Those are the nostalgia, while the generated ones are the surprise.
- **Names come from [naming.md](naming.md),** the idea plus what the power does: Cinder Bolt, Wolf's Call, Bedrock.

### Three worked examples

**Cinder Bolt** (Fire; spell; a bolt):

| Rank | What it does |
|---|---|
| Iron | Throws a bolt of fire that burns for a few seconds |
| Bronze | The burn spreads to anything close to the target |
| Silver | Burning targets take more from all fire |
| Gold | Hold the cast to throw three |
| Diamond | A target burned to death leaves a burning patch that keeps burning |

**Wolf's Call** (Wolf; summon; a place):

| Rank | What it does |
|---|---|
| Iron | Calls a wolf of your rank to fight beside you for a while |
| Bronze | Two wolves |
| Silver | The wolves share your marks: what you mark, they hunt |
| Gold | Wolves that fall rise once more |
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
- **Capped at your own rank.** Your rank rises as your powers do ([essences.md](essences.md)).
- **Crossing a rank may need a key** (D126).
- **Monster cores can hurry it,** at the books' price: each one used makes practice count for less. Proposed, Open: worth having for the choice it gives. Hurry now, and pay later.

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
- **Spoken aloud:** each working's incantation is in the tongue of its book's people.

**Ritecraft: ritual magic.** Diagrams drawn in chalk, salt or ash, with candles and components, over minutes.

| Rank | Workings |
|---|---|
| Iron | **Identify** (what a thing is and does); **Kindle** (a fire that lasts a night without fuel); **Sweeten Water** (make water safe); **Sense Pool** (how strong the magic is here, and which way it gathers) |
| Bronze | **Calm** (lower a pool for days, so fewer monsters form); **Seat Essence** (absorb an essence gently, without the pain); **Far Sight** (see a place you have marked) |
| Silver | **Call Rain** (or call it off); **Hallow** (a claim's ground resists monsters forming) |
| Gold | **Drain** (empty a pool, taking its magic as components); **Raise** (make a pool, deliberately) |
| Diamond | **Wake a Ford** (open a natural portal where the Deep runs thin) |

**Wardcraft: protecting places.** Glyphs on doors, posts and ground.

| Rank | Workings |
|---|---|
| Iron | **Alarm** (know when something crosses); **Lock** (a door only you and yours open) |
| Bronze | **Glyph of Fire** and other traps, by component; **Quiet** (no sound passes) |
| Silver | **Barrier** (a wall of force across a gap); **Turn** (magic aimed through it weakens) |
| Gold | **Standing Ward** (a ring of stones around a claim that weakens monsters' rank inside) |
| Diamond | **Sanctuary** (no monster forms or enters, while it is kept fed) |

**Deepcraft: the Deep between places.** The old-school travel magic.

| Rank | Workings |
|---|---|
| Iron | **Sense Ford** (feel which way the nearest crossing lies) |
| Bronze | **Mark** (inscribe a marker stone at a place); **Recall** (return to your marker, alone, after a long cast) |
| Silver | **Fold** (a pocket of space: a bag that holds more than it should, weighing less) |
| Gold | **Gate** (open a short-lived portal to a marker, for a party) |
| Diamond | **Bridge** (a lasting portal between two of your claims) |

Mark and Recall must sit with fast travel (D72), which already exists between claims. Proposed: Recall and Gate reach markers too, which needn't be claims, at a cost and with a long cast that can be broken.

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

- **Gods' gifts** (D156). A realm's gods can give their followers an essence or an awakening stone of their choosing, and can take it back, as in the books. Also blessings at shrines (boons for a while) and priests among the peoples who work them.
- **Presence** ([lore.md](lore.md#presence)). Trained by doing: holding it in, pressing others with it, reading others'. It is a skill, and the one magic everyone has.
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

## Open questions

1. **Attributes.** The books bind each essence to one of four attributes (Power, Speed, Spirit, Recovery), and rank comes from them. Do we want attributes at all, or let health, stamina, mana and skills carry it?
2. **Monster cores** that hurry powers at a later cost: in or out?
3. **Habits shaping draws** (point 5): in, or too hidden?
4. **Is Mark and Recall too strong** beside fast travel between claims?
5. **The core list:** which of these essences should the starting realm give, and which wait for others?
6. **Old-school touches to keep or cut:** spoken incantations, reagents used up, fizzling.
