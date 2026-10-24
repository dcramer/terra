# Combat

Combat's pipeline (docs/movement.md, "The goal"): a move goes from its spec to a judged film by commands an agent runs. This folder holds what the steps compare against; the clip and move-set checks live with the bodies in [tools/characters](../characters/README.md) (`phases.ts`, `moves.ts`).

| File | What it is |
|---|---|
| `refs/enshrouded.json` | Enshrouded's melee measured from first-party video (D180): timings in seconds, distances in body heights, the camera's framing, each with its source, timestamp, method and confidence; the ranges our set should land in (`targets`); and what these videos cannot show. How it was read and how our move set compares: [research/2026-10-24-enshrouded-combat-measured.md](../../research/2026-10-24-enshrouded-combat-measured.md) |

The reference's pictures stay out of the repository, under `~/.cache/terra-review/refs/enshrouded-combat/`.
