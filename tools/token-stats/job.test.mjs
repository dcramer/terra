// Run with `node --test tools/token-stats/job.test.mjs`. The transcript is made up, in the shape
// Claude Code writes it, with numbers worked out by hand.
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { parseTranscript } from "./context.mjs";
import { findJobs, format, mean, measure, readKind } from "./job.mjs";

const CWD = "/src/terra/.claude/worktrees/agent-1";
let minute = 0;
const stamp = () => new Date(Date.parse("2026-11-05T12:00:00Z") + minute++ * 60e3).toISOString();

// One API call of `context` tokens (all of it read from the cache but the last `write`), its
// output, and the tools it asked for.
function call(id, context, output, uses = []) {
  return JSON.stringify({
    type: "assistant",
    cwd: CWD,
    timestamp: stamp(),
    message: {
      id,
      model: "claude-opus-5-5",
      content: uses.length ? uses.map(([tid, name, input]) => ({ type: "tool_use", id: tid, name, input })) : [{ type: "text", text: "ok" }],
      usage: { input_tokens: 0, output_tokens: output, cache_read_input_tokens: context - 100, cache_creation_input_tokens: 100 },
    },
  });
}
const result = (id, chars) => JSON.stringify({ type: "user", cwd: CWD, timestamp: stamp(), message: { content: [{ type: "tool_result", tool_use_id: id, content: "x".repeat(chars) }] } });

// Prefix 10,000. Turn 1 reads the skill (estimated 1,000) and greps (500); the context then grows
// by 3,000 past turn 1's output of 200, which shares out 2,000 and 1,000. Turn 2 edits; its
// result grows the context by 50 past its output.
const lines = [
  call("m1", 10000, 200, [["t1", "Read", { file_path: `${CWD}/.agents/skills/character/SKILL.md` }], ["t2", "Bash", { command: "grep -rn wardrobe tools/characters" }]]),
  result("t1", 4000),
  result("t2", 2000),
  call("m2", 13200, 400, [["t3", "Edit", { file_path: `${CWD}/tools/characters/wardrobe.ts` }]]),
  result("t3", 40),
  call("m3", 13650, 100),
];

test("measures the context at the first edit, the summed input and what the reads before it carried", () => {
  const m = measure(parseTranscript(lines.join("\n")));
  assert.equal(m.turns, 3);
  assert.equal(m.prefix, 10000);
  assert.equal(m.firstEdit, 13200, "the context of the call that asked for the edit");
  assert.equal(m.firstEditTurn, 2);
  assert.equal(m.summed, 10000 + 13200 + 13650);
  assert.equal(m.output, 700);
  // 13,200 - 10,000 - 200 = 3,000 came back, shared 2:1 by the estimates (1,000 and 500).
  const by = Object.fromEntries(m.groups.map((g) => [g.what, g.tokens]));
  assert.deepEqual(by, { "orientation docs": 2000, search: 1000, edits: 50 });
  assert.equal(m.results, 3050);
  assert.equal(m.readBeforeEdit, 3000);
  // Both were in the context of turns 2 and 3.
  assert.equal(m.carriedBeforeEdit, 3000 * 2);
  assert.ok(Math.abs(m.calibration - 3050 / 1510) < 1e-9, "measured over estimated");
});

test("groups a result by what was read, as the research's audit did", () => {
  const tool = (t, extra) => ({ kind: "tool", tool: t, tokens: 1, ...extra });
  assert.equal(readKind(tool("Read", { path: "crates/terra_walker/src/wardrobe.rs" })), "our source");
  assert.equal(readKind(tool("Read", { path: "docs/v0-characters.md" })), "orientation docs");
  assert.equal(readKind(tool("Bash", { line: "sed -n 1,40p ~/.cache/terra-handoff/x.md" })), "orientation docs");
  assert.equal(readKind(tool("Bash", { line: "sed -n 80,120p tools/characters/slots.ts" })), "our source");
  assert.equal(readKind(tool("Bash", { line: "tail -5 target/check/clippy.log" })), "tool output and logs");
  assert.equal(readKind(tool("Bash", { line: "cd /x && git show HEAD" })), "git");
  assert.equal(readKind(tool("Bash", { line: "ls tools/characters" })), "search");
  assert.equal(readKind(tool("Grep", {})), "search");
  assert.equal(readKind(tool("Agent", { line: "finder" })), "search");
  assert.equal(readKind(tool("Bash", { line: "node tools/characters/wardrobe.ts" })), "tool output and logs");
  assert.equal(readKind(tool("Read", { path: "/tmp/a.png", images: 1 })), "images");
});

test("finds a job's transcript by its task, and prints the mean of several", () => {
  const root = mkdtempSync(join(tmpdir(), "job-"));
  const sub = join(root, "proj", "session", "subagents");
  mkdirSync(sub, { recursive: true });
  for (const [id, task] of [["a1", "char-pipeline j1: catalogue"], ["a2", "char-pipeline j2: outfits"], ["a3", "combat j4"]]) {
    writeFileSync(join(sub, `agent-${id}.jsonl`), lines.join("\n"));
    writeFileSync(join(sub, `agent-${id}.meta.json`), JSON.stringify({ agentType: "job", description: task }));
  }
  const brief = JSON.stringify({ type: "user", message: { content: "Task: char-pipeline j2. Read the handoff.\nBound: 150k context." } });
  writeFileSync(join(sub, "agent-a4.jsonl"), [brief, ...lines].join("\n"));
  writeFileSync(join(sub, "agent-a4.meta.json"), JSON.stringify({ agentType: "job", description: "Character outfits again" }));
  const jobs = findJobs(root, "char-pipeline j[12]");
  assert.deepEqual(jobs.map((j) => j.name).sort(), ["Character outfits again", "char-pipeline j1: catalogue", "char-pipeline j2: outfits"], "by the description or the brief's first line");
  const rows = jobs.map((j) => ({ name: j.name, m: measure(parseTranscript(lines.join("\n"))) }));
  assert.equal(mean(rows.map((r) => r.m)).firstEdit, 13200);
  const text = format(rows);
  assert.match(text, /^mean\s+3\s+13k/m);
  assert.ok(text.split("\n").length <= 20, "short enough to read every job");
});

test("finds a workflow's job, kept a level down under subagents/workflows/<run>/", () => {
  const root = mkdtempSync(join(tmpdir(), "job-"));
  const run = join(root, "proj", "session", "subagents", "workflows", "wf_1-2");
  mkdirSync(run, { recursive: true });
  writeFileSync(join(run, "agent-w1.jsonl"), lines.join("\n"));
  writeFileSync(join(run, "agent-w1.meta.json"), JSON.stringify({ agentType: "job", description: "wasteland j2" }));
  assert.deepEqual(findJobs(root, "wasteland j2").map((j) => j.name), ["wasteland j2"]);
});
