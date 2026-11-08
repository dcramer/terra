# terra_iso: the 2.5D top-down experiment

Began as a spike asking what a realistic 2.5D party game in the manner of Baldur's Gate II would look like on our engine and assets ([research/2026-11-05-isometric-party-spike.md](../../research/2026-11-05-isometric-party-spike.md)); now the iso experiment, its biomes data on a shared kit judged toward realism (D634), moved out of `spike/iso` on the developer's ask (2026-11-07). How it is to converge on the game's own systems: [docs/iso.md](../../docs/iso.md) (D635, Open).

A member of the root workspace, a front end on Bevy as `terra_sandbox` is (D28): the game's target, lock and Bevy patches, the repository's `assets/` beside its own (`assets/`, the `iso://` source). Its binary is `iso`.

## Run

From the repository root (the alias in `.cargo/config.toml`, a release build):

```bash
cargo iso                      # the party in the clearing
cargo iso --biome ruin         # a biome (ruin, forest, marsh, neon-street, wasteland)
```

Every flag below goes after `cargo iso`; `cargo test --release -p terra_iso` runs its tests, `node --test crates/terra_iso/loop.test.ts` its loop's, and `node crates/terra_iso/loop.ts` is the judging loop (the iso-art skill).

LMB on someone chooses them, LMB on the ground sends the chosen there in a wedge (they run when it is far), drag a box to choose several, RMB lets go, `1`-`4` / `=` choose, a portrait click chooses, Space pauses (red frame), `T` turns the hour (day, dusk, night), WASD / arrows / the screen's edge / the middle button pan, the wheel zooms.

A still with no window: `cargo iso --shot target/job/dusk.png --demo --hour dusk` (`--frames`, `--zoom`, `--focus x,z`, `--size WxH`).

The second scene, an action RPG's night fight in a ruined court (src/ruin/, experiment iso-art): `cargo iso --scene ruin` (fixed camera, clips playing), or a still with every body held at its clip's set time: `cargo iso --scene ruin --shot target/job/ruin.png`. From the repository root on the render worker: `cargo xtask remote-render --gpu --collect target/job/ruin.png -- cargo iso --scene ruin --shot target/job/ruin.png`.

A biome is a data file on the shared kit (src/kit/, D634, experiment iso-kit): `cargo iso --biome ruin` reads `biomes/ruin.ron` (its layout, its light preset from `biomes/light/`); `--scene ruin|forest|marsh` is the same.

Playing a biome (src/play.rs): `cargo iso --biome neon-street` (or `ruin`, `forest`, `marsh`) opens a window with the hero as the player. WASD runs him relative to the screen (W up it), Shift walks; a left click on the ground turns him to it and swings (moving held for the swing), and the creatures in its arc within 3 m fall; the camera follows him at its fixed angle. A `--shot`/`--film` run stays the staged fight unless `--play`, scripted by `--press wd@50-85` (keys held over those frames) and `--click 700,800@95` (a click at those pixels on that frame): `cargo iso --biome neon-street --play --film 100 --frames 40 --shot target/job/play/neon.jpg --press d@50-85 --click 700,800@95`.

## What it is

- One fixed orthographic camera, 42° down and turned 28°, never rotated, as the Infinity Engine's painted backgrounds are; drawn live in 3D instead (PBR, cascaded and point-light shadows, SSAO, TAA, bloom, distance haze). Day, dusk (the blue hour) and night (`T`); AgX by day, Tony McMapface at dusk and night, which keeps a fire orange.
- A clearing in an old wood (scene.rs, forest.rs, terrain.rs): 80 oaks framing a glade, crowns that sway and cut a dithered hole about the party, a pond with reeds, rocks, ferns and grass in drifts, the ground trodden bare about the camp and along its paths, shaded under each crown.
- The camp (camp.rs): a fire of stones and logs with alpha-blended flame tongues, coals, embers and smoke (motes.rs, smoke.rs); its light in two parts, a near one casting only the party's and the camp's big things' shadows (render layer `FIRE_CASTS`) and a far one casting none; two fallen trunks to sit on, bedrolls, a stump, a bucket, a wood heap, twigs and bark (Poly Haven scans through tools/blender/scan.py, two Tripo props).
- Life: deer, a boar and a fox grazing and wandering (wildlife.rs), fireflies at dusk and night.
- The party: a knight and three in the male body's ranger or peasant pieces, idle/walk/jog/sit clips; `--demo` makes camp, each walking to their place, three sitting by the fire, the knight on watch (rest.rs).
- BG-style furniture: green circles at the chosen's feet, live portraits rendered from each face, a log with their barks.
- How it was judged: each round a harsh judge scores the dusk still against Pillars of Eternity, Pathfinder WotR and BG3 frames, and a blind pairwise judge picks between it and the last kept still (rounds in ~/.cache/terra-review/iso-forest/).

Scratch output goes in `target/job/` (ignored).
