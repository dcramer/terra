// Run with `node --test tools/token-stats/report.test.mjs`.
import assert from "node:assert/strict";
import { sep } from "node:path";
import { test } from "node:test";
import { addTranscript, parseEnv, readSpends, spendTotals, totals } from "./report.mjs";

const ROOT = `${sep}src${sep}terra`;
const ROOTS = [ROOT];

function line({ id = "msg_1", cwd = ROOT, session = "s1", model = "claude-opus-5-5", day = "2026-10-10", type = "assistant", output = 30 } = {}) {
  return JSON.stringify({
    type,
    cwd,
    sessionId: session,
    timestamp: `${day}T12:00:00.000Z`,
    message: {
      id,
      model,
      usage: { input_tokens: 2, output_tokens: output, cache_read_input_tokens: 100, cache_creation_input_tokens: 10 },
    },
  });
}

function run(...transcripts) {
  const messages = new Map();
  for (const lines of transcripts) addTranscript(lines.join("\n"), ROOTS, messages);
  return [...messages.values()];
}

test("counts a message logged once per content block once, at its final usage", () => {
  const messages = run([line({ output: 1 }), line({ output: 333 }), line({ output: 1 })]);
  assert.deepEqual(messages, [
    { id: "msg_1", session: "s1", day: "2026-10-10", model: "claude-opus-5-5", input: 2, output: 333, cache_read: 100, cache_write: 10 },
  ]);
});

test("counts a message copied into a resumed session once, in whichever order they are read", () => {
  const original = [line({ session: "s1", day: "2026-10-09" })];
  const resumed = [line({ session: "s2", day: "2026-10-10" }), line({ id: "msg_2", session: "s2" })];
  for (const order of [[original, resumed], [resumed, original]]) {
    const messages = run(...order);
    assert.equal(messages.length, 2);
    assert.deepEqual(messages.find((m) => m.id === "msg_1"), run(original)[0]);
  }
});

test("counts a session by where it started, wherever it went after", () => {
  const worktree = `${ROOT}${sep}.claude${sep}worktrees${sep}x`;
  const elsewhere = `${sep}src${sep}other`;
  assert.equal(run([line({ id: "a", cwd: worktree }), line({ id: "b", cwd: elsewhere })]).length, 2);
  assert.equal(run([line({ id: "a", cwd: elsewhere }), line({ id: "b", cwd: ROOT })]).length, 0);
  assert.equal(run([line({ cwd: `${ROOT}form` })]).length, 0);
});

test("skips user lines, synthetic messages, messages without an id and junk", () => {
  const noId = JSON.stringify({ type: "assistant", cwd: ROOT, timestamp: "2026-10-10T00:00:00Z", message: { model: "m", usage: {} } });
  assert.deepEqual(run([line({ type: "user" }), line({ id: "s", model: "<synthetic>" }), noId, '{"usage": nope']), []);
});

test("totals by day and model", () => {
  const rows = totals(run([line(), line({ id: "b" }), line({ id: "c", day: "2026-10-11" })]));
  assert.deepEqual(rows.map((r) => [r.day, r.messages, r.output]), [["2026-10-10", 2, 60], ["2026-10-11", 1, 30]]);
});

test("reads a .env file", () => {
  const text = '# comment\nTERRA_STATS_URL=https://x.dev/ # trailing\n\nexport TERRA_STATS_KEY="a b#c"\nTERRA_STATS_USER = dcramer\r\nnot a line\n';
  assert.deepEqual(parseEnv(text), {
    TERRA_STATS_URL: "https://x.dev/",
    TERRA_STATS_KEY: "a b#c",
    TERRA_STATS_USER: "dcramer",
  });
});

test("reads each recorded task once, at its largest count and earliest day", () => {
  const spend = (id, day, credits, service = "tripo-studio") => JSON.stringify({ id, day, service, credits });
  const text = [
    spend("tripo-studio-a", "2026-10-11", 35),
    spend("tripo-studio-a", "2026-10-10", 20),
    spend("tripo-api-b", "2026-10-11", 50, "tripo-api"),
    '{"id": "broken"',
    JSON.stringify({ id: "c", day: "2026-10-11", service: "tripo-api", credits: 1.5 }),
    "",
  ].join("\n");
  const spends = readSpends(text);
  assert.deepEqual(spends, [
    { id: "tripo-studio-a", day: "2026-10-10", service: "tripo-studio", credits: 35 },
    { id: "tripo-api-b", day: "2026-10-11", service: "tripo-api", credits: 50 },
  ]);
  assert.deepEqual(spendTotals(spends), [
    { day: "2026-10-10", service: "tripo-studio", credits: 35, tasks: 1 },
    { day: "2026-10-11", service: "tripo-api", credits: 50, tasks: 1 },
  ]);
});
