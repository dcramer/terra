---
name: blender
description: Work in Blender for Terra, headless through tools/blender/run.sh or live in the developer's open Blender through its MCP server (mcp__blender__* tools). Use when asked to open, inspect, clean up, fit, render or export a model or .blend in Blender, or when a Blender MCP call fails.
---

# Blender

Blender is for cleaning up, fitting, rigging, rendering and exporting models, not for making them: the agent's own models were blockouts, and Tripo is the generator (`tripo` and `character` skills, [research](../../../research/ai-models-for-art.md)). Setup is in [docs/development.md](../../../docs/development.md#blender). Rigging traps are in the `character` skill.

## Which way

- **Headless**, the default: `tools/blender/run.sh <script.py> [--open file.blend] [args]` ([README](../../../tools/blender/README.md)). Use it for anything repeatable. A script worth keeping lives with what uses it, not in a spike someone else's work depends on.
- **Live, through MCP**, when the developer has Blender open and wants to watch, try things, or have you inspect what they have. Code sent this way is gone afterwards, so once something works, put it in a script.

## Live through MCP

- **The MCP server and Blender run on Windows.** Every path you give them is a Windows path: `wslpath -w <path>` (`\\wsl.localhost\Ubuntu\...`). Read what they write back through the WSL path.
- **"Cannot connect to Blender at localhost:9876"** means the add-on's server isn't running. `claude mcp` showing "Connected" only means the bridge is up. Ask the developer to open Blender and press Start in the MCP add-on's preferences. The add-on also needs Preferences → System → Network → Allow Online Access turned on, even for localhost. Check it from WSL with `powershell.exe Test-NetConnection localhost -Port 9876`.
- **The `*_for_cli` tools** open a `.blend` in a background Blender without the live one. They timed out until the launcher stopped passing its stdin on (docs/development.md, step 3); if they time out again, check that first. `tools/blender/run.sh --open` does the same job.
- **It is the developer's scene.** Look before you change anything (`get_objects_summary`, `get_object_detail_summary`). Keep their names and collections, and ask before deleting or overwriting anything, including saving over their file.
- **Prefer the specific tools to `execute_blender_code`.** In code, assign a dict to `result` to get data back.
- **Check the API, don't recall it.** `search_api_docs` and `get_python_api_docs` match the running Blender (5.2). Blender 5 removed `scene.node_tree` for the compositor and `Action.fcurves` (actions are slotted), and deprecated `use_nodes`. Bones' selection and visibility moved to pose bones (`Bone.select` is gone, `Bone.hide` is edit-bone visibility only). Assigning an action does not pick its slot, so a clip that assigns but does not move needs its slot set.
- **Look at what you did** with `get_screenshot_of_area_as_image` (`VIEW_3D`) or `render_viewport_to_path`, then read the image.

## Traps already found

- **Blender's Python has numpy but no PIL.** Make contact sheets with ImageMagick in WSL. Python is allowed only inside Blender (D121).
- **Operators fail quietly when headless.** Some cancel with no window (`quadriflow_remesh`), and some fail `poll()` without the right mode and context (`uv.smart_project` needs edit mode and a `temp_override`). Print every operator's return value, and print the counts that matter, such as vertices weighted. Bone heat weighting can fail with only a warning and still exit 0.
- **Ask Blender what it sees before exporting.** Print each object's parent, modifiers and vertex groups. A mesh copied from an imported one keeps the importer's parenting, and the glTF exporter then writes no skin.
- **Generated meshes are split at UV seams.** Weld with `bmesh.ops.remove_doubles(dist=1e-4)` before any topology or weight work.
- **Only deform-bone vertex groups are weights.** When copying groups, filter by the armature's deform bones, or marker groups get treated as weights.
- **Importing twice renames everything `.001`**: materials, objects and actions, so a file can end up exporting 52 clips instead of 26. Import once per session, or clear what is already there first.
- **Tripo downloads are compressed in a way Bevy can't read.** An import and export through Blender fixes it. Tripo models sit centred on the origin, so ground them first.
- **Prefer a number to a picture.** When renders keep going wrong, write a check that measures the problem (as `rig_check.py` does), rather than iterating by eye.
- **Output models and renders are build output.** Write them under an ignored folder (`shots/`, `out/`), never commit them.
