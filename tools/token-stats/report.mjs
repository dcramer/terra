#!/usr/bin/env node
// Sends this person's Claude Code token usage on this repository to the stats worker.
// See README.md. No dependencies: Node 18 or later.
//
//   node tools/token-stats/report.mjs             send totals
//   node tools/token-stats/report.mjs --dry-run   print what would be sent
//   node tools/token-stats/report.mjs --stats     print everyone's totals
//   node tools/token-stats/report.mjs --cron      print a crontab line that runs it hourly

import { execFileSync } from "node:child_process";
import { createReadStream, existsSync, readdirSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve, sep } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath, pathToFileURL } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const CONFIG_FILE = join(homedir(), ".config", "terra", "token-stats.json");

// Adds one Claude Code transcript line to `totals`, a map from "day\tmodel" to counts.
// Counts only assistant messages whose working directory is inside `root`. A message
// appears once per content block, and again in resumed sessions, so `seen` holds the
// message ids already counted.
export function addLine(line, root, seen, totals) {
  if (!line.includes('"usage"')) return;
  let entry;
  try {
    entry = JSON.parse(line);
  } catch {
    return;
  }
  const message = entry?.message;
  const usage = message?.usage;
  if (entry.type !== "assistant" || !usage || !inside(entry.cwd, root)) return;
  const model = message.model;
  if (typeof model !== "string" || model.startsWith("<")) return;
  const id = message.id ?? entry.requestId;
  if (id) {
    if (seen.has(id)) return;
    seen.add(id);
  }
  const day = typeof entry.timestamp === "string" ? entry.timestamp.slice(0, 10) : null;
  if (!day || !/^\d{4}-\d{2}-\d{2}$/.test(day)) return;

  const key = `${day}\t${model}`;
  const row = totals.get(key) ?? { day, model, input: 0, output: 0, cache_read: 0, cache_write: 0, messages: 0 };
  row.input += usage.input_tokens ?? 0;
  row.output += usage.output_tokens ?? 0;
  row.cache_read += usage.cache_read_input_tokens ?? 0;
  row.cache_write += usage.cache_creation_input_tokens ?? 0;
  row.messages += 1;
  totals.set(key, row);
}

function inside(cwd, root) {
  return typeof cwd === "string" && (cwd === root || cwd.startsWith(root + sep));
}

// The main checkout's root, so sessions in its worktrees count too.
function repoRoot() {
  const common = execFileSync("git", ["rev-parse", "--path-format=absolute", "--git-common-dir"], {
    cwd: HERE,
    encoding: "utf8",
  }).trim();
  return dirname(resolve(common));
}

// Claude Code names each project's folder after its path with every other character
// replaced by "-", so the repository and its worktrees share a prefix.
function transcripts(root) {
  const projects = join(process.env.CLAUDE_CONFIG_DIR ?? join(homedir(), ".claude"), "projects");
  if (!existsSync(projects)) return [];
  const prefix = root.replace(/[^A-Za-z0-9]/g, "-");
  const files = [];
  const walk = (dir) => {
    for (const item of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, item.name);
      if (item.isDirectory()) walk(path);
      else if (item.name.endsWith(".jsonl")) files.push(path);
    }
  };
  for (const item of readdirSync(projects, { withFileTypes: true })) {
    if (item.isDirectory() && item.name.startsWith(prefix)) walk(join(projects, item.name));
  }
  return files;
}

export async function collect(root) {
  const seen = new Set();
  const totals = new Map();
  for (const file of transcripts(root)) {
    const lines = createInterface({ input: createReadStream(file), crlfDelay: Infinity });
    for await (const line of lines) addLine(line, root, seen, totals);
  }
  return [...totals.values()].sort((a, b) => a.day.localeCompare(b.day) || a.model.localeCompare(b.model));
}

function config() {
  let file = {};
  if (existsSync(CONFIG_FILE)) file = JSON.parse(readFileSync(CONFIG_FILE, "utf8"));
  const cfg = {
    url: process.env.TERRA_STATS_URL ?? file.url,
    key: process.env.TERRA_STATS_KEY ?? file.key,
    user: process.env.TERRA_STATS_USER ?? file.user,
  };
  const missing = Object.keys(cfg).filter((k) => !cfg[k]);
  if (missing.length > 0) {
    throw new Error(`missing ${missing.join(", ")}: set them in ${CONFIG_FILE} or as TERRA_STATS_URL, _KEY, _USER`);
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
  const rows = await collect(repoRoot());
  if (args.includes("--dry-run")) {
    console.log(table(rows, ["day", "model", ...counts]));
    return;
  }
  const cfg = config();
  const result = await call(cfg, "/report", { method: "POST", body: JSON.stringify({ user: cfg.user, rows }) });
  console.log(`${new Date().toISOString()} sent ${result.rows} rows as ${cfg.user}, ${result.written} changed`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main(process.argv.slice(2)).catch((err) => {
    console.error(`token-stats: ${err.message}`);
    process.exit(1);
  });
}
