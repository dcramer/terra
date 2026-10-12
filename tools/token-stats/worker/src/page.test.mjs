import assert from "node:assert/strict";
import { test } from "node:test";
import { ledger, page, stack } from "./page.js";

const day = (d, claude, tripo) => ({
  day: d, input: 0, output: 10, cache_read: 0, cache_write: 0, messages: 1, total: 10,
  cost: claude + tripo, claude_cost: claude, tripo_cost: tripo, tripo_credits: tripo * 150,
});
const person = (user, claude, tripo) => ({
  user, ...day("2026-10-11", claude, tripo),
  models: [{ model: "claude-opus-5-5", cost: claude }, ...(tripo ? [{ model: "tripo-studio", credits: tripo * 150, cost: tripo }] : [])],
  daily: [day("2026-10-10", 0, 0), day("2026-10-11", claude, tripo)],
});
const summary = {
  generated_at: "2026-10-11T23:00:00.000Z",
  days: 2,
  totals: day("2026-10-11", 30, 2),
  users: [person("ana", 30, 2), person("ben", 5, 0)],
};

test("breaks a total down into Claude, with its tokens, and Tripo, with its credits", () => {
  const html = ledger(summary.users[0], summary.users[0].models);
  assert.match(html, /Claude <small><span title="\$30\.00 at API prices">opus-5-5<\/span><\/small>/);
  assert.match(html, /data-key="claude_cost"[^>]*>\$30\.00<\/b>/);
  assert.match(html, /data-key="output"[^>]*>10<\/b> output tokens/);
  assert.match(html, /data-key="messages"[^>]*>1<\/b> messages/);
  assert.match(html, /Tripo <small><span title="300 credits, \$2\.00">studio<\/span><\/small>/);
  assert.match(html, /data-key="tripo_cost"[^>]*>\$2\.00<\/b>/);
  assert.match(html, /data-key="tripo_credits"[^>]*>300<\/b> credits/);
});

test("leaves Tripo out of a total that has none", () => {
  const html = ledger(summary.users[1], summary.users[1].models);
  assert.match(html, /class="line claude"/);
  assert.doesNotMatch(html, /class="line tripo"/);
});

test("breaks down the total and everyone's, and records the layout", () => {
  const html = page(summary);
  assert.equal(html.match(/class="ledger"/g).length, 3);
  assert.equal(html.match(/class="line tripo"/g).length, 2);
  assert.match(html, /<main data-shape="\+ 2026-10-10 ana\+ ben">/);
});

test("stacks Tripo's cost on Claude's in a day's bar, rounding only the top", () => {
  assert.equal(stack(day("d", 3, 1), 4), '<i class="claude" style="height:75.0%"></i><i class="tripo top" style="height:25.0%"></i>');
  assert.equal(stack(day("d", 4, 0), 4), '<i class="claude top" style="height:100.0%"></i><i class="tripo top" style="height:0.0%"></i>');
});

test("says what Terra is and what the numbers count, with a link to the project", () => {
  const html = page(summary);
  assert.match(html, /<section class="about"[^>]*>[\s\S]*<strong>Terra<\/strong> is a multiplayer survival sandbox/);
  assert.match(html, /not what anyone paid/);
  assert.match(html, /<a href="https:\/\/github\.com\/dcramer\/terra">/);
});
