# Timber walls

Question: which way of drawing the timber kind procedurally from its cubes looks most beautiful and thematic, compared against today's Tripo logs? Findings: [research/timber-walls.md](../../research/timber-walls.md). Nothing depends on this spike; delete it once the developers have judged.

The same seven shapes of cubes (a straight wall, an L, a T, a wall with a door and a window, a wall part-built, timber meeting stone in an L and a T) are laid by the game's own layout (`terra_build::logs::lay`) and drawn four ways, side by side in one model per shape:

- **R**, the reference: the kit's Tripo logs (`art/props/kit-log`, `kit-log-2`, `kit-log-long`), placed and stretched as `terra_game`'s `build/pieces.rs` places them.
- **A**, round logs (`pieces.rs`): a tapered, bowed, uneven tube per log, displaced by Poly Haven's pine bark height map, sawn ends showing the `log_end` grain, daub between the courses.
- **B**, hewn timber (`pieces.rs`): a squared beam with rounded, wandering edges and adze scallops, in `rough_wood`, squared end grain, daub showing between the courses.
- **C**, timber as surface: the block kinds' surface (`terra_build::surface`) over the log cubes, in Poly Haven's `beam_wall_01`, a wall of logs, instead of stone.

Doors, windows and stone are the game's own in every one.

## Running it

The Tripo logs are ignored build output: write them with `node tools/props/props.ts refine kit-log` (and `kit-log-2`, `kit-log-long`) and copy `assets/models/kits/Log.glb`, `Log2.glb` and `LogLong.glb` to `assets/models/spike/`. Then:

```bash
cd spike/timber-walls && CARGO_TARGET_DIR=out/target cargo run --release
```

It writes `assets/models/spike/Timber-<shape>.glb` (ignored) and prints each way's triangles. Stand them on the yard beside the kit sheet:

```bash
node tools/sandbox/remote.ts start --windows -- --experiment play --level yard --kit-sheet --clean --time 15.5 \
  --model models/spike/Timber-L.glb    # and the others
node tools/sandbox/remote.ts look TW-A-L --distance 8.5 --around 215 --height 1.7 --aim 1
node tools/sandbox/remote.ts shot shots/a-l.png
```

Each way's shape is a node named `TW-<way>-<shape>`. The kit sheet's own rows can be moved out of the way with `place "<row name>, Built pieces" --translation 0,0,0`.
