# Blender

Runs a Python script inside Blender, headless, so model work is a repeatable command with nothing open. Agents can also work in a Blender someone has open, through its MCP server ([docs/development.md](../../docs/development.md#blender), and the `blender` skill).

```bash
tools/blender/run.sh path/to/script.py [args...]
tools/blender/run.sh path/to/script.py --open scene.blend [args...]
```

- **Which Blender:** the version in [version](version), pinned so every machine makes the same files; `run.sh` refuses another (exit 3). `BLENDER` if it is set, else under WSL the Windows install of that version, which renders on the real GPU, else the Mac app, or `blender` on the `PATH`. `run.sh --find` prints which. To move to a new Blender, change `version`, run `tools/characters/build_bodies.sh` and the props, and look at what changed.
- **The script** runs as `__main__` with its own folder on `sys.path`, so it can import the modules beside it. Its arguments are the ones after Blender's `--` (`sys.argv[sys.argv.index("--") + 1:]`).
- **Paths:** relative paths are from the folder you run it in. Absolute WSL paths are turned into Windows ones for a Windows Blender.
- **Options as arguments:** environment variables don't reach a Windows program from WSL, so pass a script its options as arguments.
- **Start:** without `--open`, Blender starts from its factory settings with the default scene, without anyone's preferences or add-ons.
- **Output:** Blender's noise is filtered out. The exit code is the script's: an uncaught error gives 1, and `SystemExit(n)` gives `n`.

**[scan.py](scan.py)** makes a scanned model, such as a Poly Haven rock or fern, placeable by the hundred: each piece welded, cut to a triangle budget with its textures kept, its foot at the origin, written as its own glTF binary. `tools/blender/run.sh tools/blender/scan.py MODEL.gltf OUT_PREFIX BUDGET [--whole] [--loose] [--crease DEG]`: `--loose` takes a set scanned as one object a loose part at a time, largest first, and `--crease` splits normals at creases rather than smoothing everywhere, as `tools/props/scans.ts` asks for the flora file's rocks.

**Checked by `cargo xtask check`:** [check.py](check.py) compiles every tracked script, checks what it imports exists in this Blender and what it uses from our own modules is there, and imports the shared modules, without running any script. A machine with no Blender skips it.

Scripts that run inside Blender are the one place Python is allowed (D121). A module other scripts import must run nothing when imported. Write what they make under an ignored folder such as `shots/`; models and renders are build output.
