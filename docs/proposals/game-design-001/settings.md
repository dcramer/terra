# Settings: how any theme works

A proposal for game design 001: one way for any realm's theme to work, medieval, machine, wild, gothic or another, so that each feels like itself, none is better than the others, a new one is data rather than new code, and dozens of generated realms still fit one lore. Open, for the developers to react to.

## What the developers have said

- Realms may differ in setting, "like fantasy or primitive or futuristic", shown in their cultures, which may teach different things and from which you may recruit (D86, Leaning).
- The art target is the four-world mood board: a medieval town, a techno-punk street, fantasy wilds and a gothic graveyard, in one look ([art.md](../../art.md)).
- On 2026-10-11, about a technological realm: no machine guns; advanced but on theme with magic, and not superior. Whether its makers are gone or still at work could be either. What matters is **ways to make different kinds of themes work, as the visual design goals do.**
- Also on 2026-10-11: there may be **dozens of generated realms**, so what is needed is **techniques that make each one fit the lore**, not a handful of hand-made worlds.
- Standing: one art style (D14), your gear is your gear in every realm (D86), no science fiction toolsets.

## The idea: roles, as the art has palette roles

The art makes many realms from the same models by giving every part a **palette role** (wood, stone, cloth, metal, accent, glow) that each realm fills with its own colours, under one material and one light ([research](../../../research/ai-models-for-art.md#the-bake-off-second-round-style)). The same model is a medieval cart, a gothic hearse or a brass machine by what fills its roles.

Proposed: **design works the same way.** The game defines a fixed set of **gameplay roles**: a way of using magic, a place where it pools, a culture, materials, stations, weapons, a speciality. A setting is data that fills each role with its own thing. Systems are written once against the roles (D59: a new world is a new theme, not new code).

Two rules keep settings equal:

1. **Rank is the only power axis.** Every realm has iron to diamond ranks ([essences.md](essences.md)). A setting changes how things look and work, never how strong they are: a bronze-rank crossbow and a bronze-rank bow hit about as hard.
2. **A setting is a way of using magic.** Nothing in any realm runs on anything else. "Advanced" is a style of using magic, not more of it: engineered in one realm, wild in another, corrupted in a third.

## The roles

| Role | What every setting must say | The rule that keeps it fair |
|---|---|---|
| **Magic** | How its people and land use magic | The same magic and mana underneath (D102) |
| **Pooling** | Where magic gathers, and what forms there (D103) | Monsters of every setting follow the same rank rules |
| **Wildlife** | What lives there that is not a monster | Food, hide and other materials in the same roles |
| **Cultures** | Who lives there, how they build and what they want (D86, D95) | Built from the same building parts (D108); recruits work the same way (D90) |
| **Materials** | What fills the material roles: a soft, a hard, a metal, a cloth, a rare | Same roles, same counts in recipes; a realm's rare material is what draws you there |
| **Stations** | Its look for each station role: making, smelting, cooking, a special one | Same station kinds and timing (D109) |
| **Weapons** | Its look for each weapon role: one-handed, two-handed, ranged, a focus for abilities | Same numbers by role and rank; a setting adds a small twist, never a bigger number |
| **Speciality** | What it teaches that no other setting does | One field each, all needed, none the top |
| **Look** | Its palette, its kit of models, its light | One art style, the same material and light (D14) |

## Four settings, from the mood board

| Role | Medieval town | Techno-punk street | Fantasy wilds | Gothic graveyard |
|---|---|---|---|---|
| Magic | A craft of guilds and charms | Engineered: machines that run on cores | Wild and living, in beasts and places | Corrupted: death, curses, souls |
| Pooling | Old mines, battlefields | Ruined works, power sinks | Groves, springs, beast dens | Graves, crypts, blighted ground |
| What forms | Bandit-like monsters, wraiths of old wars | Constructs gone wrong, guardians at their posts | Great beasts, fey things | The undead, haunts |
| Cultures | Farmers, guilds, a lord's town | Tinkers among the works, or the makers themselves | Tribes, druids, hermits | Wardens, gravekeepers, a faded nobility |
| Rare material | Fine steel | Cores and brass | Heartwood, living crystal | Bone-silver, grave dust |
| Special station | A guild's workshop | A core-fired engine | A grove altar | An embalmer's table |
| Ranged weapon | Crossbow | Bolt-thrower on a core | Bow | Hand crossbow of bone |
| Speciality | Trade and law: shops, prices, hiring | Holding a place: automation, defence, logistics | Personal power: essences, taming, herbs | Death itself: what you lose on dying, curses, wards |
| Look | Timber, plaster, red tile, warm light | Brass, crystal, steam and glow, neon-like magic | Moss, bark, flowers, soft light | Grey stone, iron, mist, cold light |

The techno-punk street in more depth, with its risks, is [a realm of machines](techno-realm.md).

## Making generated realms fit the lore

Four hand-made settings do not reach dozens of generated realms. Proposed: a generator that **starts from a cause and derives the rest**, so every realm is explained by the same few ideas and none is a random pile of themes. After Dwarf Fortress and Caves of Qud, which generate histories that read as lore, and No Man's Sky, whose fiction explains why its worlds repeat.

**1. One cosmology that expects variety.** Proposed, for the fiction ([arc.md](arc.md)): realms are many places in one cosmos, joined by portals, and **each is shaped by the magic that dominates it**. That one premise explains why realms differ so much, why their peoples use magic in different ways, and why portals lead from one to another, without a story written for each.

**2. A few seeds, everything else derived.** A realm is generated from a handful of choices, and every role is worked out from them by rules:

| Seed | Chooses | Example |
|---|---|---|
| **Affinity** | The kind of magic that dominates: growth, death, fire, iron, storm, crystal, and so on | Death: graves pool, the undead form, gothic cultures, bone-silver |
| **Climate** | Its land and weather, from the planet's theme (D59) | Cold and misty suits death; a hot one gives a desert of the dead |
| **Age** | Where its people are in their history: rising, thriving, declining, fallen, returning | Fallen: ruins and guardians; rising: settlements still building |
| **Rank** | How strong it is ([arc.md](arc.md)) | Iron for a starting realm, gold for a far one |

Each affinity names a **setting template**: how its people use magic, what pools and forms, its material and station roles, its palette. Climate and age bend the template; the seed makes each place on the realm unlike the others. The four mood-board worlds become four templates among many, and a new template is data.

**3. A generated history, left in the land.** Each realm gets a short history of a few events, made by rule from its seeds: a people rose, used the magic in their way, and something happened, such as overreach, war, a plague or a portal opening. The history is never shown as text. It is **built into the world**: ruins where the old works stood, magic pooling where it was overdrawn, the culture that survived and what it believes, names on old stones. Players read a realm's past from its places and people (D92), as Enshrouded's and Kenshi's are read.

**4. Threads that cross realms.** Realms feel like one cosmos when some things recur:

- **Peoples who travel**: traders, pilgrims or refugees found in many realms, carrying news and goods between them.
- **An old people who built everywhere**: the same kind of ruin in many realms, from one ancient culture, a mystery that spans the game.
- **The long threat**, if there is one (premise B in [arc.md](arc.md)), touching many realms in their own way.
- **Your own mark**: claims, recruits and what you carry go with you between realms (D86).

**5. Rules a generator must keep.** What makes a realm fit, checked whatever the seeds: everything runs on magic; nothing outclasses its rank; no science fiction toolsets; one art style; a culture builds with the same parts as everyone (D108); and a few combinations are refused where they would contradict each other, such as a fire affinity in a frozen climate without a reason in its history.

**6. Hand-made anchors.** The starting realm, and perhaps a few landmark ones, are made by hand and set the tone. Generated realms are built from hand-written templates, names and history events, so what is generated is the combination, never the content from nothing. Names come from per-culture sound sets, so a gothic people and a growth people do not name things alike.

## Why roles

- **A new setting is a template**, not new systems: it fills the roles, picks a palette and a kit, and the generator and the game run it. That is what makes dozens of realms (D17) possible.
- **No setting outclasses another**, because power lives in rank and roles share numbers. A player moves between realms for what they teach and what they hold, not to trade up.
- **It keeps themes on theme.** Every role is filled with a way of using magic, so a machine realm cannot drift into science fiction and a gothic one cannot drift into horror without rules.
- **It matches how the art is being made**, so a setting's look and its play are one piece of data.

## How it would be made

A setting is part of a realm's theme (D59): a list of what fills each role, with its palette and its kit of models. Recipes, stations, weapons and creatures name roles, and the setting says what each is called, looks like and is made of. The sandbox does a little of this already: the stations' screens and the build menu read their lists from data, and everything built uses one catalogue (D108, D109).

## Questions for the developers

1. **Are these the right roles**, and is anything missing, such as how a setting's day, weather or music differ?
2. **Is "a twist, never a bigger number" the right rule** for a setting's weapons and tools?
3. **Do the four specialities sound right**, and what would the primitive setting's be?
4. **Should settings mix**, such as a gothic quarter in a medieval town, or is a realm one setting?
5. **Is "each realm is shaped by the magic that dominates it" a premise you would want** for the fiction, or should the cosmology be left open?
6. **An old people who built everywhere:** a thread worth having, or too close to a written story?
