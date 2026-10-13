# Progression: skills, ranks and unlocks

Part of the game's design ([gameplay.md](gameplay.md), D89). This page owns how you grow: the three ways, skills, the rank ladder, crossing a rank, and how things are unlocked. Essences, powers, attributes, cores and mana are in [magic.md](magic.md).

Status: the developers have said that the player has many skills that grow by doing (D88, Decided), that a research system unlocks things to make (D68, Decided, how it works Open), and where growth comes from (D124). The ranks are the books' (D159, Decided), and a person's rank comes from their four attributes (D164, Decided). Everything else is a proposal. Skills by doing are in the tech demo; the rest is not built.

## Three layers

From the developers on 2026-10-11 (D124): **magic and special abilities come from essences; everything else comes from other sources**, some from just doing it, some from books. In the books some magic, such as astral magic, is a technique you study, and so are combat styles. So there are three ways to grow, not two. The books' mechanics can be used; which disciplines and styles there are, we invent (D125).

| | Skills | Studied techniques | Essences |
|---|---|---|---|
| What | What you can do with your hands: swim, climb, chop, smith, a weapon | Techniques anyone can learn: combat styles, and crafts of working with magic such as astral magic | Powers: magic and special abilities, what makes you more than a person |
| How you get one | By doing it (D88) | By study: from a skill book or a teacher (D92), then practice | By finding and absorbing an essence |
| How many | Many, small | A few, each deep | A few, large |
| How it grows | By doing it more | By practice, and the next rank's book or teacher | Powers by use, through ranks |
| What it says about you | What you have practised | What you have learned | Who you are |

Studied techniques are worked out with examples in [magic.md](magic.md#skill-books): combat styles and crafts, each learned from skill books in five forms, iron to diamond. Essences, powers and attributes are in [magic.md](magic.md#essences).

## Player skills

From the developers, 2026-10-10 (D88): there is a player progress system, and they really like games with **a lot of skills, so you always have a sense of progression**. Doing a thing gives you its skill automatically, and doing it more improves it.

Proposed:

- **Skills are many and small**, so something is always about to improve. A skill is an entry in a list, like an item or a buildable (D127, D136): its name, what it does at each rank, and what raises it.
- **Two ways forward**: getting better at skills you have, by using them; and gaining new ones, by doing, and perhaps by finding, a culture (D86) or a teacher.
- **Skills are yours.** They go with you to every realm, as your gear does (D86).
- **Felt rather than counted** (D93): a skill improving shows in how the action goes, and reads on the character screen.
- **In the game's model** (D58): a player's skills are a component; using one is an action the server checks and credits (D51), so a thousand players cannot grant themselves ranks.

## Skills on the rank ladder

Asked by the developers on 2026-10-11: can skills rank up iron, bronze and so on? Proposed, Open: **yes, on the same ladder as everything else** (D159), so monsters, places, portals, materials, people and skills are all read one way.

- **Each skill climbs iron, bronze, silver, gold, diamond**, with many small steps inside a rank that come often, and a rank-up now and then as a milestone. That is the feel of the grinding games with many skills: always a step coming, sometimes a big one.
- **A rank-up opens something**, not only bigger numbers: a technique, a kind of recipe, a move.
- **A skill's rank decides what rank of thing you can work.** An iron-rank smith cannot work silver-rank metal; an iron-rank miner chips at a bronze-rank vein slowly, if at all. Climbing a skill then has a place in the world to go to, the way level requirements pull a player along in RuneScape.
- **Separate ladders.** Your rank as a person comes from your four attributes, which rise with the powers bound to them (D164, [magic.md](magic.md#attributes)); it decides what you can survive. Skills decide what you can make and do, and do not raise attributes. A gold-rank crafter can be an iron-rank fighter.
- **Recommended: skills may run ahead of your own rank**, so a crafter or gatherer has a whole game of their own.
- **Recruits climb the same ladder** in their work (D123, [tribe.md](tribe.md)).
- **Grinding, but never a chore** (D122): every step should feel like progress.

## Crossing to the next rank

From the developers on 2026-10-11, unsure (D126, Leaning): you may often need a way to advance to the next rank, such as a book or someone to help you unlock it.

Proposed, Open: **steps within a rank come from doing; crossing to the next rank needs a key**, one of:

| Key | What it is | Why it is good for the game |
|---|---|---|
| **A book** | Found, bought or taken from a ruin: the next form of a style, a craft or a skill | A reason to explore (D91) and to bring things home |
| **A teacher** | Someone who already stands at that rank: a villager, a recruit, a master in another realm | People matter: helping a settlement or rescuing someone can open your next rank (D90, D123) |
| **A feat** | Something done once at the edge of what you can do: working a higher-rank material, a hard kill | Earned by daring, not by a list |

Which keys a skill takes would differ: a craft might take a teacher or a feat, a style a book or a teacher. The rank-up should feel like an event, not a number ticking over, and it keeps the grind from being only repetition (D122): you can practise to the edge of a rank, but crossing it sends you out into the world. How powers cross a rank, and what monster cores do to it (D165), is in [magic.md](magic.md#raising-powers).

## Research and unlocks

From the developers (D68): **a research system unlocks technology and things to make**; the build menu shows what you have unlocked (D67). How something is researched is open. The design proposes learning by finding rather than a tree of technology ([crafting.md](crafting.md#learning-what-to-make)), with realms opening new things as Terraria's zones do (D85).

Ways other games do it, to react to:

| Game | How you unlock |
|---|---|
| Subnautica | Scan things you find in the world, often in several fragments |
| Rust | Spend scrap at a workbench; learn by taking found items apart |
| Valheim | Pick up or stand near a new material, and what it makes appears |
| Terraria | Go to other zones, find new materials, and they open new things to make, many only for their look; a station nearby is often what is needed. Named by the developers as an inspiration (D85): ours would be realms (D87) |

What any answer needs:

- **Unlocks are data**, like buildables (D127, D136): each opens items, buildables or upgrades.
- **Something that opens one**: finding a material, a skill book, a culture (D86), a skill's rank, or a mix.
- **Whose it is.** With a thousand players on a server (D52): per player, or shared within a tribe (D166)?

## Questions

1. **How is something unlocked?** Finding, a skill's rank, a station, a culture, or a mix?
2. **Per player, or per tribe?**
3. **Can anything be lost** once unlocked?
4. **What rank does a player start at**, and how long is a rank? Do ranks pace the realms, with the starting realm the first?
5. **What do skills cover?** Fighting, movement, gathering, building, survival, crafting, or all of them?
6. **Are skills chosen?** A limited set, so players differ, or eventually everything?
7. **Skills and powers together**: does a high skill change what a power does (a skilled archer's fire arrow)?
8. **Skills on the rank ladder**, as proposed above, and **may a skill run ahead of your own rank**?
9. **Which grinding games are the reference** for many skills: RuneScape, Ultima Online, Wurm Online, Star Wars Galaxies, the Elder Scrolls games, or others?
10. **Crossing a rank**: does it need a key, and which: a book, a teacher, a feat?
