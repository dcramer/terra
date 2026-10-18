# The troll fight: a demo scene

Status: the developers' brief, 2026-10-17: "for lake one, to make things more interesting - have an agent generate like a really high fidelity realistic troll (largeish one) and have our guy fight it for some time. It will be a bigger task because it needs to do some modeling, then have it be animated with like some attack and movement, and our guy has to have a sword/shield and move sideways and have attack animations and it needs to be engaging." and "one off clips don't have to have music on". A demo for the trailer (docs/trailer.md, Scenes), not a combat system: how the game fights is [combat.md](combat.md), still to be built on, and nothing here decides it. After five rounds the fight stopped at 6/10 for want of a movement system under it, proposed in [movement.md](movement.md).

## What it is

A film of 20 to 30 seconds on the mountain lake's south shore (`lake`, the open gravel between the forest's edge and the water) in the low morning sun: a troll about 4 m tall comes out of the trees at a man with a sword and a round shield, and they fight. It is a scene (`tools/sandbox/trailer/scenes/troll.ts`): the real game drawing the place, the two fighters and what their blows raise, with the choreography and the camera scripted on frames, as the dragon's is.

## The fighters

- **The troll** is a character made as ours are (the character skill): a design sheet in a T-pose, a Tripo model and auto-rig on its Unreal skeleton, converted onto our skeleton and checked with the rig checker, exported at 4 m. On our skeleton it plays clips made for a person, slowed for its size (a creature twice our height moves about 1.5 times slower, by Froude scaling, which is what makes it read as heavy). Checked on a workbench sheet against high-fidelity game trolls (The Witcher 3's rock trolls, God of War's, Skyrim's) before it is animated.
- **The man** is the base body as the player's template, dressed, with a textured sword in the right hand and a round shield in the left (held looks, `assets/models/held/`).
- **Their clips** are made with Kimodo (`tools/characters/motion.ts`), as the game's own made clips are, and must pass the clip checker:
  - troll: idle, a lumbering walk, an overhead two-handed smash, a sideways sweep, a hit reaction, a roar;
  - man: a guard stance behind the shield, strafing left and right in guard, three sword attacks, a block, a dodge (the game's roll), a hit reaction.
  - Strafing needs a clip that travels sideways while facing ahead: `motion.ts` takes the way a clip travels relative to where it faces (`way`), holding the take's heading.

## The beats

About 21 s; every blow has a wind-up the man reads, and every landing a reaction:

1. The troll comes out of the trees and roars; the man raises his shield (0-4 s).
2. They circle: the man strafes, the troll turns to keep him in front (4-6 s).
3. Overhead smash: a long wind-up, the man dodges aside, the blow lands in the gravel in a burst of dust and stones (6-8 s).
4. The man steps in and cuts twice at its leg; it flinches, then staggers (8-10.5 s).
5. The sweep: the man takes it on his shield and is driven back into the water's edge (11-12.5 s).
6. It comes straight on and smashes again: he rolls clear and the blow lands in the shallows where he stood, the splash thrown over him (12.5-14.5 s).
7. His combination of three from the water's edge, at its front; the troll reels back up the shore and roars (15-21 s).

## How it is filmed

- **Set pieces told over the remote** (`play::staged`): a staged figure is spawned from a model, stands on the ground, plays a clip it is told, turns to face a point and moves to a point at a pace, its clip's feet matched to the pace. What its blows raise is told too: a burst of dust and stones on ground, a splash and rings on water. A scene drives them as `play::fliers` flies the dragon; when creatures think, their minds will tell the same things.
- **Timed as shipped melee is** ([research/melee-feel.md](../research/melee-feel.md)): each blow from its clip's measured moments, its wind-up held at the top, its strike fast, a hit-stop freezing both fighters for 6 to 12 frames on contact (a figure's own time slowed, `staged.rate`), its follow-through played out, and the next move started in the last one's recovery, never from an idle. The camera's shake starts as the hit-stop lets go, so the frozen frame of contact is sharp.
- **Blows land on the body, not on the place** (round 3): a clip's own step or stagger is carried in its hips (the troll's reel goes 2 m back), so the scene knows each clip's measured hips travel and places the attacker by where the target's body is on the frame of contact, 1.65 m from the troll's middle for a sword blow, its contact the moment the sword's tip, followed through the skeleton, crosses ahead of him (round 4); the next clip is blended in with the figure moved to where the hips were, as an engine takes root motion.
- **Every blow that lands is answered** (round 5): the struck figure flinches (its back, neck and arms bent over its clip for a fifth of a second, as an additive hit layer) or is thrown into its hit clip, blended in over 0.3 s so its head rises rather than jumps. The camera follows the fighters averaged over a third of a second, so a clip giving way to the next never jolts it, and each contact falls inside a shot opened on the strike, never on a shot's first frames.
- **Clips fitted to the body**: clips made on a person put the troll's hands inside its belly and the man's shield through his chest, so a figure's bones can be turned over its clips (`staged.bend`), as a retarget pose fits a clip to a heavier body: the troll's upper arms out and forward, the man's shield arm forward.
- **The camera** is close and moving but readable, as third-person action games frame a big enemy (God of War's over-the-shoulder, Elden Ring's lock-on): low over the man's shoulder so the troll fills the frame, cut on the beats, a small shake on each heavy landing, never looking into the low sun.
- **No music.** The game's own sound bed of the shore (water, wind, birds) only.
- **Judged** at each round by one harsh adversarial review against the references and the brief, on a small sheet of the film's frames. Stopped at 7.5/10, or after two rounds without a gain.
