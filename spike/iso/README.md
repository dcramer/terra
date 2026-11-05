# Isometric spike

**Question:** what would a realistic 2.5D party game in the manner of Baldur's Gate II look like on our engine and assets?

Not part of the game: its own Cargo workspace, reading the repository's `assets/` and carrying the same Bevy patches.

## Run

```bash
cd spike/iso && cargo run
```

LMB on someone chooses them, LMB on the ground sends the chosen there in a wedge (they run when it is far), drag a box to choose several, RMB lets go, `1`-`4` / `=` choose, a portrait click chooses, Space pauses (red frame), `T` turns the hour (day, dusk, night), WASD / arrows / the screen's edge / the middle button pan, the wheel zooms.

A still with no window: `cargo run -- --shot out/dusk.png --demo --hour dusk` (`--frames`, `--zoom`, `--focus x,z`, `--size WxH`).

## What it is

- One fixed orthographic camera, 42° down and turned 28°, never rotated, as the Infinity Engine's painted backgrounds are; drawn live in 3D instead (PBR, cascaded and soft sun shadows, point-light shadows from the fire and two lamps, SSAO, TAA, bloom, AgX).
- A village square: timber-framed houses built from boxes in the game's Poly Haven materials, its props, openings, trees, rocks and grass, mipmapped as `terra_world::mipmaps` does.
- The party: the male body dressed in its ranger or peasant pieces (bound to the body's joints by name), held things in the hand sockets, idle/walk/jog clips; folk and animals wandering; A* on a half-metre search map with the path pulled straight.
- BG-style furniture: green circles at the chosen's feet, a marker where they were sent, live portraits rendered from each face, a log with their barks.

Scratch output goes in `out/` (ignored).
