# tools/gpu

The GPU scheduler (D319): every unattended GPU run an agent starts goes through it, so runs share the card's 10 GB side by side while they fit, short ones go first, and a benchmark has the card alone.

```bash
cargo xtask gpu still -- target/release/terra_sandbox --offscreen ... --shot a.png > run.log 2>&1
cargo xtask gpu film --watch out/render.log -- node tools/sandbox/trailer.ts --cut ... > film.log 2>&1
cargo xtask gpu bench -- cargo xtask bench walk
cargo xtask gpu --status
```

Start it with Bash's `run_in_background` and wait for its completion notice.

- **Classes**: `still` (a few stills or vantages), `sheet` (a sheet at a small place: the yard, the fire sheet, a workbench), `film` (a film or a long probe at a big place), `bake` (a bake-off or sweep over several scenes), `bench` (a benchmark). Each has a memory estimate, a stall limit and an order in the queue (`CLASSES` in `plan.ts`).
- **Admission**: a job starts when its estimate fits: the card's memory, less what other processes hold (nvidia-smi, read live), a 400 MiB margin, and what every running job asked for or holds, whichever is more. A job that has just started holds little yet, but it is counted at its estimate. At most four run at once.
- **Order**: still first, then sheet and bench, then film and bake. Each 5 minutes of waiting is worth one step, so a film waiting 15 minutes is ahead of a new still. A job that does not fit is passed over, and smaller jobs behind it fill the gap. Once it has waited its class's drain time, it holds back those behind it until the card frees for it: 5 minutes for a bench, 10 for stills and sheets, 15 for films and bakes. This is backfill, as Slurm schedules a cluster.
- **Benchmarks** start only on an empty card, and nothing starts beside one. The job runs under `flock -x ~/.cache/terra/gpu.lock`. Every other job runs under `flock -s`. So a benchmark started outside the scheduler still waits for the shared jobs, and they for it. The game finds the turn held by a forebear and waits for neither the lock nor a slot (`terra_world::gpu_turn`).
- **Estimates are learned**: each run's peak on the card is recorded, summed over its processes by pid every 2 s. Once three runs of a class are known, the estimate is the 90th percentile of its last 20 peaks with a tenth on top. Until then it is the table's figure, measured with cooked textures (D188): a still 3,000 MiB, a sheet 5,000, a film or bake 7,900. `--vram MB` overrides it for one job known to be heavier.
- **Stalls**: a run that writes nothing for its class's stall limit is stopped (TERM, then KILL after 10 s) and exits 124, with the reason printed. The limit is 5 min for a still, 8 for a sheet and 10 for the rest. Output counts as the run's own stdout and stderr, and the growth of any `--watch FILE`, for a run that writes its log itself. `--stall MIN` changes the limit.
- **Retry**: a run that fails with the device lost or out of memory (its output says so) goes back in the queue once, in its old place. Its estimate is then a quarter over its peak, so it waits for more free memory.
- **What it says**, on stderr with a `gpu:` prefix: on start, its place in the queue, the expected wait (the queue played forward at each class's usual run time) and what is running; every 5 minutes while it waits; a line when it starts; a line when it ends, with its time and peak. It exits with the command's code.
- **`--status`**, for the coordinator: the card, then each running job (class, age, memory held and asked, time since its last output, pid, worktree, command), then each waiting job (place, wait so far, expected start). Runs outside the scheduler show as `outside`: whoever holds `gpu.queue` (counted as a film), and whoever holds `gpu.lock` (alone: a benchmark; shared: a still).
- **State** survives crashes. It lives in `~/.cache/terra/gpu/` (`TERRA_GPU_DIR`): a file per job in `jobs/` with its scheduler's pid and its command's. A job whose processes are all gone is removed by the next reader. Each admission happens inside `admit.lock`, a directory made atomically, broken if its maker died or it is older than 30 s. `history.jsonl` holds a line per finished run: class, command, worktree, wait, run time, peak, exit code, and why it failed.
- **Nesting**: a job's command runs with `TERRA_GPU_JOB` set. A `cargo xtask gpu` inside it runs its command at once, since it already has the turn.

## The old queue

`flock ~/.cache/terra/gpu.queue <cmd>` is a legacy path, documented, not taken. The scheduler does not take that lock. Taken exclusively, it would make every job serial again. Taken shared, it would starve old callers behind a steady stream of stills. Instead the scheduler counts whoever holds `gpu.queue` as a running film, at the film estimate or what its processes hold, whichever is more. Nothing is overcommitted while old scripts move over, and old scripts still queue among themselves. Move a script by replacing `flock ~/.cache/terra/gpu.queue` with `cargo xtask gpu <class> --`. Never wrap a job in `flock gpu.queue` as well.

`plan.ts` holds the decisions: order, admission, expected starts, estimates, and reading nvidia-smi and /proc/locks. They are pure functions, tested in `plan.test.ts` without a GPU. `gpu.ts` reads the machine, keeps the state and runs the job. It needs Node 22.18 or later, `flock` (util-linux) and `nvidia-smi`, and no other dependencies. Without nvidia-smi it counts the card as `TERRA_GPU_VRAM_MB` (10240) and counts requests only.
