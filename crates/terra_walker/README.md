# Walker

The walker layer, in Bevy (D198, D407): a body this machine steers over the world, and the character it is, and nothing of what the player does there.

- **The walker**, [`WalkerPlugin`](src/walker.rs): the player's own components (`Player`, `Motion`, `Stance`, `View`, `Downed`, `Stamina`), where a player arrives (`ArriveAt`) and warps (`WarpTo`), and what the layers above say the hands are on (`Hands`); how it moves over the planet, its base and the sea (`movement`), bodies pushed out of each other and of solids (`bodies`), the camera and its shake (`camera`, `shake`), the keys read into what the player means (`controls`, `intent`), and the options menu (`options`, `button`).
- **The character** ([docs/characters.md](../../docs/characters.md)): the appearance chosen into a body's morphs and materials (`appearance`, `character`), its skinning (`skinning`, with `assets/shaders/skinning.wesl`), the arms lifted to what is held (`lift`), loose pieces swinging (`swing`), the animations' pace (`pace`), and the figure that plays them (`figure`).

It stands on the world (`terra_world`) and the base on screen (`terra_bases`, for the ground a walker stands on and the hammer it holds), and on the plain crates; nothing here names gameplay, and Cargo holds that. A run with the world and the walker alone is the sandbox's `walk`. Its tests need no GPU:

```bash
cargo test -p terra_walker
```

The game adds it as the second of `GamePlugins` and re-exports it at its old paths (`terra_game::play::walker`, `terra_game::appearance`, ...). The `testing` feature gives the crates above `model_files`, what the model files under `assets/` carry, and `Intent::hold`, for their tests.
