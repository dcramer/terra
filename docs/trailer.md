# Trailer

Status: the developers' brief, 2026-10-15: "by the end of the day we will want to create a cinematic trailer, we need 4 of those (3 biomes + village) to be stunning", then "the videos should all be simple, like just a dude running through a biome and its stunning, and thats it, no need to change its direction/focus on guy, its just about the ecosystem". How it is filmed is the agent's (D357).

## What it is

One run through each place: the game's own player (`--experiment play`) running a long, nearly straight route through its most beautiful stretch, and the game's own third-person camera following steadily behind. Nothing else moves the camera. The four runs, 25 seconds each at 1080p and 30 frames a second, are cut together with crossfades between a title and an end card, over a bed of the game's own sounds and music.

```bash
cargo xtask trailer                      # every run not yet filmed, then the cut
cargo xtask trailer --only lake --again  # film one run again
cargo xtask trailer --stills             # a first, middle and last frame of each, to judge a route fast
```

Each run is left as `~/.cache/terra-review/trailer/<name>.mp4`, with its own sound; the cut as `cut.mp4` and its contact sheet `cut-sheet.jpg`. The runs are `tools/sandbox/trailer/runs/<name>.json` (place, hour, date, route, sound) and the cut `tools/sandbox/trailer/cut.json`.

## The runs

| Run | Place | Route (metres east, south of the place's start) | Hour | Why |
|---|---|---|---|---|
| forest | the lush forest (`lush`) | down the forest's own path, on its laid points, from -154,-104 south-east through the pine stand toward the low sun to 37,48, 250 m | 07:30, soft morning | The mist is thickest at dawn and lies low between the trunks, and running toward the low sun its shafts come down through them to the runner; on the path nothing stands between the camera and him; the stand's tall pines over the runner are the forest's scale. |
| lake | the mountain lake (`lake`) | up the east shore from 216,22 to 445,-213, the shore walk `cargo xtask fields --site lake` gives | 18:20, evening, 10 June | "On the side of the lake and on the precipice of lake and forest with mountains in the back" (the developers): the forest's edge on one side, the water on the other, the snowy peaks ahead. |
| tundra | the tundra (`tundra`) | south across the open heath from 0,-10 toward the fells, 150 m | 16:20, low autumn sun, 8 October | The heath's red and gold in a low sun, the fells with the first snow in their lees ahead, and the wind in everything. |
| town | the market town (`town`) | down the paved main street from 60.5,-339.0 into the market square, along its south side past the well and round the stalls' west end and back east along its north side, clear of every stall, ending on the houses round it (`Town square` in its log says where they stand) | 10:30, market morning | The square at its busiest, the sun high enough to light down the narrow street without the early sun's orange, its people and animals about their day, the setts underfoot. |

## Scenes

A run may be a scene instead of a run (`"scene"` in its file): a timeline of the game's own remote calls on given frames, the camera's cuts and glides among them, in `tools/sandbox/trailer/scenes/<name>.ts`, written as a QA film's is (`qa/script.ts`) and filmed in one scripted run (`--calls`) with the same look, warm-up and sound bed, the player hidden where it starts. A scene says the frame its film rolls on; what comes before, its setting up, is not filmed.

```bash
cargo xtask trailer --cut tools/sandbox/trailer/dragon.json --out ~/.cache/terra-review/dragon/cut.mp4
```

The first is the dragon (`dragon`, D377, D382): the burg on a rise by the mountain lake in the morning; a dragon in over the forest strafing its south wall, passing over the hall and the keep and hovering at the keep to breathe into it; the fire spreading through the hoardings, roofs and timber storeys; the towers and the keep coming down as their bond timbers burn through.

## How it is filmed

As Unreal's Movie Render Queue renders a sequence: each run is a scripted run stepped a sixtieth of a second a frame, warmed up where it starts (six seconds, so streaming, grass and the eye settle), every second frame kept, so the film plays at the game's speed however slowly it is drawn.

- **The runner** runs empty-handed (the action bar's empty last slot taken in the warm-up): the held tools are untextured stand-ins still.
- **The route** is longer than the run: at the jog's 4.3 m/s a 25 s film with its lead-in needs 114 m. A player who reaches the last spot stands idle, which on film reads as a figure that is not animated; the tool says so before it films a run whose route is short.
- **The camera** is the game's: the boom behind and over the shoulder, nothing scripted. The route is smoothed into a spot every 4 m (centripetal Catmull-Rom), and the runner heads for the point of it 6 m ahead (pure pursuit, as games' path following steers), so the runner, and the camera with them, turn a little at a time, never swinging at a corner or at a spot.
- **The look** (`--cine`, D357): the exposure held from the moment the run starts, so it does not pump between shade and open; the clock held at the hour, so shadows do not step; motion blur at a film's 180° shutter; a 2.39:1 letterbox. Depth of field is there (`look.aperture`) but off: at the follow distance it blurs the land, and the land is the point.
- **The sound** is the game's own, made in code (`terra_sandbox --sound-bed`): each place's wind, birds, water, fire or the crowd's murmur, and a phrase of the game's music, mixed to a bed, since a scripted run is silent.

`--cine` also takes a keyframed camera path (dolly, crane, orbit, push-in, a tracking shot, lens and focus per key; Sequencer's and Cinemachine's terms), and cuts within one shot: a key with `"cut": true` starts a take of its own, the anti-aliasing's history dropped and the frame drawn without motion blur on the cut, as a renderer's camera cut does. The trailer's runs do not use it; the settlement's time-lapse does (`tools/sandbox/timelapse/`, D381).

## What it costs in real time

Offline rendering is not held to the frame budget. Of the look, motion blur is one full-screen pass reading the motion vectors TAA already draws, and the held exposure and clock cost nothing; depth of field, unused, is two more passes at full size.
