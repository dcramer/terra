# memlimit

Cargo's runner on Linux, set in `.cargo/config.toml`: every binary Cargo starts (tests, benches,
`cargo run`) runs under a cap on its heap, 32G unless `TERRA_MEM_LIMIT` says otherwise
(`TERRA_MEM_LIMIT=0` lifts it).

Without it, one runaway test fills WSL's memory and the kernel's out-of-memory kill takes the
whole distro down: every shell, agent and build in it. A `terra_world` test did, twice in a day,
at 89 GB. Under the cap the allocation fails in that binary alone, which aborts with
"memory allocation of N bytes failed".

The cap is `RLIMIT_DATA` (`ulimit -d`), the process's own memory, not a cgroup: WSLg mounts over
`/run/user/1000`, which hides the user manager's bus from `systemd-run --user`.

Worktrees and checkouts under this folder (`.claude/worktrees/*`, `.git/terra-*/src`) take the
runner from this checkout's config as well as their own, so a branch from before it is capped too.
