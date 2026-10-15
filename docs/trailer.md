# Trailer

Status: the developers' brief, 2026-10-15: "by the end of the day we will want to create a cinematic trailer, we need 4 of those (3 biomes + village) to be stunning", then "the videos should all be simple, like just a dude running through a biome and its stunning, and thats it, no need to change its direction/focus on guy, its just about the ecosystem". How it is filmed is the agent's (D357).

## What it is

One run through each place: the game's own player (`--experiment play`) running a long, nearly straight route through its most beautiful stretch, and the game's own third-person camera following steadily behind. Nothing else moves the camera. The four runs, 25 seconds each at 1080p and 30 frames a second, are cut together with crossfades between a title and an end card, over a bed of the game's own sounds and music.

```bash
cargo xtask trailer                      # every run not yet filmed, then the cut
cargo xtask trailer --only lake --again  # film one run again
cargo xtask trailer --stills             # a first, middle and last frame of each, to judge a route fast
```

Each run is left as `~/.cache/terra-review/trailer/<name>.mp4`, with its own sound; the cut as `cut.mp4` and its contact sheet `cut-sheet.jpg`. The runs are `tools/sandbox/trailer/shots/<name>.json` (place, hour, date, route, sound) and the cut `tools/sandbox/trailer/cut.json`.

## The runs

| Run | Place | Route (metres east, south of the place's start) | Hour | Why |
|---|---|---|---|---|
| forest | the lush forest (`lush`) | from the floor's view at -58,-39 north and round east through the pine stand to 77,-22, the walk bench's route | 07:30, soft morning | The mist is thickest at dawn and lies low between the trunks; the stand's tall pines over the runner are the forest's scale. |
| lake | the mountain lake (`lake`) | up the east shore from 216,22 to 445,-213, the shore walk `cargo xtask fields --site lake` gives | 18:20, evening, 10 June | "On the side of the lake and on the precipice of lake and forest with mountains in the back" (the developers): the forest's edge on one side, the water on the other, the snowy peaks ahead. |
| tundra | the tundra (`tundra`) | south across the open heath from 0,-10 toward the fells, 150 m | 16:20, low autumn sun, 8 October | The heath's red and gold in a low sun, the fells with the first snow in their lees ahead, and the wind in everything. |
| town | the market town (`town`) | down the paved main street from 319.9,-278.6 into the market square, the town's own arrival route | 08:30, market morning | The square at its busiest, its people and animals about their day, the setts underfoot. |

## How it is filmed

As Unreal's Movie Render Queue renders a sequence: each run is a scripted run stepped a sixtieth of a second a frame, warmed up where it starts (six seconds, so streaming, grass and the eye settle), every second frame kept, so the film plays at the game's speed however slowly it is drawn.

- **The camera** is the game's: the boom behind and over the shoulder, nothing scripted. The route is smoothed into a spot every 4 m (centripetal Catmull-Rom), so the runner, and the camera with them, turn a little at a time, never swinging at a corner.
- **The look** (`--cine`, D357): the exposure held from the moment the run starts, so it does not pump between shade and open; the clock held at the hour, so shadows do not step; motion blur at a film's 180° shutter; a 2.39:1 letterbox. Depth of field is there (`look.aperture`) but off: at the follow distance it blurs the land, and the land is the point.
- **The sound** is the game's own, made in code (`terra_sandbox --sound-bed`): each place's wind, birds, water, fire or the crowd's murmur, and a phrase of the game's music, mixed to a bed, since a scripted run is silent.

`--cine` also takes a keyframed camera path (dolly, crane, orbit, push-in, a tracking shot, lens and focus per key; Sequencer's and Cinemachine's terms), built before the brief was simplified and tested without the GPU only. The trailer does not use it.

## What it costs in real time

Offline rendering is not held to the frame budget. Of the look, motion blur is one full-screen pass reading the motion vectors TAA already draws, and the held exposure and clock cost nothing; depth of field, unused, is two more passes at full size.
