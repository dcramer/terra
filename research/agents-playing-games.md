# How AI agents play games

2026-10-10. Question: what are the ways an AI agent can play a game (from the screen like a person, or through the game's state), what has been tried, and what would make agents efficient at playing Terra (D73)?

Read, not run: papers, project pages and write-ups, mostly through search summaries. Nothing here was built or measured.

## The options, from most human-like to most direct

| | What the agent sees | What it does | Examples | Good at | Bad at |
|---|---|---|---|---|---|
| 1. Screen and keyboard | Pixels | Keys and mouse | SIMA 2, VPT, MineRL, VideoGameBench | Any game, no work from the game | Slow, costly per step, poor at space and precise control |
| 2. Screen with a harness | Screenshots plus text pulled from the game: map, position, memory | Buttons, plus tools such as "walk to this square" | Claude and Gemini Plays Pokémon | Long games, once the harness is good | The harness does much of the work and is per-game |
| 3. Raw state, small actions | Numbers: units, maps, resources | One small action a tick | OpenAI Five (Dota 2), AlphaStar's raw interface, Unity ML-Agents, Godot RL Agents | Training with reinforcement learning, thousands of steps a second | Not how a language model wants to work |
| 4. State as objects, actions as tools or code | A description of what is near, carried and possible | High-level calls (go to, mine, craft, place) or short programs over them, with errors that say why | Mineflayer with Voyager and Mindcraft (Minecraft), Factorio Learning Environment, Project Sid | Language-model agents: few calls, long plans, many agents | Someone has to write and keep the action layer |
| 5. A client like a player's | Whatever the server sends a player, as data | The same actions a player has, through the same protocol, often as an MCP server | OpenMMO and NexWorld (announced, unverified), Screeps (players write code) | No special powers; the server's rules hold | New; little evidence at scale |

For our own testing there is also the developer's view: Bevy's remote protocol, with an MCP server over it (`bevy_brp`), reads and changes any entity or component of a running game. That is a tester's power over the world, not a player's (D69).

## What the record shows

- **Pixels alone are far from working.** VideoGameBench gives models only screenshots and no game-specific help; the best model in 2025 finished under half a percent of it, and failed at basic dragging, clicking and grid navigation. SIMA 2 (Google DeepMind, November 2025) plays commercial games, No Man's Sky among them, from the screen with a virtual keyboard and mouse, but still struggles with long tasks and precise control.
- **The harness is what makes long games work.** Claude and Gemini playing Pokémon were given text read from the game's memory, a text minimap, a pathfinding tool, a notepad, and summaries of past turns. Observers credit the tools as much as the model. Both still got lost; space is the weak point.
- **High-level actions with honest errors are the efficient middle.** Mineflayer gives Minecraft bots calls like "go to", "dig" and "craft". Voyager had GPT-4 write programs over them and keep the ones that worked as a skill library; Mindcraft gives agents about 47 tools and turns writing code off by default, since running a model's code on a public server is risky. The Factorio Learning Environment gives agents a Python API of about 27 tools and typed exceptions that say why something failed; models found real strategies but were still weak at spatial layout.
- **A thousand agents in one world has been done.** Project Sid ran 10 to more than 1,000 agents in Minecraft through Mineflayer. They took on roles, traded, voted on taxes. It needed an agent architecture of its own (PIANO) so each agent stayed coherent while acting and talking at once.
- **Agents are terrible at talking to each other about plans.** MineCollab found performance fell by up to 15% when agents had to share detailed plans.
- **Bots as players is an old, working idea.** Private World of Warcraft servers fill their worlds with server-side "playerbots", now with language models for chat; one reports about 1,800 bots, with the server never waiting on the model.

## What this suggests for Terra

Our own reading; nothing here is decided. D73 already says agents play headless, as a client like any other, keeping the same rules, most likely through MCP. Options 4 and 5 together fit that:

- **The agent is a real client of the real server**, not a separate simulation. Server-first (D51) means it gets what a player gets and can do only what a player can.
- **What it sees is the player's view as data**: what is near (by area, as the server already sends), what it carries, its attributes, what it is aimed at, and a coarse map. No pixels by default; a screenshot on request, for the rare question only sight can answer.
- **Two levels of action.** The small ones a player's input makes (move, turn, use what is in hand), and skills built on them in the client, in Rust: walk to a place by pathfinding, harvest the nearest tree, build this part here. Every write-up says space and navigation are where models fail, so pathfinding belongs to the game, not the model.
- **Every action says what happened**, and when it fails, why: out of reach, not enough wood, in the water. Factorio and Minecraft agents correct themselves from errors like these.
- **Agents act at their own pace.** A skill runs to completion and reports back while the world keeps going; the model is not asked about every frame. A paused, step-at-a-time mode is useful for local tests only, never on a shared server.
- **Calls, not code, at first.** Letting a model write programs over the actions (Voyager) is powerful, but tool calls are safer and easier to limit. Our skills can grow from what agents keep asking for.
- **The same action list for players, the debug menu, QA and agents** (D69, D81): one list, four ways in.

## Limits

- Mostly second-hand: search summaries of papers and posts, not the papers in full. OpenMMO and NexWorld are announcements; nothing confirms they run at scale.
- No numbers for what an agent player would cost us: server time, bandwidth, or model calls per hour of play.
- Nothing on agents that learn from play (training), only on agents that play with an existing model.

## Sources

- SIMA 2: [DeepMind](https://deepmind.google/blog/sima-2-an-agent-that-plays-reasons-and-learns-with-you-in-virtual-3d-worlds/), [MIT Technology Review](https://www.technologyreview.com/2025/11/13/1127921/google-deepmind-is-using-gemini-to-train-agents-inside-goat-simulator-3/)
- VideoGameBench: [arXiv 2505.18134](https://arxiv.org/abs/2505.18134); lmgame-Bench: [arXiv 2505.15146](https://arxiv.org/abs/2505.15146)
- Pokémon harnesses: [Anthropic, Harnessing Claude's intelligence](https://claude.com/blog/harnessing-claudes-intelligence), [a case study of Gemini Plays Pokémon](https://www.dbreunig.com/2025/06/17/an-agentic-case-study-playing-pokémon-with-gemini.html), [a comparison of the two harnesses](https://www.greaterwrong.com/posts/7mqp8uRnnPdbBzJZE/is-gemini-now-better-than-claude-at-pokemon)
- Minecraft: [Mineflayer](https://github.com/PrismarineJS/mineflayer), [Voyager, arXiv 2305.16291](https://arxiv.org/abs/2305.16291), [Mindcraft and MineCollab, arXiv 2504.17950](https://arxiv.org/abs/2504.17950), [Project Sid, arXiv 2411.00114](https://arxiv.org/abs/2411.00114), [VPT, arXiv 2206.11795](https://arxiv.org/abs/2206.11795)
- Factorio Learning Environment: [arXiv 2503.09617](https://arxiv.org/abs/2503.09617), [code](https://github.com/JackHopkins/factorio-learning-environment)
- Raw interfaces for training: [OpenAI Five, arXiv 1912.06680](https://arxiv.org/abs/1912.06680), [AlphaStar Unplugged, arXiv 2308.03526](https://arxiv.org/abs/2308.03526), [PySC2](https://github.com/google-deepmind/pysc2), [Godot RL Agents, arXiv 2112.03636](https://arxiv.org/abs/2112.03636), [Unity ML-Agents](https://github.com/Unity-Technologies/ml-agents)
- Clients and bots: [OpenMMO announcement](https://braindetox.kr/en/posts/openmmo_ai_human_mmorpg_2026.html), [WoW playerbots with a local model](https://github.com/Merrymak3r/wow-llm-personas), [Screeps](https://screeps.com)
- Bevy: [bevy_brp, an MCP server over Bevy's remote protocol](https://github.com/natepiano/bevy_brp)
