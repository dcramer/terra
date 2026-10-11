# Spike: the model bake-off

Throwaway. Answers one question from [research/ai-models-for-art.md](../../research/ai-models-for-art.md#a-proposed-next-step): for our look, which way of making models gets furthest — the agent writing them in Blender, a generator (Tripo), or both? Findings go in that note.

Six models, each made several ways: rocks, a fence, a workbench, a tree, a wolf and a villager.

| Route | Where |
|---|---|
| The agent in Blender, from scratch | `code/*.py` |
| Tripo: a concept image, then views, then a model | `tripo.py` |
| The agent refining Tripo's model | To come |

## Running

Blender scripts run headless. From WSL, `run.sh` uses the Windows Blender, which renders on the real GPU:

```bash
spike/model_bakeoff/run.sh code/rocks.py
```

Each writes to `out/` (not committed): a `.glb` per model, a sheet of renders in the game's light (`look.py`: our palette, a warm sun, a cool sky) with the sandbox's figure for scale, and a `.json` of checks: triangles against a budget, size, loose parts, and whether every material is in the palette.

Tripo needs `TRIPO_API_KEY` in `.env`, and credits on the account:

```bash
python3 spike/model_bakeoff/tripo.py concept wolf "grey wolf standing"
python3 spike/model_bakeoff/tripo.py views wolf spike/model_bakeoff/out/tripo/wolf-image_url.png
```

Every Tripo result is saved with a `.json` saying what was asked for, which model made it and what it cost.

`--studio` spends the Studio subscription's credits instead of the API's, through the web app's private backend ([research/tripo-studio-credits.md](../../research/tripo-studio-credits.md)). It covers `balance`, `image` and `text` so far. Put the `cookie` request header of any call the signed-in app makes to `api.tripo3d.ai` (developer tools, Network) in `.env` as `TRIPO_STUDIO_COOKIE`; it lasts about 30 days:

```bash
python3 spike/model_bakeoff/tripo.py balance --studio
python3 spike/model_bakeoff/tripo.py image wolf-studio concept.png --studio --model P1-20260311 --texture --faces 2000
```

P1 runs as Studio's Smart Mesh, which comes untextured, so `--texture` adds the app's texture step after it: about 35 + 10 credits.
