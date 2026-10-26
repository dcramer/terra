# tools/gpu

The GPU wrapper (D319): every unattended GPU run an agent starts goes through it. Shots, sheets, films and bakes start at once and share the card side by side; only a benchmark has the card alone. There is no queue and no memory admission.

```bash
cargo xtask gpu still -- target/release/terra_sandbox --offscreen ... --shot a.png > run.log 2>&1
cargo xtask gpu film --watch out/render.log -- node tools/sandbox/trailer.ts --cut ... > film.log 2>&1
cargo xtask gpu bench -- cargo xtask bench walk
cargo xtask gpu --status
```

Start it with Bash's `run_in_background` and wait for its completion notice.

- **Classes**: `still` (a few stills or vantages), `sheet` (a sheet at a small place: the yard, the fire sheet, a workbench), `film` (a film or a long probe at a big place), `bake` (a bake-off or sweep over several scenes), `bench` (a benchmark). Only `bench` is treated apart; the rest differ by their stall limit (`CLASSES` in `plan.ts`).
- **Shared runs** (`still`, `sheet`, `film`, `bake`) run the command at once under `flock -s ~/.cache/terra/gpu.lock` (`TERRA_GPU_LOCK`), held for the run's life. They wait only while a benchmark wants the card.
- **Benchmarks** leave a want-file, `gpu.want/<pid>` beside the lock, then run under `flock -x`: they wait for the shared runs holding the lock to end, run, and remove the want-file. While one is there, new shared runs hold back, so a bench is never starved. A process waiting for the lock alone by other means (the game's own bench, `flock -x` by hand) holds them back too. The game finds the turn held by a forebear and waits for neither the lock nor a slot (`terra_world::gpu_turn`).
- **Stalls**: a run is stopped (TERM, then KILL after 10 s) and exits 124, with the reason printed, only when, holding the lock, its whole process tree has made no progress for its class's stall limit: 5 min for a still, 8 for a sheet and 10 for the rest. Waiting for the lock is not a stall. Progress is any of: output on its stdout or stderr, growth of a `--watch FILE`, its CPU time growing (utime and stime from `/proc/<pid>/stat`, reaped children's included, or `ps -A -S` where there is no /proc, as on macOS, summed over its processes, at least 5% of a core over a minute), or one of its processes busy on the card's cores (`nvidia-smi pmon`). So a silent run that renders or computes is left alone, and a hung one, which uses nothing, is stopped at the limit. Under WSL a Windows program's CPU and GPU use are not seen from Linux, so it is judged by its output and `--watch` alone. `--stall MIN` changes the limit. `--vram MB`, the old scheduler's memory request, is accepted and ignored.
- **What it says**, on stderr with a `gpu:` prefix: a line if it waits for a benchmark (and every 5 minutes after), a line when it starts after waiting, a line when it ends. It exits with the command's code.
- **`--status`**, for the coordinator: the card's memory, each run (running or waiting, its age, time since its last progress, pid, worktree, command), whether a benchmark wants the card, and whoever else holds or waits for `gpu.lock` (`outside`).
- **State**: a file a run in `~/.cache/terra/gpu/runs/` (`TERRA_GPU_DIR`), with its wrapper's pid and its command's. A run whose processes are all gone is removed by the next reader, and a want-file whose wrapper died likewise.
- **Nesting**: a job's command runs with `TERRA_GPU_JOB` set. A `cargo xtask gpu` inside it runs its command at once, since it already has the turn.
- **Tests**: `node --test tools/gpu/*.test.ts`. `gpu.test.ts` runs real processes on their own state directory and lock: two stills start at once, a bench waits for a running still and holds back one asked for meanwhile, a silent busy loop survives and a silent sleep is stopped.

`plan.ts` holds the pure parts: the classes, reading nvidia-smi, /proc/locks and /proc stat lines, and the progress rule, tested in `plan.test.ts` without a GPU. `gpu.ts` takes the lock, runs the job and keeps the state. It needs Node 22.18 or later and `flock` (util-linux); without `flock`, as on macOS, it runs the command with no lock and the game takes its own turn (`gpu_turn`); the lock-order test is skipped there. `nvidia-smi` is optional.

`flock ~/.cache/terra/gpu.queue` is a dead path: nothing takes it. Replace it with `cargo xtask gpu <class> --`.
