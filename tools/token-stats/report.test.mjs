// Run with `node --test tools/token-stats/report.test.mjs`.
import assert from "node:assert/strict";
import { sep } from "node:path";
import { test } from "node:test";
import { addLine } from "./report.mjs";

const ROOT = `${sep}src${sep}terra`;

function line({ id = "msg_1", cwd = ROOT, model = "claude-opus-5-5", day = "2026-10-10", type = "assistant" } = {}) {
  return JSON.stringify({
    type,
    cwd,
    timestamp: `${day}T12:00:00.000Z`,
    requestId: `req_${id}`,
    message: {
      id,
      model,
      usage: { input_tokens: 2, output_tokens: 30, cache_read_input_tokens: 100, cache_creation_input_tokens: 10 },
    },
  });
}

function run(lines) {
  const seen = new Set();
  const totals = new Map();
  for (const l of lines) addLine(l, ROOT, seen, totals);
  return [...totals.values()];
}

test("counts a message once however often it is logged", () => {
  assert.deepEqual(run([line(), line(), line({ id: "msg_2" })]), [
    { day: "2026-10-10", model: "claude-opus-5-5", input: 4, output: 60, cache_read: 200, cache_write: 20, messages: 2 },
  ]);
});

test("splits by day and model", () => {
  const rows = run([line(), line({ id: "b", day: "2026-10-11" }), line({ id: "c", model: "claude-haiku-5-5" })]);
  assert.equal(rows.length, 3);
});

test("counts worktrees and skips other folders", () => {
  const rows = run([
    line({ id: "a", cwd: `${ROOT}${sep}.claude${sep}worktrees${sep}x` }),
    line({ id: "b", cwd: `${ROOT}form` }),
    line({ id: "c", cwd: `${sep}elsewhere` }),
  ]);
  assert.equal(rows[0].messages, 1);
});

test("skips user lines, synthetic messages and junk", () => {
  assert.deepEqual(run([line({ type: "user" }), line({ id: "s", model: "<synthetic>" }), '{"usage": nope']), []);
});
