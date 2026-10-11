#!/usr/bin/env node
// Sends this person's Claude Code token usage on this repository to the stats worker.
// See README.md. No dependencies: Node 18 or later.
//
//   node tools/token-stats/report.mjs             send messages not yet sent
//   node tools/token-stats/report.mjs --full      send every message again
//   node tools/token-stats/report.mjs --dry-run   print this person's totals, sending nothing
//   node tools/token-stats/report.mjs --stats     print everyone's totals
//   node tools/token-stats/report.mjs --cron      print a crontab line that runs it hourly

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const BATCH = 200;

// Adds the messages of one Claude Code transcript to `messages`, a map from message id.
//
// A transcript counts only if its session started inside one of `roots`, wherever it
// went after; Claude Code files a session under the folder it started in.
//
// Each message has a unique id from the API, and that id is the only thing that
// identifies it. One message is logged once per content block, with partial usage
// on the early lines, and again in every session resumed or forked from its own,
// under the new session's id and sometimes a later timestamp. So each message keeps
// the largest of each count and the earliest day, and the server keys on the id too.
export function addTranscript(text, roots, messages) {
  let started = null;
  for (const line of text.split("\n")) {
    if (started === null && !line.includes('"cwd"')) continue;
    if (started !== null && !line.includes('"usage"')) continue;
    let entry;
    try {
      entry = JSON.parse(line);
    } catch {
      continue;
    }
    if (started === null) {
      if (typeof entry?.cwd !== "string") continue;
      started = entry.cwd;
      if (!roots.some((root) => inside(started, root))) return;
    }
    const message = entry?.message;
    const usage = message?.usage;
    if (entry.type !== "assistant" || !usage || typeof message.id !== "string") continue;
    if (typeof message.model !== "string" || message.model.startsWith("<")) continue;
    const day = typeof entry.timestamp === "string" ? entry.timestamp.slice(0, 10) : "";
    if (!/^\d{4}-\d{2}-\d{2}$/.test(day)) continue;
    const session = typeof entry.sessionId === "string" ? entry.sessionId : "";

    const counts = {
      input: usage.input_tokens ?? 0,
      output: usage.output_tokens ?? 0,
      cache_read: usage.cache_read_input_tokens ?? 0,
      cache_write: usage.cache_creation_input_tokens ?? 0,
    };
    const known = messages.get(message.id);
    if (!known) {
      messages.set(message.id, { id: message.id, session, day, model: message.model, ...counts });
      continue;
    }
    for (const [key, n] of Object.entries(counts)) known[key] = Math.max(known[key], n);
    if (day < known.day || (day === known.day && session < known.session)) {
      known.day = day;
      known.session = session;
    }
  }
}

function inside(cwd, root) {
  return cwd === root || cwd.startsWith(root + sep);
}

function git(...args) {
  return execFileSync("git", args, { cwd: HERE, encoding: "utf8" }).trim();
}

// The main checkout's root, so sessions in its worktrees count too.
function repoRoot() {
  return dirname(resolve(git("rev-parse", "--path-format=absolute", "--git-common-dir")));
}

function checkoutRoot() {
  return resolve(git("rev-parse", "--show-toplevel"));
}

// The main checkout and any worktrees kept outside it.
function roots() {
  const listed = git("worktree", "list", "--porcelain")
    .split("\n")
    .filter((l) => l.startsWith("worktree "))
    .map((l) => resolve(l.slice(9)));
  const all = [repoRoot(), ...listed];
  return [...new Set(all)].filter((r) => !all.some((o) => o !== r && inside(r, o)));
}

// Claude Code names each project's folder after the path a session started in, with
// every other character replaced by "-". That can match other folders too, such as
// "terra-old"; addTranscript checks the real path.
function transcripts(roots) {
  const projects = join(process.env.CLAUDE_CONFIG_DIR ?? join(homedir(), ".claude"), "projects");
  if (!existsSync(projects)) return [];
  const prefixes = roots.map((root) => root.replace(/[^A-Za-z0-9]/g, "-"));
  const files = [];
  const walk = (dir) => {
    for (const item of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, item.name);
      if (item.isDirectory()) walk(path);
      else if (item.name.endsWith(".jsonl")) files.push(path);
    }
  };
  for (const item of readdirSync(projects, { withFileTypes: true })) {
    if (item.isDirectory() && prefixes.some((p) => item.name.startsWith(p))) walk(join(projects, item.name));
  }
  return files;
}

export function collect(roots) {
  const messages = new Map();
  for (const file of transcripts(roots)) addTranscript(readFileSync(file, "utf8"), roots, messages);
  return [...messages.values()].sort((a, b) => a.day.localeCompare(b.day) || a.id.localeCompare(b.id));
}

export function totals(messages) {
  const rows = new Map();
  for (const m of messages) {
    const key = `${m.day}\t${m.model}`;
    const row = rows.get(key) ?? { day: m.day, model: m.model, input: 0, output: 0, cache_read: 0, cache_write: 0, messages: 0 };
    for (const k of ["input", "output", "cache_read", "cache_write"]) row[k] += m[k];
    row.messages += 1;
    rows.set(key, row);
  }
  return [...rows.values()];
}

// What has been sent, so a run sends only new or grown messages. Losing it costs a
// longer run, never a wrong count: the server ignores what it already has.
function sentFile(cfg) {
  const name = createHash("sha256").update(`${cfg.url}\n${cfg.user}`).digest("hex").slice(0, 16);
  const cache = process.env.XDG_CACHE_HOME ?? join(homedir(), ".cache");
  return join(cache, "terra-token-stats", `sent-${name}.json`);
}

function signature(m) {
  return [m.day, m.input, m.output, m.cache_read, m.cache_write].join(" ");
}

// Reads KEY=value lines, ignoring blanks and # comments; a value may be quoted.
export function parseEnv(text) {
  const vars = {};
  for (const raw of text.split(/\r?\n/)) {
    const m = raw.match(/^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$/);
    if (!m) continue;
    let value = m[2];
    const quoted = value.match(/^(["'])(.*)\1$/);
    if (quoted) value = quoted[2];
    else value = value.replace(/\s+#.*$/, "");
    vars[m[1]] = value;
  }
  return vars;
}

// Settings come from the environment, then `.env` at the root of this checkout,
// then `.env` in the main checkout, so one file serves every worktree.
function config() {
  const files = [...new Set([checkoutRoot(), repoRoot()])].map((root) => join(root, ".env"));
  const sources = [process.env, ...files.filter(existsSync).map((f) => parseEnv(readFileSync(f, "utf8")))];
  const get = (name) => sources.map((s) => s[name]).find(Boolean);
  const cfg = { url: get("TERRA_STATS_URL"), key: get("TERRA_STATS_KEY"), user: get("TERRA_STATS_USER") };
  const missing = Object.keys(cfg).filter((k) => !cfg[k]);
  if (missing.length > 0) {
    throw new Error(`missing ${missing.join(", ")}: copy .env.example to .env at the repository root and fill it in`);
  }
  cfg.url = cfg.url.replace(/\/+$/, "");
  return cfg;
}

async function call(cfg, path, init = {}) {
  const res = await fetch(cfg.url + path, {
    ...init,
    headers: { Authorization: `Bearer ${cfg.key}`, "Content-Type": "application/json" },
  });
  const body = await res.text();
  if (!res.ok) throw new Error(`${path}: ${res.status} ${body}`);
  return JSON.parse(body);
}

function table(rows, cols) {
  const fmt = (v) => (typeof v === "number" ? v.toLocaleString("en-US") : String(v ?? ""));
  const cells = [cols, ...rows.map((r) => cols.map((c) => fmt(r[c])))];
  const widths = cols.map((_, i) => Math.max(...cells.map((r) => r[i].length)));
  return cells.map((r) => r.map((c, i) => (i === 0 ? c.padEnd(widths[i]) : c.padStart(widths[i]))).join("  ")).join("\n");
}

async function main(args) {
  const counts = ["input", "output", "cache_read", "cache_write", "messages"];
  if (args.includes("--cron")) {
    const log = join(homedir(), ".cache", "terra-token-stats.log");
    console.log(`0 * * * * ${process.execPath} ${join(repoRoot(), "tools", "token-stats", "report.mjs")} >> ${log} 2>&1`);
    return;
  }
  if (args.includes("--stats")) {
    const group = args.includes("--by-day") ? "user,day" : args.includes("--by-model") ? "user,model" : "user";
    const { rows } = await call(config(), `/stats?group=${group}`);
    console.log(table(rows, [...group.split(","), ...counts]));
    return;
  }
  const messages = collect(roots());
  if (args.includes("--dry-run")) {
    console.log(table(totals(messages), ["day", "model", ...counts]));
    return;
  }

  const cfg = config();
  const file = sentFile(cfg);
  let sent = {};
  if (!args.includes("--full") && existsSync(file)) {
    try {
      sent = JSON.parse(readFileSync(file, "utf8"));
    } catch {
      sent = {};
    }
  }
  const pending = messages.filter((m) => sent[m.id] !== signature(m));
  const save = () => {
    // Forget messages no longer in any transcript, so the file does not grow forever.
    const kept = Object.fromEntries(messages.filter((m) => m.id in sent).map((m) => [m.id, sent[m.id]]));
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(`${file}.tmp`, JSON.stringify(kept));
    renameSync(`${file}.tmp`, file);
  };
  for (let i = 0; i < pending.length; i += BATCH) {
    const batch = pending.slice(i, i + BATCH);
    await call(cfg, "/report", { method: "POST", body: JSON.stringify({ user: cfg.user, messages: batch }) });
    for (const m of batch) sent[m.id] = signature(m);
    save();
  }
  console.log(`${new Date().toISOString()} sent ${pending.length} of ${messages.length} messages as ${cfg.user}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main(process.argv.slice(2)).catch((err) => {
    console.error(`token-stats: ${err.message}`);
    process.exit(1);
  });
}
