# Agents in Blender: outside advice against our playbooks

2026-10-12. Question: what is the current outside advice on agents working in Blender, with skeletons and generated models, and are our playbooks (the `blender`, `character` and `tripo` skills) still current?

Mostly read through search summaries, and much of what turned up is vendor marketing or community skills, so treat tool claims as unverified. The Blender API changes below were checked against the API docs bundled with the MCP server for our Blender (5.2.2). One thing was found and fixed by running it: the MCP's background tools timing out.

## What the outside world says

- **The official MCP server is Blender Lab's** ([page](https://www.blender.org/lab/mcp-server/), [releases](https://projects.blender.org/lab/blender_mcp/releases)): 1.0.0 on 2026-04-27, 1.0.3 on 2026-09-11 (a screenshot fix), Blender 5.1 or later. Anthropic's Blender connector, launched 2026-04-28, uses it. Ours runs its main branch as of 2026-09-29, so it is current. It is an experimental Lab project; the Foundation says no generative AI is planned for Blender, and a forum thread in May asked whether the Lab project would continue. So it may not last, which is one more reason headless scripts stay our default.
- **Security.** The official page says the server runs generated code with no guards and suggests a VM or a machine with nothing sensitive on it. A [forum thread](https://devtalk.blender.org/t/blender-mcp-server-after-claude-mcp-security-for-blender-scripting-3d-agent-notes/45131) proposes read-only inspection first, then confirmation before edits, file writes and exports. Our docs say to keep it to your own machine, and the skill says to look before changing anything and ask before deleting or saving over a file. That matches.
- **The workflow practitioners converge on** is the one we already use: inspect the scene, prefer the specific tools to raw Python, verify both the data and a picture after each step, and treat a script that ran without errors as unproven. One [asset-validation skill](https://skills.sh/ifBars/blender-agent-studio/blender-asset-validation) says to check the exported file as well as the `.blend`, which our workbench step does. Ours goes further: numeric checkers (`rig_check`, `gear_check`, `deform_check`) instead of judging by eye.
- **Rigging.** Auto-riggers (Tripo, Meshy, Auto-Rig Pro's Smart) place a humanoid skeleton and compute usable weights; non-humanoids are weaker, and every guide expects a person or a script to correct the result. No source showed an LLM agent painting weights or retargeting reliably end to end. That is our split: Tripo rigs, our scripts convert and check.
- **Tripo** publicly documents rig-v2.0, with seven creature types (biped, quadruped, hexapod, octopod, avian, serpentine, aquatic), Mixamo or Tripo bone names, and a free pre-rig check ([docs](https://developers.tripo3d.com/en/models/rig)). Our capture of the Studio API ([tripo-studio-api.md](tripo-studio-api.md)) is newer (the app's `v3.0-20260909` rig). The creature types will matter when we make animals; nothing to change now.

## Blender 5 changes that bite scripts

From a [third-party guide](https://app.cinevva.com/guides/blender-5-for-game-artists), the ones that touch our scripts, checked where marked:

- Bone selection and visibility moved to pose bones in 5.0: `Bone.select` is gone and `Bone.hide` is edit-bone visibility only (checked).
- Assigning an action does not pick its slot, so a keyed clip can land in a new slot, or an assigned action can fail to move anything.
- The glTF exporter merges animations by action slot by default (`export_merge_animation='ACTION'`, checked), and skins with four influences (`export_influence_nb=4`, checked), as Bevy needs.
- 5.1 moved to Python 3.13 and stopped loading the user's site-packages by default. 5.2 added meshopt compression to the glTF exporter (checked); Bevy does not read it, so leave it off.

Our scripts already set the slot when they assign an action (`rig_render.py`, `sockets.py`, `export_character.py`) and use none of the other changed APIs. The bone and slot changes went into the `blender` skill.

## Found and fixed: the MCP's background tools

The skill said the `*_for_cli` tools time out, even on an empty file. Background Blender itself starts in two seconds, and opening a `.blend` over `\\wsl.localhost` works. The cause: the tools start Blender with `subprocess.run` and no `stdin`, so Blender inherits the server's stdin, which is the MCP pipe the server is reading. On Windows, a child that inherits a pipe its parent is blocked reading hangs. A thread reading stdin while `subprocess.run` starts Blender reproduces it, and `stdin=subprocess.DEVNULL` fixes it. Our launcher (`run.py`, [docs/development.md](../docs/development.md#blender), step 3) now sets that default, and `get_blendfile_summary_datablocks_for_cli` answered through the patched server. This belongs upstream too.
