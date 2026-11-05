# Isometric spike

**Question:** what would a realistic 2.5D party game in the manner of Baldur's Gate II look like on our engine and assets?

Findings: [research/2026-11-05-isometric-party-spike.md](../../research/2026-11-05-isometric-party-spike.md).

Not part of the game: its own Cargo workspace, reading the repository's `assets/` and carrying the same Bevy patches.

## Run

```bash
cd spike/iso && cargo run
```

LMB on someone chooses them, LMB on the ground sends the chosen there in a wedge (they run when it is far), drag a box to choose several, RMB lets go, `1`-`4` / `=` choose, a portrait click chooses, Space pauses (red frame), `T` turns the hour (day, dusk, night), WASD / arrows / the screen's edge / the middle button pan, the wheel zooms.

A still with no window: `cargo run -- --shot out/dusk.png --demo --hour dusk` (`--frames`, `--zoom`, `--focus x,z`, `--size WxH`).

## What it is

- One fixed orthographic camera, 42° down and turned 28°, never rotated, as the Infinity Engine's painted backgrounds are; drawn live in 3D instead (PBR, cascaded and point-light shadows, SSAO, TAA, bloom, distance haze). Day, dusk (the blue hour) and night (`T`); AgX by day, Tony McMapface at dusk and night, which keeps a fire orange.
- A clearing in an old wood (scene.rs, forest.rs, terrain.rs): 80 oaks framing a glade, crowns that sway and cut a dithered hole about the party, a pond with reeds, rocks, ferns and grass in drifts, the ground trodden bare about the camp and along its paths, shaded under each crown.
- The camp (camp.rs): a fire of stones and logs with alpha-blended flame tongues, coals, embers and smoke (motes.rs, smoke.rs); its light in two parts, a near one casting only the party's and the camp's big things' shadows (render layer `FIRE_CASTS`) and a far one casting none; two fallen trunks to sit on, bedrolls, a stump, a bucket, a wood heap, twigs and bark (Poly Haven scans through tools/blender/scan.py, two Tripo props).
- Life: deer, a boar and a fox grazing and wandering (wildlife.rs), fireflies at dusk and night.
- The party: a knight and three in the male body's ranger or peasant pieces, idle/walk/jog/sit clips; `--demo` makes camp, each walking to their place, three sitting by the fire, the knight on watch (rest.rs).
- BG-style furniture: green circles at the chosen's feet, live portraits rendered from each face, a log with their barks.
- How it was judged: each round a harsh judge scores the dusk still against Pillars of Eternity, Pathfinder WotR and BG3 frames, and a blind pairwise judge picks between it and the last kept still (rounds in ~/.cache/terra-review/iso-forest/).

Scratch output goes in `out/` (ignored).
