# Token stats

Records how many Claude tokens each of us spends on Terra (D82), shown at <https://terra-token-stats.dcramer.workers.dev>, and as JSON at [`/json`](https://terra-token-stats.dcramer.workers.dev/json). Two parts:

- `worker/`: a Cloudflare Worker with a D1 database. It stores each message once, by the id the API gave it, with its person, conversation, day, model and input, output, cache-read and cache-write tokens, and keeps totals per person, day and model. It runs on Cloudflare's free plan, which costs nothing and stops rather than bills when a limit is reached. A year of five busy developers is a few hundred megabytes of the free 5 GB.
- `report.mjs`: a script Claude Code runs after each turn (see below). It reads Claude Code's transcripts under `~/.claude/projects` and sends the messages it has not sent before. It needs Node 18 or later and nothing else.

## What counts, and counting once

A session counts if it started inside this repository, its worktrees included, wherever it went afterwards. Claude Code files each session under the folder it started in, and the script checks the real path, so a folder such as `terra-old` does not count.

A message's id is the only thing that identifies it. The same message turns up many times in the transcripts: once per content block, with partial usage on the early lines, and again in every conversation resumed or forked from its own, under the new conversation's id and sometimes on a later day. So the script keeps one record per message id with the largest of each count and the earliest day, and the server keys on the id too, keeping the larger counts and the earlier day when a message arrives again. Sending twice, losing the record of what was sent (`~/.cache/terra-token-stats`), or Claude Code deleting old transcripts (after 30 days by default) can never count a message twice or lose one already sent. Totals per conversation would not work: a message copied into a resumed conversation would count in both.

The page and `/json` are public. Sending and `/stats` need the shared key in `Authorization: Bearer <key>`. The key is a secret: never commit it.

## Reporting your usage

Ask for the key, then copy `.env.example` at the repository root to `.env` and fill it in. Git ignores `.env`. Worktrees read the main checkout's `.env`, so one file is enough.

```bash
TERRA_STATS_URL=https://terra-token-stats.dcramer.workers.dev
TERRA_STATS_KEY=<shared key>
TERRA_STATS_USER=<your name>
```

Variables of the same names in the environment override the file. That is all: the repository's `.claude/settings.json` runs `report.mjs --hook` whenever Claude Code finishes a turn here. The hook does nothing without a `.env`, runs at most every 15 minutes, and sends from a detached process so a turn never waits; it logs to `~/.cache/terra-token-stats/report.log`. Usage outside Claude Code turns, such as the last turn before a long break, goes out with the next one.

By hand, from the repository:

| Command | What it does |
|---|---|
| `node tools/token-stats/report.mjs --dry-run` | Print your totals by day and model, sending nothing |
| `node tools/token-stats/report.mjs` | Send new messages |
| `node tools/token-stats/report.mjs --full` | Send every message again; harmless |
| `node tools/token-stats/report.mjs --stats` | Everyone's totals; add `--by-day` or `--by-model` |
| `node tools/token-stats/report.mjs --cron` | Print a crontab line that sends hourly, if you want that too |

## Deploying the worker

It is deployed on David's Cloudflare account. To deploy changes, or to set it up again elsewhere:

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
| `GET /` | The page: everyone's totals, models, and tokens a day for 30 days |
| `GET /json` | The same as JSON |
| `POST /report` | `{"user": "dcramer", "messages": [{"id": "msg_…", "session": "…", "day": "2026-10-10", "model": "claude-opus-5-5", "input": 1, "output": 2, "cache_read": 3, "cache_write": 4}]}`, at most 500 messages |
| `GET /stats` | `group`: any of `user`, `day`, `model`, comma-separated (default `user`); `since`: a day |

Tests: `node --test tools/token-stats/report.test.mjs`.
