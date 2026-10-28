# Blender

Runs a Python script inside Blender, headless, so model work is a repeatable command with nothing open. Agents can also work in a Blender someone has open, through its MCP server ([docs/development.md](../../docs/development.md#blender), and the `blender` skill).

```bash
tools/blender/run.sh path/to/script.py [args...]
tools/blender/run.sh path/to/script.py --open scene.blend [args...]
```

- **Which Blender:** the version in [version](version), pinned so every machine makes the same files; `run.sh` refuses another (exit 3). `BLENDER` if it is set, else under WSL the Windows install of that version, which renders on the real GPU, else the Mac app, else on Linux the newest official tarball of that version unpacked in the account's `~/opt` (`~/opt/blender-5.2.3-linux-x64/blender`; the account's home, since a render worker job's `HOME` is its slot's), else `blender` on the `PATH`. `run.sh --find` prints which. The render worker has 5.2.3 there from blender.org's tarball (`curl` a mirror blender.org lists, check its `.sha256`, `tar -xJf` into `~/opt`, no sudo). To move to a new Blender, change `version`, run `tools/characters/build_bodies.sh` and the props, and look at what changed.
- **The script** runs as `__main__` with its own folder on `sys.path`, so it can import the modules beside it. Its arguments are the ones after Blender's `--` (`sys.argv[sys.argv.index("--") + 1:]`).
- **Paths:** relative paths are from the folder you run it in. Absolute WSL paths are turned into Windows ones for a Windows Blender.
- **Options as arguments:** environment variables don't reach a Windows program from WSL, so pass a script its options as arguments.
- **Start:** without `--open`, Blender starts from its factory settings with the default scene, without anyone's preferences or add-ons.
- **Output:** Blender's noise is filtered out. The script's output comes line by line, so its error is the last thing it says, read with `| tail`; the traceback starts at the script's own frame; a native crash prints the Python stack it died in. A script needs no handler of its own to be heard. The exit code is the script's: an uncaught error gives 1, and `SystemExit(n)` gives `n`; a failure also says `run.sh: <script> exited <n>` on stderr.

**[campfire.py](campfire.py)** builds the campfire from the scans [tools/props/campfire.ts](../props/campfire.ts) lays out (`props.ts refine campfire` runs both): it cuts, scales and seats each log, stone and stick, chars the logs and bakes their colour, normal and crack-glow maps high to low, with the worn ground under it, written as one glTF binary for campfire.ts to add the coal bed to.

**[scan.py](scan.py)** makes a scanned model, such as a Poly Haven rock or fern, placeable by the hundred: each piece welded, cut to a triangle budget with its textures kept, its foot at the origin, written as its own glTF binary. `tools/blender/run.sh tools/blender/scan.py MODEL.gltf OUT_PREFIX BUDGET [--whole] [--loose] [--crease DEG]`: `--loose` takes a set scanned as one object a loose part at a time, largest first, and `--crease` splits normals at creases rather than smoothing everywhere, as `tools/props/scans.ts` asks for the flora file's rocks.

**[pieces.py](pieces.py)** is what `scan.py` and `campfire.py` share: a mesh's triangles, welding it along its seams, and cutting it to a triangle budget with Blender's collapse decimation, which keeps its texture places so its scanned maps still fit.

**[tips.py](tips.py)** finds where a modelled tree's leaves go, for a crown of cards grown on a modelled trunk (D235): `tools/blender/run.sh tools/blender/tips.py MODEL.glb OUT.ron [--from 0.5] [--clump 1.0]` measures how far each point is from the foot along the surface, gathers the points past `--from` of the longest path into clumps a `--clump` apart, each with the way its branch runs and a mass by side and height, and writes them as RON for `assets/data/trees`.

**[trunk.py](trunk.py)** makes a modelled trunk ready for the game's own bark, which the lush forest lays on it from three sides (D264): `tools/blender/run.sh tools/blender/trunk.py MODEL.glb NEAR.glb FAR.glb [--near 20000] [--far 5000]` takes the trunk `tools/props` refined, faces it all outward, writes it subdivided once and cut to `--near` triangles for near the camera and cut to `--far` for beyond, smooth, with tangents, and with how open each vertex is to the sky (rays cast against the model) in its colour, without its textures. Run it on what `props refine` writes, the near file in its place.

**Checked by `cargo xtask check`:** [check.py](check.py) compiles every tracked script, checks what it imports exists in this Blender and what it uses from our own modules is there, and imports the shared modules, without running any script. A machine with no Blender skips it.

Scripts that run inside Blender are the one place Python is allowed (D121). A module other scripts import must run nothing when imported. Write what they make under an ignored folder such as `shots/`; models and renders are build output.
