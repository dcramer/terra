# Token stats

Records how many Claude tokens and Tripo credits each of us spends on Terra (D82), shown at <https://terra-token-stats.dcramer.workers.dev>, and as JSON at [`/json`](https://terra-token-stats.dcramer.workers.dev/json). Two parts:

- `worker/`: a Cloudflare Worker with a D1 database. It stores each message once, by the id the API gave it, with its person, conversation, day, model and input, output, cache-read and cache-write tokens, and keeps totals per person, day and model. It runs on Cloudflare's free plan, which costs nothing and stops rather than bills when a limit is reached. A year of five busy developers is a few hundred megabytes of the free 5 GB.
- `report.mjs`: a script Claude Code runs as you work (see below). It reads Claude Code's transcripts under `~/.claude/projects` and the credits `spend.py` recorded, and sends what it has not sent before. It needs Node 18 or later and nothing else.
- `report.mjs --spend`: records the credits a task spent on another service, such as Tripo (below).

## What counts, and counting once

A session counts if it started inside this repository, its worktrees included, wherever it went afterwards. Claude Code files each session under the folder it started in, and the script checks the real path, so a folder such as `terra-old` does not count.

A message's id is the only thing that identifies it. The same message turns up many times in the transcripts: once per content block, with partial usage on the early lines, and again in every conversation resumed or forked from its own, under the new conversation's id and sometimes on a later day. So the script keeps one record per message id with the largest of each count and the earliest day, and the server keys on the id too, keeping the larger counts and the earlier day when a message arrives again. Sending twice, losing the record of what was sent (`~/.cache/terra-token-stats`), or Claude Code deleting old transcripts (after 30 days by default) can never count a message twice or lose one already sent. Totals per conversation would not work: a message copied into a resumed conversation would count in both.

## Cost

The page leads with what the tokens would cost at the API's list prices, with the [Tripo credits](#tripo-credits) added, per person and day, and `/json` has it per model too. It leaves out cache reads: they are most of the token count, the same context read again each turn, and little of the cost. Most of us pay by subscription, so this is the price of the work on the API, not what anyone paid. The prices are in [`worker/src/prices.js`](worker/src/prices.js) and need updating when they change or a model is added; a model not listed there counts as free. Cache writes are priced at the 1-hour rate, the only kind Claude Code writes, and Haiku 5.5 at its rate for prompts up to 100,000 tokens.

## Tripo credits

The [Tripo CLI](../tripo/README.md) records the credits each task spent, read from the account's own history after every command (or `node tools/tripo/tripo.ts report`), so they are exact and include work done by hand in Tripo's web app. Each task is a line of `~/.local/state/terra-token-stats/spends.jsonl` (under `$XDG_STATE_HOME` if set), keyed by the task's id, which `report.mjs` sends with your messages and the worker stores once by that id, as it does messages. Unlike the cache, the file is the only record of what it holds until it is sent, so don't delete it.

Credits count in each person's cost, priced in [`worker/src/prices.js`](worker/src/prices.js): API credits at a cent each, their list price, and Studio credits at what they cost on the subscription, Professional's $20 a month for 3,000 credits. The subscription is paid whether we use it or not, so like the token cost this is the price of the work, not an extra bill. The page splits every total into Claude and Tripo, under the total, on each person and in each day's bar, and `/json` has `claude_cost`, `tripo_cost` and `tripo_credits` beside `cost`. Each service is also listed beside the models, with its credits.

Tripo work that the CLI has not seen, from an account nobody runs it with, can be recorded by hand, with the project's id or anything unique:

```bash
node tools/token-stats/report.mjs --spend tripo-studio <id> <credits>
```

The page and `/json` are public. Sending and `/stats` need the shared key in `Authorization: Bearer <key>`. The key is a secret: never commit it.

## The ticker

[`/ticker`](https://terra-token-stats.dcramer.workers.dev/ticker) is the running cost, with a line under it for each person and their cost, for an iframe or a stream overlay such as a Restream browser widget. It scales to fill the box it is given, keeping its shape, over a transparent background, so the box's size is the ticker's size. It checks for new numbers every 5 seconds while the page it sits on is visible, and a click opens the full page. The colours follow the embedding page's light or dark scheme; add `?theme=light` or `?theme=dark` to fix them. [`/ticker/<name>`](https://terra-token-stats.dcramer.workers.dev/ticker/dcramer) is one person's cost alone, by the name they report as, with their name and the tokens behind it (cache reads left out, as on the page); it is not found until that person has reported something.

```html
<iframe src="https://terra-token-stats.dcramer.workers.dev/ticker" width="460" height="260" style="border:0" title="What building Terra has cost"></iframe>
```

## Reporting your usage

Ask for the key, then copy `.env.example` at the repository root to `.env` and fill it in. Git ignores `.env`. Worktrees read the main checkout's `.env`, so one file is enough.

```bash
TERRA_STATS_URL=https://terra-token-stats.dcramer.workers.dev
TERRA_STATS_KEY=<shared key>
TERRA_STATS_USER=<your name>
```

Variables of the same names in the environment override the file. That is all: the repository's `.claude/settings.json` runs `report.mjs --hook` after every tool call and turn here, so a long turn reports as it goes, and again when a session ends, so nothing waits for the next session. The hook does nothing without a `.env`, sends at most once a minute (except at a session's end), and sends from a detached process so Claude Code never waits; it logs to `~/.cache/terra-token-stats/report.log`. The page checks for new numbers every 5 seconds. The worker keeps the summary it last worked out and works it out again only when a report changes something, so each check reads one row however many pages are open.

Each send reads every transcript again, which takes about half a second today. If months of transcripts make that slow, the script should remember how far into each file it has read.

By hand, from the repository:

| Command | What it does |
|---|---|
| `node tools/token-stats/report.mjs --dry-run` | Print your totals by day and model, and credits by day and service, sending nothing |
| `node tools/token-stats/report.mjs` | Send new messages |
| `node tools/token-stats/report.mjs --full` | Send every message again; harmless |
| `node tools/token-stats/report.mjs --stats` | Everyone's totals; add `--by-day` or `--by-model` |
| `node tools/token-stats/report.mjs --cron` | Print a crontab line that sends hourly, if you want that too |

## Deploying the worker

It is deployed on David's Cloudflare account, and deploys itself: Cloudflare's Workers Builds is connected to this repository, and runs `npm run build` (the tests, then a dry-run deploy that bundles and checks the worker) and `npx wrangler deploy` when anything under `tools/token-stats/` changes on `main`. Its settings live on the worker in Cloudflare's dashboard, under Settings, Builds: root directory `tools/token-stats/worker`, build command `npm run build`, deploy command `npx wrangler deploy`, watch path `tools/token-stats/*`. Deploying leaves the `API_KEY` secret as it is.

To deploy by hand, or to set it up again elsewhere:

```bash
cd tools/token-stats/worker
npm install
npx wrangler login
npx wrangler deploy
openssl rand -hex 32 | npx wrangler secret put API_KEY
```

The worker creates its tables on first use, so there is no migration step. On a new account, remove `database_id` from `wrangler.toml` first and the deploy creates the database. Share the key privately. To change the key, run the last line again and tell everyone.

To try it locally, put `API_KEY=devkey` in `worker/.dev.vars` and run `npx wrangler dev`.

## API

| Call | Body or query |
|---|---|
| `GET /` | The page: everyone's cost and tokens, models and services, and cost a day for 30 days |
| `GET /json` | The same as JSON |
| `GET /ticker` | The cost and everyone's, to embed; `theme`: `light` or `dark` |
| `GET /ticker/<name>` | One person's cost and tokens |
| `POST /report` | `{"user": "dcramer", "messages": [{"id": "msg_…", "session": "…", "day": "2026-10-10", "model": "claude-opus-5-5", "input": 1, "output": 2, "cache_read": 3, "cache_write": 4}], "spends": [{"id": "tripo-api-…", "day": "2026-10-11", "service": "tripo-api", "credits": 50}]}`, at most 500 messages and spends together |
| `GET /stats` | Tokens only. `group`: any of `user`, `day`, `model`, comma-separated (default `user`); `since`: a day |

Tests: `npm test` in `worker/`.
