# tools/render-worker

The render worker ([docs/render-worker.md](../../docs/render-worker.md), D445): agents run renders on a GPU box over ssh, at a commit of their checkout, and get back what they wrote.

```bash
cargo xtask remote-render -- cargo xtask sandbox --offscreen --experiment play --shot ~/.cache/terra-review/a.png
cargo xtask remote-render --rev 1a2b3c4 --collect shot.png -- cargo xtask sandbox --offscreen --experiment play --shot shot.png
cargo xtask remote-render --no-wait -- cargo xtask bench   # queue, print the job, return
cargo xtask remote-render --cpu -- node tools/x.ts         # a job that never draws: any free slot
cargo xtask remote-render --dirty -- cargo test -p x probe  # the working tree's uncommitted changes, as a patch, no hooks
cargo xtask remote-render --dirty -- sh ab.sh $(git rev-parse origin/main)  # an A/B: main by its sha (below)
cargo xtask remote-render --wait <job>                     # wait for it and fetch its files
cargo xtask remote-render --status                         # each box's daemon and jobs
cargo xtask remote-render --logs <job|daemon>              # a log's last lines
cargo xtask remote-render --cancel <job>                   # drop a queued job, stop a running one
cargo xtask remote-render --update                         # bring the boxes to origin/main and restart their daemons
cargo xtask remote-render --doctor                         # what each box lacks, and the fix
```

- **Files back**: what the command wrote under `~/.cache/terra-review/` while it ran (a film's `frames/` left out), or the `--collect` paths instead (in the checkout, or `~/…`; one path a `--collect`, repeated for more, a comma list refused; a path not there when the job ends is named in the log and the job's last line), in `~/.cache/terra-review/remote/<job>/` beside `job.log`. Output: a line when the job's state changes, the folder at the end; on a failure the log's last lines. Exit code: the command's (124 at its `--minutes` limit).
- **An A/B on the box names main by its sha**: a `--dirty` job's HEAD there is the patched commit (its parent your local HEAD, which need not be main), so a script that builds or checks out main beside the patch takes main's sha as an argument (`git rev-parse origin/main` here), never `HEAD`, `HEAD~1` or `main` ([docs](../../docs/render-worker.md)).
- **Files**: `client.ts` is the xtask; `box.ts` the box's commands, which the client runs over ssh; `daemon.ts` the daemon that runs jobs; `display.ts` the Xvfb it keeps for them; `doctor.ts` the box's checks; `bootstrap.sh` makes a box's folder and installs Node there; `jobs.ts` the decisions as pure functions (`jobs.test.ts`). `flow.test.ts` runs the whole path with a scratch folder for a box (`TERRA_WORKERS=local`, no ssh) in about two seconds. Node 22.18 or later, git, ssh, rsync; no dependencies.

## Settings

Every setting is in [`.env.example`](../../.env.example) with its default. The client reads the `.env` at the root of the checkout (a worktree falls back to the main checkout's); the daemon reads the `.env` in its clone on the box, `<TERRA_WORKER_DIR>/repo/.env`. Real environment variables override either. Neither file is committed.

| Variable | Read by | Default | What |
| --- | --- | --- | --- |
| `TERRA_WORKERS` | client | none | The boxes: a comma list of ssh hosts (aliases from `~/.ssh/config`, or `user@host`). A job goes to a running daemon with the fewest jobs ahead; `--worker HOST` picks one. |
| `TERRA_WORKER_DIR` | client | `terra-render` | The worker's folder on each box, relative to the home there. |
| `TERRA_RENDER_MINUTES` | client | 120 | A job's time limit when `--minutes` is not given (at most 480). |
| `TERRA_RENDER_CONNECT_TIMEOUT` | client | 10 | Seconds ssh waits to reach a box. |
| `TERRA_RENDER_WINDOW` | box | any hour | Jobs start only between these hours on the box's clock, e.g. `22:00-08:00`. |
| `TERRA_RENDER_CI_LOCK`, `TERRA_RENDER_AFTER_CI` | box | none, 1 | A CI agent's lock file; while held, no job starts unless `TERRA_RENDER_AFTER_CI=0`. |
| `TERRA_RENDER_KEEP_DAYS` | box | 7 | Days a job's files and ref are kept. |
| `TERRA_RENDER_SLOTS` | box | 4 | Jobs at once, each in a slot: a worktree checkout `slots/<n>`, its build, and `homes/<n>` its HOME. |
| `TERRA_RENDER_GPU_SLOTS` | box | 3 | Of them, GPU jobs at once; a bench builds in a slot while the box drains, then has the box alone for its timed run. A job's class comes from its command, or `--gpu`/`--cpu`. |
| `TERRA_RENDER_DISPLAY` | box | auto | The X display a job that draws gets (`:1`). Empty: the daemon's own `WAYLAND_DISPLAY`/`DISPLAY`, else one Xvfb the daemon starts on a free number from `:99` when a job first needs it, starts again if it dies and stops when it exits (`display.ts`). |
| `TERRA_REMOTE_CHECKS` | push hook | 0 | 1 runs the push hook's check on the worker at each commit pushed (`tools/git/remote-check.ts`). |

## Setting up a box

The box: Linux with an NVIDIA card and its driver (`nvidia-smi` prints the card), git, curl, rsync, Rust through [rustup](https://rustup.rs) in the user's home, and the libraries the sandbox builds against (`sudo apt-get install -y pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`; `ffmpeg` too for films). For Blender jobs, blender.org's tarball of the pinned version unpacked in `~/opt` ([tools/blender](../blender/README.md)); for sheets, ImageMagick 7's AppImage extracted into `~/opt` with its `AppRun` linked as `~/.local/bin/magick`, which is on the daemon's PATH. Neither needs sudo. You must reach it with `ssh <host>` and a key, with no password prompt.

1. `cp .env.example .env` in your checkout, and set `TERRA_WORKERS` to the box's ssh host.
2. `cargo xtask remote-render --update`. It makes `~/terra-render` on the box (no sudo): the clone pushes land in, **Node 22 LTS from nodejs.org's official tarball** in `~/terra-render/node` (its checksum checked), the job checkout and the jobs. It pushes origin/main (the first push sends the whole history, a few minutes), starts the daemon as a systemd user service (`terra-render.service`, with lingering) or else a detached supervisor, and queues the warm builds, which take a while the first time.
3. `cargo xtask remote-render --doctor` says what is still missing.
4. Optional, on the box: `~/terra-render/repo/.env` with the box's settings above, then `--update` again to restart the daemon on them.

The worker never touches any other checkout on the box. Disk: the clone, and for each slot a build and caches of tens of GB.

**Troubleshooting**

- **`nvidia-smi` fails after a kernel update**: the driver's prebuilt kernel module exists only for older kernels. `--doctor` prints the fix; on Ubuntu with the 535 driver it is `sudo apt-get update && sudo apt-get install -y linux-modules-nvidia-535-generic linux-generic`, then `sudo reboot`.
- **The daemon is down**: `--logs daemon` says why; `--update` starts it again. On the box: `systemctl --user status terra-render`.
- **A sandbox job panics with "neither WAYLAND_DISPLAY nor DISPLAY is set"**: winit's event loop needs a display even for `--offscreen`, and a systemd user service has none. The daemon gives jobs its own Xvfb then; `--doctor`'s `display:` line says which display jobs get, and the job log's `render-worker: display:` line which one it got. If Xvfb is missing: `sudo apt-get install -y xvfb`. The NVIDIA driver draws headless under Vulkan, so the card still renders (the log's adapter line names the GPU, not llvmpipe).
- **rustc panics** ("the compiler unexpectedly panicked", an ICE): in `rustc_incremental` (`SerializedDepGraph::decode`) the slot's incremental cache is corrupt, not the code; in a query (slot 1's `is_copy_raw` over `bevy_ptr::MovingPtr<…pooling::Mote>` on 2026-10-29) rustc's incremental state can be at fault too. On either the daemon removes the failed crate's sessions and runs the command once more with `CARGO_INCREMENTAL=0` (`render-worker: rustc crashed …` in the log); nothing to clear by hand. If it crashes again without incremental (the note says so), it is a rustc bug the code hits: work round it in the code or on another toolchain. Slot 3's four crashes on 2026-10-28 (terra_folk, 23:46 to 23:54Z) came from one corrupt session, not from the slot: its target is its own (rustc's `-C incremental` was `slots/3/target/release/incremental`), on the same ext4 disk as the other slots with 100 GB free and no write error in any log, and no job of its was cut short since 10-27.
- **A job waits**: `--status` gives the daemon's last word, such as "outside the window".
