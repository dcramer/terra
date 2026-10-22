# Bases

The base on screen, in Bevy (D407): a base built of the rules' cubes (`terra_build`), placed on the world and drawn, and nothing of who builds it or why.

- **The base**, [`plugin`](src/lib.rs): its site on the planet, set on the ground (`footing`), its kits read and read again when a file changes, its acts worked out on a worker and applied as one (`Build`, `Applied`, `Fallen`), the footprint flora keeps out of, the build keys, and what the layers above read of a player's hammer (`hammer`).
- **Its drawing** (docs/building-plan.md, drawing; D211): its sections laid and merged on building's own threads (`layout`), as entities (`store`), put on screen a few a frame at each detail (`draw`), far off as merged cells (`hlod`); its materials as texture arrays and atlases, block compressed (`arrays`, `atlases`, D290) and the maps its pieces are made with (`maps`); its rooms' light (`light`); the debug views, F4 (`views`, D385); and what it holds in memory (`memory`, `build.memory` over the remote).

It stands on the world (`terra_world`) and the plain building crates (`terra_build`, `terra_build_layout`, `terra_build_mesh`); nothing here names the game, the town or anything above, and Cargo holds that. Its tests need no GPU:

```bash
cargo test -p terra_bases
```

Building play (`terra_building::building`) adds it with what is built over it, and the game re-exports it at its old path, `terra_game::build`. The walker reads the hammer (`hammer::Hammer`); building play acts on `Build` and reads `BuildLook` and `Site`. How it is drawn: [docs/building-drawing.md](../../docs/building-drawing.md).
