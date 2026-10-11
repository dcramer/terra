# Spike: the model bake-off

Throwaway. Answers one question from [research/ai-models-for-art.md](../../research/ai-models-for-art.md#a-proposed-next-step): for our look, which way of making models gets furthest — the agent writing them in Blender, a generator (Tripo), or both? Findings go in that note.

Six models, each made several ways: rocks, a fence, a workbench, a tree, a wolf and a villager.

| Route | Where |
|---|---|
| The agent in Blender, from scratch | `code/*.py` |
| Tripo: a concept image, then views, then a model | `make.sh`, with [tools/tripo](../../tools/tripo) |
| The agent refining Tripo's model | To come |

## Running

Blender scripts run headless. From WSL, `run.sh` uses the Windows Blender, which renders on the real GPU:

```bash
spike/model_bakeoff/run.sh code/rocks.py
```

Each writes to `out/` (not committed): a `.glb` per model, a sheet of renders in the game's light (`look.py`: our palette, a warm sun, a cool sky) with the sandbox's figure for scale, and a `.json` of checks: triangles against a budget, size, loose parts, and whether every material is in the palette.

Tripo runs through the team's CLI, [tools/tripo](../../tools/tripo), on the Studio subscription; set it up as its README says. `make.sh` is the whole route, with a look at the concept in between:

```bash
spike/model_bakeoff/make.sh concept wolf "grey wolf standing"
spike/model_bakeoff/make.sh model wolf 8000 1.0 0,3000,1500 FurGrey Fur FurDark FurLight Black
```

The bake-off first ran on Tripo's paid API, with a Python client since retired (D121); its findings are in the research note.
