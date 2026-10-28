# Items: kinds and parts of assembled items

The checks of D565's assembled items ([docs/assembled-items.md](../../docs/assembled-items.md), D566's convention). Node 22.18+, no dependencies. Each prints a line a stage; a failure names the file, the field and the fix (D388, D556).

| Command | What it checks |
|---|---|
| `node tools/items/part.ts <part>` | One part, `assets/data/parts/<part>.json` (`part.schema.json`), against its type in `types.json`: its points (each its template's, on the line facing +Y for a join along a handle, a full point with its roll and on the surface, the origin where the type says), no node scaled, its openings measured round the line within 1 mm of their declared sizes (wider is a gap round the handle, narrower buries it; a tang's flat across Z and X), its triangles and textures within budget, its materials named by role, its bounds; its size for each kind it is made for (a head's mass in the kind's envelope, a handle's length and grip); and every pairing its kinds allow with the parts that exist (a handle's top not in the head's top face, a trim against its head, a boss or rim seated on its board): no gap, nothing buried, no two faces in one plane. A part made by code is checked from its numbers |
| `node tools/items/kind.ts [kind…]` | Every kind in `assets/data/kinds/` (or those named) against `kind.schema.json`; that the file is named for its kind, every number has its source beside it, every slot fits part types that exist in `assets/data/parts/types.json` and are made as it says, and the reference's inertia and envelope follow D459's rule from its own numbers |

`schema.ts` is the JSON Schema (2020-12) validator they share: only the keywords our schemas use, and it refuses a schema with any other, so nothing passes by a keyword it ignored. Written rather than taken from npm (ajv) because tools carry no dependencies; ajv 8 and it agree on the eleven kinds.

The joins are placed by the assembler, never here: `part.ts` asks `cargo run -p terra_items --bin assemble -- joins <part> --parts -` (the parts it checks against on stdin) how each pair sits, and measures them there (`terra_items::assembly::meet`, the one placing rule). A working part gives its `volume_m3` and is weighed in each material of `assets/data/parts/materials.json`; a trim gives its `mass_kg`. An assembled item's model, for the held-look checks, is `cargo run -p terra_bake --bin item -- <kind> <part>... --out FILE.glb` (`item.test.ts` reads one as `attach_contract.ts` and `grip.ts` read a held look).

`mesh.ts` holds the measures on a part's triangles (slices, rays from the line, distances, inside, shared planes); `fixtures.ts` the made-up parts the tests use (a box head with a round eye, a collar, a code-made haft), built from numbers so the answer is known without the checker.

The tests (`*.test.ts`) run in `cargo xtask check`'s tools stage and hold the real kinds to the check.
