# Sandbox

A test environment for working on graphics and utilities. It opens a window on a stand-in planet with a free camera, lets you change values while it runs, and can switch on experiments.

It is not the game. Nothing here is gameplay, and the planet is a formula, not voxels.

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
| `--bench` | Measure frame times with vsync off, print them, then exit |
| `--press <key>@<frame>` | Press a letter key on a given frame, for scripted runs |

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

The ground's look is in `assets/shaders/ground.wgsl`. Save it while the sandbox is running and the ground updates. The F2 panel changes the colours and the sun without touching code, and F1 lets you edit any material.

To check a change without watching it, take a screenshot and read it:

```bash
cargo run -p terra_sandbox -- --shot shots/ground.png
```

Shader reloading works when run through `cargo run`. The Windows build launched by `cargo xtask win` runs from a copy, so edits to the original are not picked up there.

## The look experiment

```bash
cargo run -p terra_sandbox -- --experiment look
```

A test of the art direction in [docs/art.md](../../docs/art.md): faceted ground, generated boulders and shrubs, a warm sun with cool shadows, haze, a gradient sky with clouds, and bloom. The sky is `assets/shaders/sky.wgsl` and reloads when saved, like the ground.

## The forest experiment

```bash
cargo run -p terra_sandbox -- --experiment look --experiment forest
```

About a million generated pines over the whole planet. Left click or F chops the one you are pointing at; K chops every tree in view. None of the trees is stored: each cell's trees are computed from the seed when the camera comes near, and a chopped tree is one bit. Measurements are in [research/forest-scale-test.md](../../research/forest-scale-test.md).

## Adding an experiment

An experiment is a small plugin that nothing else depends on.

1. Add a module in `src/experiments/` with a `pub fn plugin(app: &mut App)`.
2. List it in `ALL` in `src/experiments/mod.rs`.

It can use the `Planet` resource to ask where the ground is, and anything in `terra_core` and `terra_gen`. `scatter.rs` is the example: it places rocks from a seed and lets you harvest them.

## Known limits

- The planet's mesh is coarse, about 12 m between vertices, while objects are placed on the exact ground. Small objects can sit a little above or below what is drawn.
- No atmosphere or sky by default; the background is flat. The `look` experiment adds a sky.
- No player, gravity or collision. The camera flies freely.
