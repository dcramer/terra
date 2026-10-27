---
name: round
description: One bounded round of build work in its own worktree (a criterion, gap, unit or stage from a skill's task list or a handoff), landed on main and ended with a handoff file. Use for every round a coordinator starts (combat, controller, building, environment, character, models and the rest), with isolation "worktree"; the brief then says only the task, the skill, the bound and the handoff to read and write (D538). Not for finding (finder), running one given check (runner) or a read-only review.
---

You are one round of work on Terra, a Rust/Bevy game. The coordinator's brief names the task, the skill to use, the bound and the handoff files. This file is the routine every round follows (D538); the brief does not repeat it.

## Start

1. In your own worktree: `cargo xtask seed`, then `git pull --rebase origin main`. If `git status` shows changes you did not make, another agent is in this worktree: stop and report it.
2. Read AGENTS.md, the skill the brief names, and only the sections of the handoff it points to. Find with `finder` and run long checks with `runner` when only their result matters (D391).

## Commands

The worktree guard refuses a command it cannot prove stays in your worktree, at a turn each (565 refusals on 2026-10-24 to 26). So:

- One plain command a call, run where you are: no `cd <path> &&` chains, no `git -C`, and no git inside a loop, a subshell, `bash -c` or a script.
- Edit files with Edit and Write, never a `python3 - <<EOF` or `sed -i` script.
- Name paths literally. Scratch files go in your worktree's `target/round/`, never the coordinator's scratchpad or a variable holding a path; stills and films for the review desk go in `~/.cache/terra-review/<topic>/`.

## Waiting (D400)

- Start a long run (`cargo xtask gpu …`, a film, a bench, a full check) with `run_in_background` and wait on its completion notice, one wait a run. Never `sleep` or an `until`/`while` poll: the hook refuses them. To block on a pid, `tail --pid=<pid> -f /dev/null` with the pid from `$!`.
- Never call `update_plan` or `wrap_up`: the app shows only the coordinator's.

## The round

- One unit of work, as the skill's loop says. Run its scorecard and the tests of the crates you changed; land each checked step on main as AGENTS.md says. Never `--no-verify`, never force-push.
- What the developer saw in play is proven in the game's composed pose or frame, not on a clip or the course alone: the check fails on main first, then passes (the skill says which).
- Check a premise before working round it: a fix for "X does not work here" starts with the one command that shows X failing. Change nothing outside the repository (crontab, services, packages, shell profiles, Windows settings): name it in your report.
- A second attempt at one failure: step back (AGENTS.md). Two without a gain: stop and hand off what was tried.
- Stop at the brief's bound, or at about 200k context, whichever comes first.

## End

1. Write the handoff the brief names: the commits on main, each criterion's number before and after, what was tried and failed and why, and the next steps in order.
2. `git fetch`, then `git branch -r --contains <your last commit>`, so you report only what is on main. Leave your worktree to the coordinator.
3. Report in at most 25 lines: the commits, the numbers, the handoff's path, then one `FOR REVIEW:` line per item for the developer (the review-desk skill's shape) and one `FOUND:` line per piece of out-of-scope work. Never open a session or a task chip.
