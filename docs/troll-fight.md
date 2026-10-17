# The troll fight: a demo scene

Status: the developers' brief, 2026-10-17: "for lake one, to make things more interesting - have an agent generate like a really high fidelity realistic troll (largeish one) and have our guy fight it for some time. It will be a bigger task because it needs to do some modeling, then have it be animated with like some attack and movement, and our guy has to have a sword/shield and move sideways and have attack animations and it needs to be engaging." and "one off clips don't have to have music on". A demo for the trailer (docs/trailer.md, Scenes), not a combat system: how the game fights is [combat.md](combat.md), still to be built on, and nothing here decides it.

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

About 26 s; every blow has a wind-up the man reads, and every landing a reaction:

1. The troll comes out of the trees and roars; the man raises his shield (0-5 s).
2. They circle: the man strafes, the troll turns to keep him in front (5-8 s).
3. Overhead smash: a long wind-up, the man dodges aside, the blow lands in the gravel in a burst of dust and stones (8-11 s).
4. The man steps in and cuts twice at its leg; it staggers (11-13 s).
5. The sweep: the man takes it on his shield and is driven back a stride (13-16 s).
6. A second smash at the water's edge: he sidesteps, the blow lands in the shallows in a splash (16-19 s).
7. His combination of three; the troll reels back into the shallows and roars (19-26 s).

## How it is filmed

- **Set pieces told over the remote** (`play::staged`): a staged figure is spawned from a model, stands on the ground, plays a clip it is told, turns to face a point and moves to a point at a pace, its clip's feet matched to the pace. What its blows raise is told too: a burst of dust and stones on ground, a splash and rings on water. A scene drives them as `play::fliers` flies the dragon; when creatures think, their minds will tell the same things.
- **The camera** is close and moving but readable, as third-person action games frame a big enemy (God of War's over-the-shoulder, Elden Ring's lock-on): low over the man's shoulder so the troll fills the frame, cut on the beats, a small shake on each heavy landing, never looking into the low sun.
- **No music.** The game's own sound bed of the shore (water, wind, birds) only.
- **Judged** at each round by one harsh adversarial review against the references and the brief, on a small sheet of the film's frames. Stopped at 7.5/10, or after two rounds without a gain.
