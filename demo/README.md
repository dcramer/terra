# Tech demo

Stand on a voxel planet, dig it, place terraforming machines, and watch the simulation change the sea, the ground and the sky around you. Fly up to see it from orbit. No loading screens.

This joins the pieces proven separately in `spike/` and `prototypes/sim_toy/`: Voxel Tools terrain, the Rust simulation, and a first-person player on a round world. It is a demo of the technology, not of the game: there is no survival, no co-op and no art yet.

## Run

Build the simulation library once, and again after changing the Rust code:

```bash
./build.sh
```

Then, from WSL, using the Windows engine build:

```bash
../spike/bin/godot.windows.editor.x86_64.exe --path "$(wslpath -w "$PWD")"
```

| Input | Action |
|---|---|
| W A S D, mouse | Move, look |
| Shift | Run, or fly fast |
| Space, Ctrl | Jump; in flight, up and down |
| F | Toggle flying |
| Left click, right click | Dig, fill |
| 1, 2, 3 then E | Pick heater, gas generator or seeder, then place it where you are looking |
| `-`, `=` | Simulation slower, faster (x0, x1, x5, x25, x100) |
| Esc | Release the mouse |

To see the whole arc quickly: place four or five gas generators and a few seeders, press `=` until the simulation is at x100, wait a minute or two, then press F and fly up. The simulation runs on its own thread, so speeding it up does not slow the frame rate.

## How it fits together

- The simulation owns the planet's state and its large-scale shape. `terrain.gd` builds the voxel terrain from the same shape, so the two agree on where the ground is to within about a metre.
- The ground's colour is the simulation's surface view, drawn onto a map of the whole planet and sampled by `ground.gdshader`. It refreshes four times a second.
- The sea is one sphere whose radius follows the simulation's sea level.
- The sky colour and haze follow air pressure and thin out with altitude.
- Digging edits the voxel terrain only. Terraforming never reshapes the ground.

## Scripted run

Checks walking, gravity and digging, then terraforms on a fast clock and saves screenshots to `shots/`. Works headless (no screenshots) for a quick test, or on a GPU:

```bash
../spike/bin/godot.windows.editor.x86_64.exe --path "$(wslpath -w "$PWD")" --gpu-index 1 --disable-vsync --resolution 1920x1080 -- --tour
```

`--gpu-index 1` selects the integrated GPU, the low-end check.
