# Enshrouded's melee, measured: first round

2026-10-24. Question: what are Enshrouded's melee timings and camera framing, in numbers an agent can check ours against (D180, docs/movement.md "The goal")? The numbers are in [tools/combat/refs/enshrouded.json](../tools/combat/refs/enshrouded.json); this note says where they came from, how far they can be trusted, and where our sword-and-shield set sits outside them.

## Answer so far

Only part of the reference is measured; the rest is captured and waiting to be read (below).

- **The heavy blow** (Update 8's sword heavy, sword and round shield): 0.52 s from leaving idle to the hit, with the charge held as briefly as shown. It then spends 0.71 s bent over the blow, and is back on guard 1.08 s after the hit, about 1.6 s in all. The leap carries the body roughly half a body height forward.
- **An enemy's telegraphed overhead** (2023 build, parried on the shield): 0.8 s from the axe starting up to the hit.
- **The camera** frames the player at 0.20 to 0.27 of the frame's height, centred, with no shoulder offset in a fight. It sits a little above the head looking slightly down, and does not pull in for a fight. That is 3.2 to 4.8 body heights back for a vertical field of view of 55° to 60°. Enshrouded's field of view is a slider whose default is not published, so the distance is a range.
- **The guard**: the round shield on the left forearm across the chest and belly, the sword low in the right hand along the leg, point down.

## Sources

- **Keen's official channel**:
  - "Enshrouded - Forging the Path | Combat" ([RDQ0_R4AR-A](https://www.youtube.com/watch?v=RDQ0_R4AR-A), April 2026): the developers on Update 8's melee rework (charged heavies, i-frames on the roll, warping to targets), with gameplay inserts. This is the current melee and the main source.
  - "Enshrouded - Combat Gameplay" ([EuViyK4qLQg](https://www.youtube.com/watch?v=EuViyK4qLQg), June 2023): a sword-and-shield fight in a cave, block, parry, stagger and the merciless finisher. It shows the pre-release build, whose melee Update 8 reworked, so its timings are older than the game's.
- **The Steam store's two trailers** (1.0 launch, Wake of the Water, from the [appdetails API](https://store.steampowered.com/api/appdetails?appids=1203620)) are cinematic cuts, mostly magic, with no melee readable frame by frame. First-party footage elsewhere was enough, so no third-party gameplay was needed yet.

## Method

- Frames were caught in a headless Chromium playing the video slowed (0.25x to 0.5x), each with its media time from `requestVideoFrameCallback`, at 960 to 1280 px wide. They were laid out as labelled contact sheets and read by eye. A phase starts on the first frame where the pose leaves the previous one. The capture scripts are scratch, not committed; the pictures are in `~/.cache/terra-review/refs/enshrouded-combat/` (`ftp/`, `cg/`, `sheets/`).
- Times are to about ±0.03 s. At 0.5x the Update 8 video dropped about one frame in three, and the 2023 video repeats about one frame in ten. So a hit-stop of a few frames cannot be told from the capture: frame-to-frame differences show the repeats as false freezes.
- Distances are in body heights on screen. The camera's distance is d = 1 / (2 · share · tan(vfov / 2)) body heights, from the player's share of the frame height. The Update 8 video is 1920x900; if that is a cropped 1080-high frame, the share against the full frame is about a sixth smaller.

## Ours against it

Ours is `assets/data/moves/sword_and_shield.ron` in real seconds ((clip time − from) / speed), with the camera from `crates/terra_walker/src/camera.rs` and `controls.rs`.

| | Enshrouded | Ours | Where ours sits |
|---|---|---|---|
| Heavy, leaving idle to the hit | 0.52 s (shortest charge) | 0.61 s | inside, if a charge is held a little longer |
| Heavy, hit to back on guard | 0.71 s bent over, 1.08 s to guard | 1.18 s after the active span ends, 1.40 s after it starts | **outside**: about 0.3 s longer |
| Heavy, total | about 1.6 s | 2.02 s | **outside**: 0.4 s longer |
| Heavy lunge | about 0.4 to 0.8 body heights | drive about 1.1 m (0.6 body heights), plus the warp's step to 1.5 m off | inside, low confidence |
| Heavy hit-stop | a 2 to 4 frame flash; the freeze is not measurable | 0.2 s (12 frames at 60) | not known |
| Enemy telegraph (overhead) | 0.8 s | partner heavy 0.84 s, partner slash 0.93 s | inside |
| Player's share of frame height | 0.20 to 0.27 | about 0.38 (4.6 m back, 55° vertical) | **outside**: ours frames the player 1.4 to 1.9 times larger |
| Camera distance | 3.2 to 4.8 body heights (at 55° to 60°) | 2.6 body heights | **outside**: closer |
| Shoulder offset in a fight | none; player centred | 0.6 m right of the head | **outside** |

Not compared yet, since Enshrouded's side is not measured:
- light blow: ours 0.41 s to the hit, 1.04 s in all;
- chain cadence: ours 0.61 to 0.69 s hit to hit, 2.48 s for three;
- roll: ours 0.79 s, about 3.3 m (1.8 body heights), i-frames 0.05 to 0.50 s;
- block raise: ours 0.28 s.

## Not measured yet

Each is caught and waiting to be read, except where noted:
- **The light chain**: Update 8's sword blows on a dragon (RDQ0_R4AR-A 185 to 198 s), and the 2023 cave fight's blows (EuViyK4qLQg 27 to 51 s).
- **The roll** with Update 8's i-frames: 154 to 164 s and 171 to 184 s.
- **Spacing and the lunge**: before and after Update 8's warp, on dummies, 243 to 249 s.
- **Block raise**: neither video shows one in Update 8's build. The 2023 cave shows the block held, and a parry as a white burst on the shield.
- **Hit-stop**: needs a capture without drops (0.25x on the Update 8 video) or footage at a steady 60 fps.
- **Hit reactions**: the cave scavenger's stun (stagger rings, about 5 s, EuViyK4qLQg 36.5 to 41.5 s) is all that was seen; no flinch lengths.
