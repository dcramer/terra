# Enshrouded's melee, measured

2026-10-24, two rounds. Question: what are Enshrouded's melee timings and camera framing, in numbers an agent can check ours against (D180, docs/movement.md "The goal")? The numbers are in [tools/combat/refs/enshrouded.json](../tools/combat/refs/enshrouded.json); this note says where they came from, how far they can be trusted, and where our sword-and-shield set sits outside them.

## Answer

- **The light chain** (Update 8, sword and round shield, on a training dummy): 0.24 s from leaving idle to the blade's trail, 0.29 s to mid-sweep. Each blow's follow-through stops about 0.2 s after the hit. The chain runs at least four blows, 0.73 to 0.92 s hit to hit as the player paced it. The fourth steps in, lands with a flash, bends over, and is back on guard 1.05 s after the hit.
- **The heavy blow**: 0.52 s from leaving idle to the hit, with the charge held as briefly as shown. It then spends 0.71 s bent over the blow, and is back on guard 1.08 s after the hit, about 1.6 s in all. The leap carries the body roughly half a body height forward.
- **The roll**: 0.50 to 0.53 s from leaving the stand to standing again. That is a dive of about 0.2 s, a tucked ball of 0.1 s, and a kneel and rise of 0.2 s. Rolls chain every 0.60 to 0.64 s. Keen say the roll has i-frames, but nothing on screen marks them.
- **Hit-stop**: none. On the one landed blow caught at every frame, the body moves on every frame through the contact. A 3 to 4 frame flash and a kick of the camera mark the hit instead.
- **Spacing**: fighters trading blows stand 0.5 to 1.0 body heights apart, centre to centre (low confidence).
- **An enemy's telegraphed overhead** (2023 build, parried on the shield): 0.8 s from the axe starting up to the hit.
- **The camera** frames the player at 0.20 to 0.27 of the frame's height, centred, with no shoulder offset in a fight. It sits a little above the head looking slightly down, and does not pull in for a fight. That is 3.2 to 4.8 body heights back for a vertical field of view of 55° to 60°. Enshrouded's field of view is a slider whose default is not published, so the distance is a range.
- **The guard**: the round shield on the left forearm across the chest and belly, the sword low in the right hand along the leg, point down.

## Sources

- **Keen's official channel**:
  - "Enshrouded - Forging the Path | Combat" ([RDQ0_R4AR-A](https://www.youtube.com/watch?v=RDQ0_R4AR-A), April 2026): the developers on Update 8's melee rework (charged heavies, i-frames on the roll, warping to targets), with gameplay inserts. This is the current melee and the main source. The light chain is the "After Update 8" half of the warp comparison (244.7 to 248.8 s). The rolls are at 155 to 158 s. The spacing comes from the same fight at 176 to 178 s.
  - "Enshrouded - Combat Gameplay" ([EuViyK4qLQg](https://www.youtube.com/watch?v=EuViyK4qLQg), June 2023): a sword-and-shield fight in a cave, with block, parry, stagger and the merciless finisher. It shows the pre-release build, whose melee Update 8 reworked, so its timings are older than the game's.
- **Not used**:
  - The Steam store's two trailers (1.0 launch, Wake of the Water, from the [appdetails API](https://store.steampowered.com/api/appdetails?appids=1203620)) are cinematic cuts with no readable melee.
  - RDQ0_R4AR-A 185 to 198 s is upward slashes at flying enemies. The 211 to 221 s and 259 to 283 s inserts are a mage and an archer.

## Method

- Frames were caught in a headless Chromium playing the video slowed, each with its media time from `requestVideoFrameCallback`. They were laid out as labelled contact sheets of the player, cropped, and read by eye. A phase starts on the first frame where the pose leaves the previous one. Frame-to-frame difference in a crop (0 to 255 a pixel) showed where motion starts and stops, and whether a frame is held. The capture scripts are scratch, not committed. The pictures and scripts are in `~/.cache/terra-review/refs/enshrouded-combat/`.
- Times are to about ±0.03 s.
- Frame drops differed by capture:
  - The light chain and the landed hit were caught with 339 of 348 gaps one frame, and every frame from 247.60 to 247.95 s.
  - The heavy's capture dropped about one frame in three.
  - The roll's capture dropped one gap in fourteen.
  - The 2023 video repeats about one frame in ten.
  - The source itself repeats an odd frame (difference below 0.1, alternating with motion, at 245.32 and 248.07 s). Only a run of held frames would count as hit-stop.
- Distances are in body heights on screen. Spacing is the horizontal gap between two bodies' middles over the player's standing height, where both stand at about the same depth. The camera's distance is d = 1 / (2 · share · tan(vfov / 2)) body heights, from the player's share of the frame height.

## Ours against it

Ours is `assets/data/moves/sword_and_shield.ron` in real seconds ((clip time − from) / speed), with the camera from `crates/terra_walker/src/camera.rs` and `controls.rs`.

| | Enshrouded | Ours | Where ours sits |
|---|---|---|---|
| Light, leaving idle to the hit | 0.24 s to the trail, 0.29 s mid-sweep | 0.41 s to the active span | **outside**: about 0.15 s slower |
| Light chain, hit to hit | 0.73 to 0.92 s, player-paced (an upper bound) | 0.61 to 0.69 s | inside |
| Light chain, length | at least 4 blows | 3 (slash, backhand, finisher) | one short, low confidence |
| Chain's last blow, hit to back on guard | 1.05 s, bent over | finisher 0.78 s | just short of the target (0.8 to 1.1 s), and without the bent-over landing |
| Heavy, leaving idle to the hit | 0.52 s (shortest charge) | 0.61 s | inside, if a charge is held a little longer |
| Heavy, hit to back on guard | 0.71 s bent over, 1.08 s to guard | 1.18 s after the active span ends, 1.40 s after it starts | **outside**: about 0.3 s longer |
| Heavy, total | about 1.6 s | 2.02 s | **outside**: 0.4 s longer |
| Heavy lunge | about 0.4 to 0.8 body heights | drive about 1.1 m (0.6 body heights), plus the warp's step to 1.5 m off | inside, low confidence |
| Roll, duration | 0.50 to 0.53 s | 0.79 s | **outside**: 0.25 s longer |
| Roll to roll | 0.60 to 0.64 s | 0.66 s (cancel at 1.0) | inside, just |
| Roll distance and i-frames | not measurable from these videos | about 3.3 m (1.8 body heights); i-frames 0.05 to 0.50 s | not known |
| Hit-stop on a landed blow | none; a 3 to 4 frame flash and a camera kick | light 0.10 s, finisher 0.13 s, heavy 0.20 s (6, 8 and 12 frames) | **outside**, medium confidence (one hit, on a dummy) |
| Spacing while trading | 0.5 to 1.0 body heights between centres | warp stops at 1.45 m (0.8 body heights) | inside, low confidence |
| Enemy telegraph (overhead) | 0.8 s | partner heavy 0.84 s, partner slash 0.93 s | inside |
| Player's share of frame height | 0.20 to 0.27 | about 0.38 (4.6 m back, 55° vertical) | **outside**: ours frames the player 1.4 to 1.9 times larger |
| Camera distance | 3.2 to 4.8 body heights (at 55° to 60°) | 2.6 body heights | **outside**: closer |
| Shoulder offset in a fight | none; player centred | 0.6 m right of the head | **outside** |
| Block raise | none in Update 8's official footage | 0.28 s | not known |

## Not measured

- **Block raise**: no block in Update 8's official footage. The 2023 cave shows the block held, and a parry as a white burst on the shield.
- **Roll distance and i-frames**: every roll shown is under a camera that tracks the player. The only ruler is the enemy, at another depth. Nothing on screen draws the i-frames. A fixed-camera roll in third-party footage would give the distance; the i-frames cannot come from video.
- **A light blow's own recovery to guard**: every light blow shown is followed by the next.
- **The chain's shortest cadence**: the chain shown is paced by the player, not mashed.
- **Hit-stop on a living enemy, or on a heavy**: the only landed blow caught without drops is on a dummy.
- **Hit reactions**: the cave scavenger's stun (stagger rings, about 5 s, EuViyK4qLQg 36.5 to 41.5 s) is all that was seen; no flinch lengths.
- **Not looked at**: Keen's highlights video (ToMwQsJTaEM), and RDQ0_R4AR-A 93 to 104 s (a camp fight with weapon skills).

## Targets for the rebuilt set

Real seconds, for the sword-and-shield set and its camera (`targets` in the JSON):
- **Light blow**: 0.22 to 0.32 s from guard to the hit.
- **Light chain**: 0.55 to 0.75 s hit to hit; 3 to 4 blows.
- **The chain's last blow**: back on guard 0.8 to 1.1 s after its hit.
- **Heavy**: 0.5 to 0.65 s to the hit at its shortest charge; back on guard 0.7 to 1.1 s after the hit; 1.4 to 1.7 s in all.
- **Roll**: 0.5 to 0.6 s; chained every 0.60 to 0.66 s.
- **Hit-stop**: 0 to 0.05 s (0 to 3 frames). A landed blow reads by a flash of 3 to 4 frames and a camera kick instead.
- **Spacing while trading**: 0.5 to 1.0 body heights between centres (0.9 to 1.8 m).
- **An enemy's telegraph**: at least 0.8 s.
- **Camera**:
  - the player at 0.20 to 0.27 of the frame's height;
  - 3.2 to 4.8 body heights back;
  - centred, with no shoulder offset in a fight.
