# tools/ci

The CI agent ([docs/ci-agent.md](../../docs/ci-agent.md), D411): `cargo xtask nightly` run whenever origin/main moves, its report turned into GitHub issues labelled `ci-agent`.

- `docker compose up -d --build` (with `.env` holding `GH_TOKEN`) runs it on a box: `Dockerfile` is the toolchain, `entrypoint.sh` clones into the `work` volume and starts the watch, `compose.yaml` keeps it running. Optional settings in `.env`: `TERRA_REPO` (default dcramer/terra), `TERRA_CI_EVERY` (minutes between looks, default 2), `GIT_AUTHOR_NAME` and `GIT_AUTHOR_EMAIL` for the baseline commits.
- `cargo xtask ci --watch [minutes]` is the same without Docker; `cargo xtask ci [--dry-run] [--no-run]` runs once. `--dry-run` prints what it would do on GitHub and touches nothing; `--no-run` reads the last report without running nightly.

Output is a line for nightly and a line for the issues. Nightly's own output goes to `.git/terra-ci/run.log`, its report to `.git/terra-nightly/report.txt`. What it last posted for each stage is kept in `.git/terra-ci/seen.json`, so an unchanged failure is not posted again.

`ci.ts` reads the report and plans the issues as pure functions (`parse`, `plan`), tested in `ci.test.ts`; everything else is git, cargo and `gh`. Node 22.18 or later, no dependencies.
