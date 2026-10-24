# tools/ci

The CI agent ([docs/ci-agent.md](../../docs/ci-agent.md), D411): `cargo xtask nightly` run whenever origin/main moves, its report turned into GitHub issues labelled `ci-agent`.

- `up.sh` sets a box up in one command (docs/ci-agent.md): it keeps `GH_TOKEN` in `~/terra-ci/.env`, fetches the three files below from main and runs `docker compose up -d --build`, run again to update.
- The container: `Dockerfile` is the toolchain, `entrypoint.sh` clones into the `work` volume and starts the watch, `compose.yaml` keeps it running.
- The render worker beside it ([tools/render-worker](../render-worker/README.md), D445): `render.Dockerfile` and `render-entrypoint.sh`, the `render` service of `compose.yaml`, started when `.env` holds `COMPOSE_PROFILES=render` (`TERRA_RENDER=1 up.sh` writes it). It needs an NVIDIA card and the NVIDIA Container Toolkit. Optional settings in `.env`: `TERRA_REPO` (default dcramer/terra), `TERRA_CI_EVERY` (minutes between looks, default 2), `GIT_AUTHOR_NAME` and `GIT_AUTHOR_EMAIL` for the baseline commits.
- `cargo xtask ci --watch [minutes]` is the same without Docker; `cargo xtask ci [--dry-run] [--no-run]` runs once. `--dry-run` prints what it would do on GitHub and touches nothing; `--no-run` reads the last report without running nightly.

Output is a line for nightly and a line for the issues. Nightly's own output goes to `.git/terra-ci/run.log`, its report to `.git/terra-nightly/report.txt`. What it last posted for each stage is kept in `.git/terra-ci/seen.json`, so an unchanged failure is not posted again.

`ci.ts` reads the report and plans the issues as pure functions (`parse`, `plan`), tested in `ci.test.ts`; everything else is git, cargo and `gh`. Node 22.18 or later, no dependencies.
