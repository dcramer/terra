# Simulation toy

Prototype 2 in [docs/prototypes.md](../../docs/prototypes.md): the planet simulation on a bare globe, with no game around it. It exists to answer one question: is local-to-global terraforming legible and fun on its own?

Every rule and number in it is a first draft, meant to be changed after playing.

- The simulation is `crates/terra_sim` in the main workspace: pure Rust, no engine. `cargo test -p terra_sim` from the repo root runs its tests.
- `ext/` exposes it to Godot.
- `godot/` draws the globe and takes input.

## Build

```bash
./build.sh
```

Builds the library for the current platform, and for Windows too when the MinGW cross toolchain is installed.

## Play

From WSL, using the Windows engine build:

```bash
cd godot && ../../../spike/bin/godot.windows.editor.x86_64.exe --path "$(wslpath -w "$PWD")"
```

| Input | Action |
|---|---|
| Right-drag, wheel | Orbit, zoom |
| Left-click | Place the selected machine |
| 1, 2, 3 | Heater, gas generator, seeder |
| V | Next view: surface, temperature, moisture, life |
| Space, `-`, `=` | Pause, slower, faster |

## What the machines do

| Machine | Scope | Effect |
|---|---|---|
| Heater | Local | Warms the ground within about 300 m. Melts ice there. |
| Gas generator | Global | Thickens the air, which warms the whole planet. Air leaks away, so each generator holds up a fixed amount; how many you run sets the climate. Where you put it does not matter. |
| Seeder | Local | Plants life in its cell. Life survives only with air, warmth and moisture, then spreads to neighbouring cells by itself. |

Meltwater raises one global sea level. Moisture comes from open sea and is carried inland by air; plants hold it, so life spreading inland carries moisture with it.

## Scripted runs

Three strategies run for a fixed time, for comparing outcomes without a player. Headless prints a summary; with a display it also saves screenshots of every view to `godot/shots/`.

```bash
../../spike/bin/godot.linuxbsd.editor.x86_64 --headless --path godot -- --scenario=hot --ticks=8000
```

Scenarios: `hot` (five gas generators), `cool` (two), `cool_melt` (two, plus heaters ringing both poles). All place the same seeders.

Frame-time check, to be run on the integrated GPU as well (D25):

```bash
cd godot && ../../../spike/bin/godot.windows.editor.x86_64.exe --path "$(wslpath -w "$PWD")" --gpu-index 1 --disable-vsync -- --bench
```
