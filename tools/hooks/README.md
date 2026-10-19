# tools/hooks

Claude Code hooks that need logic (the thin ones are commands in `.claude/settings.json`).

- `wait-loops.ts`: PreToolUse on Bash (D400). Refuses an `until` or `while` loop that polls with `sleep`, or whose condition runs `pgrep` (it finds the loop's own command line and never ends), also inside `bash -c '...'`. Exit 2 with a one-line reason on stderr, which names the way out: the background task's completion notice, or `tail --pid=<pid> -f /dev/null` with a pid from `$!` or `cargo xtask gpu`. Heredoc bodies and quoted strings (a commit message, an `echo`) are not code. Node 22.18+, no dependencies; `node --test tools/hooks/wait-loops.test.ts`.
