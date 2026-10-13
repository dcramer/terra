# Docs check

Keeps the docs navigable as they grow. `cargo xtask check` runs it; on its own:

```bash
node tools/docs/docs.ts
```

It fails on:

- **A link that leads nowhere**: a relative link in any tracked `.md` to a missing file, or to a heading the file doesn't have.
- **Something missing from its index**: a doc not in `docs/README.md`, a proposal not in `docs/proposals/README.md`, a note not in `research/README.md`, a spike not in `spike/README.md`, or a tool's script not in its README.
- **A malformed decision**: a row of `docs/decisions.md` without its seven cells, with an unknown Owner or Status, with By not Developers or Agent (or — when Open, Rejected or Superseded), or in the table twice.
- **A decision cited that doesn't exist**: a `D` number in any doc that the table lacks.

The tests are `node --test tools/docs/docs.test.ts`.
