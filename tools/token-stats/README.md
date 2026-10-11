# Token stats

Records how many Claude tokens each of us spends on Terra (D82). Two parts:

- `worker/`: a Cloudflare Worker with a D1 database. It stores one row per person, UTC day and model: input, output, cache-read and cache-write tokens, and messages. It runs on Cloudflare's free plan, which costs nothing and stops rather than bills when a limit is reached.
- `report.mjs`: a script each developer runs hourly. It reads Claude Code's transcripts under `~/.claude/projects`, counts the messages from sessions in this repository and its worktrees, and sends the totals. It needs Node 18 or later and nothing else.

The script sends running totals, not increments, and the worker keeps the larger of what it has and what it gets. Sending twice changes nothing, and a day stays counted after Claude Code deletes old transcripts (after 30 days by default).

Both calls need the shared key in `Authorization: Bearer <key>`. The key is a secret: never commit it.

## Reporting your usage

Ask for the worker's address and the key, then copy `.env.example` at the repository root to `.env` and fill it in. Git ignores `.env`. Worktrees read the main checkout's `.env`, so one file is enough.

```bash
TERRA_STATS_URL=https://terra-token-stats.<account>.workers.dev
TERRA_STATS_KEY=<shared key>
TERRA_STATS_USER=<your name>
```

Variables of the same names in the environment override the file. Then, from the repository:

| Command | What it does |
|---|---|
| `node tools/token-stats/report.mjs --dry-run` | Print what would be sent |
| `node tools/token-stats/report.mjs` | Send it |
| `node tools/token-stats/report.mjs --stats` | Everyone's totals; add `--by-day` or `--by-model` |
| `node tools/token-stats/report.mjs --cron` | Print a crontab line that sends hourly |

To send hourly, add the `--cron` line with `crontab -e`. It uses absolute paths, since cron has almost no `PATH`, and logs to `~/.cache/terra-token-stats.log`. Under WSL, cron runs only if started (`sudo service cron start`, or `[boot] command` in `/etc/wsl.conf`).

## Deploying the worker

Once, by whoever holds the Cloudflare account:

```bash
cd tools/token-stats/worker
npm install
npx wrangler login
npx wrangler deploy
openssl rand -hex 32 | npx wrangler secret put API_KEY
```

The first deploy creates the D1 database, and the worker creates its table on first use, so there is no migration step. Share the printed `workers.dev` address and the key privately. To change the key, run the last line again and tell everyone.

To try it locally, put `API_KEY=devkey` in `worker/.dev.vars` and run `npx wrangler dev`.

## API

| Call | Body or query |
|---|---|
| `POST /report` | `{"user": "dcramer", "rows": [{"day": "2026-10-10", "model": "claude-opus-5-5", "input": 1, "output": 2, "cache_read": 3, "cache_write": 4, "messages": 5}]}` |
| `GET /stats` | `group`: any of `user`, `day`, `model`, comma-separated (default `user`); `since`: a day |

Tests: `node --test tools/token-stats/report.test.mjs`.
