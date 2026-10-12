# Combat

Part of [game design 001](README.md). A proposal for the developers to react to; nothing here is decided unless it cites a decision.

## What the developers have said

- They like Enshrouded's combat and movement (D85).
- Threats are answered by holding bases and outposts, with recruits who defend (D90).
- **Combat is important** (D97), and the game is **third person**, with first person perhaps an option later (D97).

## Based on

**Enshrouded** for how a fight feels: real time, close or at range, with stamina paid for attacks, dodges and blocks. **He Who Fights With Monsters** for what makes our fights different: powers from essences ([essences.md](essences.md)), and ranks so far apart that a stronger monster is a different kind of danger, not a bigger number.

## How it would work

- **Real time, in the world.** You aim, swing, shoot, dodge and block. No turns, no pause.
- **Stamina is the limit.** Attacking, dodging, blocking, sprinting and climbing all draw on it (D61), so a fight is choices about when to spend it.
- **Weapons are skills by doing** (D88). Fight with a spear and you get better with spears: faster, stronger, new moves at steps. No weapon class is locked behind a choice.
- **Combat styles are learned**, from books and practice (D124): a way to use a kind of weapon, its stances and combinations, shaping how its skill plays. Proposed in [essences.md](essences.md#three-layers).
- **Archery is a skill of its own**, from the developers on 2026-10-11 (D117), and **each style of bow probably is too**: a short bow, a long bow, a crossbow, or a realm's own ([settings.md](settings.md#weapons-different-ways-to-play)), each a different way to shoot. Proposed: a style of bow builds on archery, so a good archer is not a beginner with a new bow, but is better still with the one they know.
- **Abilities come from essences** and sit on the action bar with your weapons (D66). They are what make one fighter unlike another.
- **Where a blow lands matters**, from the developers on 2026-10-11 (D119): hitboxes, headshots and critical hits. Proposed: a creature has a few zones, such as head, body and legs, each taking more or less from a blow; a head is small and worth more. A critical is a blow that lands where it counts, or at the right moment, rather than a roll of the dice, so it rewards aim and timing. Rank still caps what a blow can do.
- **Damage shows on the body, not as numbers**, from the developers on 2026-10-12 (D132): you see a target wounded. Perhaps later rather than for the MVP. Proposed, so it works on every model without art made for each: wounds marked where blows landed, in the creature's own space, so they move with it, drawn by one shader every creature shares (or as decals, which Bevy has); bleeding while badly hurt; and the body telling the rest, a limp, a slower step, a creature that holds back. Hit zones (D119) say where the wounds go.
- **Rank decides more than skill.** A monster a rank above you is dangerous whatever your skill; two ranks above is death. That is how the world tells you where not to go, without a marker (D92).
- **You judge an enemy by looking** (D93): its size, its look, how it moves, how others react to it. A skilled hunter or fighter reads more.
- **Armour and clothing** are worn in the equipment slots (D64), and trade protection against freedom of movement and warmth ([survival.md](survival.md)).
- **Recruits fight beside you** when they are guards, and defend your claims when you are away (D90).
- **Fighting together**: several players and their recruits against a surge or a raid is the big set piece of the game ([arc.md](arc.md)).
- **Going down** drops your pack where you fell (D84); there is no fail state (D18). See [survival.md](survival.md) for where you come back.

## What it asks of the engine

Technical, mine. With a server that decides everything (D51), a swing must feel instant on your screen and be confirmed by the server, so the client shows the hit at once and the server checks it against where things were when you swung. Hit shapes come from the collision design (D53, open). Many fighters in one place at once is a measured risk for a thousand players (D52).

## Questions

1. Answered: **combat is important** (D97).
2. Answered: **third person**, perhaps with first person as an option later (D97).
3. **Lock-on**, or always free aim?
4. **Can players hurt each other** by accident? There is no player against player (D2).
5. **How deadly is it?** How many hits a player takes from an enemy of their own rank.
