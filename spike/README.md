# Tech spike: Godot 4 + Voxel Tools

Throwaway. Answers the question in [docs/prototypes.md](../docs/prototypes.md); findings go in [docs/godot-spike.md](../docs/godot-spike.md).

`bin/` and `solar_system_demo/` are not committed. To recreate them:

```bash
gh release download v1.7 --repo Zylann/godot_voxel --dir bin \
  --pattern 'godot.linuxbsd.editor.x86_64.zip' --pattern 'godot.windows.editor.x86_64.exe.zip'
git clone https://github.com/Zylann/solar_system_demo.git
```

Unzip the editors, then apply [demo.patch](demo.patch) inside `solar_system_demo/`.

## Running

Headless check from WSL (imports the project and reports script errors):

```bash
bin/godot.linuxbsd.editor.x86_64 --headless --path solar_system_demo --import
```

On a real GPU from WSL, using the Windows build. The project must be on a Windows drive, because the demo's SQLite saves fail over the `\\wsl.localhost` path:

```bash
bin/godot.windows.editor.x86_64.exe --path 'C:\path\to\copy\of\demo' -- --autostart=60 --no-detail
```

`--autostart=<seconds>` starts a game, then prints FPS, saves `_spike_shot.png` and quits. `--no-detail` turns off detail rendering, which currently hangs or crashes (see tech.md).

## Tests

Both run headless from WSL or macOS and need only the editor binary. Set `GODOT` to its path if it is not the Linux one in `bin/`.

Question 1, co-op digging (host, digging client, late joiner):

```bash
planet_test/run_dig_sync.sh
```

Question 2, changing the terrain under a dug hole:

```bash
bin/godot.linuxbsd.editor.x86_64 --headless --path planet_test res://change_test.tscn
```

Question 3, rendering from orbit to ground with a dig. Needs a real GPU, so on Windows it uses the Windows build; screenshots land in `planet_test/shots/`:

```bash
cd planet_test && ../bin/godot.windows.editor.x86_64.exe --path "$(wslpath -w "$PWD")" res://view_test.tscn -- --radius=2000
```

The same check doubles as the performance test. `--gpu-index 1` selects the integrated GPU, which is the low-end check (D25):

```bash
cd planet_test && ../bin/godot.windows.editor.x86_64.exe --path "$(wslpath -w "$PWD")" --gpu-index 1 --disable-vsync --resolution 1920x1080 res://view_test.tscn -- --radius=2000
```

Simulation tick in GDScript, then in Rust inside the engine (build `sim_rust` first and copy the library to `planet_test/ext/`):

```bash
bin/godot.linuxbsd.editor.x86_64 --headless --path planet_test --script res://sim_bench.gd
```

```bash
cd sim_rust && cargo build --release && cp target/release/libterra_sim_bench.so ../planet_test/ext/
```

For the Windows engine, cross-build from WSL (needs `mingw-w64` and `rustup target add x86_64-pc-windows-gnu`):

```bash
cd sim_rust && cargo build --release --target x86_64-pc-windows-gnu && cp target/x86_64-pc-windows-gnu/release/terra_sim_bench.dll ../planet_test/ext/
```

```bash
bin/godot.linuxbsd.editor.x86_64 --headless --path planet_test --script res://sim_bench_rust.gd
```
