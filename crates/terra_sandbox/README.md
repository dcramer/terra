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
| `--settle <frames>` | Frames to run before capturing or measuring, 120 by default |
| `--press <key>@<frame>` | Press a letter or digit key on a given frame, for scripted runs. `<key>@<from>-<to>` holds it. |
| `--vantage <x,y,z,x,y,z>` | In the play experiment: a fixed camera, given as where it stands and what it looks at, in metres east, up and south of the base's corner |

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

The ground's look is in `assets/shaders/ground.wgsl`, the sea's in `water.wgsl`, the sky's in `sky.wgsl`, and the wind that moves plants in `wind.wgsl`. Save one while the sandbox is running and it updates. The F2 panel changes the colours and the sun without touching code, and F1 lets you edit any material.

To check a change without watching it, take a screenshot and read it:

```bash
cargo run -p terra_sandbox -- --shot shots/ground.png
```

Shader reloading works when run through `cargo run`. The Windows build launched by `cargo xtask win` runs from a copy, so edits to the original are not picked up there.

## The play experiment

```bash
cargo xtask win -p terra_sandbox -- --experiment play
```

The proof of concept: an outpost in a meadow by a lake, a little before sunset, and a player with a builder tool. You steer an animated figure in third person. It walks on the planet, climbs the steps to the base's door, is held up by floors, stopped by walls, windows, trees and boulders, and let through doors.

![The outpost](../../docs/images/outpost-2026-10-10.png)

| Input | Action |
|---|---|
| Click | Capture the mouse; Esc releases it |
| Mouse | Look |
| W A S D, Shift, Space | Move, sprint, jump |
| Hold right mouse | The dial of parts. Move the mouse toward one and let go. B does the same. |
| Tap right mouse | Put the part away, or put back what you picked up |
| Left click | Build, place, or pick up. E does the same. |
| Scroll wheel | Turn an item or a room; push and pull a wall. Z and X do the same. |
| R | The next variant of the part; with Shift, the one before |
| Q | Take-apart mode, and back |
| F | Move mode, and back |
| G | The grid on and off |
| Hold Alt | The opposite of the grid setting, while held |
| Middle click | Pick another of the item you are looking at. V does the same. |
| 1 to 9, 0 | The first ten parts directly |
| F3 | Show the frame rate |

The buttons follow Subnautica 2 where we could confirm what it does: right mouse is the build menu, left mouse builds, and taking apart and moving are modes on Q and F. The rest is the usual habit of building games. See [research/subnautica-2-building.md](../../research/subnautica-2-building.md).

Building starts from a part. Pick one from the dial, cycle through its variants, and aim. What would be built is shown before you commit: the outline of a room, the cut of a window, the wall a push would move, or a ghost of the item where it would stand. It is red where it cannot go.

![The dial of parts](../../docs/images/outpost-dial-2026-10-10.png)

| Part | Variants | What a left click does |
|---|---|---|
| Room | Four sizes; the wheel turns it | Builds a room on the ground you aim at. Aimed across a room already there, it extends that room to the combined shape. |
| Reshape | | Pushes the wall you aim at out. The wheel pushes and pulls. |
| Wall | Solid, doorway, window | Builds an interior wall on the nearest edge between two floor tiles. Indoors only. |
| Window | Framed, wide, tall | Cuts that window in the wall tile |
| Door | | Cuts a door |
| Remove (Q) | | Takes away the item or interior wall, or closes the window or door |
| Move (F) | | Picks an item up, then puts it down |
| Seat, Table, Bed, Storage, Kitchen, Decor, Kit, Fire | Twenty-seven items between them | Stands the item on the floor or ground you aim at |

Everything has a box. An item cannot be placed where its box would share space with another item, a wall, a tree or a boulder, and a walker cannot enter one. Rugs are the exception: they lie under anything. When something is in the way, the prompt says what, and an item that is in the way is outlined.

With the grid on, items land on half-metre steps and turn in eighths of a circle. The grid is drawn on the floor or ground round the item, with tile edges stronger than the steps between. With the grid off, items go exactly where you aim and turn in finer steps. Either way:

- Aim at a wall and the item stands flush against it.
- Aim at the side of another item and it stands flush beside that, lined up with it if nearly so.
- Aim at the top of another item and it stands on it.
- Aim at nothing in reach and it is offered a few steps in front of you.

Rooms are always on the base's three-metre tiles, so a new room lines up with the rooms already there. A room's foundation may go into the ground, but the room itself may not: where the ground is higher than its floor, the outline is red and it cannot be built.

![An interior wall with a doorway](../../docs/images/outpost-wall-2026-10-10.png)

The game holds the mouse while you play and lets go when the window is no longer in front; click to take it back.

Everything in and around the example base was placed through the same call the builder makes, and can be moved or taken away. The list of parts is in `catalogue.rs` and is a stand-in; nothing costs anything.

It is made of these pieces, each a module in `src/experiments/play/`:

| Module | What it is |
|---|---|
| `scene` | The sun, the sky's colours, haze, shadows, and the camera's effects |
| `flora` | Trees, rocks, bushes and flowers placed from the seed, and the grass |
| `catalogue` | The parts the builder can build, and their variants |
| `tool` | The builder: what it is aimed at, the preview, and building |
| `dial` | The dial of parts and the prompts |
| `props` | Items that have been placed, and stopping a walker at them |
| `figure` | The player's body and which animation it plays |
| `models` | Reading shapes out of model files |

The models are free ones, recoloured in code; see [THIRD_PARTY.md](../../THIRD_PARTY.md). How it was put together and what it costs to draw is in [research/outpost-scene.md](../../research/outpost-scene.md).

What is placeholder: the figure is a mannequin from an animation library; the planet is the stand-in one, so it cannot be dug and the base stands on a foundation clear of it; collision is arithmetic (circles round trees and boulders, boxes round items, a grid for walls), not a physics engine, and bushes, small rocks and the figure's own limbs have none; you cannot stand on an item, and an item stood on another stays in the air if the one under it goes; there are no stairs, so upper storeys cannot be reached, and items go on the ground floor only; grass is planted once around the base and runs out about 100 m away; and placed items are not saved or part of the base's list of operations.

In a scripted run the game clock advances a sixtieth of a second per frame and real keyboard and mouse input is ignored, so the same script does the same thing on any machine:

```bash
cargo run -p terra_sandbox -- --experiment play --settle 300 --press w@20-215 --press 0@240 --press e@260 --shot shots/a-shelf-indoors.png
cargo run -p terra_sandbox -- --experiment play --vantage 0,2.4,24,9,1.6,0 --shot shots/outpost.png
```

## The look experiment

```bash
cargo run -p terra_sandbox -- --experiment look
```

An early test of the art direction in [docs/art.md](../../docs/art.md): faceted ground, generated boulders and shrubs, a warm sun with cool shadows, haze, a gradient sky with clouds, and bloom. The play experiment has since gone further.

## The forest experiment

```bash
cargo run -p terra_sandbox -- --experiment look --experiment forest
```

About a million generated pines over the whole planet. Left click or F chops the one you are pointing at; K chops every tree in view. None of the trees is stored: each cell's trees are computed from the seed when the camera comes near, and a chopped tree is one bit. Measurements are in [research/forest-scale-test.md](../../research/forest-scale-test.md).

## The build experiment

```bash
cargo run -p terra_sandbox -- --experiment look --experiment build
```

A base you shape, inspired by Subnautica 2. It stands on the most level ground near the start, on a foundation, with steps up to its door. Aim with the mark in the middle of the view: G pushes the wall you are looking at out by a tile, H pulls it in, V cycles the tile through solid, three sizes of window and a door, and B adds a room on open ground. The rules live in `terra_build`; what it does and does not do is in [research/subnautica-2-building.md](../../research/subnautica-2-building.md).

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
