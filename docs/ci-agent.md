# The CI agent: a machine that runs the slow checks and files issues

The commit and push gates scale with the change (D398), so the slow checks (every test, the artifacts ratchet over every scene, fuzz over 200 seeds) run only in `cargo xtask nightly` (D411). The CI agent runs that continuously on a machine of ours: whenever origin/main moves, it runs and turns what breaks into GitHub issues ([tools/ci](../tools/ci/README.md), D411).

## Running it

It ships as a container, so the box needs only Docker. On the box, in `tools/ci/` of any copy of the repository:

1. Write `tools/ci/.env` (git ignores it) with `GH_TOKEN=<token>`: a classic GitHub token with only the `public_repo` scope, which covers the baseline push and the issues. A fine-grained token cannot reach a repository its owner only collaborates on, as dcramer/terra is for all but its owner.
2. Run `docker compose up -d --build`.

The container clones the repository into its `work` volume, then watches origin/main every 2 minutes (`TERRA_CI_EVERY`). Nightly takes 20–30 minutes, so each run takes in the commits that landed during the last one, and the bisect names the one that broke a stage.

- `docker compose logs -f` shows it.
- `docker compose run --rm ci --dry-run` runs once and prints what it would file.
- It updates itself: when a pull changes `tools/ci`, it exits and the restart policy starts it on the new code. A change to the Dockerfile needs `docker compose up -d --build` again.

Without Docker, `cargo xtask ci --watch` does the same in a checkout; `cargo xtask ci [--dry-run] [--no-run]` runs once.

## What it files

- **One issue per failing stage**, labelled `ci-agent`, found again by a marker in its body. It quotes the commit, the first bad commit from nightly's bisect, and the failing lines.
- **A comment** when a stage still fails with different lines. Nothing when the lines are the same.
- **Closing:** the issue is closed with a comment once its stage passes.
- **`nightly: run fails to run`** when nightly cannot start (fetch, build of xtask), quoting the end of its output.
- **`CI agent status`**, edited every run with the time, the machine and the last nightly line. If it stops changing, the box is down.

A passing nightly lowers the baselines and pushes that commit to main (D414). So the box needs push access, and only the box should run nightly.

## Why this shape

- **Not GitHub Actions.** GitHub advises against self-hosted runners on public repositories, since a pull request's workflow runs on the machine. GitHub Actions stays off (D37).
- **Pull-based.** Like Chromium's and LLVM's build bots, it only pulls, so it works behind a home router with no open ports. Nightly already holds the hard parts (its own worktree, the bisect, the lowering, the report), so the agent is a thin layer turning the report into issues.

## The box

- **Hardware:** any machine with Docker; Linux, or Windows with Docker Desktop. 32 GB RAM or more, and about 100 GB of free disk for the volumes: history, checkout, the seed build and nightly's worktree.
- **No GPU:** nightly needs none. Benchmarks will, with the GPU passed into the container.
- **Credentials:** the token is the only credential, kept in `.env` on the box and never in the repository. It runs main's code, as anyone with push access can already make it do.

## Next

- **Benchmarks:** `cargo xtask bench` after a passing nightly, against the box's own last results, which needs bench to write its results outside `research/benchmarks.csv`. Also `cargo xtask rebuilds` daily, and `filmcheck` once it exists.
- **Triage by an agent:** a bounded `claude -p` triage of each new issue (D400), commenting the cause and never landing. It waits until the issues show the need.
