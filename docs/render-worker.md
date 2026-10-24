# The render worker: a GPU box that takes render jobs over SSH

Renders wait hours on the developer's one 10 GB card (D319), and rounds lose most of their time there. The render worker is a second machine with a bigger card that takes those runs from agents: an agent names a commit and a command, and gets the files the command wrote back in a local folder (D445, [tools/render-worker](../tools/render-worker/README.md)).

```bash
cargo xtask remote-render -- cargo xtask sandbox --offscreen --experiment play --shot ~/.cache/terra-review/a.png
cargo xtask remote-render --rev <sha> -- cargo xtask trailer --cut tools/sandbox/trailer/cut.json
cargo xtask remote-render --status
```

It prints where the files landed (`~/.cache/terra-review/remote/<job>/`) and exits with the command's code. Start it with `run_in_background` and wait for its completion notice, as for `cargo xtask gpu` (D319, D400); several at once run side by side ([slots](#when-it-runs)). With `TERRA_REMOTE_CHECKS=1` the push hook runs its checks there too ([development.md](development.md)).

## How a job flows

Everything goes over ssh to the hosts in `TERRA_WORKERS` (`.env`). Nothing on the box listens on a port.

1. **Push.** The client pushes the commit (HEAD, or `--rev`; it need not be on origin) to the box's clone, `<TERRA_WORKER_DIR>/repo`, as `refs/jobs/<id>`. Uncommitted changes stay behind; it says so.
2. **Queue.** It hands the job (commit, command, class, files to collect, time limit) to `box.ts submit` over ssh, which writes `jobs/<id>/job.json` and wakes the daemon. With several workers, the job goes to a running daemon with the fewest jobs ahead.
3. **Run.** The daemon starts each job in a free slot (below), the oldest first (after an update, its warm builds first), when the box's settings allow. In the slot's job checkout `slots/<n>/`, a git worktree of the clone whose `target/` is kept, it checks the job's commit out clean, runs `cargo xtask cook` and then the command at `nice 10` under `timeout`, with `HOME` the slot's `homes/<n>/` and `CARGO_TARGET_DIR` the slot's `target/`, so caches carry from job to job, slots never wait on each other's cargo lock, and nothing mixes with the developer's. The environment holds no token and no ssh agent.
4. **Collect.** What the command wrote under `~/.cache/terra-review/` while it ran (a film's `frames/` left out), or the `--collect` paths, goes into `jobs/<id>/out/` beside `job.log`.
5. **Wait and fetch.** The client runs `box.ts wait <id>` over ssh: the box watches the job's folder and prints a line when its state changes, so nothing polls across the network. A dropped connection is tried again. Then `rsync` brings `out/` into `~/.cache/terra-review/remote/<job>/`.

Jobs older than `TERRA_RENDER_KEEP_DAYS` (7) are deleted with their files and refs. `--no-wait` queues and prints the job, `--wait <job>` takes it up again, `--cancel <job>` drops a queued job or stops a running one, `--logs <job|daemon>` prints a log's end.

## The daemon

`tools/render-worker/daemon.ts`, run from the box's clone with the Node installed beside it. It is a systemd user service (`terra-render.service`) when the user's systemd lingers, so it runs without a login and starts at boot; where lingering cannot be turned on without sudo, a detached supervisor (its own session, so it outlives the ssh session) restarts it instead. Its log is `<TERRA_WORKER_DIR>/daemon.log`; `daemon.json` holds its pid, commit and last word, which `--status` shows.

**Self-update.** `cargo xtask remote-render --update` fetches origin/main, makes sure the box has its folder, clone and Node (`bootstrap.sh`), pushes main as `refs/worker/main`, and runs the new code's own `box.ts update` from an export of it: the clone is checked out at the new commit, the new daemon must pass `--selftest` or the clone goes back to the commit before, the warm builds are queued, one for each slot, and the daemon restarts at once, or once its running jobs end (it starts no new one meanwhile). The warm builds (`cargo build -p xtask -p terra_sandbox`, the release sandbox and clippy over the workspace) then run in every slot side by side before any other job, so the next render builds, and the push hook's remote check lints, only what its commit changed. An agent that finds the worker broken lands a fix on main and runs `--update`; the developer never needs to visit.

**Doctor.** `--doctor` prints a line each for the GPU (`nvidia-smi`), Vulkan (loader, NVIDIA ICD, the device when vulkan-tools is installed), Node, cargo, the libraries the sandbox builds against, ffmpeg, free disk, the settings and the daemon, and under each failure what fixes it.

## When it runs

Settings of the box, in the `.env` of its clone (`<TERRA_WORKER_DIR>/repo/.env`, documented in `.env.example`):

- `TERRA_RENDER_WINDOW=22:00-08:00`: jobs start only between those hours on the box's clock. Empty (default) is any hour. A running job always finishes.
- `TERRA_RENDER_CI_LOCK`: a CI agent's lock file; while it is held no job starts (`TERRA_RENDER_AFTER_CI=0` ignores it).
- `TERRA_RENDER_KEEP_DAYS`: days a job is kept.
- `TERRA_RENDER_SLOTS` (4) and `TERRA_RENDER_GPU_SLOTS` (3): slots below.

**Slots.** The box (32 cores, 125 GB, an RTX 4090 with 24 GB) runs `TERRA_RENDER_SLOTS` jobs side by side, each in a slot of its own: a checkout, a build and a `HOME`. A job's class says what it takes. A **CPU** job (`cargo test`, `check`, `clippy`, `build`, `cargo xtask check`, the warm builds, or `--cpu`) takes any free slot. A **GPU** job (a trailer, the sandbox, any other command, or `--gpu`) also takes a share of the card, and at most `TERRA_RENDER_GPU_SLOTS` run at once, since the card holds about three films at 7 GB each; a CPU job queued behind a waiting GPU job passes it. A **bench** (`cargo xtask bench`, `cargo xtask gpu bench`) has the box alone, cores and card, since a build beside it moves its frame times (D185): it waits for every running job to end, and nothing queued behind it starts meanwhile. `--status` shows each slot and its job. The planner is `plan` in `jobs.ts`, tested in `jobs.test.ts`.

## Security

- **Whoever can ssh in can queue**, and only they: the box's ssh keys are the access control. That is the same trust as a login, since a job runs code as that user.
- **No open port, no token.** The daemon listens on nothing and holds no credential; jobs reach it through files in its folder.
- **No secrets in a job.** The command runs without `*_TOKEN`, cookies, keys or the ssh agent in its environment, and with its own `HOME`. This guards against accidents, not against someone who can log in.

## Why this shape

- **SSH, not GitHub**: the developers want no token on the box yet (D445). ssh is already how they reach it, carries git and rsync, and lets an agent debug the box directly. The first shape, draft GitHub releases as queue and file store, is gone.
- **Native, not Docker**: the box has cargo; Docker would need the NVIDIA Container Toolkit (sudo) and Vulkan inside a container, and every update an image build. A native daemon updates by a checkout and a restart. Isolation would not change the trust: the job runs as the user either way.
- **Not a self-hosted GitHub Actions runner or an HTTP API**: a runner needs GitHub and is advised against on a public repository; an API needs a port and its own authentication, which ssh already gives.

## The box

Linux with an NVIDIA card, its driver, the Vulkan loader, git, curl, rsync, and the libraries the sandbox builds against; Rust through rustup in the user's home; the user able to ssh in with a key. Node is installed into the worker's folder by `--update`. Setup: [tools/render-worker](../tools/render-worker/README.md).
