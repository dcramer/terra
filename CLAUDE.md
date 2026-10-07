# Terra

A co-op survival sandbox about terraforming a dead planet. Pre-production.

Everything is being built in Rust on Bevy, with our own voxel terrain (see `docs/tech.md`). No Rust workspace exists yet; milestone R1 in `docs/prototypes.md` creates it.

| Folder | What it is |
|---|---|
| `docs/` | Decisions and plans. Start here. |
| `research/` | Findings, surveys and measurements, one dated note per question. |
| `demo/` | The playable tech demo on Godot. A working reference until the Rust build replaces it. |
| `prototypes/sim_toy/` | The planet simulation (Rust) and a globe to watch it on (Godot). |
| `spike/` | Throwaway experiments, including the Rust toolchain check in `spike/rust_stack/`. |

## Before doing anything

Read [docs/README.md](docs/README.md), then [docs/decisions.md](docs/decisions.md).

## Rules

- `docs/decisions.md` is the source of truth. Don't contradict a Decided item; raise it instead.
- When a decision is made or changes, update `docs/decisions.md` and any doc it affects in the same change, and add a line to its log.
- Don't record a proposal as Decided. Unconfirmed ideas are Leaning or Open.
- Keep docs short: intent, constraints and reasons, not implementation detail.
- Don't overbuild. Nothing is built on a Leaning or Open decision except a spike, and a spike answers one stated question with the least code that can answer it, then stops. No game systems until the decisions they rest on are Decided.
- Technical decisions are yours to make: decide, record the reason in `docs/decisions.md`, and keep going. Design decisions belong to the developers: give a recommendation, mark it Open, and don't build on it.
- When you investigate something, write it up as a dated note in `research/` and link it from `research/README.md`. Research informs decisions; it does not make them.
- This is a public repository. Don't commit engine binaries, build output, logs, or anything personal.
