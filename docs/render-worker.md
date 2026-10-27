# The render worker: a GPU box that takes render jobs over SSH

Renders wait hours on the developer's one 10 GB card (D319), and rounds lose most of their time there. The render worker is a second machine with a bigger card that takes those runs from agents: an agent names a commit and a command, and gets the files the command wrote back in a local folder (D445, [tools/render-worker](../tools/render-worker/README.md)).

```bash
cargo xtask remote-render -- cargo xtask sandbox --offscreen --experiment play --shot ~/.cache/terra-review/a.png
cargo xtask remote-render --rev <sha> -- cargo xtask trailer --cut tools/sandbox/trailer/cut.json
cargo xtask remote-render --dirty -- cargo test -p terra_build probe -- --nocapture   # uncommitted changes, as a patch
cargo xtask remote-render --status
```

It prints where the files landed (`~/.cache/terra-review/remote/<job>/`) and exits with the command's code. Start it with `run_in_background` and wait for its completion notice, as for `cargo xtask gpu` (D319, D400); several at once run side by side ([slots](#when-it-runs)). With `TERRA_REMOTE_CHECKS=1` the push hook runs its checks there too ([development.md](development.md)).

## How a job flows

Everything goes over ssh to the hosts in `TERRA_WORKERS` (`.env`). Nothing on the box listens on a port.

1. **Push.** The client pushes the commit (HEAD, or `--rev`; it need not be on origin) to the box's clone, `<TERRA_WORKER_DIR>/repo`, as `refs/jobs/<id>`. Uncommitted changes stay behind and it says so, unless `--dirty` ships them ([below](#a-patch-of-uncommitted-changes)). Then it sends the Git LFS objects the commit points to, which a push carries only as pointers (D525): `rsync --ignore-existing` from this checkout's `.git/lfs/objects` into the clone's, so only those the box lacks travel and nothing is downloaded from GitHub; an object this machine lacks too is fetched from origin first (`git lfs fetch`). The clone's own git-lfs (`bootstrap.sh`) smudges its checkouts from that store.
2. **Queue.** It hands the job (commit, command, class, files to collect, time limit) to `box.ts submit` over ssh, which writes `jobs/<id>/job.json` and wakes the daemon. With several workers, the job goes to a running daemon with the fewest jobs ahead.
3. **Run.** The daemon starts each job in a free slot (below), the oldest first (after an update, its warm builds first), when the box's settings allow. In the slot's job checkout `slots/<n>/`, a git worktree of the clone whose `target/` is kept, it checks the job's commit out clean, runs `cargo xtask cook` and then the command at `nice 10` (a bench otherwise, below) under `timeout`, with `HOME` the slot's `homes/<n>/` and `CARGO_TARGET_DIR` the slot's `target/`, so caches carry from job to job, slots never wait on each other's cargo lock, and nothing mixes with the developer's. The environment holds no token and no ssh agent. A slot's build can go stale (an incremental release rlib missing a symbol a newer crate asks for), so a command that fails on a link error, or on rustc's E0460 or E0786, naming a workspace crate is cleaned of the crates the error names and the package that failed to compile, in the profile its paths show (`cargo clean --release -p …`), and run once more; a registry crate is cleaned too when its own rlib is corrupt (lld finds the rlib wanting a symbol it says that same rlib defines, as a build cut short leaves one: slot 4's bevy_reflect on 2026-10-27), by its Cargo.lock name, as build farms retry a cache's fault; a `render-worker: stale build` line in the job log and the job's note (shown when it ends) say so (`jobs.ts` `staleBuild`). An error in the code is never run twice.
4. **Collect.** What the command wrote under `~/.cache/terra-review/` while it ran (a film's `frames/` left out), or the `--collect` paths, goes into `jobs/<id>/out/` beside `job.log`.
5. **Wait and fetch.** The client runs `box.ts wait <id>` over ssh: the box watches the job's folder and prints a line when its state changes, so nothing polls across the network. A dropped connection is tried again. Then `rsync` brings `out/` into `~/.cache/terra-review/remote/<job>/`.

Jobs older than `TERRA_RENDER_KEEP_DAYS` (7) are deleted with their files and refs. `--no-wait` queues and prints the job, `--wait <job>` takes it up again, `--cancel <job>` drops a queued job or stops a running one, `--logs <job|daemon>` prints a log's end.

## A patch of uncommitted changes

A throwaway probe (a debug print, an A/B constant, a trace system) runs with `--dirty`, as a try server runs a patch that is never landed. The client snapshots the working tree as `git stash create` does: every change against HEAD, staged or not, and untracked files that are not ignored, added to a scratch copy of the index and committed with HEAD as parent by plumbing (`write-tree`, `commit-tree`), which runs no hook (`client.ts` `snapshot`). The snapshot is on no branch here; it goes only to the box's clone, as `refs/jobs/<id>-patch`, and expires with the job; origin and main's hooks never see it. Nothing passes it for a commit: the job's id, folder and files' folder end in `-patch`, `--status` lists it as `<base>+patch`, `job.log` opens with a `render-worker: PATCH` line and the diff's stat, `patch.diff` lies beside it, and the client's last line says it ran a patch of uncommitted changes on the base. The patch is always against HEAD (`--rev` with `--dirty` is refused). A throwaway branch or `--no-verify` is never needed; what lands is committed and checked as ever.

**A frame capture** (RenderDoc, fire rounds t11-t12) is such a patch: wgpu-hal hooks RenderDoc only with debug assertions (`[profile.release.package.wgpu-hal] debug-assertions = true`), and the patch calls `start_graphics_debugger_capture` and its stop around one frame. The command runs the sandbox under `timeout … env LD_PRELOAD=<renderdoc>/lib/librenderdoc.so ENABLE_VULKAN_RENDERDOC_CAPTURE=1 RENDERDOC_CAPFILE=<path> XDG_DATA_DIRS=<renderdoc>/xdg:…` (the env after `timeout`, as RenderDoc unhooks children; the layer's JSON on `XDG_DATA_DIRS`, the box's loader having no `VK_ADD_IMPLICIT_LAYER_PATH`), writing the capture outside the slot (about 2 GB, never collected). Replay scripts run headless as `xvfb-run -a qrenderdoc --python <script>` in a `--gpu` job, and need `~/.local/share/qrenderdoc/UI.config` with `"rdocConfigData": 1` and analytics opted out in the slot's own home (`HOME` is `homes/<n>/`), or a first-run dialog blocks them.

## The daemon

`tools/render-worker/daemon.ts`, run from the box's clone with the Node installed beside it. It is a systemd user service (`terra-render.service`) when the user's systemd lingers, so it runs without a login and starts at boot; where lingering cannot be turned on without sudo, a detached supervisor (its own session, so it outlives the ssh session) restarts it instead. Its log is `<TERRA_WORKER_DIR>/daemon.log`; `daemon.json` holds its pid, commit and last word, which `--status` shows.

**Self-update.** `cargo xtask remote-render --update` fetches origin/main, makes sure the box has its folder, clone, Node and git-lfs, set as the clone's filter (`bootstrap.sh`), pushes main as `refs/worker/main` with its LFS objects, and runs the new code's own `box.ts update` from an export of it: the clone is checked out at the new commit, the new daemon must pass `--selftest` or the clone goes back to the commit before, the warm builds are queued, one for each slot, and the daemon restarts at once, or once its running jobs end (it starts no new one meanwhile). The warm builds (`cargo build -p xtask -p terra_sandbox`, the release sandbox and clippy over the workspace) then run in every slot side by side before any other job, so the next render builds, and the push hook's remote check lints, only what its commit changed. An agent that finds the worker broken lands a fix on main and runs `--update`; the developer never needs to visit.

**Doctor.** `--doctor` prints a line each for the GPU (`nvidia-smi`), Vulkan (loader, NVIDIA ICD, the device when vulkan-tools is installed), Node, cargo, the libraries the sandbox builds against, ffmpeg, ImageMagick, Blender (which renderer it gets), free disk, the settings and the daemon, and under each failure what fixes it.

## When it runs

Settings of the box, in the `.env` of its clone (`<TERRA_WORKER_DIR>/repo/.env`, documented in `.env.example`):

- `TERRA_RENDER_WINDOW=22:00-08:00`: jobs start only between those hours on the box's clock. Empty (default) is any hour. A running job always finishes.
- `TERRA_RENDER_CI_LOCK`: a CI agent's lock file; while it is held no job starts (`TERRA_RENDER_AFTER_CI=0` ignores it).
- `TERRA_RENDER_KEEP_DAYS`: days a job is kept.
- `TERRA_RENDER_SLOTS` (4) and `TERRA_RENDER_GPU_SLOTS` (3): slots below.

**Slots.** The box (32 cores, 125 GB, an RTX 4090 with 24 GB) runs `TERRA_RENDER_SLOTS` jobs side by side, each in a slot of its own: a checkout, a build and a `HOME`. A job's class says what it takes. A **CPU** job (`cargo test`, `check`, `clippy`, `build`, `cargo xtask check`, the warm builds, or `--cpu`) takes any free slot. A **GPU** job (a trailer, the sandbox, any other command, or `--gpu`) also takes a share of the card, and at most `TERRA_RENDER_GPU_SLOTS` run at once, since the card holds about three films at 7 GB each; a CPU job queued behind a waiting GPU job passes it. A **bench** (`cargo xtask bench`, `cargo xtask gpu bench`) has the box alone, cores and card, since a build beside it moves its frame times (D185): it waits for every running job to end, and nothing queued behind it starts meanwhile, an update's warm builds neither. It runs at the normal priority, not `nice 10`, and on a hybrid processor pinned to the performance cores (`taskset -c`, the processors `lscpu` gives the higher top clocks, `jobs.ts` `performanceCores`; the daemon's first log line names them), so its main and render threads never land on an efficiency core, a 1.5x swing on the box's i9-14900K. `--status` shows each slot and its job. The planner is `plan` in `jobs.ts`, tested in `jobs.test.ts`.

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
