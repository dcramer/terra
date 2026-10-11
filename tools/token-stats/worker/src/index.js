// Records Claude token usage per message, and totals per person, day and model, and the
// Tripo credits each task spent. See ../README.md.
//
// GET  /       the page: tokens per person, public
// GET  /json   the same as JSON, public
// GET  /ticker the running cost alone, small enough to embed in an iframe, public
// GET  /ticker/<user> the same for one person, public
// POST /report {user, messages: [{id, session, day, model, input, output, cache_read, cache_write}],
//               spends: [{id, day, service, credits}]}
// GET  /stats  ?group=user,day,model&since=YYYY-MM-DD
//
// /report and /stats need `Authorization: Bearer <API_KEY>`.
//
// Errors and traces go to Sentry when `SENTRY_DSN` is set (wrangler.toml), tagged
// with the deployed version.

import * as Sentry from "@sentry/cloudflare";
import { page } from "./page.js";
import { cost, creditCost } from "./prices.js";
import { ticker } from "./ticker.js";

const USER = /^[A-Za-z0-9_.-]{1,40}$/;
const ID = /^[A-Za-z0-9_-]{1,100}$/;
const SESSION = /^[A-Za-z0-9_-]{0,100}$/;
const DAY = /^\d{4}-\d{2}-\d{2}$/;
const MODEL = /^[A-Za-z0-9_.:@/-]{1,100}$/;
const SERVICE = /^[a-z0-9-]{1,40}$/;
const COUNTS = ["input", "output", "cache_read", "cache_write"];
const GROUPS = ["user", "day", "model"];
const MAX_MESSAGES = 500;
const CHART_DAYS = 30;
// Raise when the summary gains or loses a field, so a snapshot saved by an older
// deploy is worked out again rather than handed to a page that expects the new one.
const SUMMARY_VERSION = 2;

// Every message is stored once, keyed by the id the API gave it, so it cannot be
// counted twice however often it is sent or from how many transcripts. `usage`
// holds the totals, kept in step by triggers, so reading stats stays cheap.
const SCHEMA = [
  `CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    user TEXT NOT NULL,
    session TEXT NOT NULL,
    day TEXT NOT NULL,
    model TEXT NOT NULL,
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    cache_read_tokens INTEGER NOT NULL,
    cache_write_tokens INTEGER NOT NULL,
    updated_at TEXT NOT NULL
  )`,
  `CREATE TABLE IF NOT EXISTS usage (
    user TEXT NOT NULL,
    day TEXT NOT NULL,
    model TEXT NOT NULL,
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    cache_read_tokens INTEGER NOT NULL,
    cache_write_tokens INTEGER NOT NULL,
    messages INTEGER NOT NULL,
    PRIMARY KEY (user, day, model)
  )`,
  // The last summary, so the page can poll every few seconds for the price of one row.
  `CREATE TABLE IF NOT EXISTS snapshot (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    day TEXT NOT NULL,
    json TEXT NOT NULL
  )`,
  // Credits spent on other services, such as Tripo's, one row per task. They are few,
  // so the summary sums them as it goes, with no totals table.
  `CREATE TABLE IF NOT EXISTS spends (
    id TEXT PRIMARY KEY,
    user TEXT NOT NULL,
    day TEXT NOT NULL,
    service TEXT NOT NULL,
    credits INTEGER NOT NULL,
    updated_at TEXT NOT NULL
  )`,
  `CREATE TRIGGER IF NOT EXISTS messages_insert AFTER INSERT ON messages BEGIN
    ${addToUsage("new", 1)}
  END`,
  `CREATE TRIGGER IF NOT EXISTS messages_update AFTER UPDATE ON messages BEGIN
    ${addToUsage("old", -1)}
    ${addToUsage("new", 1)}
  END`,
];

function addToUsage(row, sign) {
  const v = (col) => (sign < 0 ? `-${row}.${col}` : `${row}.${col}`);
  return `INSERT INTO usage (user, day, model, input_tokens, output_tokens,
      cache_read_tokens, cache_write_tokens, messages)
    VALUES (${row}.user, ${row}.day, ${row}.model, ${v("input_tokens")}, ${v("output_tokens")},
      ${v("cache_read_tokens")}, ${v("cache_write_tokens")}, ${sign})
    ON CONFLICT (user, day, model) DO UPDATE SET
      input_tokens = input_tokens + excluded.input_tokens,
      output_tokens = output_tokens + excluded.output_tokens,
      cache_read_tokens = cache_read_tokens + excluded.cache_read_tokens,
      cache_write_tokens = cache_write_tokens + excluded.cache_write_tokens,
      messages = messages + excluded.messages;`;
}

// A message sent again keeps the larger counts and the earlier day, and is not
// written at all when nothing changed. One statement for every message, since the
// free plan allows 50 queries a request and 100 bound values a query.
const UPSERT = `INSERT INTO messages (id, user, session, day, model, input_tokens, output_tokens,
    cache_read_tokens, cache_write_tokens, updated_at)
  SELECT m.value ->> 'id', ?1, m.value ->> 'session', m.value ->> 'day', m.value ->> 'model',
    m.value ->> 'input', m.value ->> 'output', m.value ->> 'cache_read', m.value ->> 'cache_write', ?3
  FROM json_each(?2) AS m WHERE true
  ON CONFLICT (id) DO UPDATE SET
    day = min(messages.day, excluded.day),
    input_tokens = max(messages.input_tokens, excluded.input_tokens),
    output_tokens = max(messages.output_tokens, excluded.output_tokens),
    cache_read_tokens = max(messages.cache_read_tokens, excluded.cache_read_tokens),
    cache_write_tokens = max(messages.cache_write_tokens, excluded.cache_write_tokens),
    updated_at = excluded.updated_at
  WHERE excluded.day < messages.day
    OR excluded.input_tokens > messages.input_tokens
    OR excluded.output_tokens > messages.output_tokens
    OR excluded.cache_read_tokens > messages.cache_read_tokens
    OR excluded.cache_write_tokens > messages.cache_write_tokens`;

// A task sent again keeps the larger count and the earlier day, as a message does.
const UPSERT_SPENDS = `INSERT INTO spends (id, user, day, service, credits, updated_at)
  SELECT s.value ->> 'id', ?1, s.value ->> 'day', s.value ->> 'service', s.value ->> 'credits', ?3
  FROM json_each(?2) AS s WHERE true
  ON CONFLICT (id) DO UPDATE SET
    day = min(spends.day, excluded.day),
    credits = max(spends.credits, excluded.credits),
    updated_at = excluded.updated_at
  WHERE excluded.day < spends.day OR excluded.credits > spends.credits`;

// The page and the ticker poll every 5 seconds while open, so their requests are
// traced at a sample. Reports are traced whole. The key is filtered from headers by
// default; bodies and addresses are left out, since they say nothing about a fault.
const sentry = (env) => ({
  dsn: env.SENTRY_DSN,
  tracesSampler: ({ name }) => (name.startsWith("GET") ? 0.05 : 1),
  dataCollection: { userInfo: false, httpBodies: [] },
});

export default Sentry.withSentry(sentry, {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/") {
      return new Response(page(await snapshot(env)), { headers: { "Content-Type": "text/html; charset=utf-8", ...PUBLIC } });
    }
    const tickerPath = url.pathname.match(/^\/ticker(?:\/([^/]+))?\/?$/);
    if (request.method === "GET" && tickerPath) {
      // Names need no escaping in a path, so one that does is not a name.
      const user = tickerPath[1] ?? null;
      if (user !== null && !USER.test(user)) return json({ error: "not found" }, 404);
      const html = ticker(await snapshot(env), url.searchParams.get("theme"), user);
      if (html === null) return json({ error: "no such user" }, 404);
      return new Response(html, { headers: { "Content-Type": "text/html; charset=utf-8", ...PUBLIC } });
    }
    if (request.method === "GET" && url.pathname === "/json") {
      return json(await snapshot(env), 200, PUBLIC);
    }
    if (!(await authorized(request, env))) return json({ error: "unauthorized" }, 401);
    try {
      if (request.method === "POST" && url.pathname === "/report") return await report(request, env);
      if (request.method === "GET" && url.pathname === "/stats") return await stats(url, env);
      return json({ error: "not found" }, 404);
    } catch (err) {
      if (err instanceof BadRequest) return json({ error: err.message }, 400);
      throw err;
    }
  },
});

class BadRequest extends Error {}

async function authorized(request, env) {
  const header = request.headers.get("Authorization") ?? "";
  if (!env.API_KEY || !header.startsWith("Bearer ")) return false;
  // Compare digests so the comparison takes the same time whatever the key's length.
  const [given, wanted] = await Promise.all(
    [header.slice(7), env.API_KEY].map((s) => crypto.subtle.digest("SHA-256", new TextEncoder().encode(s))),
  );
  return crypto.subtle.timingSafeEqual(given, wanted);
}

async function report(request, env) {
  let body;
  try {
    body = await request.json();
  } catch {
    throw new BadRequest("body must be JSON");
  }
  if (typeof body?.user !== "string" || !USER.test(body.user)) throw new BadRequest("bad user");
  const given = { messages: body.messages ?? [], spends: body.spends ?? [] };
  if (!Array.isArray(given.messages)) throw new BadRequest("bad messages");
  if (!Array.isArray(given.spends)) throw new BadRequest("bad spends");
  if (given.messages.length + given.spends.length > MAX_MESSAGES) throw new BadRequest("too many messages and spends");
  const messages = given.messages.map(cleanMessage);
  const spends = given.spends.map(cleanSpend);
  if (messages.length + spends.length === 0) return json({ messages: 0, spends: 0, rows_written: 0 });

  const now = new Date().toISOString();
  const writes = [
    [UPSERT, messages],
    [UPSERT_SPENDS, spends],
  ].filter(([, rows]) => rows.length > 0);
  const results = await withTables(env, () =>
    env.DB.batch(writes.map(([sql, rows]) => env.DB.prepare(sql).bind(body.user, JSON.stringify(rows), now))),
  );
  const written = results.reduce((sum, r) => sum + r.meta.changes, 0);
  if (written > 0) await saveSnapshot(env);
  return json({ messages: messages.length, spends: spends.length, rows_written: written });
}

// The summary as of the last report that changed anything, worked out again when a
// new day moves the chart, or when a deploy changes its shape. Reading it costs one
// row however often the page polls, where working it out reads every row of `usage`.
async function snapshot(env) {
  const row = await withTables(env, () => env.DB.prepare("SELECT day, json FROM snapshot WHERE id = 1").first());
  if (row && row.day === new Date().toISOString().slice(0, 10)) {
    const s = JSON.parse(row.json);
    if (s.version === SUMMARY_VERSION) return s;
  }
  return await saveSnapshot(env);
}

async function saveSnapshot(env) {
  const s = await summary(env);
  await env.DB.prepare("INSERT OR REPLACE INTO snapshot (id, day, json) VALUES (1, ?1, ?2)")
    .bind(s.generated_at.slice(0, 10), JSON.stringify(s))
    .run();
  return s;
}

function cleanMessage(m) {
  if (typeof m?.id !== "string" || !ID.test(m.id)) throw new BadRequest("bad id");
  const session = m.session ?? "";
  if (typeof session !== "string" || !SESSION.test(session)) throw new BadRequest("bad session");
  if (typeof m.day !== "string" || !DAY.test(m.day)) throw new BadRequest("bad day");
  if (typeof m.model !== "string" || !MODEL.test(m.model)) throw new BadRequest("bad model");
  const clean = { id: m.id, session, day: m.day, model: m.model };
  for (const key of COUNTS) {
    const n = m[key] ?? 0;
    if (!Number.isSafeInteger(n) || n < 0) throw new BadRequest(`bad ${key}`);
    clean[key] = n;
  }
  return clean;
}

function cleanSpend(s) {
  if (typeof s?.id !== "string" || !ID.test(s.id)) throw new BadRequest("bad spend id");
  if (typeof s.day !== "string" || !DAY.test(s.day)) throw new BadRequest("bad spend day");
  if (typeof s.service !== "string" || !SERVICE.test(s.service)) throw new BadRequest("bad service");
  if (!Number.isSafeInteger(s.credits) || s.credits < 0) throw new BadRequest("bad credits");
  return { id: s.id, day: s.day, service: s.service, credits: s.credits };
}

async function stats(url, env) {
  const group = (url.searchParams.get("group") ?? "user").split(",").filter(Boolean);
  if (group.length === 0 || group.some((g) => !GROUPS.includes(g))) throw new BadRequest("bad group");
  const since = url.searchParams.get("since") ?? "0000-00-00";
  if (!DAY.test(since)) throw new BadRequest("bad since");

  // `group` holds only names from GROUPS, so it is safe to place in the query.
  const cols = group.join(", ");
  const { results } = await withTables(env, () =>
    env.DB.prepare(
      `SELECT ${cols},
        sum(input_tokens) AS input, sum(output_tokens) AS output,
        sum(cache_read_tokens) AS cache_read, sum(cache_write_tokens) AS cache_write,
        sum(messages) AS messages
      FROM usage WHERE day >= ?1 GROUP BY ${cols} ORDER BY ${cols}`,
    )
      .bind(since)
      .all(),
  );
  return json({ since, group, rows: results });
}

// The page polls every few seconds and must see new numbers.
const PUBLIC = { "Cache-Control": "no-cache" };
const SUMS = `sum(input_tokens) AS input, sum(output_tokens) AS output,
  sum(cache_read_tokens) AS cache_read, sum(cache_write_tokens) AS cache_write, sum(messages) AS messages`;

// Everyone's totals, their models, and tokens a day for the last CHART_DAYS days, each
// with `cost`: what the tokens would cost at API list prices, in dollars (prices.js).
// Credits spent on other services count in `cost` too, and each service is listed with
// the models, with its `credits` and no tokens. `claude_cost` and `tripo_cost` split
// `cost` for the page, per person, per day and in the totals.
async function summary(env) {
  const today = new Date();
  const since = new Date(today.getTime() - (CHART_DAYS - 1) * 86400000).toISOString().slice(0, 10);
  const [people, models, daily, spent, spentDaily] = await withTables(env, () =>
    env.DB.batch([
      env.DB.prepare(`SELECT user, ${SUMS}, min(day) AS first_day, max(day) AS last_day
        FROM usage WHERE messages > 0 GROUP BY user`),
      env.DB.prepare(`SELECT user, model, ${SUMS} FROM usage WHERE messages > 0 GROUP BY user, model`),
      env.DB.prepare(`SELECT user, day, model, ${SUMS} FROM usage WHERE day >= ?1 GROUP BY user, day, model`).bind(since),
      env.DB.prepare(`SELECT user, service, sum(credits) AS credits, min(day) AS first_day, max(day) AS last_day
        FROM spends GROUP BY user, service`),
      env.DB.prepare(`SELECT user, day, service, sum(credits) AS credits FROM spends WHERE day >= ?1
        GROUP BY user, day, service`).bind(since),
    ]),
  );
  const withTotal = (r) => ({ ...r, total: r.input + r.output + r.cache_read + r.cache_write });
  const zero = () => ({
    input: 0, output: 0, cache_read: 0, cache_write: 0, messages: 0, total: 0,
    cost: 0, claude_cost: 0, tripo_cost: 0, tripo_credits: 0,
  });
  const add = (into, r) => {
    for (const k of Object.keys(into)) into[k] += r[k] ?? 0;
    return into;
  };
  const priced = (r) => {
    const c = cost(r.model, r);
    return { ...withTotal(r), cost: c, claude_cost: c };
  };
  const spend = (r) => {
    const c = creditCost(r.service, r.credits);
    const tripo = r.service.startsWith("tripo");
    return { model: r.service, ...zero(), credits: r.credits, cost: c, tripo_cost: tripo ? c : 0, tripo_credits: tripo ? r.credits : 0 };
  };
  const sum = (rows, key) => rows.reduce((total, r) => total + (r[key] ?? 0), 0);
  const days = Array.from({ length: CHART_DAYS }, (_, i) =>
    new Date(Date.parse(since) + i * 86400000).toISOString().slice(0, 10),
  );
  // Someone who has only spent credits is listed too.
  const names = [...new Set([...people.results, ...spent.results].map((r) => r.user))];
  const users = names.map((user) => {
    const p = people.results.find((r) => r.user === user);
    const services = spent.results.filter((s) => s.user === user);
    const mine = [
      ...models.results
        .filter((m) => m.user === user)
        .map(priced)
        .map(({ user, ...m }) => m)
        .sort((a, b) => b.total - a.total),
      ...services.map(spend).sort((a, b) => b.cost - a.cost),
    ];
    const byDay = new Map();
    const on = (day) => byDay.get(day) ?? byDay.set(day, zero()).get(day);
    for (const d of daily.results.filter((d) => d.user === user)) add(on(d.day), priced(d));
    for (const d of spentDaily.results.filter((d) => d.user === user)) add(on(d.day), spend(d));
    const spans = [p, ...services].filter(Boolean);
    return {
      ...(p ? withTotal(p) : { user, ...zero() }),
      first_day: spans.map((s) => s.first_day).sort()[0],
      last_day: spans.map((s) => s.last_day).sort().at(-1),
      cost: sum(mine, "cost"),
      claude_cost: sum(mine, "claude_cost"),
      tripo_cost: sum(mine, "tripo_cost"),
      tripo_credits: sum(mine, "tripo_credits"),
      models: mine,
      daily: days.map((day) => ({ day, ...(byDay.get(day) ?? zero()) })),
    };
  });
  users.sort((a, b) => b.cost - a.cost);
  const totals = users.reduce(add, zero());
  return { version: SUMMARY_VERSION, generated_at: today.toISOString(), days: CHART_DAYS, totals, users };
}

// Create the tables the first time they are missing, so a deploy needs no migration step.
async function withTables(env, query) {
  try {
    return await query();
  } catch (err) {
    if (!String(err?.message).includes("no such table")) throw err;
    await env.DB.batch(SCHEMA.map((sql) => env.DB.prepare(sql)));
    return await query();
  }
}

function json(value, status = 200, headers = {}) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json", ...headers },
  });
}
