# Uthana CLI

Makes animation clips from text with [Uthana](https://uthana.com), retargeted onto our body, for the clips Kimodo makes badly: idles, sprint, swimming, reactions, climbing (D532). TypeScript, run directly by Node 22.18 or later, with no dependencies.

## Setup

1. Make an account at <https://uthana.com> and add credits (pay as you go; turn auto top-up off).
2. Copy the API key from the account settings.
3. Put it in `.env` at the repository root (worktrees read the main checkout's): `UTHANA_API_KEY=<key>`.

Check it with `node tools/uthana/uthana.ts org`.

## Use

```bash
node tools/uthana/uthana.ts motion "a man stands at ease, weight shifting, arms loose at his sides" -o idle.glb --length 6
```

The first `motion` uploads `assets/models/character/base.glb` as a character (its id kept in `~/.cache/terra/uthana.json`, uploaded again when the body changes), so every clip comes on our skeleton. The clip is saved motion only, at 30 fps. A clip is judged by the character tools' checks ([tools/characters](../characters/README.md)) and the workbench, as a Kimodo take is. Each `motion` prints one line of JSON with the motion, the character, the dollars spent and the file.

| Model | `--model` | Cost (2026-10-26) | Notes |
|---|---|---|---|
| Text-to-motion 1.0 | `1` | $0.02 a second | |
| Text-to-motion 2.0 | `2` (default) | $0.03 a second | `--seed`, `--foot-ik` |
| Text-to-motion 3.0 | `3` | $0.10 a second | 4–10 s, a job polled every 5 s; unlocked by asking Uthana's support |

`--in-place` takes out the root's horizontal motion. `schema` lists the arguments the live API takes, for when the docs and the API disagree.

## Rights

Clips made with paid credits are ours ("you own all right, title, and interest in and to the Customer Outputs generated under such Paid Plan", uthana.com/terms), so they may sit in the repository and steer Kimodo. Free-plan clips are Uthana's under CC BY 4.0: don't use the free plan. Uthana may train on what we make.
