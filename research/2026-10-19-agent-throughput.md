# Agent throughput: where the hours went

2026-10-19. The question, from the developer: "we have started moving very slowly and i dont know to what extent its resource contention vs. we need to work on our patterns". We measured the 48 hours from 2026-10-17 09:20 to 2026-10-19 09:20 PDT in the coordinator's session: 202 subagents, 217 agent-hours, 162 commits on main.

## Answer

Most of the slowness comes from how we use the machine, not from running out of it. Agents spent **62% of their hours waiting on GPU runs**, but the GPU was not the limit. It was busy about 27% of the time during a queue turn, the CPU stalled 7% of the uptime, and memory was never short. The waiting comes from one serial queue (`gpu.queue`) that every run went through. A still that draws in about 30 s waited 10 to 40 minutes behind 25-minute films and bakes. The other big losses also come from our patterns:

- rounds that hit the context limit before they landed (27% of hours);
- waits that never ended until the coordinator stopped them (about 10%);
- landing behind 20-minute push checks. That one is already fixed (D398, D37).

Builds, seeds and link times cost 2.5%, so a shared target or sccache would gain little.

## How it was measured

A Node script streamed every transcript (the session's subagent JSONL files, 1.8 GB, and the coordinator's, 238 MB) and attributed each gap between two records:

- a gap ending at the model's message counts as model time;
- a gap ending at a tool's result counts as that tool's time;
- a gap ending at a background task's notice counts as a wait on that task.

Commands were then sorted by what they ran or waited on: a polling loop on a film's log counts as a film wait. A background task's ID ties it to its command, including commands moved to the background at the 600 s limit (850 were). Tokens come from each message's `usage`, counted once per message. Landings are the `old..new HEAD -> main` lines in push output, counted with `git rev-list`. Scores come from the 63 `review*.md` files written in the window. Load and GPU figures come from `/proc/pressure`, `~/.cache/terra-review/perf-audit/` (`gpu.csv`, 232 nvidia-smi samples during a film's queue turn; `slow-tests.tsv`; `ff.log`), and 226 load samples in run logs.

What the figures don't show:

- Agent-hours overlap in wall time: on average 4.5 agents were active at once.
- No log records when a run took the queue, so its wait is an estimate (below).
- Landings found in push output (94 commits by 59 agents) undercount main's 162, because some push output was gone and other sessions land too.

## Where the hours went

| Where | Agent-hours | Share |
|---|---|---|
| **Waiting on GPU runs** (foreground, polling loops, background waits) | **134.7** | **62%** |
| · stills and probes | 56.7 | 26% |
| · films | 30.0 | 14% |
| · fire sheet | 18.6 | 9% |
| · bakes, Blender, Kimodo | 10.4 | 5% |
| · bench, and runs not identified | 19.0 | 9% |
| Model turns (thinking and writing) | 30.0 | 14% |
| Landing: commit, push hooks, pull, land scripts | 19.7 | 9% |
| Other waits on runs (VRAM probes, encodes, servers) | 16.3 | 8% |
| Builds, tests, checks, seeds | 5.4 | 2.5% |
| Reading and editing tools | 4.5 | 2% |
| Sleeps, subagent waits, prompts, short commands | 6.7 | 3% |
| **Total** | **217.3** | |

Cross-cutting:

- **Queueing inside the GPU wait.** Over the window, 134 agent-hours of GPU waiting fell in 42.4 hours of wall time: 3.2 agents waited at once on average, against 4.5 active. Runs mostly went one at a time through the queue, so at most about 42 of the 134 hours could have been GPU work. That puts the queueing at about 90 hours at most. The `flock gpu.queue` calls whose start and end we know put it at 45% of their turn, a lower bound. Two corroborations: a lake still ran 31 s from its turn to its exit (`ff.log`), and handoffs record still turns of 10 to 25 minutes. Our estimate: **60 to 90 agent-hours were spent queueing**.
- **Hung waits.**
  - 18 background waits never ended until the coordinator stopped them: 21.3 h.
  - 24 `until ! pgrep -f X` loops matched their own command line: 6.2 h. These overlap the 21.3 h.
  - In all, 815 polling loops on renders: 97.8 h of wall time and 1,196 wake-up turns.
- **Landing, by day:** 13.0 h on 10-17, when a push ran a 20 to 28 minute full check and often lost the race to main; 1.5 h on 10-18; 1.1 h on 10-19.

Outcomes:

| Agents | Count | Hours | Commits landed | Render calls |
|---|---|---|---|---|
| Workers that landed | 59 | 116.7 | 94 | 849 |
| Workers that landed nothing, ending past 200k context | 30 | 59.6 | 0 | 382 |
| Workers that landed nothing, otherwise (stopped, research, handed off) | 44 | 33.8 | 0 | 244 |
| Reviews | 56 | 6.7 | — | 39 |
| Helpers (finder, runner) | 13 | 0.5 | — | 1 |

- **Renders:** 1,515 render-related tool calls, 16 for each commit landed; 1.4 agent-hours of GPU waiting per commit.
- **Scores:** across the four filmed workstreams, 26 of 39 reviews did not raise the best score:
  - castle 4 → 7 over 14 reviews, four flat at 4 first;
  - settlement 5.5 → 6.5 over 12, with a dip to 5;
  - lake 5 → 8 over 9;
  - troll 4 → 6 over 8.
- **Context at the end:** a median of 190k across workers; 62 of 133 workers passed 200k, against the 120 to 150k hand-off their briefs set (RULES.md said 250k).
- **Duplicated agents:** we found only one agent overlapping another on the same brief, 0.1 h. Duplicated waits inside agents were common ("stacked 7 duplicate wait loops", LESSONS.md).

Tokens (subagents):

- 1.98 billion cache-read tokens, 30.6M written to cache, 6.6M output. The coordinator read 0.52 billion more, its context reaching 967k, with 8 compactions.
- 14,765 turns at a mean context of 136k. 41% of the turns followed a file read.

Machine:

- **CPU:** 24 threads. Over 36 h of uptime, `/proc/pressure/cpu` "some" totals 2.6 h (7%). Load samples have a median of 20 and a p90 of 82, peaking at 181 during building's test sweeps, which slow films sharing the CPU in those bursts (LESSONS: "don't render meanwhile").
- **Memory:** memory and IO pressure under 0.1%. 62 GB of RAM, 53 GB available.
- **GPU:** 27% mean use and 6.0 GB VRAM while a film held the queue. The limit was VRAM: films take 6 to 8 GB and a yard run 5.6 GiB of the 10 GB card. Out-of-memory crashes cost 25 to 50 minutes a time.

## The top five causes, by hours lost

1. **A serial GPU queue for every run (about 60 to 90 h).** From 10-17 12:30 the round's RULES.md sent every run through `flock gpu.queue`, against D319, which lets shots and films share. Stills waited behind films, bakes and fire sheets, and the GPU sat about 70% idle. Other losses:
   - `flock` is not fair: 14 Kimodo turns waited 40+ minutes;
   - a bench held the queue 50 minutes waiting for quiet;
   - a hung sandbox held it 27 minutes;
   - weather r3's film waited 4 hours.
2. **Rounds that end at the context limit before landing (59.6 h, 27%).** Thirty workers passed 200k and landed nothing. Handoffs give the same reason again and again: whole-file reads, surveys in the main thread, full-size stills read as images, three to five probes before the cause. The next round starts by reading again: 41% of turns follow a read.
3. **Hung and duplicated waits (about 21 to 27 h).** Waits on runs that had already died or never started; `pgrep -f` loops that match themselves; several waits on one thing. Each wake-up of a polling loop is a turn at about 170k of context.
4. **Landing behind long push checks (13 h on 10-17).** 20 to 28 minute checks on push, lost races to main, a remote that hung up after a passed check. Already fixed: D398 and D37 (2026-10-19) left the hooks formatting and linting only, and landing fell to about 1.3 h a day.
5. **Tuning loops without a gain (about 40 h, overlapping 1 and 2).** 26 of 39 reviews did not raise the best score. A round is several probes, a film and a review, about 1.6 agent-hours. Plateaus ran on: castle at 4 for four reviews; settlement 5 to 6 for nine. Probes found causes one at a time ("three ~12-min tinted probes ... all came back no").

## What to change

Each fix is marked as technical (ours to decide, D412) or as needing the developer.

1. **A GPU scheduler instead of one queue** (technical). Expected gain: 40 to 60 agent-hours per 48 h, and the GPU from about 27% to over 60% busy. Expect stills, probes and fire sheets to wait under 2 minutes instead of 10 to 40. What it does:
   - One command, `cargo xtask gpu <class> -- <cmd>`, run with `run_in_background`.
   - Each job declares its class (still, sheet, film, bake, bench) and its VRAM need; the table is measured: yard 5.6 GiB, film 6 to 8 GB, a still under 3 GB.
   - It admits jobs side by side while free VRAM allows (nvidia-smi), with short jobs ahead of films.
   - Each class has its own deadline, after which a run that writes nothing is killed.
   - It logs every wait and run, so the next audit is exact.
   - A bench still takes the GPU alone (D319).

   D188's compressed textures (reserved VRAM 8.6 to 6.8 GiB) make two runs fit at once. This replaces RULES.md's GPU sections and brings practice back in line with D319.
2. **A warm render server for stills** (technical, after 1). One sandbox per worktree stays up and takes stills and vantages by `cargo xtask remote`. Expected gain: modest, 5 to 10 h. A still's cold start is about 30 s, so the queue costs more than the start. It matters most for probe batches of 5 to 20 stills at one place, which would also stop holding the queue between shots.
3. **Context discipline, enforced** (technical). Expected gain: about 30 of the 59.6 h lost to rounds that never landed, and about a quarter of the tokens. Measures:
   - D390's nudge becomes a stop: at 150k the agent lands what it has and writes the handoff.
   - Land after each step, before the first film.
   - Briefs say "read only the handoff".
   - Surveys go to finder, measurements to runner (D391).
   - The review gets a small sheet and stills are read as 600 px crops.
   - One probe tints every suspect at once.
4. **Waits only by notice** (technical). Expected gain: 20 to 25 h and about 1,000 turns. Measures:
   - A PreToolUse hook refuses `until`/`while ... sleep`, `sleep N; echo` and `pgrep -f` waits, naming the fix.
   - The scheduler in 1 and every long tool exit with one line and a non-zero code on a stall.
   - The coordinator gets a list of each agent's open waits and their age, so hung ones are found in minutes, not hours.
5. **Plateau rules for look rounds** (needs the developer: the score bar and when to stop are design calls, D412). Expected gain: 15 to 25 h. Proposed rules:
   - Two reviews without a gain end the workstream's tuning; the next round steps back (AGENTS.md "step back on a second attempt") with a measured comparison against the reference before any film.
   - Films only after the stills meet the numbers.
   - The developer sets the bar for each trailer scene (now 7.5/10) and how many rounds a scene may take.
6. **Keep landing cheap** (technical, done). D398 and D37 hold. We recommend no shared target or sccache: builds are 2.5% of the hours, and [worktree-builds.md](worktree-builds.md) found sccache saved nothing. Rebuild the release binary once after each pull, outside the GPU turn.

Doing 1, 3 and 4 would return up to about 100 of the 217 agent-hours in a 48-hour window (their gains overlap). At the same number of agents, that is close to doubling the output.
