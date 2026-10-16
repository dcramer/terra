// Run with `node --test tools/token-stats/context.test.mjs`. The transcripts here are made up,
// in the shape Claude Code writes them.
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { analyse, cause, commandKey, decide, format, imageTokens, lastContext, parseTranscript, shortPath, transcriptFor } from "./context.mjs";

const T0 = Date.parse("2026-10-16T12:00:00.000Z");
const at = (minutes) => new Date(T0 + minutes * 60e3).toISOString();
const CWD = "/src/terra/.claude/worktrees/agent-1";

// An API call, logged once per content block as Claude Code does.
function call(id, minute, { read = 0, write = 0, output = 10, ttl = "1h", blocks = [{ type: "text", text: "ok" }], agent } = {}) {
  return blocks.map((block) =>
    JSON.stringify({
      type: "assistant",
      sessionId: "s1",
      cwd: CWD,
      ...(agent ? { agentId: agent, isSidechain: true } : {}),
      timestamp: at(minute),
      message: {
        id,
        model: "claude-opus-5-5",
        content: [block],
        usage: {
          input_tokens: 2,
          output_tokens: output,
          cache_read_input_tokens: read,
          cache_creation_input_tokens: write,
          cache_creation: { ephemeral_5m_input_tokens: ttl === "5m" ? write : 0, ephemeral_1h_input_tokens: ttl === "1h" ? write : 0 },
        },
      },
    }),
  );
}

const use = (id, name, input) => ({ type: "tool_use", id, name, input });

function result(id, minute, text) {
  return JSON.stringify({
    type: "user",
    sessionId: "s1",
    cwd: CWD,
    timestamp: at(minute),
    message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: text }] },
  });
}

const attachment = (minute, type, chars) => JSON.stringify({ type: "attachment", timestamp: at(minute), attachment: { type, content: "x".repeat(chars) } });

test("one API call logged per content block is one call, with its tool results paired to their tools", () => {
  const lines = [
    ...call("m1", 0, { write: 20000, blocks: [{ type: "thinking", thinking: "" }, use("t1", "Read", { file_path: `${CWD}/docs/tech.md` }), use("t2", "Bash", { command: "cd /x && cargo xtask check --quick 2>&1 | tail" })] }),
    result("t1", 0.1, "r".repeat(4000)),
    result("t2", 0.2, "b".repeat(8000)),
    attachment(0.2, "prompt_snapshot", 400000), // logged, not sent
    attachment(0.2, "nested_memory", 400),
    ...call("m2", 0.5, { read: 20002, write: 3200 }),
  ];
  const t = parseTranscript(lines.join("\n"));
  assert.equal(t.calls.length, 2);
  const added = t.calls[1].added;
  assert.deepEqual(
    added.map((a) => [a.kind, a.tool ?? a.type, a.path ?? a.command, a.tokens > 0]),
    [
      ["tool", "Read", "docs/tech.md", true],
      ["tool", "Bash", "cargo xtask check", true],
      ["attachment", "nested_memory", undefined, true],
    ],
  );
  assert.equal(added[0].tokens, 1000);
  assert.equal(added[1].tokens, 2000);
});

test("names the likely cause of a cache write", () => {
  const prev = { input: 2, cache_read: 100000, cache_write: 5000, output: 300, ttl: "1h", lastTime: T0 };
  const next = (minutes, extra = {}) => ({ time: T0 + minutes * 60e3, cache_read: 105000, cache_write: 2000, added: [], ...extra });
  assert.equal(cause(next(1), null).kind, "first turn");
  assert.equal(cause(next(1, { added: [{ kind: "compaction", tokens: 0 }] }), prev).kind, "compaction");
  assert.equal(cause(next(61), prev).kind, "cache expired");
  assert.equal(cause(next(10), prev).kind, "growth", "an hour's cache outlives ten idle minutes");
  assert.equal(cause(next(10), { ...prev, ttl: "5m" }).kind, "cache expired", "a subagent's five-minute cache does not");
  const build = cause(next(8, { added: [{ kind: "tool", tool: "Bash", command: "cargo xtask check", tokens: 50 }] }), { ...prev, ttl: "5m" });
  assert.equal(build.kind, "cache expired", "a long tool run expires a five-minute cache as idling does");
  assert.match(build.note, /cargo xtask check/, "and the note names what ran");
  assert.equal(cause(next(1, { cache_read: 0 }), prev).kind, "cache miss");
  const grown = cause(next(1, { added: [{ kind: "tool", tool: "Bash", command: "cargo xtask check", tokens: 1700 }] }), prev);
  assert.equal(grown.kind, "growth");
  assert.deepEqual(grown.split, { "tool results": 1700, attachments: 0, prompts: 0, "previous output": 300 });
  assert.match(grown.note, /cargo xtask check/);
});

test("shares growth by what was added, counts a call copied into a resumed session once, and lists subagents' first turns", () => {
  const main = parseTranscript(
    [
      ...call("m1", 0, { write: 30000, output: 1000, blocks: [use("t1", "Bash", { command: "git log" })] }),
      result("t1", 0.1, "g".repeat(12000)), // 3000 tokens, against 1000 of output
      ...call("m2", 0.5, { read: 30002, write: 4000 }),
    ].join("\n"),
  );
  const resumed = parseTranscript([...call("m2", 0.5, { read: 30002, write: 4000 }), ...call("m3", 2, { read: 34002, write: 500, output: 600 })].join("\n"));
  const sub = parseTranscript(call("a1", 1, { write: 70000, ttl: "5m", agent: "abc" }).join("\n"));
  const r = analyse([main, resumed, sub], { meta: new Map([["abc", { agentType: "finder", description: "Find the seed" }]]) });
  assert.equal(r.sum.calls, 4);
  assert.equal(r.sum.cache_write, 30000 + 4000 + 500 + 70000);
  assert.equal(r.sum.write5m, 70000);
  const share = Object.fromEntries(r.causes.map((c) => [c.kind, c.tokens]));
  assert.equal(share["growth: tool results"], 3000);
  assert.equal(share["growth: previous output"], 1000 + 500, "m3 follows the copy of m2 it was resumed after");
  assert.equal(share["first turn"], 30000 + 70000);
  assert.deepEqual(r.firsts.map((f) => [f.who, f.type, f.write, f.ttl]), [["agent abc", "finder", 70000, "5m"]]);
  assert.deepEqual(r.commands.map((c) => [c.key, c.tokens]), [["git log", 3000]]);
  assert.ok(format(r, { files: 3 }).split("\n").length <= 60);
});

test("groups commands and paths the way they are run and read", () => {
  assert.equal(commandKey("cd /a && FOO=1 cargo xtask sandbox --shot x.png"), "cargo xtask sandbox");
  assert.equal(commandKey("timeout 600 cargo test -p terra_build"), "cargo test");
  assert.equal(commandKey("node tools/sandbox/look.ts --all | tail"), "node look.ts");
  assert.equal(commandKey("grep -n foo bar"), "grep");
  assert.equal(shortPath("/home/x/terra/.claude/worktrees/agent-9/crates/a.rs"), "crates/a.rs");
  assert.equal(shortPath("/src/terra/AGENTS.md", "/src/terra"), "AGENTS.md");
});

test("prices an image by its size", () => {
  const png = Buffer.alloc(32);
  png.writeUInt32BE(0x89504e47, 0);
  png.writeUInt32BE(1000, 16);
  png.writeUInt32BE(600, 20);
  assert.equal(imageTokens(png.toString("base64")), 800);
  png.writeUInt32BE(3136, 16); // scaled down to 1568 wide
  png.writeUInt32BE(1568, 20);
  assert.equal(imageTokens(png.toString("base64")), Math.ceil((1568 * 784) / 750));
});

test("nudges once per threshold crossed, and again when the context grows back after compaction", () => {
  let seen = -1;
  const step = (context) => {
    const { keep, text } = decide(context, seen);
    seen = keep;
    return text !== null;
  };
  assert.deepEqual(
    [150e3, 201e3, 260e3, 299e3, 301e3, 320e3, 60e3, 205e3, 520e3].map(step),
    [false, true, false, false, true, false, false, true, true],
  );
  const { text } = decide(201e3, -1);
  assert.match(text, /201k tokens/);
  assert.ok(!text.includes("\n"), "one line");
});

test("reads the last call's context from a tail that starts mid-line", () => {
  const lines = [...call("m1", 0, { read: 1000, write: 10 }), result("t", 1, "x"), ...call("m2", 2, { read: 250000, write: 3000 }), result("t", 3, "y")];
  const tail = lines.join("\n").slice(40);
  assert.equal(lastContext(tail), 2 + 250000 + 3000);
  assert.equal(lastContext(result("t", 1, "x")), null);
});

test("a hook inside a subagent reads the subagent's own transcript", () => {
  const dir = mkdtempSync(join(tmpdir(), "context-"));
  const session = join(dir, "s1.jsonl");
  writeFileSync(session, "");
  assert.equal(transcriptFor({ transcript_path: session }), session);
  assert.equal(transcriptFor({ transcript_path: session, agent_id: "abc" }), session, "falls back while the subagent has none");
  mkdirSync(join(dir, "s1", "subagents"), { recursive: true });
  writeFileSync(join(dir, "s1", "subagents", "agent-abc.jsonl"), "");
  assert.equal(transcriptFor({ transcript_path: session, agent_id: "abc" }), join(dir, "s1", "subagents", "agent-abc.jsonl"));
});
