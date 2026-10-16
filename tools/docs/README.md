# Docs check

Keeps the docs navigable as they grow. `cargo xtask check` runs it; on its own:

```bash
node tools/docs/docs.ts
```

It fails on:

- **A link that leads nowhere**: a relative link in any tracked `.md` to a missing file, or to a heading the file doesn't have.
- **Something missing from its index**: a doc not in `docs/README.md`, a proposal not in `docs/proposals/README.md`, a note not in `research/README.md`, a spike not in `spike/README.md`, or a tool's script not in its README.
- **A malformed decision** (D389): a file in `docs/decisions/` not named for its number, or not shaped `# Dnnn: Topic`, a blank line, `Area: … · Owner: … · By: … · Status: …`, a blank line, the decision, then optionally `## Why` and the reason; or with an Area that is not one of the index's headings (— is allowed when Rejected or Superseded), an unknown Owner or Status, or By not Developers or Agent (or — when Open, Rejected or Superseded).
- **A decisions index that isn't its files**: [docs/decisions.md](../../docs/decisions.md) below its preamble is written from the files, so a hand edit, or a table written into the page, fails with where decisions live and the command that writes the page. Its areas and their one-line intros are `AREAS` in `docs.ts`.
- **A decision cited that doesn't exist**: a `D` number in any tracked doc, code, data or script that has no file.
- **A contract that isn't its data**: the voxel contract's junction matrix ([docs/building-voxels.md](../../docs/building-voxels.md)) is written from [docs/building-voxels.json](../../docs/building-voxels.json); the page must match it, and every test it names must exist. Edit the data, then `node tools/docs/docs.ts --render` writes the page. Whether each row's code, tests and cases are there is terra_build's test `every_row_of_the_voxel_contract_is_proved`.

`node tools/docs/docs.ts --render` writes both rendered pages. `node tools/docs/docs.ts --next` (`cargo xtask decision`) prints the next free decision number, the higher of the files here and on origin/main after a fetch; two agents taking one at once meet as the rebase stopping on the file both added, and whoever pushes second renumbers theirs. `node tools/docs/docs.ts --show D28 D198` (`cargo xtask decision D28 D198`) prints those decisions in full, each with the log's lines that name it.

The tests are `node --test tools/docs/docs.test.ts`.
