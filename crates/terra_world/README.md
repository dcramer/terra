# World

The world on screen, in Bevy (D198): what every run stands on, and nothing of what a player does there.

- **The shell** every run starts in: the window and camera, the launch options, scripted runs and screenshots, the frame-time readout, the remote protocol, and sound (`shell`, `debug`, `remote`, `record`, `loading`, `audio`).
- **The planet**: its voxel terrain, sea, sky, and **the start**, the fixed place every east-and-south position is measured from (`planet`, `terrain`, `sky`, `start`).
- **The world layer of the game**, [`WorldPlugin`](src/world.rs): the light and air, the day, what grows and its data file, the tick and levels of detail (`scene`, `skylight`, `daylight`, `flora`, `flora_kinds`, `data`, `tick`, `lod`).

It builds, tests and runs without the game:

```bash
cargo test -p terra_world
cargo run -p terra_world --example world                                    # fly over the world
cargo run -p terra_world --example world -- --offscreen --time 13 --shot world.png
```

The game (`terra_game`) stands on it, and talks to it through small types it owns: a `Footprint` keeps what grows out of what is built, a `Viewpoint` is what detail is graded from, `FloraSystems` and the tick's `Sim` sets are what the game orders itself by, and flora's kinds name the items they give by name, for the game to make them items. Nothing here names anything of the game; Cargo enforces that, since the game depends on this crate.

The sandbox's `walk` experiment adds the game's walker to it, a body to walk through the world with: [docs/tech.md](../../docs/tech.md#layers-of-the-game-d198).
