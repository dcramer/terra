# tools/continuous

The continuous checks on main (D37, D398): the heavy checks that no hook runs and nothing waits on before landing (GPU renders, the fire sheet, look checks, benches, long sweeps), run on the GPU machine whenever origin/main moves.

```bash
cargo xtask continuous                  # once: every check due at origin/main's tip
cargo xtask continuous --watch [min]    # the same every so many minutes (default 5), for a coordinator
cargo xtask continuous --install [min]  # a systemd user timer that runs it every so many minutes
```

- **A check is a row of `checks.json`**: `name`, `does`, the command (`run`, which passes when it exits 0), `reads` (the name of a list in `xtask/src/scope.rs`, such as `FIRE`; left out, it runs whenever main moves), `minutes` (stopped after, default 60) and `hours` (run again after so long although nothing changed). A new heavy check is a row here, not a hook stage.
- **Where it runs**: a worktree of its own detached at main's tip, `.git/terra-continuous/src`, made and seeded the first time, reset and cleaned (its build kept) each run, at `nice 10`. Checks run one after another; a run while one is going says so and does nothing.
- **What it says**: a line a check that ran. `.git/terra-continuous/status.txt` is a line a check at the last commit it ran; a failure after a pass names the range from the last passing commit and the commits in it touching what the check reads. `history.log` keeps every verdict, `logs/<name>.log` the last run's output, `records.json` what each check last saw.
- **The timer**: `--install` writes `terra-continuous.service` and `.timer` in `~/.config/systemd/user`, running `cargo xtask continuous` from the main checkout every so many minutes after the last run ended, and enables it. `systemctl --user list-timers terra-continuous.timer` shows it, `journalctl --user -u terra-continuous` its lines, `systemctl --user disable --now terra-continuous.timer` stops it.

Today it holds one check: the fire sheet's gate, `cargo xtask fire --gate` against `tools/fire/held.json` (D415), which takes the GPU through its own queue. `continuous.ts` is the tool: its decisions (`due`, `verdict`) and reading `scope.rs` are pure functions, tested in `continuous.test.ts`. Node 22.18 or later, git and cargo; no dependencies.
