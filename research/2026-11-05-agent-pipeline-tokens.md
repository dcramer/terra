# What it costs an agent to drive the character pipeline, and how to make it cheap

2026-11-05. The developer: "the efficiency is CRITICAL here so we can design characters that are great. people online have figured this out with agents, you must too", and "really i need a toolchain/pipeline that is token efficient for all this work and i dont know that we have it yet".

[character-pipeline](2026-11-02-character-pipeline.md) covers what the pipeline does. This note covers what it costs an agent to drive it: how people online keep 3D and character agent work cheap, what our last five character jobs spent and on what, and the changes in order of tokens saved per unit of work. A separate job (char-pipeline j3) is measuring the tally for adding one outfit. This note does not redo that.

Grades on sources: **[F]** first-hand account; **[M]** measured by the author; **[V]** vendor or vendor-adjacent; **[P]** primary documentation; **[S]** secondary.

## In plain words

- **A character job costs about 21 million input tokens, mostly the same context read again.** Over 144 turns on average, the context grows to 218k at its peak. Cost scales with turns times context size, so a token read early is paid on every later turn. At Opus 5.5 list prices that is about $8 to $9 a job: cache reads about half, cache writes a quarter, output the rest.
- **Before a job changes anything, it has already spent 41% of its bill.** The context at a job's first edit averages 111k, so 86k has been read on top of the fixed 25k prefix. Those reads are then carried through about 100 more turns.
- **The biggest single sink is reading our own source code to learn how it works:** 50k tokens a job on average, 40% of everything tools return. Next come the agent's own output kept in context (70k), the docs read to find out where things stand (29k), and searching (25k in 41 calls, more calls than any other kind).
- **Pictures and Blender output are not the problem.** A job looks at 2 to 7 stills, which cost 3k to 8k tokens (at most 6%). Blender runs and logs return about 500 tokens a call. Online advice to shrink screenshots is right in general, but it would save us little.
- **What people online report, measured:** results shrink when tools return verdicts and counts instead of dumps. Examples: Anthropic's concise mode, about a third of the tokens; Cursor writing long output to files, 46.9% fewer tokens; Frugal Blender MCP returning 202 tokens for 109 operations; context editing, 84% fewer tokens. Other levers are loading tool and skill detail only when it is needed (77k down to 8.7k), keeping a skill under 500 lines with references one level deep, and running scripts instead of reading them. Nobody published a first-hand character job with token numbers. The two first-hand Blender accounts both found that one long session drifts and degrades, and both recommend fresh sessions from a saved milestone.
- **Judging a look without a person works best as numbers first, then one paired comparison.** A VLM ranking multi-view renders pair by pair agreed with experts at Kendall's τ 0.71 (GPTEval3D). VLMs still miss subtle visual differences (BlenderGym). So our metric checks (`layers.ts`, `gear_check.py`, `intact.ts`) are the cheap judge, and a still is a single before/after pair. That is what we already do.
- **Recommendation:** give character work what combat and the controller have: a status command that prints, in a page, each criterion's number, the next tasks and the files that own them. Add a code map, so a job reads 40 lines instead of the log, handoff, decisions and research note, and stops grepping its way around 150 files. Split the skill. End a job at one landed change or about 120k context. The order and how to measure each step are at the end.

## Sources

| Source | Date | Grade | What it says, with numbers |
|---|---|---|---|
| Anthropic, [Writing effective tools for agents](https://www.anthropic.com/engineering/writing-tools-for-agents) | 2025-09-11 | P, M | Paginate, filter and truncate with sensible defaults. Claude Code caps a tool result at 25,000 tokens. A `response_format` concise/detailed switch cut a Slack result from 206 to 72 tokens. Errors should say how to fix the call. |
| Anthropic, [Effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) | 2025-09-29 | P | Context rot: recall falls as the window fills. Load files just in time by path, head and tail. Clear old tool results. Keep notes outside the window. Subagents use tens of thousands of tokens and return 1,000 to 2,000. |
| Anthropic, [Context management](https://claude.com/blog/context-management) | 2025-09-29 | P, M | Context editing gave 29% better results and used 84% fewer tokens in a 100-turn web-search eval. With the memory tool, results were 39% better. |
| Anthropic, [Code execution with MCP](https://www.anthropic.com/engineering/code-execution-with-mcp) | 2025-11-04 | P | Tools as files the agent lists and reads when it needs them. Data is filtered in code before it reaches the model: a 10,000-row sheet becomes 5 rows. An illustrative workflow went from 150,000 tokens to 2,000. Code that works gets saved as a skill. |
| Anthropic, [Advanced tool use](https://www.anthropic.com/engineering/advanced-tool-use) | 2025-11-24 | P, M | Tool Search: 77k of definitions down to 8.7k (−85%), and Opus 4.5 MCP accuracy rose from 79.5% to 88.1%. Programmatic tool calling: 43,588 tokens down to 27,297 a task (−37%). |
| Anthropic, [Multi-agent research system](https://www.anthropic.com/engineering/multi-agent-research-system) | 2025-06-13 | P, M | An agent uses about 4× a chat's tokens, a multi-agent system about 15×. Token use alone explained 80% of the variance in BrowseComp results. Subagents should write their output to files and pass references back. |
| Anthropic, [Agent Skills](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills) and [authoring best practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices) | 2025-10-16, current | P | Three levels: metadata, SKILL.md, linked files. Keep SKILL.md under 500 lines and references one level deep. Files over 100 lines get a contents list. Say whether a script is to be run or read. Validators should give specific errors. Plan, validate, execute. |
| Anthropic, [Vision](https://platform.claude.com/docs/en/build-with-claude/vision) | current | P | An image costs ⌈w/28⌉×⌈h/28⌉ tokens. On Claude 4.7 and later a 1920×1080 still is 2,691 tokens, 1092×1092 is 1,521, 200×200 is 64, and a 4K image is capped at 4,784. |
| Cursor, [Dynamic context discovery](https://cursor.com/blog/dynamic-context-discovery) | 2026-01 | V, M | Long tool and terminal output goes to files the agent tails. MCP descriptions are synced to a folder. In an A/B test of runs that called an MCP tool, total tokens fell 46.9%. |
| Frugal Blender MCP, [PyPI](https://pypi.org/project/frugal-blender-mcp) | 2026 | V, M | 4 tools with 338 tokens of fixed context, against 4,722 (Blender Lab) and 7,464 (ahujasid). Schemas are fetched only when used. Up to 100 operations a call. Results are counts and errors, never the scene: 109 operations returned 202 tokens. It looks at a capture rather than dumping the scene. |
| MindStudio, [Claude Blender MCP donut test](https://www.mindstudio.ai/blog/claude-blender-mcp-60-percent-tokens-donut-test-results) | 2026-05-01 | F | Two hours used 60% of a Max 5× session. Failures: sprinkles clipping and at the wrong scale, the cup clipping, and at the context limit a magenta wash with the work declared done. With no way to compact mid-session, the author recommends saving at each milestone and starting fresh. |
| rex-marks-the-spot, [Blender LLM integration](https://rex-marks-the-spot.pages.dev/blog/posts/blender-llm-integration-research.html) | 2026-02-01 | F | Render, look, retry. It added a second camera instead of moving the first. "Slow and expensive", and judging "good enough" was still manual. |
| Wu et al., [GPTEval3D](https://arxiv.org/abs/2401.04092) (CVPR 2024) | 2024-01 | M | A VLM comparing two assets' multi-view renders agreed with experts at Kendall's τ 0.71 on average (0.82 text match, 0.64 plausibility). |
| Gu et al., [BlenderGym](https://openaccess.thecvf.com/content/CVPR2025/html/Gu_BlenderGym_Benchmarking_Foundational_Model_Systems_for_Graphics_Editing_CVPR_2025_paper.html) (CVPR 2025) | 2025-06 | M | Every VLM fell well short of human Blender users, missing subtle visual differences. More verifier compute (re-judging fewer candidates) helped more than more generation. |
| Lu et al., [LL3M](https://arxiv.org/abs/2508.08228) | 2025-08 | M | Six agents (planner, retrieval, coder, critic, verifier, user) share a code context and a Blender API RAG. The critic judges renders. The authors list automatic visual feedback as the weak point. |
| Unity [prefab parser MCP](https://lobehub.com/fr/mcp/luckynee-unity-prefab-parser-mcp), [UnityMCP tool audit](https://glama.ai/mcp/servers/@Bluepuff71/UnityMCP/blob/dfed172af5ffe530547a69d03a0dee00e07d82ef/docs/plans/tool-optimization-analysis.md) | 2026 | V | Emitting only inspector-visible fields claims 70 to 90% fewer tokens, where a raw prefab is 10k to 50k. Scene listing should page and drill down from a shallow default. |
| Zechner, [MCP vs CLI](https://mariozechner.at/posts/2025-08-15-mcp-vs-cli/) | 2025-08-15 | M | 120 runs: both approaches succeeded every time, within a few percent on cost. What mattered was the tool's output, not the protocol. Later vendor benchmarks claiming CLI is 10 to 32× cheaper are [V]. |
| OpenAI, harness engineering ([secondary](https://ai-playbooks.pages.dev/reviews/harness-engineering-leveraging-codex-in-an-agent-first-world)) | 2026-02 | S | AGENTS.md as a map of about 100 lines into a docs tree that is the record. One big instruction file crowded out the task. |

Not found: a first-hand account of an agent making or dressing a character with token numbers. Online figures such as "a feature costs 15k to 35k tokens" come without a method and are left out.

## Our audit

**Method.** The five finished character jobs since 2026-11-02 were tallied from their transcripts by size and tool name, never read: char-switch j7, j8 and j9, and char-pipeline j1 and j2. The research job is left out, and j3 is still running. The tool results were measured exactly: the context's growth between two API calls, minus the earlier call's output, is what came back. That is 1.6× the 3.6-characters-a-token estimate (2.2 characters a token on our code and logs), and the parts add up to the final context within 1%. `tools/token-stats/context.mjs` estimates at 4 characters a token, so it undercounts the same way. Its "73% of what it wrote" line is that gap. The tally script is kept with this job's handoff (`~/.cache/terra-handoff/char-pipeline-tokens-tally.ts`).

| Job | Turns | Context at first edit | Peak | Summed input | Own output | Tool results | Carried from before first edit |
|---|---|---|---|---|---|---|---|
| char-switch j7, pack straps | 109 | 102k | 173k | 11.9M | 59k | 87k | 5.2M (44%) |
| char-switch j8, the man into the game | 165 | 139k | 244k | 26.8M | 67k | 153k | 13.4M (50%) |
| char-switch j9, build step, townsfolk | 160 | 75k | 209k | 22.1M | 67k | 118k | 6.1M (28%) |
| char-pipeline j1, catalogue and assembly | 163 | 122k | 256k | 27.4M | 91k | 135k | 11.1M (41%) |
| char-pipeline j2, outfits, pairings | 123 | 117k | 206k | 17.1M | 69k | 114k | 7.9M (46%) |
| **mean** | **144** | **111k** | **218k** | **21.1M** | **70k** | **121k** | **8.7M (41%)** |

The final context is the fixed prefix (25k) plus tool results (121k) plus the agent's own output (70k: thinking, commands, edits and files written). `context.mjs` priced j1 and j8 at $18.67 together: cache reads 54M, writes 597k, output 158k. One write of 115k came from a cache that had expired after an 11-hour wait in j1.

What the tool results were, measured in tokens a job (each figure is the mean, with the range in brackets):

| What was read | Tokens | Share of results | Notes |
|---|---|---|---|
| Our source code (Rust, TypeScript, Python), by `sed -n`, `cat` and Read | 50k (21k to 72k) | 40% | `appearance.rs` read whole (7k), `slots.ts` lines 80 to 450 (5k), `build_steps.ts` whole (5.5k), `townsfolk.rs` in five ranges |
| Docs to find where things stand: skill, `v0-characters.md`, decisions, research, handoff | 29k (16k to 38k) | 24% | Read early, so each token is carried about 130 turns |
| Search (`grep`, `find`, `ls`) | 25k (12k to 38k) | 21% | 41 calls a job (22 to 60), the most of any kind |
| Our tools' output and logs (Blender runs, `node tools/characters/…`, `out/*.log`, cargo) | 15k (10k to 25k) | 12% | About 500 tokens a Blender run. j7, the Blender-heavy job, was the highest |
| git (`show`, `diff`, `log`) | 3k to 12k | 5% | j7's `git show` of j6's four files was 9k |
| Stills | 7k (3k to 8k), 2 to 7 images | 6% | 1,100 to 2,700 tokens each |

The sizes of what a job is pointed at:

- `.agents/skills/character/SKILL.md` is 271 lines and 38 KB, about 17k tokens. It has 141 characters a line, and is nearly three times the size of the combat skill (13 KB) and 1.7× the controller's (23 KB). Jobs read it whole (14k measured) or in ranges.
- `tools/characters/README.md` is 44 KB (about 20k tokens) and is never read whole.
- `docs/v0-characters.md` is 25 KB in 82 lines (about 11k). The decisions D599, D600 and D602 are about 4k to 10k a job, and the research notes 5k to 8k.
- `tools/characters/` holds 152 files, with 26,710 lines of Python and TypeScript. 109 of those files name no ✓/✗, PASS/FAIL or summary line (some are libraries, not checks), and 14 can print JSON.
- Character work has no scorecard command and no ordered task list. Combat has one (`cargo xtask combat`, D452), and so has the controller (`cargo xtask controller`, D457). Each character job rebuilds "where are we" from the log, the handoff, decisions and code.
- Each agent's fixed prefix of 25k includes a 7.7k listing of about 45 skills, most of them for mail, Sentry and office files, and 3.5k of instructions. The prefix is read every turn: 3.6M of a job's summed input (17%).

### The top five sinks

1. **Reading our own source to learn how it works:** 50k tokens a job (40% of tool results), carried 1.4M to 8.6M.
2. **The agent's own output kept in context** (thinking, commands, edit text): 70k a job, a third of the final context. It grows with turns.
3. **Orientation documents read before work starts:** 29k a job. Counted with the source and search reads that come before the first edit, they carry 41% of the job's summed input (8.7M of 21.1M).
4. **Searching our files:** 25k a job in 41 calls. It is the largest number of turns, and each turn re-reads the whole context.
5. **The fixed prefix times the turns:** 25k × 144 turns = 3.6M a job, 7.7k of it a skill listing that has nothing to do with Terra.

The multiplier over all five is the number of turns. A job of 160 turns averaging 170k reads 27M. Two jobs of 80 turns each, at an average of 90k, would read about 14M between them, if starting a job cost little to read.

## Recommendation

These are ordered by input tokens saved a job for the work they take. Each is one job, measured by running the tally on the next three character jobs against the means above (111k at the first edit, 21.1M summed, 144 turns).

0. **Measure first (half a job).** Fold the tally's measures into `tools/token-stats/context.mjs`. Add, per transcript, the context at the first edit, the summed input, what was carried and by what was read, and measured rather than estimated result sizes (the 1.6× above). *Check:* it reproduces this note's table.
1. **A status command for character work (1 job), as combat and the controller have.** `cargo xtask characters` should print in 40 lines or fewer each criterion's number against its bar: `layers.ts` worst point, `gear_check.py` worst clip, `intact.ts`, pairings, rig check. Below that, the ordered next tasks, each with the files and line ranges that own it and the command that checks it. The handoff becomes its own line. It replaces the log, handoff, decisions and research reads at the start (29k plus the hunting around them). *Expected:* context at the first edit falls from 111k to about 60k, and summed input by 4M to 6M a job (20 to 30%). *Check:* context at the first edit and summed input.
2. **Split the skill (half a job).** A core of 120 lines or fewer (about 4k tokens): the loop, a table of commands with what each prints, and one line a trap with a link. Gear, morphs, clips, rebuilding and the trap details move to `references/`, one level deep (Anthropic's rule). *Expected:* 8k to 10k fewer tokens read early, about 1.2M summed a job. *Check:* the tally's "skill" row.
3. **A code map (1 job).** `node tools/characters/map.ts <topic>` (strap, outfit, morph, clip, export, game) should print the files for the topic, a line each, with their entry functions and line numbers, drawn from the files' header comments, and a test should keep it current. Add the same for `terra_walker`'s character modules. It takes the place of the README nobody reads and of the 41 searches. *Expected:* 15k to 25k fewer tokens and about 20 fewer turns a job, 2M to 3M summed. *Check:* the tally's search calls and source tokens.
4. **Shorter jobs (no build, a line in the briefs), after 1.** End a character job at one landed change or about 120k context, whichever comes first, instead of about 200k. *Expected:* 30 to 50% less summed input for each landed commit, once starting a job is cheap. *Check:* summed input for each commit on main.
5. **Verdict-first output on the character checks (1 job).** One line a check (`✓/✗ name number (bar)`) with the details in a log it names (D388), and `--json` for scripts. *Expected:* 5k to 10k fewer tokens on a Blender-heavy job (j7 spent 25k on tool output and logs), less on others. *Check:* the tally's tool-run and log rows.
6. **Trim the prefix (the developer's call).** Disabling the unrelated plugins for this repository in `.claude/settings.json` would drop most of the 7.7k skill listing, about 1M summed a job (5%), for no build work. It changes the developer's own sessions in Terra, so it goes on the desk.
7. **Effort for mechanical jobs (an experiment, after 1).** Thinking is most of the 70k of output. Run two like jobs (a fit, a rebuild) at medium effort and judge them by the same scorecard. Change nothing until the scorecard shows that quality holds.

What not to do: shrink stills further, or put a Blender MCP in place of the headless scripts. Stills are 6% of a job and 1092 px is enough for our checks. Our scripts already return small results. The cost is in reading our own code and docs to find out where things stand, and in the turns that reading takes.
