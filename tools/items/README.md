# Items: kinds and parts of assembled items

The checks of D565's assembled items ([docs/assembled-items.md](../../docs/assembled-items.md), D566's convention). Node 22.18+, no dependencies. Each prints a line a stage; a failure names the file, the field and the fix (D388, D556).

| Command | What it checks |
|---|---|
| `node tools/items/kind.ts [kind…]` | Every kind in `assets/data/kinds/` (or those named) against `kind.schema.json`; that the file is named for its kind, every number has its source beside it, every slot fits part types that exist in `assets/data/parts/types.json` and are made as it says, and the reference's inertia and envelope follow D459's rule from its own numbers |

`schema.ts` is the JSON Schema (2020-12) validator they share: only the keywords our schemas use, and it refuses a schema with any other, so nothing passes by a keyword it ignored. Written rather than taken from npm (ajv) because tools carry no dependencies; ajv 8 and it agree on the eleven kinds.

The tests (`*.test.ts`) run in `cargo xtask check`'s tools stage and hold the real kinds to the check.
