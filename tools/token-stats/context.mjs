#!/usr/bin/env node
// Where the context goes (D390): reads the same Claude Code transcripts as report.mjs and
// says how big each session's context grew, what each large cache write was most likely
// for, and which tool results took the most of it. See README.md. No dependencies.
//
//   node tools/token-stats/context.mjs                  this repository's sessions
//   node tools/token-stats/context.mjs --days 2         only API calls of the last two days
//   node tools/token-stats/context.mjs --session ID     one session (an id's prefix) and its subagents
//   node tools/token-stats/context.mjs PATH…            these transcripts, or the .jsonl files under these folders
//   node tools/token-stats/context.mjs --hook           the context nudge, run by Claude Code after each
//                                                       tool call (.claude/settings.json)

import { closeSync, existsSync, fstatSync, mkdirSync, openSync, readdirSync, readFileSync, readSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

// Tokens a character of text, roughly, for tool results and other text the transcript
// keeps without counting. The report prints how far the estimates fall short of the
// cache writes measured, so it is clear how rough they are.
const CHARS_PER_TOKEN = 4;
// Attachments the transcript logs that are not sent to the model.
const UNSENT = new Set(["prompt_snapshot", "deferred_tools_record", "credential_org", "remote_session_change"]);
// How long a cache entry lives, by the kind the call wrote.
const TTL_MS = { "5m": 5 * 60 * 1000, "1h": 60 * 60 * 1000 };

// The nudge holds each agent to its context bound (D627): a job to the bound its brief names
// ("Bound: stop at 150k context"), any other session to DEFAULT_BOUND. It warns once at
// WARN_AT of the bound (finish the step, start nothing new), then tells it to stop at the
// bound and again at each STEP_PAST more. Each turn rereads the whole context, so a turn
// past the bound costs more than the handoff that would end it.
export const DEFAULT_BOUND = 150_000;
export const WARN_AT = 0.8;
export const STEP_PAST = 20_000;
const NUDGE_STATE = join(process.env.XDG_CACHE_HOME ?? join(homedir(), ".cache"), "terra-token-stats", "nudge");

export function estimate(chars) {
  return Math.ceil(chars / CHARS_PER_TOKEN);
}

// Tokens an image costs, from its size: scaled to fit 1568 pixels on its long side, a token
// every 750 pixels. Unknown sizes count as the most a full-size image costs.
export function imageTokens(base64) {
  const size = imageSize(base64);
  if (!size) return 1600;
  const scale = Math.min(1, 1568 / Math.max(size.width, size.height));
  return Math.ceil((size.width * scale * (size.height * scale)) / 750);
}

function imageSize(base64) {
  if (typeof base64 !== "string") return null;
  const head = Buffer.from(base64.slice(0, 64), "base64");
  if (head.length >= 24 && head.readUInt32BE(0) === 0x89504e47) return { width: head.readUInt32BE(16), height: head.readUInt32BE(20) };
  if (head[0] !== 0xff || head[1] !== 0xd8) return null;
  const bytes = Buffer.from(base64, "base64");
  for (let i = 2; i + 9 < bytes.length; ) {
    if (bytes[i] !== 0xff) return null;
    const marker = bytes[i + 1];
    if (marker >= 0xc0 && marker <= 0xcf && ![0xc4, 0xc8, 0xcc].includes(marker)) {
      return { height: bytes.readUInt16BE(i + 5), width: bytes.readUInt16BE(i + 7) };
    }
    i += 2 + bytes.readUInt16BE(i + 2);
  }
  return null;
}

// What a tool result's content costs: text by its length, images by their size.
function contentTokens(content) {
  if (typeof content === "string") return { tokens: estimate(content.length), images: 0 };
  let tokens = 0;
  let images = 0;
  for (const part of Array.isArray(content) ? content : []) {
    if (part?.type === "image") {
      tokens += imageTokens(part.source?.data);
      images += 1;
    } else if (typeof part?.text === "string") tokens += estimate(part.text.length);
    else tokens += estimate(JSON.stringify(part ?? "").length);
  }
  return { tokens, images };
}

// A path without the checkout or worktree it was read in, so one file read from several
// worktrees counts as one.
export function shortPath(path, cwd = "") {
  let p = String(path ?? "");
  const worktree = p.match(/^.*?\/\.claude\/worktrees\/[^/]+\/(.*)$/);
  if (worktree) return worktree[1];
  if (cwd && p.startsWith(cwd + "/")) return p.slice(cwd.length + 1);
  const home = homedir();
  return p.startsWith(home + "/") ? "~/" + p.slice(home.length + 1) : p;
}

// What a Bash command is, for grouping: `cargo xtask <task>`, `cargo <command>`, the script
// node or git ran, or the first word, past any `cd …&&`, variables and wrappers.
export function commandKey(command) {
  const parts = String(command ?? "").split(/&&|;|\|\|/).map((s) => s.trim()).filter(Boolean);
  const main = parts.find((p) => !/^(cd|export|source|set)\b/.test(p)) ?? parts[0] ?? "";
  const words = main.split(/\s+/).filter((w) => !/^[A-Za-z_][A-Za-z0-9_]*=/.test(w));
  while (["timeout", "time", "nice", "env", "sudo", "flock"].includes(words[0])) {
    words.shift();
    while (words[0] && (/^-/.test(words[0]) || /^\d/.test(words[0]) || words[0].includes("/") )) words.shift();
  }
  const [first = "", second = "", third = ""] = words.map((w) => w.replace(/^["']|["']$/g, ""));
  const name = basename(first);
  if (name === "cargo" && second === "xtask") return `cargo xtask ${third}`.trim();
  if (name === "cargo" || name === "git" || name === "npm" || name === "npx" || name === "make") return `${name} ${second}`.trim();
  if (name === "node" || name === "python3" || name === "bash" || name === "sh") return `${name} ${basename(second)}`.trim();
  return name;
}

// One transcript, read into its API calls, each with what was added to the context
// between it and the call before it.
export function parseTranscript(text, file = "") {
  const calls = [];
  const byId = new Map();
  const tools = new Map(); // tool_use id -> { name, input }
  let pending = []; // items added since the last call began
  let session = "";
  let cwd = "";
  let agent = null;
  for (const line of text.split("\n")) {
    if (!line) continue;
    let entry;
    try {
      entry = JSON.parse(line);
    } catch {
      continue;
    }
    if (!session && typeof entry.sessionId === "string") session = entry.sessionId;
    if (!cwd && typeof entry.cwd === "string") cwd = entry.cwd;
    if (!agent && typeof entry.agentId === "string") agent = entry.agentId;
    const time = Date.parse(entry.timestamp ?? "");
    const message = entry.message;
    const content = Array.isArray(message?.content) ? message.content : [];

    if (entry.type === "assistant" && message?.usage && typeof message.id === "string") {
      const named = [];
      for (const block of content) {
        if (block?.type === "tool_use") {
          tools.set(block.id, { name: block.name, input: block.input ?? {} });
          named.push(block.name);
        }
      }
      const u = message.usage;
      const known = byId.get(message.id);
      if (known) {
        known.uses.push(...named);
        known.output = Math.max(known.output, u.output_tokens ?? 0);
        known.cache_read = Math.max(known.cache_read, u.cache_read_input_tokens ?? 0);
        known.cache_write = Math.max(known.cache_write, u.cache_creation_input_tokens ?? 0);
        known.input = Math.max(known.input, u.input_tokens ?? 0);
        known.lastTime = Number.isFinite(time) ? time : known.lastTime;
        continue;
      }
      if (typeof message.model !== "string" || message.model.startsWith("<")) continue;
      const write5m = u.cache_creation?.ephemeral_5m_input_tokens ?? 0;
      const call = {
        id: message.id,
        model: message.model,
        time: Number.isFinite(time) ? time : null,
        lastTime: Number.isFinite(time) ? time : null,
        input: u.input_tokens ?? 0,
        output: u.output_tokens ?? 0,
        cache_read: u.cache_read_input_tokens ?? 0,
        cache_write: u.cache_creation_input_tokens ?? 0,
        ttl: write5m > 0 && write5m >= (u.cache_creation?.ephemeral_1h_input_tokens ?? 0) ? "5m" : "1h",
        uses: named, // the tools this call asked for, by name
        added: pending,
      };
      pending = [];
      calls.push(call);
      byId.set(message.id, call);
      continue;
    }

    if (entry.type === "system" && entry.subtype === "compact_boundary") pending.push({ kind: "compaction", tokens: 0 });
    else if (entry.type === "user" && entry.isCompactSummary) {
      pending.push({ kind: "compaction", tokens: estimate(JSON.stringify(message?.content ?? "").length) });
    } else if (entry.type === "user") {
      if (typeof message?.content === "string") pending.push({ kind: "prompt", tokens: estimate(message.content.length) });
      for (const block of content) {
        if (block?.type === "tool_result") {
          const tool = tools.get(block.tool_use_id) ?? { name: "?", input: {} };
          const { tokens, images } = contentTokens(block.content);
          const item = { kind: "tool", tool: tool.name, tokens, images };
          if (tool.name === "Read") item.path = shortPath(tool.input.file_path, cwd);
          if (tool.name === "Bash" || /device_bash$/.test(tool.name)) {
            item.command = commandKey(tool.input.command);
            item.line = String(tool.input.command ?? "").slice(0, 300); // for job.mjs's grouping by what was read
          }
          if (/^(Grep|Glob)$/.test(tool.name)) item.line = String(tool.input.pattern ?? "");
          if (tool.name === "Agent") item.line = String(tool.input.subagent_type ?? "");
          pending.push(item);
        } else if (block?.type === "image") {
          pending.push({ kind: "prompt", tokens: imageTokens(block.source?.data), images: 1 });
        } else if (typeof block?.text === "string") pending.push({ kind: "prompt", tokens: estimate(block.text.length) });
      }
    } else if (entry.type === "attachment" && entry.attachment && !UNSENT.has(entry.attachment.type)) {
      pending.push({ kind: "attachment", type: entry.attachment.type, tokens: estimate(JSON.stringify(entry.attachment).length) });
    }
  }
  return { file, session, agent, cwd, calls };
}

// Why a call wrote what it wrote to the cache, most likely.
export function cause(call, previous) {
  if (!previous) return { kind: "first turn", note: call.cache_read > 0 ? `${k(call.cache_read)} read from an earlier session` : "nothing cached" };
  if (call.added.some((a) => a.kind === "compaction")) return { kind: "compaction", note: "" };
  const gapMs = call.time !== null && previous.lastTime !== null ? call.time - previous.lastTime : 0;
  if (gapMs > TTL_MS[previous.ttl]) {
    // The gap includes the tools run between the two calls: a long build expires the cache too.
    const ran = call.added.find((a) => a.kind === "tool" && a.command) ?? call.added.find((a) => a.kind === "tool");
    return { kind: "cache expired", note: `${minutes(gapMs)} after the call before${ran ? `, ${describe(ran)}` : ""}, ${previous.ttl} cache` };
  }
  const before = previous.input + previous.cache_read + previous.cache_write;
  if (call.cache_read < before * 0.5) return { kind: "cache miss", note: `read ${k(call.cache_read)} of ${k(before)}` };
  // Otherwise the context grew by what was added since the call before, and by that call's
  // own output, thinking included.
  const split = { "tool results": 0, attachments: 0, prompts: 0, "previous output": previous.output };
  for (const a of call.added) split[a.kind === "tool" ? "tool results" : a.kind === "attachment" ? "attachments" : "prompts"] += a.tokens;
  const added = Object.values(split).reduce((x, y) => x + y, 0);
  const largest = [...call.added].sort((a, b) => b.tokens - a.tokens)[0];
  let note = `previous turn's output ~${k(previous.output)}`;
  if (largest && largest.tokens >= previous.output) {
    note = `${largest.kind === "tool" ? describe(largest) : largest.kind === "attachment" ? largest.type : "prompt"} ~${k(largest.tokens)}`;
  }
  return { kind: "growth", note: `${note} of ~${k(added)} added`, split, added };
}

function describe(item) {
  if (item.path) return `Read ${item.path}`;
  if (item.command) return `${item.tool === "Bash" ? "Bash" : item.tool.replace(/^mcp__/, "")} \`${item.command}\``;
  return item.tool;
}

function k(n) {
  if (n >= 1e6) return `${(n / 1e6).toFixed(n >= 1e7 ? 0 : 1)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(n >= 1e4 ? 0 : 1)}k`;
  return String(n);
}

function minutes(ms) {
  return ms >= 3600e3 ? `${(ms / 3600e3).toFixed(1)}h` : `${Math.round(ms / 60e3)}m`;
}

function median(values) {
  if (values.length === 0) return 0;
  const s = [...values].sort((a, b) => a - b);
  return s[Math.floor(s.length / 2)];
}

// The report, from parsed transcripts. `since` drops calls before it (ms); a call whose id an
// earlier transcript holds, copied into a resumed session, counts once.
export function analyse(parsed, { since = 0, cost = null, meta = new Map() } = {}) {
  const seen = new Set();
  const sessions = [];
  const writes = [];
  const causes = new Map();
  const groups = { tool: new Map(), path: new Map(), command: new Map(), attachment: new Map() };
  const images = { n: 0, tokens: 0 };
  const firsts = [];
  const sum = { calls: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, write5m: 0, cost: 0 };
  let estimated = 0;
  let grown = 0;
  const add = (map, key, tokens) => {
    const row = map.get(key) ?? { key, n: 0, tokens: 0, max: 0 };
    row.n += 1;
    row.tokens += tokens;
    row.max = Math.max(row.max, tokens);
    map.set(key, row);
  };
  for (const t of parsed) {
    const label = t.agent ? `agent ${t.agent.slice(0, 8)}` : (t.session || basename(t.file)).slice(0, 8);
    const contexts = [];
    let written = 0;
    let previous = null;
    for (const [i, call] of t.calls.entries()) {
      const counted = !seen.has(call.id) && (call.time ?? 0) >= since;
      seen.add(call.id);
      if (counted) {
        const ctx = call.input + call.cache_read + call.cache_write;
        contexts.push(ctx);
        written += call.cache_write;
        for (const key of ["input", "output", "cache_read", "cache_write"]) sum[key] += call[key];
        if (call.ttl === "5m") sum.write5m += call.cache_write;
        sum.calls += 1;
        if (cost) sum.cost += cost(call.model, call);
        const why = cause(call, previous);
        const count = (kind, tokens) => {
          const c = causes.get(kind) ?? { kind, n: 0, tokens: 0 };
          c.n += 1;
          c.tokens += tokens;
          causes.set(kind, c);
        };
        if (why.kind === "growth") {
          // Shared out by what each kind of addition is estimated at.
          for (const [kind, tokens] of Object.entries(why.split)) {
            if (tokens > 0) count(`growth: ${kind}`, (call.cache_write * tokens) / why.added);
          }
          grown += call.cache_write;
          estimated += why.added;
        } else count(why.kind, call.cache_write);
        writes.push({ who: label, turn: i + 1, tokens: call.cache_write, why });
        for (const a of call.added) {
          if (a.images) {
            images.n += a.images;
            images.tokens += a.tokens;
          }
          if (a.kind === "attachment") add(groups.attachment, a.type, a.tokens);
          if (a.kind !== "tool") continue;
          add(groups.tool, a.tool.replace(/^mcp__/, ""), a.tokens);
          if (a.path) add(groups.path, a.path, a.tokens);
          if (a.command) add(groups.command, a.command, a.tokens);
        }
        if (i === 0 && t.agent) {
          const m = meta.get(t.agent) ?? {};
          firsts.push({ who: label, type: m.agentType ?? "", what: m.description ?? "", model: call.model, ctx, read: call.cache_read, write: call.cache_write, ttl: call.ttl });
        }
      }
      previous = call;
    }
    if (contexts.length === 0) continue;
    const m = t.agent ? meta.get(t.agent) ?? {} : {};
    sessions.push({
      who: label,
      kind: t.agent ? m.agentType ?? "subagent" : "main",
      model: t.calls[0]?.model.replace(/^claude-/, "").replace(/-\d{8}$/, "") ?? "",
      turns: contexts.length,
      peak: Math.max(...contexts),
      median: median(contexts),
      written,
    });
  }
  const sorted = (map) => [...map.values()].sort((a, b) => b.tokens - a.tokens);
  return {
    sum,
    sessions: sessions.sort((a, b) => b.written - a.written),
    causes: sorted(causes),
    writes: writes.sort((a, b) => b.tokens - a.tokens),
    tools: sorted(groups.tool),
    paths: sorted(groups.path),
    commands: sorted(groups.command),
    attachments: sorted(groups.attachment),
    images,
    firsts: firsts.sort((a, b) => b.write - a.write),
    calibration: grown > 0 ? estimated / grown : null,
  };
}

function table(rows, cols, indent = "  ") {
  const cells = [cols.map((c) => c[0]), ...rows.map((r) => cols.map(([, f]) => String(f(r))))];
  const widths = cols.map((_, i) => Math.max(...cells.map((r) => r[i].length)));
  return cells.map((r) => indent + r.map((c, i) => (i === 0 ? c.padEnd(widths[i]) : c.padStart(widths[i]))).join("  ").trimEnd());
}

function clip(s, n) {
  return s.length > n ? s.slice(0, n - 1) + "…" : s;
}

// The report as at most 60 lines.
export function format(r, { files = 0 } = {}) {
  const s = r.sum;
  const out = [];
  const pct = (n, of) => (of > 0 ? `${Math.round((100 * n) / of)}%` : "-");
  out.push(`Context: ${files} transcripts, ${s.calls} API calls. Context per call = input + cache read + cache write.`);
  out.push(
    `Cache writes ${k(s.cache_write)} (${pct(s.write5m, s.cache_write)} at the 5-minute TTL), reads ${k(s.cache_read)}, output ${k(s.output)}` +
      (s.cost > 0 ? `; $${s.cost.toFixed(2)} at list price` : ""),
  );
  if (s.calls === 0) return out.join("\n");
  out.push("", `Sessions, by cache written (${Math.min(5, r.sessions.length)} of ${r.sessions.length})`);
  out.push(...table(r.sessions.slice(0, 5), [["who", (x) => x.who], ["kind", (x) => clip(x.kind, 16)], ["model", (x) => x.model], ["turns", (x) => x.turns], ["peak", (x) => k(x.peak)], ["median", (x) => k(x.median)], ["written", (x) => k(x.written)]]));
  out.push("", "Cache writes by likely cause");
  out.push(...table(r.causes, [["cause", (x) => x.kind], ["calls", (x) => x.n], ["written", (x) => k(Math.round(x.tokens))], ["share", (x) => pct(x.tokens, s.cache_write)]]));
  if (r.calibration !== null) out.push(`  (growth is shared out by estimates, ${CHARS_PER_TOKEN} characters a token, which come to ${Math.round(r.calibration * 100)}% of what it wrote)`);
  out.push("", "Largest cache writes");
  for (const w of r.writes.slice(0, 5)) out.push(`  ${k(w.tokens).padStart(5)}  ${w.who} turn ${w.turn}: ${clip(w.why.kind + (w.why.note ? `, ${w.why.note}` : ""), 80)}`);
  const top = (title, rows, n) => {
    if (rows.length === 0) return;
    out.push("", title);
    out.push(...table(rows.slice(0, n), [["", (x) => clip(x.key, 48)], ["n", (x) => x.n], ["tokens", (x) => k(x.tokens)], ["largest", (x) => k(x.max)]]));
  };
  top("Tool results by tool, estimated", r.tools, 3);
  top("Reads by path", r.paths, 3);
  top("Bash by command", r.commands, 3);
  if (r.images.n > 0) out.push(`  images: ${r.images.n}, ~${k(r.images.tokens)} tokens`);
  top("Added by Claude Code (attachments)", r.attachments, 3);
  if (r.firsts.length > 0) {
    out.push("", "Subagents' first turns (the prefix each writes)");
    out.push(...table(r.firsts.slice(0, 4), [["who", (x) => x.who], ["type", (x) => clip(x.type, 16)], ["prefix", (x) => k(x.ctx)], ["read", (x) => k(x.read)], ["written", (x) => k(x.write)], ["ttl", (x) => x.ttl], ["task", (x) => clip(x.what, 28)]]));
  }
  return out.slice(0, 60).join("\n");
}

function jsonlFiles(path) {
  const st = statSync(path);
  if (!st.isDirectory()) return [path];
  const files = [];
  for (const item of readdirSync(path, { withFileTypes: true })) {
    const p = join(path, item.name);
    if (item.isDirectory()) files.push(...jsonlFiles(p));
    else if (item.name.endsWith(".jsonl")) files.push(p);
  }
  return files;
}

// A subagent's type and task, from the .meta.json Claude Code keeps beside its transcript.
function readMeta(files) {
  const meta = new Map();
  for (const f of files) {
    const m = basename(f).match(/^agent-(.+)\.jsonl$/);
    const metaFile = f.replace(/\.jsonl$/, ".meta.json");
    if (!m || !existsSync(metaFile)) continue;
    try {
      meta.set(m[1], JSON.parse(readFileSync(metaFile, "utf8")));
    } catch {
      // A transcript without its meta still counts.
    }
  }
  return meta;
}

async function main(args) {
  if (args.includes("--hook")) return hook();
  const value = (flag) => {
    const i = args.indexOf(flag);
    return i >= 0 ? args[i + 1] : undefined;
  };
  const days = Number(value("--days") ?? 0);
  const sessionId = value("--session");
  const paths = args.filter((a, i) => !a.startsWith("--") && !["--days", "--session"].includes(args[i - 1]));
  let files;
  let roots = null;
  let projects = null;
  if (paths.length > 0) files = paths.flatMap((p) => jsonlFiles(resolve(p)));
  else {
    const report = await import("./report.mjs");
    roots = report.roots();
    projects = report.projectsDir();
    files = report.transcripts(roots);
  }
  files.sort((a, b) => statSync(a).mtimeMs - statSync(b).mtimeMs);
  let parsed = files.map((f) => parseTranscript(readFileSync(f, "utf8"), f));
  if (roots) parsed = parsed.filter((t) => roots.some((r) => t.cwd === r || t.cwd.startsWith(r + "/")));
  if (roots && parsed.length === 0) {
    // Only sessions started inside the repository count by default: say where it looked,
    // so a session started elsewhere is not taken for no sessions at all.
    console.log(
      `No transcripts found: looked in ${projects} for sessions started in ${roots.join(" or ")}. ` +
        `For a session started elsewhere, pass its transcripts or their folder ` +
        `(node tools/token-stats/context.mjs ${join(projects, "<folder>")}), or set CLAUDE_CONFIG_DIR to the Claude Code folder holding them.`,
    );
    return;
  }
  if (sessionId) parsed = parsed.filter((t) => t.session.startsWith(sessionId));
  const { cost } = await import("./worker/src/prices.js");
  const since = days > 0 ? Date.now() - days * 86400e3 : 0;
  console.log(format(analyse(parsed, { since, cost, meta: readMeta(files) }), { files: parsed.length }));
}

// --- The context nudge -------------------------------------------------------------------

// The context of the last API call in a transcript's tail, or null if the tail has none.
export function lastContext(tail) {
  const lines = tail.split("\n");
  for (let i = lines.length - 1; i >= 0; i--) {
    const line = lines[i];
    if (!line.includes('"usage"') || !line.includes('"assistant"')) continue;
    try {
      const e = JSON.parse(line);
      const u = e?.message?.usage;
      if (e.type !== "assistant" || !u) continue;
      return (u.input_tokens ?? 0) + (u.cache_read_input_tokens ?? 0) + (u.cache_creation_input_tokens ?? 0);
    } catch {
      // A line cut by the tail's start.
    }
  }
  return null;
}

// The context bound a brief names, in tokens, or null: the first "<n>k context" (or
// "context of about <n>k") in a sentence that says bound or stop.
export function briefBound(prompt) {
  if (typeof prompt !== "string") return null;
  for (const sentence of prompt.split(/(?<=[.;])\s+|\n/)) {
    if (!/\b(bound|stop)/i.test(sentence)) continue;
    const m = sentence.match(/(\d{2,3})\s*k\s+(?:tokens\s+)?(?:of\s+)?context|context\s+(?:of\s+)?(?:about\s+)?(\d{2,3})\s*k\b/i);
    if (m) return Number(m[1] ?? m[2]) * 1000;
  }
  return null;
}

// The text of a transcript's first prompt, from its head, or null.
export function firstPrompt(head) {
  for (const line of head.split("\n")) {
    if (!line.includes('"user"')) continue;
    try {
      const e = JSON.parse(line);
      if (e.type !== "user") continue;
      const c = e.message?.content;
      if (typeof c === "string") return c;
      if (Array.isArray(c)) return c.map((b) => (b?.type === "text" ? b.text : "")).join("\n");
    } catch {
      // A line cut by the head's end.
    }
  }
  return null;
}

// Which nudge a context has reached: -1 below the warning, 0 the warning, 1 the bound, then
// one more for each STEP_PAST past it.
export function level(context, bound = DEFAULT_BOUND) {
  if (context < bound * WARN_AT) return -1;
  if (context < bound) return 0;
  return 1 + Math.floor((context - bound) / STEP_PAST);
}

export function nudgeText(context, bound = DEFAULT_BOUND, { job = false } = {}) {
  const at = `Context ${Math.round(context / 1000)}k of your ${Math.round(bound / 1000)}k bound (D627)`;
  if (context < bound) {
    return job
      ? `${at}: finish and land the step you are on, start no new one, then write the handoff (with job.mjs's numbers) and report.`
      : `${at}: hold no file dumps (finder and runner read, you take their reports), start no new line of work, and get the lead handoff ready.`;
  }
  return job
    ? `STOP: ${at}, past it. Do not read another file or start another step: commit what is checked, write the handoff (what landed, ` +
        `job.mjs's numbers, what is left in order), report and end now. Unchecked work stays in the worktree, named in the handoff.`
    : `STOP: ${at}, past it. Start no new job: write the lead handoff (~/.cache/terra-handoff/<topic>-lead.md: what runs, what landed, ` +
        `what is next), then ask the developer to /clear and continue from it.`;
}

// Decides whether to nudge, given the context now and the level last seen; returns the
// level to keep and the text to send, if any. A crossing nudges once; a context that
// shrinks (after compaction) is nudged again when it grows back past a threshold.
export function decide(context, seen, bound = DEFAULT_BOUND, { job = false } = {}) {
  const now = level(context, bound);
  return { keep: now, text: now > seen ? nudgeText(context, bound, { job }) : null };
}

// The transcript a hook's call is about: a subagent's own when it runs inside one, since
// Claude Code passes the session's transcript_path with the subagent's agent_id. A workflow's
// agents keep theirs a level down, in subagents/workflows/<run>/; without looking there a
// workflow job read its coordinator's context as its own and stopped at its first call.
export function transcriptFor(input) {
  const path = input?.transcript_path;
  if (typeof path !== "string") return null;
  if (typeof input.agent_id === "string" && /^[A-Za-z0-9_-]+$/.test(input.agent_id)) {
    const subagents = join(dirname(path), basename(path, ".jsonl"), "subagents");
    const name = `agent-${input.agent_id}.jsonl`;
    const own = join(subagents, name);
    if (own !== path && existsSync(own)) return own;
    const workflows = join(subagents, "workflows");
    if (existsSync(workflows)) {
      for (const run of readdirSync(workflows)) {
        const inRun = join(workflows, run, name);
        if (existsSync(inRun)) return inRun;
      }
    }
  }
  return path;
}

function readTail(path) {
  const fd = openSync(path, "r");
  try {
    const size = fstatSync(fd).size;
    // A tool call's line can be long; widen the tail until it holds an API call's usage.
    for (let want = 64 * 1024; ; want *= 4) {
      const n = Math.min(size, want);
      const buf = Buffer.alloc(n);
      readSync(fd, buf, 0, n, size - n);
      const context = lastContext(buf.toString("utf8"));
      if (context !== null || n === size || want >= 4 * 1024 * 1024) return context;
    }
  } finally {
    closeSync(fd);
  }
}

function readHead(path) {
  const fd = openSync(path, "r");
  try {
    const buf = Buffer.alloc(Math.min(fstatSync(fd).size, 256 * 1024));
    readSync(fd, buf, 0, buf.length, 0);
    return buf.toString("utf8");
  } finally {
    closeSync(fd);
  }
}

// Run by Claude Code after every tool call, so it must be quick and never fail: it reads
// only the transcript's tail and one small state file.
function hook() {
  try {
    const input = JSON.parse(readFileSync(0, "utf8"));
    const path = transcriptFor(input);
    if (!path || !existsSync(path)) return;
    const context = readTail(path);
    if (context === null) return;
    const key = `${input.session_id ?? basename(path)}-${input.agent_id ?? "main"}`.replace(/[^A-Za-z0-9_-]/g, "-");
    const state = join(NUDGE_STATE, key);
    const job = typeof input.agent_id === "string";
    let kept = null;
    try {
      kept = JSON.parse(readFileSync(state, "utf8"));
    } catch {
      // First call of this session.
    }
    if (!kept || !Number.isInteger(kept.seen) || !(kept.bound > 0)) {
      // A job's bound is in its brief, read once from the transcript's head and kept.
      kept = { seen: Number.isInteger(kept) ? kept : -1, bound: (job && briefBound(firstPrompt(readHead(path)))) || DEFAULT_BOUND };
    }
    const { keep, text } = decide(context, kept.seen, kept.bound, { job });
    if (keep !== kept.seen || !existsSync(state)) {
      mkdirSync(NUDGE_STATE, { recursive: true });
      writeFileSync(state, JSON.stringify({ seen: keep, bound: kept.bound }));
    }
    if (text) process.stdout.write(JSON.stringify({ hookSpecificOutput: { hookEventName: "PostToolUse", additionalContext: text } }));
  } catch {
    // Never in the way of the work.
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main(process.argv.slice(2)).catch((err) => {
    console.error(`context: ${err.message}`);
    process.exit(process.argv.includes("--hook") ? 0 : 1);
  });
}
