# The render worker: a GPU box that takes render jobs

Renders wait hours on the developer's one 10 GB card (D319), and rounds lose most of their time there. The render worker is a second machine with a bigger card that takes those runs from agents: an agent pushes its commit, queues a job (the commit and the command), and gets the job's files back in a local folder (D445, [tools/render-worker](../tools/render-worker/README.md)).

```bash
cargo xtask remote-render --rev <sha> -- cargo xtask trailer --cut tools/sandbox/trailer/cut.json
cargo xtask remote-render -- cargo xtask sandbox --offscreen --experiment play --shot ~/.cache/terra-review/a.png
cargo xtask remote-render --status
```

It prints where the files landed (`~/.cache/terra-review/remote/<job>/`) and exits with the command's code. Start it with `run_in_background` and wait for its completion notice, as for `cargo xtask gpu` (D319, D400).

## How a job flows

1. **Queue.** The client checks the commit is on a branch of origin (push first), then makes a **draft GitHub release** `render-<job>` whose notes hold the job: commit, command, files to collect, time limit, state `queued`.
2. **Take.** The worker, a container on the box beside the CI agent's (docs/ci-agent.md), lists draft releases every 30 s. When the policy allows (below) it takes the oldest queued job, marks it `running`, and checks the commit out in its own checkout, keeping its `target/` so a job builds only what changed. It runs `cargo xtask cook`, then the command, at `nice 10`, with a fresh `HOME` whose `.cache/terra` is the box's shared cache.
3. **Collect.** What the command wrote under `~` (`~/.cache/terra-review/...`, where films, sheets and shots go) and any `--collect` paths of the checkout are packed into `files.tar` and uploaded to the release with `job.log`; the state becomes `done` or `failed` with the exit code.
4. **Fetch.** The client looks at the job every 15 s, then less often, up to a minute, and prints a line only when its state changes. Once it ends it downloads and unpacks the files and prints the folder.

Jobs older than 7 days are deleted with their files. `--no-wait` queues and prints the job, `--wait <job>` takes it up again, `--cancel <job>` deletes a job not yet started.

## When it runs

The box runs CI first (the developers: "in the off times when the main CI/CD isn't running"). Two settings in the box's `.env`:

- `TERRA_RENDER_AFTER_CI=1` (default): a job starts only while nightly is not running (the CI agent's lock). Nightly may start while a job runs; both are niced and share the box.
- `TERRA_RENDER_WINDOW=22:00-08:00`: jobs start only between those hours, in the box's `TZ`. Empty (default) is any hour. A running job always finishes.

One job runs at a time, the whole card its own.

## Security

The repository is public, so the box never runs what someone outside it chose:

- **Only people with push access can queue.** A draft release can be made and seen only by them. Its files are not public either.
- **Only pushed commits run.** The worker fetches origin's branches alone and refuses a commit none of them contains, since GitHub serves a fork's commits by hash through the parent repository.
- **No token in the job.** The command runs without `GH_TOKEN` or any other secret in its environment. The token, the same `public_repo` one the CI agent holds, stays in `~/terra-ci/.env` on the box. This guards against accidents, not against a pusher: whoever can push can make the box run code, as with the CI agent.
- **No open ports.** The worker only pulls, as the CI agent does.

## Why this shape

- **Not a self-hosted GitHub Actions runner**, though `workflow_dispatch` with `gh run download` would be the least code. GitHub advises against self-hosted runners on public repositories, since a pull request's workflow can reach the machine, and D411 and D37 keep Actions off it. Holding back for CI would also mean stopping the runner service around nightly.
- **Not an HTTP API on the LAN**: it needs an open port, its own authentication and a way for agents to find the box, and works only at home.
- **Draft releases** give the queue, its access control and the file store in one place, with the token the box already has and the `gh` agents already use. Issues would be public and need their own check of who wrote them; a branch of job files would have every agent racing to push to it. A release file may be 2 GiB; a job whose files exceed that fails with a line saying so.

## The box

The CI agent's box (docs/ci-agent.md) with an NVIDIA card: the driver, Docker and the NVIDIA Container Toolkit. The worker's container is the CI agent's toolchain with the Vulkan loader, ffmpeg and the libraries Bevy loads; the toolkit puts the driver in. Setup is one command; [tools/render-worker](../tools/render-worker/README.md) lists what must be done by hand first.
