# Tripo CLI

Makes, textures, rigs and animates 3D models with [Tripo](https://studio.tripo3d.ai), spending our Studio subscription's credits rather than API credits. It does from the command line what the Studio web app does, so a person or an agent can script it. TypeScript, run directly by Node 22.18 or later, with no dependencies and no build step.

## Setup

The CLI logs in as you, with the session cookie of a browser signed in to Studio. It lasts about 30 days.

1. Sign in at <https://studio.tripo3d.ai>.
2. Open developer tools, then **Application → Cookies → https://studio.tripo3d.ai**, and copy the value of `ory_kratos_session`.
3. Put it in `.env` at the repository root (worktrees read the main checkout's): `TRIPO_STUDIO_COOKIE=<value>`. The whole `cookie` header of any request to `api.tripo3d.ai` works too.

Check it with `node tools/tripo/tripo.ts balance`. When it says the login failed, copy the cookie again.

## Use

```bash
tripo=tools/tripo/tripo.ts

node $tripo image "a weathered supply crate, plain grey background" -o crate.png
node $tripo model crate.png -o crate.glb              # prints {"project": "…", "credits": 25, …}
node $tripo rig <project>                              # a humanoid, in a T- or A-pose
node $tripo animate <project> walk run idle -o crate-anim.glb
```

Every command that makes or changes a model prints one line of JSON with the `project` it worked on, the `credits` it spent and the `files` it wrote, so the next command can take the project. A project is the id or the project's Studio URL. A project keeps every step done to it; each command works on its latest. `node $tripo <command> --help` lists a command's options.

| Command | What it does | Credits |
|---|---|---|
| `balance` | Credits left | |
| `list` | Your projects, newest first | |
| `show PROJECT` | What has been done to a project: textured, rigged, its animations | |
| `image PROMPT [--ref IMG …]` | Images from a prompt, optionally like up to ten reference images. `--t-pose` for a figure to rig, `--count`, `--aspect`, `--4k`, `--model` | free (100 a month), then 10 |
| `views IMAGE` | Front, left, back and right views of the subject, for `model --views` | up to 20 |
| `model IMAGE` | A model from one image | 25 (H3.1, textured) |
| `model --text PROMPT` | A model from a prompt | 25 |
| `model --views F L B R` | A model from views; `-` for a missing one, front and one more needed | 25; 45 with `--model p1` |
| `import FILE` | Brings in our own GLB, FBX, OBJ or STL to work on | free |
| `texture PROJECT --image IMG \| --views F L B R \| --prompt TEXT` | Textures a model; `--quality standard\|detailed\|extreme` is 2K, 4K or 8K | 10 |
| `upscale PROJECT [--8k]` | Raises the texture to 4K or 8K | 10 |
| `pbr PROJECT` | Adds normal, roughness and metallic maps | 5 |
| `retopo PROJECT --faces N [--quad]` | Rebuilds the mesh at a face budget | 5 |
| `segment PROJECT` | Splits the model into parts | 40 |
| `fill PROJECT [--ai]` | Closes the holes segmenting leaves, or rebuilds hidden parts | free |
| `uv PROJECT` | Unwraps the UVs again (Smart UV) | 20 (free during a trial) |
| `rig PROJECT [--skeleton …]` | Auto-rig, with fingers, by Tripo's current (V3) rigger. `unreal` (default), `mixamo` (65 joints), `actorcore`, `unity` and `vrm` name the bones for that target; `tripo` is Tripo's older V1 rig (78 joints, `BoneRoot`, `Hip`, `L_Thigh`…). Only `unreal` weights its twist bones (upper arm, forearm, thigh, calf); V1's and `actorcore`'s twist and share bones move nothing, and `mixamo` and `unity` have none on the limbs | 20 |
| `animate PROJECT NAME … [--prompt TEXT]` | Puts preset animations (`walk`, `run`, `idle`, `jump`, …) or ones made from a description on a rigged model, and exports them in one GLB or FBX | presets free, `--prompt` 20 |
| `export PROJECT --format glb\|fbx\|obj\|stl\|usdz\|3mf` | Exports in a format; `--animations` to include animations, `--texture-size`. OBJ comes zipped with its textures | free |
| `download PROJECT` | The model as Studio stores it | free |
| `report [--all]` | Notes the account's spending for the token stats; every command does this too | |

`model` options: `--model` picks the generator: `h3.1` (the default, the best quality), `h3.0`, `h2.5`, or Smart Mesh `p1` and `p2`, which make cleaner low-poly meshes and are textured in a second step, about 45 credits in all. `--faces` (default 10,000), `--quad`, `--no-texture`, `--detailed` (finer geometry and a 4K texture, more credits), `--pbr`, `--parts`, `--public`.

`download` gives Studio's meshopt-compressed GLB, which some tools cannot open; `export --format glb` gives a plain one.

A project runs one operation at a time, so a command on a project that is busy waits for what is running, then starts. A command stopped with Ctrl-C leaves its operation running on Studio; the next command on that project waits for it. The `credits` a command prints are read from the balance before and after, so they are off when two runs overlap.

## Credits in the token stats

The [token stats](../token-stats/README.md) count Tripo's credits beside Claude's tokens, priced at what they cost on our plan. After every command the CLI reads the account's own charge history and notes each charge once, by its operation, in the ledger `report.mjs` sends from (`~/.local/state/terra-token-stats/spends.jsonl`), so work done by hand in the web app counts too, the next time anyone on that account runs the CLI. `report --all` notes the whole history. It needs nothing beyond the token stats' own setup; without it, the ledger simply waits.

## When it breaks

This is not a public API: it makes the requests the web app makes, and Tripo can change them without notice. When a command fails where it used to work, capture what the web app sends now and fix the request in `tripo.ts` (the requests) or `studio.ts` (login, uploads, waiting). `node --test` in this folder runs the tests that need no Tripo. The way to capture is in [research/tripo-studio-api.md](../../research/tripo-studio-api.md).
