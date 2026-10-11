// Records Claude token usage per message, and totals per person, day and model.
// See ../README.md.
//
// GET  /       the page: tokens per person, public
// GET  /json   the same as JSON, public
// POST /report {user, messages: [{id, session, day, model, input, output, cache_read, cache_write}]}
// GET  /stats  ?group=user,day,model&since=YYYY-MM-DD
//
// /report and /stats need `Authorization: Bearer <API_KEY>`.

import { page } from "./page.js";
import { cost } from "./prices.js";

const USER = /^[A-Za-z0-9_.-]{1,40}$/;
const ID = /^[A-Za-z0-9_-]{1,100}$/;
const SESSION = /^[A-Za-z0-9_-]{0,100}$/;
const DAY = /^\d{4}-\d{2}-\d{2}$/;
const MODEL = /^[A-Za-z0-9_.:@/-]{1,100}$/;
const COUNTS = ["input", "output", "cache_read", "cache_write"];
const GROUPS = ["user", "day", "model"];
const MAX_MESSAGES = 500;
const CHART_DAYS = 30;

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

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/") {
      return new Response(page(await summary(env)), { headers: { "Content-Type": "text/html; charset=utf-8", ...PUBLIC } });
    }
    if (request.method === "GET" && url.pathname === "/json") {
      return json(await summary(env), 200, PUBLIC);
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
};

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
  if (!Array.isArray(body.messages) || body.messages.length > MAX_MESSAGES) throw new BadRequest("bad messages");
  const messages = body.messages.map(cleanMessage);
  if (messages.length === 0) return json({ messages: 0, rows_written: 0 });

  const result = await withTables(env, () =>
    env.DB.prepare(UPSERT).bind(body.user, JSON.stringify(messages), new Date().toISOString()).run(),
  );
  return json({ messages: messages.length, rows_written: result.meta.changes });
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

// Short, so the page's minute-by-minute refresh sees new numbers.
const PUBLIC = { "Cache-Control": "public, max-age=30" };
const SUMS = `sum(input_tokens) AS input, sum(output_tokens) AS output,
  sum(cache_read_tokens) AS cache_read, sum(cache_write_tokens) AS cache_write, sum(messages) AS messages`;

// Everyone's totals, their models, and tokens a day for the last CHART_DAYS days, each
// with `cost`: what the tokens would cost at API list prices, in dollars (prices.js).
async function summary(env) {
  const today = new Date();
  const since = new Date(today.getTime() - (CHART_DAYS - 1) * 86400000).toISOString().slice(0, 10);
  const [people, models, daily] = await withTables(env, () =>
    env.DB.batch([
      env.DB.prepare(`SELECT user, ${SUMS}, min(day) AS first_day, max(day) AS last_day
        FROM usage WHERE messages > 0 GROUP BY user`),
      env.DB.prepare(`SELECT user, model, ${SUMS} FROM usage WHERE messages > 0 GROUP BY user, model`),
      env.DB.prepare(`SELECT user, day, model, ${SUMS} FROM usage WHERE day >= ?1 GROUP BY user, day, model`).bind(since),
    ]),
  );
  const withTotal = (r) => ({ ...r, total: r.input + r.output + r.cache_read + r.cache_write });
  const zero = () => ({ input: 0, output: 0, cache_read: 0, cache_write: 0, messages: 0, total: 0, cost: 0 });
  const add = (into, r) => {
    for (const k of Object.keys(into)) into[k] += r[k];
    return into;
  };
  const priced = (r) => ({ ...withTotal(r), cost: cost(r.model, r) });
  const days = Array.from({ length: CHART_DAYS }, (_, i) =>
    new Date(Date.parse(since) + i * 86400000).toISOString().slice(0, 10),
  );
  const users = people.results.map(withTotal).map((p) => {
    const mine = models.results
      .filter((m) => m.user === p.user)
      .map(priced)
      .map(({ user, ...m }) => m)
      .sort((a, b) => b.total - a.total);
    const byDay = new Map();
    for (const d of daily.results.filter((d) => d.user === p.user)) {
      add(byDay.get(d.day) ?? byDay.set(d.day, zero()).get(d.day), priced(d));
    }
    return {
      ...p,
      cost: mine.reduce((sum, m) => sum + m.cost, 0),
      models: mine,
      daily: days.map((day) => ({ day, ...(byDay.get(day) ?? zero()) })),
    };
  });
  users.sort((a, b) => b.cost - a.cost);
  const totals = users.reduce(add, zero());
  return { generated_at: today.toISOString(), days: CHART_DAYS, totals, users };
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
