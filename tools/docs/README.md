# Docs check

Keeps the docs navigable as they grow. `cargo xtask check` runs it; on its own:

```bash
node tools/docs/docs.ts
```

It fails on:

- **A link that leads nowhere**: a relative link in any tracked `.md` to a missing file, or to a heading the file doesn't have.
- **Something missing from its index**: a doc not in `docs/README.md`, a proposal not in `docs/proposals/README.md`, a note not in `research/README.md`, a spike not in `spike/README.md`, or a tool's script not in its README.
- **A malformed decision**: a row of `docs/decisions.md` without its seven cells, with an unknown Owner or Status, with By not Developers or Agent (or — when Open, Rejected or Superseded), or in the table twice. Two agents taking the same number at once is how a number gets in twice: the fault names both rows and the next free number, and whoever pushes second renumbers theirs.
- **A decision cited that doesn't exist**: a `D` number in any doc that the table lacks.
- **A contract that isn't its data**: the voxel contract's junction matrix ([docs/building-voxels.md](../../docs/building-voxels.md)) is written from [docs/building-voxels.json](../../docs/building-voxels.json); the page must match it, and every test it names must exist. Edit the data, then `node tools/docs/docs.ts --render` writes the page. Whether each row's code, tests and cases are there is terra_build's test `every_row_of_the_voxel_contract_is_proved`.

`node tools/docs/docs.ts --next` (`cargo xtask decision`) prints the next free decision number, the higher of the one here and the one on origin/main after a fetch.

The tests are `node --test tools/docs/docs.test.ts`.
