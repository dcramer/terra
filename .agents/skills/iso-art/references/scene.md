# Making a good biome scene

A biome is a scene of the iso spike (`spike/iso/`, D133: throwaway, nothing lasting depends on it), chosen by `--scene NAME`. The ruin at night (`spike/iso/src/ruin/`) is the worked example: copy its split, not its numbers, for a new biome. Build it in steps, a still each, measured and landed on its own: camera and ground; walls and props; light; the fight; effects; ambient; the film.

## Modules (one each, numbers to tune at its top)

| Module | Holds |
|---|---|
| `mod.rs` | the scene's plugin, adding the rest |
| `camera.rs` | the ARPG view (below) |
| `ground.rs`, `blocks.rs` | the floor; the box-mapped block mesh builder |
| `walls.rs`, `beyond.rs`, `dress.rs`, `grass.rs` | standing structure; what fills to the frame's edges; props; ground cover |
| `light.rs`, `fire.rs` | key and fill; local sources with their flames |
| `fight.rs` | the cast posed at a clip and time |
| `effects/` | `kit.rs` (clock, camera, fires found, sparks, noise, glow materials), `flame.rs`, `spell.rs`, `hit.rs` |
| `ambient/` | `fog.rs`, `dust.rs`, `grass.rs` + `wind.wesl`, `banner.rs` |

## Camera

Perspective, FOV 30 vertical, pitch 55, yaw 20, about 19 m from a focus between the fighters at chest height (`camera.rs`), so a 1.85 m hero is 0.10 to 0.13 of the frame (the refs' 0.07 to 0.20, median 0.12; at 23.5 m the knight read 0.07 and too small). EV100 8, TAA with contrast-adaptive sharpening 0.6 (TAA's blur was the glade's biggest gap), SSAO, TonyMcMapface, bloom. Fixed in the spike: a still is judged from the game's pose. Which camera the game has is D629 (Open).

## Ground and walls from the scanned materials

Use the repository's Poly Haven scans in `assets/materials/` at their true size (each one's metres across is in assets/materials/README.md), box-mapped in world space (the dominant normal axis), never a texture stretched per block.
- Floor: one mesh of `large_grey_tiles` at its 3 m, cut along a noise contour into holes and a ragged edge over `packed_earth`, plus loose and displaced slabs where it breaks. Separate slabs of one size read as a checkerboard (judge r1); a hue tint per slab reads pink, green, grey; vary value only.
- Walls: one continuous skin of `castle_wall_varriation` at its 2 m, so courses run unbroken; the top broken to a jagged line and a ridge, faces a little proud or back, darker at the foot, mossed on top, fallen stones heaped below. Stacked boxes step like voxels.
- Fill the frame to its edges (`beyond.rs`): the refs have no margin of bare ground. Rocks from `assets/models/nature/`: the `Rock_Moss_*` set only in a night scene (the bare `Rock_1..10` render near white).
- Dress at the edges and about the fight: the iso props, the game's own (`assets/models/props/`), bones, debris, ground cover in drifts. A detail texture (D362, `assets/materials/detail_stone`) is the cheap way to the refs' sharpness: fine detail is the measure the ruin is furthest from (0.06 to 0.10 against 0.13 to 0.23).

## Light against the refs' numbers

Measure every still (judging.md) and tune to the refs of the same kind of frame. Night (vr-lamplit-street, poe2-fire-spiral, d4-dungeon-fight): luma p5 0.0002 to 0.0007, p50 0.0077 to 0.0145, p95 0.043 to 0.138, bright share 0.0003 to 0.015, saturation 0.42 to 0.75. Day (le-desert-fight, d4-fire-breath, poe2-shock-nova): p5 0.007 to 0.036, p50 0.09 to 0.15, p95 0.31 to 0.44.
- The sky fill sets p5: the ruin's is 1.5 nits (5 left the blacks 3 to 10 times too bright). A moon of 190 lux, bluish, low from the NW (`light.rs`).
- Pools set p95: one short-range shadowed point light a fire and no far fill light; the ruin's brazier 140k lm cut off at 8 m, torches 25k lm to 6 m (`fire.rs`). A wide wash of fill flattens the frame into one even light (judge r1).
- Stage the fight in the pools: the hero lit from before, the creature from behind, within about 2 m of a source.
- A brazier's bowl casting shadow blacks out its own pool: make it `NotShadowCaster`. Coals: a bed of lumps heaped to a yellow heart, dull red at the rim, over an additive glow.
- Rim: a light behind a figure spills everywhere (Bevy lights every mesh in reach) and barely reads from 55 degrees; use a fresnel emissive material (`spike/iso/src/rim.rs`'s pattern).

## The fight

`fight.rs` poses each of the cast at a clip and time: when the GLB's AnimationPlayer appears, a one-clip graph, `seek_to(at)`, speed 0 under `--shot` (real time would make a still random), playing in a film. Models face +Z: yaw = atan2(dx, dz). Choose the moment by a sheet of times (traps.md): the knight's chop reads at 1.3 s of his 2 s attack; 1.0 reads as arms raised. Hold the blow's moment in `hit.rs` to match.

## Effects (`effects/`, a fixed pool of entities, nothing spawned at run time)

- Fire: a 32-frame flipbook made at startup from rising noise (white-hot root, yellow, orange, ragged red edge) on 3 upright cards a fire with a white-hot core card; embers as velocity streaks; smoke puffs, unlit blend, warm at the bottom greying as they widen. At 70 px a metre make them bigger than feels right: flames 1.3x, embers 0.035 m, smoke alpha 0.6.
- Spell: a bolt is a core sphere, a glow card, a ribbon trail rebuilt each frame and its own light; the impact a swelling flash, sparks and a light dying in 0.09 s. Judge r3 wanted a hard white core, a rough trail, a pool of its colour on the ground.
- Hit: sparks, a flash card about 0.6 m, a light for 0.06 s, the creature's emissive (emissive texture = its albedo, strength under 1: a flat emissive over 1 turns it into a pink silhouette) white then red then off over 0.17 s, a flinch about its feet, and hit-stop: both clips at speed 0 for 80 ms.
- Every effect has a light, a bright core over bloom, and fades rather than pops.

## Ambient and living nature (`ambient/`)

- Fog: lit alpha-blended horizontal cards low over the ground with tattered noise, drifting on the scene's `WIND` and fading in and out, alpha at most about 0.34. It lifted the mids into lavender smudges at r3: keep it low and in the pools.
- Dust motes in each fire's light, fireflies blinking by the trees and grassy edges: soft additive dots facing the camera. Under 0.07 m they do not show at 960 px.
- Wind: a `StandardMaterial` extended with a vertex shader (and its prepass) bending blades by height in rolling gusts; cloth (a banner) on the same material, by how far down. Only a film shows it.
- Water: an untested trap so far (traps.md); start from black water with a narrow streak of the fire's reflection.

## Shooting and the film (on the render worker, embargoed machines build nothing)

- Still, from the repository root, `--dirty` for uncommitted code: `cargo xtask remote-render --gpu --dirty --minutes 40 --collect target/job/amb -- sh -c "mkdir -p target/job/amb; cargo test --release --manifest-path spike/iso/Cargo.toml > target/job/amb/test.txt 2>&1; cargo run --release --manifest-path spike/iso/Cargo.toml -- --scene ruin --shot target/job/amb/s.png > target/job/amb/run.txt 2>&1; true"`. Files land in `~/.cache/terra-review/remote/<job>/target/job/`; block with `cargo xtask remote-render --wait <job>`; grep `test result|panicked|ERROR` in its `job.log` and the run file.
- Film: the same with `--size 1280x720 --frames 300 --film 480 --shot target/job/film/f.jpg` (480 frames at a fixed 30 a game second from frame 300), then `ffmpeg -framerate 30 -i <remote>/target/job/film/f-%04d.jpg -c:v libx264 -crf 18 -pix_fmt yuv420p ~/.cache/terra-review/iso-art/film/NAME.mp4`. About 2 minutes with the build, 43 MB back. 12 to 20 s a biome for the review desk.
- Stills and films go to `~/.cache/terra-review/iso-art/<round>/`, each with its stats line, for the judge and the desk.
