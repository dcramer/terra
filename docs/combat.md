# Combat

Part of the game's design ([gameplay.md](gameplay.md), adopted as the source of truth on 2026-10-12, D89). Each item is as settled as the decision it cites; the rest is a proposal for the developers to react to.

## What the developers have said

- They like Enshrouded's combat and movement (D85), and **Enshrouded is the baseline for how combat plays** (D180): fast-paced, fun, with good animations and smooth play; an action RPG, not pressing a button to fire an ability.
- Threats are answered by holding bases and outposts, with recruits who defend (D90).
- **Combat is important** (D97), and the game is **third person**, with first person perhaps an option later (D97).

## Based on

**Enshrouded is the baseline** (D180) for how a fight plays: a fast-paced action RPG, real time, close or at range, with good animations and smooth play. A fight is weapon play, timing and footwork, with stamina paid for attacks, dodges and blocks; abilities add to it rather than being it. What Enshrouded does is in [research/enshrouded-combat.md](../research/enshrouded-combat.md). **He Who Fights With Monsters** for what makes our fights different: powers from essences ([magic.md](magic.md#essences)), and ranks so far apart that a stronger monster is a different kind of danger, not a bigger number.

## How it would work

- **Real time, in the world.** You aim, swing, shoot, dodge and block. No turns, no pause.
- **Read, defend, punish**, proposed after Enshrouded: light attacks that chain and heavy ones that break a guard; a dodge roll; a block, and a parry raised just as a blow starts; enemies that tell their attacks, so you learn each kind; parries and heavy blows that stagger an enemy, leaving it open for a moment to a blow that counts (a critical by timing, D119). The stagger shows on the body, which reels and drops its guard, not as a bar (D132).
- **Stamina is the limit.** Attacking, dodging, blocking, sprinting and climbing all draw on it (D61), so a fight is choices about when to spend it.
- **Weapons are skills by doing** (D88). Fight with a spear and you get better with spears: faster, stronger, new moves at steps. No weapon class is locked behind a choice.
- **Combat styles are learned**, from books and practice (D124): a way to use a kind of weapon, its stances and combinations, shaping how its skill plays. Proposed in [magic.md](magic.md#combat-styles).
- **Archery is a skill of its own**, from the developers on 2026-10-11 (D117), and **each style of bow probably is too**: a short bow, a long bow, a crossbow, or a realm's own ([settings.md](settings.md#weapons-different-ways-to-play)), each a different way to shoot. Proposed: a style of bow builds on archery, so a good archer is not a beginner with a new bow, but is better still with the one they know.
- **Abilities come from essences** and sit on the action bar with your weapons (D66). They are what make one fighter unlike another, and they add moves and options to weapon play rather than replacing it with a rotation of cooldowns (D180).
- **Where a blow lands matters**, from the developers on 2026-10-11 (D119): hitboxes, headshots and critical hits. Proposed: a creature has a few zones, such as head, body and legs, each taking more or less from a blow; a head is small and worth more. A critical is a blow that lands where it counts, or at the right moment, rather than a roll of the dice, so it rewards aim and timing. Rank still caps what a blow can do. In the tech demo: zones are spheres round the joints of each creature's model that its kind names, so they move as it does; a head takes twice what a body does, legs 0.6; a blow on a creature that has not noticed anyone, grazing or resting, takes half as much again, so stalking pays; an arrow strikes the first zone it passes through, a swing the one under the aim. Proposed numbers.
- **Damage shows on the body, not as numbers**, from the developers on 2026-10-12 (D132): you see a target wounded. Perhaps later rather than for the MVP. Proposed, so it works on every model without art made for each: wounds marked where blows landed, in the creature's own space, so they move with it, drawn by one shader every creature shares (or as decals, which Bevy has); bleeding while badly hurt; and the body telling the rest, a limp, a slower step, a creature that holds back. Hit zones (D119) say where the wounds go.
- **Rank decides more than skill.** A monster a rank above you is dangerous whatever your skill; two ranks above is death. That is how the world tells you where not to go, without a marker (D92).
- **You judge an enemy by looking** (D93): its size, its look, how it moves, how others react to it. A skilled hunter or fighter reads more.
- **Armour and clothing** are worn in the equipment slots (D64), and trade protection against freedom of movement and warmth ([survival.md](survival.md)).
- **Recruits fight beside you** when they are guards, and defend your claims when you are away (D90).
- **Fighting together**: several players and their recruits against a surge or a raid is the big set piece of the game ([arc.md](arc.md)).
- **Going down** drops your pack where you fell (D84); there is no fail state (D18). See [survival.md](survival.md) for where you come back.

## What it asks of the engine

Technical, mine. Feel is mostly animation (D180): a move starts the moment it is pressed, chains into the next, can be cancelled into a dodge, and a hit lands with weight, the struck body reacting. With a server that decides everything (D51), a swing must feel instant on your screen and be confirmed by the server, so the client shows the hit at once and the server checks it against where things were when you swung. Hit shapes come from the collision design (D53, open). Many fighters in one place at once is a measured risk for a thousand players (D52). How bodies move and fight, in layers shared by players and monsters, is proposed in [movement.md](movement.md).

## Questions

1. Answered: **combat is important** (D97).
2. Answered: **third person**, perhaps with first person as an option later (D97).
3. **Lock-on**, or always free aim?
4. **Can players hurt each other** by accident? There is no player against player (D2).
5. **How deadly is it?** How many hits a player takes from an enemy of their own rank.
