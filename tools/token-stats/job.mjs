#!/usr/bin/env node
// What a job's context cost, measured (D390): for each transcript, the context when it first
// edited a file, its summed input (every call reads the whole context again), its own output,
// what its tool results really took and how much of the summed input they carried, by what was
// read. The yardstick for making agent work cheaper (research/2026-11-05-agent-pipeline-tokens.md).
// See README.md. No dependencies.
//
//   node tools/token-stats/job.mjs PATH…           these transcripts, or the .jsonl files under these folders
//   node tools/token-stats/job.mjs --job REGEX     subagents whose task (.meta.json, or a job's brief's first line) matches, e.g. 'char-switch j[789]'
//   … --json                                       the same as JSON
//
// A transcript does not count the tokens of a tool result, so context.mjs estimates them at four
// characters a token. Here they are measured: the context grows between two calls by the earlier
// call's output plus what came back, and that growth is shared among what came back by its
// estimates. On our code and logs it is about 1.6 times the estimate.

import { closeSync, existsSync, fstatSync, openSync, readdirSync, readFileSync, readSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { firstPrompt, parseTranscript } from "./context.mjs";

const EDITS = new Set(["Edit", "Write", "NotebookEdit", "MultiEdit"]);
const ORIENT = /(^|[\s/"'=])(\.agents\/skills|\.claude\/skills|docs|research)\/|terra-handoff|AGENTS\.md|CLAUDE\.md|README\.md|xtask decision/;
const SOURCE = /\.(rs|ts|mjs|js|py|sh|wesl|wgsl|json|ron|toml)\b/;
const READS = /^\s*(sed|cat|head|tail|awk|less|nl)\b/;
const SEARCHES = /^\s*(grep|rg|find|ls|wc|tree|fd)\b/;

// What a tool result was, for the table: the groups the research's audit used.
export function readKind(item) {
  if (item.kind !== "tool") return item.kind === "attachment" ? "added by Claude Code" : "prompts and notices";
  if (item.images) return "images";
  if (item.tool === "Skill") return "orientation docs";
  if (item.tool === "Read") {
    const p = item.path ?? "";
    if (/\.log$|\/out\/|target\//.test(p)) return "tool output and logs";
    if (ORIENT.test(p)) return "orientation docs";
    return SOURCE.test(p) ? "our source" : "other";
  }
  if (item.tool === "Grep" || item.tool === "Glob") return "search";
  if (item.tool === "Agent") return item.line === "finder" ? "search" : "subagents";
  if (item.tool === "Bash" || /device_bash$/.test(item.tool ?? "")) {
    const line = (item.line ?? "").replace(/^\s*cd [^&;]+(&&|;)\s*/, "");
    if (/^\s*git\b/.test(line)) return "git";
    if (SEARCHES.test(line)) return "search";
    if (READS.test(line)) {
      if (/\.log\b|\/out\/|target\//.test(line)) return "tool output and logs";
      if (ORIENT.test(line)) return "orientation docs";
      return SOURCE.test(line) ? "our source" : "other";
    }
    if (/xtask decision/.test(line)) return "orientation docs";
    return "tool output and logs";
  }
  if (EDITS.has(item.tool)) return "edits";
  return "other";
}

// One transcript's numbers, from context.mjs's parse.
export function measure(t) {
  const calls = t.calls;
  const n = calls.length;
  const ctx = calls.map((c) => c.input + c.cache_read + c.cache_write);
  const firstEdit = calls.findIndex((c) => c.uses.some((u) => EDITS.has(u)));
  const items = [];
  let estimated = 0;
  let measured = 0;
  for (const [i, call] of calls.entries()) {
    const here = call.added.filter((a) => a.tokens > 0);
    const sum = here.reduce((s, a) => s + a.tokens, 0);
    const gap = i > 0 ? ctx[i] - ctx[i - 1] - calls[i - 1].output : 0;
    // Measured when the context grew by more than the call before wrote; otherwise (the first
    // call, a compaction) the estimate stands.
    const scale = i > 0 && gap > 0 && sum > 0 ? gap / sum : 1;
    for (const a of here) {
      const tokens = Math.round(a.tokens * scale);
      if (a.kind === "tool") {
        estimated += a.tokens;
        measured += tokens;
      }
      items.push({ what: readKind(a), tool: a.kind === "tool", tokens, at: i, carried: tokens * (n - i) });
    }
  }
  const summed = ctx.reduce((s, c) => s + c, 0);
  const groups = new Map();
  for (const it of items) {
    if (!it.tool) continue;
    const g = groups.get(it.what) ?? { what: it.what, calls: 0, tokens: 0, carried: 0 };
    g.calls += 1;
    g.tokens += it.tokens;
    g.carried += it.carried;
    groups.set(it.what, g);
  }
  const before = firstEdit < 0 ? [] : items.filter((it) => it.tool && it.at <= firstEdit);
  return {
    turns: n,
    prefix: ctx[0] ?? 0,
    firstEdit: firstEdit < 0 ? null : ctx[firstEdit],
    firstEditTurn: firstEdit < 0 ? null : firstEdit + 1,
    peak: n ? Math.max(...ctx) : 0,
    final: ctx[n - 1] ?? 0,
    summed,
    output: calls.reduce((s, c) => s + c.output, 0),
    results: items.filter((it) => it.tool).reduce((s, it) => s + it.tokens, 0),
    readBeforeEdit: before.reduce((s, it) => s + it.tokens, 0),
    carriedBeforeEdit: before.reduce((s, it) => s + it.carried, 0),
    calibration: estimated > 0 ? measured / estimated : null,
    groups: [...groups.values()].sort((a, b) => b.tokens - a.tokens),
  };
}

// The mean of several jobs' numbers, groups included.
export function mean(jobs) {
  const keys = ["turns", "prefix", "firstEdit", "peak", "final", "summed", "output", "results", "readBeforeEdit", "carriedBeforeEdit"];
  const m = {};
  for (const key of keys) {
    const vals = jobs.map((j) => j[key]).filter((v) => v !== null);
    m[key] = vals.length ? Math.round(vals.reduce((s, v) => s + v, 0) / vals.length) : null;
  }
  const groups = new Map();
  for (const j of jobs) {
    for (const g of j.groups) {
      const r = groups.get(g.what) ?? { what: g.what, calls: 0, tokens: 0, carried: 0 };
      r.calls += g.calls / jobs.length;
      r.tokens += g.tokens / jobs.length;
      r.carried += g.carried / jobs.length;
      groups.set(g.what, r);
    }
  }
  m.groups = [...groups.values()].sort((a, b) => b.tokens - a.tokens);
  return m;
}

const k = (n) => (n === null ? "-" : n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${Math.round(n / 1e3)}k` : String(Math.round(n)));
const pct = (a, b) => (b > 0 ? `${Math.round((100 * a) / b)}%` : "-");

export function format(rows) {
  const cols = ["job", "turns", "first edit", "peak", "summed", "output", "results", "carried before edit"];
  const line = (name, r) => [name, String(r.turns), k(r.firstEdit), k(r.peak), k(r.summed), k(r.output), k(r.results), `${k(r.carriedBeforeEdit)} (${pct(r.carriedBeforeEdit, r.summed)})`];
  const cells = [cols, ...rows.map((r) => line(r.name, r.m))];
  const m = rows.length > 1 ? mean(rows.map((r) => r.m)) : rows[0]?.m;
  if (rows.length > 1) cells.push(line("mean", m));
  const widths = cols.map((_, i) => Math.max(...cells.map((c) => c[i].length)));
  const out = cells.map((c) => c.map((x, i) => (i === 0 ? x.padEnd(widths[i]) : x.padStart(widths[i]))).join("  "));
  if (!m) return "No API calls in these transcripts.";
  out.push("", `Tool results by what was read${rows.length > 1 ? ", a job on average" : ""}: tokens (share), calls, carried`);
  for (const g of m.groups.slice(0, 9)) out.push(`  ${g.what.padEnd(22)} ${k(g.tokens).padStart(5)} (${pct(g.tokens, m.results).padStart(3)})  ${String(Math.round(g.calls)).padStart(3)} calls  ${k(g.carried).padStart(5)}`);
  const cal = rows.map((r) => r.m.calibration).filter((c) => c !== null);
  if (cal.length) out.push(`  (measured from the context's growth: ${(cal.reduce((s, c) => s + c, 0) / cal.length).toFixed(2)}x context.mjs's estimate at 4 characters a token)`);
  return out.join("\n");
}

function jsonlFiles(path) {
  if (!statSync(path).isDirectory()) return [path];
  return readdirSync(path, { withFileTypes: true }).flatMap((d) => (d.isDirectory() ? jsonlFiles(join(path, d.name)) : d.name.endsWith(".jsonl") ? [join(path, d.name)] : []));
}

// The first line of a transcript's brief ("Task: token-policy j1. Read …"), from its head.
function briefLine(transcript) {
  const fd = openSync(transcript, "r");
  try {
    const buf = Buffer.alloc(Math.min(fstatSync(fd).size, 16 * 1024));
    readSync(fd, buf, 0, buf.length, 0);
    return (firstPrompt(buf.toString("utf8")) ?? "").split("\n")[0].slice(0, 200);
  } finally {
    closeSync(fd);
  }
}

// Subagent transcripts whose task matches, by the Agent call's description or the first
// line of the brief, with the task as their name.
export function findJobs(projects, pattern) {
  const re = new RegExp(pattern, "i");
  const found = [];
  if (!existsSync(projects)) return found;
  for (const project of readdirSync(projects)) {
    const dir = join(projects, project);
    if (!statSync(dir).isDirectory()) continue;
    for (const session of readdirSync(dir)) {
      const sub = join(dir, session, "subagents");
      if (!existsSync(sub)) continue;
      for (const f of readdirSync(sub)) {
        if (!f.endsWith(".meta.json")) continue;
        let meta;
        try {
          meta = JSON.parse(readFileSync(join(sub, f), "utf8"));
        } catch {
          continue;
        }
        const transcript = join(sub, f.replace(/\.meta\.json$/, ".jsonl"));
        if (!existsSync(transcript)) continue;
        if (re.test(meta.description ?? "")) found.push({ name: meta.description, file: transcript });
        else if (meta.agentType === "job" && re.test(briefLine(transcript))) found.push({ name: meta.description, file: transcript });
      }
    }
  }
  return found.sort((a, b) => statSync(a.file).mtimeMs - statSync(b.file).mtimeMs);
}

function main(args) {
  const i = args.indexOf("--job");
  const projects = join(process.env.CLAUDE_CONFIG_DIR ?? join(homedir(), ".claude"), "projects");
  let jobs;
  if (i >= 0) {
    jobs = findJobs(projects, args[i + 1] ?? "");
    if (!jobs.length) {
      console.log(`✗ no subagent under ${projects} has a task matching /${args[i + 1]}/: name part of the job's description (the Agent call's), e.g. --job 'char-pipeline j[1-3]', or pass its transcript.`);
      process.exitCode = 1;
      return;
    }
  } else {
    const paths = args.filter((a, j) => !a.startsWith("--") && args[j - 1] !== "--job");
    if (!paths.length) {
      console.log("Usage: node tools/token-stats/job.mjs PATH… | --job REGEX [--json] (see the file's header)");
      process.exitCode = 1;
      return;
    }
    jobs = paths.flatMap((p) => jsonlFiles(resolve(p))).map((file) => ({ name: basename(file, ".jsonl"), file }));
  }
  const rows = jobs.map((j) => ({ name: j.name.slice(0, 40), m: measure(parseTranscript(readFileSync(j.file, "utf8"), j.file)) }));
  if (args.includes("--json")) console.log(JSON.stringify({ jobs: rows, mean: mean(rows.map((r) => r.m)) }, null, 1));
  else console.log(format(rows));
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) main(process.argv.slice(2));
