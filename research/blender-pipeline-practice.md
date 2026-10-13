# Blender in the pipeline: how studios do it

2026-10-12. Question: is running our asset pipeline as Python scripts inside a headless Blender (D115, D147, D121) the way game studios do it, and what do they do that we don't?

Read through search summaries and vendor or community pages; studios rarely publish their pipelines, so much of this is the common shape, not named practice. Nothing was measured.

## What we do

- 25 Blender scripts, about 5,100 lines: 23 in `tools/characters/` (convert, fit, check, morph, export), 2 in `tools/props/` (the clean-up). `look.py` is a shared module 18 of them import; `twist`, `patches`, `skin_piece`, `lift` and `bands` are shared too.
- They run headless through `tools/blender/run.sh`, chained by shell scripts (`build_bodies.sh` is 110 lines) or by TypeScript (`props.ts`). The live MCP session is optional.
- Checks are numeric scripts (`rig_check`, `gear_check`, `deform_check`, `clip_check`), and a build ends `BUILD pass` or `BUILD fail`.

## What studios do

- **Scripts in the DCC's own Python.** Maya, Houdini and Blender tooling is Python inside the program, written by technical artists; job ads for the role ask for exactly this. Embark, a studio whose main language is Rust, writes its Blender tools in Python too ([blender-tools](https://github.com/embarkstudios/blender-tools)). So Python here is normal, and D121's exception is the industry's line.
- **Headless batch runs.** `blender -b --python` on a build machine, with arguments after `--`, is the usual way to process many assets. Prefer `bpy.data` to `bpy.ops`, since operators depend on context. We do both.
- **Validate before you publish.** Pipeline frameworks such as Pyblish (used by AYON and ftrack) split a publish into collect, validate, extract and integrate, and nothing is published unless every validator passes. Our checkers and `BUILD pass` are the same idea, and stricter than most: numbers rather than a look.
- **One pinned DCC version a project.** Blender's manual advises one LTS version a project, and each Blender takes one Python version only (`bpy` 5.2.2 on PyPI needs Python 3.13 exactly). Studios install the same version everywhere, so a rebuild gives the same files.
- **Tools packaged in layers, not loose files.** The common shape is a core library with no DCC imports, thin per-DCC adapters, then entry points or an add-on, with a one-way dependency rule checked automatically (pythontk/blendertk, TikWorks, ftrack's framework). Artist-facing actions ship as an add-on; batch jobs are entry points.
- **Tested and linted like other code.** [pytest-blender](https://pypi.org/project/pytest-blender/) runs pytest inside Blender; the `bpy` wheel lets tests import Blender in a plain Python; [fake-bpy-module](https://pypi.org/project/fake-bpy-module/) gives type stubs per Blender version for editors and type checks.
- **Last-mile processing elsewhere.** Format-level work (welding, simplifying, texture compression, pruning) is often done outside the DCC: [glTF Transform](https://gltf-transform.dev/cli) in Node, meshoptimizer, or the engine's import step (Unreal's Interchange pipelines, Godot's post-import scripts, Bevy's [asset processor](https://docs.rs/bevy/latest/bevy/asset/processor/index.html), which reprocesses only what changed). Rigging, skinning, retargeting and baking stay in the DCC.
- **Houdini PDG** is what larger studios use to run procedural asset graphs across a farm, with dependency tracking. Licences and Houdini skills make it a poor fit for us now.
- **Agents in a live Blender** (the MCP route) is not established practice; Blender's own MCP server is an experimental Lab project. Keeping headless scripts as the default is the safe side.

## Where we stand

Broadly, yes: Python inside a headless Blender, with numeric validators, is how this is done. The differences:

1. **No pinned Blender.** `run.sh` takes the newest one installed, so two machines can build different files, and an upgrade changes output silently (Blender 5 broke bone selection and action slots, [agents-and-blender.md](agents-and-blender.md)). Studios pin. The fix is small: name the version in one place and have `run.sh` refuse or warn on a mismatch.
2. **The Python is outside the gate.** `cargo xtask check` doesn't lint, compile or test any of the 5,100 lines; a typo shows up only when someone runs the script. The least that would help is a syntax and import check of every script in Blender when one is installed. A lint (ruff, a single binary) needs no Python setup.
3. **A flat folder.** At 25 scripts, one shared module and a README table, this is fine. If it keeps growing, the next step is the studio shape: shared code in a package, scripts as thin entry points.
4. **Logic in shell.** `build_bodies.sh` and `make_morph.sh` hold lists of assets and the order of the build, which D121 says belongs in TypeScript once it's real logic; `props.ts` already drives Blender that way. This is also where the build could learn to skip what hasn't changed, as Bevy's processor and PDG do.
5. **Format-only steps could leave Blender.** Re-exporting a Tripo file so Bevy can read it, and grounding a model, are glTF edits that glTF Transform can do in Node, faster and without Blender installed. It is only worth doing if those steps get in the way.
6. **One loose script.** `sockets.py` is in no README or build; only research notes mention it. Document it as a check or delete it.

None of these is a design question. 1 and 2 are cheap and catch real faults; 3 to 5 are for when the pipeline grows.

## Sources

- [Embark: blender-tools](https://github.com/embarkstudios/blender-tools); [Using Blender in game development](https://www.blender.org/?p=74597) (Embark, on blender.org)
- [Blender manual: deploying Blender in production](https://docs.blender.org/manual/en/5.0/advanced/deploying_blender.html); [bpy on PyPI](https://pypi.org/project/bpy/)
- [AYON: creator and publisher](https://help.ayon.app/articles/1075843-creator-publisher); [ftrack: publishing with Pyblish](https://ftrack-connect-documentation.readthedocs.io/en/latest/developing/publish_using_pyblish.html)
- [TikWorks package structure](https://tikworks.readthedocs.io/en/latest/architecture/core_concepts.html); [ftrack DCC integration](https://developer.ftrack.com/integrating-pipelines/framework/dcc-integration/)
- [pytest-blender](https://pypi.org/project/pytest-blender/3.0.0); [fake-bpy-module](https://pypi.org/project/fake-bpy-module/20241014)
- [glTF Transform CLI](https://gltf-transform.dev/cli); [Unreal Interchange](https://dev.epicgames.com/documentation/en-us/unreal-engine/importing-assets-using-interchange-in-unreal-engine); [Godot .blend import](https://docs.godotengine.org/en/latest/classes/class_editorsceneformatimporterblend.html)
- [Houdini PDG](https://www.sidefx.com/products/houdini/pdg/); [Blender Lab MCP server](https://www.blender.org/lab/mcp-server/)
- [Polycount and tech-artists.org threads](https://www.tech-artists.org/t/ta-interview-questions/14694) on what technical artists build
