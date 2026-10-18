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
- **A guide that has drifted from the code** (D396): a `cargo xtask <task>` named in docs/, a skill, AGENTS.md or a tool's README that is no task in `xtask/src/main.rs`'s `TASKS`; or a repository path in a skill's code spans (a word starting with a top folder, `crates/…`, less a `:line`) that is neither tracked nor somewhere git ignores, where a tool writes. A word with a placeholder (`<name>`, `NAME`, `Dnnn`, `*`, `{a,b}`) is not a path.
- **A contract that isn't its data**: the voxel contract's junction matrix ([docs/building-voxels.md](../../docs/building-voxels.md)) is written from [docs/building-voxels.json](../../docs/building-voxels.json); the page must match it, and every test it names must exist. Edit the data, then `node tools/docs/docs.ts --render` writes the page. Whether each row's code, tests and cases are there is terra_build's test `every_row_of_the_voxel_contract_is_proved`.

- **A research index that isn't its notes** (D409): [research/README.md](../../research/README.md)'s table is written from the notes, each note's `# Title` and the date in its name or at its top, newest first; a note with neither fails.
- **A design call settled by an agent** (D412): a decision with Owner Design and Status Decided must be By Developers; an agent's design call stays Leaning until they settle it. `SETTLED_BY_AGENTS` lists those from before the rule, for the developers to confirm or reopen.

`node tools/docs/docs.ts --render` writes the rendered pages. `merge.ts` is git's merge driver for the two indexes (D409, `merge=rendered` in `.gitattributes`, set up by any `cargo xtask` task): it merges them row by row, keyed by what each row links to, so two agents adding a decision or a note at once never conflict; what it cannot place it leaves as an ordinary conflict for `--render` to mend. `node tools/docs/docs.ts --next` (`cargo xtask decision`) prints the next free decision number, the higher of the files here and on origin/main after a fetch; two agents taking one at once meet as the rebase stopping on the file both added, and whoever pushes second renumbers theirs. `node tools/docs/docs.ts --show D28 D198` (`cargo xtask decision D28 D198`) prints those decisions in full, each with the log's lines that name it.

The tests are `node --test tools/docs/docs.test.ts`.
