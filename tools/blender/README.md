# Blender

Runs a Python script inside Blender, headless, so model work is a repeatable command with nothing open. Agents can also work in a Blender someone has open, through its MCP server ([docs/development.md](../../docs/development.md#blender), and the `blender` skill).

```bash
tools/blender/run.sh path/to/script.py [args...]
tools/blender/run.sh path/to/script.py --open scene.blend [args...]
```

- **Which Blender:** `BLENDER` if it is set. Otherwise, under WSL, the newest Windows install, which renders on the real GPU. Otherwise the Mac app, or `blender` on the `PATH`.
- **The script** runs as `__main__` with its own folder on `sys.path`, so it can import the modules beside it. Its arguments are the ones after Blender's `--` (`sys.argv[sys.argv.index("--") + 1:]`).
- **Paths:** relative paths are from the folder you run it in. Absolute WSL paths are turned into Windows ones for a Windows Blender.
- **Options as arguments:** environment variables don't reach a Windows program from WSL, so pass a script its options as arguments.
- **Start:** without `--open`, Blender starts from its factory settings with the default scene, without anyone's preferences or add-ons.
- **Output:** Blender's noise is filtered out. The exit code is the script's: an uncaught error gives 1, and `SystemExit(n)` gives `n`.

Scripts that run inside Blender are the one place Python is allowed (D121). Write what they make under an ignored folder such as `shots/`; models and renders are build output.
