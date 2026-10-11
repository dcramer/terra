// Records Claude token usage per person, day and model. See ../README.md.
//
// POST /report  {user, rows: [{day, model, input, output, cache_read, cache_write, messages}]}
// GET  /stats?group=user,day,model&since=YYYY-MM-DD
//
// Both need `Authorization: Bearer <API_KEY>`.

const USER = /^[A-Za-z0-9_.-]{1,40}$/;
const DAY = /^\d{4}-\d{2}-\d{2}$/;
const MODEL = /^[A-Za-z0-9_.:@/-]{1,100}$/;
const COUNTS = ["input", "output", "cache_read", "cache_write", "messages"];
const GROUPS = ["user", "day", "model"];
const MAX_ROWS = 2000;

const SCHEMA = `CREATE TABLE IF NOT EXISTS usage (
  user TEXT NOT NULL,
  day TEXT NOT NULL,
  model TEXT NOT NULL,
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens INTEGER NOT NULL DEFAULT 0,
  cache_write_tokens INTEGER NOT NULL DEFAULT 0,
  messages INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user, day, model)
)`;

// The reporter sends running totals, so a row only grows: keep the larger of old
// and new, and skip the write when nothing grew. One statement for every row,
// since the free plan allows 50 queries a request and 100 bound values a query.
const UPSERT = `INSERT INTO usage (user, day, model, input_tokens, output_tokens,
    cache_read_tokens, cache_write_tokens, messages, updated_at)
  SELECT ?1, r.value ->> 'day', r.value ->> 'model', r.value ->> 'input', r.value ->> 'output',
    r.value ->> 'cache_read', r.value ->> 'cache_write', r.value ->> 'messages', ?3
  FROM json_each(?2) AS r WHERE true
  ON CONFLICT (user, day, model) DO UPDATE SET
    input_tokens = max(usage.input_tokens, excluded.input_tokens),
    output_tokens = max(usage.output_tokens, excluded.output_tokens),
    cache_read_tokens = max(usage.cache_read_tokens, excluded.cache_read_tokens),
    cache_write_tokens = max(usage.cache_write_tokens, excluded.cache_write_tokens),
    messages = max(usage.messages, excluded.messages),
    updated_at = excluded.updated_at
  WHERE excluded.input_tokens > usage.input_tokens
    OR excluded.output_tokens > usage.output_tokens
    OR excluded.cache_read_tokens > usage.cache_read_tokens
    OR excluded.cache_write_tokens > usage.cache_write_tokens
    OR excluded.messages > usage.messages`;

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
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
  if (!Array.isArray(body.rows) || body.rows.length > MAX_ROWS) throw new BadRequest("bad rows");
  const rows = body.rows.map(cleanRow);
  if (rows.length === 0) return json({ rows: 0, written: 0 });

  const result = await withTable(env, () =>
    env.DB.prepare(UPSERT).bind(body.user, JSON.stringify(rows), new Date().toISOString()).run(),
  );
  return json({ rows: rows.length, written: result.meta.changes });
}

function cleanRow(row) {
  if (typeof row?.day !== "string" || !DAY.test(row.day)) throw new BadRequest("bad day");
  if (typeof row.model !== "string" || !MODEL.test(row.model)) throw new BadRequest("bad model");
  const clean = { day: row.day, model: row.model };
  for (const key of COUNTS) {
    const n = row[key] ?? 0;
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
  const { results } = await withTable(env, () =>
    env.DB.prepare(
      `SELECT ${cols},
        sum(input_tokens) AS input, sum(output_tokens) AS output,
        sum(cache_read_tokens) AS cache_read, sum(cache_write_tokens) AS cache_write,
        sum(messages) AS messages, max(updated_at) AS updated_at
      FROM usage WHERE day >= ?1 GROUP BY ${cols} ORDER BY ${cols}`,
    )
      .bind(since)
      .all(),
  );
  return json({ since, group, rows: results });
}

// Create the table the first time it is missing, so a deploy needs no migration step.
async function withTable(env, query) {
  try {
    return await query();
  } catch (err) {
    if (!String(err?.message).includes("no such table")) throw err;
    await env.DB.prepare(SCHEMA).run();
    return await query();
  }
}

function json(value, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
