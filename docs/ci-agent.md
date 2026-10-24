# The CI agent: a machine that runs the slow checks and files issues

The commit and push gates scale with the change (D398), so the slow checks (every test, the artifacts ratchet over every scene, fuzz over 200 seeds) run only in `cargo xtask nightly` (D411). The CI agent runs that continuously on a machine of ours: whenever origin/main moves, it runs and turns what breaks into GitHub issues ([tools/ci](../tools/ci/README.md), D411).

## Running it

On the box, which needs only Docker, over SSH:

```bash
GH_TOKEN=<token> sh -c 'curl -fsSL https://raw.githubusercontent.com/dcramer/terra/main/tools/ci/up.sh | sh'
```

The token is a classic GitHub token with only the `public_repo` scope, which covers the baseline push and the issues. A fine-grained token cannot reach a repository its holder only collaborates on.

`up.sh` keeps the token in `~/terra-ci/.env`, fetches the container's files there, and starts it. Run the same command without `GH_TOKEN` to update it.

The container clones the repository into a volume of its own, then looks at origin/main every 2 minutes. Nightly takes 20–30 minutes, so each run takes in the commits that landed during the last one, and the bisect names the one that broke a stage. When a pull changes `tools/ci`, it exits and Docker starts it on the new code.

- `docker compose -f ~/terra-ci/compose.yaml logs -f` shows it.
- `cargo xtask ci --watch` does the same in a checkout without Docker; `cargo xtask ci --dry-run` runs once and prints what it would file.

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
- **No GPU:** nightly needs none. With an NVIDIA card, the render worker runs beside it and takes agents' renders when nightly is not running ([render-worker.md](render-worker.md), D445).
- **Credentials:** the token is the only one, kept in `~/terra-ci/.env` on the box and never in the repository. It runs main's code, as anyone with push access can already make it do.

## Next

- **Benchmarks:** `cargo xtask bench` after a passing nightly, against the box's own last results, which needs bench to write its results outside `research/benchmarks.csv`. Also `cargo xtask rebuilds` daily, and `filmcheck` once it exists.
- **Triage by an agent:** a bounded `claude -p` triage of each new issue (D400), commenting the cause and never landing. It waits until the issues show the need.
