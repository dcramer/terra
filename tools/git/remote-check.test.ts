import assert from "node:assert/strict";
import { test } from "node:test";
import { enabled, pushed } from "./remote-check.ts";

const a = "a".repeat(40);
const b = "b".repeat(40);
const zero = "0".repeat(40);

test("the commits a push sends: each ref's local sha once, deletions left out", () => {
  const stdin = `refs/heads/x ${a} refs/heads/main ${b}\nHEAD ${a} refs/heads/other ${zero}\n(delete) ${zero} refs/heads/gone ${b}\n`;
  assert.deepEqual(pushed(stdin), [a]);
  assert.deepEqual(pushed(""), []);
});

test("on only when TERRA_REMOTE_CHECKS is 1, the environment over .env", () => {
  assert.ok(enabled("/", { TERRA_REMOTE_CHECKS: "1" }));
  assert.ok(!enabled("/", { TERRA_REMOTE_CHECKS: "0" }));
  assert.ok(!enabled("/", { TERRA_REMOTE_CHECKS: "yes" }));
});
