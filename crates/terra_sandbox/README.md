# Sandbox

A test environment for the game. It opens a window on a stand-in planet with a free camera, lets you change values while it runs, and can switch on experiments: the game itself (`play`, from [terra_game](../terra_game/README.md)), the world and a body to walk it with and nothing else of the game (`walk`), testbeds that try one thing (`scatter`, `forest`, `look`, `build`, `lush`), and developer tools (`workbench`, `icons`, `crowd`).

The game's code is in `terra_game`; what is here is the launcher, the testbeds and the tools. The planet is a formula, not voxels.

## Run

```bash
cargo run -p terra_sandbox -- --experiment scatter
```

Under WSL that renders on the CPU, which is slow but correct. To use the real GPU:

```bash
cargo xtask win -p terra_sandbox -- --experiment scatter
```

| Option | What it does |
|---|---|
| `--experiment <name>` | Switch on an experiment. Repeatable. |
| `--list` | List the experiments |
| `--radius <metres>` | Planet radius, 2000 by default |
| `--shot <file.png>` | Save a screenshot once the scene has settled, then exit |
| `--rolling` | With `--level yard`, the ground rolls gently (swells of 6 m, bumps of 1.1 m) rather than flat, for the lush forest |
| `--shots <prefix>@<frames>` | Save a screenshot on each of those frames, counted as `--press` counts them, as `<prefix>-<frame>.png`, then exit: a list and ranges, like `out/walk@300-320,600`, so what pops in or out shows from one frame to the next |
| `--clean` | Leave the frame-time readout off the screen, for pictures |
| `--mute` | No sound. Scripted runs are always silent, and print what they would have played: `SANDBOX heard: step.grass 4` |
| `--sound` | Sound under WSL, where it is off by default because WSL usually has no sound device |
| `--bench` | Measure frame times with vsync off, print them, the median game, render-CPU and GPU parts of a frame, and when the slowest frames came, then exit. `cargo xtask bench` runs the benchmark scenes this way ([development.md](../../docs/development.md#measuring-performance)) |
| `--bench-frames <n>` | How many frames `--bench` times (600 by default) |
| `--offscreen` | In a scripted or background run: draw into a picture 1920x1080 instead of the window, which stays hidden. Works with the screen asleep or locked, covers nothing, and measures every machine at the same size; a benchmark waits on the GPU as a window would. Use it for screenshots and benchmarks on macOS, which draws nothing of a covered window |
| `--on-top` | In a scripted or background run: float the window above the others, still without taking focus, so it is drawn. Only needed to watch one; `--offscreen` needs no window |
| `--settle <frames>` | Frames to run before capturing or measuring, 120 by default, counted from when the loading screen goes. A script's `--press` and `--warp` frames are counted from then too. |
| `--loading` | Keep the loading screen up, to look at it (D135) |
| `--press <key>@<frame>` | Press a letter, digit, F1 to F12, `esc`, `enter`, `backspace`, `equal`, `minus`, `pageup`, `pagedown` or an arrow (`left`, `right`, `up`, `down`) on a given frame, for scripted runs. `<key>@<from>-<to>` holds it. The arrows look about, so a script can aim. |
| `--save <file>` | In the play experiment: keep the saved game in this file. Scripted runs save and load only when given one. |
| `--fresh` | In the play experiment: start over, throwing away the saved game |
| `--time <hour>` | In the play experiment: set the clock to an hour of the day, 0 to 24, such as 22 for night |
| `--at <east,south>` | In the play experiment: start the player so many metres east and south of the start, for testing. The log says where the deer, the spiders, the village and each person are. |
| `--face <east,south>` | In the play experiment: start facing that spot, for testing. |
| `--warp <east,south:east,south@frame>` | In the play experiment: on that frame, move the player to the first spot, facing the second, for testing. Repeatable. |
| `--route <east,south>` | Walk the player to this spot, then the next given, turning no faster than the arrow keys do, as a player would: a scripted walk for benchmarks and checks. Repeatable; once at the last, they stand. Any experiment with the walker (`walk`, `play`). |
| `--give <item>:<count>` | In the play experiment: carry something extra, for testing: a raw item such as `stone:10`, a tool such as `spear:1` (put on the bar), clothing such as `leather_coat:1` (worn), or `health`, `fed` or `stamina` at a value out of 100. Added to a loaded save too, so scripted runs can follow on from each other. Repeatable. |
| `--vantage <x,y,z,x,y,z>` | In the play and walk experiments: a fixed camera, given as where it stands and what it looks at, in metres east, up and south of the start |
| `--remote` | Let a script drive the run over Bevy's remote protocol, on `localhost:15702`: [tools/sandbox](../../tools/sandbox/README.md) |
| `--background` | Stay out of the way of whoever is at the machine, as scripted runs do: the window opens behind the others without taking focus, never takes the mouse, ignores the real keyboard and mouse, and is silent. `tools/sandbox` passes it by default |
| `--steady` | Advance the game a sixtieth of a second every frame, however fast frames are drawn, as scripted runs do. `tools/sandbox` passes it by default |
| `--model <path>` | In the workbench: a model to look at, a `.glb` or `.gltf` under `assets/`. Repeatable: models stand side by side doing the same thing |
| `--prop <name>` | In the workbench: an item of the build catalogue, such as `Forge`, drawn as it is when placed in play and stood in a row beside the models, its size printed (`workbench: prop …`). Repeatable; `all` for every item. With `--sides 0`, a row to judge proportions against a body |
| `--anim <clip>` | In the workbench: an animation to play. Repeatable; the first plays when looking by hand |
| `--hold <item>` | In the workbench: an item's held look in the right hand, as in play, named as in `--give`, such as `stone_axe` |
| `--wear <item>` | In the workbench: an item's look, on every model, named as in `--give`, such as `leather_coat`. Repeatable |
| `--template <name>[@<height>]` | In the workbench: the template the next model's body is shaped as (`models/character/templates.json`), such as `female` or `heavy`, and its height against the base's, such as `male@1.15`. Repeatable, in the order of the models |
| `--crowd <n>` | In the crowd experiment: how many dressed characters (100 by default) |
| `--linear-skinning` | Skin with Bevy's own linear blend instead of our dual quaternions, to compare the two |
| `--joints` | In the workbench: print where each model's joints and sockets stand once shaped (`JOINTS` lines), to compare with the export's `SOCKETS` (`tools/characters/sockets.py`) |
| `--attach <file>[@<socket>]` | In the workbench: any look, on a socket such as `models/x.glb@socket.back`, or without one as its file says. Repeatable |
| `--focus <name>` | In the workbench: look closely at one part, such as `socket.hand.R` |
| `--sheet <prefix>` | In the workbench: one picture per `--anim`, `<prefix>-<clip>.png`, then exit: rows are the sides, columns the moments. In `icons`: also save the baked icons laid out, `<prefix>.png` and `<prefix>-half.png` |
| `--sides <degrees,…>` | In the workbench sheet: where to look from, round from the front; 30,120,210,300 by default |
| `--moments <fraction,…>` | In the workbench sheet: how far through each clip; 0.4 by default |

## Controls

| Input | Action |
|---|---|
| Hold right mouse button, or press M | Look around |
| W A S D | Move |
| Q, E | Down, up |
| Left shift | Move fast |
| Scroll wheel | Change speed |
| F1 | Inspector: every entity, resource and asset, editable live |
| F2 | Tweaks: sun, sea level, ground colours |

The camera is Bevy's free camera, which treats +Y as up. The start is at the planet's pole, where that matches the ground; fly far around the planet and the horizon tilts.

## Working on graphics

The ground's look is in `assets/shaders/ground.wesl`, the sea's in `water.wesl`, the sky's in `sky.wesl`, and the wind that moves plants in `wind.wesl`. Save one while the sandbox is running and it updates. The F2 panel changes the colours and the sun without touching code, and F1 lets you edit any material.

To check a change without watching it, take a screenshot and read it:

```bash
cargo run -p terra_sandbox -- --shot shots/ground.png
```

Shader reloading works when run through `cargo run`. The Windows build launched by `cargo xtask win` runs from a copy, so edits to the original are not picked up there.

## The play and build experiments

The game itself, `play`, and the base you shape, `build`, live in [terra_game](../terra_game/README.md), which says how they work. The sandbox launches them:

```bash
cargo xtask play
cargo run -p terra_sandbox -- --experiment look --experiment build
```

## The look experiment

```bash
cargo run -p terra_sandbox -- --experiment look
```

An early test of the art direction in [docs/art-style.md](../../docs/art-style.md): faceted ground, generated boulders and shrubs, a warm sun with cool shadows, haze, a gradient sky with clouds, and bloom. The play experiment has since gone further.

## The forest experiment

```bash
cargo run -p terra_sandbox -- --experiment look --experiment forest
```

About a million generated pines over the whole planet. Left click or F chops the one you are pointing at; K chops every tree in view. None of the trees is stored: each cell's trees are computed from the seed when the camera comes near, and a chopped tree is one bit. Measurements are in [research/forest-scale-test.md](../../research/forest-scale-test.md).

## The crowd

What many characters cost: `--crowd <n>` dressed bodies in a grid, every template in turn, walking, jogging and idle, each from its own point in its clip. Once all are dressed it prints what they add up to: looks worn, skinned meshes, joint matrices sent each frame and entities. With `--bench`, the frame times and their parts. Measure on the real GPU, and on the integrated one too, as Bevy picks it from `WGPU_POWER_PREF` (passed to Windows by `WSLENV`):

```bash
cargo xtask win -p terra_sandbox --release --copy crowd -- --experiment crowd --crowd 500 --bench
WGPU_POWER_PREF=low WSLENV=WGPU_POWER_PREF cargo xtask win -p terra_sandbox --release --copy crowd -- --experiment crowd --crowd 100 --bench
```

## The workbench

```bash
cargo xtask win -p terra_sandbox --copy wb -- --experiment workbench --model models/character/mannequin.glb --model models/character/base.glb --anim Jog_Fwd_Loop --sides 30,100 --moments 0.1,0.35,0.6,0.85 --sheet shots/wb/compare --clean --settle 60
```

Models on a plain floor in the game's own light and renderer, playing animations (D71), holding and wearing things as they would in play (`--hold`, `--wear`, `--attach`, through the shared `character` module). It prints each model's clips, joints and sockets. Fly round by hand, drive it live with `--remote` ([tools/sandbox](../../tools/sandbox/README.md), which adds `pose CLIP FRACTION`), or give `--sheet` to save one contact sheet per clip and exit, which is how an agent checks a model. The command above puts the mannequin and the base body side by side through the jog, from two sides at four moments. Run sheets on the Windows GPU; the CPU renderer under WSL is far too slow. How characters are made and checked before they get here is in `.agents/skills/character/SKILL.md`.

## Item icons

```bash
cargo xtask icons --sheet shots/icons/all
```

The icon baker (D129): renders every item in the catalogue from its own look in the house style (D130) with the icon renderer (`terra_game`'s `icons.rs`), writes `assets/icons/<item>.png` sized to the item's cells and `assets/icons/manifest.txt`, lists items with no look yet, and exits; `--sheet` also saves the icons laid out on empty slots, at 96 and 48 pixels a cell. Icons painted by `tools/icons/paint.ts` are kept, never replaced by renders; `--renders <folder>` writes every render there too, for the painter. The play experiment loads the files. A test fails when an icon is stale. How to make, check and fix icons is in `.agents/skills/item-icons/SKILL.md`.

## Adding an experiment

An experiment is a small plugin that nothing else depends on.

1. Add a module in `src/experiments/` with a `pub fn plugin(app: &mut App)`.
2. List it in `ALL` in `src/experiments/mod.rs`.

It can use the `Planet` resource to ask where the ground is, and anything in `terra_core` and `terra_gen`. `scatter.rs` is the example: it places rocks from a seed and lets you harvest them.

## Known limits

- The planet's mesh is fine, about a metre between vertices, only within 250 m of the start. Beyond that it is coarse, about 16 m, and cuts across hollows and humps by metres. Ask the planet for `drawn` rather than `surface` to put something on the ground as it is seen out there.
- No sky by default; the background is flat. The `look` and `play` experiments add one.
- Outside the `play` experiment there is no player, gravity or collision, and the camera flies freely.
- The play experiment is not tuned for weak graphics hardware: every tree is drawn at full detail at any distance.
